import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:kilikili/common/widgets/video_card/cover_pill.dart';
import 'package:kilikili/common/widgets/stat/stat.dart';
import 'package:kilikili/models/common/stat_type.dart';

void main() {
  testWidgets('顶部渐变贴边、信息留 5 像素并在窄卡片中保持单行', (tester) async {
    await tester.pumpWidget(
      const MaterialApp(
        home: Scaffold(
          body: SizedBox(
            width: 180,
            height: 140,
            child: Stack(
              children: [
                CoverTopInfoBar(
                  stats: [(StatType.play, 123456), (StatType.danmaku, 7890)],
                  duration: '123:45',
                ),
              ],
            ),
          ),
        ),
      ),
    );

    final bar = find.byType(CoverTopInfoBar);
    expect(tester.getTopLeft(bar), Offset.zero);
    expect(tester.getSize(bar).width, 180);
    expect(find.text('123:45'), findsOneWidget);
    expect(tester.getTopLeft(find.byType(StatWidget).first).dy, 5);
    expect(tester.getTopLeft(find.text('123:45')).dy, 5);
    expect(tester.takeException(), isNull);
  });

  testWidgets('长观看进度在窄封面中不会溢出', (tester) async {
    await tester.pumpWidget(
      const MaterialApp(
        home: Scaffold(
          body: SizedBox(
            width: 140,
            height: 120,
            child: Stack(
              children: [
                CoverTopInfoBar(
                  stats: [(StatType.play, 1234567), (StatType.danmaku, 12345)],
                  duration: '123:45:67/234:56:78',
                ),
              ],
            ),
          ),
        ),
      ),
    );
    expect(tester.takeException(), isNull);
  });

  testWidgets('没有统计值时，时长仍靠右显示', (tester) async {
    await tester.pumpWidget(
      const MaterialApp(
        home: Scaffold(
          body: SizedBox(
            width: 200,
            height: 120,
            child: Stack(
              children: [CoverTopInfoBar(duration: '12:34')],
            ),
          ),
        ),
      ),
    );
    expect(tester.getTopRight(find.text('12:34')).dx, greaterThan(150));
  });
}
