import 'package:kilikili/common/widgets/image/image_save.dart';
import 'package:kilikili/common/widgets/video_card/cover_scrim_card.dart';
import 'package:kilikili/services/rust/rust_fav.dart';
import 'package:kilikili/utils/bili_utils.dart';
import 'package:material_ui/material_ui.dart';

class FavVideoItem extends StatelessWidget {
  const FavVideoItem({
    super.key,
    this.onTap,
    this.onLongPress,
    required this.heroTag,
    required this.item,
  });

  final String heroTag;
  final FavFolder item;
  final VoidCallback? onTap;
  final VoidCallback? onLongPress;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 4),
      child: Hero(
        tag: heroTag,
        child: CoverScrimCard(
          cover: item.cover,
          title: item.title,
          metaBuilder: (context, colors) => [
            Flexible(
              child: Text(
                [
                  '${item.mediaCount}个内容',
                  BiliUtils.isPublicFavText(item.attr),
                  if (item.intro?.isNotEmpty == true) item.intro!,
                ].join(' · '),
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: TextStyle(fontSize: 11, color: colors.subtitle),
              ),
            ),
          ],
          onTap: onTap,
          onLongPress:
              onLongPress ??
              (onTap == null
                  ? null
                  : () => imageSaveDialog(
                      title: item.title,
                      cover: item.cover,
                    )),
        ),
      ),
    );
  }
}
