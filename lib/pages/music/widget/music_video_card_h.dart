import 'package:kilikili/common/widgets/image/image_save.dart';
import 'package:kilikili/common/widgets/video_card/cover_pill.dart';
import 'package:kilikili/common/widgets/video_card/cover_scrim_card.dart';
import 'package:kilikili/http/search.dart';
import 'package:kilikili/models/common/stat_type.dart';
import 'package:kilikili/services/rust/rust_misc.dart';
import 'package:kilikili/models_new/video/video_detail/dimension.dart';
import 'package:kilikili/utils/duration_utils.dart';
import 'package:kilikili/utils/page_utils.dart';
import 'package:kilikili/utils/platform_utils.dart';
import 'package:material_ui/material_ui.dart';

class MusicVideoCardH extends StatelessWidget {
  const MusicVideoCardH({super.key, required this.videoItem});

  final BgmRecommend videoItem;

  Future<void> _open() async {
    int? cid = videoItem.cid;
    Dimension? dimension;
    if (cid == null) {
      if (await SearchHttp.ab2cWithDimension(bvid: videoItem.bvid)
          case final res?) {
        cid = res.cid;
        dimension = res.dimension;
      }
    }
    if (cid != null) {
      PageUtils.toVideoPage(
        bvid: videoItem.bvid,
        cid: cid,
        cover: videoItem.cover,
        title: videoItem.title,
        dimension: dimension,
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
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 4),
      child: CoverScrimCard(
        cover: videoItem.cover,
        title: videoItem.title ?? '',
        upName: videoItem.upNickName,
        onTap: _open,
        onLongPress: onLongPress,
        onSecondaryTap: PlatformUtils.isMobile ? null : onLongPress,
        topStart: CoverStatsPill(
          stats: [
            (StatType.play, videoItem.play),
            (StatType.danmaku, videoItem.danmu),
          ],
        ),
        coverOverlays: [
          if ((videoItem.duration ?? 0) > 0)
            CoverDurationPill(
              text: DurationUtils.formatDuration(videoItem.duration),
              top: 6,
              right: 6,
            ),
        ],
        metaBuilder: videoItem.labels.isEmpty
            ? null
            : (context, colors) => [
                Flexible(
                  child: Text(
                    videoItem.labels.join(' · '),
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: TextStyle(fontSize: 11, color: colors.subtitle),
                  ),
                ),
              ],
      ),
    );
  }
}
