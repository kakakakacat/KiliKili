import 'package:kilikili/common/style.dart';
import 'package:kilikili/common/widgets/badge.dart';
import 'package:kilikili/common/widgets/image/image_save.dart';
import 'package:kilikili/common/widgets/image/network_img_layer.dart';
import 'package:kilikili/utils/date_utils.dart';
import 'package:kilikili/utils/page_utils.dart';
import 'package:kilikili/utils/platform_utils.dart';
import 'package:kilikili/services/rust/rust_search.dart';
import 'package:material_ui/material_ui.dart';

class SearchPgcCard extends StatelessWidget {
  const SearchPgcCard({
    super.key,
    required this.item,
  });

  final SearchPgcItem item;

  void onLongPress() => showPgcCover(
    title: item.title,
    cover: item.cover,
  );

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    const TextStyle style = TextStyle(fontSize: 13);
    return Material(
      type: MaterialType.transparency,
      child: InkWell(
        onTap: () => PageUtils.viewPgc(seasonId: item.seasonId),
        onLongPress: onLongPress,
        onSecondaryTap: PlatformUtils.isMobile ? null : onLongPress,
        child: Padding(
          padding: const EdgeInsets.symmetric(
            horizontal: Style.safeSpace,
            vertical: 5,
          ),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Stack(
                clipBehavior: Clip.none,
                children: [
                  NetworkImgLayer(
                    width: 111,
                    height: 148,
                    src: item.cover,
                  ),
                  PBadge(
                    text: item.seasonTypeName,
                    top: 6.0,
                    right: 6.0,
                    bottom: null,
                    left: null,
                  ),
                ],
              ),
              const SizedBox(width: 10),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text.rich(
                      TextSpan(
                        children: item.titleSpans
                            .map(
                              (e) => TextSpan(
                                text: e.text,
                                style: TextStyle(
                                  color: e.isEm
                                      ? theme.colorScheme.primary
                                      : theme.colorScheme.onSurface,
                                ),
                              ),
                            )
                            .toList(),
                      ),
                    ),
                    const SizedBox(height: 12),
                    Text('评分:${item.mediaScore?.score}', style: style),
                    Text.rich(
                      style: style,
                      TextSpan(
                        children: [
                          if (item.areas?.isNotEmpty == true)
                            TextSpan(text: '${item.areas!}  ·  '),
                          TextSpan(
                            text: DateFormatUtils.dateFormat(item.pubtime),
                          ),
                        ],
                      ),
                    ),
                    Text.rich(
                      style: style,
                      TextSpan(
                        children: [
                          if (item.styles?.isNotEmpty == true)
                            TextSpan(text: '${item.styles!}  ·  '),
                          if (item.indexShow?.isNotEmpty == true)
                            TextSpan(text: item.indexShow!),
                        ],
                      ),
                    ),
                  ],
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
