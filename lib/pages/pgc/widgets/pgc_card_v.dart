import 'package:kilikili/common/widgets/badge.dart';
import 'package:kilikili/common/widgets/image/image_save.dart';
import 'package:kilikili/common/widgets/video_card/cover_scrim_card.dart';
import 'package:kilikili/models/common/badge_type.dart';
import 'package:kilikili/models_new/fav/fav_pgc/list.dart';
import 'package:kilikili/utils/page_utils.dart';
import 'package:kilikili/utils/platform_utils.dart';
import 'package:material_ui/material_ui.dart';

// 视频卡片 - 垂直布局
class PgcCardV extends StatelessWidget {
  const PgcCardV({
    super.key,
    required this.item,
  });

  final FavPgcItemModel item;

  void onLongPress() => showPgcCover(title: item.title, cover: item.cover);

  @override
  Widget build(BuildContext context) {
    final subtitle = item.progress ?? item.newEp?.indexShow;
    return CoverScrimCard(
      cover: item.cover,
      title: item.title!,
      titleMaxLines: 1,
      onTap: () => PageUtils.viewPgc(seasonId: item.seasonId),
      onLongPress: onLongPress,
      onSecondaryTap: PlatformUtils.isMobile ? null : onLongPress,
      coverOverlays: [
        PBadge(text: item.badge, top: 6, right: 6),
        if (item.isFinish == 0 && item.renewalTime?.isNotEmpty == true)
          PBadge(
            // 更新时间原本在左下角，那里现在是文案遮罩（issue #108）。
            text: item.renewalTime,
            top: 6,
            left: 6,
            type: PBadgeType.gray,
          ),
      ],
      metaBuilder: subtitle == null
          ? null
          : (context, colors) => [
              Flexible(
                child: Text(
                  subtitle,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: TextStyle(fontSize: 11, color: colors.subtitle),
                ),
              ),
            ],
    );
  }
}
