import 'package:kilikili/common/widgets/badge.dart';
import 'package:kilikili/common/widgets/custom_icon.dart';
import 'package:kilikili/common/widgets/image/image_save.dart';
import 'package:kilikili/common/widgets/video_card/cover_pill.dart';
import 'package:kilikili/common/widgets/video_card/cover_scrim_card.dart';
import 'package:kilikili/grpc/bilibili/app/interfaces/v1.pb.dart' show Arc;
import 'package:kilikili/http/user.dart';
import 'package:kilikili/models/common/badge_type.dart';
import 'package:kilikili/models/common/stat_type.dart';
import 'package:kilikili/utils/date_utils.dart';
import 'package:kilikili/utils/duration_utils.dart';
import 'package:kilikili/utils/em.dart';
import 'package:kilikili/utils/extension/dimension_ext.dart';
import 'package:kilikili/utils/id_utils.dart';
import 'package:kilikili/utils/page_utils.dart';
import 'package:kilikili/utils/platform_utils.dart';
import 'package:kilikili/utils/utils.dart';
import 'package:fixnum/fixnum.dart' show Int64;
import 'package:material_design_icons_flutter/material_design_icons_flutter.dart';
import 'package:material_ui/material_ui.dart';

class SearchArchiveGrpc extends StatelessWidget {
  const SearchArchiveGrpc({super.key, required this.item});

  final Arc item;

  @override
  Widget build(BuildContext context) {
    final arc = item.archive;
    final bvid = IdUtils.av2bv(arc.aid.toInt());
    final regTitle = Em.regTitle(arc.title);
    final title = regTitle.map((e) => e.text).join();
    final date = DateFormatUtils.dateFormat(arc.pubdate.toInt());
    void onLongPress() =>
        imageSaveDialog(bvid: bvid, title: title, cover: arc.pic);
    void open() {
      if (item.isPugv) {
        PageUtils.viewPgcFromUri(item.uri, isPgc: false);
      } else if (arc.hasRedirectUrl()) {
        PageUtils.viewPgcFromUri(arc.redirectUrl);
      } else {
        PageUtils.toVideoPage(
          bvid: bvid,
          cid: arc.firstCid.toInt(),
          cover: arc.pic,
          title: title,
          isVertical: arc.dimension.isVertical,
        );
      }
    }

    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 4),
      child: CoverScrimCard(
        cover: arc.pic,
        title: title,
        upName: arc.author.name,
        onTap: open,
        onLongPress: onLongPress,
        onSecondaryTap: PlatformUtils.isMobile ? null : onLongPress,
        titleBuilder: (context, colors) => Text.rich(
          TextSpan(
            style: TextStyle(color: colors.title, height: 1.32),
            children: [
              for (final part in regTitle)
                TextSpan(
                  text: part.text,
                  style: part.isEm
                      ? const TextStyle(
                          fontWeight: FontWeight.bold,
                          decoration: TextDecoration.underline,
                        )
                      : null,
                ),
            ],
          ),
          maxLines: 2,
          overflow: TextOverflow.ellipsis,
        ),
        topStart: CoverStatsPill(
          stats: [
            (StatType.play, arc.stat.view),
            (StatType.danmaku, arc.stat.danmaku),
          ],
        ),
        coverOverlays: [
          if (item.isPugv) const PBadge(text: '课堂', top: 6, right: 6),
          if (arc.duration > Int64.ZERO)
            PBadge(
              text: DurationUtils.formatDuration(arc.duration.toInt()),
              top: item.isPugv ? 32 : 6,
              right: 6,
              type: PBadgeType.gray,
            ),
        ],
        metaBuilder: date.isEmpty
            ? null
            : (context, colors) => [
                const Spacer(),
                Text(
                  date,
                  maxLines: 1,
                  style: TextStyle(fontSize: 11, color: colors.subtitle),
                ),
              ],
        trailing: SizedBox.square(
          dimension: 29,
          child: PopupMenuButton(
            padding: EdgeInsets.zero,
            icon: const Icon(
              Icons.more_vert_outlined,
              color: Colors.white,
              size: 17,
            ),
            position: PopupMenuPosition.under,
            itemBuilder: (context) => [
              PopupMenuItem(
                height: 45,
                onTap: () => Utils.copyText(bvid),
                child: Row(
                  spacing: 6,
                  children: [
                    const Icon(CustomIcons.identifier_circle, size: 16),
                    Text(bvid, style: const TextStyle(fontSize: 13)),
                  ],
                ),
              ),
              PopupMenuItem(
                height: 45,
                onTap: () => UserHttp.toViewLater(bvid: bvid),
                child: const Row(
                  spacing: 6,
                  children: [
                    Icon(MdiIcons.clockTimeEightOutline, size: 16),
                    Text('稍后再看', style: TextStyle(fontSize: 13)),
                  ],
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
