import 'package:flutter_test/flutter_test.dart';
import 'package:kilikili/pages/mine/widgets/mine_surfaces.dart';
import 'package:kilikili/common/widgets/main_layout.dart';
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
                details: const [
                  MineInfoPill(
                    icon: Icons.toll_outlined,
                    label: '硬币',
                    value: '2114.7',
                  ),
                  MineInfoPill(
                    icon: Icons.stars_outlined,
                    label: '经验',
                    value: '49367/49367',
                  ),
                ],
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
  testWidgets('背景延伸到状态栏，内容只避让一次顶部安全区', (tester) async {
    const contentKey = Key('mine-content');
    const backgroundKey = Key('mine-background');
    await tester.pumpWidget(
      const MaterialApp(
        home: MediaQuery(
          data: MediaQueryData(
            padding: EdgeInsets.only(top: 24),
            viewPadding: EdgeInsets.only(top: 24),
          ),
          child: MainPageInset(
            extendBehindStatusBar: true,
            child: MinePageBackground(
              key: backgroundKey,
              child: SizedBox.expand(key: contentKey),
            ),
          ),
        ),
      ),
    );
    expect(tester.getTopLeft(find.byKey(backgroundKey)).dy, 0);
    expect(tester.getTopLeft(find.byKey(contentKey)).dy, 24);
    expect(tester.takeException(), isNull);
  });

  testWidgets('其他主页面保留原来的状态栏留白', (tester) async {
    const key = Key('other-page');
    await tester.pumpWidget(
      const MaterialApp(
        home: MediaQuery(
          data: MediaQueryData(
            padding: EdgeInsets.only(top: 24),
            viewPadding: EdgeInsets.only(top: 24),
          ),
          child: MainPageInset(child: SizedBox.expand(key: key)),
        ),
      ),
    );
    expect(tester.getTopLeft(find.byKey(key)).dy, 24);
    expect(tester.takeException(), isNull);
  });
  for (final scenario in [
    (width: 600.0, scale: 1.0, count: 5, inline: true),
    (width: 320.0, scale: 1.0, count: 5, inline: false),
    (width: 600.0, scale: 2.0, count: 5, inline: false),
    (width: 600.0, scale: 1.0, count: 7, inline: false),
  ]) {
    testWidgets('账户工具自适应：$scenario', (tester) async {
      tester.view.physicalSize = Size(scenario.width, 1400);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      var profileTaps = 0;
      var toolTaps = 0;
      await tester.pumpWidget(
        MaterialApp(
          builder: (context, child) => MediaQuery(
            data: MediaQuery.of(context)
                .copyWith(textScaler: TextScaler.linear(scenario.scale)),
            child: child!,
          ),
          home: Scaffold(
            body: SingleChildScrollView(
              padding: const EdgeInsets.all(20),
              child: MineAccountCard(
                avatar: const SizedBox.square(
                  dimension: 64,
                  child: Icon(Icons.person),
                ),
                name: '卡卡卡卡喵',
                level: const SizedBox(width: 26, height: 12),
                details: const [
                  MineInfoPill(
                    icon: Icons.toll_outlined,
                    label: '硬币',
                    value: '2114.7',
                  ),
                  MineInfoPill(
                    icon: Icons.stars_outlined,
                    label: '经验',
                    value: '49367/49367',
                  ),
                ],
                statistics: [
                  for (final label in ['动态', '关注', '粉丝'])
                    MineStatButton(count: '21', label: label, onTap: () {}),
                ],
                tools: List.generate(
                  scenario.count,
                  (i) => MineToolButton(
                    tooltip: '账户工具 $i',
                    icon: Icons.settings_outlined,
                    onPressed: () => toolTaps++,
                  ),
                ),
                onTap: () => profileTaps++,
              ),
            ),
          ),
        ),
      );
      expect(tester.takeException(), isNull);
      final capsule = tester.getRect(find.byType(MineToolCapsule));
      final name = tester.getRect(find.text('卡卡卡卡喵'));
      final card = tester.getRect(find.byType(MineAccountCard));
      expect(capsule.right, closeTo(card.right - 16, 0.1));
      if (scenario.inline) {
        expect(capsule.center.dy, closeTo(name.center.dy, 0.1));
      } else {
        expect(name.top, greaterThan(capsule.bottom));
      }
      for (var i = 0; i < scenario.count; i++) {
        final finder = find.byTooltip('账户工具 $i');
        expect(tester.getSize(finder).width, greaterThanOrEqualTo(44));
        await tester.tap(finder);
      }
      expect(toolTaps, scenario.count);
      expect(profileTaps, 0);
    });
  }
}
