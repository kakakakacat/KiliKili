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

final enabled = Platform.environment['KILIKILI_PLAYER_RUNTIME_TESTS'] == '1';

void main() {
  setUpAll(() async {
    if (!enabled) return;
    await RustLib.init(
      externalLibrary: ExternalLibrary.open(
        Platform.environment['KILIKILI_PLAYER_BRIDGE_LIBRARY']!,
      ),
    );
    MediaKit.ensureInitialized(
      libmpv: Platform.environment['KILIKILI_LIBMPV'],
    );
    final eventLoop =
        Platform.environment['KILIKILI_MEDIA_KIT_EVENT_LOOP_LIBRARY'];
    if (eventLoop != null) DynamicLibrary.open(eventLoop);
    NativePlayer.test = true;
  });
  test(
    'real media-kit uses backend clock for open/pause/seek/speed/replace',
    () async {
      final baseline = playerPortSessionCount();
      final session = RustPlayerPortSession();
      final player = await NativePlayer.create(
        configuration: const PlayerConfiguration(
          options: {'vo': 'null', 'ao': 'null', 'hwdec': 'no'},
        ),
      );
      final adapter = FlutterMediaKitAdapter(
        session: session,
        backend: MediaKitBackend(player),
      );
      final snapshots = <PlayerPortBatch>[];
      final subscription = adapter.events.listen(snapshots.add);
      Future<PlayerPortSnapshot> waitFor(
        bool Function(PlayerPortSnapshot) predicate,
      ) async {
        final deadline = DateTime.now().add(const Duration(seconds: 8));
        while (DateTime.now().isBefore(deadline)) {
          final snapshot = snapshots.lastOrNull?.snapshot;
          if (snapshot != null && predicate(snapshot)) return snapshot;
          await Future<void>.delayed(const Duration(milliseconds: 40));
        }
        throw StateError('没有等到后端事实，最后快照: ${snapshots.lastOrNull?.snapshot}');
      }

      try {
        final media = Platform.environment['KILIKILI_PLAYER_MEDIA']!;
        final ticket = session.prepare(PlayerIntent(kind: .open, video: media));
        expect(await adapter.execute(ticket), isTrue);
        await waitFor((s) => s.durationMs != null);
        await adapter.send(const PlayerIntent(kind: .play));
        await waitFor(
          (s) => s.state == .playing && s.positionMs > BigInt.from(200),
        );
        await adapter.send(const PlayerIntent(kind: .pause));
        final paused = await waitFor((s) => s.state == .paused);
        await Future<void>.delayed(const Duration(milliseconds: 250));
        expect(snapshots.last.snapshot!.positionMs, paused.positionMs);
        expect(
          await adapter.send(
            PlayerIntent(kind: .seek, positionMs: BigInt.from(5000)),
          ),
          isTrue,
        );
        await waitFor((s) => (s.positionMs.toInt() - 5000).abs() < 150);
        await adapter.send(const PlayerIntent(kind: .setSpeed, speed: 2));
        await waitFor((s) => s.speed == 2);
        await adapter.send(const PlayerIntent(kind: .setSpeed, speed: 0.5));
        await waitFor((s) => s.speed == 0.5);
        expect(
          await adapter.send(
            PlayerIntent(kind: .seek, positionMs: BigInt.from(1000)),
          ),
          isTrue,
        );
        await waitFor((s) => (s.positionMs.toInt() - 1000).abs() < 150);
        final replacement = session.prepare(
          PlayerIntent(kind: .open, video: media),
        );
        expect(await adapter.execute(replacement), isTrue);
        final before = snapshots.length;
        await waitFor((s) => s.positionMs < BigInt.from(500));
        expect(snapshots.last.generation, replacement.generation);
        expect(snapshots.length, greaterThanOrEqualTo(before));
        // 这是无窗口软件解码验证，不宣称视频表面、弹幕或真机生命周期通过。
        // ignore: avoid_print
        print(
          'Player Port runtime: ${snapshots.length} batches; pause, seek ±, speed 0.5/2, replace PASS',
        );
      } finally {
        await adapter.dispose();
        await subscription.cancel();
        expect(playerPortSessionCount(), baseline);
        await Future<void>.delayed(const Duration(seconds: 6));
      }
    },
    skip: !enabled,
    timeout: const Timeout(Duration(seconds: 50)),
  );
  test(
    'DASH resources preserve both tracks and injected HTTP headers',
    () async {
      final mediaDir = File(Platform.environment['KILIKILI_PLAYER_MEDIA']!)
          .parent;
      final video = File('${mediaDir.path}/video.mp4').readAsBytesSync();
      final audio = File('${mediaDir.path}/audio.m4a').readAsBytesSync();
      final requests = <(String, String?, String?)>[];
      final server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
      server.listen((request) async {
        requests.add((
          request.uri.path,
          request.headers.value('user-agent'),
          request.headers.value('referer'),
        ));
        final bytes = request.uri.path == '/video.mp4' ? video : audio;
        final range = RegExp(r'bytes=(\d+)-(\d*)')
            .firstMatch(request.headers.value('range') ?? '');
        final start = range == null ? 0 : int.parse(range[1]!);
        final end = range == null || range[2]!.isEmpty
            ? bytes.length - 1
            : int.parse(range[2]!);
        final response = request.response;
        response.headers.set('Accept-Ranges', 'bytes');
        response.headers.contentType = ContentType('video', 'mp4');
        if (range != null) {
          response.statusCode = HttpStatus.partialContent;
          response.headers.set(
            'Content-Range',
            'bytes $start-$end/${bytes.length}',
          );
        }
        response
          ..contentLength = end - start + 1
          ..add(bytes.sublist(start, end + 1));
        await response.close();
      });
      final baseline = playerPortSessionCount();
      final session = RustPlayerPortSession();
      final player = await NativePlayer.create(
        configuration: const PlayerConfiguration(
          options: {'vo': 'null', 'ao': 'null', 'hwdec': 'no'},
        ),
      );
      final adapter = FlutterMediaKitAdapter(
        session: session,
        backend: MediaKitBackend(player),
      );
      try {
        final playable = adapter.events
            .firstWhere(
              (b) =>
                  b.snapshot?.videoTrack != null &&
                  b.snapshot?.audioTrack != null,
            )
            .timeout(const Duration(seconds: 8));
        expect(
          await adapter.send(
            PlayerIntent(
              kind: .open,
              video: 'http://127.0.0.1:${server.port}/video.mp4',
              audio: 'http://127.0.0.1:${server.port}/audio.m4a',
              userAgent: 'KiliKili-PlayerPort-Fixture',
              referer: 'https://fixture.invalid/',
            ),
          ),
          isTrue,
        );
        final batch = await playable;
        expect(
          requests.map((r) => r.$1).toSet(),
          containsAll(['/video.mp4', '/audio.m4a']),
        );
        expect(
          requests.every(
            (r) =>
                r.$2 == 'KiliKili-PlayerPort-Fixture' &&
                r.$3 == 'https://fixture.invalid/',
          ),
          isTrue,
        );
        expect(
          await adapter.send(
            PlayerIntent(
              kind: .selectTracks,
              videoTrack: batch.snapshot!.videoTrack,
              audioTrack: batch.snapshot!.audioTrack,
            ),
          ),
          isTrue,
        );
      } finally {
        await adapter.dispose();
        await server.close(force: true);
        expect(playerPortSessionCount(), baseline);
        await Future<void>.delayed(const Duration(seconds: 6));
      }
    },
    skip: !enabled,
    timeout: const Timeout(Duration(seconds: 40)),
  );
}
