import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:kilikili/rust/api/misc.dart' as rust_api;
import 'package:kilikili/rust/api/transport.dart';

import 'bridge_env.dart';

/// 云端弹幕屏蔽规则的两个**写操作**跨 FFI 的集成测试
/// （#66 的尾巴，ADR-013 的第一次落地）。
///
/// ## 这里验的是请求面，不是响应
///
/// ADR-013 决策六：写操作的响应通常只有 `{"code":0}`，对它冻结快照几乎
/// 什么都证明不了；真正会错、而且错了要到真机上才暴露的是**请求**——
/// 方法、content-type、字段名与取值、csrf 放在哪里。
///
/// Rust 侧的 `danmaku_filter::write_tests` 已经钉住了构造出来的
/// `PreparedWrite` 长什么样；这里验的是另一件事：**那个形状真的被发出去了**
/// ——跨过 FFI、经过 `send_write` 补 csrf、经过 reqwest 编码之后，落到
/// 服务端手上的还是它。两者都绿，这条写路径才算有回归保护。
void main() {
  rust_api.MiscContext contextFor(int port, {String? csrf}) =>
      rust_api.MiscContext(
        baseUrl: 'http://127.0.0.1:$port',
        headers: const [],
        csrf: csrf,
        mixinKey: '',
        wts: 0,
        timeoutMs: BigInt.from(3000),
        transport: TransportSettings(
          // 重试次数设 0，免得桩服务端被重发的请求搅乱计数。写操作的
          // 重试规则本身由 `http::should_retry` 的全表单测钉住。
          retryCount: 0,
          retryDelayMs: BigInt.zero,
          acceptInvalidCerts: false,
          http1Only: true,
        ),
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

  test('新增：POST 表单、字段齐、csrf 由 Rust 补上', () async {
    final (server, seen) = await stubServer(
      '{"code":0,"message":"0","data":{"id":77,"type":0,"filter":"剧透"}}',
    );
    addTearDown(() => server.close(force: true));

    final result = await rust_api.addDanmakuFilter(
      context: contextFor(server.port, csrf: 'csrf-placeholder'),
      filter: '剧透',
      kind: 0,
    );

    final req = await seen;
    expect(req.method, 'POST', reason: '写操作不能退化成 GET');
    expect(req.path, '/x/dm/filter/user/add');
    expect(
      req.contentType,
      'application/x-www-form-urlencoded',
      reason: '基线这条显式写了 formUrlEncoded，换成 json 服务端不认',
    );
    expect(req.form['type'], '0');
    expect(req.form['filter'], '剧透');
    expect(
      req.form['csrf'],
      'csrf-placeholder',
      reason: 'csrf 由 send_write 统一补，调用点不经手（ADR-013 决策四）',
    );

    expect(result.errorKind, isNull);
    expect(result.data?.id, 77);
    expect(result.data?.filter, '剧透');
  }, skip: bridgeSkip);

  test('删除：只带 ids 与 csrf', () async {
    final (server, seen) = await stubServer('{"code":0,"message":"0"}');
    addTearDown(() => server.close(force: true));

    final result = await rust_api.delDanmakuFilter(
      context: contextFor(server.port, csrf: 'csrf-placeholder'),
      ids: 4242,
    );

    final req = await seen;
    expect(req.method, 'POST');
    expect(req.path, '/x/dm/filter/user/del');
    expect(req.form, {'ids': '4242', 'csrf': 'csrf-placeholder'});
    // 成功时没有载荷：errorKind 为空就是全部结论。
    expect(result.errorKind, isNull);
  }, skip: bridgeSkip);

  test('未登录：不伪造一个空 csrf', () async {
    final (server, seen) = await stubServer(
      '{"code":-101,"message":"账号未登录"}',
    );
    addTearDown(() => server.close(force: true));

    final result = await rust_api.delDanmakuFilter(
      context: contextFor(server.port),
      ids: 1,
    );

    final req = await seen;
    expect(
      req.form.containsKey('csrf'),
      isFalse,
      reason: '带空 csrf 会换来 -111（校验失败），掩盖掉真实原因 -101',
    );
    // 失败是**值**不是异常（ADR-011 决策一）：走到这里就说明没抛。
    expect(result.errorKind, 'auth');
    expect(result.errorCode, -101);
  }, skip: bridgeSkip);

  test('新增失败时 data 为空而不是抛异常', () async {
    final (server, _) = await stubServer(
      '{"code":-101,"message":"账号未登录"}',
    );
    addTearDown(() => server.close(force: true));

    final result = await rust_api.addDanmakuFilter(
      context: contextFor(server.port, csrf: 'csrf-placeholder'),
      filter: '剧透',
      kind: 0,
    );

    expect(result.data, isNull);
    expect(result.errorKind, 'auth');
    expect(result.errorCode, -101);
  }, skip: bridgeSkip);
}

/// 桩服务端收到的那一次请求里，本测试关心的几项。
class _SeenRequest {
  const _SeenRequest({
    required this.method,
    required this.path,
    required this.contentType,
    required this.form,
  });

  final String method;
  final String path;
  final String? contentType;
  final Map<String, String> form;
}
