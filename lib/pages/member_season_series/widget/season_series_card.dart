import 'package:kilikili/common/widgets/badge.dart';
import 'package:kilikili/common/widgets/image/image_save.dart';
import 'package:kilikili/common/widgets/video_card/cover_scrim_card.dart';
import 'package:kilikili/services/rust/rust_space.dart';
import 'package:kilikili/utils/date_utils.dart';
import 'package:kilikili/utils/platform_utils.dart';
import 'package:material_ui/material_ui.dart';

class SeasonSeriesCard extends StatelessWidget {
  const SeasonSeriesCard({super.key, required this.item, required this.onTap});

  final SpaceSeasonSeries item;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final meta = item;
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
        date: DateFormatUtils.dateFormat(meta.ptime),
        onTap: onTap,
        onLongPress: onLongPress,
        onSecondaryTap: PlatformUtils.isMobile ? null : onLongPress,
      ),
    );
  }
}
