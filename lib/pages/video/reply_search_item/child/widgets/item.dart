import 'package:kilikili/common/widgets/badge.dart';
import 'package:kilikili/common/widgets/image/image_save.dart';
import 'package:kilikili/common/widgets/video_card/cover_scrim_card.dart';
import 'package:kilikili/models/common/badge_type.dart';
import 'package:kilikili/models/common/reply/reply_search_type.dart';
import 'package:kilikili/services/rust/rust_reply_grpc.dart';
import 'package:kilikili/utils/duration_utils.dart';
import 'package:kilikili/utils/platform_utils.dart';
import 'package:get/get.dart';
import 'package:material_ui/material_ui.dart';

/// 名字带 `Card` 是为了与 Rust 导出的条目实体 [ReplySearchItem] 区分：
/// 那边是数据，这边是画它的卡片。
class ReplySearchItemCard extends StatelessWidget {
  const ReplySearchItemCard({
    super.key,
    required this.item,
    required this.type,
  });

  final ReplySearchItem item;
  final ReplySearchType type;

  @override
  Widget build(BuildContext context) {
    // 「UGC 取时长、PGC 取分区、专栏取第一张封面」这些分支在 Rust 投影时
    // 就分完了：没有的那一项是 null。
    final title = item.title;
    final cover = item.cover;
    final upNickname = item.upNickname;
    final category = item.category;
    final duration = item.duration;
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
