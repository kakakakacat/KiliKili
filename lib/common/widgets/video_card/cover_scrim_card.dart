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

/// 信息行里 UP 名**之后**那几项的构造器。
///
/// 返回的是一串 widget 而不是一个 Row：这一行的排布（UP 名占多宽、谁先
/// 被挤掉）归卡片管，调用方只决定放什么。要把某几项推到右边，在返回的列
/// 表里插一个 `Spacer()`。列表里的项会拿到**无界**的主轴约束，所以放短
/// 内容（角标、日期），长文本自己包一层 `Flexible`。
typedef CoverScrimMetaBuilder = List<Widget> Function(
  BuildContext context,
  CoverScrimColors colors,
);

/// 统一的「大封面卡片 + 底部叠加文案」（issue #108）。
///
/// 整张卡片就是封面，标题与辅助信息压在封面底部的遮罩上。遮罩颜色随封面
/// 取色变化，对比度由 `bili_core::cover_scrim` 保证——所以**文案层不要再
/// 自己指定颜色**，使用 [CoverScrimBuilder] 提供的配色。
///
/// ## 遮罩：上方一段渐变，文字落在渐变到头之后
///
/// 颜色是 `bili_core::cover_scrim` 从封面底部区域算出的**主色**，按对比度
/// 需要调过明暗之后带着 alpha 用。遮罩从上往下由全透明渐变到这个颜色，
/// 渐变段高 [_fadeHeight]；**文字不落在渐变里**，它从渐变到头的地方才开始。
///
/// 这一条不能妥协：Rust 算对比度时假设的背景是「遮罩按自己的 alpha 合成
/// 到封面上」这一个结果。文字压在渐变中段的话实际背景比算的亮，那条保证
/// 就不成立了。想让遮罩整体更透，调 `ALPHA_MIN` —— 它会被对比度搜索接住，
/// 渐变不会。
///
/// ## 卡片比例由格子决定
///
/// 卡片自己**不套 `AspectRatio`**：整张卡就是封面，调用方的 grid delegate
/// （或固定高度的容器）已经按封面比例切好格子，这里铺满就对了。再套一层
/// 反而会和 `Card` 的默认 margin 打架——比例是对内容算的，margin 是加在外
/// 面的，算出来的卡片会比格子窄一截。
///
/// ## 封面上的东西 vs 遮罩里的东西
///
/// 这两层的可读性是两套规则，别混：
///
/// - **遮罩里**（[upName]、标题、[metaBuilder]）用 `colors` 里的颜色，对比
///   度由 Rust 保证，不要另外指定颜色；
/// - **封面上**（[topStart] 的统计药丸、[coverOverlays] 的时长角标）不在那
///   套保证里，一律自带半透明深色底 + 白字，不随封面变。
///
/// [coverOverlays] 原样放进封面的 Stack 里（`PBadge` 自带 `Positioned`），
/// 不参与点击。要吃点击的浮层走 [trailing]。
class CoverScrimCard extends StatelessWidget {
  const CoverScrimCard({
    super.key,
    required this.cover,
    required this.title,
    this.upName,
    this.upFollowed = false,
    this.titleMaxLines = 2,
    this.titleStyle,
    this.titleBuilder,
    this.metaBuilder,
    this.topStart,
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

  /// UP 名 / 主播名，排在遮罩里信息行的最前面。
  ///
  /// 它是这一行里唯一会被挤掉的项：空间不够时先截它，角标和日期都比它短
  /// 且少一个字就看不懂。
  final String? upName;

  /// 已登录**且**已关注时，在 [upName] 后面补一个对勾圆圈。
  ///
  /// 调用方自己带上「已登录」这一半判断：`isFollowed` 这类字段在匿名态下
  /// 是服务端给的默认值，不验登录会在未登录时也打上勾。
  final bool upFollowed;

  final int titleMaxLines;
  final TextStyle? titleStyle;

  /// 标题的自定义渲染：搜索结果要给命中词上色，纯字符串表达不了。给了它
  /// 就取代默认那行 `Text`，[titleMaxLines] 与截断要自己处理，颜色用
  /// `colors.title`。[title] 仍然要传，它是无障碍朗读用的文本。
  final CoverScrimBuilder? titleBuilder;

  /// 信息行里 [upName] 之后的那几项（角标、分区、日期…）。
  final CoverScrimMetaBuilder? metaBuilder;

  /// 封面**左上角**的浮层，放统计药丸（[CoverStatsPill]）。
  final Widget? topStart;

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

  /// 遮罩上方渐变段的高度。文字从这一段**之后**才开始，见类文档。
  static const _fadeHeight = 34.0;

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
          if (topStart case final topStart?)
            Positioned(top: 6, left: 6, child: topStart),
          Positioned(
            left: 0,
            right: 0,
            bottom: 0,
            child: IgnorePointer(
              child: _ScrimLayer(
                cover: cover,
                tintRgb:
                    ColorScheme.of(context).primary.toARGB32() & 0x00ffffff,
                titleMaxLines: titleMaxLines,
                titleStyle: titleStyle,
                titleBuilder: titleBuilder,
                title: title,
                semanticsLabel: semanticsLabel,
                upName: upName,
                upFollowed: upFollowed,
                metaBuilder: metaBuilder,
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
    required this.tintRgb,
    required this.title,
    required this.titleMaxLines,
    required this.titleStyle,
    required this.titleBuilder,
    required this.semanticsLabel,
    required this.upName,
    required this.upFollowed,
    required this.metaBuilder,
    required this.reserveTrailing,
  });

  final String? cover;
  final int tintRgb;
  final String title;

  final int titleMaxLines;
  final TextStyle? titleStyle;
  final CoverScrimBuilder? titleBuilder;
  final String? semanticsLabel;
  final String? upName;
  final bool upFollowed;
  final CoverScrimMetaBuilder? metaBuilder;
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
    if (oldWidget.cover != widget.cover ||
        oldWidget.tintRgb != widget.tintRgb) {
      _load();
    }
  }

  void _load() {
    final generation = ++_generation;
    final cover = widget.cover;
    if (cover == null || cover.isEmpty) {
      // 压根没有封面就没有「等它出现」这回事，直接给回退配色——否则标题
      // 无处可落，这张卡会变成一块空白。
      _colors = CoverScrimCache.fallback(widget.tintRgb);
      return;
    }
    // 命中缓存就同步用上：滚回来的卡片不该再淡入一次。
    final cached = CoverScrimCache.peek(cover, widget.tintRgb);
    _colors = cached;
    if (cached != null) return;
    CoverScrimCache.of(cover, widget.tintRgb).then((colors) {
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
        tween: _ScrimColorsTween(
          end: colors ?? CoverScrimCache.fallback(widget.tintRgb),
        ),
        duration: const Duration(milliseconds: 220),
        curve: Curves.easeOut,
        builder: (context, colors, _) => Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            // 渐变段：全透明 → 遮罩色。文字不落在这一段里。
            SizedBox(
              height: CoverScrimCard._fadeHeight,
              child: DecoratedBox(
                decoration: BoxDecoration(
                  gradient: LinearGradient(
                    begin: Alignment.topCenter,
                    end: Alignment.bottomCenter,
                    colors: [
                      colors.scrim.withValues(alpha: 0),
                      colors.scrim.withValues(alpha: colors.scrim.a * 0.45),
                      colors.scrim,
                    ],
                    // 中间那一档压在 0.6 而不是 0.5：线性渐变在视觉上前半
                    // 段显得太实，往后挪一点才像是「浮出来」的。
                    stops: const [0, 0.6, 1],
                  ),
                ),
              ),
            ),
            ColoredBox(
              color: colors.scrim,
              child: Padding(
                padding: EdgeInsets.fromLTRB(
                  9,
                  0,
                  widget.reserveTrailing ? CoverScrimCard._trailingWidth : 9,
                  7,
                ),
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    if (_meta(context, colors) case final meta
                        when meta.isNotEmpty)
                      Padding(
                        padding: const EdgeInsets.only(bottom: 2),
                        child: Row(spacing: 4, children: meta),
                      ),
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
                        style:
                            (widget.titleStyle ?? const TextStyle(height: 1.32))
                                .copyWith(color: colors.title),
                      ),
                  ],
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }

  /// 信息行：UP 名（+ 已关注的对勾）在前，调用方给的那几项在后。
  List<Widget> _meta(BuildContext context, CoverScrimColors colors) => [
    if (widget.upName case final name? when name.isNotEmpty) ...[
      // Flexible 只给 UP 名：这一行里只有它可以被截断还看得懂。
      Flexible(
        child: DecoratedBox(
          decoration: BoxDecoration(
            color: colors.tint,
            borderRadius: BorderRadius.circular(4),
          ),
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 4, vertical: 1),
            child: Text(
              name,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              semanticsLabel: 'UP：$name',
              style: TextStyle(
                fontSize: 11,
                height: 1.4,
                color: colors.tintLabel,
              ),
            ),
          ),
        ),
      ),
      if (widget.upFollowed)
        Icon(
          Icons.check_circle_rounded,
          size: 12,
          color: colors.subtitle,
          semanticLabel: '已关注',
        ),
    ],
    ...?widget.metaBuilder?.call(context, colors),
  ];
}

class _ScrimColorsTween extends Tween<CoverScrimColors> {
  _ScrimColorsTween({super.end});

  @override
  CoverScrimColors lerp(double t) => CoverScrimColors.lerp(begin!, end!, t);
}
