import 'package:kilikili/common/widgets/image/image_save.dart';
import 'package:kilikili/common/widgets/video_card/cover_pill.dart';
import 'package:kilikili/common/widgets/video_card/cover_scrim_card.dart';
import 'package:kilikili/models/common/stat_type.dart';
import 'package:kilikili/utils/app_scheme.dart';
import 'package:kilikili/utils/platform_utils.dart';
import 'package:kilikili/services/rust/rust_pgc.dart';
import 'package:material_ui/material_ui.dart';

class PgcRankCard extends StatelessWidget {
  const PgcRankCard({super.key, required this.item});

  final PgcRankItem item;

  void onLongPress() => showPgcCover(title: item.title, cover: item.cover);

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 4),
      child: CoverScrimCard(
        cover: item.cover,
        title: item.title ?? '',
        onTap: item.url == null
            ? null
            : () => PiliScheme.routePushFromUrl(item.url!),
        onLongPress: onLongPress,
        onSecondaryTap: PlatformUtils.isMobile ? null : onLongPress,
        topStart: CoverStatsPill(
          stats: [
            (StatType.play, item.view),
            (StatType.follow, item.follow),
          ],
        ),
        metaBuilder: item.newEpIndexShow?.isNotEmpty == true
            ? (context, colors) => [
                Flexible(
                  child: Text(
                    item.newEpIndexShow!,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: TextStyle(fontSize: 11, color: colors.subtitle),
                  ),
                ),
              ]
            : null,
      ),
    );
  }
}
