import 'package:kilikili/common/widgets/badge.dart';
import 'package:kilikili/common/widgets/image/image_save.dart';
import 'package:kilikili/common/widgets/progress_bar/video_progress_indicator.dart';
import 'package:kilikili/common/widgets/video_card/cover_pill.dart';
import 'package:kilikili/common/widgets/video_card/cover_scrim_card.dart';
import 'package:kilikili/common/widgets/video_popup_menu.dart';
import 'package:kilikili/models/common/badge_type.dart';
import 'package:kilikili/models/common/stat_type.dart';
import 'package:kilikili/models_new/space/space_archive/item.dart';
import 'package:kilikili/utils/date_utils.dart';
import 'package:kilikili/utils/duration_utils.dart';
import 'package:kilikili/utils/extension/dimension_ext.dart';
import 'package:kilikili/utils/page_utils.dart';
import 'package:kilikili/utils/platform_utils.dart';
import 'package:material_ui/material_ui.dart';

class VideoCardHMemberVideo extends StatelessWidget {
  const VideoCardHMemberVideo({
    super.key,
    required this.videoItem,
    this.onTap,
    this.bvid,
    this.fromViewAid,
  });

  final SpaceArchiveItem videoItem;
  final VoidCallback? onTap;
  final dynamic bvid;
  final String? fromViewAid;

  void _open() {
    if (onTap case final tap?) {
      tap();
      return;
    }
    final isPgc = videoItem.isPgc == true;
    final isPugv = videoItem.isPugv == true;
    if ((isPgc || isPugv) && videoItem.uri?.isNotEmpty == true) {
      if (PageUtils.viewPgcFromUri(videoItem.uri!, isPgc: isPgc)) return;
    }
    if (videoItem.bvid == null || videoItem.cid == null) return;
    bool isVertical = false;
    if (videoItem.uri case final uri?) isVertical = uri.isVerticalFromUri;
    PageUtils.toVideoPage(
      bvid: videoItem.bvid,
      cid: videoItem.cid!,
      cover: videoItem.cover,
      title: videoItem.title,
      isVertical: isVertical,
    );
  }

  @override
  Widget build(BuildContext context) {
    final colorScheme = ColorScheme.of(context);
    final isCurr =
        fromViewAid == videoItem.param ||
        (videoItem.bvid != null && videoItem.bvid == bvid);
    final history = videoItem.history;
    final progress = history?.progress;
    final duration = history?.duration;
    final hasProgress = progress != null && duration != null && duration > 0;
    final badge = videoItem.badges?.isNotEmpty == true
        ? videoItem.badges!.map((item) => item.text).join('|')
        : null;
    final date = videoItem.season != null
        ? DateFormatUtils.dateFormat(videoItem.season!.mtime)
        : videoItem.publishTimeText ?? '';
    void onLongPress() => imageSaveDialog(
      title: videoItem.title,
      cover: videoItem.cover,
      bvid: videoItem.bvid,
    );
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 4),
      child: CoverScrimCard(
        cover: videoItem.cover,
        title: videoItem.title,
        titleStyle: isCurr
            ? const TextStyle(fontWeight: FontWeight.bold, height: 1.32)
            : null,
        onTap: _open,
        onLongPress: onLongPress,
        onSecondaryTap: PlatformUtils.isMobile ? null : onLongPress,
        topStart: CoverStatsPill(
          stats: [
            (StatType.play, videoItem.stat.view),
            (StatType.danmaku, videoItem.stat.danmu),
          ],
        ),
        coverOverlays: [
          if (badge != null)
            PBadge(
              text: badge,
              top: 6,
              right: 6,
              type: videoItem.badges!.first.text == '充电专属'
                  ? PBadgeType.error
                  : PBadgeType.primary,
            ),
          if (isCurr)
            const PBadge(text: '上次观看', top: 34, left: 6, type: PBadgeType.gray),
          if (hasProgress || videoItem.duration > 0)
            PBadge(
              text: hasProgress
                  ? progress == duration
                        ? '已看完'
                        : '${DurationUtils.formatDuration(progress)}/${DurationUtils.formatDuration(duration)}'
                  : DurationUtils.formatDuration(videoItem.duration),
              top: badge == null ? 6 : 32,
              right: 6,
              type: PBadgeType.gray,
            ),
        ],
        foregroundOverlays: [
          if (hasProgress)
            Positioned(
              left: 0,
              right: 0,
              bottom: 0,
              child: VideoProgressIndicator(
                color: colorScheme.primary,
                backgroundColor: colorScheme.secondaryContainer,
                progress: progress / duration,
              ),
            ),
        ],
        metaBuilder: date.isEmpty
            ? null
            : (context, colors) => [
                const Spacer(),
                Text(
                  date,
                  maxLines: 1,
                  style: TextStyle(fontSize: 11, color: colors.subtitle),
                ),
              ],
        trailing: SizedBox.square(
          dimension: 29,
          child: VideoPopupMenu(
            iconSize: 17,
            onCover: true,
            videoItem: videoItem,
          ),
        ),
      ),
    );
  }
}
