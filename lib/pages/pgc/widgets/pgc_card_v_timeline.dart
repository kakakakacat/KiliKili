import 'package:kilikili/common/widgets/badge.dart';
import 'package:kilikili/common/widgets/image/image_save.dart';
import 'package:kilikili/common/widgets/video_card/cover_scrim_card.dart';
import 'package:kilikili/models/common/badge_type.dart';
import 'package:kilikili/services/rust/rust_pgc.dart';
import 'package:kilikili/utils/page_utils.dart';
import 'package:kilikili/utils/platform_utils.dart';
import 'package:material_ui/material_ui.dart';

// 视频卡片 - 垂直布局
class PgcCardVTimeline extends StatelessWidget {
  const PgcCardVTimeline({
    super.key,
    required this.item,
  });

  final TimelineEpisode item;

  void onLongPress() => showPgcCover(title: item.title, cover: item.cover);

  @override
  Widget build(BuildContext context) {
    return CoverScrimCard(
      cover: item.cover,
      title: item.title ?? '',
      titleMaxLines: 1,
      onTap: () =>
          PageUtils.viewPgc(seasonId: item.seasonId, epId: item.episodeId),
      onLongPress: onLongPress,
      onSecondaryTap: PlatformUtils.isMobile ? null : onLongPress,
      coverOverlays: [
        if (item.follow == 1) const PBadge(text: '已追番', right: 6, top: 6),
        PBadge(
          // 更新时间原本在左下角，那里现在是文案遮罩（issue #108）。
          text: '${item.pubTime}',
          left: 6,
          top: 6,
          type: PBadgeType.gray,
        ),
      ],
      metaBuilder: (context, colors) => [
        Flexible(
          child: Text(
            item.pubIndex ?? '',
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: TextStyle(fontSize: 11, color: colors.subtitle),
          ),
        ),
      ],
    );
  }
}
