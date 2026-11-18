import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:kilikili/rust/api/fav.dart' as fav_api;
import 'package:kilikili/rust/api/transport.dart';

import 'bridge_env.dart';

/// 收藏夹切片跨 FFI 的集成测试（S8 / issue #58）。
///
/// Rust 单测钉住构造出来的 `PreparedWrite`，这里钉住**真的发出去的**那一个
/// ——跨过 FFI、经过 `send_write` 补 csrf、经过 reqwest 编码之后。
///
/// 只挑这一片里**最容易悄悄错**的几条，不是把 34 条都跑一遍：
///
/// 1. **排序的 csrf 被 APP 签名覆盖**。它是全仓库唯一一条「csrf 不由
///    `send_write` 补」的路径（见 `bili-api/src/fav.rs` 模块文档）。补错
///    位置和没补的表现一样：服务端回 `-111`，看起来像协议问题。
/// 2. **图文收藏是 JSON 体 + csrf 在查询串**。这一片其余 21 条都是表单，
///    统一成表单会让 CI 全绿而真机上回一个无从反推的业务错误码
///    （ADR-013 补充七）。
/// 3. **写操作的响应没有 data**。`{"code":0,"message":"0"}` 必须判成功
///    ——这正是 ADR-013 补充三那条只有端到端才抓得到的错。
/// 4. 业务失败是返回值不是异常，错误码原样带回。
void main() {
  TransportSettings noRetry() => TransportSettings(
    // 重试次数设 0，免得桩服务端被重发的请求搅乱计数。
    retryCount: 0,
    retryDelayMs: BigInt.zero,
    acceptInvalidCerts: false,
    http1Only: true,
  );

  fav_api.FavContext context(int port, {String? csrf}) => fav_api.FavContext(
    baseUrl: 'http://127.0.0.1:$port',
    headers: const [],
    csrf: csrf,
    timeoutMs: BigInt.from(3000),
    transport: noRetry(),
  );

  /// 起一个只回一句话的桩服务端，并把它收到的那一次请求记下来。
  Future<(HttpServer, Future<_SeenRequest>)> stubServer([
    String body = '{"code":0,"message":"0","ttl":1}',
  ]) async {
    final server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
    final seen = Completer<_SeenRequest>();
    server.listen((request) async {
      final raw = await utf8.decoder.bind(request).join();
      if (!seen.isCompleted) {
        seen.complete(
          _SeenRequest(
            method: request.method,
            path: request.uri.path,
            query: request.uri.queryParameters,
            contentType: request.headers.contentType?.mimeType,
            body: raw,
          ),
        );
      }
      request.response.headers.contentType = ContentType.json;
      request.response.write(body);
      await request.response.close();
    });
    return (server, seen.future);
  }

  setUpAll(() async {
    if (bridgeTestsRequested) await initRustBridge();
  });

  tearDownAll(() async {
    if (bridgeTestsRequested) await closeRustBridge();
  });

  test('收藏夹排序：csrf 进了表单，并且被 APP 签名覆盖', () async {
    final (server, seen) = await stubServer();
    addTearDown(() => server.close(force: true));

    final result = await fav_api.sortFavFolder(
      context: context(server.port, csrf: 'csrf-placeholder'),
      signature: fav_api.FavSignature(
        appkey: 'appkey-placeholder',
        appsec: 'appsec-placeholder',
        ts: 1700000000,
      ),
      sort: '2101,2102',
    );

    final req = await seen;
    final form = Uri.splitQueryString(req.body);
    expect(req.method, 'POST');
    expect(req.path, '/x/v3/fav/folder/sort');
    expect(req.contentType, 'application/x-www-form-urlencoded');
    expect(form['sort'], '2101,2102');
    expect(form['csrf'], 'csrf-placeholder');
    expect(form['appkey'], 'appkey-placeholder');
    expect(form['ts'], '1700000000');
    expect(form['sign'], isNotNull, reason: '没签名服务端直接拒');
    expect(
      req.body.indexOf('csrf='),
      lessThan(req.body.indexOf('&sign=')),
      reason: 'csrf 必须在签名之前进表单，否则 sign 覆盖不到它',
    );
    expect(
      req.query,
      isEmpty,
      reason: '这条的参数全在体里，csrf 不该再出现在查询串上',
    );
    expect(result.errorKind, isNull);
  }, skip: bridgeSkip);

  test('图文收藏：体是 JSON，csrf 在查询串上', () async {
    final (server, seen) = await stubServer();
    addTearDown(() => server.close(force: true));

    final result = await fav_api.communityAction(
      context: context(server.port, csrf: 'csrf-placeholder'),
      opusId: '990000000000000001',
      action: 4,
    );

    final req = await seen;
    expect(req.path, '/x/community/cosmo/interface/simple_action');
    expect(
      req.contentType,
      'application/json',
      reason: '统一成表单会让 CI 全绿，而真机上服务端回一个无从反推的业务错误码',
    );
    expect(
      req.query['csrf'],
      'csrf-placeholder',
      reason: '基线把它放在 queryParameters 里；补进体里服务端会回 -111',
    );
    final body = jsonDecode(req.body) as Map<String, dynamic>;
    expect(body['action'], 4);
    expect(
      body['entity']['object_id_str'],
      '990000000000000001',
      reason: 'opus id 超出 JS 安全整数范围，必须是字符串',
    );
    expect(body['entity']['type']['biz'], 2);
    expect(result.errorKind, isNull);
  }, skip: bridgeSkip);

  test('没有 data 的响应就是写操作的成功', () async {
    final (server, seen) = await stubServer();
    addTearDown(() => server.close(force: true));

    final result = await fav_api.favVideo(
      context: context(server.port, csrf: 'csrf-placeholder'),
      resources: '110000001:2',
      addIds: '2101',
      delIds: '',
    );

    final form = Uri.splitQueryString((await seen).body);
    expect(form['resources'], '110000001:2');
    expect(form['add_media_ids'], '2101');
    expect(
      form['del_media_ids'],
      '',
      reason: '空串也要发出去：基线对两侧都是 `?? \'\'`',
    );
    expect(result.errorKind, isNull, reason: '没有 data 才是写操作的正常响应');
  }, skip: bridgeSkip);

  test('未登录时不伪造空 csrf', () async {
    final (server, seen) = await stubServer();
    addTearDown(() => server.close(force: true));

    await fav_api.favFolder(
      context: context(server.port),
      mediaId: 2101,
      fav: true,
    );

    final form = Uri.splitQueryString((await seen).body);
    expect(
      form.containsKey('csrf'),
      isFalse,
      reason: '带着空 csrf 发出去，服务端回的是 -111，看起来像协议问题；'
          '不带则是 -101，那才是真实原因',
    );
  }, skip: bridgeSkip);

  test('业务失败是值不是异常，错误码原样带回', () async {
    final (server, _) = await stubServer('{"code":-101,"message":"账号未登录"}');
    addTearDown(() => server.close(force: true));

    final result = await fav_api.cleanFav(
      context: context(server.port),
      mediaId: 2101,
    );

    expect(result.errorKind, 'auth');
    expect(result.errorCode, -101);
  }, skip: bridgeSkip);

  test('只读路径：收藏夹详情跨 FFI 原样回来', () async {
    final raw = File(
      'rust/crates/bili-api/tests/fixtures/fav/fav_detail.raw.json',
    ).readAsStringSync();
    final (server, seen) = await stubServer(raw);
    addTearDown(() => server.close(force: true));

    final result = await fav_api.fetchFavDetail(
      context: context(server.port),
      mediaId: 2101,
      pn: 1,
      ps: 20,
      keyword: '',
      order: fav_api.FavOrder.mtime,
      kind: 0,
      pageDesc: false,
    );

    final req = await seen;
    expect(req.method, 'GET');
    expect(req.path, '/x/v3/fav/resource/list');
    expect(req.query['order'], 'mtime');
    expect(req.query['platform'], 'web');

    expect(result.errorKind, isNull);
    final data = result.data!;
    expect(data.info!.title, '默认收藏夹');
    expect(data.medias.length, 3);
    expect(data.medias[0].ugcFirstCid, 220000001);
    expect(data.medias[1].ogvSeasonId, 44000001);
    expect(
      data.medias[2].bvid,
      'BV1yy411c7YY',
      reason: '只有 bv_id 没有 bvid 的那一项也要认，少认一个的表现是卡片点不开',
    );
  }, skip: bridgeSkip);
}

/// 桩服务端收到的那一次请求里，本测试关心的几项。
class _SeenRequest {
  const _SeenRequest({
    required this.method,
    required this.path,
    required this.query,
    required this.contentType,
    required this.body,
  });

  final String method;
  final String path;
  final Map<String, String> query;
  final String? contentType;

  /// 原始体。这一片两种 content-type 都有，不能统一按表单拆。
  final String body;
}
