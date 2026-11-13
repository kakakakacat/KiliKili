import 'package:kilikili/common/widgets/badge.dart';
import 'package:kilikili/common/widgets/dialog/dialog.dart';
import 'package:kilikili/common/widgets/dialog/simple_dialog_option.dart';
import 'package:kilikili/common/widgets/video_card/cover_scrim_card.dart';
import 'package:kilikili/common/widgets/select_mask.dart';
import 'package:kilikili/models_new/download/download_info.dart';
import 'package:kilikili/pages/common/multi_select/base.dart';
import 'package:kilikili/pages/download/detail/view.dart';
import 'package:kilikili/services/download/download_service.dart';
import 'package:kilikili/utils/bili_utils.dart';
import 'package:kilikili/utils/extension/num_ext.dart';
import 'package:kilikili/utils/platform_utils.dart';
import 'package:kilikili/utils/storage.dart';
import 'package:get/get_core/src/get_main.dart';
import 'package:get/get_navigation/src/extension_navigation.dart';
import 'package:material_ui/material_ui.dart';

class PageInfoItem<T extends MultiSelectData> extends StatelessWidget {
  const PageInfoItem({
    super.key,
    required this.controller,
    required this.downloadService,
    required this.seasonInfo,
    required this.pageInfo,
    required this.enableMultiSelect,
    required this.progress,
    required this.updatePageDm,
    required this.updatePageSeg,
    required this.updatePageVolume,
    required this.updatePagePlayInfo,
    required this.updatePageSeasonInfo,
  });

  final BaseMultiSelectMixin<T> controller;
  final DownloadService downloadService;
  final T seasonInfo;
  final DownloadPageInfo pageInfo;
  final bool enableMultiSelect;
  final ChangeNotifier progress;
  final ValueChanged<DownloadPageInfo> updatePageDm;
  final ValueChanged<DownloadPageInfo> updatePageSeg;
  final ValueChanged<DownloadPageInfo> updatePageVolume;
  final ValueChanged<DownloadPageInfo> updatePagePlayInfo;
  final ValueChanged<DownloadPageInfo> updatePageSeasonInfo;

  @override
  Widget build(BuildContext context) {
    final colorScheme = ColorScheme.of(context);

    void onLongPress() => enableMultiSelect
        ? null
        : showDialog(
            context: context,
            builder: (context) => SimpleDialog(
              clipBehavior: Clip.hardEdge,
              contentPadding: const EdgeInsets.symmetric(vertical: 12),
              children: [
                DialogOption(
                  onPressed: () {
                    Get.back();
                    showConfirmDialog(
                      context: context,
                      title: const Text('确定删除？'),
                      onConfirm: () async {
                        await GStorage.watchProgress.deleteAll(
                          pageInfo.entries.map((e) => e.cid.toString()),
                        );
                        downloadService.deletePage(
                          pageDirPath: pageInfo.dirPath,
                        );
                      },
                    );
                  },
                  child: const Text('删除'),
                ),
                DialogOption(
                  onPressed: () {
                    Get.back();
                    updatePageDm(pageInfo);
                  },
                  child: const Text('更新弹幕'),
                ),
                DialogOption(
                  onPressed: () {
                    Get.back();
                    updatePageSeg(pageInfo);
                  },
                  child: const Text('更新片段'),
                ),
                DialogOption(
                  onPressed: () {
                    Get.back();
                    updatePageSeasonInfo(pageInfo);
                  },
                  child: const Text('更新合集信息'),
                ),
                DialogOption(
                  onPressed: () {
                    Get.back();
                    updatePageVolume(pageInfo);
                  },
                  child: const Text('更新音量均衡'),
                ),
                DialogOption(
                  onPressed: () {
                    Get.back();
                    updatePagePlayInfo(pageInfo);
                  },
                  child: const Text('更新字幕、分段信息'),
                ),
              ],
            ),
          );

    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 4),
      child: CoverScrimCard(
        cover: pageInfo.cover,
        title: pageInfo.title,
        upName: pageInfo.entries.first.ownerName,
        coverOverlays: [
          PBadge(
            text: '${pageInfo.entries.length}个视频',
            left: 6,
            top: 6,
            isBold: false,
            type: .gray,
          ),
          if (pageInfo.seasonType case final pgcType?)
            PBadge(
              text: BiliUtils.pgcType2Label(pgcType),
              right: 6,
              top: 6,
            ),
        ],
        foregroundOverlays: [
          Positioned.fill(child: selectMask(colorScheme, seasonInfo.checked)),
        ],
        metaBuilder: (context, colors) => [
          Text(
            pageInfo.entries.fold(0, (p, n) => p + n.totalBytes).formatSize,
            style: TextStyle(fontSize: 11, color: colors.subtitle),
          ),
        ],
        trailing: pageInfo.entries.first.moreBtn(
          colorScheme.copyWith(outline: Colors.white),
        ),
        onTap: () {
          if (controller.enableMultiSelect.value) {
            controller.onSelect(seasonInfo);
            return;
          }
          Get.to(
            DownloadDetailPage(
              pageId: pageInfo.pageId,
              title: pageInfo.title,
              progress: progress,
            ),
          );
        },
        onLongPress: onLongPress,
        onSecondaryTap: PlatformUtils.isMobile ? null : onLongPress,
      ),
    );
  }
}
