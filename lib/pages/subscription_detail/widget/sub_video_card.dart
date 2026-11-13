import 'package:kilikili/common/widgets/image/image_save.dart';
import 'package:kilikili/common/widgets/video_card/cover_scrim_card.dart';
import 'package:kilikili/http/search.dart';
import 'package:kilikili/models/common/stat_type.dart';
import 'package:kilikili/models_new/sub/sub_detail/media.dart';
import 'package:kilikili/utils/date_utils.dart';
import 'package:kilikili/utils/duration_utils.dart';
import 'package:kilikili/utils/page_utils.dart';
import 'package:kilikili/utils/platform_utils.dart';
import 'package:material_ui/material_ui.dart';

class SubVideoCardH extends StatelessWidget {
  const SubVideoCardH({super.key, required this.videoItem, this.searchType});

  final SubDetailItemModel videoItem;
  final int? searchType;

  Future<void> _open() async {
    final res = await SearchHttp.ab2cWithDimension(bvid: videoItem.bvid);
    final cid = res?.cid;
    if (cid != null) {
      PageUtils.toVideoPage(
        bvid: videoItem.bvid,
        cid: cid,
        cover: videoItem.cover,
        title: videoItem.title,
        dimension: res!.dimension,
      );
    }
  }

  @override
  Widget build(BuildContext context) {
    void onLongPress() => imageSaveDialog(
      title: videoItem.title,
      cover: videoItem.cover,
      bvid: videoItem.bvid,
    );
    final date = DateFormatUtils.dateFormat(videoItem.pubtime);
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 4),
      child: CoverScrimCard(
        cover: videoItem.cover,
        title: videoItem.title ?? '',
        onTap: _open,
        onLongPress: onLongPress,
        onSecondaryTap: PlatformUtils.isMobile ? null : onLongPress,
        topStats: [
          (StatType.play, videoItem.cntInfo?.play),
          (StatType.danmaku, videoItem.cntInfo?.danmaku),
        ],
        topDuration: (videoItem.duration ?? 0) > 0
            ? DurationUtils.formatDuration(videoItem.duration)
            : null,
        metaBuilder: date.isEmpty
            ? null
            : (context, colors) => [
                Text(
                  date,
                  maxLines: 1,
                  style: TextStyle(fontSize: 11, color: colors.subtitle),
                ),
              ],
      ),
    );
  }
}
