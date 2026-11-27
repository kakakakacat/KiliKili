// Copyright 2014 The Flutter Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'dart:ui';

import 'package:flutter/gestures.dart' show kTouchSlop;
import 'package:flutter/scheduler.dart' show SchedulerBinding, Ticker;
import 'package:kilikili/rust/api/glass_droplet.dart';
import 'package:kilikili/services/rust_core.dart';
import 'package:kilikili/utils/extension/theme_ext.dart';
import 'package:material_ui/material_ui.dart';

const double _kMaxLabelTextScaleFactor = 1.3;

// 三种首页底栏共用高度，避免切换样式后胶囊尺寸跳变。
const mainNavigationBarHeight = 64.0;
const _kIndicatorHeight = mainNavigationBarHeight - 2 * _kIndicatorPaddingInt;
const _kIndicatorWidth = 86.0;
const _kIndicatorPaddingInt = 4.0;
const _kIndicatorPadding = EdgeInsets.all(_kIndicatorPaddingInt);
const _kBorderRadius = BorderRadius.all(.circular(mainNavigationBarHeight / 2));
const _kNavigationShape = RoundedSuperellipseBorder(
  borderRadius: _kBorderRadius,
);

/// 统一的胶囊玻璃底层，供三种底部导航样式共用。
///
/// ## 选中指示（#266）
///
/// 参照 iOS 26 tab bar（所有者给的录屏逐帧看过），选中指示有两种形态：
///
/// - **静止**：一块哑光灰色胶囊，没有玻璃。
/// - **切换或按住**：灰色淡出，**升起**一块透明透镜，比静止胶囊大一圈、可以
///   超出底栏上下边；放大下面的图标与文字，边缘强折射带色散，里面的图标被
///   染成选中色。到位后透镜落下，变回灰色胶囊。
///
/// 位置、升起程度、按下落在哪格、拖动吸附到哪格全由 Rust 决定
/// （`bili_core::glass_droplet`，AGENTS.md 第 2 节），这里只做三件事：用
/// [Ticker] 按真实流逝时间推它、把「格」换算成像素、画出来。透镜由
/// `shaders/glass_lens.frag` 画，它是叠在导航内容**之上**的第二个
/// [BackdropFilter]，所以采得到图标；只在升起时挂上。
///
/// 退路：
///
/// - 系统打开了「减弱动态效果」（#266 验收第 4 条）：不升起透镜，灰色胶囊按
///   原来的 300ms `easeOutCubic` 平移；拖动选择仍可用，胶囊按格跳。
/// - Rust Core 不可用（初始化失败，或没有原生库的组件测试进程）：同上，且没
///   有按压与拖动。为它在 Dart 再写一份规则正是第 2 节禁止的。
/// - 着色器不可用（非 Impeller）：照常移动，没有透镜，只有灰色胶囊。
///
/// ## 按压与拖动（#266 所有者定）
///
/// - 按下哪格，透镜**立即**升起并跑过去；真正切换仍由内层导航的点击完成，
///   这里不重复选中（重复点同一个 tab 在首页是「回顶/刷新」，不能多触发）。
/// - 按下后移动超过 [kTouchSlop]：进入拖动，透镜中心跟手；内层的点击此时
///   已被手势竞技场取消，松手后由这里吸附到最近一格并调
///   [onDestinationSelected]。
///
/// 手势用 [Listener] 读原始触点，不进手势竞技场，所以三种底栏（悬浮 / MD3 /
/// 经典，后两者是 Flutter 自带控件）都不用改内部就能生效。
class GlassNavigationSurface extends StatefulWidget {
  const GlassNavigationSurface({
    super.key,
    required this.child,
    this.selectedIndex,
    this.destinationCount,
    this.onDestinationSelected,
  });

  final Widget child;
  final int? selectedIndex;
  final int? destinationCount;

  /// 拖动选择松手时调用。点按不经过这里，由内层导航自己处理。
  final ValueChanged<int>? onDestinationSelected;

  @override
  State<GlassNavigationSurface> createState() => _GlassNavigationSurfaceState();
}

class _GlassNavigationSurfaceState extends State<GlassNavigationSurface>
    with SingleTickerProviderStateMixin {
  static Future<FragmentProgram>? _glassProgram;
  static Future<FragmentProgram>? _lensProgram;

  late final Ticker _ticker = createTicker(_onTick);

  /// 每推一帧通知一次。只有选中胶囊与透镜两处监听，导航内容不重建。
  final _frame = _FrameNotifier();

  GlassDropletState? _state;

  /// 最近一帧的形状；null 表示还没推过（静止在 [_selected] 上）。
  GlassDropletShape? _shape;
  Duration _lastElapsed = Duration.zero;

  /// Rust 调用抛过异常就永久退回平移，不在每一帧重试同一个错误。
  bool _rustFailed = false;

  /// 复用同一个着色器对象，每帧改 uniform 再包一个新的 `ImageFilter`：
  /// 原生滤镜在包装时按当时的 uniform 建出来，判等也比 uniform，所以
  /// 改了就会重绘、没改就不会。
  FragmentShader? _lensShader;

  /// 手指按下或拖动期间透镜的目标格，优先于 [_selected]。松手后保留到下一
  /// 帧画完：那时内层导航的点击（或这里的拖动吸附）已经把选中改过来了，
  /// 再交还给 [_selected] 就不会往旧位置抖一下。
  double? _touchTarget;
  bool _pressed = false;
  bool _dragging = false;
  int? _pointer;
  Offset _downPosition = Offset.zero;

  /// 最近一次布局的尺寸，供触点换算成「格」。
  Size _size = Size.zero;

  int? get _selected {
    final count = widget.destinationCount;
    final index = widget.selectedIndex;
    if (count == null || count <= 0 || index == null) return null;
    return index.clamp(0, count - 1);
  }

  bool get _rustUsable => RustCore.isReady && !_rustFailed;

  bool _useSprings(BuildContext context) =>
      _rustUsable && !MediaQuery.disableAnimationsOf(context);

  @override
  void didUpdateWidget(GlassNavigationSurface oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.destinationCount != widget.destinationCount) {
      // tab 数量变了，「格」的含义跟着变，旧状态没有意义，直接落到新位置。
      _reset();
    } else if (oldWidget.selectedIndex != widget.selectedIndex) {
      _kick();
    }
  }

  void _reset() {
    _ticker.stop();
    _state = null;
    _shape = null;
  }

  /// 手指此刻的位置，单位是格。
  double _slotOf(Offset local, int count) {
    final slotWidth = (_size.width - 2 * _kIndicatorPaddingInt) / count;
    if (slotWidth <= 0) return 0;
    return (local.dx - _kIndicatorPaddingInt) / slotWidth;
  }

  void _kick() {
    _frame.notify();
    if (_state != null && !_ticker.isActive) {
      _lastElapsed = Duration.zero;
      _ticker.start();
    }
  }

  void _onPointerDown(PointerDownEvent e) {
    final count = widget.destinationCount;
    if (_pointer != null || count == null || count <= 0 || !_rustUsable) {
      return;
    }
    try {
      _touchTarget = glassDropletPressTarget(
        pointer: _slotOf(e.localPosition, count),
        count: count,
      );
    } catch (_) {
      return;
    }
    _pointer = e.pointer;
    _downPosition = e.localPosition;
    _pressed = true;
    _dragging = false;
    _kick();
  }

  void _onPointerMove(PointerMoveEvent e) {
    final count = widget.destinationCount;
    if (e.pointer != _pointer || count == null || count <= 0) return;
    if (!_dragging && (e.localPosition - _downPosition).distance > kTouchSlop) {
      _dragging = true;
    }
    if (!_dragging) return;
    try {
      _touchTarget = glassDropletDragTarget(
        pointer: _slotOf(e.localPosition, count),
        count: count,
      );
    } catch (_) {
      return;
    }
    _kick();
  }

  void _onPointerUp(PointerUpEvent e) {
    if (e.pointer != _pointer) return;
    final count = widget.destinationCount;
    final target = _touchTarget;
    if (_dragging && count != null && count > 0 && target != null) {
      try {
        final index = glassDropletSnapIndex(target: target, count: count);
        _touchTarget = index.toDouble();
        // 拖回原来那格不算选择：`MainController.setIndex` 对重复选中的处理
        // 是 500ms 内第二次就回顶/刷新，拖一下不该触发它。
        if (index != widget.selectedIndex) {
          widget.onDestinationSelected?.call(index);
        }
      } catch (_) {
        _touchTarget = null;
      }
    }
    _endTouch(keepTarget: true);
  }

  void _onPointerCancel(PointerCancelEvent e) {
    if (e.pointer != _pointer) return;
    _endTouch(keepTarget: false);
  }

  void _endTouch({required bool keepTarget}) {
    _pointer = null;
    _pressed = false;
    _dragging = false;
    if (!keepTarget) {
      _touchTarget = null;
    } else {
      final released = _touchTarget;
      SchedulerBinding.instance.addPostFrameCallback((_) {
        // 期间又按下了就别清掉新的目标。
        if (!mounted || _pointer != null || _touchTarget != released) return;
        _touchTarget = null;
        _kick();
      });
    }
    _kick();
  }

  void _onTick(Duration elapsed) {
    final state = _state;
    final target = _touchTarget ?? _selected?.toDouble();
    if (state == null || target == null) {
      _ticker.stop();
      return;
    }
    final dt = (elapsed - _lastElapsed).inMicroseconds / 1e6;
    _lastElapsed = elapsed;
    try {
      final frame = glassDropletStep(
        state: state,
        target: target,
        pressed: _pressed,
        dtSeconds: dt,
      );
      _state = frame.state;
      _shape = frame.shape;
      if (frame.settled) _ticker.stop();
    } catch (_) {
      _reset();
      _rustFailed = true;
      setState(() {});
    }
    _frame.notify();
  }

  @override
  void dispose() {
    _ticker.dispose();
    _frame.dispose();
    _lensShader?.dispose();
    super.dispose();
  }

  /// 选中指示当前的外接框，像素，相对底栏左上角。升起时比静止胶囊大，
  /// 可能超出底栏的上下边（透镜层不受底栏裁剪）。
  Rect _indicatorRect(Size size, int selected, int count) {
    final slotWidth = (size.width - 2 * _kIndicatorPaddingInt) / count;
    final shape = _shape;
    final left = shape?.left ?? selected.toDouble();
    final right = shape?.right ?? selected + 1.0;
    final restHeight = size.height - 2 * _kIndicatorPaddingInt;
    final height = restHeight * (shape?.heightScale ?? 1.0);
    return Rect.fromLTWH(
      _kIndicatorPaddingInt + slotWidth * left,
      _kIndicatorPaddingInt + (restHeight - height) / 2,
      slotWidth * (right - left),
      height,
    );
  }

  /// 平移退路里胶囊停在哪格：按住或拖动时跟着触点吸附，否则是选中项。
  int _fallbackIndex(int selected, int count) {
    final target = _touchTarget;
    if (target == null) return selected;
    try {
      return glassDropletSnapIndex(target: target, count: count);
    } catch (_) {
      return selected;
    }
  }

  ImageFilter _lensFilter(
    FragmentProgram program,
    GlassDropletShape shape,
    ColorScheme scheme,
  ) {
    // 下标从 2 起：0、1 是引擎填的 u_size。顺序见 glass_lens.frag。
    final accent = scheme.primary;
    final shader = (_lensShader ??= program.fragmentShader())
      ..setFloat(2, shape.lensZoom)
      ..setFloat(3, shape.lift)
      ..setFloat(4, accent.r)
      ..setFloat(5, accent.g)
      ..setFloat(6, accent.b)
      ..setFloat(7, scheme.brightness == Brightness.light ? 1 : 0);
    return ImageFilter.shader(shader);
  }

  @override
  Widget build(BuildContext context) {
    final scheme = ColorScheme.of(context);
    final supportsShader = ImageFilter.isShaderFilterSupported;
    final selected = _selected;
    final count = widget.destinationCount;
    final springs = selected != null && _useSprings(context);
    if (!springs) {
      _reset();
    } else if (_state == null) {
      try {
        _state = glassDropletRest(index: selected.toDouble());
      } catch (_) {
        _rustFailed = true;
        return build(context);
      }
    }

    // 静止时的选中胶囊：哑光灰，不带玻璃（iOS 26 tab bar 静止时就是这样）。
    final pill = IgnorePointer(
      child: DecoratedBox(
        decoration: BoxDecoration(
          borderRadius: _kBorderRadius,
          color: scheme.onSurface.withValues(
            alpha: scheme.brightness == Brightness.light ? 0.08 : 0.14,
          ),
        ),
      ),
    );

    return LayoutBuilder(
      builder: (context, constraints) {
        final size = constraints.biggest;
        _size = size;
        final deform = springs && size.isFinite && count != null;
        return FutureBuilder<List<FragmentProgram>>(
          future: supportsShader
              ? Future.wait([
                  _glassProgram ??= FragmentProgram.fromAsset(
                    'shaders/liquid_glass.frag',
                  ),
                  _lensProgram ??= FragmentProgram.fromAsset(
                    'shaders/glass_lens.frag',
                  ),
                ])
              : null,
          builder: (context, snapshot) {
            final programs = supportsShader && snapshot.hasData
                ? snapshot.data!
                : null;
            final glassProgram = programs?[0];
            final lensProgram = programs?[1];
            // 有透镜可画时，灰色胶囊随升起淡出；没有透镜时一直显示。
            final lensOn = deform && lensProgram != null;

            Widget? indicator;
            if (selected != null && count != null) {
              if (deform) {
                indicator = ListenableBuilder(
                  listenable: _frame,
                  builder: (context, child) => Positioned.fromRect(
                    key: const Key('glass-selected-indicator'),
                    rect: _indicatorRect(size, selected, count),
                    child: Opacity(
                      opacity: lensOn
                          ? 1 - (_shape?.lift ?? 0).clamp(0.0, 1.0)
                          : 1,
                      child: child,
                    ),
                  ),
                  child: pill,
                );
              } else {
                final slotWidth =
                    (constraints.maxWidth - 2 * _kIndicatorPaddingInt) / count;
                indicator = ListenableBuilder(
                  listenable: _frame,
                  builder: (context, child) => AnimatedPositioned(
                    key: const Key('glass-selected-indicator'),
                    duration: const Duration(milliseconds: 300),
                    curve: Curves.easeOutCubic,
                    // 「减弱动态效果」下拖动选择仍然可用，只是胶囊按格跳。
                    left:
                        _kIndicatorPaddingInt +
                        slotWidth * _fallbackIndex(selected, count),
                    top: _kIndicatorPaddingInt,
                    bottom: _kIndicatorPaddingInt,
                    width: slotWidth,
                    child: child!,
                  ),
                  child: pill,
                );
              }
            }

            final content = Stack(
              fit: StackFit.passthrough,
              children: [
                Positioned.fill(
                  child: DecoratedBox(
                    decoration: BoxDecoration(
                      gradient: LinearGradient(
                        begin: Alignment.topLeft,
                        end: Alignment.bottomRight,
                        // 液态玻璃着色器只折射、不模糊，底下卡片的标题会原样
                        // 透上来，和导航文字叠在一起（#252 所有者反馈「太透、
                        // 看不清」）。所以中间区域靠这层着色压暗背板；边缘高
                        // 光与折射仍由着色器负责，玻璃感保留。此前为 0.24 /
                        // 0.10 / 0.18（#122 调高透明度时的取值）。
                        colors: [
                          scheme.surface.withValues(alpha: 0.78),
                          scheme.surface.withValues(alpha: 0.68),
                          scheme.surface.withValues(alpha: 0.74),
                        ],
                      ),
                      borderRadius: _kBorderRadius,
                      border: Border.all(
                        color: Colors.white.withValues(alpha: 0.36),
                      ),
                    ),
                  ),
                ),
                ?indicator,
                // 三种导航与选中背景共用分栏边界，避免首尾项目向外偏移。
                Padding(
                  padding: _kIndicatorPadding,
                  // 安全区已由胶囊外层负责，内部导航不能再次插入状态栏留白。
                  child: MediaQuery.removePadding(
                    context: context,
                    removeTop: true,
                    removeBottom: true,
                    removeLeft: true,
                    removeRight: true,
                    child: Theme(
                      data: Theme.of(context).copyWith(
                        splashFactory: NoSplash.splashFactory,
                        splashColor: Colors.transparent,
                        highlightColor: Colors.transparent,
                      ),
                      child: NavigationBarTheme(
                        data: NavigationBarTheme.of(context).copyWith(
                          overlayColor: const WidgetStatePropertyAll(
                            Colors.transparent,
                          ),
                        ),
                        child: Center(child: widget.child),
                      ),
                    ),
                  ),
                ),
              ],
            );

            final bar = DecoratedBox(
              decoration: BoxDecoration(
                borderRadius: _kBorderRadius,
                boxShadow: [
                  BoxShadow(
                    color: Colors.black.withValues(alpha: 0.22),
                    blurRadius: 22,
                    offset: const Offset(0, 7),
                  ),
                ],
              ),
              child: ClipRRect(
                borderRadius: _kBorderRadius,
                child: BackdropFilter(
                  // 非 Impeller 后端无法对背板运行自定义折射，保留可读的透明
                  // 材质。
                  filter: glassProgram != null
                      ? ImageFilter.shader(glassProgram.fragmentShader())
                      : ImageFilter.blur(sigmaX: 10, sigmaY: 10),
                  child: Listener(
                    behavior: HitTestBehavior.translucent,
                    onPointerDown: _onPointerDown,
                    onPointerMove: _onPointerMove,
                    onPointerUp: _onPointerUp,
                    onPointerCancel: _onPointerCancel,
                    child: content,
                  ),
                ),
              ),
            );
            if (!lensOn) return bar;

            // 透镜放在底栏的裁剪之外：升起时它比底栏还高一点（录屏里如此）。
            return Stack(
              clipBehavior: Clip.none,
              fit: StackFit.passthrough,
              children: [
                bar,
                ListenableBuilder(
                  listenable: _frame,
                  builder: (context, _) {
                    final shape = _shape;
                    // 没升起时不挂这一层，省掉一次背板采样。
                    if (shape == null || shape.lift <= 0.01) {
                      return const SizedBox.shrink();
                    }
                    final rect = _indicatorRect(size, selected, count);
                    return Positioned.fromRect(
                      rect: rect,
                      // 不给透镜加 BoxShadow：阴影先画在底下，会被透镜自己的
                      // BackdropFilter 采进去，透镜下半部发灰。轮廓靠着色器
                      // 里的白色高光边。
                      child: IgnorePointer(
                        child: ClipRRect(
                          borderRadius: BorderRadius.circular(
                            rect.shortestSide / 2,
                          ),
                          child: BackdropFilter(
                            filter: _lensFilter(lensProgram, shape, scheme),
                            child: const SizedBox.expand(),
                          ),
                        ),
                      ),
                    );
                  },
                ),
              ],
            );
          },
        );
      },
    );
  }
}

class _FrameNotifier extends ChangeNotifier {
  void notify() => notifyListeners();
}

/// ref [NavigationBar]
class FloatingNavigationBar extends StatelessWidget {
  // ignore: prefer_const_constructors_in_immutables
  FloatingNavigationBar({
    super.key,
    this.animationDuration = const Duration(milliseconds: 500),
    this.selectedIndex = 0,
    required this.destinations,
    this.onDestinationSelected,
    this.backgroundColor,
    this.elevation,
    this.shadowColor,
    this.surfaceTintColor,
    this.indicatorColor,
    this.indicatorShape,
    this.labelBehavior,
    this.overlayColor,
    this.labelTextStyle,
    this.labelPadding,
    this.bottomPadding = 8.0,
  }) : assert(destinations.length >= 2),
       assert(0 <= selectedIndex && selectedIndex < destinations.length);

  final Duration animationDuration;
  final int selectedIndex;
  final List<Widget> destinations;
  final ValueChanged<int>? onDestinationSelected;
  final Color? backgroundColor;
  final double? elevation;
  final Color? shadowColor;
  final Color? surfaceTintColor;
  final Color? indicatorColor;
  final ShapeBorder? indicatorShape;
  final NavigationDestinationLabelBehavior? labelBehavior;
  final WidgetStateProperty<Color?>? overlayColor;
  final WidgetStateProperty<TextStyle?>? labelTextStyle;
  final EdgeInsetsGeometry? labelPadding;
  final double bottomPadding;

  VoidCallback _handleTap(int index) {
    return onDestinationSelected != null
        ? () => onDestinationSelected!(index)
        : () {};
  }

  @override
  Widget build(BuildContext context) {
    final defaults = _NavigationBarDefaultsM3(context);

    final navigationBarTheme = NavigationBarTheme.of(context);
    final effectiveLabelBehavior =
        labelBehavior ??
        navigationBarTheme.labelBehavior ??
        defaults.labelBehavior!;

    final padding = MediaQuery.viewPaddingOf(context);

    return Padding(
      padding: .fromLTRB(
        padding.left + 12,
        0,
        padding.right + 12,
        bottomPadding + padding.bottom,
      ),
      child: SizedBox(
        height: mainNavigationBarHeight,
        width: destinations.length * _kIndicatorWidth,
        child: GlassNavigationSurface(
          selectedIndex: selectedIndex,
          destinationCount: destinations.length,
          onDestinationSelected: onDestinationSelected,
          child: Row(
            crossAxisAlignment: .stretch,
            children: <Widget>[
              for (int i = 0; i < destinations.length; i++)
                Expanded(
                  child: _SelectableAnimatedBuilder(
                    duration: animationDuration,
                    isSelected: i == selectedIndex,
                    builder: (context, animation) {
                      return _NavigationDestinationInfo(
                        index: i,
                        selectedIndex: selectedIndex,
                        totalNumberOfDestinations: destinations.length,
                        selectedAnimation: animation,
                        labelBehavior: effectiveLabelBehavior,
                        indicatorColor: indicatorColor,
                        indicatorShape: indicatorShape,
                        overlayColor: overlayColor,
                        onTap: _handleTap(i),
                        labelTextStyle: labelTextStyle,
                        labelPadding: labelPadding,
                        child: destinations[i],
                      );
                    },
                  ),
                ),
            ],
          ),
        ),
      ),
    );
  }
}

class FloatingNavigationDestination extends StatelessWidget {
  const FloatingNavigationDestination({
    super.key,
    required this.icon,
    this.selectedIcon,
    required this.label,
    this.tooltip,
    this.enabled = true,
  });

  final Widget icon;

  final Widget? selectedIcon;

  final String label;

  final String? tooltip;

  final bool enabled;

  @override
  Widget build(BuildContext context) {
    final info = _NavigationDestinationInfo.of(context);
    const selectedState = <WidgetState>{WidgetState.selected};
    const unselectedState = <WidgetState>{};
    const disabledState = <WidgetState>{WidgetState.disabled};

    final navigationBarTheme = NavigationBarTheme.of(context);
    final defaults = _NavigationBarDefaultsM3(context);
    final animation = info.selectedAnimation;

    return Stack(
      alignment: .center,
      clipBehavior: .none,
      children: [
        _NavigationDestinationBuilder(
          label: label,
          tooltip: tooltip,
          enabled: enabled,
          buildIcon: (context) {
            final IconThemeData selectedIconTheme =
                navigationBarTheme.iconTheme?.resolve(selectedState) ??
                defaults.iconTheme!.resolve(selectedState)!;
            final IconThemeData unselectedIconTheme =
                navigationBarTheme.iconTheme?.resolve(unselectedState) ??
                defaults.iconTheme!.resolve(unselectedState)!;
            final IconThemeData disabledIconTheme =
                navigationBarTheme.iconTheme?.resolve(disabledState) ??
                defaults.iconTheme!.resolve(disabledState)!;

            final Widget selectedIconWidget = IconTheme.merge(
              data: enabled ? selectedIconTheme : disabledIconTheme,
              child: selectedIcon ?? icon,
            );
            final Widget unselectedIconWidget = IconTheme.merge(
              data: enabled ? unselectedIconTheme : disabledIconTheme,
              child: icon,
            );
            return _StatusTransitionWidgetBuilder(
              animation: animation,
              builder: (context, child) {
                return animation.isForwardOrCompleted
                    ? selectedIconWidget
                    : unselectedIconWidget;
              },
            );
          },
          buildLabel: (context) {
            final TextStyle? effectiveSelectedLabelTextStyle =
                info.labelTextStyle?.resolve(selectedState) ??
                navigationBarTheme.labelTextStyle?.resolve(selectedState) ??
                defaults.labelTextStyle!.resolve(selectedState);
            final TextStyle? effectiveUnselectedLabelTextStyle =
                info.labelTextStyle?.resolve(unselectedState) ??
                navigationBarTheme.labelTextStyle?.resolve(unselectedState) ??
                defaults.labelTextStyle!.resolve(unselectedState);
            final TextStyle? effectiveDisabledLabelTextStyle =
                info.labelTextStyle?.resolve(disabledState) ??
                navigationBarTheme.labelTextStyle?.resolve(disabledState) ??
                defaults.labelTextStyle!.resolve(disabledState);
            final EdgeInsetsGeometry labelPadding =
                info.labelPadding ??
                navigationBarTheme.labelPadding ??
                defaults.labelPadding!;

            final textStyle = enabled
                ? animation.isForwardOrCompleted
                      ? effectiveSelectedLabelTextStyle
                      : effectiveUnselectedLabelTextStyle
                : effectiveDisabledLabelTextStyle;

            return Padding(
              padding: labelPadding,
              child: MediaQuery.withClampedTextScaling(
                maxScaleFactor: _kMaxLabelTextScaleFactor,
                child: Text(label, style: textStyle),
              ),
            );
          },
        ),
      ],
    );
  }
}

class _NavigationDestinationBuilder extends StatefulWidget {
  const _NavigationDestinationBuilder({
    required this.buildIcon,
    required this.buildLabel,
    required this.label,
    this.tooltip,
    this.enabled = true,
  });

  final WidgetBuilder buildIcon;

  final WidgetBuilder buildLabel;

  final String label;

  final String? tooltip;

  final bool enabled;

  @override
  State<_NavigationDestinationBuilder> createState() =>
      _NavigationDestinationBuilderState();
}

class _NavigationDestinationBuilderState
    extends State<_NavigationDestinationBuilder> {
  final GlobalKey iconKey = GlobalKey();

  @override
  Widget build(BuildContext context) {
    final info = _NavigationDestinationInfo.of(context);

    final child = GestureDetector(
      behavior: .opaque,
      onTap: widget.enabled ? info.onTap : null,
      child: _NavigationBarDestinationLayout(
        icon: widget.buildIcon(context),
        iconKey: iconKey,
        label: widget.buildLabel(context),
      ),
    );
    if (info.labelBehavior == .alwaysShow) {
      return child;
    }
    return _NavigationBarDestinationTooltip(
      message: widget.tooltip ?? widget.label,
      child: child,
    );
  }
}

class _NavigationDestinationInfo extends InheritedWidget {
  const _NavigationDestinationInfo({
    required this.index,
    required this.selectedIndex,
    required this.totalNumberOfDestinations,
    required this.selectedAnimation,
    required this.labelBehavior,
    required this.indicatorColor,
    required this.indicatorShape,
    required this.overlayColor,
    required this.onTap,
    this.labelTextStyle,
    this.labelPadding,
    required super.child,
  });

  final int index;

  final int selectedIndex;

  final int totalNumberOfDestinations;

  final Animation<double> selectedAnimation;

  final NavigationDestinationLabelBehavior labelBehavior;

  final Color? indicatorColor;

  final ShapeBorder? indicatorShape;

  final WidgetStateProperty<Color?>? overlayColor;

  final VoidCallback onTap;

  final WidgetStateProperty<TextStyle?>? labelTextStyle;

  final EdgeInsetsGeometry? labelPadding;

  static _NavigationDestinationInfo of(BuildContext context) {
    final _NavigationDestinationInfo? result = context
        .dependOnInheritedWidgetOfExactType<_NavigationDestinationInfo>();
    assert(
      result != null,
      'Navigation destinations need a _NavigationDestinationInfo parent, '
      'which is usually provided by NavigationBar.',
    );
    return result!;
  }

  @override
  bool updateShouldNotify(_NavigationDestinationInfo oldWidget) {
    return index != oldWidget.index ||
        totalNumberOfDestinations != oldWidget.totalNumberOfDestinations ||
        selectedAnimation != oldWidget.selectedAnimation ||
        labelBehavior != oldWidget.labelBehavior ||
        onTap != oldWidget.onTap;
  }
}

class NavigationIndicator extends StatelessWidget {
  const NavigationIndicator({
    super.key,
    required this.animation,
    this.color,
    this.width = _kIndicatorWidth,
    this.height = _kIndicatorHeight,
  });

  final Animation<double> animation;

  final Color? color;

  final double width;

  final double height;

  static final _anim = Tween<double>(
    begin: .5,
    end: 1.0,
  ).chain(CurveTween(curve: Curves.easeInOutCubicEmphasized));

  @override
  Widget build(BuildContext context) {
    return AnimatedBuilder(
      animation: animation,
      builder: (context, child) {
        final double scale = animation.isDismissed
            ? 0.0
            : _anim.evaluate(animation);

        return Transform(
          alignment: Alignment.center,
          transform: Matrix4.diagonal3Values(scale, 1.0, 1.0),
          child: child,
        );
      },

      child: _StatusTransitionWidgetBuilder(
        animation: animation,
        builder: (context, child) {
          return _SelectableAnimatedBuilder(
            isSelected: animation.isForwardOrCompleted,
            duration: const Duration(milliseconds: 100),
            alwaysDoFullAnimation: true,
            builder: (context, fadeAnimation) {
              return FadeTransition(
                opacity: fadeAnimation,
                child: DecoratedBox(
                  decoration: ShapeDecoration(
                    shape: _kNavigationShape,
                    color: color ?? Theme.of(context).colorScheme.secondary,
                  ),
                  child: const SizedBox(
                    width: _kIndicatorWidth,
                    height: _kIndicatorHeight,
                  ),
                ),
              );
            },
          );
        },
      ),
    );
  }
}

class _NavigationBarDestinationLayout extends StatelessWidget {
  const _NavigationBarDestinationLayout({
    required this.icon,
    required this.iconKey,
    required this.label,
  });

  final Widget icon;

  final GlobalKey iconKey;

  final Widget label;

  @override
  Widget build(BuildContext context) {
    return _DestinationLayoutAnimationBuilder(
      builder: (context, animation) {
        return CustomMultiChildLayout(
          delegate: _NavigationDestinationLayoutDelegate(animation: animation),
          children: <Widget>[
            LayoutId(
              id: _NavigationDestinationLayoutDelegate.iconId,
              child: KeyedSubtree(key: iconKey, child: icon),
            ),
            LayoutId(
              id: _NavigationDestinationLayoutDelegate.labelId,
              child: FadeTransition(
                alwaysIncludeSemantics: true,
                opacity: animation,
                child: label,
              ),
            ),
          ],
        );
      },
    );
  }
}

class _DestinationLayoutAnimationBuilder extends StatelessWidget {
  const _DestinationLayoutAnimationBuilder({required this.builder});

  final Widget Function(BuildContext, Animation<double>) builder;

  @override
  Widget build(BuildContext context) {
    final info = _NavigationDestinationInfo.of(context);
    switch (info.labelBehavior) {
      case NavigationDestinationLabelBehavior.alwaysShow:
        return builder(context, kAlwaysCompleteAnimation);
      case NavigationDestinationLabelBehavior.alwaysHide:
        return builder(context, kAlwaysDismissedAnimation);
      case NavigationDestinationLabelBehavior.onlyShowSelected:
        return _CurvedAnimationBuilder(
          animation: info.selectedAnimation,
          curve: Curves.easeInOutCubicEmphasized,
          reverseCurve: Curves.easeInOutCubicEmphasized.flipped,
          builder: builder,
        );
    }
  }
}

class _NavigationBarDestinationTooltip extends StatelessWidget {
  const _NavigationBarDestinationTooltip({
    required this.message,
    required this.child,
  });

  final String message;

  final Widget child;

  @override
  Widget build(BuildContext context) {
    return Tooltip(
      message: message,
      verticalOffset: 34,
      excludeFromSemantics: true,
      preferBelow: false,
      child: child,
    );
  }
}

class _NavigationDestinationLayoutDelegate extends MultiChildLayoutDelegate {
  _NavigationDestinationLayoutDelegate({required this.animation})
    : super(relayout: animation);

  final Animation<double> animation;

  static const int iconId = 1;

  static const int labelId = 2;

  /// 图标与文字之间收掉的距离（逻辑像素，#259）。实测 320dpi 平板上两者
  /// 可见间距约 10.5：`labelPadding` 只占 2，其余是图标字形下方留白与中文
  /// 字体行高的上沿，只改 `labelPadding` 减不到一半。所以在布局里让文字
  /// 上移，并按一半修正图标位置，使组合仍垂直居中。
  static const double gapReduction = 5;

  @override
  void performLayout(Size size) {
    double halfWidth(Size size) => size.width / 2;
    double halfHeight(Size size) => size.height / 2;

    final Size iconSize = layoutChild(iconId, BoxConstraints.loose(size));
    final Size labelSize = layoutChild(labelId, BoxConstraints.loose(size));

    final double yPositionOffset = Tween<double>(
      begin: halfHeight(iconSize),

      end: halfHeight(iconSize) + halfHeight(labelSize) - gapReduction / 2,
    ).transform(animation.value);
    final double iconYPosition = halfHeight(size) - yPositionOffset;

    positionChild(
      iconId,
      Offset(
        halfWidth(size) - halfWidth(iconSize),
        iconYPosition,
      ),
    );

    positionChild(
      labelId,
      Offset(
        halfWidth(size) - halfWidth(labelSize),

        iconYPosition + iconSize.height - gapReduction * animation.value,
      ),
    );
  }

  @override
  bool shouldRelayout(_NavigationDestinationLayoutDelegate oldDelegate) {
    return oldDelegate.animation != animation;
  }
}

class _StatusTransitionWidgetBuilder extends StatusTransitionWidget {
  const _StatusTransitionWidgetBuilder({
    required super.animation,
    required this.builder,
    // ignore: unused_element_parameter
    this.child,
  });

  final TransitionBuilder builder;

  final Widget? child;

  @override
  Widget build(BuildContext context) => builder(context, child);
}

class _SelectableAnimatedBuilder extends StatefulWidget {
  const _SelectableAnimatedBuilder({
    required this.isSelected,
    this.duration = const Duration(milliseconds: 200),
    this.alwaysDoFullAnimation = false,
    required this.builder,
  });

  final bool isSelected;

  final Duration duration;

  final bool alwaysDoFullAnimation;

  final Widget Function(BuildContext, Animation<double>) builder;

  @override
  _SelectableAnimatedBuilderState createState() =>
      _SelectableAnimatedBuilderState();
}

class _SelectableAnimatedBuilderState extends State<_SelectableAnimatedBuilder>
    with SingleTickerProviderStateMixin {
  late AnimationController _controller;

  @override
  void initState() {
    super.initState();
    _controller = AnimationController(vsync: this);
    _controller.duration = widget.duration;
    _controller.value = widget.isSelected ? 1.0 : 0.0;
  }

  @override
  void didUpdateWidget(_SelectableAnimatedBuilder oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.duration != widget.duration) {
      _controller.duration = widget.duration;
    }
    if (oldWidget.isSelected != widget.isSelected) {
      if (widget.isSelected) {
        _controller.forward(from: widget.alwaysDoFullAnimation ? 0 : null);
      } else {
        _controller.reverse(from: widget.alwaysDoFullAnimation ? 1 : null);
      }
    }
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return widget.builder(context, _controller);
  }
}

class _CurvedAnimationBuilder extends StatefulWidget {
  const _CurvedAnimationBuilder({
    required this.animation,
    required this.curve,
    required this.reverseCurve,
    required this.builder,
  });

  final Animation<double> animation;
  final Curve curve;
  final Curve reverseCurve;
  final Widget Function(BuildContext, Animation<double>) builder;

  @override
  _CurvedAnimationBuilderState createState() => _CurvedAnimationBuilderState();
}

class _CurvedAnimationBuilderState extends State<_CurvedAnimationBuilder> {
  late AnimationStatus _animationDirection;
  AnimationStatus? _preservedDirection;

  @override
  void initState() {
    super.initState();
    _animationDirection = widget.animation.status;
    _updateStatus(widget.animation.status);
    widget.animation.addStatusListener(_updateStatus);
  }

  @override
  void dispose() {
    widget.animation.removeStatusListener(_updateStatus);
    super.dispose();
  }

  void _updateStatus(AnimationStatus status) {
    if (_animationDirection != status) {
      setState(() {
        _animationDirection = status;
      });
    }
    switch (status) {
      case AnimationStatus.forward || AnimationStatus.reverse
          when _preservedDirection != null:
        break;
      case AnimationStatus.forward || AnimationStatus.reverse:
        setState(() {
          _preservedDirection = status;
        });
      case AnimationStatus.completed || AnimationStatus.dismissed:
        setState(() {
          _preservedDirection = null;
        });
    }
  }

  @override
  Widget build(BuildContext context) {
    final shouldUseForwardCurve =
        (_preservedDirection ?? _animationDirection) != AnimationStatus.reverse;

    final Animation<double> curvedAnimation = CurveTween(
      curve: shouldUseForwardCurve ? widget.curve : widget.reverseCurve,
    ).animate(widget.animation);

    return widget.builder(context, curvedAnimation);
  }
}

const _indicatorDark = Color(0x15FFFFFF);
const _indicatorLight = Color(0x10000000);

class _NavigationBarDefaultsM3 extends NavigationBarThemeData {
  _NavigationBarDefaultsM3(this.context)
    : super(
        height: mainNavigationBarHeight,
        elevation: 3.0,
        labelBehavior: NavigationDestinationLabelBehavior.alwaysShow,
      );

  final BuildContext context;
  late final _colors = Theme.of(context).colorScheme;
  late final _textTheme = Theme.of(context).textTheme;

  BorderSide get borderSide => _colors.isDark
      ? const BorderSide(color: Color(0x08FFFFFF))
      : const BorderSide(color: Color(0x08000000));

  @override
  Color? get backgroundColor => _colors.surfaceContainer;

  @override
  Color? get shadowColor => Colors.transparent;

  @override
  Color? get surfaceTintColor => Colors.transparent;

  @override
  WidgetStateProperty<IconThemeData?>? get iconTheme {
    return WidgetStateProperty.resolveWith((Set<WidgetState> states) {
      return IconThemeData(
        size: 24.0,
        color: states.contains(WidgetState.disabled)
            ? _colors.onSurfaceVariant.withValues(alpha: 0.38)
            : states.contains(WidgetState.selected)
            ? _colors.onSecondaryContainer
            : _colors.onSurfaceVariant,
      );
    });
  }

  @override
  Color? get indicatorColor =>
      _colors.isDark ? _indicatorDark : _indicatorLight;

  @override
  ShapeBorder? get indicatorShape => const StadiumBorder();

  @override
  WidgetStateProperty<TextStyle?>? get labelTextStyle {
    return WidgetStateProperty.resolveWith((Set<WidgetState> states) {
      final TextStyle style = _textTheme.labelMedium!;
      return style.apply(
        color: states.contains(WidgetState.disabled)
            ? _colors.onSurfaceVariant.withValues(alpha: 0.38)
            : states.contains(WidgetState.selected)
            ? _colors.onSurface
            : _colors.onSurfaceVariant,
      );
    });
  }

  @override
  EdgeInsetsGeometry? get labelPadding => const EdgeInsets.only(top: 2);
}
