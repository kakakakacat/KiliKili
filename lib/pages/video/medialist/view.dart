import 'dart:math';

import 'package:kilikili/common/widgets/badge.dart';
import 'package:kilikili/common/widgets/button/icon_button.dart';
import 'package:kilikili/common/widgets/dialog/dialog.dart';
import 'package:kilikili/common/widgets/flutter/refresh_indicator.dart';
import 'package:kilikili/common/widgets/image/image_save.dart';
import 'package:kilikili/common/widgets/video_card/cover_scrim_card.dart';
import 'package:kilikili/models/common/badge_type.dart';
import 'package:kilikili/models_new/media_list/media_list.dart';
import 'package:kilikili/models_new/video/video_detail/episode.dart';
import 'package:kilikili/pages/common/slide/common_slide_page.dart';
import 'package:kilikili/utils/duration_utils.dart';
import 'package:kilikili/utils/num_utils.dart';
import 'package:kilikili/utils/platform_utils.dart';
import 'package:kilikili/utils/scroll_offset_helper.dart';
import 'package:flutter_smart_dialog/flutter_smart_dialog.dart';
import 'package:get/get.dart';
import 'package:material_design_icons_flutter/material_design_icons_flutter.dart';
import 'package:material_ui/material_ui.dart';

class MediaListPanel extends CommonSlidePage {
  const MediaListPanel({
    super.key,
    required this.mediaList,
    required this.onChangeEpisode,
    this.panelTitle,
    required this.bvid,
    required this.loadMoreMedia,
    required this.count,
    required this.desc,
    required this.onReverse,
    required this.loadPrevious,
    this.onDelete,
  });

  final RxList<MediaListItemModel> mediaList;
  final ValueChanged<BaseEpisodeItem> onChangeEpisode;
  final String? panelTitle;
  final String bvid;
  final VoidCallback loadMoreMedia;
  final int? count;
  final bool desc;
  final VoidCallback onReverse;
  final RefreshCallback? loadPrevious;
  final void Function(MediaListItemModel item, int index)? onDelete;

  @override
  State<MediaListPanel> createState() => _MediaListPanelState();
}

class _MediaListPanelState extends State<MediaListPanel>
    with SingleTickerProviderStateMixin, CommonSlideMixin, ScrollOffsetMixin {
  @override
  final scrollController = ScrollController();
  @override
  late final int itemIndex;
  @override
  int get itemCount => widget.mediaList.length;

  @override
  void initState() {
    super.initState();
    final bvid = widget.bvid;
    itemIndex = max(
      0,
      widget.mediaList.indexWhere((item) => item.bvid == bvid),
    );
    if (itemIndex > 0) {
      WidgetsBinding.instance.addPostFrameCallback(jumpToCurrent);
    }
  }

  @override
  void dispose() {
    scrollController.dispose();
    super.dispose();
  }

  @override
  Widget buildPage() {
    return Material(
      color: theme.colorScheme.surface,
      child: Column(
        children: [
          Container(
            height: 45,
            decoration: BoxDecoration(
              border: Border(
                bottom: BorderSide(
                  color: theme.colorScheme.outline.withValues(alpha: 0.1),
                ),
              ),
            ),
            child: Row(
              children: [
                const SizedBox(width: 16),
                Expanded(
                  child: Text(
                    widget.panelTitle ?? '稍后再看',
                    style: const TextStyle(fontSize: 16),
                  ),
                ),
                iconButton(
                  iconSize: 20,
                  tooltip: widget.desc ? '顺序播放' : '倒序播放',
                  icon: widget.desc
                      ? const Icon(MdiIcons.sortAscending)
                      : const Icon(MdiIcons.sortDescending),
                  onPressed: () {
                    Get.back();
                    widget.onReverse();
                  },
                ),
                iconButton(
                  iconSize: 20,
                  tooltip: '关闭',
                  icon: const Icon(Icons.close),
                  onPressed: Get.back,
                ),
                const SizedBox(width: 14),
              ],
            ),
          ),
          Expanded(child: enableSlide ? slideList() : buildList()),
        ],
      ),
    );
  }

  @override
  Widget buildList() {
    return widget.loadPrevious != null
        ? refreshIndicator(
            onRefresh: widget.loadPrevious!,
            child: _buildList(),
          )
        : _buildList();
  }

  Widget _buildList() {
    final showDelBtn = widget.onDelete != null && widget.mediaList.length > 1;
    return CustomScrollView(
      controller: scrollController,
      physics: const AlwaysScrollableScrollPhysics(),
      slivers: [
        SliverPadding(
          padding: EdgeInsets.only(
            top: 7,
            bottom: MediaQuery.viewPaddingOf(context).bottom + 100,
          ),
          sliver: Obx(
            () => SliverFixedExtentList.builder(
              itemExtent: 200,
              itemCount: widget.mediaList.length,
              itemBuilder: (context, index) {
                if (index == widget.mediaList.length - 1 &&
                    (widget.count == null ||
                        widget.mediaList.length < widget.count!)) {
                  widget.loadMoreMedia();
                }
                final item = widget.mediaList[index];
                final isCurr = item.bvid == widget.bvid;
                return _buildItem(index, item, isCurr, showDelBtn);
              },
            ),
          ),
        ),
      ],
    );
  }

  Widget _buildItem(
    int index,
    MediaListItemModel item,
    bool isCurr,
    bool showDelBtn,
  ) {
    void onLongPress() => imageSaveDialog(
      title: item.title,
      cover: item.cover,
      aid: item.aid,
      bvid: item.bvid,
    );
    return Center(
      child: SizedBox(
        width: 256,
        height: 200,
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 4),
          child: CoverScrimCard(
            cover: item.cover,
            title: item.title ?? '',
            upName: item.upper?.name,
            titleStyle: TextStyle(fontWeight: isCurr ? FontWeight.bold : null),
            titleBuilder: item.type == 24 && item.intro?.isNotEmpty == true
                ? (context, colors) => Column(
                    mainAxisSize: MainAxisSize.min,
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        item.title ?? '',
                        maxLines: 2,
                        overflow: TextOverflow.ellipsis,
                        style: TextStyle(
                          color: colors.title,
                          fontWeight: isCurr ? FontWeight.bold : null,
                        ),
                      ),
                      Text(
                        item.intro!,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: TextStyle(fontSize: 11, color: colors.subtitle),
                      ),
                    ],
                  )
                : null,
            coverOverlays: [
              if (item.badge?.isNotEmpty == true)
                PBadge(
                  text: item.badge,
                  right: 6,
                  top: 6,
                  type: item.badge == '充电专属'
                      ? PBadgeType.error
                      : PBadgeType.primary,
                ),
              if (item.duration != 0)
                PBadge(
                  text: DurationUtils.formatDuration(item.duration),
                  right: 6,
                  bottom: 6,
                  type: PBadgeType.gray,
                ),
              if (isCurr) const PBadge(text: '正在播放', left: 6, top: 6),
            ],
            metaBuilder: (context, colors) => [
              if (item.type == 2 && item.cntInfo != null)
                Flexible(
                  child: Text(
                    '${NumUtils.numFormat(item.cntInfo!.play)}播放 · ${NumUtils.numFormat(item.cntInfo!.danmaku)}弹幕',
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: TextStyle(fontSize: 11, color: colors.subtitle),
                  ),
                ),
            ],
            trailing: showDelBtn && !isCurr
                ? IconButton(
                    onPressed: () => showConfirmDialog(
                      context: context,
                      title: const Text('确定移除该视频？'),
                      onConfirm: () => widget.onDelete!(item, index),
                    ),
                    icon: const Icon(
                      Icons.clear,
                      size: 18,
                      color: Colors.white,
                    ),
                    tooltip: '移除视频',
                  )
                : null,
            onTap: () {
              if (item.type != 2) {
                SmartDialog.showToast('不支持播放该类型视频');
                return;
              }
              Get.back();
              widget.onChangeEpisode(item);
            },
            onLongPress: onLongPress,
            onSecondaryTap: PlatformUtils.isMobile ? null : onLongPress,
          ),
        ),
      ),
    );
  }
}
