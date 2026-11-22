import 'package:kilikili/common/widgets/dialog/dialog.dart';
import 'package:kilikili/pages/common/common_whisper_controller.dart';
import 'package:kilikili/pages/contact/view.dart';
import 'package:kilikili/pages/whisper_settings/view.dart';
import 'package:kilikili/services/rust/rust_im.dart';
import 'package:flutter_smart_dialog/flutter_smart_dialog.dart';
import 'package:get/get.dart';
import 'package:material_design_icons_flutter/material_design_icons_flutter.dart';
import 'package:material_ui/material_ui.dart';

/// 私信列表顶部「…」菜单里那一项的图标与动作。
///
/// 类型从 protobuf 的 `ThreeDotItemType` 换成了 Rust 投影出来的
/// [ImThreeDotKind]（S12 续单 / #63）。两边的分支一一对应，只是枚举里
/// **只有界面真的有分支的那几种**，其余在 Rust 侧就收敛成了
/// [ImThreeDotKind.other]——协议里那十种有四种基线从来没处理过。
extension ImThreeDotKindExt on ImThreeDotKind {
  Icon get icon => switch (this) {
    ImThreeDotKind.msgSetting => const Icon(Icons.settings, size: 20),
    ImThreeDotKind.readAll => const Icon(Icons.cleaning_services, size: 20),
    ImThreeDotKind.clearList => const Icon(
      Icons.delete_forever_outlined,
      size: 20,
    ),
    ImThreeDotKind.upHelper => const Icon(Icons.live_tv, size: 20),
    ImThreeDotKind.contacts => const Icon(
      Icons.account_box_outlined,
      size: 20,
    ),
    ImThreeDotKind.fansGroupHelper => const Icon(
      Icons.notifications_none,
      size: 20,
    ),
    ImThreeDotKind.other => const Icon(MdiIcons.circleMedium, size: 20),
  };

  void action({
    required BuildContext context,
    required CommonWhisperController controller,
    required ImThreeDotItem item,
  }) {
    switch (this) {
      case ImThreeDotKind.readAll:
        showConfirmDialog(
          context: context,
          title: const Text('一键已读'),
          content: const Text('是否清除全部新消息提醒？'),
          onConfirm: controller.onClearUnread,
        );
      case ImThreeDotKind.clearList:
        showConfirmDialog(
          context: context,
          title: const Text('清空列表'),
          content: const Text('清空后所有消息将被删除，无法恢复'),
          onConfirm: controller.onDeleteList,
        );
      case ImThreeDotKind.msgSetting:
        Get.to(
          WhisperSettingsPage(imSettingType: RustIm.settingPageAll),
        );
      case ImThreeDotKind.upHelper:
        final matched = RegExp(
          r'/(\d{3,})',
        ).firstMatch(item.url ?? '')?.group(1);
        if (matched != null) {
          final talkerId = int.parse(matched);
          Get.toNamed(
            '/whisperDetail',
            arguments: {
              'talkerId': talkerId,
              'name': item.title,
              'face': switch (talkerId) {
                844424930131966 => 'https://message.biliimg.com/bfs/im/489a63efadfb202366c2f88853d2217b5ddc7a13.png',
                844424930131964 => 'https://i0.hdslb.com/bfs/im_new/58eda511672db078466e7ab8db22a95c1503684976.png',
                _ => item.icon,
              },
            },
          );
        }
      case ImThreeDotKind.contacts:
        Get.to(const ContactPage(isFromSelect: false));
      case ImThreeDotKind.fansGroupHelper:
      case ImThreeDotKind.other:
        SmartDialog.showToast('TODO: ${item.title ?? name}');
    }
  }
}
