import 'dart:io';

import 'package:kilikili/common/style.dart';
import 'package:kilikili/common/widgets/image/network_img_layer.dart';
import 'package:kilikili/common/widgets/video_card/cover_pill.dart';
import 'package:kilikili/common/widgets/video_card/cover_zoom.dart';
import 'package:kilikili/models/common/stat_type.dart';
import 'package:kilikili/services/rust/rust_cover_scrim.dart';
import 'package:kilikili/utils/extension/num_ext.dart';
import 'package:material_ui/material_ui.dart';

// 用卡片的人一定要用到这组颜色，让他们再 import 一次取色服务没有意义。
export 'package:kilikili/services/rust/rust_cover_scrim.dart'
    show CoverScrimColors;

/// 遮罩信息行的构造器。
///
/// 返回的是一串 widget 而不是一个 Row：这一行的排布归卡片管，调用方只决定
/// 放什么。要把某几项推到右边，在返回的列表里插一个 `Spacer()`。列表里的
/// 项会拿到**无界**的主轴约束，所以放短内容（角标、分区），长文本自己包
/// 一层 `Flexible`。
typedef CoverScrimMetaBuilder = List<Widget> Function(
  BuildContext context,
  CoverScrimColors colors,
);

/// 统一的视频封面卡片：封面在上，「UP 主名 + 标题」在封面下方（#295）。
///
/// ## 布局
///
/// - **封面**：顶部信息行（[topStats] / [topDuration]，#124）、角标
///   （[coverOverlays]）、[topStart] 都压在封面上；[metaBuilder] 给出的
///   辅助信息（分区、在线人数、简介…）放在封面底部一条**矮遮罩**里。
/// - **封面下方**：`UP 主名  标题……` 作为同一段文字连续排，最多
///   [titleMaxLines] 行，超出用省略号。发布日期 [date] 和操作按钮
///   [trailing] 在右下角，二者垂直居中，与最后一行文字对齐；标题最后一行
///   放不下时只有这一行给它们让位。
///
/// ## 卡片高度由格子决定
///
/// 封面吃掉格子扣除文字区之后的全部高度，所以调用方的 grid delegate 要按
/// 「封面比例 + [textExtent]」切格子（见 `Grid.coverCardDelegate`）。父级
/// 高度无界时（例如动态里的视频卡），改传 [coverAspectRatio]，卡片按封面
/// 比例自己定高。
///
/// ## 遮罩
///
/// 颜色是 `bili_core::cover_scrim` 从封面底部区域算出的主色，按对比度需要
/// 调过明暗之后带着 alpha 用。遮罩从上往下由全透明渐变到这个颜色，渐变段
/// 高 [_fadeHeight]；**文字不落在渐变里**，它从渐变到头的地方才开始——
/// Rust 算对比度时假设的背景是「遮罩按自己的 alpha 合成到封面上」，文字压
/// 在渐变中段的话那条保证就不成立了。遮罩里的文字用 [CoverScrimColors]
/// 的颜色，不要另外指定。没有辅助信息的卡片不画遮罩。
class CoverScrimCard extends StatelessWidget {
  const CoverScrimCard({
    super.key,
    required this.cover,
    this.localCover,
    required this.title,
    this.titleSpans,
    this.upName,
    this.upFollowed = false,
    this.titleMaxLines = 2,
    this.titleStyle,
    this.date,
    this.metaBuilder,
    this.topStart,
    this.topStats = const [],
    this.topDuration,
    this.coverOverlays = const [],
    this.foregroundOverlays = const [],
    this.trailing,
    this.onTap,
    this.onLongPress,
    this.onSecondaryTap,
    this.borderRadius = Style.coverCardRadius,
    this.elevation = 4,
    this.semanticsLabel,
    this.coverAspectRatio,
  });

  final String? cover;

  /// 离线缓存的本地封面；存在时显示和取色都优先使用它。
  final String? localCover;
  final String title;

  /// 标题的富文本（例如搜索结果给命中词加粗）。只允许 [TextSpan]：右下角
  /// 让位要按行切分文字，[WidgetSpan] 量不出来。给了它就取代 [title] 的
  /// 显示，[title] 仍然要传，它是无障碍朗读用的文本。
  final List<TextSpan>? titleSpans;

  /// UP 名 / 主播名，做成主题色底白字的胶囊，排在标题最前面、同一段文字里。
  final String? upName;

  /// 已登录**且**已关注时，在 [upName] 后面补一个对勾圆圈。
  ///
  /// 调用方自己带上「已登录」这一半判断：`isFollowed` 这类字段在匿名态下
  /// 是服务端给的默认值，不验登录会在未登录时也打上勾。
  final bool upFollowed;

  final int titleMaxLines;

  /// 叠加在默认标题样式上（例如「正在播放」加粗）。不要在这里指定颜色。
  final TextStyle? titleStyle;

  /// 右下角的日期，紧挨在 [trailing] 左边。
  final String? date;

  /// 封面底部遮罩里的辅助信息。
  final CoverScrimMetaBuilder? metaBuilder;

  /// 直播围观数等仍需单独摆放的左上角浮层，紧贴封面左上角、无间距。
  final Widget? topStart;

  /// 视频顶部共用渐变行；没有对应数据的卡片只展示可用项。
  final List<(StatType, dynamic)> topStats;
  final String? topDuration;

  /// 盖在封面上的角标，自带 `Positioned`。
  final List<Widget> coverOverlays;

  /// 盖在封面最上层的进度条、选中遮罩等，自带 `Positioned`。
  final List<Widget> foregroundOverlays;

  /// 右下角可点的按钮（如 `VideoPopupMenu`），放在 [_trailingSize] 见方的
  /// 格子里。它在封面下方的卡片底色上，图标用主题色，不要再写死白色。
  final Widget? trailing;

  final VoidCallback? onTap;
  final VoidCallback? onLongPress;
  final VoidCallback? onSecondaryTap;
  final BorderRadius borderRadius;

  /// 卡片的投影高度。
  ///
  /// 全局 `cardTheme` 把 `shadowColor` 设成了透明、`elevation` 设成 1，
  /// 也就是**全app的卡片本来都没有影子**——那套默认是给「浅色底上的浅色
  /// 卡」定的，在那里影子确实多余。这里不一样：卡片大半是封面，和背景之
  /// 间没有颜色落差，不给影子就只是一块贴在页面上的图。所以下面显式覆盖
  /// `shadowColor`，改这个值之前先知道它是有意偏离主题的。
  final double elevation;

  final String? semanticsLabel;

  /// 父级高度无界时，按这个比例给封面定高；为 null 时封面铺满格子剩下的
  /// 高度。
  final double? coverAspectRatio;

  static const _titleFontSize = 14.0;
  static const _titleHeight = 1.32;
  static const _dateFontSize = 11.0;

  /// 文字区的内边距。
  static const _padH = 8.0;
  static const _padTop = 6.0;
  static const _padBottom = 6.0;

  /// [trailing] 的格子边长与离卡片右边缘的距离。
  static const _trailingSize = 29.0;
  static const _trailingRight = 2.0;

  /// 文字和右下角那组（日期 + 按钮）之间的最小间距。
  static const _groupGap = 6.0;

  /// 遮罩上方渐变段的高度。文字从这一段**之后**才开始，见类文档。
  static const _fadeHeight = 18.0;

  /// 封面下方文字区的高度（未缩放字号）。grid delegate 用它给格子加高。
  ///
  /// 系统放大字号时实际文字区会更高，多出来的部分从封面里扣（封面按
  /// `BoxFit.cover` 裁切），不会溢出。
  static double textExtent([int maxLines = 2]) =>
      _padTop + _padBottom + maxLines * _titleFontSize * _titleHeight;

  @override
  Widget build(BuildContext context) {
    final colorScheme = ColorScheme.of(context);
    final lineHeight =
        MediaQuery.textScalerOf(context).scale(_titleFontSize) * _titleHeight;
    // 封面放大进播放页（#265）：每张网络封面都是一个可放大的源。点下去时
    // 记成「刚点的那张」，播放页入口按封面地址认领，见 [CoverZoomSource]。
    final coverZoom = localCover == null && cover?.isNotEmpty == true
        ? CoverZoomSource(cover: cover)
        : null;
    final Widget coverStack = _CoverStack(
      cover: cover,
      coverZoom: coverZoom,
      localCover: localCover,
      topStats: topStats,
      topDuration: topDuration,
      topStart: topStart,
      coverOverlays: coverOverlays,
      foregroundOverlays: foregroundOverlays,
      metaBuilder: metaBuilder,
    );
    final coverArea = switch (coverZoom) {
      final zoom? => zoom.sourceHero(coverStack),
      null => coverStack,
    };
    final card = Card(
      // hardEdge 足够：圆角由 Card 的 shape 裁，封面自己不再裁一次，省一层
      // saveLayer。
      clipBehavior: Clip.hardEdge,
      shape: RoundedRectangleBorder(borderRadius: borderRadius),
      elevation: elevation,
      shadowColor: colorScheme.shadow,
      child: Stack(
        fit: StackFit.passthrough,
        children: [
          Column(
            mainAxisSize: coverAspectRatio == null
                ? MainAxisSize.max
                : MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              if (coverAspectRatio case final ratio?)
                AspectRatio(aspectRatio: ratio, child: coverArea)
              else
                Expanded(child: coverArea),
              _TextStrip(
                span: TextSpan(children: titleSpans ?? [TextSpan(text: title)]),
                upLabel: _upLabel(colorScheme),
                upColor: colorScheme.primary,
                semanticsLabel: [
                  if (upName case final name? when name.isNotEmpty) 'UP：$name',
                  if (upFollowed && upName?.isNotEmpty == true) '已关注',
                  semanticsLabel ?? title,
                  if (date case final date? when date.isNotEmpty) date,
                ].join('，'),
                style: TextStyle(
                  fontSize: _titleFontSize,
                  height: _titleHeight,
                  color: colorScheme.onSurface,
                ).merge(titleStyle),
                maxLines: titleMaxLines,
                lineHeight: lineHeight,
                date: date,
                dateStyle: TextStyle(
                  fontSize: _dateFontSize,
                  height: 1,
                  color: colorScheme.outline,
                ),
                reserveTrailing: trailing != null,
              ),
            ],
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
          // 按钮要吃点击，必须在 InkWell 之上，所以不放进文字区，而是在这里
          // 按同一套尺寸对齐到最后一行文字的垂直中线。
          if (trailing case final trailing?)
            Positioned(
              right: _trailingRight,
              bottom: _padBottom + lineHeight / 2 - _trailingSize / 2,
              child: IconTheme.merge(
                data: IconThemeData(color: colorScheme.outline),
                child: SizedBox.square(
                  dimension: _trailingSize,
                  child: trailing,
                ),
              ),
            ),
        ],
      ),
    );
    if (coverZoom == null) return card;
    // 用 Listener 而不是包一层 onTap：有的卡片（如动态）不走自己的 onTap，
    // 点击由外层处理，指针抬起时记一下就能覆盖所有入口。
    return Listener(
      onPointerUp: (_) => coverZoom.markTapped(),
      child: card,
    );
  }

  /// 胶囊里的字：`UP 主名 [已关注]`，白字（主题色底上的 onPrimary）。
  TextSpan? _upLabel(ColorScheme colorScheme) {
    final name = upName;
    if (name == null || name.isEmpty) return null;
    return TextSpan(
      style: TextStyle(
        fontSize: 11,
        height: 1.2,
        color: colorScheme.onPrimary,
        fontWeight: FontWeight.w500,
      ),
      children: [
        TextSpan(text: name),
        if (upFollowed)
          TextSpan(
            text:
                ' ${String.fromCharCode(Icons.check_circle_rounded.codePoint)}',
            style: TextStyle(
              fontFamily: Icons.check_circle_rounded.fontFamily,
              package: Icons.check_circle_rounded.fontPackage,
            ),
          ),
      ],
    );
  }
}

/// 封面 + 压在封面上的所有东西。
class _CoverStack extends StatelessWidget {
  const _CoverStack({
    required this.cover,
    required this.coverZoom,
    required this.localCover,
    required this.topStats,
    required this.topDuration,
    required this.topStart,
    required this.coverOverlays,
    required this.foregroundOverlays,
    required this.metaBuilder,
  });

  final String? cover;
  final CoverZoomSource? coverZoom;
  final String? localCover;
  final List<(StatType, dynamic)> topStats;
  final String? topDuration;
  final Widget? topStart;
  final List<Widget> coverOverlays;
  final List<Widget> foregroundOverlays;
  final CoverScrimMetaBuilder? metaBuilder;

  @override
  Widget build(BuildContext context) {
    // 遮罩只为辅助信息而画；用回退配色试跑一次构造器判断有没有内容，省得
    // 为一条空遮罩去解码封面取色。
    final hasMeta =
        metaBuilder?.call(context, CoverScrimCache.fallback).isNotEmpty == true;
    return Stack(
      fit: StackFit.expand,
      children: [
        LayoutBuilder(
          builder: (context, constraints) {
            // 记下封面尺寸：放大转场要按同一尺寸取图才能命中这张图的缓存。
            coverZoom?.size = constraints.biggest;
            return localCover != null
                ? Image.file(
                    File(localCover!),
                    width: constraints.maxWidth,
                    height: constraints.maxHeight,
                    fit: BoxFit.cover,
                    cacheWidth: constraints.maxWidth.cacheSize(context),
                    colorBlendMode: NetworkImgLayer.reduce
                        ? BlendMode.modulate
                        : null,
                    color: NetworkImgLayer.reduce
                        ? NetworkImgLayer.reduceLuxColor
                        : null,
                  )
                : NetworkImgLayer(
                    src: cover,
                    width: constraints.maxWidth,
                    height: constraints.maxHeight,
                    borderRadius: BorderRadius.zero,
                  );
          },
        ),
        CoverTopInfoBar(stats: topStats, duration: topDuration),
        if (topStart case final topStart?)
          Positioned(top: 0, left: 0, child: topStart),
        if (hasMeta)
          Positioned(
            left: 0,
            right: 0,
            bottom: 0,
            child: IgnorePointer(
              child: _ScrimLayer(
                cover: localCover == null ? cover : 'file:$localCover',
                metaBuilder: metaBuilder!,
              ),
            ),
          ),
        // 角标在底部遮罩之上：#295 之后角标贴在封面右下角，正好落在遮罩带里，
        // 画在遮罩下面会被盖住。
        ...coverOverlays,
        ...foregroundOverlays,
      ],
    );
  }
}

/// 封面下方的文字区：`UP 主名  标题`，右下角是日期（按钮由卡片另放）。
///
/// ## 右下角怎么让位
///
/// 日期和按钮只占最后一行的右端。先按整行宽度排一次：最后一行放得下
/// 「文字 + 让位宽度」，或者文字根本没排到最后一行，就原样显示；否则把
/// 前 `maxLines - 1` 行原样切出来，剩下的文字放进最后一行、右侧让出那组
/// 的宽度，超出用省略号。这样只有最后一行变窄，前面几行仍是整行宽度。
class _TextStrip extends StatelessWidget {
  const _TextStrip({
    required this.span,
    required this.upLabel,
    required this.upColor,
    required this.semanticsLabel,
    required this.style,
    required this.maxLines,
    required this.lineHeight,
    required this.date,
    required this.dateStyle,
    required this.reserveTrailing,
  });

  final TextSpan span;

  /// UP 名胶囊里的字；为 null 时没有胶囊。
  final TextSpan? upLabel;
  final Color upColor;
  final String semanticsLabel;
  final TextStyle style;
  final int maxLines;
  final double lineHeight;
  final String? date;
  final TextStyle dateStyle;
  final bool reserveTrailing;

  @override
  Widget build(BuildContext context) {
    final date = this.date?.isNotEmpty == true ? this.date : null;
    final scaler = MediaQuery.textScalerOf(context);
    // 测量必须和真正绘制时用同一套样式：Text 会把 DefaultTextStyle（主题
    // 字体、字距等）合并进来，TextPainter 不会，得手动合并。
    final inherited = DefaultTextStyle.of(context).style;
    final direction = Directionality.of(context);
    // 右下角那组离文字区右边缘（不是卡片右边缘）的距离。
    const trailingInset =
        CoverScrimCard._trailingRight +
        CoverScrimCard._trailingSize -
        CoverScrimCard._padH;
    final dateRight = reserveTrailing ? trailingInset + 2 : 0.0;
    double groupWidth = 0;
    if (date != null) {
      final painter = TextPainter(
        text: TextSpan(text: date, style: inherited.merge(dateStyle)),
        textDirection: direction,
        textScaler: scaler,
        maxLines: 1,
      )..layout();
      groupWidth = dateRight + painter.width;
      painter.dispose();
    } else if (reserveTrailing) {
      groupWidth = trailingInset;
    }
    if (groupWidth > 0) groupWidth += CoverScrimCard._groupGap;

    return Semantics(
      container: true,
      label: semanticsLabel,
      child: ExcludeSemantics(
        child: Padding(
          padding: const EdgeInsets.fromLTRB(
            CoverScrimCard._padH,
            CoverScrimCard._padTop,
            CoverScrimCard._padH,
            CoverScrimCard._padBottom,
          ),
          child: SizedBox(
            // 固定排满 maxLines 行的高度：同一排卡片的文字区和右下角按钮
            // 才对得齐，短标题不会让封面忽高忽低。
            height: lineHeight * maxLines,
            child: Stack(
              clipBehavior: Clip.none,
              children: [
                Positioned.fill(
                  child: LayoutBuilder(
                    builder: (context, constraints) => _title(
                      context,
                      inherited,
                      constraints.maxWidth,
                      groupWidth,
                      scaler,
                      direction,
                    ),
                  ),
                ),
                if (date != null)
                  Positioned(
                    right: dateRight,
                    bottom: 0,
                    height: lineHeight,
                    child: Align(
                      alignment: Alignment.centerRight,
                      child: Text(date, maxLines: 1, style: dateStyle),
                    ),
                  ),
              ],
            ),
          ),
        ),
      ),
    );
  }

  Widget _title(
    BuildContext context,
    TextStyle inherited,
    double width,
    double groupWidth,
    TextScaler scaler,
    TextDirection direction,
  ) {
    Widget text(TextSpan span, int maxLines) => Text.rich(
      span,
      maxLines: maxLines,
      overflow: TextOverflow.ellipsis,
      style: style,
    );

    // UP 名胶囊作为 WidgetSpan 排在最前面。尺寸在这里先量好并写死在胶囊
    // 上，下面按行切分时给 TextPainter 的占位尺寸才和实际绘制一致。
    final pill = _pill(inherited, width, scaler, direction);
    final span = pill == null
        ? this.span
        : TextSpan(children: [pill.span, this.span]);

    if (groupWidth <= 0) return text(span, maxLines);
    if (maxLines == 1) {
      return Padding(
        padding: EdgeInsets.only(right: groupWidth),
        child: text(span, 1),
      );
    }

    final painter = TextPainter(
      text: TextSpan(style: inherited.merge(style), children: [span]),
      textDirection: direction,
      textScaler: scaler,
      textHeightBehavior: DefaultTextStyle.of(context).textHeightBehavior,
    );
    if (pill != null) {
      painter.setPlaceholderDimensions([
        PlaceholderDimensions(
          size: pill.size,
          alignment: PlaceholderAlignment.middle,
        ),
      ]);
    }
    painter.layout(maxWidth: width);
    final lines = painter.computeLineMetrics();
    final int splitAt;
    if (lines.length < maxLines ||
        (lines.length == maxLines && lines.last.width + groupWidth <= width)) {
      painter.dispose();
      return text(span, maxLines);
    }
    // 最后一行第一个字符的位置。
    splitAt = painter
        .getPositionForOffset(Offset(0, lines[maxLines - 1].baseline))
        .offset;
    painter.dispose();

    final runs = <InlineSpan>[];
    _flatten(span, null, runs);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        text(_slice(runs, 0, splitAt), maxLines - 1),
        Padding(
          padding: EdgeInsets.only(right: groupWidth),
          child: text(_slice(runs, splitAt, null), 1),
        ),
      ],
    );
  }

  /// UP 名胶囊：量好的尺寸 + 对应的 [WidgetSpan]。
  ({Size size, WidgetSpan span})? _pill(
    TextStyle inherited,
    double width,
    TextScaler scaler,
    TextDirection direction,
  ) {
    final label = upLabel;
    if (label == null) return null;
    final painter = TextPainter(
      text: TextSpan(style: inherited.merge(label.style), children: [label]),
      textDirection: direction,
      textScaler: scaler,
      maxLines: 1,
    )..layout();
    final labelSize = painter.size;
    painter.dispose();
    // UP 名一律完整显示（所有者要求，#300）。B 站昵称最多 16 个字，正常
    // 宽度下放得下；只有窄到一行都放不下时才按整行宽度兜底，否则胶囊会
    // 撑出文字区。
    final maxLabelWidth = width - _pillPadding.horizontal - _pillGap;
    final pillSize = Size(
      labelSize.width.clamp(0, maxLabelWidth.clamp(0, double.infinity)) +
          _pillPadding.horizontal,
      labelSize.height + _pillPadding.vertical,
    );
    return (
      size: Size(pillSize.width + _pillGap, pillSize.height),
      span: WidgetSpan(
        alignment: PlaceholderAlignment.middle,
        child: Padding(
          padding: const EdgeInsetsDirectional.only(end: _pillGap),
          child: SizedBox.fromSize(
            size: pillSize,
            child: DecoratedBox(
              decoration: BoxDecoration(
                color: upColor,
                borderRadius: const BorderRadius.all(Radius.circular(4)),
              ),
              child: Padding(
                padding: _pillPadding,
                child: Text.rich(
                  label,
                  maxLines: 1,
                  softWrap: false,
                  overflow: TextOverflow.ellipsis,
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }

  static const _pillPadding = EdgeInsets.symmetric(horizontal: 5, vertical: 1);
  static const _pillGap = 5.0;

  /// 把嵌套的 [TextSpan] 摊平成一串「文字 + 已合并样式」；胶囊的
  /// [WidgetSpan] 原样保留，占一个字符位。
  static void _flatten(
    InlineSpan span,
    TextStyle? parent,
    List<InlineSpan> out,
  ) {
    if (span is! TextSpan) {
      out.add(span);
      return;
    }
    final style = parent == null ? span.style : parent.merge(span.style);
    if (span.text case final text? when text.isNotEmpty) {
      out.add(TextSpan(text: text, style: style));
    }
    for (final child in span.children ?? const <InlineSpan>[]) {
      _flatten(child, style, out);
    }
  }

  /// 按字符区间 `[start, end)` 截取摊平后的文字；`end` 为 null 表示到末尾。
  static TextSpan _slice(List<InlineSpan> runs, int start, int? end) {
    final children = <InlineSpan>[];
    var offset = 0;
    for (final run in runs) {
      if (run is! TextSpan) {
        if (offset >= start && (end == null || offset < end)) {
          children.add(run);
        }
        offset += 1;
        continue;
      }
      final text = run.text!;
      final runEnd = offset + text.length;
      final from = start.clamp(offset, runEnd);
      final to = (end ?? runEnd).clamp(offset, runEnd);
      if (from < to) {
        children.add(
          TextSpan(
            text: text.substring(from - offset, to - offset),
            style: run.style,
          ),
        );
      }
      offset = runEnd;
    }
    return TextSpan(children: children);
  }
}

class _ScrimLayer extends StatefulWidget {
  const _ScrimLayer({required this.cover, required this.metaBuilder});

  final String? cover;
  final CoverScrimMetaBuilder metaBuilder;

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
    final generation = ++_generation;
    final cover = widget.cover;
    if (cover == null || cover.isEmpty) {
      // 压根没有封面就没有「等它出现」这回事，直接给回退配色——否则信息
      // 无处可落。
      _colors = CoverScrimCache.fallback;
      return;
    }
    // 命中缓存就同步用上：滚回来的卡片不该再淡入一次。
    final cached = CoverScrimCache.peek(cover);
    _colors = cached;
    if (cached != null) return;
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
                padding: const EdgeInsets.fromLTRB(9, 0, 9, 5),
                child: _metaRow(widget.metaBuilder(context, colors)),
              ),
            ),
          ],
        ),
      ),
    );
  }

  /// 信息行。调用方用 `Spacer()` 把后面几项推到右边（见
  /// [CoverScrimMetaBuilder]）。
  ///
  /// 不能把前面的 `Flexible` 和调用方的 `Spacer` 直接放进同一个 Row：两者
  /// 都是 flex 1，剩余宽度会**对半分**，名字短时 `Flexible` 用不完的那一
  /// 半又落在右边那几项的右侧，它们因此不贴边（#245）。所以按第一个
  /// `Spacer` 拆开：它之前的项放进 `Expanded` 吃掉全部剩余宽度，之后的项
  /// 按自身宽度贴右。
  Widget _metaRow(List<Widget> items) {
    final split = items.indexWhere((w) => w is Spacer);
    if (split < 0) return Row(spacing: 4, children: items);
    return Row(
      spacing: 4,
      children: [
        Expanded(child: Row(spacing: 4, children: items.sublist(0, split))),
        ...items.sublist(split + 1),
      ],
    );
  }
}

class _ScrimColorsTween extends Tween<CoverScrimColors> {
  _ScrimColorsTween({super.end});

  @override
  CoverScrimColors lerp(double t) => CoverScrimColors.lerp(begin!, end!, t);
}
