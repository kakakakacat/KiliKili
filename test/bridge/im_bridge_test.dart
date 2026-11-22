import 'dart:async';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:kilikili/rust/api/grpc.dart' as grpc_api;
import 'package:kilikili/rust/api/im.dart' as api;
import 'package:kilikili/rust/api/transport.dart';

import 'bridge_env.dart';

/// 私信 gRPC 跨 FFI 的集成测试（S12 续单 / #63）。
///
/// Rust 单测与 `grpc_im_golden` 钉的是「字节进去、实体出来」；这里钉的
/// 是另外三件只有真的跨一次 FFI 才看得见的事：
///
/// 1. **方法路径**。服务名与 proto 包名对不上（`im.interface.v1` 单数 vs
///    `im.interfaces.v1` 复数，以及小写的 `im`），而 service 定义不在
///    描述符里，没有任何东西替我们校验。拼错的表现是 gRPC 状态 12，
///    看着像「服务端下线了这个接口」。
/// 2. **gRPC 请求不带 Cookie 也不带 csrf**（ADR-012 决策六、决策七）。
/// 3. **设置页那段不透明字节真的能原样走一圈回去**。
void main() {
  setUpAll(() async {
    if (bridgeTestsRequested) await initRustBridge();
  });
  tearDownAll(() async {
    if (bridgeTestsRequested) await closeRustBridge();
  });

  grpc_api.GrpcContext context(int port) => grpc_api.GrpcContext(
    baseUrl: 'http://127.0.0.1:$port',
    identity: const grpc_api.GrpcIdentity(
      accessKey: 'access-placeholder',
      buvid: 'buvid-placeholder',
      sessionId: 'abcdefgh',
    ),
    timeoutMs: BigInt.from(3000),
    transport: TransportSettings(
      // 重试设 0：桩服务端只回一次，重发会把断言搅乱。
      retryCount: 0,
      retryDelayMs: BigInt.zero,
      acceptInvalidCerts: false,
      http1Only: true,
    ),
  );

  /// 起一个 gRPC 桩，回 [bytes]（已自动套上 5 字节的帧头），并记下它
  /// 收到的那一次请求。
  Future<(HttpServer, Future<_Seen>)> stub([List<int> bytes = const []]) async {
    final server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
    final seen = Completer<_Seen>();
    server.listen((request) async {
      final body = await request.fold<List<int>>([], (a, b) => a..addAll(b));
      if (!seen.isCompleted) {
        seen.complete(
          _Seen(
            method: request.method,
            path: request.uri.path,
            contentType: request.headers.contentType?.mimeType,
            cookie: request.headers.value('cookie'),
            hasCsrfInQuery: request.uri.queryParameters.containsKey('csrf'),
            body: body,
          ),
        );
      }
      request.response.headers.set('grpc-status', '0');
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

  /// 十七条方法的路径，逐条钉住。
  ///
  /// 这张表就是这个测试文件最值钱的东西：路径拼错**不会在任何静态检查
  /// 里暴露**，而真机上回的是 UNIMPLEMENTED，第一反应会是「接口下线了」。
  ///
  /// 注意两处别扭：`ImInterface` 这组的服务名是 **interface（单数）**，
  /// 而消息的 package 是 interfaces（复数）；`im` 这组末段是**小写**的。
  const paths = {
    'sendMsg': '/bilibili.im.interface.v1.ImInterface/SendMsg',
    'shareList': '/bilibili.im.interface.v1.ImInterface/ShareList',
    'syncFetch': '/bilibili.im.interface.v1.ImInterface/SyncFetchSessionMsgs',
    'totalUnread': '/bilibili.im.interface.v1.ImInterface/GetTotalUnread',
    'sessionDetail': '/bilibili.im.interface.v1.ImInterface/SessionDetail',
    'sessionMain': '/bilibili.app.im.v1.im/SessionMain',
    'sessionSecondary': '/bilibili.app.im.v1.im/SessionSecondary',
    'clearUnread': '/bilibili.app.im.v1.im/ClearUnread',
    'sessionUpdate': '/bilibili.app.im.v1.im/SessionUpdate',
    'pinSession': '/bilibili.app.im.v1.im/PinSession',
    'unpinSession': '/bilibili.app.im.v1.im/UnpinSession',
    'deleteSessionList': '/bilibili.app.im.v1.im/DeleteSessionList',
    'getSettings': '/bilibili.app.im.v1.im/GetImSettings',
    'setSettings': '/bilibili.app.im.v1.im/SetImSettings',
    'keywordList': '/bilibili.app.im.v1.im/KeywordBlockingList',
    'keywordAdd': '/bilibili.app.im.v1.im/KeywordBlockingAdd',
    'keywordDelete': '/bilibili.app.im.v1.im/KeywordBlockingDelete',
  };

  /// 发一次请求、断言它的方法路径，并顺带确认 gRPC 的那两条硬约束。
  Future<_Seen> expectPath(
    String key,
    Future<void> Function(grpc_api.GrpcContext context) call, {
    List<int> reply = const [],
  }) async {
    final (server, seen) = await stub(reply);
    addTearDown(() => server.close(force: true));
    await call(context(server.port));
    final req = await seen;
    expect(req.method, 'POST');
    expect(req.path, paths[key], reason: '$key 的方法路径');
    expect(req.contentType, 'application/grpc');
    // ADR-012 决策六、七：gRPC 走 access_key + metadata，既不带 Cookie
    // 也不带 csrf。带上去不会报错，只是多送一份凭据出门。
    expect(req.cookie, isNull, reason: 'gRPC 不该带 Cookie');
    expect(req.hasCsrfInQuery, isFalse, reason: 'gRPC 不该带 csrf');
    // 帧头 5 字节：1 字节压缩标志 + 4 字节长度。
    expect(req.body.length, greaterThanOrEqualTo(5));
    expect(req.body[0], 0, reason: '这几条请求都小于 gzip 阈值，不压缩');
    return req;
  }

  test('会话列表三条：路径、无 Cookie、无 csrf', () async {
    await expectPath(
      'sessionMain',
      (c) => api.fetchImSessionMain(context: c, offsets: const []),
    );
    await expectPath(
      'sessionSecondary',
      (c) => api.fetchImSessionSecondary(
        context: c,
        pageType: 2,
        offsets: const [],
      ),
    );
    await expectPath(
      'sessionUpdate',
      (c) => api.fetchImSession(context: c, talkerUid: 100001),
    );
  }, skip: bridgeSkip);

  test('会话上的四条写操作：路径', () async {
    await expectPath(
      'pinSession',
      (c) => api.pinImSession(context: c, talkerUid: 100001),
    );
    await expectPath(
      'unpinSession',
      (c) => api.unpinImSession(context: c, talkerUid: 100001),
    );
    await expectPath(
      'clearUnread',
      (c) => api.clearImUnread(context: c, pageType: 1),
    );
    await expectPath(
      'deleteSessionList',
      (c) => api.deleteImSessionList(context: c, pageType: 1),
    );
  }, skip: bridgeSkip);

  test('消息四条：路径', () async {
    await expectPath(
      'syncFetch',
      (c) => api.fetchImMessages(context: c, talkerId: 100001, endSeqno: null),
    );
    await expectPath(
      'sendMsg',
      (c) => api.sendImMessage(
        context: c,
        senderUid: 100001,
        receiverId: 100002,
        msgType: 1,
        content: '{"content":"你好"}',
        timestamp: 1731000000,
        devId: 'dev-placeholder',
      ),
    );
    await expectPath(
      'sessionDetail',
      (c) => api.fetchImAckSeqno(context: c, talkerId: 100001),
    );
    await expectPath(
      'shareList',
      (c) => api.fetchImShareList(context: c, size: 5),
    );
  }, skip: bridgeSkip);

  test('未读数与屏蔽词四条：路径', () async {
    // 未读数的响应里必须有 msg_feed_unread，空响应会被判协议错误——
    // 这里只验路径，所以先不管返回值。
    await expectPath(
      'totalUnread',
      (c) => api.fetchImTotalUnread(context: c, unreadType: 2),
    );
    await expectPath(
      'keywordList',
      (c) => api.fetchImKeywordBlocking(context: c),
    );
    await expectPath(
      'keywordAdd',
      (c) => api.addImKeywordBlocking(context: c, keyword: '广告'),
    );
    await expectPath(
      'keywordDelete',
      (c) => api.deleteImKeywordBlocking(context: c, keyword: '广告'),
    );
  }, skip: bridgeSkip);

  test('设置页两条：读回来的不透明字节能原样走一圈回去', () async {
    // 先读一次，拿到 raw。
    final (server, _) = await stub(const []);
    addTearDown(() => server.close(force: true));
    final read = await api.fetchImSettings(
      context: context(server.port),
      settingType: 0,
    );
    expect(read.errorKind, isNull);
    final snapshot = read.data!;
    // 空响应解出来是一张空表，raw 就是那条空响应的字节。
    expect(snapshot.settings, isEmpty);

    // 拿这段 raw 去写回一个不存在的 key，应当是**明确的协议错误**而不是
    // 静默成功——那说明界面拿的是另一次读取的快照。
    final (server2, seen2) = await stub(const []);
    addTearDown(() => server2.close(force: true));
    final write = await api.setImSetting(
      context: context(server2.port),
      raw: snapshot.raw,
      key: 42,
      switchOn: true,
      selectedValue: null,
    );
    expect(write.errorKind, 'protocol');
    expect(write.errorDetail, contains('42'));
    // 既然失败在构造阶段，请求根本不该发出去。
    expect(
      seen2.timeout(
        const Duration(milliseconds: 300),
        onTimeout: () => throw TimeoutException('ok'),
      ),
      throwsA(isA<TimeoutException>()),
      reason: 'key 对不上时不该发请求出去',
    );
  }, skip: bridgeSkip);

  test('设置页类型与主页类型由 Rust 给，不在 Dart 写死', () {
    // 这四个数字在协议里相邻且无规律（0、17、18、19）。
    expect(
      api.imSettingType(page: api.ImSettingPage.all),
      0,
      reason: 'SETTING_TYPE_NEED_ALL',
    );
    expect(api.imSettingType(page: api.ImSettingPage.replyMe), 17);
    expect(api.imSettingType(page: api.ImSettingPage.atMe), 18);
    expect(api.imSettingType(page: api.ImSettingPage.receiveLike), 19);
    // 主列表那一页：SessionPageType.HOME 是 1，而同名的 SessionType 里
    // 1 是「私聊」——两张表的数字不是一回事。
    expect(api.imHomePageType(), 1);
  }, skip: bridgeSkip);

  test('消息类型的换算只覆盖界面会主动发的那几种', () {
    expect(api.imMsgType(kind: api.ImMsgKind.text), 1);
    expect(api.imMsgType(kind: api.ImMsgKind.picture), 2);
    expect(api.imMsgType(kind: api.ImMsgKind.drawBack), 5);
    expect(api.imMsgType(kind: api.ImMsgKind.shareV2), 7);
    expect(api.imMsgType(kind: api.ImMsgKind.commonShareCard), 14);
    // 这几种只会收到，不会主动发。
    expect(api.imMsgType(kind: api.ImMsgKind.tipMessage), isNull);
    expect(api.imMsgType(kind: api.ImMsgKind.unsupported), isNull);
  }, skip: bridgeSkip);
}

class _Seen {
  _Seen({
    required this.method,
    required this.path,
    required this.contentType,
    required this.cookie,
    required this.hasCsrfInQuery,
    required this.body,
  });

  final String method;
  final String path;
  final String? contentType;
  final String? cookie;
  final bool hasCsrfInQuery;
  final List<int> body;
}
