import 'package:kilikili/common/widgets/video_card/cover_scrim_card.dart';
import 'package:material_ui/material_ui.dart';

/// 直播卡片共用的那一行：分区 + 围观数。
///
/// 四个直播卡（首页、关注、直播搜索、综合搜索）的数据源各不相同，但这一行
/// 的排布一直是同一套，拆出来免得改一次要对着抄四遍。
///
/// 主播名**不在**这里：它和视频卡的 UP 名一样挪到了封面左上角，见
/// `CoverScrimCard.upName`。
///
/// 这些字段原本压在封面底部一条写死的 `black54` 渐变上——issue #108 要换掉
/// 的正是这种与封面无关的固定遮罩，现在并进随封面取色的遮罩里。
Widget liveSubtitle({
  required CoverScrimColors colors,
  String? areaName,
  String? watched,
}) {
  final style = TextStyle(fontSize: 11, color: colors.subtitle);
  return Padding(
    padding: const EdgeInsets.only(top: 2),
    child: Row(
      spacing: 6,
      children: [
        if (areaName != null)
          Flexible(
            child: Text(
              areaName,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: style,
            ),
          ),
        const Spacer(),
        if (watched != null) Text(watched, maxLines: 1, style: style),
      ],
    ),
  );
}
