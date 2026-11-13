import 'package:kilikili/common/widgets/badge.dart';
import 'package:kilikili/common/widgets/image/image_save.dart';
import 'package:kilikili/common/widgets/video_card/cover_scrim_card.dart';
import 'package:kilikili/models_new/space/space_season_series/season.dart';
import 'package:kilikili/utils/date_utils.dart';
import 'package:kilikili/utils/platform_utils.dart';
import 'package:material_ui/material_ui.dart';

class SeasonSeriesCard extends StatelessWidget {
  const SeasonSeriesCard({super.key, required this.item, required this.onTap});

  final SpaceSsModel item;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final meta = item.meta!;
    void onLongPress() => imageSaveDialog(
      title: meta.name,
      cover: meta.cover,
    );
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 4),
      child: CoverScrimCard(
        cover: meta.cover,
        title: meta.name ?? '',
        coverOverlays: [
          PBadge(
            text: '${meta.seasonId != null ? '合集' : '列表'}: ${meta.total}',
            top: 6,
            right: 6,
          ),
        ],
        metaBuilder: (context, colors) => [
          Text(
            DateFormatUtils.dateFormat(meta.ptime),
            style: TextStyle(fontSize: 11, color: colors.subtitle),
          ),
        ],
        onTap: onTap,
        onLongPress: onLongPress,
        onSecondaryTap: PlatformUtils.isMobile ? null : onLongPress,
      ),
    );
  }
}
