// 生成 APP 签名 golden fixture：从 Dart 基线实现产出期望值，供 Rust 侧对照。
//
// 本文件**逐字复制** lib/utils/app_sign.dart 的算法（appSign 与它私有的
// _makeQueryFromParametersDefault），只把「就地取系统时间」改成由调用方
// 传入 ts——ADR-008 决策五要求请求构造是纯函数，否则 fixture 无从对照。
// 复制而非 import，是为了让 fixture 反映基线此刻的真实行为；若上游改了
// 算法，这里会与之产生差异，从而暴露出来（ADR-008 决策三）。
//
// 用法: dart tool/fixture-gen/app_sign_fixture.dart > rust/crates/bili-protocol/tests/fixtures/app_sign.json
import 'dart:convert' show JsonEncoder, utf8;

import 'package:crypto/crypto.dart' show md5;

// ---- 以下为 lib/utils/app_sign.dart 的逐字复制（ts 改为入参）----

void appSign(
  Map<String, dynamic> params,
  String appkey,
  String appsec,
  int ts,
) {
  params['appkey'] = appkey;
  params['ts'] = ts.toString();
  final sorted = params.entries.toList()
    ..sort((a, b) => a.key.compareTo(b.key));
  params['sign'] = md5
      .convert(utf8.encode(_makeQueryFromParametersDefault(sorted) + appsec))
      .toString();
}

String _makeQueryFromParametersDefault(
  List<MapEntry<String, dynamic>> queryParameters,
) {
  final result = StringBuffer();
  var separator = '';

  void writeParameter(String key, String? value) {
    result.write(separator);
    separator = '&';
    result.write(Uri.encodeComponent(key));
    if (value != null) {
      result
        ..write('=')
        ..write(Uri.encodeComponent(value));
    }
  }

  for (final i in queryParameters) {
    if (i.value case final Iterable<String> values) {
      for (final String value in values) {
        writeParameter(i.key, value);
      }
    } else {
      writeParameter(i.key, i.value?.toString());
    }
  }
  return result.toString();
}

// ---- 复制结束 ----

void main() {
  // 基线 Constants.appKey / appSec。移动端 Android 的公开 appkey，
  // 不是用户凭据。
  const appkey = 'dfca71928277209b';
  const appsec = 'b5475a8825547a4fc26c7d518eaaa02e';
  const ts = 1700000000;

  // 取材自基线 LiveHttp 里实际发出的那几组参数：直播分区列表（最短）、
  // 首页推荐（含 statistics 这段 JSON）、以及两组专门压编码口径的。
  final cases = <Map<String, Object>>[
    {
      'name': 'area_list',
      'params': {
        'actionKey': 'appkey',
        'build': 8430300,
        'channel': 'master',
        'version': '8.43.0',
        'c_locale': 'zh_CN',
        'device': 'android',
        'disable_rcmd': 0,
        'mobi_app': 'android',
        'platform': 'android',
        's_locale': 'zh_CN',
        'statistics': '{"appId":1,"platform":3,"version":"8.43.0","abtest":""}',
      },
    },
    {
      'name': 'feed_index',
      'params': {
        'channel': 'master',
        'actionKey': 'appkey',
        'build': 8430300,
        'version': '8.43.0',
        'c_locale': 'zh_CN',
        'device': 'android',
        'device_name': 'android',
        'device_type': 0,
        'fnval': 912,
        'disable_rcmd': 0,
        'https_url_req': 1,
        'mobi_app': 'android',
        'network': 'wifi',
        'page': 1,
        'platform': 'android',
        's_locale': 'zh_CN',
        'scale': 2,
        'statistics': '{"appId":1,"platform":3,"version":"8.43.0","abtest":""}',
      },
    },
    {
      // WBI 会把 !'()* 从值里剔除，APP 签名不会。两套口径一旦被合并，
      // 这条的期望值就对不上了。
      'name': 'chars_kept',
      'params': {'q': "a!b'c(d)e*f"},
    },
    {
      'name': 'needs_percent_encoding',
      'params': {'q': '测试 & =+#'},
    },
    {
      'name': 'empty_value',
      'params': {'q': ''},
    },
  ];

  final out = {
    // 与 bili-protocol 的 PROTOCOL_VERSION 对应（ADR-008 决策二）。
    'protocol_version': 1,
    'appkey': appkey,
    'appsec': appsec,
    'ts': ts,
    'cases': [
      for (final c in cases)
        () {
          final params = Map<String, dynamic>.from(
            c['params']! as Map<String, Object>,
          );
          appSign(params, appkey, appsec, ts);
          return {
            'name': c['name'],
            'params': c['params'],
            // 整组已签名参数，按 Rust 侧的产出顺序（key 升序 + sign 垫底）
            // 给出，使「签名覆盖了什么」也一并被锁死。
            'signed': [
              for (final k
                  in (params.keys.toList()..sort())
                    ..remove('sign')
                    ..add('sign'))
                [k, params[k].toString()],
            ],
            'sign': params['sign'],
          };
        }(),
    ],
  };
  print(const JsonEncoder.withIndent('  ').convert(out));
}
