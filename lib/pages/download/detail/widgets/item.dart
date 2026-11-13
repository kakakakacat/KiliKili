import 'dart:async';
import 'dart:io';

import 'package:kilikili/common/style.dart';
import 'package:kilikili/common/widgets/video_card/cover_pill.dart';
import 'package:kilikili/common/widgets/dialog/dialog.dart';
import 'package:kilikili/common/widgets/dialog/simple_dialog_option.dart';
import 'package:kilikili/common/widgets/video_card/cover_scrim_card.dart';
import 'package:kilikili/common/widgets/progress_bar/video_progress_indicator.dart';
import 'package:kilikili/common/widgets/select_mask.dart';
import 'package:kilikili/models/common/sponsor_block/segment_type.dart';
import 'package:kilikili/models/common/video/source_type.dart';
import 'package:kilikili/models_new/download/bili_download_entry_info.dart';
import 'package:kilikili/pages/common/multi_select/base.dart';
import 'package:kilikili/pages/download/downloading/view.dart';
import 'package:kilikili/services/download/download_service.dart';
import 'package:kilikili/utils/duration_utils.dart';
import 'package:kilikili/utils/extension/num_ext.dart';
import 'package:kilikili/utils/page_utils.dart';
import 'package:kilikili/utils/path_utils.dart';
import 'package:kilikili/utils/platform_utils.dart';
import 'package:kilikili/utils/storage.dart';
import 'package:flutter_smart_dialog/flutter_smart_dialog.dart';
import 'package:get/get.dart';
import 'package:material_ui/material_ui.dart';
import 'package:path/path.dart' as path;

class DetailItem extends StatelessWidget {
  const DetailItem({
    super.key,
    required this.entry,
    this.progress,
    required this.downloadService,
    this.onDelete,
    required this.showTitle,
    this.isCurr = false,
    //
    required this.controller,
    this.checked,
    this.onSelect,
  });

  final BiliDownloadEntryInfo entry;
  final ChangeNotifier? progress;
  final DownloadService downloadService;
  final VoidCallback? onDelete;
  final bool showTitle;
  final bool isCurr;
  //
  final MultiSelectBase controller;
  final bool? checked;
  final ValueChanged<BiliDownloadEntryInfo>? onSelect;

  void showSegmentsDetail(BuildContext context) => showDialog(
    context: context,
    builder: (context) => SimpleDialog(
      clipBehavior: .hardEdge,
      contentPadding: const .symmetric(vertical: 10),
      children: entry.segments!
          .map(
            (item) => ListTile(
              dense: true,
              title: Text(SegmentType.values.byName(item.category).title),
              contentPadding: const .only(left: 16, right: 8),
              subtitle: Text(
                '${DurationUtils.formatDuration(item.segment.first / 1000)} 至 ${DurationUtils.formatDuration(item.segment[1] / 1000)}',
                style: const TextStyle(fontSize: 13),
              ),
            ),
          )
          .toList(),
    ),
  );

  Future<void> _onUpdate(Future<bool> Function() onUpdate) async {
    Get.back();
    final res = await onUpdate();
    if (res) {
      SmartDialog.showToast('更新成功');
    } else {
      SmartDialog.showToast('更新失败');
    }
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final cid = entry.source?.cid ?? entry.pageData?.cid;
    final canDel = onDelete != null;
    final enableMultiSelect = controller.enableMultiSelect.value;
    void onLongPress() => canDel && !enableMultiSelect
        ? showDialog(
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
                      title: const Text('确定删除该视频？'),
                      onConfirm: onDelete,
                    );
                  },
                  child: const Text('删除'),
                ),
                DialogOption(
                  onPressed: () => _onUpdate(
                    () => downloadService.downloadDanmaku(
                      entry: entry,
                      isUpdate: true,
                    ),
                  ),
                  child: const Text('更新弹幕'),
                ),
                if (entry.segments != null)
                  DialogOption(
                    onPressed: () {
                      Get.back();
                      showSegmentsDetail(context);
                    },
                    child: const Text('空降片段'),
                  ),
                if (entry.isCompleted) ...[
                  if (entry.ep != null)
                    DialogOption(
                      onPressed: () => _onUpdate(
                        () => downloadService.updatePgcSegmentsAndVolume(entry),
                      ),
                      child: const Text('更新空降、音量均衡'),
                    )
                  else ...[
                    DialogOption(
                      onPressed: () => _onUpdate(
                        () => downloadService.updateUgcSeasonInfo(
                          entry,
                          refresh: true,
                        ),
                      ),
                      child: const Text('更新合集信息'),
                    ),
                    DialogOption(
                      onPressed: () => _onUpdate(
                        () => downloadService.updateUgcSegments(entry),
                      ),
                      child: const Text('更新空降片段'),
                    ),
                    DialogOption(
                      onPressed: () => _onUpdate(
                        () => downloadService.updateUgcVolume(entry),
                      ),
                      child: const Text('更新音量均衡'),
                    ),
                  ],
                  DialogOption(
                    onPressed: () =>
                        _onUpdate(() => downloadService.updatePlayInfo(entry)),
                    child: const Text('更新字幕、分段信息'),
                  ),
                ],
              ],
            ),
          )
        : null;

    final coverPath = path.join(entry.entryDirPath, PathUtils.coverName);
    final hasLocalCover = File(coverPath).existsSync();
    final totalDuration = entry.totalTimeMilli ~/ 1000;
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 4),
      child: CoverScrimCard(
        cover: entry.cover,
        localCover: hasLocalCover ? coverPath : null,
        title: showTitle ? entry.title : entry.showTitle,
        upName: entry.ownerName,
        coverOverlays: [
          entry.qaBadge,
          if (totalDuration != 0)
            CoverDurationPill(
              text: DurationUtils.formatDuration(totalDuration),
              right: 6,
              top: 6,
            ),
        ],
        foregroundOverlays: [
          Positioned.fill(
            child: selectMask(theme.colorScheme, checked ?? entry.checked),
          ),
          if (progress != null && cid != null)
            ListenableBuilder(
              listenable: progress!,
              builder: (context, _) {
                final watched = GStorage.watchProgress.get(cid.toString());
                return watched == null || totalDuration == 0
                    ? const SizedBox.shrink()
                    : Positioned(
                        left: 0,
                        right: 0,
                        bottom: 0,
                        child: VideoProgressIndicator(
                          color: theme.colorScheme.primary,
                          backgroundColor: theme.colorScheme.secondaryContainer,
                          progress: watched / entry.totalTimeMilli,
                        ),
                      );
              },
            ),
        ],
        metaBuilder: (context, colors) => [
          if (entry.isCompleted)
            Flexible(
              child: Text(
                entry.totalBytes.formatSize,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: TextStyle(fontSize: 11, color: colors.subtitle),
              ),
            )
          else
            Flexible(
              child: isCurr
                  ? Obx(() {
                      final current = downloadService.curDownload.value;
                      return Text(
                        current?.status.message ?? entry.status.message,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: TextStyle(fontSize: 11, color: colors.subtitle),
                      );
                    })
                  : Text(
                      entry.status.message,
                      style: TextStyle(fontSize: 11, color: colors.subtitle),
                    ),
            ),
        ],
        trailing: entry.isCompleted
            ? entry.moreBtn(theme.colorScheme.copyWith(outline: Colors.white))
            : null,
        onTap: () async {
          if (!canDel) {
            Get.to(const DownloadingPage());
            return;
          }
          if (enableMultiSelect) {
            (onSelect ?? controller.onSelect).call(entry);
            return;
          }
          if (entry.isCompleted) {
            await PageUtils.toVideoPage(
              aid: entry.avid,
              cid: cid!,
              cover: entry.cover,
              title: entry.showTitle,
              isVertical: entry.pageData?.isVertical ?? false,
              extraArguments: {
                'sourceType': SourceType.file,
                'entry': entry,
                'dirPath': entry.entryDirPath,
              },
            );
            if (context.mounted) {
              Timer(const Duration(milliseconds: 400), () {
                if (context.mounted) {
                  // ignore: invalid_use_of_protected_member, invalid_use_of_visible_for_testing_member
                  progress?.notifyListeners();
                }
              });
            }
          } else {
            final curDownload = downloadService.curDownload.value;
            if (curDownload != null &&
                curDownload.cid == cid &&
                curDownload.status.isDownloading) {
              downloadService.cancelDownload(
                isDelete: false,
                downloadNext: false,
              );
            } else {
              downloadService.queueDownload(entry);
            }
          }
        },
        onLongPress: onLongPress,
        onSecondaryTap: PlatformUtils.isMobile ? null : onLongPress,
      ),
    );
  }

  Widget entryProgress(ThemeData theme) => progressWidget(
    statusMsg: entry.status.message,
    progressStr: entry.totalBytes == 0
        ? ''
        : '${entry.downloadedBytes.formatSize}/${entry.totalBytes.formatSize}',
    progress: entry.totalBytes == 0
        ? 0
        : entry.downloadedBytes / entry.totalBytes,
    color: theme.colorScheme.outline,
    highlightColor: theme.highlightColor,
  );

  Widget progressWidget({
    required String statusMsg,
    required String progressStr,
    required double progress,
    required Color color,
    required Color highlightColor,
  }) {
    return Column(
      spacing: 6,
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Row(
          mainAxisAlignment: MainAxisAlignment.spaceBetween,
          children: [
            Text(
              statusMsg,
              style: TextStyle(
                fontSize: 12,
                height: 1,
                color: color,
              ),
            ),
            Text(
              progressStr,
              style: TextStyle(
                fontSize: 12,
                height: 1,
                color: color,
              ),
            ),
          ],
        ),
        LinearProgressIndicator(
          // ignore: deprecated_member_use
          year2023: true,
          minHeight: 2.5,
          borderRadius: Style.mdRadius,
          color: color,
          backgroundColor: highlightColor,
          value: progress,
        ),
      ],
    );
  }
}
