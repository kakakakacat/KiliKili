import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:kilikili/rust/api/misc.dart' as misc_api;
import 'package:kilikili/rust/api/pgc.dart' as pgc_api;
import 'package:kilikili/rust/api/transport.dart';

import 'bridge_env.dart';

/// 第二批写操作跨 FFI 的集成测试：短评五条（#60）、想听与关注分组排序（#66）。
///
/// 验的是**请求面**（ADR-013 决策六）：写操作的响应通常只有 `{"code":0}`，
/// 冻结它证明不了什么；会错的是方法、content-type、字段名与取值、csrf 放在
/// 哪里。Rust 单测钉住构造出来的 `PreparedWrite`，这里钉住**真的发出去的**
/// 那一个——跨过 FFI、经过 `send_write` 补 csrf、经过 reqwest 编码之后。
///
/// 三条最该被钉住的，都是「写反了也能编译、也不报错、只是功能悄悄失效」：
/// 想听那个反直觉的 `state` 映射、`share_feed` 的省略、以及分组排序把参数
/// 同时放在查询串和表单里。
void main() {
  TransportSettings noRetry() => TransportSettings(
    // 重试次数设 0，免得桩服务端被重发的请求搅乱计数。
    retryCount: 0,
    retryDelayMs: BigInt.zero,
    acceptInvalidCerts: false,
    http1Only: true,
  );

  misc_api.MiscContext miscContext(int port, {String? csrf}) =>
      misc_api.MiscContext(
        baseUrl: 'http://127.0.0.1:$port',
        headers: const [],
        csrf: csrf,
        mixinKey: '',
        wts: 0,
        timeoutMs: BigInt.from(3000),
        transport: noRetry(),
      );

  pgc_api.PgcContext pgcContext(int port, {String? csrf}) => pgc_api.PgcContext(
    baseUrl: 'http://127.0.0.1:$port',
    headers: const [],
    csrf: csrf,
    mixinKey: '',
    wts: 0,
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
            form: Uri.splitQueryString(raw),
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

  // -------------------------------------------------------------- #66

  test('想听：state 的映射是反直觉的那一种，1 才是加入', () async {
    final (server, seen) = await stubServer();
    addTearDown(() => server.close(force: true));

    final result = await misc_api.updateWish(
      context: miscContext(server.port, csrf: 'csrf-placeholder'),
      musicId: 'bgm-123',
      wish: true,
    );

    final req = await seen;
    expect(req.method, 'POST');
    expect(req.path, '/x/copyright-music-publicity/bgm/wish/update');
    expect(req.contentType, 'application/x-www-form-urlencoded');
    expect(req.form['music_id'], 'bgm-123');
    expect(
      req.form['state'],
      '1',
      reason: '想听要发 1。写成 2 的表现是按钮点一下没反应——服务端收下了，'
          '但做的是反向操作',
    );
    expect(req.form['csrf'], 'csrf-placeholder');
    expect(result.errorKind, isNull);
  }, skip: bridgeSkip);

  test('想听：取消发 2', () async {
    final (server, seen) = await stubServer();
    addTearDown(() => server.close(force: true));

    await misc_api.updateWish(
      context: miscContext(server.port, csrf: 'csrf-placeholder'),
      musicId: 'bgm-123',
      wish: false,
    );

    expect((await seen).form['state'], '2');
  }, skip: bridgeSkip);

  test('关注分组排序：参数同时分布在查询串与表单里', () async {
    final (server, seen) = await stubServer();
    addTearDown(() => server.close(force: true));

    final result = await misc_api.sortFollowTag(
      context: miscContext(server.port, csrf: 'csrf-placeholder'),
      tagids: '3,1,2',
    );

    final req = await seen;
    expect(req.path, '/x/relation/tags/update_sort');
    expect(
      req.query['x-bili-device-req-json'],
      '{"platform":"web","device":"pc","spmid":"333.1387"}',
      reason: '设备标识串在 URL 上。缺了它服务端按另一套风控规则处理',
    );
    expect(req.form['tagids'], '3,1,2', reason: '有查询串不等于表单可以空着');
    expect(req.form['csrf'], 'csrf-placeholder');
    expect(result.errorKind, isNull);
  }, skip: bridgeSkip);

  // -------------------------------------------------------------- #60

  test('发短评：不分享时整个 share_feed 字段都不发', () async {
    final (server, seen) = await stubServer();
    addTearDown(() => server.close(force: true));

    await pgc_api.pgcReviewPost(
      context: pgcContext(server.port, csrf: 'csrf-placeholder'),
      mediaId: 100,
      score: 10,
      content: '好看',
      shareFeed: false,
    );

    final req = await seen;
    expect(req.path, '/pgc/review/short/post');
    expect(req.form['score'], '10');
    expect(req.form['content'], '好看');
    expect(
      req.form.containsKey('share_feed'),
      isFalse,
      reason: '基线是 `if (shareFeed) share_feed: 1`——补成 share_feed=0 '
          '是另一个请求，服务端未必同解',
    );
  }, skip: bridgeSkip);

  test('发短评：分享时才带 share_feed=1', () async {
    final (server, seen) = await stubServer();
    addTearDown(() => server.close(force: true));

    await pgc_api.pgcReviewPost(
      context: pgcContext(server.port, csrf: 'csrf-placeholder'),
      mediaId: 100,
      score: 10,
      content: '好看',
      shareFeed: true,
    );

    expect((await seen).form['share_feed'], '1');
  }, skip: bridgeSkip);

  test('点赞：review_type 固定为 2（短评）', () async {
    final (server, seen) = await stubServer();
    addTearDown(() => server.close(force: true));

    await pgc_api.pgcReviewLike(
      context: pgcContext(server.port, csrf: 'csrf-placeholder'),
      mediaId: 100,
      reviewId: 200,
    );

    final req = await seen;
    expect(req.path, '/pgc/review/action/like');
    expect(req.form['review_type'], '2', reason: '1 是长评，这条路径只做短评');
    expect(req.form['review_id'], '200');
  }, skip: bridgeSkip);

  test('删短评：成功响应没有 data，不能判成协议错误', () async {
    // 这正是上一批抓到的那个坑（ADR-013 补充三）：写操作的信封没有 data，
    // 拿只读那套 unwrap_envelope 去解会把成功判成 protocol。
    final (server, seen) = await stubServer('{"code":0,"message":"0","ttl":1}');
    addTearDown(() => server.close(force: true));

    final result = await pgc_api.pgcReviewDel(
      context: pgcContext(server.port, csrf: 'csrf-placeholder'),
      mediaId: 100,
      reviewId: 200,
    );

    expect((await seen).path, '/pgc/review/short/del');
    expect(result.errorKind, isNull, reason: '没有 data 才是写操作的正常响应');
  }, skip: bridgeSkip);

  test('业务失败是值不是异常，错误码原样带回', () async {
    final (server, _) = await stubServer('{"code":-101,"message":"账号未登录"}');
    addTearDown(() => server.close(force: true));

    final result = await pgc_api.pgcReviewDel(
      context: pgcContext(server.port),
      mediaId: 100,
      reviewId: 200,
    );

    expect(result.errorKind, 'auth');
    expect(result.errorCode, -101);
  }, skip: bridgeSkip);
}

/// 桩服务端收到的那一次请求里，本测试关心的几项。
class _SeenRequest {
  const _SeenRequest({
    required this.method,
    required this.path,
    required this.query,
    required this.contentType,
    required this.form,
  });

  final String method;
  final String path;
  final Map<String, String> query;
  final String? contentType;
  final Map<String, String> form;
}
