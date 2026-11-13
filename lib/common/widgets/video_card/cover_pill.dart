import 'package:kilikili/common/style.dart';
import 'package:kilikili/common/widgets/stat/stat.dart';
import 'package:kilikili/models/common/stat_type.dart';
import 'package:material_ui/material_ui.dart';

/// 压在封面上的半透明深色小药丸。
///
/// 它落在封面上而不是底部遮罩里，**不在 `cover_scrim` 的对比度保证范围
/// 内**——那套保证只对遮罩合成出来的那块背景成立。所以这里固定用
/// `black45` + 白字。统计与时长共用这个容器，避免两侧文字行高和内边距
/// 不同导致角标高低不齐。
class CoverPill extends StatelessWidget {
  const CoverPill({super.key, required this.children});

  final List<Widget> children;

  @override
  Widget build(BuildContext context) {
    if (children.isEmpty) return const SizedBox.shrink();
    return DecoratedBox(
      decoration: const BoxDecoration(
        color: Colors.black45,
        borderRadius: Style.coverCardRadius,
      ),
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          spacing: 6,
          children: children,
        ),
      ),
    );
  }
}

/// 播放量 / 弹幕数这类带图标的统计药丸。
class CoverStatsPill extends StatelessWidget {
  const CoverStatsPill({super.key, required this.stats});

  /// 每项一个 `(类型, 值)`；值为 null 的项自动略过，省得每个调用点都写一遍
  /// 条件。
  final List<(StatType, dynamic)> stats;

  @override
  Widget build(BuildContext context) => CoverPill(
    children: [
      for (final (type, value) in stats)
        if (value != null)
          StatWidget(
            type: type,
            value: value,
            color: Colors.white,
            iconSize: 12,
          ),
    ],
  );
}

/// 封面右上角的时长或观看进度，与左侧统计药丸共用尺寸和底色。
class CoverDurationPill extends StatelessWidget {
  const CoverDurationPill({
    super.key,
    required this.text,
    this.top = 6,
    this.right = 6,
  });

  final String text;
  final double top;
  final double right;

  @override
  Widget build(BuildContext context) => Positioned(
    top: top,
    right: right,
    child: CoverPill(
      children: [
        Text(
          text,
          style: const TextStyle(fontSize: 12, color: Colors.white),
        ),
      ],
    ),
  );
}

/// 直播的围观数药丸。
///
/// 不走 [CoverStatsPill]：接口给的已经是「1.2万看过」这种**成品字符串**，
/// 再经 `StatWidget` 的 `numFormat` 会被当成数字二次格式化。
class CoverWatchedPill extends StatelessWidget {
  const CoverWatchedPill({super.key, required this.watched});

  final String? watched;

  @override
  Widget build(BuildContext context) {
    final watched = this.watched;
    if (watched == null || watched.isEmpty) return const SizedBox.shrink();
    return CoverPill(
      children: [
        Row(
          mainAxisSize: MainAxisSize.min,
          spacing: 2,
          children: [
            Icon(
              StatType.view.iconData,
              size: 12,
              color: Colors.white,
              semanticLabel: StatType.view.label,
            ),
            Text(
              watched,
              style: const TextStyle(fontSize: 12, color: Colors.white),
            ),
          ],
        ),
      ],
    );
  }
}
