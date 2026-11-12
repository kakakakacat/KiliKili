import 'package:kilikili/common/widgets/image/image_save.dart';
import 'package:kilikili/common/widgets/video_card/cover_scrim_card.dart';
import 'package:kilikili/common/widgets/video_card/live_subtitle.dart';
import 'package:kilikili/http/live.dart';
import 'package:kilikili/services/rust/rust_live.dart';
import 'package:kilikili/pages/search/widgets/search_text.dart';
import 'package:kilikili/utils/page_utils.dart';
import 'package:kilikili/utils/platform_utils.dart';
import 'package:flutter_smart_dialog/flutter_smart_dialog.dart';
import 'package:get/get.dart';
import 'package:material_ui/material_ui.dart';

// 视频卡片 - 垂直布局
class LiveCardVApp extends StatelessWidget {
  final LiveRoomCard item;
  final bool showFirstFrame;

  const LiveCardVApp({
    super.key,
    required this.item,
    this.showFirstFrame = false,
  });

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final cover = showFirstFrame ? item.systemCover : item.cover;
    void onLongPress() => imageSaveDialog(title: item.title, cover: cover);
    return CoverScrimCard(
      cover: cover,
      title: item.title.toString(),
      upName: item.uname,
      onTap: () => PageUtils.toLiveRoom(item.roomid),
      onLongPress: onLongPress,
      onSecondaryTap: PlatformUtils.isMobile ? null : onLongPress,
      subtitleBuilder: (context, colors) => liveSubtitle(
        colors: colors,
        areaName: item.areaName,
        watched: item.watchedText,
      ),
      trailing: item.feedback.isEmpty
          ? null
          : SizedBox.square(
              dimension: 29,
              child: _feedbackButton(context, theme),
            ),
    );
  }

  /// 「不感兴趣」反馈入口。原本浮在卡片右下角的卡片外侧，现在落在遮罩上。
  ///
  /// 图标色刻意**不**跟遮罩取色走：它是个独立的操作按钮而不是文案，跟着
  /// 封面变色反而会让人以为它和标题是一体的。压在遮罩上统一用白色 + 阴影。
  Widget _feedbackButton(BuildContext context, ThemeData theme) {
    return IconButton(
      padding: EdgeInsets.zero,
      onPressed: () {
        Widget actionButton(LiveFeedbackReason r) => SearchText(
          text: r.name!,
          onTap: (_) async {
            Get.back();
            SmartDialog.showLoading(msg: '正在提交');
            final res = await LiveHttp.liveFeedback(
              item.roomid!,
              r.id!,
              r.idType!,
            );
            SmartDialog.dismiss();
            if (res.isSuccess) {
              SmartDialog.showToast('提交成功');
            } else {
              res.toast();
            }
          },
        );

        final feedback = item.feedback;
        showDialog(
          context: context,
          builder: (context) {
            return SimpleDialog(
              contentPadding: const .fromLTRB(24, 16, 24, 19),
              children: [
                for (var i in feedback) ...[
                  const SizedBox(height: 5),
                  Text.rich(
                    TextSpan(
                      children: [
                        TextSpan(
                          text: i.title,
                          style: theme.textTheme.titleMedium,
                        ),
                        TextSpan(
                          text: '\n${i.subtitle}',
                          style: TextStyle(
                            color: theme.colorScheme.outline,
                          ),
                        ),
                      ],
                    ),
                  ),
                  const SizedBox(height: 5),
                  Wrap(
                    spacing: 8.0,
                    runSpacing: 8.0,
                    children: i.reasons.map(actionButton).toList(),
                  ),
                ],
                const Divider(),
                Center(
                  child: FilledButton.tonal(
                    onPressed: Get.back,
                    style: FilledButton.styleFrom(
                      visualDensity: VisualDensity.compact,
                    ),
                    child: const Text('取消'),
                  ),
                ),
              ],
            );
          },
        );
      },
      icon: const Icon(
        Icons.more_vert_outlined,
        size: 17,
        color: Colors.white,
        shadows: [Shadow(blurRadius: 3, color: Colors.black54)],
      ),
    );
  }
}
