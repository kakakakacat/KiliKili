import 'package:kilikili/common/widgets/image/image_save.dart';
import 'package:kilikili/common/widgets/video_card/cover_scrim_card.dart';
import 'package:kilikili/models_new/space/space_archive/item.dart';
import 'package:kilikili/utils/page_utils.dart';
import 'package:kilikili/utils/platform_utils.dart';
import 'package:material_ui/material_ui.dart';

// 视频卡片 - 垂直布局
class PgcCardVMemberPgc extends StatelessWidget {
  const PgcCardVMemberPgc(this.item, {super.key});

  final SpaceArchiveItem item;

  void onLongPress() => showPgcCover(title: item.title, cover: item.cover);

  @override
  Widget build(BuildContext context) {
    return CoverScrimCard(
      cover: item.cover,
      title: item.title,
      onTap: () => PageUtils.viewPgc(seasonId: item.param),
      onLongPress: onLongPress,
      onSecondaryTap: PlatformUtils.isMobile ? null : onLongPress,
    );
  }
}
