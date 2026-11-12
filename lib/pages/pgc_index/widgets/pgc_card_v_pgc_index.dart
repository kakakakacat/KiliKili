import 'package:kilikili/common/widgets/badge.dart';
import 'package:kilikili/common/widgets/image/image_save.dart';
import 'package:kilikili/common/widgets/video_card/cover_scrim_card.dart';
import 'package:kilikili/models/common/badge_type.dart';
import 'package:kilikili/services/rust/rust_pgc.dart';
import 'package:kilikili/utils/page_utils.dart';
import 'package:kilikili/utils/platform_utils.dart';
import 'package:material_ui/material_ui.dart';

// 视频卡片 - 垂直布局
class PgcCardVPgcIndex extends StatelessWidget {
  const PgcCardVPgcIndex({
    super.key,
    required this.item,
  });

  final PgcIndexItem item;

  void onLongPress() => showPgcCover(title: item.title, cover: item.cover);

  @override
  Widget build(BuildContext context) {
    return CoverScrimCard(
      cover: item.cover,
      title: item.title!,
      titleMaxLines: 1,
      onTap: () => PageUtils.viewPgc(seasonId: item.seasonId),
      onLongPress: onLongPress,
      onSecondaryTap: PlatformUtils.isMobile ? null : onLongPress,
      coverOverlays: [
        PBadge(text: item.badge, top: 6, right: 6),
        PBadge(
          // 排名原本在左下角，那里现在是文案遮罩（issue #108）。
          text: item.order,
          top: 6,
          left: 6,
          type: PBadgeType.gray,
        ),
      ],
      subtitleBuilder: item.indexShow == null
          ? null
          : (context, colors) => Text(
              item.indexShow!,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: TextStyle(fontSize: 11, color: colors.subtitle),
            ),
    );
  }
}
