import 'package:kilikili/common/widgets/badge.dart';
import 'package:kilikili/common/widgets/image/image_save.dart';
import 'package:kilikili/common/widgets/video_card/cover_scrim_card.dart';
import 'package:kilikili/models_new/sub/sub/list.dart';
import 'package:kilikili/pages/subscription_detail/view.dart';
import 'package:kilikili/utils/platform_utils.dart';
import 'package:kilikili/utils/utils.dart';
import 'package:flutter_smart_dialog/flutter_smart_dialog.dart';
import 'package:get/get.dart';
import 'package:material_ui/material_ui.dart';

class SubItem extends StatelessWidget {
  final SubItemModel item;
  final VoidCallback cancelSub;
  const SubItem({
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
          upName: item.upper?.name,
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

Future<void>? onPushSub(String label, String heroTag, SubItemModel item) {
  if (item.state == 1) {
    SmartDialog.showToast('该$label已失效');
    return null;
  }
  if (item.type == 11) {
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
