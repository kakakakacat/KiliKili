import 'package:material_ui/material_ui.dart'
    show BorderRadius, Radius, BoxConstraints, ButtonStyle, VisualDensity;

abstract final class Style {
  static const cardSpace = 8.0;
  static const safeSpace = 12.0;
  static const mdRadius = BorderRadius.all(imgRadius);
  static const imgRadius = Radius.circular(10);
  static const borderRadius6 = BorderRadius.all(.circular(6));
  static const aspectRatio = 16 / 10;
  static const aspectRatio16x9 = 16 / 9;
  static const aspectRatio3x4 = 3 / 4;

  /// 封面卡片里**封面**的宽高比。
  ///
  /// 标题和 UP 主名已经移到封面下方（#295），封面不再需要为叠加文案加高，
  /// 回到和其他列表一致的 16:10。格子高度 = 宽 / 本比例 +
  /// `CoverScrimCard.textExtent`，见 `Grid.coverCardDelegate`。
  static const coverCardRatio = aspectRatio;

  /// 竖版封面大卡片的圆角。比 [mdRadius] 大一圈，配合阴影。
  static const coverCardRadius = BorderRadius.all(.circular(14));

  static const imgMaxRatio = 2.6;
  static const bottomSheetRadius = BorderRadius.vertical(top: .circular(18));
  static const dialogFixedConstraints = BoxConstraints.tightFor(width: 420);
  static const topBarHeight = 52.0;
  static const buttonStyle = ButtonStyle(
    visualDensity: VisualDensity(horizontal: -2, vertical: -1.25),
    tapTargetSize: .shrinkWrap,
  );
  static const placeHolder = '\uFFFC';
}
