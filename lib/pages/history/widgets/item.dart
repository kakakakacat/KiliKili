import 'package:kilikili/common/widgets/badge.dart';
import 'package:kilikili/common/widgets/progress_bar/video_progress_indicator.dart';
import 'package:kilikili/common/widgets/select_mask.dart';
import 'package:kilikili/common/widgets/video_card/cover_scrim_card.dart';
import 'package:kilikili/http/search.dart';
import 'package:kilikili/services/rust/rust_history.dart';
import 'package:kilikili/models_new/video/video_detail/dimension.dart';
import 'package:kilikili/pages/article/view.dart';
import 'package:kilikili/pages/common/multi_select/id_based.dart';
import 'package:kilikili/utils/date_utils.dart';
import 'package:kilikili/utils/duration_utils.dart';
import 'package:kilikili/utils/id_utils.dart';
import 'package:kilikili/utils/page_utils.dart';
import 'package:kilikili/utils/platform_utils.dart';
import 'package:flutter_smart_dialog/flutter_smart_dialog.dart';
import 'package:get/get.dart';
import 'package:material_design_icons_flutter/material_design_icons_flutter.dart';
import 'package:material_ui/material_ui.dart';

/// 角标。基线把它做成 `HistoryItemModel.label`（一个返回 Widget 的 getter）
/// ——那是渲染，随 S7 #57 搬到这里；判据所需的原始值
/// （`isFav` / `badge` / `liveStatus`）由 Rust 给。
Widget? historyLabel(HistoryItem item) {
  final hasDuration = item.duration != null && item.duration != 0;
  final top = hasDuration ? 54.0 : 6.0;
  if (item.isFav == 1) {
    return PBadge(text: '已收藏', top: top, right: 6.0, type: .gray);
  }
  if (item.badge?.isNotEmpty == true) {
    return PBadge(
      text: item.badge,
      top: top,
      right: 6.0,
      // 直播条目没在播时角标置灰，与基线一致。
      type: item.history.business == 'live' && item.liveStatus != 1
          ? .gray
          : .primary,
    );
  }
  return null;
}

/// 历史记录的一张卡片。
///
/// 类名带 `Card` 后缀是为了给 Rust 的实体让出 `HistoryItem` 这个名字——
/// 协议里的东西叫什么，导出面就叫什么，宿主的 widget 换个名字更便宜。
/// 封面上的进度条与「看到哪」角标。
///
/// 与 [historyLabel] 同理：基线把它做成 `HistoryItemModel.progressIndicator`，
/// 是渲染，随 S7 #57 搬到这里。列表卡片自己内联画了一份（它要把两件事拆到
/// `topDuration` 与 `foregroundOverlays` 两个位置），这个版本给「我的」页
/// 那种把两样叠在封面上的小卡片用。
Iterable<Widget> historyProgressIndicator(
  HistoryItem item,
  ColorScheme colorScheme,
) sync* {
  final progress = item.progress;
  final duration = item.duration;
  yield PBadge(
    text: progress == -1
        ? '已看完'
        : '${DurationUtils.formatDuration(progress)}/${DurationUtils.formatDuration(duration)}',
    right: 6.0,
    bottom: 8.0,
    type: .gray,
  );
  if (progress != null && progress != 0 && duration != null && duration != 0) {
    yield Positioned(
      left: 0,
      right: 0,
      bottom: 0,
      child: VideoProgressIndicator(
        color: colorScheme.primary,
        backgroundColor: colorScheme.secondaryContainer,
        progress: progress == -1 ? 1 : progress / duration,
      ),
    );
  }
}

class HistoryItemCard extends StatelessWidget {
  const HistoryItemCard({
    super.key,
    required this.item,
    required this.ctr,
    required this.onDelete,
  });

  final HistoryItem item;
  final IdMultiSelectMixin<dynamic, HistoryItem, String> ctr;
  final void Function(int kid, String business) onDelete;

  @override
  Widget build(BuildContext context) {
    final colorScheme = ColorScheme.of(context);
    final business = item.history.business;
    final enableMultiSelect = ctr.enableMultiSelect.value;
    final progress = item.progress;
    final hasDuration = item.duration != null && item.duration != 0;
    final hasProgress = hasDuration && progress != null && progress != 0;
    final label = historyLabel(item);
    final date = item.viewAt == null
        ? ''
        : DateFormatUtils.chatFormat(item.viewAt!, isHistory: true);
    final onLongPress = enableMultiSelect
        ? null
        : () => ctr
            ..enableMultiSelect.value = true
            ..onSelect(item);

    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 4),
      child: CoverScrimCard(
        cover: item.cover?.isNotEmpty == true
            ? item.cover
            : item.covers.firstOrNull,
        title: item.title ?? '',
        upName: item.authorName,
        onTap: enableMultiSelect
            ? () => ctr.onSelect(item)
            : () => onPushHistory(item),
        onLongPress: onLongPress,
        onSecondaryTap: PlatformUtils.isMobile ? null : onLongPress,
        topDuration: hasDuration
            ? progress == -1
                  ? '已看完'
                  : '${DurationUtils.formatDuration(progress)}/${DurationUtils.formatDuration(item.duration)}'
            : null,
        coverOverlays: [?label],
        foregroundOverlays: [
          if (hasProgress && item.duration != null && item.duration != 0)
            Positioned(
              left: 0,
              right: 0,
              bottom: 0,
              child: VideoProgressIndicator(
                color: colorScheme.primary,
                backgroundColor: colorScheme.secondaryContainer,
                progress: progress == -1 ? 1 : progress / item.duration!,
              ),
            ),
          Positioned.fill(
            child: selectMask(colorScheme, ctr.isChecked(item)),
          ),
        ],
        metaBuilder: (context, colors) => [
          if (item.history.business == 'pgc' &&
              item.showTitle?.isNotEmpty == true)
            Flexible(
              child: Text(
                item.showTitle!,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: TextStyle(fontSize: 11, color: colors.subtitle),
              ),
            ),
        ],
        date: date,
        trailing: SizedBox.square(
          dimension: 29,
          child: PopupMenuButton(
            padding: EdgeInsets.zero,
            tooltip: '功能菜单',
            icon: const Icon(Icons.more_vert_outlined, size: 18),
            position: PopupMenuPosition.under,
            itemBuilder: (_) => [
              if (item.authorMid != null && item.authorName?.isNotEmpty == true)
                PopupMenuItem(
                  onTap: () => Get.toNamed('/member?mid=${item.authorMid}'),
                  height: 38,
                  child: Row(
                    children: [
                      const Icon(MdiIcons.accountCircleOutline, size: 16),
                      const SizedBox(width: 6),
                      Text(
                        '访问：${item.authorName}',
                        style: const TextStyle(fontSize: 13),
                      ),
                    ],
                  ),
                ),
              if (business != 'pgc' &&
                  item.badge != '番剧' &&
                  item.tagName?.contains('动画') != true &&
                  business != 'live' &&
                  business?.contains('article') != true)
                PopupMenuItem(
                  onTap: () => RustHistory.toViewLater(bvid: item.history.bvid),
                  height: 38,
                  child: const Row(
                    children: [
                      Icon(Icons.watch_later_outlined, size: 16),
                      SizedBox(width: 6),
                      Text('稍后再看', style: TextStyle(fontSize: 13)),
                    ],
                  ),
                ),
              PopupMenuItem(
                onTap: () => onDelete(item.kid!, business!),
                height: 38,
                child: const Row(
                  children: [
                    Icon(Icons.close_outlined, size: 16),
                    SizedBox(width: 6),
                    Text('删除记录', style: TextStyle(fontSize: 13)),
                  ],
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

Future<void> onPushHistory(HistoryItem item) async {
  final business = item.history.business;
  if (business?.contains('article') == true) {
    ArticlePage.toArticlePage(
      id: (business == 'article-list' ? item.history.cid : item.history.oid)
          .toString(),
      type: .read,
    );
  } else if (business == 'live') {
    if (item.liveStatus == 1) {
      PageUtils.toLiveRoom(item.history.oid);
    } else {
      SmartDialog.showToast('直播未开播');
    }
  } else if (business == 'pgc') {
    PageUtils.viewPgc(
      epId: item.history.epid,
      progress: RustHistory.playbackProgressMs(item.progress),
    );
  } else if (business == 'cheese') {
    if (item.uri?.isNotEmpty == true) {
      PageUtils.viewPgcFromUri(
        item.uri!,
        isPgc: false,
        aid: item.history.oid,
        progress: RustHistory.playbackProgressMs(item.progress),
      );
    }
  } else {
    final aid = item.history.oid!;
    final bvid = item.history.bvid ?? IdUtils.av2bv(aid);
    int? cid = item.history.cid;
    Dimension? dimension;
    if (cid == null) {
      if (await SearchHttp.ab2cWithDimension(
            aid: aid,
            bvid: bvid,
            part: item.history.page,
          )
          case final res?) {
        cid = res.cid;
        dimension = res.dimension;
      }
    }
    if (cid != null) {
      // TODO: dimension
      PageUtils.toVideoPage(
        aid: aid,
        bvid: bvid,
        cid: cid,
        cover: item.cover,
        title: item.title,
        dimension: dimension,
        progress: RustHistory.playbackProgressMs(item.progress),
      );
    }
  }
}
