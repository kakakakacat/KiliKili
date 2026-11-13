import 'package:kilikili/common/widgets/video_card/cover_pill.dart';
import 'package:kilikili/common/widgets/progress_bar/video_progress_indicator.dart';
import 'package:kilikili/common/widgets/select_mask.dart';
import 'package:kilikili/common/widgets/video_card/cover_scrim_card.dart';
import 'package:kilikili/http/search.dart';
import 'package:kilikili/http/user.dart';
import 'package:kilikili/models_new/history/list.dart';
import 'package:kilikili/models_new/video/video_detail/dimension.dart';
import 'package:kilikili/pages/article/view.dart';
import 'package:kilikili/pages/common/multi_select/base.dart';
import 'package:kilikili/utils/date_utils.dart';
import 'package:kilikili/utils/duration_utils.dart';
import 'package:kilikili/utils/id_utils.dart';
import 'package:kilikili/utils/page_utils.dart';
import 'package:kilikili/utils/platform_utils.dart';
import 'package:flutter_smart_dialog/flutter_smart_dialog.dart';
import 'package:get/get.dart';
import 'package:material_design_icons_flutter/material_design_icons_flutter.dart';
import 'package:material_ui/material_ui.dart';

class HistoryItem extends StatelessWidget {
  const HistoryItem({
    super.key,
    required this.item,
    required this.ctr,
    required this.onDelete,
  });

  final HistoryItemModel item;
  final MultiSelectBase ctr;
  final void Function(int kid, String business) onDelete;

  @override
  Widget build(BuildContext context) {
    final colorScheme = ColorScheme.of(context);
    final business = item.history.business;
    final enableMultiSelect = ctr.enableMultiSelect.value;
    final progress = item.progress;
    final hasProgress = item.hasDuration && progress != null && progress != 0;
    final label = item.label;
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
            : item.covers?.firstOrNull,
        title: item.title ?? '',
        titleMaxLines: (item.videos ?? 0) > 1 ? 1 : 2,
        upName: item.authorName,
        onTap: enableMultiSelect
            ? () => ctr.onSelect(item)
            : () => onPushHistory(item),
        onLongPress: onLongPress,
        onSecondaryTap: PlatformUtils.isMobile ? null : onLongPress,
        coverOverlays: [
          ?label,
          if (item.hasDuration)
            CoverDurationPill(
              text: progress == -1
                  ? '已看完'
                  : '${DurationUtils.formatDuration(progress)}/${DurationUtils.formatDuration(item.duration)}',
              right: 6,
              top: label == null ? 6 : 32,
            ),
        ],
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
            child: selectMask(colorScheme, item.checked),
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
          const Spacer(),
          if (date.isNotEmpty)
            Text(
              date,
              maxLines: 1,
              style: TextStyle(fontSize: 11, color: colors.subtitle),
            ),
        ],
        trailing: SizedBox.square(
          dimension: 29,
          child: PopupMenuButton(
            padding: EdgeInsets.zero,
            tooltip: '功能菜单',
            icon: const Icon(
              Icons.more_vert_outlined,
              color: Colors.white,
              size: 18,
            ),
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
                  onTap: () => UserHttp.toViewLater(bvid: item.history.bvid),
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

Future<void> onPushHistory(HistoryItemModel item) async {
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
      progress: item.playbackProgress,
    );
  } else if (business == 'cheese') {
    if (item.uri?.isNotEmpty == true) {
      PageUtils.viewPgcFromUri(
        item.uri!,
        isPgc: false,
        aid: item.history.oid,
        progress: item.playbackProgress,
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
        progress: item.playbackProgress,
      );
    }
  }
}
