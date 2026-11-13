import 'package:kilikili/common/widgets/image/image_save.dart';
import 'package:kilikili/common/widgets/video_card/cover_pill.dart';
import 'package:kilikili/common/widgets/video_card/cover_scrim_card.dart';
import 'package:kilikili/common/widgets/video_card/live_meta.dart';
import 'package:kilikili/services/rust/rust_live.dart';
import 'package:kilikili/utils/page_utils.dart';
import 'package:kilikili/utils/platform_utils.dart';
import 'package:material_ui/material_ui.dart';

// 视频卡片 - 垂直布局
class LiveCardVSearch extends StatelessWidget {
  final LiveSearchRoom item;

  const LiveCardVSearch({
    super.key,
    required this.item,
  });

  @override
  Widget build(BuildContext context) {
    void onLongPress() => imageSaveDialog(
      title: item.title,
      cover: item.cover,
    );
    return CoverScrimCard(
      cover: item.cover,
      title: item.title.toString(),
      onTap: () => PageUtils.toLiveRoom(item.roomid),
      onLongPress: onLongPress,
      onSecondaryTap: PlatformUtils.isMobile ? null : onLongPress,
      topStart: CoverWatchedPill(watched: item.watchedText),
      metaBuilder: (context, colors) =>
          liveMeta(colors: colors, areaName: item.name),
    );
  }
}
