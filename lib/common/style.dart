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

  /// 竖版封面大卡片的格子比例（issue #108）。
  ///
  /// 比封面源图的 16:9 **高**，有两个原因：文案叠在封面上要占掉底部一条，
  /// 格子再扁就只剩一道缝；而且整张卡就是封面，扁格子在多列下会挤成一片。
  /// 代价是 `BoxFit.cover` 把封面上下各裁掉约 12%——封面的主体通常在中间，
  /// 这个代价比把文案压成一行小。
  static const coverCardRatio = 4 / 3;

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
