import 'dart:math' as math;

import 'package:flutter_test/flutter_test.dart';
import 'package:kilikili/common/widgets/expandable.dart';
import 'package:material_ui/material_ui.dart';

/// 读出三角指示当前实际绘制的旋转角（弧度，取 [-pi, pi]）。
///
/// 不读 `AnimatedRotation.turns`——那只是传进去的目标值，连点时错位恰恰发生
/// 在「目标对了但画出来的角度没跟上」这一步，所以要看渲染出来的矩阵。
double _renderedAngle(WidgetTester tester) {
  final transform = tester
      .widget<Transform>(
        find.descendant(
          of: find.byType(ExpandIndicator),
          matching: find.byType(Transform),
        ),
      )
      .transform;
  return math.atan2(transform.storage[1], transform.storage[0]);
}

Future<void> _pump(WidgetTester tester, bool expand) => tester.pumpWidget(
  MaterialApp(
    home: Scaffold(
      body: Center(child: ExpandIndicator(expand: expand)),
    ),
  ),
);

void main() {
  testWidgets('折叠态三角朝下，展开态转到朝上', (tester) async {
    await _pump(tester, false);
    expect(_renderedAngle(tester), moreOrLessEquals(0, epsilon: 1e-6));

    await _pump(tester, true);
    await tester.pumpAndSettle();
    expect(
      _renderedAngle(tester).abs(),
      moreOrLessEquals(math.pi, epsilon: 1e-6),
    );

    await _pump(tester, false);
    await tester.pumpAndSettle();
    expect(_renderedAngle(tester), moreOrLessEquals(0, epsilon: 1e-6));
  });

  testWidgets('动画时长与简介展开一致', (tester) async {
    await _pump(tester, false);
    await _pump(tester, true);

    // 时长内还在转；刚好到点就该停在终态。
    await tester.pump(kIntroExpandDuration ~/ 2);
    expect(_renderedAngle(tester).abs(), lessThan(math.pi - 1e-3));

    await tester.pump(kIntroExpandDuration ~/ 2);
    expect(
      _renderedAngle(tester).abs(),
      moreOrLessEquals(math.pi, epsilon: 1e-6),
    );
  });

  testWidgets('动画途中反向连点，最终角度跟随最后一次状态', (tester) async {
    await _pump(tester, false);
    await _pump(tester, true);
    await tester.pump(const Duration(milliseconds: 40));

    // 还没转完就点回去：不应停在中间角度，也不应卡在展开态。
    await _pump(tester, false);
    await tester.pumpAndSettle();
    expect(_renderedAngle(tester), moreOrLessEquals(0, epsilon: 1e-6));

    await _pump(tester, true);
    await tester.pump(const Duration(milliseconds: 40));
    await _pump(tester, false);
    await tester.pump(const Duration(milliseconds: 40));
    await _pump(tester, true);
    await tester.pumpAndSettle();
    expect(
      _renderedAngle(tester).abs(),
      moreOrLessEquals(math.pi, epsilon: 1e-6),
    );
  });
}
