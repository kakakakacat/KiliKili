import 'package:flutter_test/flutter_test.dart';
import 'package:kilikili/pages/mine/widgets/mine_surfaces.dart';
import 'package:material_ui/material_ui.dart';

void main() {
  testWidgets('七个工具在窄屏换行，保持点击区域和回调', (tester) async {
    tester.view.physicalSize = const Size(320, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    var taps = 0;
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: Padding(
            padding: const EdgeInsets.all(20),
            child: MineToolCapsule(
              children: List.generate(
                7,
                (i) => MineToolButton(
                  tooltip: '工具 $i',
                  icon: Icons.settings_outlined,
                  onPressed: () => taps++,
                ),
              ),
            ),
          ),
        ),
      ),
    );
    expect(tester.takeException(), isNull);
    for (var i = 0; i < 7; i++) {
      final button = find.byTooltip('工具 $i');
      expect(tester.getSize(button).width, greaterThanOrEqualTo(44));
      expect(tester.getSize(button).height, greaterThanOrEqualTo(44));
      await tester.tap(button);
    }
    expect(taps, 7);
  });

  testWidgets('选中语义随真实状态更新，普通入口没有开关语义', (tester) async {
    final semantics = tester.ensureSemantics();
    var selected = false;
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: StatefulBuilder(
            builder: (context, setState) => MineToolCapsule(
              children: [
                MineToolButton(
                  tooltip: '无痕模式',
                  icon: Icons.visibility_off_outlined,
                  selected: selected,
                  onPressed: () => setState(() => selected = !selected),
                ),
                MineToolButton(
                  tooltip: '设置',
                  icon: Icons.settings_outlined,
                  onPressed: () {},
                ),
              ],
            ),
          ),
        ),
      ),
    );
    final toggle = find.byType(MineToolButton).first;
    expect(
      tester.getSemantics(toggle).flagsCollection.isToggled.toBoolOrNull() !=
          null,
      isTrue,
    );
    expect(
      tester.getSemantics(toggle).flagsCollection.isToggled.toBoolOrNull(),
      isFalse,
    );
    await tester.tap(find.byTooltip('无痕模式'));
    await tester.pumpAndSettle();
    expect(
      tester.getSemantics(toggle).flagsCollection.isToggled.toBoolOrNull(),
      isTrue,
    );
    expect(
      tester
              .getSemantics(find.byType(MineToolButton).last)
              .flagsCollection
              .isToggled
              .toBoolOrNull() !=
          null,
      isFalse,
    );
    semantics.dispose();
  });

  for (final width in [320.0, 600.0, 1000.0]) {
    testWidgets('快捷底座在 $width 宽度和两倍字号下保留四个可点击入口', (tester) async {
      tester.view.physicalSize = Size(width, 1000);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      final taps = <String>[];
      const labels = ['离线缓存', '观看记录', '我的订阅', '稍后再看'];
      await tester.pumpWidget(
        MaterialApp(
          builder: (context, child) => MediaQuery(
            data: MediaQuery.of(context)
                .copyWith(textScaler: const TextScaler.linear(2)),
            child: child!,
          ),
          home: Scaffold(
            body: Padding(
              padding: const EdgeInsets.all(20),
              child: MineActionDock(
                actions: [
                  for (final label in labels)
                    (
                      icon: Icons.history,
                      title: label,
                      onTap: () => taps.add(label),
                    ),
                ],
              ),
            ),
          ),
        ),
      );
      expect(tester.takeException(), isNull);
      for (final label in labels) {
        await tester.tap(find.text(label));
      }
      expect(taps, labels);
    });
  }
  testWidgets('窄屏两倍字号的长昵称、统计和收藏卡片不溢出', (tester) async {
    tester.view.physicalSize = const Size(320, 1400);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final taps = <String>[];
    await tester.pumpWidget(
      MaterialApp(
        builder: (context, child) => MediaQuery(
          data: MediaQuery.of(context)
              .copyWith(textScaler: const TextScaler.linear(2)),
          child: child!,
        ),
        home: Scaffold(
          body: ListView(
            padding: const EdgeInsets.all(20),
            children: [
              MineAccountCard(
                avatar: const SizedBox.square(
                  dimension: 64,
                  child: Icon(Icons.person),
                ),
                name: '这是很长很长的用户昵称用于验证截断',
                level: const Text('6', textScaler: TextScaler.noScaling),
                details: const [Text('硬币 2114.7'), Text('经验 49367/49367')],
                statistics: [
                  for (final label in ['动态', '关注', '粉丝'])
                    MineStatButton(
                      count: '123456789',
                      label: label,
                      onTap: () => taps.add(label),
                    ),
                ],
                onTap: () => taps.add('头像'),
                onLongPress: () => taps.add('长按'),
              ),
              const SizedBox(height: 20),
              SizedBox(
                height: 244,
                child: MineCollectionTile(
                  cover: const ColoredBox(color: Colors.grey),
                  title: '很长很长的默认收藏夹名称',
                  subtitle: '65 条视频 · 公开',
                  onTap: () => taps.add('收藏'),
                ),
              ),
            ],
          ),
        ),
      ),
    );
    expect(tester.takeException(), isNull);
    await tester.tap(find.text('这是很长很长的用户昵称用于验证截断'));
    await tester.longPress(find.text('这是很长很长的用户昵称用于验证截断'));
    for (final label in ['动态', '关注', '粉丝']) {
      await tester.tap(find.text(label));
    }
    await tester.tap(find.text('很长很长的默认收藏夹名称'));
    expect(taps, ['头像', '长按', '动态', '关注', '粉丝', '收藏']);
  });
}
