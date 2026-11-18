import 'package:kilikili/common/widgets/badge.dart';
import 'package:kilikili/common/widgets/image/image_save.dart';
import 'package:kilikili/common/widgets/video_card/cover_scrim_card.dart';
import 'package:kilikili/services/rust/rust_user.dart' show SubItem;
import 'package:kilikili/pages/subscription_detail/view.dart';
import 'package:kilikili/utils/bili_utils.dart';
import 'package:kilikili/utils/num_utils.dart';
import 'package:kilikili/utils/platform_utils.dart';
import 'package:kilikili/utils/utils.dart';
import 'package:get/get.dart';
import 'package:material_ui/material_ui.dart';

class MemberFavItem extends StatelessWidget {
  const MemberFavItem({super.key, required this.item, this.onDelete});

  final SubItem item;
  final ValueChanged<bool?>? onDelete;

  @override
  Widget build(BuildContext context) {
    void onLongPress() => imageSaveDialog(title: item.title, cover: item.cover);
    final info = item.kind == 0
        ? '${item.mediaCount}个内容 · ${BiliUtils.isPublicFavText(item.attr)}'
        : item.kind == 11
        ? '${item.mediaCount}个内容 · ${item.upperName}'
        : item.kind == 21
        ? '${item.mediaCount}个视频 · ${NumUtils.numFormat(item.viewCount)}播放'
        : '${item.mediaCount}个内容';
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 4),
      child: CoverScrimCard(
        cover: item.cover,
        title: item.title ?? '',
        upName: item.kind == 21 ? item.upperName : null,
        coverOverlays: [
          if (item.kind == 21)
            const PBadge(right: 6, top: 6, text: '合集')
          else if (item.kind == 11)
            const PBadge(right: 6, top: 6, text: '收藏夹'),
        ],
        metaBuilder: (context, colors) => [
          Flexible(
            child: Text(
              info,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: TextStyle(fontSize: 11, color: colors.subtitle),
            ),
          ),
        ],
        onTap: () async {
          if (item.state == 1) return;
          if (item.kind == 0 || item.kind == 11) {
            final isDeleted = await Get.toNamed(
              '/favDetail',
              parameters: {
                'mediaId': item.id.toString(),
                'heroTag': Utils.makeHeroTag(item.id),
              },
            );
            onDelete?.call(isDeleted);
          } else {
            SubDetailPage.toSubDetailPage(item.id!, subInfo: item);
          }
        },
        onLongPress: onLongPress,
        onSecondaryTap: PlatformUtils.isMobile ? null : onLongPress,
      ),
    );
  }
}
