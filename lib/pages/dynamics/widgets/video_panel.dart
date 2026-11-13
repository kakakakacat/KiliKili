// 视频或合集封面与文案共用取色遮罩。
import 'package:kilikili/common/style.dart';
import 'package:kilikili/common/widgets/badge.dart';
import 'package:kilikili/common/widgets/video_card/cover_scrim_card.dart';
import 'package:kilikili/models/common/badge_type.dart';
import 'package:kilikili/models/dynamics/result.dart';
import 'package:kilikili/utils/num_utils.dart';
import 'package:material_ui/material_ui.dart';

Widget videoSeasonWidget(
  BuildContext context, {
  required int floor,
  required ThemeData theme,
  required DynamicItemModel item,
  required bool isSave,
  required bool isDetail,
}) {
  final DynamicArchiveModel? video = switch (item.type) {
    'DYNAMIC_TYPE_AV' => item.modules.moduleDynamic?.major?.archive,
    'DYNAMIC_TYPE_UGC_SEASON' => item.modules.moduleDynamic?.major?.ugcSeason,
    'DYNAMIC_TYPE_PGC' ||
    'DYNAMIC_TYPE_PGC_UNION' => item.modules.moduleDynamic?.major?.pgc,
    'DYNAMIC_TYPE_COURSES_SEASON' => item.modules.moduleDynamic?.major?.courses,
    _ => null,
  };
  if (video == null) return const SizedBox.shrink();

  return Padding(
    padding: floor == 1
        ? const EdgeInsets.symmetric(horizontal: 12)
        : EdgeInsets.zero,
    child: AspectRatio(
      aspectRatio: Style.coverCardRatio,
      child: CoverScrimCard(
        cover: video.cover,
        title: video.title ?? '',
        titleMaxLines: isDetail ? 3 : 2,
        coverOverlays: [
          if (video.badge?.text case final badge?)
            PBadge(
              text: badge,
              top: 8,
              right: 10,
              type: badge == '充电专属' ? PBadgeType.error : PBadgeType.primary,
            ),
          if (video.durationText case final duration?)
            PBadge(text: duration, top: 8, left: 10, type: PBadgeType.gray),
        ],
        metaBuilder: (context, colors) => [
          if (video.stat case final stat?)
            Flexible(
              child: Text(
                '${NumUtils.numFormat(stat.play)}播放 · ${NumUtils.numFormat(stat.danmu)}弹幕',
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: TextStyle(fontSize: 11, color: colors.subtitle),
              ),
            ),
        ],
      ),
    ),
  );
}
