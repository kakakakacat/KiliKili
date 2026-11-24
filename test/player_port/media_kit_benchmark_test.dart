import 'dart:async';
import 'dart:convert';
import 'dart:ffi';
import 'dart:io';

import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart'
    show ExternalLibrary;
import 'package:flutter_test/flutter_test.dart';
import 'package:media_kit/media_kit.dart';
import 'package:kilikili/rust/frb_generated.dart';
import 'package:kilikili/rust/api/player_port.dart';
import 'package:kilikili/plugin/pl_player/player_port/flutter_media_kit_adapter.dart';
import 'package:kilikili/plugin/pl_player/player_port/media_kit_backend.dart';

void main() {
  final mode = Platform.environment['KILIKILI_PLAYER_BENCH_MODE'];
  test(
    'same-process-shape baseline / Player Port measurement',
    () async {
      await RustLib.init(
        externalLibrary: ExternalLibrary.open(
          Platform.environment['KILIKILI_PLAYER_BRIDGE_LIBRARY']!,
        ),
      );
      MediaKit.ensureInitialized(
        libmpv: Platform.environment['KILIKILI_LIBMPV'],
      );
      DynamicLibrary.open(
        Platform.environment['KILIKILI_MEDIA_KIT_EVENT_LOOP_LIBRARY']!,
      );
      NativePlayer.test = true;
      final player = await NativePlayer.create(
        configuration: const PlayerConfiguration(
          options: {'vo': 'null', 'ao': 'null', 'hwdec': 'no'},
        ),
      );
      final port = mode == 'port'
          ? FlutterMediaKitAdapter(
              session: RustPlayerPortSession(),
              backend: MediaKitBackend(player),
            )
          : null;
      var callbacks = 0;
      final subscription = port != null
          ? port.events.listen((_) => callbacks++)
          : player.stream.position.listen((_) => callbacks++);
      final clock = Stopwatch()..start();
      final rss = <int>[];
      final timer = Timer.periodic(
        const Duration(milliseconds: 100),
        (_) => rss.add(ProcessInfo.currentRss),
      );
      final commands = <String, int>{};
      Future<void> command(String name, Future<dynamic> Function() run) async {
        final watch = Stopwatch()..start();
        await run();
        commands[name] = watch.elapsedMicroseconds;
      }

      try {
        final media = Platform.environment['KILIKILI_PLAYER_MEDIA']!;
        await command(
          'open',
          () => port == null
              ? player.open(Media(media), play: false)
              : port.send(PlayerIntent(kind: .open, video: media)),
        );
        await command(
          'play',
          () => port == null
              ? player.play()
              : port.send(const PlayerIntent(kind: .play)),
        );
        // 测量窗口相同；不把原生 dispose 的固定延时计入命令耗时。
        final before = callbacks;
        final window = Stopwatch()..start();
        await Future<void>.delayed(const Duration(seconds: 8));
        final measuredCallbacks = callbacks - before;
        final measuredMs = window.elapsedMilliseconds;
        if (port != null) expect(measuredCallbacks, lessThanOrEqualTo(205));
        await command(
          'pause',
          () => port == null
              ? player.pause()
              : port.send(const PlayerIntent(kind: .pause)),
        );
        await Future<void>.delayed(const Duration(milliseconds: 1400));
        await command(
          'seek',
          () => port == null
              ? player.seek(const Duration(seconds: 3))
              : port.send(
                  PlayerIntent(kind: .seek, positionMs: BigInt.from(3000)),
                ),
        );
        await command(
          'speed',
          () => port == null
              ? player.setRate(2)
              : port.send(const PlayerIntent(kind: .setSpeed, speed: 2)),
        );
        timer.cancel();
        // baseline 的 Future 仅保证命令送达；port 还等待实际 open/seek 确认。
        // 因此这些耗时用于显示确认成本，不能据此声称后端更快/更慢。
        final result = {
          'mode': mode,
          'wall_ms': clock.elapsedMilliseconds,
          'playing_window_ms': measuredMs,
          'playing_callbacks': measuredCallbacks,
          'callbacks_per_second': measuredCallbacks * 1000 / measuredMs,
          'rss_mean_bytes': rss.reduce((a, b) => a + b) ~/ rss.length,
          'rss_peak_bytes': rss.reduce((a, b) => a > b ? a : b),
          'command_us': commands,
        };
        // ignore: avoid_print
        print('PLAYER_BENCH ${jsonEncode(result)}');
      } finally {
        timer.cancel();
        await subscription.cancel();
        if (port != null) {
          await port.dispose();
        } else {
          await player.dispose();
        }
        await Future<void>.delayed(const Duration(seconds: 6));
      }
    },
    skip: mode != 'baseline' && mode != 'port',
    timeout: const Timeout(Duration(seconds: 40)),
  );
}
