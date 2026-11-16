import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:kilikili/rust/api/transport.dart';
import 'package:kilikili/rust/api/video_write.dart' as rust_api;

import 'bridge_env.dart';

/// 视频页写操作跨 FFI 的集成测试（S4 / issue #54）。
///
/// 验的是**请求面**（ADR-013 决策六）：这一片的响应大多只有
/// `{"code":0}`，冻结它证明不了什么；会错的是方法、content-type、
/// 字段名与取值、csrf 放在哪里、签名有没有发出去。
///
/// Rust 单测（`bili_api::video_write::tests`）钉住的是**构造出来的**
/// `PreparedWrite`；这里钉住的是**真的发出去的**那一个——跨过 FFI、
/// 经过 `send_write` 补 csrf、经过 reqwest 编码之后落到服务端手上的东西。
///
/// 这一片有三件事只有走到这一层才看得出来，也正是本文件的重点：
///
/// 1. **APP 端接口不带 csrf、但要带 access_key 与 sign**，而且走的是
///    另一个主机；
/// 2. **`roomEntryAction` 的体是 JSON、csrf 在查询串上**——它是整片里
///    唯一一条吃 dio 默认 content-type 的接口，统一成表单服务端不认；
/// 3. **「不感兴趣」是 GET**，退化成 POST 或者反过来都编译得过。
void main() {
  TransportSettings noRetry() => TransportSettings(
    // 重试设 0，免得桩服务端被重发的请求搅乱计数。重试规则本身由
    // `http::should_retry` 的全表单测钉住。
    retryCount: 0,
    retryDelayMs: BigInt.zero,
    acceptInvalidCerts: false,
    http1Only: true,
  );

  rust_api.VideoWriteContext webContext(int port, {String? csrf}) =>
      rust_api.VideoWriteContext(
        baseUrl: 'http://127.0.0.1:$port',
        headers: const [],
        csrf: csrf,
        timeoutMs: BigInt.from(3000),
        transport: noRetry(),
      );

  /// 占位 appkey/appsec，与 `bili_protocol::app_sign` 的测试同源，
  /// 不是用户凭据。
  rust_api.AppWriteContext appContext(int port, {String? accessKey}) =>
      rust_api.AppWriteContext(
        baseUrl: 'http://127.0.0.1:$port',
        headers: const [],
        appkey: 'dfca71928277209b',
        appsec: 'b5475a8825547a4fc26c7d518eaaa02e',
        accessKey: accessKey,
        ts: 1700000000,
        timeoutMs: BigInt.from(3000),
        transport: noRetry(),
      );

  /// 起一个只回一句话的桩服务端，并把它收到的那一次请求记下来。
  Future<(HttpServer, Future<_SeenRequest>)> stubServer(String body) async {
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
            rawBody: raw,
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

  test('点赞：APP 端签名、不带 csrf，like=0 才是点赞', () async {
    final (server, seen) = await stubServer(
      '{"code":0,"message":"0","data":{"toast":"已赞"}}',
    );
    addTearDown(() => server.close(force: true));

    final result = await rust_api.likeVideo(
      context: appContext(server.port, accessKey: 'access-key-placeholder'),
      bvid: 'BV1xx411c7mD',
      like: true,
    );

    final req = await seen;
    expect(req.method, 'POST');
    expect(req.path, '/x/v2/view/like');
    expect(req.contentType, 'application/x-www-form-urlencoded');
    expect(req.form['aid'], '2', reason: 'BV1xx411c7mD 是 av2');
    expect(
      req.form['like'],
      '0',
      reason: 'APP 端这条接口 0 才是点赞、1 是取消，写反了只是点赞变取消',
    );
    expect(req.form['access_key'], 'access-key-placeholder');
    expect(req.form['appkey'], 'dfca71928277209b');
    expect(req.form['ts'], '1700000000');
    expect(req.form['sign'], isNotNull, reason: '签名必须随请求发出');
    expect(
      req.form.containsKey('csrf'),
      isFalse,
      reason: 'APP 端接口鉴权靠 access_key，补 csrf 只会和基线对不上',
    );

    expect(result.errorKind, isNull);
    expect(result.toast, '已赞');
  }, skip: bridgeSkip);

  test('点赞：未登录时整个 access_key 字段不出现', () async {
    final (server, seen) = await stubServer('{"code":-101,"message":"未登录"}');
    addTearDown(() => server.close(force: true));

    final result = await rust_api.likeVideo(
      context: appContext(server.port),
      bvid: 'BV1xx411c7mD',
      like: false,
    );

    final req = await seen;
    expect(
      req.form.containsKey('access_key'),
      isFalse,
      reason: '补一个空串会让签名覆盖一个服务端不认的值',
    );
    expect(req.form['like'], '1', reason: '目标是取消赞');
    // 失败是**值**不是异常（ADR-011 决策一）：走到这里就说明没抛。
    expect(result.errorKind, 'auth');
    expect(result.errorCode, -101);
  }, skip: bridgeSkip);

  test('非法 BV 号：根本不发请求', () async {
    final (server, seen) = await stubServer('{"code":0,"message":"0"}');
    addTearDown(() => server.close(force: true));

    final result = await rust_api.coinVideo(
      context: appContext(server.port, accessKey: 'k'),
      bvid: 'not-a-bvid',
      multiply: 2,
      selectLike: 0,
    );

    expect(result.errorKind, 'protocol');
    await expectLater(
      seen
          .then<_SeenRequest?>((r) => r)
          .timeout(const Duration(milliseconds: 300), onTimeout: () => null),
      completion(isNull),
      reason: '转错一位就是把币投到别人的稿件上，宁可一个字节都不发',
    );
  }, skip: bridgeSkip);

  test('进直播间上报：JSON 体 + csrf 在查询串上', () async {
    final (server, seen) = await stubServer('{"code":0,"message":"0"}');
    addTearDown(() => server.close(force: true));

    await rust_api.roomEntryAction(
      context: webContext(server.port, csrf: 'csrf-placeholder'),
      roomId: 123,
    );

    final req = await seen;
    expect(req.method, 'POST');
    expect(req.path, '/xlive/web-room/v1/index/roomEntryAction');
    expect(
      req.contentType,
      'application/json',
      reason: '基线这条没写 contentType，吃的是 dio 的默认 JSON',
    );
    expect(
      req.query['csrf'],
      'csrf-placeholder',
      reason: '这条的 csrf 在查询串上，不在体里',
    );
    final body = jsonDecode(req.rawBody) as Map<String, dynamic>;
    expect(
      body['room_id'],
      123,
      reason: 'room_id 是数字不是字符串——基线传的是 Object，dio 按 JSON 序列化',
    );
    expect(body['platform'], 'pc');
    expect(body.containsKey('csrf'), isFalse, reason: 'csrf 不该同时出现两处');
  }, skip: bridgeSkip);

  test('关注：参数同时分布在查询串与表单里', () async {
    final (server, seen) = await stubServer('{"code":0,"message":"0"}');
    addTearDown(() => server.close(force: true));

    final result = await rust_api.relationMod(
      context: webContext(server.port, csrf: 'csrf-placeholder'),
      mid: 123,
      act: 1,
      reSrc: 11,
      fp: 'test-ua',
    );

    final req = await seen;
    expect(req.method, 'POST');
    expect(req.path, '/x/relation/modify');
    expect(req.query['statistics'], '{"appId":100,"platform":5}');
    expect(req.query['x-bili-device-req-json'], isNotNull);
    expect(req.form['fid'], '123');
    expect(req.form['act'], '1');
    expect(req.form['re_src'], '11');
    expect(
      req.form['extend_content'],
      '{"entity":"user","entity_id":123,"fp":"test-ua"}',
    );
    expect(req.form['csrf'], 'csrf-placeholder');
    expect(result.errorKind, isNull);
  }, skip: bridgeSkip);

  test('不感兴趣：是 GET，参数带签名走查询串', () async {
    final (server, seen) = await stubServer('{"code":0,"message":"0"}');
    addTearDown(() => server.close(force: true));

    final result = await rust_api.feedDislike(
      context: appContext(server.port, accessKey: 'access-key-placeholder'),
      goto: 'av',
      id: 42,
      reasonId: 7,
      cancel: false,
    );

    final req = await seen;
    expect(
      req.method,
      'GET',
      reason: '基线这条写操作用的就是 GET，换成 POST 等于顺手改协议',
    );
    expect(req.path, '/x/feed/dislike');
    expect(req.query['goto'], 'av');
    expect(req.query['id'], '42');
    expect(req.query['reason_id'], '7');
    expect(req.query.containsKey('feedback_id'), isFalse, reason: '二选一');
    expect(req.query['sign'], isNotNull);
    expect(result.errorKind, isNull);
  }, skip: bridgeSkip);

  test('不感兴趣：两个 id 都给时不猜一个发出去', () async {
    final (server, seen) = await stubServer('{"code":0,"message":"0"}');
    addTearDown(() => server.close(force: true));

    final result = await rust_api.feedDislike(
      context: appContext(server.port, accessKey: 'k'),
      goto: 'av',
      id: 42,
      reasonId: 7,
      feedbackId: 9,
      cancel: true,
    );

    expect(result.errorKind, 'protocol');
    await expectLater(
      seen
          .then<_SeenRequest?>((r) => r)
          .timeout(const Duration(milliseconds: 300), onTimeout: () => null),
      completion(isNull),
      reason: '基线在调用点用 assert 表达同一件事，而 assert 在 release 不执行',
    );
  }, skip: bridgeSkip);

  test('一键三连：部分成功照样是 Success，三件事各自回执', () async {
    final (server, seen) = await stubServer(
      '{"code":0,"message":"0","data":{"like":true,"coin":false,"fav":true,"multiply":2}}',
    );
    addTearDown(() => server.close(force: true));

    final result = await rust_api.ugcTriple(
      context: webContext(server.port, csrf: 'csrf-placeholder'),
      bvid: 'BV1xx411c7mD',
    );

    final req = await seen;
    expect(req.form['aid'], '2');
    expect(req.form['statistics'], '{"appId":100,"platform":5}');
    expect(req.form['csrf'], 'csrf-placeholder');

    expect(result.errorKind, isNull, reason: '投币没成不代表整次调用失败');
    expect(result.like, isTrue);
    expect(result.coin, isFalse);
    expect(result.multiply, 2);
  }, skip: bridgeSkip);

  test('追番状态：批量改时 season_id 是一串逗号分隔的 id', () async {
    final (server, seen) = await stubServer(
      '{"code":0,"message":"0","result":{"toast":"已更新"}}',
    );
    addTearDown(() => server.close(force: true));

    final result = await rust_api.pgcUpdate(
      context: webContext(server.port, csrf: 'csrf-placeholder'),
      seasonIds: '1,2,3',
      status: 2,
    );

    final req = await seen;
    expect(req.path, '/pgc/web/follow/status/update');
    expect(
      req.form['season_id'],
      '1,2,3',
      reason: '追番列表页的批量操作一次改掉勾选的若干季',
    );
    expect(req.form['status'], '2');
    // 这条的载荷在 result 里而不是 data 里。
    expect(result.toast, '已更新');
  }, skip: bridgeSkip);

  test('点踩：服务端回非 JSON 时归 remote，且原文可见', () async {
    final server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
    addTearDown(() => server.close(force: true));
    server.listen((request) async {
      await utf8.decoder.bind(request).join();
      request.response.headers.contentType = ContentType.html;
      request.response.write('<html>blocked</html>');
      await request.response.close();
    });

    final result = await rust_api.dislikeVideo(
      context: appContext(server.port, accessKey: 'k'),
      bvid: 'BV1xx411c7mD',
      dislike: true,
    );

    expect(
      result.errorKind,
      'remote',
      reason: '归 protocol 会读成「协议变了该改代码」，而这多半只是被网关拦了',
    );
    expect(result.errorDetail, contains('blocked'));
  }, skip: bridgeSkip);
}

/// 桩服务端收到的那一次请求里，本测试关心的几项。
class _SeenRequest {
  _SeenRequest({
    required this.method,
    required this.path,
    required this.query,
    required this.contentType,
    required this.rawBody,
  });

  final String method;
  final String path;
  final Map<String, String> query;
  final String? contentType;
  final String rawBody;

  /// 表单体解出来的字段。JSON 体请直接看 [rawBody]。
  late final Map<String, String> form = Uri.splitQueryString(rawBody);
}
