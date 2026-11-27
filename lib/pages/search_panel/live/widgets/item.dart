import 'package:kilikili/common/widgets/video_card/cover_pill.dart';
import 'package:kilikili/common/widgets/video_card/cover_scrim_card.dart';
import 'package:kilikili/common/widgets/video_card/live_meta.dart';
import 'package:kilikili/common/widgets/image/image_save.dart';
import 'package:kilikili/services/rust/rust_search.dart';
import 'package:kilikili/utils/num_utils.dart';
import 'package:kilikili/utils/page_utils.dart';
import 'package:kilikili/utils/platform_utils.dart';
import 'package:material_ui/material_ui.dart';

class LiveItem extends StatelessWidget {
  final SearchLiveItem liveItem;

  const LiveItem({super.key, required this.liveItem});

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    void onLongPress() => imageSaveDialog(
      title: liveItem.title,
      cover: liveItem.cover,
    );
    return CoverScrimCard(
      cover: liveItem.cover,
      title: liveItem.titleSpans.map((e) => e.text).join(),
      upName: liveItem.uname,
      onTap: () => PageUtils.toLiveRoom(liveItem.roomid),
      onLongPress: onLongPress,
      onSecondaryTap: PlatformUtils.isMobile ? null : onLongPress,
      // 命中词用主题色标出来。标题已经在封面下方的卡片底色上（#295），
      // 不再随封面遮罩变色，直接用 primary 即可。
      titleSpans: [
        for (final e in liveItem.titleSpans)
          TextSpan(
            text: e.text,
            style: e.isEm ? TextStyle(color: theme.colorScheme.primary) : null,
          ),
      ],
      topStart: CoverWatchedPill(
        watched: '${NumUtils.numFormat(liveItem.online)}围观',
      ),
      metaBuilder: (context, colors) =>
          liveMeta(colors: colors, areaName: liveItem.cateName),
    );
  }
}
