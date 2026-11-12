import 'package:kilikili/common/widgets/video_card/cover_scrim_card.dart';
import 'package:material_ui/material_ui.dart';

/// 直播卡片信息行里**主播名之后**的那几项：分区名。
///
/// 四个直播卡（首页、关注、直播搜索、综合搜索）的数据源各不相同，但这一行
/// 的排布一直是同一套，拆出来免得改一次要对着抄四遍。
///
/// 围观数**不在**这里：它和视频卡的播放量一样属于「有多热」，挪到了封面
/// 左上角的 [CoverStatsPill]（issue #108）。主播名也不在，它由
/// `CoverScrimCard.upName` 统一排在这一行最前面。
List<Widget> liveMeta({
  required CoverScrimColors colors,
  String? areaName,
}) => [
  if (areaName != null && areaName.isNotEmpty) ...[
    const Spacer(),
    Flexible(
      child: Text(
        areaName,
        maxLines: 1,
        overflow: TextOverflow.ellipsis,
        style: TextStyle(fontSize: 11, color: colors.subtitle),
      ),
    ),
  ],
];
