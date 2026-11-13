import 'package:kilikili/common/widgets/badge.dart';
import 'package:kilikili/common/widgets/progress_bar/video_progress_indicator.dart';
import 'package:kilikili/common/widgets/select_mask.dart';
import 'package:kilikili/common/widgets/video_card/cover_pill.dart';
import 'package:kilikili/common/widgets/video_card/cover_scrim_card.dart';
import 'package:kilikili/http/search.dart';
import 'package:kilikili/models/common/badge_type.dart';
import 'package:kilikili/models/common/stat_type.dart';
import 'package:kilikili/models_new/later/list.dart';
import 'package:kilikili/pages/later/controller.dart';
import 'package:kilikili/utils/duration_utils.dart';
import 'package:kilikili/utils/page_utils.dart';
import 'package:kilikili/utils/platform_utils.dart';
import 'package:flutter_smart_dialog/flutter_smart_dialog.dart';
import 'package:material_ui/material_ui.dart';

class VideoCardHLater extends StatelessWidget {
  const VideoCardHLater({
    super.key,
    required this.ctr,
    required this.index,
    required this.videoItem,
    required this.onViewLater,
  });

  final int index;
  final BaseLaterController ctr;
  final LaterItemModel videoItem;
  final ValueChanged<int> onViewLater;

  @override
  Widget build(BuildContext context) {
    final colorScheme = ColorScheme.of(context);
    final enableMultiSelect = ctr.enableMultiSelect.value;
    final progress = videoItem.progress;
    final duration = videoItem.duration;
    final hasProgress =
        progress != null && progress != 0 && duration != null && duration != 0;
    final isPgc = videoItem.isPgc == true && videoItem.bangumi != null;
    final label = videoItem.label;
    final onLongPress = enableMultiSelect
        ? null
        : () => ctr
            ..enableMultiSelect.value = true
            ..onSelect(videoItem);
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 4),
      child: CoverScrimCard(
        cover: videoItem.pic,
        title: isPgc
            ? videoItem.bangumi!.season?.title ?? videoItem.title ?? ''
            : videoItem.title ?? '',
        upName: isPgc ? null : videoItem.owner?.name,
        onTap: enableMultiSelect
            ? () => ctr.onSelect(videoItem)
            : () => onPushLater(videoItem, onViewLater: onViewLater),
        onLongPress: onLongPress,
        onSecondaryTap: PlatformUtils.isMobile ? null : onLongPress,
        topStart: isPgc
            ? null
            : CoverStatsPill(
                stats: [
                  (StatType.play, videoItem.stat?.view),
                  (StatType.danmaku, videoItem.stat?.danmaku),
                ],
              ),
        coverOverlays: [
          ?label,
          if (hasProgress || (duration != null && duration > 0))
            PBadge(
              text: hasProgress
                  ? progress == -1
                        ? '已看完'
                        : '${DurationUtils.formatDuration(progress)}/${DurationUtils.formatDuration(duration)}'
                  : DurationUtils.formatDuration(duration),
              top: label == null ? 6 : 32,
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
                progress: progress == -1 ? 1 : progress / duration,
              ),
            ),
          Positioned.fill(child: selectMask(colorScheme, videoItem.checked)),
        ],
        metaBuilder: isPgc && videoItem.subtitle?.isNotEmpty == true
            ? (context, colors) => [
                Flexible(
                  child: Text(
                    videoItem.subtitle!,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: TextStyle(fontSize: 11, color: colors.subtitle),
                  ),
                ),
              ]
            : null,
        trailing: SizedBox.square(
          dimension: 29,
          child: IconButton(
            tooltip: '移除',
            onPressed: () => ctr.toViewDel(context, index, videoItem.aid!),
            icon: const Icon(Icons.clear, color: Colors.white, size: 18),
          ),
        ),
      ),
    );
  }
}

Future<void> onPushLater(
  LaterItemModel item, {
  ValueChanged<int>? onViewLater,
}) async {
  if (item.isPugv ?? false) {
    PageUtils.viewPugv(seasonId: item.aid);
    return;
  }
  if (item.isPgc ?? false) {
    if (item.bangumi?.epId != null) {
      PageUtils.viewPgc(epId: item.bangumi!.epId, viewLater: true);
    } else if (item.redirectUrl?.isNotEmpty == true) {
      PageUtils.viewPgcFromUri(item.redirectUrl!, viewLater: true);
    }
    return;
  }
  try {
    final cid =
        item.cid ??
        await SearchHttp.ab2c(
          aid: item.aid,
          bvid: item.bvid,
        );
    if (cid != null) {
      if (onViewLater != null) {
        onViewLater(cid);
      } else {
        await PageUtils.toVideoPage(
          bvid: item.bvid,
          cid: cid,
          cover: item.pic,
          title: item.title,
          dimension: item.dimension,
          extraArguments: const {'viewLater': true},
        );
      }
    }
  } catch (err) {
    SmartDialog.showToast(err.toString());
  }
}
