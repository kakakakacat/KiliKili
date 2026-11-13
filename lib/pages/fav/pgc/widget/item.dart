import 'package:kilikili/common/widgets/badge.dart';
import 'package:kilikili/common/widgets/select_mask.dart';
import 'package:kilikili/common/widgets/video_card/cover_scrim_card.dart';
import 'package:kilikili/models_new/fav/fav_pgc/list.dart';
import 'package:kilikili/pages/common/multi_select/base.dart';
import 'package:kilikili/utils/page_utils.dart';
import 'package:kilikili/utils/platform_utils.dart';
import 'package:material_ui/material_ui.dart';

class FavPgcItem extends StatelessWidget {
  const FavPgcItem({
    super.key,
    required this.item,
    required this.ctr,
    required this.onSelect,
    required this.onUpdateStatus,
  });

  final FavPgcItemModel item;
  final MultiSelectBase ctr;
  final VoidCallback onSelect;
  final VoidCallback onUpdateStatus;

  void onLongPress() {
    if (!ctr.enableMultiSelect.value) {
      ctr.enableMultiSelect.value = true;
      onSelect();
    }
  }

  @override
  Widget build(BuildContext context) {
    final colorScheme = ColorScheme.of(context);
    final episode = item.newEp?.indexShow;
    final renewal = item.isFinish == 0 ? item.renewalTime : null;
    final details = [
      if (episode != null && episode.isNotEmpty) episode,
      if (renewal != null && renewal.isNotEmpty) renewal,
      if (item.progress?.isNotEmpty == true) item.progress!,
    ].join(' · ');
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 4),
      child: CoverScrimCard(
        cover: item.cover,
        title: item.title ?? '',
        onTap: () {
          if (ctr.enableMultiSelect.value) {
            onSelect();
          } else {
            PageUtils.viewPgc(seasonId: item.seasonId);
          }
        },
        onLongPress: onLongPress,
        onSecondaryTap: PlatformUtils.isMobile ? null : onLongPress,
        coverOverlays: [
          PBadge(text: item.badge, top: 6, right: 6),
        ],
        foregroundOverlays: [
          Positioned.fill(child: selectMask(colorScheme, item.checked)),
        ],
        metaBuilder: details.isEmpty
            ? null
            : (context, colors) => [
                Flexible(
                  child: Text(
                    details,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: TextStyle(fontSize: 11, color: colors.subtitle),
                  ),
                ),
              ],
        trailing: SizedBox.square(
          dimension: 29,
          child: IconButton(
            tooltip: '修改追番状态',
            onPressed: onUpdateStatus,
            icon: const Icon(Icons.more_vert, color: Colors.white, size: 18),
          ),
        ),
      ),
    );
  }
}
