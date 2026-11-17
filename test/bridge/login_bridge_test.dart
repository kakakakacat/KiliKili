import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:kilikili/rust/api/login.dart' as login_api;
import 'package:kilikili/rust/api/transport.dart';

import 'bridge_env.dart';

/// 登录流程跨 FFI 的集成测试（S14 / #65）。
///
/// 验的是**请求面**：登录接口的响应是凭据，冻结它既危险又证明不了什么；
/// 会错的是签名、字段名、编码，以及——最要紧的——**明文密码有没有被原样
/// 发出去**。Rust 单测钉住构造出来的 `PreparedWrite`，这里钉住真的发出去
/// 的那一个：跨过 FFI、经过 RSA 加密、经过 reqwest 的表单编码之后。
void main() {
  TransportSettings noRetry() => TransportSettings(
    // 这一片全是 Unsafe 定级，本来就不会自动重发；设 0 是为了让桩服务端
    // 的计数不受影响。
    retryCount: 0,
    retryDelayMs: BigInt.zero,
    acceptInvalidCerts: false,
    http1Only: true,
  );

  login_api.LoginContext context(int port, {String? csrf}) =>
      login_api.LoginContext(
        baseUrl: 'http://127.0.0.1:$port',
        csrf: csrf,
        headers: const [('user-agent', 'Dart/3.6 (dart:io)')],
        timeoutMs: BigInt.from(3000),
        transport: noRetry(),
        appkey: 'dfca71928277209b',
        appsec: 'b5475a8825547a4fc26c7d518eaaa02e',
        buvid: 'XY0123456789ABCDEF0123456789ABCDEF',
        userAgent: 'BiliDroid-test',
        statistics: '{"appId":5,"platform":3}',
        traceId: 'trace-id',
        utcOffsetSecs: 8 * 3600,
      );

  /// 起一个按路径回话的桩服务端，并把收到的每条请求记下来。
  Future<(HttpServer, List<_SeenRequest>)> stubServer(
    Map<String, String> bodies,
  ) async {
    final server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
    final seen = <_SeenRequest>[];
    server.listen((request) async {
      final raw = await utf8.decoder.bind(request).join();
      seen.add(
        _SeenRequest(
          method: request.method,
          path: request.uri.path,
          query: request.uri.queryParameters,
          contentType: request.headers.contentType?.mimeType,
          headers: {
            for (final name in ['buvid', 'app-key', 'user-agent', 'referer'])
              name: request.headers.value(name),
          },
          rawBody: raw,
          form: raw.isEmpty ? const {} : Uri.splitQueryString(raw),
        ),
      );
      request.response.headers.contentType = ContentType.json;
      request.response.write(
        bodies[request.uri.path] ?? '{"code":0,"message":"0","ttl":1}',
      );
      await request.response.close();
    });
    return (server, seen);
  }

  setUpAll(() async {
    if (bridgeTestsRequested) await initRustBridge();
  });

  tearDownAll(() async {
    if (bridgeTestsRequested) await closeRustBridge();
  });

  test('短信验证码：签名齐全，没给的人机验证字段整条不出现', () async {
    final (server, seen) = await stubServer({
      '/x/passport-login/sms/send': '{"code":0,"message":"0","data":{"captcha_key":"ck","recaptcha_url":""}}',
    });
    addTearDown(() => server.close(force: true));

    final res = await login_api.loginSendSmsCode(
      context: context(server.port),
      cid: '86',
      tel: '13800000000',
      captcha: const login_api.CaptchaSolutionData(challenge: 'ch'),
    );

    expect(res.errorKind, isNull, reason: res.errorDetail);
    expect(res.data!.status, login_api.SmsSendStatus.sent);
    expect(res.data!.captchaKey, 'ck');

    final sent = seen.single;
    expect(sent.method, 'POST');
    expect(sent.contentType, 'application/x-www-form-urlencoded');
    expect(sent.form['tel'], '13800000000');
    expect(sent.form['cid'], '86');
    expect(sent.form['appkey'], 'dfca71928277209b');
    expect(sent.form['sign'], isNotEmpty);
    expect(sent.form['login_session_id'], hasLength(32));
    expect(sent.form['gee_challenge'], 'ch');
    // 没给的三个字段必须整条不出现，而不是以空串出现。
    expect(sent.form.containsKey('gee_seccode'), isFalse);
    expect(sent.form.containsKey('gee_validate'), isFalse);
    expect(sent.form.containsKey('recaptcha_token'), isFalse);
    // 登录专用头（基线 `LoginHttp.headers`）。
    expect(sent.headers['buvid'], 'XY0123456789ABCDEF0123456789ABCDEF');
    expect(sent.headers['app-key'], 'android_hd');
    expect(sent.headers['user-agent'], 'BiliDroid-test');
  }, skip: bridgeSkip);

  test('密码登录：明文密码不出现在请求里，dt 已百分号编码', () async {
    const plaintext = 'hunter2-plaintext';
    final (server, seen) = await stubServer({
      // 1024 位测试公钥，与任何线上密钥无关。
      '/x/passport-login/web/key':
          '{"code":0,"message":"0","data":{"hash":"salt-","key":"$_testPublicKeyJson"}}',
      '/x/passport-login/oauth2/login': '{"code":-105,"message":"需要验证码","data":{"url":"https://x?recaptcha_token=tk&gee_gt=gt&gee_challenge=ch"}}',
    });
    addTearDown(() => server.close(force: true));

    final res = await login_api.loginByPassword(
      context: context(server.port),
      username: 'someone',
      password: plaintext,
      captcha: const login_api.CaptchaSolutionData(),
    );

    expect(res.errorKind, isNull, reason: res.errorDetail);
    expect(res.data!.status, login_api.PasswordLoginStatus.captchaRequired);
    expect(res.data!.captcha!.gt, 'gt');

    // 先取公钥，再登录：公钥与盐全程不出 Rust。
    expect(seen.map((e) => e.path), [
      '/x/passport-login/web/key',
      '/x/passport-login/oauth2/login',
    ]);
    final login = seen.last;
    for (final request in seen) {
      expect(
        request.rawBody.contains(plaintext),
        isFalse,
        reason: '明文密码绝不能出现在任何一条请求里',
      );
      expect(request.query.values.join().contains(plaintext), isFalse);
    }
    expect(base64.decode(login.form['password']!), isNotEmpty);
    // dt 在基线里被 encodeComponent 过一次，表单再编码一次：解一层之后
    // 仍是合法 base64。少这一层就等于改了请求面。
    final dt = login.form['dt']!;
    expect(dt, contains('%'));
    expect(base64.decode(Uri.decodeComponent(dt)), isNotEmpty);
    expect(login.form['device_id'], hasLength(64));
    expect(login.form['bili_local_id'], login.form['device_id']);
  }, skip: bridgeSkip);

  test('登出：csrf 放在 biliCSRF 字段里', () async {
    final (server, seen) = await stubServer(const {});
    addTearDown(() => server.close(force: true));

    final res = await login_api.loginLogout(
      context: context(server.port, csrf: 'fake-csrf'),
    );

    expect(res.errorKind, isNull, reason: res.errorDetail);
    expect(seen.single.path, '/login/exit/v2');
    expect(seen.single.form['biliCSRF'], 'fake-csrf');
    // 这条接口不认 `csrf` 这个键名，带上只会多一个无用字段。
    expect(seen.single.form.containsKey('csrf'), isFalse);
  }, skip: bridgeSkip);

  test('已登录设备：csrf 与 access_key 在查询串上并参与签名', () async {
    final (server, seen) = await stubServer({
      '/x/safecenter/user_login_devices': '{"code":0,"message":"0","data":{"devices":[{"device_name":"d","is_current_device":true}]}}',
    });
    addTearDown(() => server.close(force: true));

    final res = await login_api.loginDevices(
      context: context(server.port, csrf: 'fake-csrf'),
      accessKey: 'fake-access-key',
    );

    expect(res.errorKind, isNull, reason: res.errorDetail);
    expect(res.data!.single.isCurrentDevice, isTrue);
    final request = seen.single;
    expect(request.method, 'GET');
    expect(request.query['csrf'], 'fake-csrf');
    expect(request.query['access_key'], 'fake-access-key');
    expect(request.query['sign'], isNotEmpty);
  }, skip: bridgeSkip);

  test('风控短信：Referer 用服务端给的那个地址', () async {
    final (server, seen) = await stubServer({
      '/x/safecenter/common/sms/send':
          '{"code":0,"message":"0","data":{"captcha_key":"safe-ck"}}',
    });
    addTearDown(() => server.close(force: true));

    const referer = 'https://passport.bilibili.com/h5-app/passport/risk/verify';
    final res = await login_api.loginSafeCenterSms(
      context: context(server.port),
      verification: const login_api.RiskVerificationData(
        refererUrl: referer,
        tmpCode: 'tmp',
        requestId: 'rid',
        source: 'risk',
        message: '',
      ),
      captcha: const login_api.CaptchaSolutionData(),
    );

    expect(res.errorKind, isNull, reason: res.errorDetail);
    expect(res.data, 'safe-ck');
    expect(seen.single.headers['referer'], referer);
    expect(seen.single.form['tmp_code'], 'tmp');
    expect(seen.single.form['sms_type'], 'loginTelCheck');
  }, skip: bridgeSkip);
}

/// 测试用 2048 位 RSA 公钥（SPKI PEM，JSON 里换行写成 \n）。
///
/// 与任何线上密钥无关：生成于本测试，私钥随手丢弃——这里只需要"能加密"。
///
/// 必须是 2048 位：`parse_public_key` 自 issue #156 起拒绝更短的模数，
/// 原先那把 1024 位的会在"公钥无法解析"下失败。
const String _testPublicKeyJson =
    '-----BEGIN PUBLIC KEY-----\\n'
    'MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAuLA4LxGL6tJOSDBgRh2p\\n'
    'UdYjs9amP4EiFBeKAxtyEDhahePDUoj085KG3FhIE+3hIUTBARhyrDzyMH37q470\\n'
    'IqkrPtud4TMD1uWHymTIwZeDOajqFr0a/ws56sJB5fgRQQUBBhRz1hrY3e9qUgcq\\n'
    'YRFejYZl3/3bCpmp5uIGGvR1anlrfcRKUbHqRtXgykX4PCffAH4nsE+uX+eqhNVN\\n'
    'TWB5ADtjJCkzNf55zALa0SpkuPGzWzHoZYgEqQxzCcI08TifEU203iAVO5bp3GOS\\n'
    's7uFUUrs+k0p1Jpc6LtUWdwZA15GWtbtRht7kD/CNfVrUmliJWnp1wnqpzqwlCKB\\n'
    'PwIDAQAB\\n'
    '-----END PUBLIC KEY-----';

class _SeenRequest {
  _SeenRequest({
    required this.method,
    required this.path,
    required this.query,
    required this.contentType,
    required this.headers,
    required this.rawBody,
    required this.form,
  });

  final String method;
  final String path;
  final Map<String, String> query;
  final String? contentType;
  final Map<String, String?> headers;
  final String rawBody;
  final Map<String, String> form;
}
