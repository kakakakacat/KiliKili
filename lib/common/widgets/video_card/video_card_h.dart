import 'package:kilikili/common/widgets/badge.dart';
import 'package:kilikili/common/widgets/image/image_save.dart';
import 'package:kilikili/common/widgets/progress_bar/video_progress_indicator.dart';
import 'package:kilikili/common/widgets/video_card/cover_pill.dart';
import 'package:kilikili/common/widgets/video_card/cover_scrim_card.dart';
import 'package:kilikili/common/widgets/video_popup_menu.dart';
import 'package:kilikili/http/search.dart';
import 'package:kilikili/models/common/badge_type.dart';
import 'package:kilikili/models/common/stat_type.dart';
import 'package:kilikili/models/horizontal_video_model.dart';
import 'package:kilikili/models_new/video/video_detail/dimension.dart';
import 'package:kilikili/utils/date_utils.dart';
import 'package:kilikili/utils/duration_utils.dart';
import 'package:kilikili/utils/page_utils.dart';
import 'package:kilikili/utils/platform_utils.dart';
import 'package:material_ui/material_ui.dart';

/// 搜索、相关推荐与榜单共用的视频封面卡。
class VideoCardH extends StatelessWidget {
  const VideoCardH({
    super.key,
    required this.videoItem,
    this.onTap,
    this.onViewLater,
    this.onRemove,
  });

  final HorizontalVideoModel videoItem;
  final VoidCallback? onTap;
  final ValueChanged<int>? onViewLater;
  final VoidCallback? onRemove;

  void onLongPress() => imageSaveDialog(
    bvid: videoItem.bvid,
    title: videoItem.title,
    cover: videoItem.cover,
  );

  @override
  Widget build(BuildContext context) {
    final progress = videoItem.progress;
    final date = videoItem.pubdate == null
        ? ''
        : DateFormatUtils.dateFormat(videoItem.pubdate!);
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 4),
      child: CoverScrimCard(
        cover: videoItem.cover,
        title: videoItem.title,
        upName: videoItem.owner.name,
        onTap: onTap ?? () => pushVideoH(videoItem),
        onLongPress: onLongPress,
        onSecondaryTap: PlatformUtils.isMobile ? null : onLongPress,
        titleBuilder: videoItem.titleList?.isNotEmpty == true
            ? (context, colors) => Text.rich(
                TextSpan(
                  style: TextStyle(color: colors.title, height: 1.32),
                  children: [
                    for (final span in videoItem.titleList!)
                      TextSpan(
                        text: span.text,
                        style: span.isEm
                            ? const TextStyle(
                                fontWeight: FontWeight.bold,
                                decoration: TextDecoration.underline,
                              )
                            : null,
                      ),
                  ],
                ),
                maxLines: 2,
                overflow: TextOverflow.ellipsis,
              )
            : null,
        topStart: videoItem.isLive == true
            ? null
            : CoverStatsPill(
                stats: [
                  (StatType.play, videoItem.stat.view),
                  (StatType.danmaku, videoItem.stat.danmu),
                ],
              ),
        coverOverlays: [
          if (videoItem.badge case final badge?)
            PBadge(
              text: badge,
              top: 6,
              right: 6,
              type: badge == '充电专属' ? PBadgeType.error : PBadgeType.primary,
            ),
          if (progress != null && progress != 0)
            CoverDurationPill(
              text: progress == -1
                  ? '已看完'
                  : '${DurationUtils.formatDuration(progress)}/${DurationUtils.formatDuration(videoItem.duration)}',
              right: 6,
              top: videoItem.badge == null ? 6 : 32,
            )
          else if (videoItem.duration > 0)
            CoverDurationPill(
              text: DurationUtils.formatDuration(videoItem.duration),
              right: 6,
              top: videoItem.badge == null ? 6 : 32,
            ),
          if (progress != null && progress != 0)
            Positioned(
              left: 0,
              right: 0,
              top: 0,
              child: VideoProgressIndicator(
                color: ColorScheme.of(context).primary,
                backgroundColor: ColorScheme.of(context).secondaryContainer,
                progress: progress == -1 ? 1 : progress / videoItem.duration,
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
            onRemove: onRemove,
          ),
        ),
      ),
    );
  }
}

Future<void> pushVideoH(HorizontalVideoModel videoItem) async {
  if (videoItem.isPugv ?? false) {
    PageUtils.viewPugv(seasonId: videoItem.seasonId);
    return;
  }

  if (videoItem.isLive ?? false) {
    if (videoItem.roomId case final roomId?) {
      PageUtils.toLiveRoom(roomId);
    }
    return;
  }

  if (videoItem.redirectUrl?.isNotEmpty == true &&
      PageUtils.viewPgcFromUri(videoItem.redirectUrl!)) {
    return;
  }

  int? cid = videoItem.cid;
  Dimension? dimension = videoItem.dimension;
  if (cid == null) {
    if (await SearchHttp.ab2cWithDimension(
          aid: videoItem.aid,
          bvid: videoItem.bvid,
        )
        case final res?) {
      cid = res.cid;
      dimension = res.dimension;
    }
  }
  if (cid != null) {
    PageUtils.toVideoPage(
      bvid: videoItem.bvid,
      cid: cid,
      cover: videoItem.cover,
      title: videoItem.title,
      dimension: dimension,
    );
  }
}
