import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:get/get.dart';
import 'package:hive_ce/hive.dart';
import 'package:kilikili/common/widgets/floating_navigation_bar.dart';
import 'package:kilikili/models/common/nav_bar_config.dart';
import 'package:kilikili/pages/main/controller.dart';
import 'package:kilikili/services/account_service.dart';
import 'package:kilikili/utils/feed_back.dart';
import 'package:kilikili/utils/storage.dart';
import 'package:kilikili/utils/storage_key.dart';
import 'package:material_ui/material_ui.dart';

class _AccountService extends AccountService {
  // 页面切换测试不初始化账号，也不发网络请求。
  @override
  // 只替代账号服务的初始化，避免依赖应用全局账号盒。
  // ignore: must_call_super
  void onInit() {}
}

Widget _navigation(String style, MainController main) => Obx(() {
  final index = main.selectedIndex.value;
  const destinations = NavigationBarType.values;
  if (style == '悬浮底栏') {
    return FloatingNavigationBar(
      selectedIndex: index,
      onDestinationSelected: main.setIndex,
      destinations: destinations
          .map(
            (nav) => FloatingNavigationDestination(
              label: nav.label,
              icon: nav.icon,
              selectedIcon: nav.selectIcon,
            ),
          )
          .toList(),
    );
  }
  return GlassNavigationSurface(
    selectedIndex: index,
    destinationCount: destinations.length,
    child: style == 'MD3 底栏'
        ? NavigationBar(
            selectedIndex: index,
            onDestinationSelected: main.setIndex,
            destinations: destinations
                .map(
                  (nav) => NavigationDestination(
                    label: nav.label,
                    icon: nav.icon,
                  ),
                )
                .toList(),
          )
        : BottomNavigationBar(
            currentIndex: index,
            onTap: main.setIndex,
            items: destinations
                .map(
                  (nav) => BottomNavigationBarItem(
                    label: nav.label,
                    icon: nav.icon,
                  ),
                )
                .toList(),
          ),
  );
});

void main() {
  late Directory directory;

  setUpAll(() async {
    directory = await Directory.systemTemp.createTemp('navigation-switch-');
    Hive.init(directory.path);
    GStorage.setting = await Hive.openBox('setting');
    enableFeedback = false;
  });

  setUp(() {
    Get.put<AccountService>(_AccountService());
  });

  tearDown(() async {
    Get.reset();
    await GStorage.setting.clear();
  });

  tearDownAll(() async {
    await Hive.close();
    await directory.delete(recursive: true);
  });

  for (final style in ['悬浮底栏', 'MD3 底栏', '经典底栏', '侧栏']) {
    final bottomNav = style != '侧栏';
    testWidgets('关闭动画时$style：跨页首帧直接显示目标，不构建中间页', (
      tester,
    ) async {
      await tester.runAsync(
        () => GStorage.setting.put(SettingBoxKey.mainTabBarView, false),
      );
      final pages = PageController();
      final main = MainController()
        ..navigationBars = NavigationBarType.values
        ..useBottomNav = bottomNav
        ..controller = pages;
      final built = <int>[];
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            bottomNavigationBar: bottomNav
                ? SizedBox(height: 80, child: _navigation(style, main))
                : null,
            body: PageView(
              controller: pages,
              physics: const NeverScrollableScrollPhysics(),
              children: List.generate(
                3,
                (index) => Builder(
                  builder: (_) {
                    built.add(index);
                    return Center(child: Text('page-$index'));
                  },
                ),
              ),
            ),
          ),
        ),
      );
      expect(built, [0]);

      if (bottomNav) {
        await tester.tap(find.text('我的'));
      } else {
        main.setIndex(2);
      }
      expect(main.selectedIndex.value, 2);
      await tester.pump();
      expect(pages.page, 2);
      expect(find.text('page-2').hitTestable(), findsOneWidget);
      expect(built, isNot(contains(1)));

      main
        ..setIndex(1)
        ..setIndex(0);
      await tester.pump();
      expect(main.selectedIndex.value, 0);
      expect(pages.page, 0);
      expect(find.text('page-0').hitTestable(), findsOneWidget);
      await tester.pumpWidget(const SizedBox.shrink());
      pages.dispose();
    });
  }

  testWidgets('开启动画时保留左右转场，连续点击最终显示最后目标', (tester) async {
    await tester.runAsync(
      () => GStorage.setting.put(SettingBoxKey.mainTabBarView, true),
    );
    final tabs = TabController(length: 3, vsync: tester);
    final main = MainController()
      ..navigationBars = NavigationBarType.values
      ..useBottomNav = true
      ..controller = tabs;
    await tester.pumpWidget(
      MaterialApp(
        home: TabBarView(
          controller: tabs,
          physics: const NeverScrollableScrollPhysics(),
          children: List.generate(
            3,
            (index) => Center(child: Text('page-$index')),
          ),
        ),
      ),
    );
    main.setIndex(2);
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 60));
    expect(tabs.animation!.value, greaterThan(0));
    expect(tabs.animation!.value, lessThan(2));
    main
      ..setIndex(1)
      ..setIndex(0);
    await tester.pumpAndSettle();
    expect(main.selectedIndex.value, 0);
    expect(tabs.index, 0);
    expect(find.text('page-0').hitTestable(), findsOneWidget);
    await tester.pumpWidget(const SizedBox.shrink());
    tabs.dispose();
  });
}
