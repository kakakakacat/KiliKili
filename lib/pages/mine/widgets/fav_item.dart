import 'package:kilikili/common/widgets/image/network_img_layer.dart';
import 'package:kilikili/pages/mine/widgets/mine_surfaces.dart';
import 'package:kilikili/services/rust/rust_fav.dart';
import 'package:kilikili/utils/bili_utils.dart';
import 'package:kilikili/utils/utils.dart';
import 'package:get/get.dart';
import 'package:material_ui/material_ui.dart';

class FavFolderItem extends StatelessWidget {
  const FavFolderItem({
    super.key,
    required this.item,
    required this.onPop,
  });

  final FavFolder item;
  final VoidCallback onPop;

  @override
  Widget build(BuildContext context) {
    final heroTag = Utils.makeHeroTag(item.id);
    return MineCollectionTile(
      onTap: () => Get.toNamed(
        '/favDetail',
        arguments: item,
        parameters: {'mediaId': item.id.toString(), 'heroTag': heroTag},
      )?.whenComplete(onPop),
      cover: Hero(
        tag: heroTag,
        child: NetworkImgLayer(src: item.cover, width: 112, height: 90),
      ),
      title: item.title,
      subtitle:
          '${item.mediaCount} 条视频 · ${BiliUtils.isPublicFavText(item.attr)}',
    );
  }
}
