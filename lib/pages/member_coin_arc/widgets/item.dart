import 'package:kilikili/common/widgets/badge.dart';
import 'package:kilikili/common/widgets/image/image_save.dart';
import 'package:kilikili/common/widgets/video_card/cover_pill.dart';
import 'package:kilikili/common/widgets/video_card/cover_scrim_card.dart';
import 'package:kilikili/common/widgets/video_card/video_card_v.dart';
import 'package:kilikili/http/search.dart';
import 'package:kilikili/models/common/stat_type.dart';
import 'package:kilikili/models_new/member/coin_like_arc/item.dart';
import 'package:kilikili/utils/date_utils.dart';
import 'package:kilikili/utils/duration_utils.dart';
import 'package:kilikili/utils/page_utils.dart';
import 'package:kilikili/utils/platform_utils.dart';
import 'package:material_ui/material_ui.dart';

class MemberCoinLikeItem extends StatelessWidget {
  final CoinLikeArcItem item;

  const MemberCoinLikeItem({
    super.key,
    required this.item,
  });

  Future<void> _onTap() async {
    if (item.isPgc == true) {
      if (item.uri?.isNotEmpty == true) {
        PageUtils.viewPgcFromUri(item.uri!);
      }
      return;
    }

    if (item.param != null) {
      final res = await SearchHttp.ab2cWithDimension(aid: item.param);
      final cid = res?.cid;
      if (cid != null) {
        PageUtils.toVideoPage(
          aid: int.parse(item.param!),
          cid: cid,
          cover: item.cover,
          title: item.title,
          dimension: res!.dimension,
        );
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    void onLongPress() => imageSaveDialog(
      title: item.title,
      cover: item.cover,
      aid: item.param,
    );
    return CoverScrimCard(
      cover: item.cover,
      title: '${item.title}',
      onTap: _onTap,
      onLongPress: onLongPress,
      onSecondaryTap: PlatformUtils.isMobile ? null : onLongPress,
      coverOverlays: [
        if (item.isCooperation == true)
          const PBadge(text: '合作', top: 6, left: 6)
        else if (item.isSteins == true)
          const PBadge(text: '互动', top: 6, left: 6),
        if (item.duration != null && item.duration! > 0)
          CoverDurationPill(
            // 时长原本在右下角，那里现在是文案遮罩（issue #108）。
            top: 6,
            right: 6,
            text: DurationUtils.formatDuration(item.duration),
          ),
      ],
      topStart: CoverStatsPill(
        stats: [
          (StatType.play, item.play),
          (StatType.danmaku, item.danmaku),
        ],
      ),
      metaBuilder: (context, colors) => [
        const Spacer(),
        Text(
          DateFormatUtils.dateFormat(
            item.ctime,
            short: VideoCardV.shortFormat,
            long: VideoCardV.longFormat,
          ),
          maxLines: 1,
          style: TextStyle(fontSize: 11, color: colors.subtitle),
        ),
      ],
    );
  }
}
