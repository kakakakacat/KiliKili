import 'package:kilikili/common/widgets/badge.dart';
import 'package:kilikili/common/widgets/image/image_save.dart';
import 'package:kilikili/common/widgets/video_card/cover_scrim_card.dart';
import 'package:kilikili/utils/date_utils.dart';
import 'package:kilikili/utils/page_utils.dart';
import 'package:kilikili/utils/platform_utils.dart';
import 'package:material_ui/material_ui.dart';

class MemberCheeseItem extends StatelessWidget {
  const MemberCheeseItem({
    super.key,
    this.cover,
    this.title,
    this.marks,
    this.seasonId,
    this.status,
    this.ctime,
    this.onRemove,
  });

  final String? cover, title, status;
  final List<String>? marks;
  final int? seasonId, ctime;
  final VoidCallback? onRemove;

  @override
  Widget build(BuildContext context) {
    void onLongPress() => imageSaveDialog(title: title, cover: cover);
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 4),
      child: CoverScrimCard(
        cover: cover,
        title: title ?? '',
        onTap: () => PageUtils.viewPugv(seasonId: seasonId),
        onLongPress: onLongPress,
        onSecondaryTap: PlatformUtils.isMobile ? null : onLongPress,
        coverOverlays: [
          if (marks?.isNotEmpty == true)
            PBadge(text: marks!.join('|'), top: 6, right: 6),
        ],
        metaBuilder: status == null && ctime == null
            ? null
            : (context, colors) => [
                if (status?.isNotEmpty == true)
                  Flexible(
                    child: Text(
                      status!,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: TextStyle(fontSize: 11, color: colors.subtitle),
                    ),
                  ),
                if (ctime != null) ...[
                  const Spacer(),
                  Flexible(
                    child: Text(
                      '收藏于${DateFormatUtils.dateFormat(ctime)}',
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: TextStyle(fontSize: 11, color: colors.subtitle),
                    ),
                  ),
                ],
              ],
        trailing: onRemove == null
            ? null
            : SizedBox.square(
                dimension: 29,
                child: IconButton(
                  tooltip: '移除',
                  onPressed: onRemove,
                  icon: const Icon(Icons.clear, color: Colors.white, size: 18),
                ),
              ),
      ),
    );
  }
}
