import 'package:flutter_test/flutter_test.dart';
import 'package:kilikili/common/widgets/floating_navigation_bar.dart';
import 'package:material_ui/material_ui.dart';

import 'bridge_env.dart';

/// 底栏液态水滴（#266）跨 FFI 的组件测试。
///
/// 形变规则本身由 `bili_core::glass_droplet` 的单测钉住；这里验的是宿主
/// 那一半：`Ticker` 真的在推、「格」到像素的换算对、静止后回到原来的胶
/// 囊、连点停在最后一格、「减弱动态效果」下不变形。没有原生库时组件只
/// 会走平移退路，所以这组测试必须加载真正的动态库。
void main() {
  setUpAll(() async {
    if (!bridgeTestsRequested) return;
    await initRustBridge();
  });

  tearDownAll(() async {
    if (!bridgeTestsRequested) return;
    await closeRustBridge();
  });

  const barWidth = 3 * 86.0;
  final indicator = find.byKey(const Key('glass-selected-indicator'));

  Widget bar(
    int selected, {
    bool disableAnimations = false,
    ValueChanged<int>? onSelected,
  }) => MaterialApp(
    home: MediaQuery(
      data: MediaQueryData(disableAnimations: disableAnimations),
      child: Scaffold(
        body: Center(
          child: SizedBox(
            width: barWidth,
            height: mainNavigationBarHeight,
            child: GlassNavigationSurface(
              selectedIndex: selected,
              destinationCount: 3,
              onDestinationSelected: onSelected,
              child: const SizedBox.expand(),
            ),
          ),
        ),
      ),
    ),
  );

  testWidgets('切换时拉长、变扁，静止后回到原胶囊', skip: !bridgeTestsRequested, (tester) async {
    await tester.pumpWidget(bar(0));
    final rest0 = tester.getRect(indicator);

    await tester.pumpWidget(bar(1));
    var maxWidth = 0.0;
    var minHeight = double.infinity;
    var maxRight = 0.0;
    for (var i = 0; i < 30; i++) {
      await tester.pump(const Duration(milliseconds: 16));
      final r = tester.getRect(indicator);
      maxWidth = r.width > maxWidth ? r.width : maxWidth;
      minHeight = r.height < minHeight ? r.height : minHeight;
      maxRight = r.right > maxRight ? r.right : maxRight;
    }
    expect(maxWidth, greaterThan(rest0.width * 1.1));
    expect(minHeight, lessThan(rest0.height * 0.95));

    await tester.pumpAndSettle();
    final rest1 = tester.getRect(indicator);
    expect(rest1.width, closeTo(rest0.width, 0.01));
    expect(rest1.height, closeTo(rest0.height, 0.01));
    expect(rest1.left, closeTo(rest0.left + rest0.width, 0.01));
    // 前缘冲过了目标再回来。
    expect(maxRight, greaterThan(rest1.right + 1));
  });

  testWidgets('快速连点停在最后一格', skip: !bridgeTestsRequested, (tester) async {
    await tester.pumpWidget(bar(0));
    final rest0 = tester.getRect(indicator);
    for (final i in [2, 0, 1, 2, 1]) {
      await tester.pumpWidget(bar(i));
      await tester.pump(const Duration(milliseconds: 40));
    }
    await tester.pumpAndSettle();
    final r = tester.getRect(indicator);
    expect(r.left, closeTo(rest0.left + rest0.width, 0.01));
    expect(r.width, closeTo(rest0.width, 0.01));
  });

  testWidgets('减弱动态效果时只平移、不变形', skip: !bridgeTestsRequested, (tester) async {
    await tester.pumpWidget(bar(0, disableAnimations: true));
    final rest0 = tester.getRect(indicator);
    await tester.pumpWidget(bar(2, disableAnimations: true));
    for (var i = 0; i < 20; i++) {
      await tester.pump(const Duration(milliseconds: 16));
      final r = tester.getRect(indicator);
      expect(r.width, closeTo(rest0.width, 0.01));
      expect(r.height, closeTo(rest0.height, 0.01));
    }
    await tester.pumpAndSettle();
    expect(
      tester.getRect(indicator).left,
      closeTo(rest0.left + 2 * rest0.width, 0.01),
    );
  });

  /// 第 [slot] 格的中心，全局坐标。
  Offset slotCenter(WidgetTester tester, double slot) {
    final surface = tester.getRect(find.byType(GlassNavigationSurface));
    final slotWidth = (surface.width - 8) / 3;
    return Offset(surface.left + 4 + slotWidth * slot, surface.center.dy);
  }

  testWidgets('按下即跑到按下那格并鼓起，松手未切换则回到选中项', skip: !bridgeTestsRequested, (
    tester,
  ) async {
    await tester.pumpWidget(bar(0));
    final rest0 = tester.getRect(indicator);
    final gesture = await tester.startGesture(slotCenter(tester, 2.5));
    await tester.pumpAndSettle();
    final held = tester.getRect(indicator);
    // 鼓起 12%，中心落在第 2 格中心。
    expect(held.height, greaterThan(rest0.height * 1.05));
    expect(
      held.center.dx,
      closeTo(rest0.center.dx + 2 * rest0.width, 0.5),
    );
    await gesture.up();
    await tester.pumpAndSettle();
    // 测试里没有内层导航去改选中，水滴回到第 0 格、恢复原大小。
    final back = tester.getRect(indicator);
    expect(back.left, closeTo(rest0.left, 0.01));
    expect(back.height, closeTo(rest0.height, 0.01));
  });

  testWidgets('拖动时水滴跟手，松手吸附到最近一格并选中', skip: !bridgeTestsRequested, (
    tester,
  ) async {
    var selected = 0;
    final picks = <int>[];
    await tester.pumpWidget(
      StatefulBuilder(
        builder: (context, setState) => bar(
          selected,
          onSelected: (i) {
            picks.add(i);
            setState(() => selected = i);
          },
        ),
      ),
    );
    final rest0 = tester.getRect(indicator);
    final gesture = await tester.startGesture(slotCenter(tester, 0.5));
    await tester.pump();
    // 拖到第 2 格靠左的位置：跟手时中心对着手指，松手吸附到手指所在的第 2 格。
    await gesture.moveTo(slotCenter(tester, 2.2));
    await tester.pumpAndSettle();
    final dragging = tester.getRect(indicator);
    expect(dragging.center.dx, closeTo(slotCenter(tester, 2.2).dx, 1.0));
    expect(picks, isEmpty, reason: '拖动中不切换');

    await gesture.up();
    await tester.pumpAndSettle();
    expect(picks, [2]);
    final settled = tester.getRect(indicator);
    expect(settled.left, closeTo(rest0.left + 2 * rest0.width, 0.01));
    expect(settled.width, closeTo(rest0.width, 0.01));
  });

  testWidgets('拖回原来那格松手不触发选中', skip: !bridgeTestsRequested, (tester) async {
    final picks = <int>[];
    await tester.pumpWidget(bar(1, onSelected: picks.add));
    final gesture = await tester.startGesture(slotCenter(tester, 1.5));
    await gesture.moveTo(slotCenter(tester, 2.5));
    await tester.pump(const Duration(milliseconds: 100));
    await gesture.moveTo(slotCenter(tester, 1.4));
    await tester.pumpAndSettle();
    await gesture.up();
    await tester.pumpAndSettle();
    expect(picks, isEmpty);
  });

  testWidgets('减弱动态效果时拖动选择照常可用', skip: !bridgeTestsRequested, (tester) async {
    final picks = <int>[];
    await tester.pumpWidget(
      bar(0, disableAnimations: true, onSelected: picks.add),
    );
    final rest0 = tester.getRect(indicator);
    final gesture = await tester.startGesture(slotCenter(tester, 0.5));
    await gesture.moveTo(slotCenter(tester, 2.4));
    await tester.pumpAndSettle();
    // 按格跳，不变形。
    final r = tester.getRect(indicator);
    expect(r.width, closeTo(rest0.width, 0.01));
    expect(r.left, closeTo(rest0.left + 2 * rest0.width, 0.01));
    await gesture.up();
    await tester.pumpAndSettle();
    expect(picks, [2]);
  });
}
