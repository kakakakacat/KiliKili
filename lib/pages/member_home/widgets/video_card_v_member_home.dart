import 'package:kilikili/common/widgets/badge.dart';
import 'package:kilikili/common/widgets/image/image_save.dart';
import 'package:kilikili/common/widgets/video_card/cover_scrim_card.dart';
import 'package:kilikili/http/search.dart';
import 'package:kilikili/models/common/badge_type.dart';
import 'package:kilikili/models_new/space/space_archive/item.dart';
import 'package:kilikili/models_new/video/video_detail/dimension.dart';
import 'package:kilikili/utils/app_scheme.dart';
import 'package:kilikili/utils/duration_utils.dart';
import 'package:kilikili/utils/id_utils.dart';
import 'package:kilikili/utils/page_utils.dart';
import 'package:kilikili/utils/platform_utils.dart';
import 'package:material_ui/material_ui.dart';

// 视频卡片 - 垂直布局
class VideoCardVMemberHome extends StatelessWidget {
  final SpaceArchiveItem videoItem;

  const VideoCardVMemberHome(this.videoItem, {super.key});

  Future<void> onPushDetail() async {
    String? goto = videoItem.goto;
    switch (goto) {
      case 'bangumi':
        PageUtils.viewPgc(epId: .parse(videoItem.param!));
        break;

      case 'av':
        if (videoItem.isPgc == true) {
          if (videoItem.uri?.isNotEmpty == true) {
            PageUtils.viewPgcFromUri(videoItem.uri!);
          }
          return;
        }

        String? aid = videoItem.param;
        String? bvid = videoItem.bvid;
        if (aid == null && bvid == null) {
          return;
        }

        bvid ??= IdUtils.av2bv(int.parse(aid!));
        int? cid = videoItem.cid;
        Dimension? dimension;
        if (cid == null) {
          if (await SearchHttp.ab2cWithDimension(aid: aid, bvid: bvid)
              case final res?) {
            cid = res.cid;
            dimension = res.dimension;
          }
        }
        if (cid != null) {
          PageUtils.toVideoPage(
            bvid: bvid,
            cid: cid,
            cover: videoItem.cover,
            title: videoItem.title,
            dimension: dimension,
          );
        }
        break;

      default:
        if (videoItem.uri?.isNotEmpty == true) {
          PiliScheme.routePushFromUrl(videoItem.uri!);
        }
    }
  }

  @override
  Widget build(BuildContext context) {
    void onLongPress() => imageSaveDialog(
      title: videoItem.title,
      cover: videoItem.cover,
      aid: videoItem.param,
      bvid: videoItem.bvid,
    );
    return CoverScrimCard(
      cover: videoItem.cover,
      title: videoItem.title,
      onTap: onPushDetail,
      onLongPress: onLongPress,
      onSecondaryTap: PlatformUtils.isMobile ? null : onLongPress,
      coverOverlays: [
        if (videoItem.duration > 0)
          PBadge(
            // 时长原本在右下角，那里现在是文案遮罩（issue #108）。
            top: 6,
            right: 6,
            size: PBadgeSize.small,
            type: PBadgeType.gray,
            text: DurationUtils.formatDuration(videoItem.duration),
          ),
        if (videoItem.badges?.isNotEmpty == true)
          PBadge(
            text: videoItem.badges!.map((e) => e.text ?? '').join('|'),
            top: 6,
            left: 6,
            type: videoItem.badges!.first.text == '充电专属'
                ? PBadgeType.error
                : PBadgeType.primary,
          )
        else if (videoItem.isCooperation == true)
          const PBadge(text: '合作', top: 6, left: 6)
        else if (videoItem.isSteins == true)
          const PBadge(text: '互动', top: 6, left: 6),
      ],
    );
  }
}
