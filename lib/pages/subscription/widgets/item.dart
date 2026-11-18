import 'package:kilikili/common/widgets/badge.dart';
import 'package:kilikili/common/widgets/image/image_save.dart';
import 'package:kilikili/common/widgets/video_card/cover_scrim_card.dart';
import 'package:kilikili/services/rust/rust_user.dart' show SubItem;
import 'package:kilikili/pages/subscription_detail/view.dart';
import 'package:kilikili/utils/platform_utils.dart';
import 'package:kilikili/utils/utils.dart';
import 'package:flutter_smart_dialog/flutter_smart_dialog.dart';
import 'package:get/get.dart';
import 'package:material_ui/material_ui.dart';

/// 订阅项的类型标签。
///
/// 是界面文案（i18n），按 AGENTS.md §2 留在宿主——协议那一侧只有数字
/// `kind`（11 收藏夹 / 21 合集）。基线把它写在 `SubItemModel` 上，模型随
/// S8 删掉后落在这里。
extension SubItemLabel on SubItem {
  String get type2Label => switch (kind) {
    11 => '收藏夹',
    21 => '合集',
    _ => '其它($kind)',
  };
}

/// 订阅列表里的一张卡片。
///
/// 叫 `SubFolderCard` 而不是 `SubItem`：后者现在是 Rust 导出的那个数据
/// 类型（`rust_user.dart`），同名会把两者混在一起。
class SubFolderCard extends StatelessWidget {
  final SubItem item;
  final VoidCallback cancelSub;
  const SubFolderCard({
    super.key,
    required this.item,
    required this.cancelSub,
  });

  void onLongPress() => imageSaveDialog(title: item.title, cover: item.cover);

  @override
  Widget build(BuildContext context) {
    final heroTag = Utils.makeHeroTag(item.id);
    final label = item.type2Label;
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 4),
      child: Hero(
        tag: heroTag,
        child: CoverScrimCard(
          cover: item.cover,
          title: item.title ?? '',
          upName: item.upperName,
          coverOverlays: [PBadge(right: 6, top: 6, text: label)],
          metaBuilder: (context, colors) => [
            Text(
              '${item.mediaCount}个视频',
              style: TextStyle(fontSize: 11, color: colors.subtitle),
            ),
          ],
          trailing: IconButton(
            onPressed: cancelSub,
            icon: const Icon(
              Icons.delete_outline,
              size: 18,
              color: Colors.white,
            ),
            tooltip: '取消订阅',
          ),
          onTap: () => onPushSub(label, heroTag, item),
          onLongPress: onLongPress,
          onSecondaryTap: PlatformUtils.isMobile ? null : onLongPress,
        ),
      ),
    );
  }
}

Future<void>? onPushSub(String label, String heroTag, SubItem item) {
  if (item.state == 1) {
    SmartDialog.showToast('该$label已失效');
    return null;
  }
  if (item.kind == 11) {
    return Get.toNamed(
      '/favDetail',
      parameters: {
        'mediaId': item.id!.toString(),
        'heroTag': heroTag,
      },
    );
  } else {
    return SubDetailPage.toSubDetailPage(
      item.id!,
      heroTag: heroTag,
      subInfo: item,
    );
  }
}
