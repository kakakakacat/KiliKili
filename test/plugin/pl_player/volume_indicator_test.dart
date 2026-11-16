import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:kilikili/plugin/pl_player/widgets/volume_indicator.dart';

void main() {
  testWidgets('音量填充从底部连续增高，且不显示数字', (tester) async {
    for (final level in [0.0, 0.25, 0.5, 1.0]) {
      await tester.pumpWidget(
        MaterialApp(
          home: Center(child: VolumeIndicator(level: level)),
        ),
      );
      expect(find.byType(Text), findsNothing);
      final track = tester.getRect(find.byType(ClipRRect));
      final fill = tester.getRect(find.byKey(const ValueKey('volume-fill')));
      expect(track.height, greaterThan(track.width));
      expect(fill.height, closeTo(track.height * level, 0.01));
      expect(fill.bottom, closeTo(track.bottom, 0.01));
      expect(
        find.byIcon(
          level == 0 ? Icons.volume_off_rounded : Icons.volume_up_rounded,
        ),
        findsNWidgets(2),
      );
    }
  });

  testWidgets('横竖屏与小窗中胶囊保持在播放器范围内', (tester) async {
    for (final size in [
      const Size(360, 203),
      const Size(800, 450),
      const Size(120, 80),
    ]) {
      await tester.pumpWidget(
        MaterialApp(
          home: Center(
            child: SizedBox.fromSize(
              size: size,
              child: const VolumeIndicator(level: 0.5),
            ),
          ),
        ),
      );
      expect(tester.takeException(), isNull);
      final indicator = tester.getRect(find.byType(VolumeIndicator));
      final track = tester.getRect(find.byType(ClipRRect));
      expect(indicator.contains(track.topLeft), isTrue);
      expect(indicator.contains(track.bottomRight), isTrue);
    }
  });
}
