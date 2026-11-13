import 'package:kilikili/common/widgets/badge.dart';
import 'package:kilikili/common/widgets/image/image_save.dart';
import 'package:kilikili/common/widgets/video_card/cover_scrim_card.dart';
import 'package:kilikili/services/rust/rust_search.dart';
import 'package:kilikili/utils/date_utils.dart';
import 'package:kilikili/utils/page_utils.dart';
import 'package:kilikili/utils/platform_utils.dart';
import 'package:material_ui/material_ui.dart';

class SearchPgcCard extends StatelessWidget {
  const SearchPgcCard({super.key, required this.item});

  final SearchPgcItem item;

  void onLongPress() => showPgcCover(title: item.title, cover: item.cover);

  @override
  Widget build(BuildContext context) {
    final date = DateFormatUtils.dateFormat(item.pubtime);
    final areas = item.areas?.isNotEmpty == true ? item.areas! : '';
    final styles = item.styles?.isNotEmpty == true ? item.styles! : '';
    final index = item.indexShow?.isNotEmpty == true ? item.indexShow! : '';
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 4),
      child: CoverScrimCard(
        cover: item.cover,
        title: item.title,
        onTap: () => PageUtils.viewPgc(seasonId: item.seasonId),
        onLongPress: onLongPress,
        onSecondaryTap: PlatformUtils.isMobile ? null : onLongPress,
        coverOverlays: [
          PBadge(text: item.seasonTypeName, top: 6, right: 6),
        ],
        titleBuilder: (context, colors) => Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text.rich(
              TextSpan(
                style: TextStyle(color: colors.title, height: 1.32),
                children: [
                  for (final span in item.titleSpans)
                    TextSpan(
                      text: span.text,
                      style: span.isEm
                          ? const TextStyle(
                              fontWeight: FontWeight.bold,
                              decoration: TextDecoration.underline,
                            )
                          : null,
                    ),
                ],
              ),
              maxLines: 2,
              overflow: TextOverflow.ellipsis,
            ),
            if (item.mediaScore?.score != null ||
                areas.isNotEmpty ||
                date.isNotEmpty)
              Text(
                [
                  if (item.mediaScore?.score != null)
                    '评分:${item.mediaScore!.score}',
                  if (areas.isNotEmpty) areas,
                  if (date.isNotEmpty) date,
                ].join(' · '),
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: TextStyle(fontSize: 11, color: colors.subtitle),
              ),
            if (styles.isNotEmpty || index.isNotEmpty)
              Text(
                [
                  if (styles.isNotEmpty) styles,
                  if (index.isNotEmpty) index,
                ].join(' · '),
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: TextStyle(fontSize: 11, color: colors.subtitle),
              ),
          ],
        ),
      ),
    );
  }
}
