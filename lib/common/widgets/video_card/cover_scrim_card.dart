import 'package:kilikili/common/style.dart';
import 'package:kilikili/common/widgets/image/network_img_layer.dart';
import 'package:kilikili/services/rust/rust_cover_scrim.dart';
import 'package:material_ui/material_ui.dart';

// 用卡片的人一定要用到这组颜色，让他们再 import 一次取色服务没有意义。
export 'package:kilikili/services/rust/rust_cover_scrim.dart'
    show CoverScrimColors;

/// 文案层的构造器：拿到当前封面算出来的配色，自己决定画什么。
typedef CoverScrimBuilder = Widget Function(
  BuildContext context,
  CoverScrimColors colors,
);

/// 统一的「大封面卡片 + 底部叠加文案」（issue #108）。
///
/// 整张卡片就是封面，标题与辅助信息压在封面底部的遮罩上。遮罩颜色随封面
/// 取色变化，对比度由 `bili_core::cover_scrim` 保证——所以**文案层不要再
/// 自己指定颜色**，用 [CoverScrimBuilder] 给的那两个。
///
/// ## 遮罩是一整块纯色半透明，没有渐变
///
/// 颜色是 `bili_core::cover_scrim` 从封面底部区域算出的**主色**，按对比度
/// 需要调过明暗之后带着 alpha 直接铺满这一条。
///
/// 曾经在文案上方加过一段渐变过渡，去掉了：Rust 算对比度时假设的背景是
/// 「遮罩按自己的 alpha 合成到封面上」这一个结果，文字一旦压在渐变中段，
/// 实际背景比算的要亮，那条保证就不成立了。要软化边界只能靠降低 alpha，
/// 不能靠渐变——降 alpha 会被对比度搜索接住，渐变不会。
///
/// ## 卡片比例由格子决定
///
/// 卡片自己**不套 `AspectRatio`**：整张卡就是封面，调用方的 grid delegate
/// （或固定高度的容器）已经按封面比例切好格子，这里铺满就对了。再套一层
/// 反而会和 `Card` 的默认 margin 打架——比例是对内容算的，margin 是加在外
/// 面的，算出来的卡片会比格子窄一截。
///
/// ## 角标
///
/// [coverOverlays] 原样放进封面的 Stack 里（`PBadge` 自带 `Positioned`），
/// 不参与点击。要吃点击的浮层走 [trailing]。
class CoverScrimCard extends StatelessWidget {
  const CoverScrimCard({
    super.key,
    required this.cover,
    required this.title,
    this.upName,
    this.titleMaxLines = 2,
    this.titleStyle,
    this.titleBuilder,
    this.subtitleBuilder,
    this.coverOverlays = const [],
    this.trailing,
    this.onTap,
    this.onLongPress,
    this.onSecondaryTap,
    this.borderRadius = Style.coverCardRadius,
    this.elevation = 4,
    this.semanticsLabel,
  });

  final String? cover;
  final String title;

  /// UP 名 / 主播名，压在封面**左上角**。
  ///
  /// 它落在封面上而不是底部遮罩里，所以不在 `cover_scrim` 的对比度保证
  /// 范围内——那套保证只对遮罩合成出来的那块背景成立。这里改用和时长角标
  /// 同一套半透明深色底 + 白字，不依赖封面本身的颜色。
  final String? upName;

  final int titleMaxLines;
  final TextStyle? titleStyle;

  /// 标题的自定义渲染：搜索结果要给命中词上色，纯字符串表达不了。给了它
  /// 就取代默认那行 `Text`，[titleMaxLines] 与截断要自己处理，颜色用
  /// `colors.title`。[title] 仍然要传，它是无障碍朗读用的文本。
  final CoverScrimBuilder? titleBuilder;

  /// 标题下面那一行（UP 名、播放量、集数…）。null 就只有标题。
  final CoverScrimBuilder? subtitleBuilder;

  /// 盖在封面上的角标，自带 `Positioned`。
  final List<Widget> coverOverlays;

  /// 右下角可点的浮层（如 `VideoPopupMenu`）。给它让出的宽度见
  /// [_trailingWidth]。
  final Widget? trailing;

  final VoidCallback? onTap;
  final VoidCallback? onLongPress;
  final VoidCallback? onSecondaryTap;
  final BorderRadius borderRadius;

  /// 卡片的投影高度。
  ///
  /// 全局 `cardTheme` 把 `shadowColor` 设成了透明、`elevation` 设成 1，
  /// 也就是**全app的卡片本来都没有影子**——那套默认是给「浅色底上的浅色
  /// 卡」定的，在那里影子确实多余。这里不一样：卡片整张是封面，和背景之
  /// 间没有颜色落差，不给影子就只是一块贴在页面上的图。所以下面显式覆盖
  /// `shadowColor`，改这个值之前先知道它是有意偏离主题的。
  final double elevation;

  final String? semanticsLabel;

  /// [trailing] 占位宽度，和 `VideoPopupMenu` 的实际尺寸对齐。
  static const _trailingWidth = 32.0;

  /// [upName] 右边留给时长角标的宽度。
  static const _upNameRightInset = 54.0;

  @override
  Widget build(BuildContext context) {
    return Card(
      // hardEdge 足够：圆角由 Card 的 shape 裁，封面自己不再裁一次，省一层
      // saveLayer。
      clipBehavior: Clip.hardEdge,
      shape: RoundedRectangleBorder(borderRadius: borderRadius),
      elevation: elevation,
      shadowColor: ColorScheme.of(context).shadow,
      child: Stack(
        fit: StackFit.expand,
        children: [
          LayoutBuilder(
            builder: (context, constraints) => NetworkImgLayer(
              src: cover,
              width: constraints.maxWidth,
              height: constraints.maxHeight,
              borderRadius: BorderRadius.zero,
            ),
          ),
          ...coverOverlays,
          if (upName case final name? when name.isNotEmpty)
            Positioned(
              top: 6,
              left: 6,
              // 给右上角的时长 / 徽标让位；超出就截断，不换行。
              right: _upNameRightInset,
              child: Align(
                alignment: Alignment.centerLeft,
                child: DecoratedBox(
                  decoration: const BoxDecoration(
                    color: Colors.black45,
                    borderRadius: Style.borderRadius6,
                  ),
                  child: Padding(
                    padding: const EdgeInsets.symmetric(
                      horizontal: 6,
                      vertical: 2,
                    ),
                    child: Text(
                      name,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      semanticsLabel: 'UP：$name',
                      style: const TextStyle(
                        fontSize: 11,
                        height: 1.3,
                        color: Colors.white,
                      ),
                    ),
                  ),
                ),
              ),
            ),
          Positioned(
            left: 0,
            right: 0,
            bottom: 0,
            child: IgnorePointer(
              child: _ScrimLayer(
                cover: cover,
                titleMaxLines: titleMaxLines,
                titleStyle: titleStyle,
                titleBuilder: titleBuilder,
                title: title,
                semanticsLabel: semanticsLabel,
                subtitleBuilder: subtitleBuilder,
                reserveTrailing: trailing != null,
              ),
            ),
          ),
          // 水波纹要画在封面**之上**，所以这里单独起一层透明 Material；
          // 直接用 Card 自带的那层会被封面盖掉。
          Positioned.fill(
            child: Material(
              type: MaterialType.transparency,
              child: InkWell(
                onTap: onTap,
                onLongPress: onLongPress,
                onSecondaryTap: onSecondaryTap,
              ),
            ),
          ),
          if (trailing case final trailing?)
            Positioned(right: 0, bottom: 0, child: trailing),
        ],
      ),
    );
  }
}

class _ScrimLayer extends StatefulWidget {
  const _ScrimLayer({
    required this.cover,
    required this.title,
    required this.titleMaxLines,
    required this.titleStyle,
    required this.titleBuilder,
    required this.semanticsLabel,
    required this.subtitleBuilder,
    required this.reserveTrailing,
  });

  final String? cover;
  final String title;

  final int titleMaxLines;
  final TextStyle? titleStyle;
  final CoverScrimBuilder? titleBuilder;
  final String? semanticsLabel;
  final CoverScrimBuilder? subtitleBuilder;
  final bool reserveTrailing;

  @override
  State<_ScrimLayer> createState() => _ScrimLayerState();
}

class _ScrimLayerState extends State<_ScrimLayer> {
  /// null = 封面还在路上，遮罩还不该出现。
  ///
  /// 遮罩要和封面**一起**出现：在占位图上先铺一条深色带，看上去像卡片坏了
  /// 一半。所以取色没算完就整条不画，算完再跟着封面一起淡入。
  CoverScrimColors? _colors;

  /// 封面换了之后，上一张的异步结果必须丢掉，否则列表项复用时会串色。
  int _generation = 0;

  @override
  void initState() {
    super.initState();
    _load();
  }

  @override
  void didUpdateWidget(_ScrimLayer oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.cover != widget.cover) _load();
  }

  void _load() {
    final cover = widget.cover;
    if (cover == null || cover.isEmpty) {
      // 压根没有封面就没有「等它出现」这回事，直接给回退配色——否则标题
      // 无处可落，这张卡会变成一块空白。
      _colors = CoverScrimCache.fallback;
      return;
    }
    // 命中缓存就同步用上：滚回来的卡片不该再淡入一次。
    final cached = CoverScrimCache.peek(cover);
    _colors = cached;
    if (cached != null) return;
    final generation = ++_generation;
    CoverScrimCache.of(cover).then((colors) {
      if (!mounted || generation != _generation || colors == _colors) return;
      setState(() => _colors = colors);
    });
  }

  @override
  Widget build(BuildContext context) {
    final colors = _colors;
    return AnimatedOpacity(
      // 和 NetworkImgLayer 的 fadeInDuration 对齐，两者一起浮出来。
      // opacity 为 1 时 Opacity 会短路掉 saveLayer，稳态不额外花钱。
      opacity: colors == null ? 0 : 1,
      duration: const Duration(milliseconds: 120),
      child: TweenAnimationBuilder<CoverScrimColors>(
        // begin 留空：首帧（含缓存命中）直接到位，只有异步算完才补一段过渡。
        tween: _ScrimColorsTween(end: colors ?? CoverScrimCache.fallback),
        duration: const Duration(milliseconds: 220),
        curve: Curves.easeOut,
        builder: (context, colors, _) => ColoredBox(
          color: colors.scrim,
          child: Padding(
            padding: EdgeInsets.fromLTRB(
              9,
              7,
              widget.reserveTrailing ? CoverScrimCard._trailingWidth : 9,
              7,
            ),
            child: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                if (widget.titleBuilder case final build?)
                  Semantics(
                    label: widget.semanticsLabel ?? widget.title,
                    child: build(context, colors),
                  )
                else
                  Text(
                    widget.title,
                    maxLines: widget.titleMaxLines,
                    overflow: TextOverflow.ellipsis,
                    semanticsLabel: widget.semanticsLabel,
                    style: (widget.titleStyle ?? const TextStyle(height: 1.32))
                        .copyWith(color: colors.title),
                  ),
                if (widget.subtitleBuilder case final build?)
                  build(context, colors),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

class _ScrimColorsTween extends Tween<CoverScrimColors> {
  _ScrimColorsTween({super.end});

  @override
  CoverScrimColors lerp(double t) => CoverScrimColors.lerp(begin!, end!, t);
}
