import 'dart:async';

import 'package:material_ui/material_ui.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:kilikili/pages/login/sms_code_field.dart';

void main() {
  late TextEditingController controller;
  late FocusNode focusNode;

  setUp(() {
    controller = TextEditingController();
    focusNode = FocusNode();
  });
  tearDown(() {
    controller.dispose();
    focusNode.dispose();
  });

  Future<void> mount(WidgetTester tester, Future<void> Function() submit) =>
      tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: SmsCodeField(
              controller: controller,
              focusNode: focusNode,
              onSubmit: submit,
            ),
          ),
        ),
      );

  testWidgets('不足六位不提交，输入第六位自动提交', (tester) async {
    var submissions = 0;
    await mount(tester, () async {
      submissions++;
    });
    await tester.enterText(find.byType(TextField), '12345');
    expect(submissions, 0);
    await tester.enterText(find.byType(TextField), '123456');
    expect(submissions, 1);
  });

  testWidgets('粘贴仅保留六位数字并自动提交，支持系统验证码填充', (tester) async {
    String? submitted;
    await mount(tester, () async {
      submitted = controller.text;
    });
    await tester.enterText(find.byType(TextField), '12a34 5678');
    expect(controller.text, '123456');
    expect(submitted, '123456');
    expect(
      tester.widget<TextField>(find.byType(TextField)).autofillHints,
      contains(AutofillHints.oneTimeCode),
    );
  });

  testWidgets('提交过程中编辑不重复请求，失败返回后可修改重试', (tester) async {
    final pending = Completer<void>();
    var submissions = 0;
    await mount(tester, () {
      submissions++;
      return pending.future;
    });
    await tester.enterText(find.byType(TextField), '123456');
    await tester.enterText(find.byType(TextField), '654321');
    expect(submissions, 1);
    pending.complete();
    await tester.pump();
    await tester.enterText(find.byType(TextField), '654320');
    expect(submissions, 2);
  });

  testWidgets('外部焦点请求进入验证码输入框', (tester) async {
    await mount(tester, () async {});
    focusNode.requestFocus();
    await tester.pump();
    expect(focusNode.hasFocus, isTrue);
    expect(tester.testTextInput.isVisible, isTrue);
  });
}
