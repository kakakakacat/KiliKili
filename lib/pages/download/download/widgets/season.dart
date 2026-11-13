import 'package:kilikili/common/widgets/badge.dart';
import 'package:kilikili/common/widgets/dialog/dialog.dart';
import 'package:kilikili/common/widgets/dialog/simple_dialog_option.dart';
import 'package:kilikili/common/widgets/video_card/cover_scrim_card.dart';
import 'package:kilikili/common/widgets/select_mask.dart';
import 'package:kilikili/models_new/download/bili_download_entry_info.dart';
import 'package:kilikili/models_new/download/download_info.dart';
import 'package:kilikili/pages/common/multi_select/base.dart';
import 'package:kilikili/pages/download/page/view.dart';
import 'package:kilikili/services/download/download_service.dart';
import 'package:kilikili/utils/platform_utils.dart';
import 'package:kilikili/utils/storage.dart';
import 'package:get/get_core/src/get_main.dart';
import 'package:get/get_navigation/src/extension_navigation.dart';
import 'package:material_ui/material_ui.dart';

class SeasonInfoItem extends StatelessWidget {
  const SeasonInfoItem({
    super.key,
    required this.controller,
    required this.downloadService,
    required this.season,
    required this.seasonInfo,
    required this.enableMultiSelect,
    required this.progress,
    required this.updateSeasonDm,
    required this.updateSeasonSeg,
    required this.updateSeasonVolume,
    required this.updateSeasonPlayInfo,
    required this.updateSeasonSeasonInfo,
  });

  final BaseMultiSelectMixin<DownloadSeasonInfo> controller;
  final DownloadService downloadService;
  final DownloadSeasonInfo season;
  final SeasonInfo seasonInfo;
  final bool enableMultiSelect;
  final ChangeNotifier progress;
  final ValueChanged<DownloadSeasonInfo> updateSeasonDm;
  final ValueChanged<DownloadSeasonInfo> updateSeasonSeg;
  final ValueChanged<DownloadSeasonInfo> updateSeasonVolume;
  final ValueChanged<DownloadSeasonInfo> updateSeasonPlayInfo;
  final ValueChanged<DownloadSeasonInfo> updateSeasonSeasonInfo;

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
                        for (final pageInfo in season.pages) {
                          await GStorage.watchProgress.deleteAll(
                            pageInfo.entries.map((e) => e.cid.toString()),
                          );
                          downloadService.deletePage(
                            pageDirPath: pageInfo.dirPath,
                          );
                        }
                      },
                    );
                  },
                  child: const Text('删除'),
                ),
                DialogOption(
                  onPressed: () {
                    Get.back();
                    updateSeasonDm(season);
                  },
                  child: const Text('更新弹幕'),
                ),
                DialogOption(
                  onPressed: () {
                    Get.back();
                    updateSeasonSeg(season);
                  },
                  child: const Text('更新片段'),
                ),
                DialogOption(
                  onPressed: () {
                    Get.back();
                    updateSeasonSeasonInfo(season);
                  },
                  child: const Text('更新合集信息'),
                ),
                DialogOption(
                  onPressed: () {
                    Get.back();
                    updateSeasonVolume(season);
                  },
                  child: const Text('更新音量均衡'),
                ),
                DialogOption(
                  onPressed: () {
                    Get.back();
                    updateSeasonPlayInfo(season);
                  },
                  child: const Text('更新字幕、分段信息'),
                ),
              ],
            ),
          );

    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 4),
      child: CoverScrimCard(
        cover: seasonInfo.cover,
        title: seasonInfo.title,
        upName: seasonInfo.uname,
        coverOverlays: [
          const PBadge(text: '合集', top: 6, right: 6),
          PBadge(
            text: '${season.pages.fold(0, (a, b) => a + b.entries.length)}个视频',
            left: 6,
            top: 6,
            isBold: false,
            type: .gray,
          ),
        ],
        foregroundOverlays: [
          Positioned.fill(child: selectMask(colorScheme, season.checked)),
        ],
        onTap: () {
          if (controller.enableMultiSelect.value) {
            controller.onSelect(season);
            return;
          }
          Get.to(DownloadPagePage(seasonInfo: seasonInfo, progress: progress));
        },
        onLongPress: onLongPress,
        onSecondaryTap: PlatformUtils.isMobile ? null : onLongPress,
      ),
    );
  }
}
