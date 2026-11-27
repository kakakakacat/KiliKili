import 'package:kilikili/common/widgets/stat/stat.dart';
import 'package:kilikili/models/common/stat_type.dart';
import 'package:material_ui/material_ui.dart';

/// 视频封面的整行顶部信息，渐变从封面边缘开始，由卡片圆角统一裁切。
class CoverTopInfoBar extends StatelessWidget {
  const CoverTopInfoBar({super.key, this.stats = const [], this.duration});

  final List<(StatType, dynamic)> stats;
  final String? duration;

  @override
  Widget build(BuildContext context) {
    final visibleStats = stats.where((item) => item.$2 != null).toList();
    if (visibleStats.isEmpty && (duration == null || duration!.isEmpty)) {
      return const SizedBox.shrink();
    }
    return Positioned(
      top: 0,
      left: 0,
      right: 0,
      child: Container(
        height: 48,
        decoration: const BoxDecoration(
          gradient: LinearGradient(
            begin: Alignment.topCenter,
            end: Alignment.bottomCenter,
            colors: [Color(0xCC000000), Color(0x00000000)],
          ),
        ),
        child: Padding(
          padding: const EdgeInsets.fromLTRB(8, 5, 8, 14),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              if (visibleStats.isEmpty) const Spacer(),
              if (visibleStats.isNotEmpty)
                Expanded(
                  flex: 3,
                  child: FittedBox(
                    alignment: Alignment.topLeft,
                    fit: BoxFit.scaleDown,
                    child: Row(
                      mainAxisSize: MainAxisSize.min,
                      spacing: 6,
                      children: [
                        for (final (type, value) in visibleStats)
                          StatWidget(
                            type: type,
                            value: value,
                            color: Colors.white,
                            iconSize: 12,
                          ),
                      ],
                    ),
                  ),
                ),
              if (duration case final text? when text.isNotEmpty) ...[
                if (visibleStats.isNotEmpty) const SizedBox(width: 6),
                Flexible(
                  flex: 2,
                  child: Align(
                    alignment: Alignment.topRight,
                    child: Text(
                      text,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: const TextStyle(fontSize: 12, color: Colors.white),
                    ),
                  ),
                ),
              ],
            ],
          ),
        ),
      ),
    );
  }
}

/// 直播的围观数角标。
///
/// 只有图标和文字，没有底色，直接压在封面左上角（`CoverScrimCard.topStart`
/// 的位置是 `(0, 0)`，边距由这里的 padding 给）。
///
/// 它落在封面上而不是底部遮罩里，**不在 `cover_scrim` 的对比度保证范围
/// 内**；没有底色之后，浅色封面上白字可能看不清。
///
/// 不走 [CoverTopInfoBar]：接口给的已经是「1.2万看过」这种**成品字符串**，
/// 再经 `StatWidget` 的 `numFormat` 会被当成数字二次格式化。
class CoverWatchedPill extends StatelessWidget {
  const CoverWatchedPill({super.key, required this.watched});

  final String? watched;

  @override
  Widget build(BuildContext context) {
    final watched = this.watched;
    if (watched == null || watched.isEmpty) return const SizedBox.shrink();
    return Padding(
      padding: const EdgeInsets.fromLTRB(8, 5, 8, 5),
      child: Row(
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
    );
  }
}
