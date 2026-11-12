import 'package:kilikili/common/widgets/image/image_save.dart';
import 'package:kilikili/common/widgets/video_card/cover_scrim_card.dart';
import 'package:kilikili/common/widgets/video_card/live_subtitle.dart';
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
      // 分区与围观数原本压在一条固定的 black54 渐变上（issue #108 要换掉的
      // 正是这种写死的遮罩），现在并进随封面取色的遮罩里。
      subtitleBuilder: (context, colors) => liveSubtitle(
        colors: colors,
        areaName: liveItem.areaName,
        watched: liveItem.textSmall == null ? null : '${liveItem.textSmall}围观',
      ),
    );
  }
}
