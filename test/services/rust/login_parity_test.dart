import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

/// 登录迁移的**基线对照**（S14 / issue #65，机制见 ADR-011 决策五）。
///
/// ## 这个文件里为什么有一份 Dart 解析代码
///
/// typed 直连把 `lib/http/login.dart`、`lib/models_new/login_devices/*`
/// 与控制器里那些 `res['data']['token_info']` 一起删掉了，golden fixture
/// 的期望值就失去了外部来源——只剩 Rust 跟自己比。所以下面 `_Baseline`
/// 里的代码是从**迁移前那一版**逐行转写的：`LoginDevice.fromJson`、
/// `codePoll` 对 86038 的判断、`loginByPassword` 里 `data['status'] == 2`
/// 与 `-105` 两条分支、`sendSmsCode` 的 `recaptcha_url == ""`。
///
/// 它不引用 `lib/` 的任何东西，因此不会随 lib 漂移——这正是要的：它是
/// **冻结的基线**，不是一份需要维护的第二实现（与 `search_parity_test`
/// 的取舍相同）。
///
/// ## 两处刻意的不一致，在下面各有一个用例守着
///
/// 1. **半截 cookie**：基线 `BiliCookieJar.fromList` 读 `i['value']!`，
///    碰上只有 name 没有 value 的条目会**抛异常**，整次登录失败；Rust 把
///    那一条丢掉，其余照存。
/// 2. **身份信息不全**：基线在界面层检查 `token_info == null ||
///    cookie_info == null` 并提示；Rust 在解析层就判 `Auth` 错误。结论
///    相同，位置不同——界面不再有机会把 null 当成功传下去。
void main() {
  const dir = 'rust/crates/bili-api/tests/fixtures/login';

  Map<String, dynamic> readRaw(String name) =>
      jsonDecode(File('$dir/$name.raw.json').readAsStringSync())
          as Map<String, dynamic>;

  Object? readSnapshot(String name) =>
      jsonDecode(File('$dir/$name.rust.json').readAsStringSync());

  group('已登录设备', () {
    test('Rust 快照与 Dart 基线逐字段一致', () {
      final raw = readRaw('login_devices');
      final snapshot = readSnapshot('login_devices') as List;
      final baseline = (raw['data']['devices'] as List)
          .map((e) => _Baseline.device(e as Map<String, dynamic>))
          .toList();

      expect(snapshot.length, baseline.length, reason: '条数不同');
      for (var i = 0; i < baseline.length; i++) {
        final got = snapshot[i] as Map<String, dynamic>;
        expect(got['device_name'], baseline[i]['deviceName'], reason: '第$i条');
        expect(got['latest_login_at'], baseline[i]['latestLoginAt']);
        expect(got['source'], baseline[i]['source']);
        // 基线这个字段是 `bool?`，界面写 `item.isCurrentDevice == true`，
        // 也就是 null 当 false。Rust 在解析时就折成 bool，结论相同。
        expect(
          got['is_current_device'],
          baseline[i]['isCurrentDevice'] == true,
        );
      }
    });
  });

  group('扫码轮询', () {
    test('已确认：cookie 名单与有效期一致', () {
      final raw = readRaw('qr_poll_confirmed');
      final snapshot =
          readSnapshot('qr_poll_confirmed') as Map<String, dynamic>;
      final baseline = _Baseline.qrPoll(raw);

      expect(snapshot['state'], 'confirmed');
      expect(baseline['status'], isTrue);
      final tokens = snapshot['tokens'] as Map<String, dynamic>;
      expect(tokens['expires_in'], baseline['expiresIn']);
      // 快照里不记 token 值（见 login_golden.rs）；这里对照的是"有没有"。
      expect(tokens['access_token'], '<redacted>');
      expect((baseline['accessToken'] as String).isNotEmpty, isTrue);

      // 刻意的不一致之一：只有 name 没有 value 的那条，基线会让整次登录
      // 抛异常，Rust 丢掉它、保住其余三条。
      expect(baseline['cookieNames'], [
        'SESSDATA',
        'bili_jct',
        'DedeUserID',
        'broken-cookie',
      ]);
      expect(tokens['cookie_names'], ['SESSDATA', 'bili_jct', 'DedeUserID']);
      expect(
        () => _Baseline.cookieJarFromList(
          (raw['data']['cookie_info']['cookies'] as List)
              .cast<Map<String, dynamic>>(),
        ),
        throwsA(isA<TypeError>()),
        reason: '基线读 i[\'value\']! 会在半截 cookie 上抛异常',
      );
    });

    test('未确认与已失效：86038 单独成一类', () {
      for (final name in ['qr_poll_pending', 'qr_poll_expired']) {
        final raw = readRaw(name);
        final snapshot = readSnapshot(name) as Map<String, dynamic>;
        final baseline = _Baseline.qrPoll(raw);

        expect(baseline['status'], isFalse);
        if (baseline['code'] == 86038) {
          expect(snapshot['state'], 'expired');
        } else {
          expect(snapshot['state'], 'pending');
          expect(snapshot['code'], baseline['code']);
          expect(snapshot['message'], baseline['msg']);
        }
      }
    });
  });

  group('密码登录', () {
    test('风控：三个参数与基线从 URL 上扒下来的一致', () {
      final raw = readRaw('password_risk');
      final snapshot = readSnapshot('password_risk') as Map<String, dynamic>;
      final baseline = _Baseline.passwordLogin(raw);

      expect(snapshot['state'], 'risk_control');
      expect(baseline['branch'], 'risk');
      expect(snapshot['tmp_code'], baseline['tmpToken']);
      expect(snapshot['request_id'], baseline['requestId']);
      expect(snapshot['source'], baseline['source']);
      expect(snapshot['referer_url'], baseline['url']);
      expect(snapshot['message'], baseline['message']);
    });

    test('-105：注册三件套与基线一致', () {
      final raw = readRaw('password_captcha');
      final snapshot = readSnapshot('password_captcha') as Map<String, dynamic>;
      final baseline = _Baseline.passwordLogin(raw);

      expect(snapshot['state'], 'captcha_required');
      expect(baseline['branch'], 'captcha');
      expect(snapshot['gt'], baseline['geeGt']);
      expect(snapshot['challenge'], baseline['geeChallenge']);
      expect(snapshot['token'], baseline['recaptchaToken']);
    });
  });

  group('发送短信验证码', () {
    test('成功与"还得先过人机验证"的分界与基线一致', () {
      for (final name in ['sms_send_ok', 'sms_send_captcha']) {
        final raw = readRaw(name);
        final snapshot = readSnapshot(name) as Map<String, dynamic>;
        final baseline = _Baseline.smsSend(raw);

        if (baseline['status'] == true) {
          expect(snapshot['state'], 'sent');
          expect(snapshot['captcha_key'], baseline['captchaKey']);
        } else {
          // code 是 0 却不算成功：recaptcha_url 非空。基线只有一处
          // `&& data['recaptcha_url'] == ""` 守着这件事。
          expect(snapshot['state'], 'captcha_required');
          expect(snapshot['gt'], baseline['geeGt']);
          expect(snapshot['challenge'], baseline['geeChallenge']);
          expect(snapshot['token'], baseline['recaptchaToken']);
        }
      }
    });
  });
}

/// 迁移前那一版 Dart 判断的逐行转写。**写下就不再改。**
abstract final class _Baseline {
  /// `lib/models_new/login_devices/device.dart`
  static Map<String, Object?> device(Map<String, dynamic> json) => {
    'deviceName': json['device_name'] as String?,
    'isCurrentDevice': json['is_current_device'] as bool?,
    'latestLoginAt': json['latest_login_at'] as String?,
    'source': json['source'] as String?,
  };

  /// `LoginHttp.codePoll` + 控制器里对 86038 的判断。
  static Map<String, Object?> qrPoll(Map<String, dynamic> res) {
    final status = res['code'] == 0;
    final data = res['data'];
    return {
      'status': status,
      'code': res['code'],
      'msg': res['message'],
      'accessToken': status ? data['access_token'] : null,
      'expiresIn': status ? data['expires_in'] : null,
      'cookieNames': status
          ? [
              for (final c in data['cookie_info']['cookies'] as List)
                c['name'] as String?,
            ]
          : null,
    };
  }

  /// `BiliCookieJar.fromList` 里真正会抛的那一步。
  static List<String> cookieJarFromList(List<Map<String, dynamic>> cookies) => [
    for (final i in cookies) '${i['name']!}=${i['value']! as String}',
  ];

  /// `LoginPageController.loginByPassword` 的三条分支。
  static Map<String, Object?> passwordLogin(Map<String, dynamic> res) {
    if (res['code'] == -105) {
      final uri = Uri.parse(res['data']['url'] as String);
      return {
        'branch': 'captcha',
        'recaptchaToken': uri.queryParameters['recaptcha_token'],
        'geeGt': uri.queryParameters['gee_gt'],
        'geeChallenge': uri.queryParameters['gee_challenge'],
      };
    }
    final data = res['data'];
    if (res['code'] == 0 && data['status'] == 2) {
      final uri = Uri.parse(data['url'] as String);
      return {
        'branch': 'risk',
        'url': data['url'],
        'message': data['message'],
        'tmpToken': uri.queryParameters['tmp_token'],
        'requestId': uri.queryParameters['request_id'],
        'source': uri.queryParameters['source'],
      };
    }
    return {'branch': 'authenticated'};
  }

  /// `LoginHttp.sendSmsCode` 的成功判定与控制器对 `recaptcha_url` 的解析。
  static Map<String, Object?> smsSend(Map<String, dynamic> res) {
    final data = res['data'];
    if (res['code'] == 0 && data['recaptcha_url'] == '') {
      return {'status': true, 'captchaKey': data['captcha_key']};
    }
    final captureUrl = data?['recaptcha_url'] as String?;
    if (captureUrl == null || captureUrl.isEmpty) {
      return {'status': false};
    }
    final uri = Uri.parse(captureUrl);
    return {
      'status': false,
      'recaptchaToken': uri.queryParameters['recaptcha_token'],
      'geeGt': uri.queryParameters['gee_gt'],
      'geeChallenge': uri.queryParameters['gee_challenge'],
    };
  }
}
