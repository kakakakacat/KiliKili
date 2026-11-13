import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:kilikili/rust/api/grpc.dart' as api;
import 'package:kilikili/rust/api/transport.dart';

import 'bridge_env.dart';

void main() {
  setUpAll(() async {
    if (bridgeTestsRequested) await initRustBridge();
  });
  tearDownAll(() async {
    if (bridgeTestsRequested) await closeRustBridge();
  });
  api.GrpcContext context(int port) => api.GrpcContext(
    baseUrl: 'http://127.0.0.1:$port',
    identity: const api.GrpcIdentity(
      accessKey: 'access-placeholder',
      buvid: 'buvid-placeholder',
      sessionId: 'abcdefgh',
    ),
    timeoutMs: BigInt.from(3000),
    transport: TransportSettings(
      retryCount: 0,
      retryDelayMs: BigInt.zero,
      acceptInvalidCerts: false,
      http1Only: true,
    ),
  );
  Future<(HttpServer, Future<List<int>>)> stub(
    List<int> bytes, {
    String status = '0',
    String? statusDetails,
  }) async {
    final server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
    final seen = Completer<List<int>>();
    server.listen((request) async {
      final body = await request.fold<List<int>>([], (a, b) => a..addAll(b));
      expect(request.method, 'POST');
      expect(request.headers.contentType?.mimeType, 'application/grpc');
      expect(
        request.headers.value('authorization'),
        'identify_v1 access-placeholder',
      );
      expect(request.headers.value('cookie'), isNull);
      expect(request.uri.queryParameters.containsKey('csrf'), isFalse);
      if (!seen.isCompleted) seen.complete(body);
      request.response.headers.set('grpc-status', status);
      if (statusDetails != null) {
        request.response.headers.set('grpc-status-details-bin', statusDetails);
      }
      request.response.add([
        0,
        0,
        0,
        (bytes.length >> 8) & 255,
        bytes.length & 255,
        ...bytes,
      ]);
      await request.response.close();
    });
    return (server, seen.future);
  }

  test('metadata 跨 FFI 与 Dart 冻结字节一致', () {
    final expected =
        (jsonDecode(
              File(
                'rust/crates/bili-api/tests/fixtures/grpc/metadata.dart.json',
              ).readAsStringSync(),
            ) as List).last
            as Map;
    final headers = Map.fromEntries(
      api
          .buildGrpcHeaders(
            identity: const api.GrpcIdentity(
              accessKey: 'access-placeholder',
              buvid: 'buvid-placeholder',
              sessionId: 'abcdefgh',
            ),
          )
          .map((e) => MapEntry(e.$1, e.$2)),
    );
    expect(headers['x-bili-metadata-bin'], expected['metadata']);
  }, skip: bridgeSkip);
  test('分段经真实 HTTP、帧解码与 FFI 返回字节', () async {
    final payload = File(
      'rust/crates/bili-api/tests/fixtures/grpc/dm_segment.payload.pb',
    ).readAsBytesSync();
    final (server, seen) = await stub(payload);
    addTearDown(() => server.close(force: true));
    final result = await api.fetchDmSegment(
      context: context(server.port),
      cid: 2,
      segmentIndex: 1,
      kind: 0,
    );
    expect(result.errorKind, isNull);
    expect(result.data, payload);
    expect(await seen, [0, 0, 0, 0, 6, 0x10, 2, 0x18, 0, 0x20, 1]);
  }, skip: bridgeSkip);
  test('字幕结果直接返回 typed 数据与错误值', () async {
    final bytes = File(
      'rust/crates/bili-api/tests/fixtures/dm_view/typical.raw.pb',
    ).readAsBytesSync();
    final (server, seen) = await stub(bytes);
    addTearDown(() => server.close(force: true));
    final result = await api.fetchDmSubtitles(
      context: context(server.port),
      aid: 1,
      cid: 2,
    );
    expect(result.errorKind, isNull);
    expect(result.data, isNotEmpty);
    expect(await seen, [0, 0, 0, 0, 6, 8, 1, 0x10, 2, 0x18, 1]);
    final expected = jsonDecode(
      File(
        'rust/crates/bili-api/tests/fixtures/dm_view/typical.normalized.json',
      ).readAsStringSync(),
    );
    expect([
      for (final s in result.data!)
        {
          'id': s.id.toInt(),
          'lan': s.lan,
          'lan_doc': s.lanDoc,
          'subtitle_url': s.subtitleUrl,
          'subtitle_type': s.subtitleType,
          'is_ai': s.isAi,
        },
    ], expected);
  }, skip: bridgeSkip);
  test('损坏的字幕响应返回 protocol，不抛异常', () async {
    final (server, _) = await stub([0x1a, 0x7f, 1, 2]);
    addTearDown(() => server.close(force: true));
    final result = await api.fetchDmSubtitles(
      context: context(server.port),
      aid: 1,
      cid: 2,
    );
    expect(result.errorKind, 'protocol');
    expect(result.data, isNull);
  }, skip: bridgeSkip);
  test('嵌套业务错误码跨 FFI 原样返回', () async {
    final inner = [
      8,
      0xf0,
      0xfc,
      0xff,
      0xff,
      0xff,
      0xff,
      0xff,
      0xff,
      0xff,
      1,
      18,
      3,
      98,
      97,
      100,
    ];
    final any = [18, inner.length, ...inner];
    final (server, _) = await stub(
      [],
      status: '13',
      statusDetails: base64Encode([8, 13, 26, any.length, ...any]),
    );
    addTearDown(() => server.close(force: true));
    final result = await api.fetchDmSubtitles(
      context: context(server.port),
      aid: 1,
      cid: 2,
    );
    expect(result.errorKind, 'remote');
    expect(result.errorCode, -400);
    expect(result.errorDetail, 'bad');
    expect(result.data, isNull);
  }, skip: bridgeSkip);
}
