import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:kilikili/rust/api/search.dart' as rust_api;
import 'package:kilikili/rust/api/transport.dart';

import 'bridge_env.dart';

/// Typed 直连的搜索路径：真实 HTTP 请求 -> Rust 解析 -> FRB -> Dart。
/// 输入与 search_golden.rs、search_parity_test.dart 共用同一份脱敏样本。
void main() {
  const fixtureDir = 'rust/crates/bili-api/tests/fixtures/search';

  rust_api.SearchContext contextFor(int port) => rust_api.SearchContext(
    baseUrl: 'http://127.0.0.1:$port',
    headers: const [],
    mixinKey: '',
    wts: 0,
    timeoutMs: BigInt.from(3000),
    transport: TransportSettings(
      retryCount: 0,
      retryDelayMs: BigInt.zero,
      acceptInvalidCerts: false,
      http1Only: true,
    ),
  );

  setUpAll(() async {
    if (bridgeTestsRequested) await initRustBridge();
  });

  tearDownAll(() async {
    if (bridgeTestsRequested) await closeRustBridge();
  });

  test('热搜经真实 Rust 网络路径和 FFI 返回冻结的基线结果', () async {
    final body = File('$fixtureDir/search_trending.raw.json')
        .readAsStringSync();
    final expected = jsonDecode(
      File('$fixtureDir/search_trending.rust.json').readAsStringSync(),
    ) as Map<String, dynamic>;
    final server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
    addTearDown(() => server.close(force: true));

    var requests = 0;
    String? seenPath;
    String? seenLimit;
    server.listen((request) async {
      requests++;
      seenPath = request.uri.path;
      seenLimit = request.uri.queryParameters['limit'];
      request.response.headers.contentType = ContentType.json;
      request.response.write(body);
      await request.response.close();
    });

    final result = await rust_api.searchTrending(
      context: contextFor(server.port),
      limit: 10,
      needsTop: true,
    );

    expect(requests, 1, reason: 'Rust 没有实际发出请求');
    expect(seenPath, '/x/v2/search/trending/ranking');
    expect(seenLimit, '10');
    expect(result.errorKind, isNull);
    expect(result.numResults, expected['top_count']);
    final items = result.data;
    expect(items, isNotNull);
    final golden = expected['list'] as List<dynamic>;
    expect(items, hasLength(golden.length));
    for (var i = 0; i < golden.length; i++) {
      final want = golden[i] as Map<String, dynamic>;
      final got = items![i];
      expect(got.keyword, want['keyword'], reason: '第 $i 条 keyword');
      expect(got.showName, want['show_name'], reason: '第 $i 条 show_name');
      expect(got.icon, want['icon'], reason: '第 $i 条 icon');
      expect(
        got.showLiveIcon,
        want['show_live_icon'],
        reason: '第 $i 条 show_live_icon',
      );
      expect(
        got.recommendReason,
        want['recommend_reason'],
        reason: '第 $i 条 reason',
      );
    }
  }, skip: bridgeSkip);

  test('搜索发现经 App 路径和签名通过 FFI 返回推荐词及业务错误', () async {
    final server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
    addTearDown(() => server.close(force: true));
    final seen = <Uri>[];
    server.listen((request) async {
      seen.add(request.uri);
      request.response.headers.contentType = ContentType.json;
      request.response.write(
        seen.length == 1
            ? '{"code":0,"data":{"list":[{"keyword":"示例推荐"}]}}'
            : '{"code":-400,"message":"请求错误"}',
      );
      await request.response.close();
    });

    final result = await rust_api.searchRecommend(
      context: contextFor(server.port),
      appkey: 'appkey',
      appsec: 'appsec',
      accessKey: 'test-access-key',
    );
    expect(result.errorKind, isNull);
    expect(result.data!.single.keyword, '示例推荐');
    expect(seen.single.path, '/x/v2/search/recommend');
    expect(seen.single.queryParameters['access_key'], 'test-access-key');
    expect(seen.single.queryParameters['appkey'], 'appkey');
    expect(seen.single.queryParameters['ts'], '0');
    expect(seen.single.queryParameters['sign'], isNotEmpty);
    expect(seen.single.queryParameters.containsKey('w_rid'), isFalse);

    final error = await rust_api.searchRecommend(
      context: contextFor(server.port),
      appkey: 'appkey',
      appsec: 'appsec',
    );
    expect(error.data, isNull);
    expect(error.errorCode, -400);
    expect(error.errorDetail, 'code -400: 请求错误');
    expect(seen.last.queryParameters.containsKey('access_key'), isFalse);
  }, skip: bridgeSkip);

  test('损坏的响应通过 FFI 返回协议错误', () async {
    final server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
    addTearDown(() => server.close(force: true));
    var requests = 0;
    server.listen((request) async {
      requests++;
      request.response.write('{invalid json');
      await request.response.close();
    });

    final result = await rust_api.searchTrending(
      context: contextFor(server.port),
      limit: 10,
      needsTop: true,
    );

    expect(requests, 1);
    expect(result.data, isNull);
    expect(result.errorKind, 'protocol');
  }, skip: bridgeSkip);
}
