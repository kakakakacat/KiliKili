import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:typed_data' show ByteData, Uint8List;

import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart'
    show ExternalLibrary;
import 'package:flutter_rust_bridge/flutter_rust_bridge.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:kilikili/rust/api/init.dart';
import 'package:kilikili/rust/api/live.dart' show LiveContext;
import 'package:kilikili/rust/api/live_danmaku.dart';
import 'package:kilikili/rust/api/transport.dart';
import 'package:kilikili/rust/frb_generated.dart';

final enabled = Platform.environment['KILIKILI_RUST_BRIDGE_TESTS'] == '1';
const skipReason = '需要 KILIKILI_RUST_BRIDGE_TESTS=1 和编译后的 Rust 原生库';
const fixtureDir = 'rust/crates/bili-api/tests/fixtures/live_danmaku';

TransportSettings settings() => TransportSettings(
  retryCount: 0,
  retryDelayMs: BigInt.zero,
  acceptInvalidCerts: false,
  http1Only: true,
);
LiveBlockRules rules() =>
    LiveBlockRules(keywords: const [], uids: Int64List.fromList([]));

Uint8List packet(int operation, List<int> body, {int version = 1}) {
  final header = ByteData(16)
    ..setUint32(0, body.length + 16)
    ..setUint16(4, 16)
    ..setUint16(6, version)
    ..setUint32(8, operation)
    ..setUint32(12, 1);
  return Uint8List.fromList([...header.buffer.asUint8List(), ...body]);
}

void main() {
  setUpAll(() async {
    if (!enabled) return;
    final library = Platform.environment['KILIKILI_LIVE_BRIDGE_LIBRARY'];
    await RustLib.init(
      externalLibrary: library == null ? null : ExternalLibrary.open(library),
    );
    await initApp(maxLevel: 'off');
  });

  test('REST typed data crosses FFI with explicit header overrides', () async {
    final server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
    final captured = <HttpRequest>[];
    server.listen((request) async {
      captured.add(request);
      request.response.headers.contentType = ContentType.json;
      if (request.uri.path.contains('gethistory')) {
        request.response.write(
          File('$fixtureDir/prefetch.json').readAsStringSync(),
        );
      } else if (request.uri.path.contains('getDanmuInfo')) {
        request.response.write(
          File('$fixtureDir/token.json').readAsStringSync(),
        );
      } else {
        final event = jsonDecode(
          File('$fixtureDir/super_chat.json').readAsStringSync(),
        ) as Map;
        request.response.write(
          jsonEncode({
            'code': 0,
            'data': {
              'list': [event['data']],
            },
          }),
        );
      }
      await request.response.close();
    });
    final context = LiveContext(
      baseUrl: 'http://127.0.0.1:${server.port}',
      cookie: 'fixture-cookie',
      headers: const [('User-Agent', 'old'), ('Referer', 'old')],
      timeoutMs: BigInt.from(2000),
      transport: settings(),
    );
    try {
      final prefetch = await fetchLiveDanmakuPrefetch(
        context: context,
        roomId: 42,
        userAgent: 'fixture-UA',
      );
      expect(prefetch.errorKind, isNull);
      expect(prefetch.data, hasLength(10));
      expect(captured.first.headers.value('user-agent'), 'fixture-UA');
      expect(
        captured.first.headers.value('referer'),
        'https://live.bilibili.com/42',
      );
      expect(captured.first.headers.value('cookie'), 'fixture-cookie');
      final token = await fetchLiveDanmakuToken(
        context: context,
        roomId: 42,
        mixinKey: 'fixture-key',
        wts: 100,
      );
      expect(token.data!.servers, isNotEmpty);
      expect(
        captured[1].uri.queryParameters.keys,
        containsAll(['id', 'web_location', 'wts', 'w_rid']),
      );
      final sc = await fetchLiveSuperChat(context: context, roomId: 42);
      expect(sc.errorKind, isNull);
      expect(sc.data!.single.price, 30);
      expect(sc.data!.single.roomId, 42);
    } finally {
      await server.close(force: true);
    }
  }, skip: enabled ? false : skipReason);

  test('zlib event batch and generation survive the real bridge', () async {
    final server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
    WebSocket? socket;
    final disconnected = Completer<void>();
    server.listen((request) async {
      socket = await WebSocketTransformer.upgrade(request);
      socket!.listen(
        (data) {
          final bytes = data as List<int>;
          expect(
            ByteData.sublistView(Uint8List.fromList(bytes)).getUint32(8),
            7,
          );
          socket!.add(packet(8, utf8.encode('{"code":0}')));
          final body = File('$fixtureDir/danmaku.json').readAsBytesSync();
          socket!.add(
            packet(5, ZLibEncoder().convert(packet(5, body)), version: 2),
          );
        },
        onDone: () {
          if (!disconnected.isCompleted) disconnected.complete();
        },
      );
    });
    final opened = liveSocketOpen(
      servers: ['ws://127.0.0.1:${server.port}/sub'],
      roomId: 42,
      uid: 0,
      token: 'fixture',
      generation: 33,
      transport: settings(),
      rules: rules(),
    );
    final handle = opened.data!;
    try {
      expect(
        liveSocketPoll(sessionId: handle.sessionId, generation: 32).errorKind,
        'cancelled',
      );
      final event = await (() async {
        for (var attempt = 0; attempt < 200; attempt++) {
          final response = liveSocketPoll(
            sessionId: handle.sessionId,
            generation: 33,
          );
          expect(response.errorKind, isNull);
          if (response.data!.events.isNotEmpty) {
            return response.data!.events.single;
          }
          await Future<void>.delayed(const Duration(milliseconds: 10));
        }
        throw StateError('等待直播弹幕批次超时');
      })();
      expect(event.kind, LiveEventKind.danmaku);
      final message = event.danmaku!;
      expect(message.segments, hasLength(2));
      final index = message.segments.last.emoteIndex!;
      expect(message.emotes[index].emote.url, startsWith('https://'));
      expect(
        liveDanmakuFilterMask(
          candidates: [
            LiveFilterCandidate(text: message.text, mid: message.mid),
          ],
          rules: LiveBlockRules(
            keywords: [message.text],
            uids: Int64List.fromList([]),
          ),
        ),
        [false],
      );
    } finally {
      liveSocketClose(sessionId: handle.sessionId, generation: 33);
      expect(
        liveSocketPoll(sessionId: handle.sessionId, generation: 33).errorKind,
        'cancelled',
      );
      await disconnected.future.timeout(const Duration(seconds: 2));
      await socket?.close();
      await server.close(force: true);
    }
  }, skip: enabled ? false : skipReason);
}
