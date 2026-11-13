import 'package:kilikili/common/widgets/badge.dart';
import 'package:kilikili/common/widgets/image/image_save.dart';
import 'package:kilikili/common/widgets/video_card/cover_scrim_card.dart';
import 'package:kilikili/grpc/bilibili/main/community/reply/v1.pb.dart'
    show SearchItem, SearchItemVideoSubType;
import 'package:kilikili/models/common/badge_type.dart';
import 'package:kilikili/models/common/reply/reply_search_type.dart';
import 'package:kilikili/utils/duration_utils.dart';
import 'package:kilikili/utils/platform_utils.dart';
import 'package:get/get.dart';
import 'package:material_ui/material_ui.dart';

class ReplySearchItem extends StatelessWidget {
  const ReplySearchItem({super.key, required this.item, required this.type});

  final SearchItem item;
  final ReplySearchType type;

  @override
  Widget build(BuildContext context) {
    String title = '';
    String cover = '';
    String? upNickname;
    String? category;
    int? duration;
    switch (type) {
      case ReplySearchType.video:
        if (item.video.type == SearchItemVideoSubType.UGC) {
          final ugc = item.video.ugc;
          title = ugc.title;
          cover = ugc.cover;
          upNickname = ugc.upNickname;
          duration = ugc.duration.toInt();
        } else {
          final pgc = item.video.pgc;
          title = pgc.title;
          cover = pgc.cover;
          category = pgc.category;
        }
      case ReplySearchType.article:
        final article = item.article;
        title = article.title;
        cover = article.covers.firstOrNull ?? '';
        upNickname = article.upNickname;
    }
    void onLongPress() => imageSaveDialog(title: title, cover: cover);
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 4),
      child: CoverScrimCard(
        cover: cover,
        title: title,
        upName: upNickname,
        onTap: () => Get.back(result: (title: title, url: item.url)),
        onLongPress: onLongPress,
        onSecondaryTap: PlatformUtils.isMobile ? null : onLongPress,
        coverOverlays: [
          if (category != null) PBadge(text: category, top: 6, right: 6),
          if (duration != null && duration > 0)
            PBadge(
              text: DurationUtils.formatDuration(duration),
              top: category == null ? 6 : 32,
              right: 6,
              type: PBadgeType.gray,
            ),
        ],
      ),
    );
  }
}
