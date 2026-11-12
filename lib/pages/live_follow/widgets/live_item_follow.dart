import 'package:kilikili/common/widgets/image/image_save.dart';
import 'package:kilikili/common/widgets/video_card/cover_pill.dart';
import 'package:kilikili/common/widgets/video_card/cover_scrim_card.dart';
import 'package:kilikili/common/widgets/video_card/live_meta.dart';
import 'package:kilikili/services/rust/rust_live.dart';
import 'package:kilikili/utils/page_utils.dart';
import 'package:kilikili/utils/platform_utils.dart';
import 'package:material_ui/material_ui.dart';

// 视频卡片 - 垂直布局
class LiveCardVFollow extends StatelessWidget {
  final LiveFollowItem liveItem;

  const LiveCardVFollow({
    super.key,
    required this.liveItem,
  });

  @override
  Widget build(BuildContext context) {
    void onLongPress() => imageSaveDialog(
      title: liveItem.title,
      cover: liveItem.roomCover,
    );
    return CoverScrimCard(
      cover: liveItem.roomCover,
      title: liveItem.title.toString(),
      upName: liveItem.uname,
      onTap: () => PageUtils.toLiveRoom(liveItem.roomid),
      onLongPress: onLongPress,
      onSecondaryTap: PlatformUtils.isMobile ? null : onLongPress,
      topStart: CoverWatchedPill(
        watched: liveItem.textSmall == null ? null : '${liveItem.textSmall}围观',
      ),
      metaBuilder: (context, colors) =>
          liveMeta(colors: colors, areaName: liveItem.areaName),
    );
  }
}
