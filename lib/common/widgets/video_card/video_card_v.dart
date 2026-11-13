import 'package:kilikili/common/widgets/badge.dart';
import 'package:kilikili/common/widgets/image/image_save.dart';
import 'package:kilikili/common/widgets/video_card/cover_pill.dart';
import 'package:kilikili/common/widgets/video_card/cover_scrim_card.dart';
import 'package:kilikili/common/widgets/video_popup_menu.dart';
import 'package:kilikili/http/search.dart';
import 'package:kilikili/models/common/stat_type.dart';
import 'package:kilikili/models/home/rcmd/result.dart';
import 'package:kilikili/models/model_rec_video_item.dart';
import 'package:kilikili/models_new/video/video_detail/dimension.dart';
import 'package:kilikili/utils/accounts.dart';
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
      // 匿名态下服务端照样会给 is_followed，不验登录会在未登录时也打上勾。
      upFollowed: videoItem.isFollowed && Accounts.main.isLogin,
      onTap: onPushDetail,
      onLongPress: onLongPress,
      onSecondaryTap: PlatformUtils.isMobile ? null : onLongPress,
      topStart: CoverStatsPill(
        stats: [
          (StatType.play, videoItem.stat.view),
          // 推荐流的 stat 里只有 view / like / danmaku，没有评论数，所以
          // 这一项是**弹幕数**。要换成评论数得先有接口给。
          if (videoItem.goto != 'picture')
            (StatType.danmaku, videoItem.stat.danmu),
        ],
      ),
      coverOverlays: [
        if (videoItem.duration > 0)
          CoverDurationPill(
            top: 6,
            right: 6,
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
      metaBuilder: _meta,
    );
  }

  /// UP 名之后那几项：角标 + 发布时间。
  ///
  /// 播放量与弹幕数已经挪到封面左上角的药丸里（issue #108），这一行只剩
  /// 「这条稿件是什么」而不是「它有多热」。
  List<Widget> _meta(BuildContext context, CoverScrimColors colors) => [
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
    if (videoItem is RcmdVideoItemModel) ...[
      const Spacer(),
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
  ];

  static final shortFormat = DateFormat('M-d');
  static final longFormat = DateFormat('yy-M-d');
}
