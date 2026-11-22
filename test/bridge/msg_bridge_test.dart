import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:kilikili/rust/api/msg.dart' as msg_api;
import 'package:kilikili/rust/api/transport.dart';

import 'bridge_env.dart';

/// 消息中心九条写操作跨 FFI 的集成测试（S12 / #63）。
///
/// 验的是**请求面**（ADR-013 决策六）：这一片的写操作成功时只回
/// `{"code":0}`，冻结响应证明不了什么；会错的是方法、content-type、
/// 字段名与取值、以及 **csrf 放在哪里**。Rust 单测钉住构造出来的
/// `PreparedWrite`，这里钉住**真的发出去的**那一个——跨过 FFI、经过
/// `send_write` 补 csrf、经过 reqwest 编码之后。
///
/// 这一片最该被钉住的四条，都是「写错了也能编译、也不报错、只是功能
/// 悄悄失效或者换回一个无从反推的 `-111`」：
///
/// 1. `csrf` 与 `csrf_token` 在表单里要给**两份**（ADR-013 补充十六）；
/// 2. 删系统通知是 **JSON 体 + 查询串里再带一份 csrf**，而且 `mobi_app`
///    两处都是 `android`；
/// 3. 删会话带 WBI 签名、**签名覆盖 csrf**，所以表单里只能有一个 csrf
///    ——`send_write` 要是又补一份，就会出现两个同名字段（补充十七）；
/// 4. 两条「写操作却用 GET」（补充十八）。
void main() {
  TransportSettings noRetry() => TransportSettings(
    // 重试次数设 0，免得桩服务端被重发的请求搅乱计数。
    retryCount: 0,
    retryDelayMs: BigInt.zero,
    acceptInvalidCerts: false,
    http1Only: true,
  );

  msg_api.MsgContext context(int port, {String? csrf, String mixinKey = ''}) =>
      msg_api.MsgContext(
        baseUrl: 'http://127.0.0.1:$port',
        headers: const [],
        csrf: csrf,
        mixinKey: mixinKey,
        wts: 1702204169,
        timeoutMs: BigInt.from(3000),
        transport: noRetry(),
      );

  /// 起一个只回一句话的桩服务端，并把它收到的那一次请求记下来。
  ///
  /// `rawBody` 单独留着：删系统通知那条是 JSON 体，用
  /// `Uri.splitQueryString` 解会得到一个毫无意义的单键 map。
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

  // ------------------------------------------------ csrf 的双字段形态

  test('删消息中心条目：csrf 与 csrf_token 都在表单里', () async {
    final (server, seen) = await stubServer();
    addTearDown(() => server.close(force: true));

    final result = await msg_api.delMsgFeed(
      context: context(server.port, csrf: 'csrf-placeholder'),
      tp: 1,
      id: '1700000000000000001',
    );

    final req = await seen;
    expect(req.method, 'POST');
    expect(req.path, '/x/msgfeed/del');
    expect(req.contentType, 'application/x-www-form-urlencoded');
    expect(req.form['tp'], '1');
    expect(req.form['id'], '1700000000000000001');
    expect(req.form['mobi_app'], 'web');
    expect(req.form['csrf'], 'csrf-placeholder');
    expect(
      req.form['csrf_token'],
      'csrf-placeholder',
      reason: '少了 csrf_token 服务端回 -111，与完全不带 csrf 时的表现一样',
    );
    expect(result.errorKind, isNull);
  }, skip: bridgeSkip);

  test('免打扰：uid 是自己、dnd_uid 是对方，两个 csrf 字段都在', () async {
    final (server, seen) = await stubServer();
    addTearDown(() => server.close(force: true));

    await msg_api.setMsgDnd(
      context: context(server.port, csrf: 'csrf-placeholder'),
      uid: 100001,
      setting: 0,
      dndUid: 100002,
    );

    final req = await seen;
    expect(req.path, '/link_setting/v1/link_setting/set_msg_dnd');
    // 这两个传反了服务端照样回 200，只是设置落在了自己头上。
    expect(req.form['uid'], '100001', reason: 'uid 是自己的 mid');
    expect(req.form['dnd_uid'], '100002', reason: 'dnd_uid 是对方的');
    expect(req.form['setting'], '0', reason: 'setting 是目标状态，不是切换');
    expect(req.form['csrf_token'], 'csrf-placeholder');
    expect(req.form['csrf'], 'csrf-placeholder');
  }, skip: bridgeSkip);

  test('内容推送开关：talker_uid 是对方，setting 是目标状态', () async {
    final (server, seen) = await stubServer();
    addTearDown(() => server.close(force: true));

    await msg_api.setPushSs(
      context: context(server.port, csrf: 'csrf-placeholder'),
      setting: 1,
      talkerUid: 100002,
    );

    final req = await seen;
    expect(req.path, '/link_setting/v1/link_setting/set_push_ss');
    expect(req.form['talker_uid'], '100002');
    expect(req.form['setting'], '1');
    expect(req.form['csrf_token'], 'csrf-placeholder');
    expect(req.form['csrf'], 'csrf-placeholder');
  }, skip: bridgeSkip);

  test('某条赞的通知开关：notice_state 是目标状态', () async {
    final (server, seen) = await stubServer();
    addTearDown(() => server.close(force: true));

    await msg_api.setNotice(
      context: context(server.port, csrf: 'csrf-placeholder'),
      id: '1700000000000000020',
      noticeState: 1,
    );

    final req = await seen;
    expect(req.path, '/x/msgfeed/notice');
    expect(req.form['id'], '1700000000000000020');
    expect(req.form['notice_state'], '1');
    expect(req.form['tp'], '0');
    expect(req.form['csrf_token'], 'csrf-placeholder');
    expect(req.form['csrf'], 'csrf-placeholder');
  }, skip: bridgeSkip);

  // ------------------------------------------------ JSON 体 + 两处 csrf

  test('删系统通知：JSON 体、ids 是数组、csrf 在体和查询串各一份', () async {
    final (server, seen) = await stubServer();
    addTearDown(() => server.close(force: true));

    await msg_api.delSysMsg(
      context: context(server.port, csrf: 'csrf-placeholder'),
      id: '5000001',
    );

    final req = await seen;
    expect(req.method, 'POST');
    expect(req.path, '/x/sys-msg/del_notify_list');
    expect(req.contentType, 'application/json');

    final body = jsonDecode(req.rawBody) as Map<String, dynamic>;
    expect(
      body['ids'],
      ['5000001'],
      reason: 'ids 是数组。发成裸值服务端不报错，只是静默不删',
    );
    expect(body['station_ids'], isEmpty);
    expect(body['type'], 4);
    expect(
      body['mobi_app'],
      'android',
      reason: '这条接口只认 APP 端标识，写成 web 不会报错',
    );
    expect(body['csrf'], 'csrf-placeholder');

    expect(req.query['mobi_app'], 'android');
    expect(
      req.query['csrf'],
      'csrf-placeholder',
      reason: '体里有了还要再在查询串里带一份，基线如此',
    );
  }, skip: bridgeSkip);

  // ------------------------------------------------ 签名覆盖 csrf

  test('删会话：WBI 签名的表单体，w_rid 在最后且只有一个 csrf', () async {
    final (server, seen) = await stubServer();
    addTearDown(() => server.close(force: true));

    await msg_api.removeSession(
      context: context(
        server.port,
        // 上下文里**也**给一个 csrf，专门用来验 send_write 不会再补一遍。
        csrf: 'should-not-be-appended',
        mixinKey: 'ea1db124af3c7062474693fa704f4ff8',
      ),
      talkerId: 100002,
      csrf: 'csrf-placeholder',
    );

    final req = await seen;
    expect(req.method, 'POST');
    expect(req.path, '/session_svr/v1/session_svr/remove_session');
    expect(req.contentType, 'application/x-www-form-urlencoded');
    expect(req.form['talker_id'], '100002');
    expect(req.form['session_type'], '1');
    expect(req.form['wts'], '1702204169');
    expect(req.form['w_rid'], isNotEmpty);

    // 关键一条：csrf 只能出现一次，而且是构造时放进去、被签名覆盖的那个。
    // send_write 要是又补一份，表单里会出现两个同名字段，而 w_rid 覆盖
    // 不到后补的那个——服务端回 -111，看起来像协议问题。
    expect(
      'csrf=csrf-placeholder'.allMatches(req.rawBody).length,
      1,
      reason: 'csrf 必须只出现一次：已被签名覆盖，不能再补',
    );
    expect(
      req.rawBody.contains('should-not-be-appended'),
      isFalse,
      reason: '上下文里的 csrf 不该被 send_write 追加进来',
    );

    // w_rid 要在最后：它是对前面所有字段的签名。
    expect(
      req.rawBody.split('&').last.startsWith('w_rid='),
      isTrue,
      reason: 'w_rid 是对前面全部字段的签名，位置固定在末尾',
    );
  }, skip: bridgeSkip);

  // ------------------------------------------------ 写操作却用 GET

  test('系统通知已读游标：是 GET，参数在查询串上', () async {
    final (server, seen) = await stubServer();
    addTearDown(() => server.close(force: true));

    await msg_api.updateSysCursor(
      context: context(server.port),
      cursor: 1731000030000000000,
      csrf: 'csrf-placeholder',
    );

    final req = await seen;
    expect(
      req.method,
      'GET',
      reason: '基线发的就是 GET。改成 POST 是一次没人验证过的协议改动',
    );
    expect(req.path, '/x/sys-msg/update_cursor');
    expect(req.query['cursor'], '1731000030000000000');
    expect(req.query['csrf'], 'csrf-placeholder');
  }, skip: bridgeSkip);

  test('私信已读位点：GET + WBI 签名，csrf 也在签名范围里', () async {
    final (server, seen) = await stubServer();
    addTearDown(() => server.close(force: true));

    await msg_api.ackSession(
      context: context(
        server.port,
        mixinKey: 'ea1db124af3c7062474693fa704f4ff8',
      ),
      talkerId: 100002,
      ackSeqno: 42,
      csrf: 'csrf-placeholder',
    );

    final req = await seen;
    expect(req.method, 'GET');
    expect(req.path, '/session_svr/v1/session_svr/update_ack');
    expect(req.query['talker_id'], '100002');
    expect(req.query['ack_seqno'], '42');
    expect(req.query['session_type'], '1');
    expect(req.query['csrf'], 'csrf-placeholder');
    expect(req.query['csrf_token'], 'csrf-placeholder');
    expect(req.query['wts'], '1702204169');
    expect(req.query['w_rid'], isNotEmpty);
  }, skip: bridgeSkip);

  // ------------------------------------------------ 举报

  test('举报：comment 与 extra 是已序列化的 JSON 字符串', () async {
    final (server, seen) = await stubServer();
    addTearDown(() => server.close(force: true));

    await msg_api.imReport(
      context: context(server.port, csrf: 'csrf-placeholder'),
      accusedUid: 100002,
      reasonType: 3,
      reasonDesc: '垃圾广告',
      commentJson: '{"group_id":0,"msg_key":"123456"}',
      extraJson: '{"msg_keys":[]}',
    );

    final req = await seen;
    expect(req.method, 'POST');
    expect(req.path, '/x/bplus/im/report/add');
    expect(req.contentType, 'application/x-www-form-urlencoded');
    expect(req.form['biz_code'], '4');
    expect(req.form['accused_uid'], '100002');
    expect(
      req.form['object_id'],
      '100002',
      reason: '私信举报的「对象」就是那个人，不是某一条消息',
    );
    expect(req.form['module'], '604');
    expect(req.form['reason_type'], '3');
    expect(req.form['reason_desc'], '垃圾广告');
    expect(
      req.form['comment'],
      '{"group_id":0,"msg_key":"123456"}',
      reason: '发成对象的话服务端收到的是 [object Object] 那一类的东西',
    );
    expect(req.form['extra'], '{"msg_keys":[]}');
    // 举报只补 csrf，不补 csrf_token——这一条和同片的其余五条不一样。
    expect(req.form['csrf'], 'csrf-placeholder');
    expect(req.form.containsKey('csrf_token'), isFalse);
  }, skip: bridgeSkip);

  // ------------------------------------------------ 只读但要 csrf

  test('会话设置：只读的 GET，但 csrf 与 csrf_token 都要带', () async {
    final (server, seen) = await stubServer(
      '{"code":0,"message":"0","data":{"follow_status":2,"push_setting":1,"show_push_setting":1}}',
    );
    addTearDown(() => server.close(force: true));

    final result = await msg_api.fetchSessionSs(
      context: context(server.port),
      talkerUid: 100002,
      csrf: 'csrf-placeholder',
    );

    final req = await seen;
    expect(req.method, 'GET');
    expect(req.path, '/link_setting/v1/link_setting/get_session_ss');
    expect(req.query['talker_uid'], '100002');
    expect(
      req.query['csrf_token'],
      'csrf-placeholder',
      reason: 'api.vc 的 link_setting 组即使只读也校验 csrf，不带就回 -111',
    );
    expect(req.query['csrf'], 'csrf-placeholder');
    expect(result.data?.followStatus, 2);
    expect(result.data?.pushSetting, 1);
  }, skip: bridgeSkip);

  // ------------------------------------------------ 未读数的红点

  test('未读数：badge 由 Rust 算，私信六项、消息中心按勾选', () async {
    final (pmServer, _) = await stubServer(
      '{"code":0,"message":"0","data":{"follow_unread":1,"unfollow_unread":2,'
      '"biz_msg_follow_unread":4,"biz_msg_unfollow_unread":8,'
      '"unfollow_push_msg":16,"custom_unread":32,'
      '"dustbin_push_msg":64,"dustbin_unread":128}}',
    );
    addTearDown(() => pmServer.close(force: true));

    final pm = await msg_api.fetchSingleUnread(context: context(pmServer.port));
    expect(
      pm.data?.badge,
      63,
      reason: '六项相加。两个 dustbin_* 合计 192，算进去的话这里会是 255',
    );

    final (feedServer, _) = await stubServer(
      '{"code":0,"message":"0","data":{"at":1,"like":2,"reply":4,"sys_msg":8}}',
    );
    addTearDown(() => feedServer.close(force: true));

    final all = await msg_api.fetchFeedUnread(
      context: context(feedServer.port),
      pick: const msg_api.FeedBadgeSelection(
        reply: true,
        at: true,
        like: true,
        sysMsg: true,
      ),
    );
    expect(all.data?.badge, 15);

    final (feedServer2, _) = await stubServer(
      '{"code":0,"message":"0","data":{"at":1,"like":2,"reply":4,"sys_msg":8}}',
    );
    addTearDown(() => feedServer2.close(force: true));

    final noLike = await msg_api.fetchFeedUnread(
      context: context(feedServer2.port),
      pick: const msg_api.FeedBadgeSelection(
        reply: true,
        at: true,
        like: false,
        sysMsg: true,
      ),
    );
    expect(noLike.data?.badge, 13, reason: '关掉赞应当正好少 2');
  }, skip: bridgeSkip);
}

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

  /// 表单体解出来的字段。JSON 体的那条别用它——`splitQueryString` 会给出
  /// 一个毫无意义的单键 map，断言会以难以理解的方式失败。
  Map<String, String> get form => Uri.splitQueryString(rawBody);
}
