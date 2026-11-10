// 生成 WBI golden fixture：从 Dart 基线实现产出期望值，供 Rust 侧对照。
//
// 本文件**逐字复制** lib/utils/wbi_sign.dart 的纯算法部分（getMixinKey、
// encWbi），不含 Hive/网络依赖。复制而非 import，是为了让 fixture 反映
// 基线此刻的真实行为；若上游改了算法，这里会与之产生差异，从而暴露出来。
//
// 用法: dart tool/fixture-gen/wbi_fixture.dart > rust/crates/bili-protocol/tests/fixtures/wbi.json
import 'dart:convert';
import 'package:crypto/crypto.dart';

final RegExp _chrFilter = RegExp(r"[!\'\(\)\*]");
const _mixinKeyEncTab = <int>[
  46, 47, 18, 2, 53, 8, 23, 32, 15, 50, 10, 31, 58, 3, 45, 35,
  27, 43, 5, 49, 33, 9, 42, 19, 29, 28, 14, 39, 12, 38, 41, 13,
];

String getMixinKey(String orig) =>
    String.fromCharCodes(_mixinKeyEncTab.map(orig.codeUnitAt));

String encWbi(Map<String, Object> params, String mixinKey) {
  final keys = params.keys.toList()..sort();
  final queryStr = keys
      .map((i) =>
          '${Uri.encodeComponent(i)}=${Uri.encodeComponent(params[i].toString().replaceAll(_chrFilter, ''))}')
      .join('&');
  return md5.convert(utf8.encode(queryStr + mixinKey)).toString();
}

void main() {
  // 固定输入。img/sub key 为符合真实长度(32)的占位值，非真实凭据。
  const imgKey = '7cd084941338484aae1ad9425b84077c';
  const subKey = '4932caff0ff746eab6f01bf08b70ac45';
  final mixinKey = getMixinKey(imgKey + subKey);

  final cases = <Map<String, Object>>[
    {'name': 'single_bvid', 'params': {'bvid': 'BV1L94y1H7CV', 'wts': 1702204169}},
    {'name': 'multi_param_sorted', 'params': {'qn': 80, 'bvid': 'BV1L94y1H7CV', 'cid': 1304392106, 'wts': 1702204169}},
    {'name': 'chars_stripped', 'params': {'q': "a!b'c(d)e*f", 'wts': 1702204169}},
    {'name': 'needs_percent_encoding', 'params': {'q': '测试 & =+#', 'wts': 1702204169}},
    {'name': 'empty_value', 'params': {'q': '', 'wts': 1702204169}},
  ];

  final out = {
    'img_key': imgKey,
    'sub_key': subKey,
    'mixin_key': mixinKey,
    'cases': [
      for (final c in cases)
        {
          'name': c['name'],
          'params': c['params'],
          'w_rid': encWbi(c['params'] as Map<String, Object>, mixinKey),
        }
    ],
  };
  print(const JsonEncoder.withIndent('  ').convert(out));
}
