import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:kilikili/common/widgets/video_card/cover_pill.dart';
import 'package:kilikili/models/common/stat_type.dart';

void main() {
  testWidgets('封面统计药丸与时长角标等高', (tester) async {
    await tester.pumpWidget(
      const MaterialApp(
        home: Scaffold(
          body: SizedBox(
            width: 300,
            height: 100,
            child: Stack(
              children: [
                Positioned(
                  top: 6,
                  left: 6,
                  child: CoverStatsPill(
                    stats: [
                      (StatType.play, 1234),
                      (StatType.danmaku, 56),
                    ],
                  ),
                ),
                CoverDurationPill(
                  top: 6,
                  right: 6,
                  text: '12:34',
                ),
              ],
            ),
          ),
        ),
      ),
    );

    final statsHeight = tester.getSize(find.byType(CoverPill).first).height;
    final durationHeight = tester.getSize(find.byType(CoverPill).last).height;
    expect(statsHeight, durationHeight);
  });
}
