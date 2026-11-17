import 'package:kilikili/common/widgets/badge.dart';
import 'package:kilikili/common/widgets/custom_icon.dart';
import 'package:kilikili/common/widgets/image/image_save.dart';
import 'package:kilikili/common/widgets/video_card/cover_scrim_card.dart';
import 'package:kilikili/services/rust/rust_history.dart';
import 'package:kilikili/services/rust/rust_space.dart' as rust;
import 'package:kilikili/models/common/stat_type.dart';
import 'package:kilikili/utils/date_utils.dart';
import 'package:kilikili/utils/duration_utils.dart';
import 'package:kilikili/utils/id_utils.dart';
import 'package:kilikili/utils/page_utils.dart';
import 'package:kilikili/utils/platform_utils.dart';
import 'package:kilikili/utils/utils.dart';
import 'package:material_design_icons_flutter/material_design_icons_flutter.dart';
import 'package:material_ui/material_ui.dart';

class SearchArchiveGrpc extends StatelessWidget {
  const SearchArchiveGrpc({super.key, required this.item});

  final rust.SpaceSearchArchiveItem item;

  @override
  Widget build(BuildContext context) {
    final bvid = IdUtils.av2bv(item.aid?.toInt() ?? 0);
    // 高亮分段与实体解码都在 Rust 做好了（同搜索页），这里只拼回纯文本。
    final titleSpans = item.titleSpans;
    final title = titleSpans.map((e) => e.text).join();
    final date = DateFormatUtils.dateFormat(item.pubdate?.toInt());
    void onLongPress() =>
        imageSaveDialog(bvid: bvid, title: title, cover: item.cover);
    void open() {
      if (item.isPugv) {
        PageUtils.viewPgcFromUri(item.uri ?? '', isPgc: false);
      } else if (item.redirectUrl case final url?) {
        PageUtils.viewPgcFromUri(url);
      } else {
        PageUtils.toVideoPage(
          bvid: bvid,
          cid: item.firstCid?.toInt() ?? 0,
          cover: item.cover,
          title: title,
          isVertical: item.isVertical,
        );
      }
    }

    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 4),
      child: CoverScrimCard(
        cover: item.cover,
        title: title,
        upName: item.authorName,
        onTap: open,
        onLongPress: onLongPress,
        onSecondaryTap: PlatformUtils.isMobile ? null : onLongPress,
        titleBuilder: (context, colors) => Text.rich(
          TextSpan(
            style: TextStyle(color: colors.title, height: 1.32),
            children: [
              for (final part in titleSpans)
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
        topStats: [
          (StatType.play, item.view?.toInt()),
          (StatType.danmaku, item.danmaku?.toInt()),
        ],
        topDuration: (item.duration?.toInt() ?? 0) > 0
            ? DurationUtils.formatDuration(item.duration!.toInt())
            : null,
        coverOverlays: [
          if (item.isPugv) const PBadge(text: '课堂', top: 54, right: 6),
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
                onTap: () => RustHistory.toViewLater(bvid: bvid),
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
