import 'package:flutter_test/flutter_test.dart';
import 'package:kilikili/common/widgets/short_tab_indicator.dart';
import 'package:material_ui/material_ui.dart';

void main() {
  test('横向短条居中，距底边 2 像素', () {
    const indicator = ShortTabIndicator(color: Colors.blue);
    final bounds = indicator
        .getClipPath(const Rect.fromLTWH(0, 0, 100, 40), TextDirection.ltr)
        .getBounds();

    expect(bounds, const Rect.fromLTWH(38, 35, 24, 3));
  });

  test('竖向短条居中，距左边 2 像素', () {
    final indicator = const ShortTabIndicator(
      color: Colors.blue,
    ).vertical();
    final bounds = indicator
        .getClipPath(const Rect.fromLTWH(0, 0, 40, 100), TextDirection.ltr)
        .getBounds();

    expect(bounds, const Rect.fromLTWH(2, 38, 3, 24));
  });

  test('主题色切换时插值颜色', () {
    const from = ShortTabIndicator(color: Colors.red);
    const to = ShortTabIndicator(color: Colors.blue);

    final middle = Decoration.lerp(from, to, 0.5) as ShortTabIndicator;
    expect(middle.color, Color.lerp(Colors.red, Colors.blue, 0.5));
  });
}
