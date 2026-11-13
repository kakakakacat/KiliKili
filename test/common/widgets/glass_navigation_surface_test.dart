import 'dart:ui';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:kilikili/common/widgets/floating_navigation_bar.dart';

void main() {
  testWidgets('导航胶囊裁切玻璃背景并保留子项点击', (tester) async {
    var tapped = false;
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: Center(
            child: SizedBox(
              width: 280,
              height: 64,
              child: GlassNavigationSurface(
                child: TextButton(
                  onPressed: () => tapped = true,
                  child: const Text('首页'),
                ),
              ),
            ),
          ),
        ),
      ),
    );

    expect(find.byType(BackdropFilter), findsOneWidget);
    expect(find.byType(ClipRRect), findsOneWidget);
    await tester.tap(find.text('首页'));
    expect(tapped, isTrue);
  });

  testWidgets('五个导航项在窄屏中仍可点击且不溢出', (tester) async {
    var selected = -1;
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: Center(
            child: SizedBox(
              width: 320,
              height: 100,
              child: FloatingNavigationBar(
                destinations: [
                  for (var i = 0; i < 5; i++)
                    FloatingNavigationDestination(
                      icon: const Icon(Icons.circle_outlined),
                      label: '项目$i',
                    ),
                ],
                onDestinationSelected: (value) => selected = value,
              ),
            ),
          ),
        ),
      ),
    );
    expect(tester.takeException(), isNull);
    await tester.tap(find.text('项目4'));
    expect(selected, 4);
  });

  testWidgets('选中背景同时包住图标与文字，并随选中项移动', (tester) async {
    Widget buildBar(int selected) => MaterialApp(
      home: Scaffold(
        body: Center(
          child: SizedBox(
            width: 300,
            height: 80,
            child: GlassNavigationSurface(
              selectedIndex: selected,
              destinationCount: 3,
              child: NavigationBar(
                height: 80,
                backgroundColor: Colors.transparent,
                indicatorColor: Colors.transparent,
                selectedIndex: selected,
                destinations: const [
                  NavigationDestination(icon: Icon(Icons.home), label: '首页'),
                  NavigationDestination(icon: Icon(Icons.update), label: '动态'),
                  NavigationDestination(icon: Icon(Icons.person), label: '我的'),
                ],
              ),
            ),
          ),
        ),
      ),
    );

    await tester.pumpWidget(buildBar(0));
    final selectedBackground = find.byKey(
      const Key('glass-selected-indicator'),
    );
    final firstRect = tester.getRect(selectedBackground);
    final iconRect = tester.getRect(find.byIcon(Icons.home));
    final labelRect = tester.getRect(find.text('首页'));
    expect(firstRect.contains(iconRect.topLeft), isTrue);
    expect(firstRect.contains(labelRect.bottomRight), isTrue);

    await tester.pumpWidget(buildBar(1));
    await tester.pumpAndSettle();
    final secondRect = tester.getRect(selectedBackground);
    expect(secondRect.left, greaterThan(firstRect.left));
    expect(
      secondRect.contains(tester.getRect(find.text('动态')).bottomRight),
      isTrue,
    );
  });

  testWidgets('悬浮导航的选中背景完整包住图标和文字', (tester) async {
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: Align(
            alignment: Alignment.bottomCenter,
            child: FloatingNavigationBar(
              destinations: const [
                FloatingNavigationDestination(icon: Icon(Icons.home), label: '首页'),
                FloatingNavigationDestination(icon: Icon(Icons.person), label: '我的'),
              ],
            ),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();
    final background = tester.getRect(
      find.byKey(const Key('glass-selected-indicator')),
    );
    expect(background.contains(tester.getRect(find.byIcon(Icons.home)).topLeft), isTrue);
    expect(background.contains(tester.getRect(find.text('首页')).bottomRight), isTrue);
  });
}
