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
}
