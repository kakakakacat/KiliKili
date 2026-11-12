import 'package:kilikili/common/widgets/badge.dart';
import 'package:kilikili/common/widgets/image/image_save.dart';
import 'package:kilikili/common/widgets/stat/stat.dart';
import 'package:kilikili/common/widgets/video_card/cover_scrim_card.dart';
import 'package:kilikili/common/widgets/video_popup_menu.dart';
import 'package:kilikili/http/search.dart';
import 'package:kilikili/models/home/rcmd/result.dart';
import 'package:kilikili/models/model_rec_video_item.dart';
import 'package:kilikili/models_new/video/video_detail/dimension.dart';
import 'package:kilikili/utils/app_scheme.dart';
import 'package:kilikili/utils/date_utils.dart';
import 'package:kilikili/utils/duration_utils.dart';
import 'package:kilikili/utils/extension/dimension_ext.dart';
import 'package:kilikili/utils/id_utils.dart';
import 'package:kilikili/utils/page_utils.dart';
import 'package:kilikili/utils/platform_utils.dart';
import 'package:flutter_smart_dialog/flutter_smart_dialog.dart';
import 'package:intl/intl.dart';
import 'package:material_ui/material_ui.dart';

// 视频卡片 - 垂直布局
class VideoCardV extends StatelessWidget {
  final BaseRcmdVideoItemModel videoItem;
  final VoidCallback? onRemove;

  const VideoCardV({
    super.key,
    required this.videoItem,
    this.onRemove,
  });

  Future<void> onPushDetail() async {
    switch (videoItem.goto) {
      case 'bangumi':
        PageUtils.viewPgc(epId: videoItem.param!);
        break;
      case 'av':
        var bvid = videoItem.bvid ?? IdUtils.av2bv(videoItem.aid!);
        var cid = videoItem.cid;
        bool isVertical = false;
        Dimension? dimension;
        if (videoItem is RcmdVideoItemAppModel) {
          if (videoItem.uri case final uri?) {
            isVertical = uri.isVerticalFromUri;
          }
        }
        if (cid == null) {
          if (await SearchHttp.ab2cWithDimension(aid: videoItem.aid, bvid: bvid)
              case final res?) {
            cid = res.cid;
            dimension = res.dimension;
          }
        }
        if (cid != null) {
          PageUtils.toVideoPage(
            aid: videoItem.aid,
            bvid: bvid,
            cid: cid,
            cover: videoItem.cover,
            title: videoItem.title,
            isVertical: isVertical,
            dimension: dimension,
          );
        }
        break;
      // 动态
      case 'picture':
        try {
          PiliScheme.routePushFromUrl(videoItem.uri!);
        } catch (err) {
          SmartDialog.showToast(err.toString());
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
      bvid: videoItem.bvid,
    );
    return CoverScrimCard(
      cover: videoItem.cover,
      title: videoItem.title,
      upName: videoItem.owner.name?.toString(),
      onTap: onPushDetail,
      onLongPress: onLongPress,
      onSecondaryTap: PlatformUtils.isMobile ? null : onLongPress,
      coverOverlays: [
        if (videoItem.duration > 0)
          PBadge(
            // 时长角标原本在右下角，那里现在是文案遮罩；挪到右上角，和其余
            // 卡片保持一致（issue #108）。
            top: 6,
            right: 6,
            size: .small,
            type: .gray,
            text: DurationUtils.formatDuration(videoItem.duration),
          ),
      ],
      trailing: videoItem.goto == 'av'
          ? SizedBox.square(
              dimension: 29,
              child: VideoPopupMenu(
                iconSize: 17,
                onCover: true,
                videoItem: videoItem,
                onRemove: onRemove,
              ),
            )
          : null,
      subtitleBuilder: _subtitle,
    );
  }

  /// 标题下面那一行：角标 + 播放/弹幕 + 发布时间。
  ///
  /// 基线把它们铺成三行（标题、数据、UP 名）。叠加到封面上之后三行会吃掉
  /// 大半张封面，所以压成一行，UP 名另挪到封面左上角。
  Widget _subtitle(BuildContext context, CoverScrimColors colors) {
    final showDate = videoItem is RcmdVideoItemModel;
    return Padding(
      padding: const .only(top: 2),
      child: Row(
        spacing: 3,
        children: [
          if (videoItem.goto == 'bangumi')
            PBadge(
              text: videoItem.pgcBadge,
              isStack: false,
              size: .small,
              type: .line_primary,
              fontSize: 9,
            ),
          if (videoItem.rcmdReason != null)
            PBadge(
              text: videoItem.rcmdReason,
              isStack: false,
              size: .small,
              type: .secondary,
            ),
          if (videoItem.goto == 'picture')
            const PBadge(
              text: '动态',
              isStack: false,
              size: .small,
              type: .line_primary,
              fontSize: 9,
            ),
          if (videoItem.isFollowed)
            const PBadge(
              text: '已关注',
              isStack: false,
              size: .small,
              type: .secondary,
            ),
          const Spacer(),
          StatWidget(
            type: .play,
            value: videoItem.stat.view,
            color: colors.subtitle,
            iconSize: 12,
          ),
          if (videoItem.goto != 'picture')
            StatWidget(
              type: .danmaku,
              value: videoItem.stat.danmu,
              color: colors.subtitle,
              iconSize: 12,
            ),
          if (showDate)
            Text(
              DateFormatUtils.dateFormat(
                videoItem.pubdate,
                short: shortFormat,
                long: longFormat,
              ),
              maxLines: 1,
              style: TextStyle(fontSize: 11, color: colors.subtitle),
            ),
        ],
      ),
    );
  }

  static final shortFormat = DateFormat('M-d');
  static final longFormat = DateFormat('yy-M-d');
}
