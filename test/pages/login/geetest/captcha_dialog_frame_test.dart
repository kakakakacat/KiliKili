import 'package:material_ui/material_ui.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:kilikili/pages/login/geetest/captcha_dialog_frame.dart';

void main() {
  for (final size in [
    const Size(240, 480),
    const Size(390, 844),
    const Size(1280, 800),
    const Size(640, 320),
  ]) {
    testWidgets('验证码区域在 $size 下保持居中且不超出屏幕', (tester) async {
      tester.view.physicalSize = size;
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      await tester.pumpWidget(
        const MaterialApp(
          home: CaptchaDialogFrame(
            child: ColoredBox(key: Key('captcha'), color: Colors.white),
          ),
        ),
      );
      final rect = tester.getRect(find.byKey(const Key('captcha')));
      expect(rect.width, lessThanOrEqualTo(360));
      expect(rect.height, lessThanOrEqualTo(440));
      expect(rect.left, greaterThanOrEqualTo(16));
      expect(rect.right, lessThanOrEqualTo(size.width - 16));
      expect(rect.top, greaterThanOrEqualTo(16));
      expect(rect.bottom, lessThanOrEqualTo(size.height - 16));
      expect(rect.center.dx, closeTo(size.width / 2, 1));
      expect(rect.center.dy, closeTo(size.height / 2, 1));
      expect(tester.takeException(), isNull);
    });
  }

  testWidgets('键盘弹出后验证码区域收缩并避让', (tester) async {
    tester.view.physicalSize = const Size(390, 844);
    tester.view.devicePixelRatio = 1;
    tester.view.viewInsets = const FakeViewPadding(bottom: 300);
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    addTearDown(tester.view.resetViewInsets);
    await tester.pumpWidget(
      const MaterialApp(
        home: CaptchaDialogFrame(
          child: ColoredBox(key: Key('captcha'), color: Colors.white),
        ),
      ),
    );
    final rect = tester.getRect(find.byKey(const Key('captcha')));
    expect(rect.bottom, lessThanOrEqualTo(844 - 300 - 16));
    expect(tester.takeException(), isNull);
  });
}
