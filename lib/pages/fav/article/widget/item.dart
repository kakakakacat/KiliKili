import 'package:kilikili/common/style.dart';
import 'package:kilikili/common/widgets/button/icon_button.dart';
import 'package:kilikili/common/widgets/image/network_img_layer.dart';
import 'package:kilikili/common/widgets/stat/stat.dart';
import 'package:kilikili/models/common/stat_type.dart';
import 'package:kilikili/services/rust/rust_fav.dart';
import 'package:kilikili/pages/article/view.dart';
import 'package:material_ui/material_ui.dart';

class FavArticleCard extends StatelessWidget {
  const FavArticleCard({
    super.key,
    required this.item,
    required this.onDelete,
  });

  final FavArticleItem item;
  final VoidCallback onDelete;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Material(
      type: MaterialType.transparency,
      child: Stack(
        clipBehavior: Clip.none,
        children: [
          InkWell(
            onTap: () => ArticlePage.toArticlePage(
              id: item.opusId!,
              type: .opus,
            ),
            child: Padding(
              padding: const EdgeInsets.symmetric(
                horizontal: Style.safeSpace,
                vertical: 5,
              ),
              child: Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  if (item.coverUrl != null) ...[
                    AspectRatio(
                      aspectRatio: Style.aspectRatio,
                      child: LayoutBuilder(
                        builder: (context, boxConstraints) {
                          return NetworkImgLayer(
                            src: item.coverUrl,
                            width: boxConstraints.maxWidth,
                            height: boxConstraints.maxHeight,
                          );
                        },
                      ),
                    ),
                    const SizedBox(width: 10),
                  ],
                  Expanded(
                    child: Column(
                      mainAxisSize: MainAxisSize.min,
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Expanded(
                          child: Text(
                            item.content!,
                            style: TextStyle(
                              fontSize: theme.textTheme.bodyMedium!.fontSize,
                              height: 1.42,
                              letterSpacing: 0.3,
                            ),
                            maxLines: 2,
                            overflow: TextOverflow.ellipsis,
                          ),
                        ),
                        Text(
                          item.authorName ?? '',
                          maxLines: 1,
                          style: TextStyle(
                            fontSize: 13,
                            height: 1,
                            color: theme.colorScheme.outline,
                          ),
                        ),
                        const SizedBox(height: 3),
                        Row(
                          children: [
                            StatWidget(
                              type: StatType.like,
                              value: item.like,
                              color: theme.colorScheme.outline,
                            ),
                            Text(
                              '  ·  ${item.pubTime}',
                              maxLines: 1,
                              style: TextStyle(
                                fontSize: 13,
                                height: 1,
                                color: theme.colorScheme.outline,
                              ),
                            ),
                          ],
                        ),
                      ],
                    ),
                  ),
                ],
              ),
            ),
          ),
          Positioned(
            right: 12,
            bottom: -6,
            child: iconButton(
              iconSize: 18,
              onPressed: onDelete,
              icon: const Icon(Icons.clear),
              iconColor: theme.colorScheme.outline,
            ),
          ),
        ],
      ),
    );
  }
}
