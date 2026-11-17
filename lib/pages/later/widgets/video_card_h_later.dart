import 'package:kilikili/common/widgets/badge.dart';
import 'package:kilikili/common/widgets/progress_bar/video_progress_indicator.dart';
import 'package:kilikili/common/widgets/select_mask.dart';
import 'package:kilikili/common/widgets/video_card/cover_scrim_card.dart';
import 'package:kilikili/http/search.dart';
import 'package:kilikili/models/common/stat_type.dart';
import 'package:kilikili/models_new/video/video_detail/dimension.dart';
import 'package:kilikili/services/rust/rust_history.dart';
import 'package:kilikili/pages/later/controller.dart';
import 'package:kilikili/utils/duration_utils.dart';
import 'package:kilikili/utils/page_utils.dart';
import 'package:kilikili/utils/platform_utils.dart';
import 'package:flutter_smart_dialog/flutter_smart_dialog.dart';
import 'package:material_ui/material_ui.dart';

/// 角标。基线把它做成 `LaterItemModel.label`（一个返回 Widget 的 getter）
/// ——那是渲染，随 S7 #57 搬到这里；判据所需的原始值
/// （`isCharging` / `isCooperation` / `pgcLabel` / `isPugv`）由 Rust 给。
Widget? laterLabel(LaterItem item) {
  final duration = item.duration;
  // #124 的顶部信息行不存在时，保留角标原来的贴顶位置。
  final top =
      (duration != null && duration > 0) ||
          (item.isPgc != true && (item.view != null || item.danmaku != null))
      ? 54.0
      : 6.0;
  if (item.isCharging) {
    return PBadge(text: '充电专属', top: top, right: 6.0, type: .error);
  }
  if (item.isCooperation == 1) {
    return PBadge(text: '合作', top: top, right: 6.0);
  }
  if (item.pgcLabel != null) {
    return PBadge(text: item.pgcLabel, top: top, right: 6.0);
  }
  if (item.isPugv == true) {
    return PBadge(text: '课堂', top: top, right: 6.0);
  }
  return null;
}

/// 封面上的进度条与时长角标。
///
/// 与 [laterLabel] 同理：基线把它做成 `LaterItemModel.progressIndicator`，
/// 是渲染，随 S7 #57 搬到这里。列表卡片自己内联画了一份（它把时长放在
/// `topDuration`），这个版本给「我的」页那种叠在封面上的小卡片用。
Iterable<Widget> laterProgressIndicator(
  LaterItem item,
  ColorScheme colorScheme,
) sync* {
  final progress = item.progress;
  final duration = item.duration;
  if (progress != null && progress != 0 && duration != null && duration != 0) {
    yield PBadge(
      text: progress == -1
          ? '已看完'
          : '${DurationUtils.formatDuration(progress)}/${DurationUtils.formatDuration(duration)}',
      right: 6,
      bottom: 8,
      type: .gray,
    );
    yield Positioned(
      left: 0,
      bottom: 0,
      right: 0,
      child: VideoProgressIndicator(
        color: colorScheme.primary,
        backgroundColor: colorScheme.secondaryContainer,
        progress: progress == -1 ? 1 : progress / duration,
      ),
    );
  } else if (duration != null && duration > 0) {
    yield PBadge(
      text: DurationUtils.formatDuration(duration),
      right: 6.0,
      bottom: 6.0,
      type: .gray,
    );
  }
}

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
  final LaterItem videoItem;
  final ValueChanged<int> onViewLater;

  @override
  Widget build(BuildContext context) {
    final colorScheme = ColorScheme.of(context);
    final enableMultiSelect = ctr.enableMultiSelect.value;
    final progress = videoItem.progress;
    final duration = videoItem.duration;
    final hasProgress =
        progress != null && progress != 0 && duration != null && duration != 0;
    final isPgc = videoItem.isPgc == true && videoItem.seasonTitle != null;
    final label = laterLabel(videoItem);
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
            ? videoItem.seasonTitle ?? videoItem.title ?? ''
            : videoItem.title ?? '',
        upName: isPgc ? null : videoItem.ownerName,
        onTap: enableMultiSelect
            ? () => ctr.onSelect(videoItem)
            : () => onPushLater(videoItem, onViewLater: onViewLater),
        onLongPress: onLongPress,
        onSecondaryTap: PlatformUtils.isMobile ? null : onLongPress,
        topStats: isPgc
            ? const []
            : [
                (StatType.play, videoItem.view),
                (StatType.danmaku, videoItem.danmaku),
              ],
        topDuration: hasProgress || (duration != null && duration > 0)
            ? hasProgress
                  ? progress == -1
                        ? '已看完'
                        : '${DurationUtils.formatDuration(progress)}/${DurationUtils.formatDuration(duration)}'
                  : DurationUtils.formatDuration(duration)
            : null,
        coverOverlays: [
          ?label,
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
          Positioned.fill(
            child: selectMask(colorScheme, ctr.isChecked(videoItem)),
          ),
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
  LaterItem item, {
  ValueChanged<int>? onViewLater,
}) async {
  if (item.isPugv ?? false) {
    PageUtils.viewPugv(seasonId: item.aid);
    return;
  }
  if (item.isPgc ?? false) {
    if (item.epId != null) {
      PageUtils.viewPgc(epId: item.epId, viewLater: true);
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
          // Rust 给的宽高已按 rotate 摆正，这里只是装进宿主的容器。
          dimension: item.width == null || item.height == null
              ? null
              : Dimension(width: item.width, height: item.height),
          extraArguments: const {'viewLater': true},
        );
      }
    }
  } catch (err) {
    SmartDialog.showToast(err.toString());
  }
}
