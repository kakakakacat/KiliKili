// 生成 playurl 请求 golden fixture：从 Dart 基线产出期望的查询串与签名，
// 供 Rust 侧对照（rust/crates/bili-api/tests/playurl_golden.rs）。
//
// 本文件**逐字复制**了迁移前 lib/http/video.dart::videoUrl 的参数组装、
// lib/models/common/video/video_type.dart 的接口路径，以及
// lib/utils/wbi_sign.dart 的签名算法。
//
// 2026-11-11（issue #79）：`videoUrl` 随取流迁成 typed 直连而删除，所以
// 这份拷贝不再有「跟着上游一起变」的对照对象——它从此是**冻结的基线
// 快照**。仍然保留，因为它锁的是**请求面**（查询串与签名），而请求面
// 正是最容易在迁移中悄悄改掉、又最难从响应里看出来的部分：参数少一个、
// 顺序换一下，服务端照样返回 200，只是风控画像变了。
//
// 上游若改了取流的参数组装，这里不会自动发现——那要靠同步上游时比对
// `docs/architecture/protocol-changelog.md`。
//
// 随机的 dm_img_str / dm_cover_img_str 在这里被固定成占位值——随机值无法
// 对照固定期望。它们的**形状**（长度区间、字符集）由 bili-protocol 的
// risk.rs 单元测试锁定。
//
// 用法: dart tool/fixture-gen/playurl_fixture.dart \
//         > rust/crates/bili-api/tests/fixtures/playurl.json
import 'dart:convert';

import 'package:crypto/crypto.dart';

// 与 bili-protocol 的 PROTOCOL_VERSION 对应（ADR-008）。
const protocolVersion = 1;

final RegExp _chrFilter = RegExp(r"[!\'\(\)\*]");
const _mixinKeyEncTab = <int>[
  46,
  47,
  18,
  2,
  53,
  8,
  23,
  32,
  15,
  50,
  10,
  31,
  58,
  3,
  45,
  35,
  27,
  43,
  5,
  49,
  33,
  9,
  42,
  19,
  29,
  28,
  14,
  39,
  12,
  38,
  41,
  13,
];

String getMixinKey(String orig) =>
    String.fromCharCodes(_mixinKeyEncTab.map(orig.codeUnitAt));

String encWbi(Map<String, Object> params, String mixinKey) {
  final keys = params.keys.toList()..sort();
  final queryStr = keys
      .map(
        (i) =>
            '${Uri.encodeComponent(i)}=${Uri.encodeComponent(params[i].toString().replaceAll(_chrFilter, ''))}',
      )
      .join('&');
  return md5.convert(utf8.encode(queryStr + mixinKey)).toString();
}

// lib/http/api.dart
const ugcUrl = '/x/player/wbi/playurl';
const pgcUrl = '/pgc/player/web/v2/playurl';
const pugvUrl = '/pugv/player/web/playurl';
const ogvUrl = '/ogv/player/playview';

// lib/models/common/video/video_type.dart
const _apis = {
  'ugc': (api: ugcUrl, fallbackApi: ugcUrl, bizType: null),
  'pgc': (api: ogvUrl, fallbackApi: pgcUrl, bizType: 'ogv'),
  'pugv': (api: ogvUrl, fallbackApi: pugvUrl, bizType: 'pugv'),
};

/// lib/http/video.dart::videoUrl 的参数组装部分，逐字复制。
Map<String, Object> buildParams({
  int? avid,
  String? bvid,
  int? epid,
  int? seasonId,
  required int cid,
  required int qn,
  required bool tryLook,
  required String videoType,
  String? language,
  bool voiceBalance = false,
  bool fallback = false,
  required String dmImgStr,
  required String dmCoverImgStr,
}) {
  final bizType = _apis[videoType]!.bizType;
  return <String, Object>{
    'avid': ?avid,
    'bvid': ?bvid,
    'ep_id': ?epid,
    'season_id': ?seasonId,
    'cid': cid,
    'qn': qn,
    'fnval': 4048,
    'fourk': 1,
    'fnver': 0,
    'voice_balance': voiceBalance ? 1 : 0,
    'gaia_source': 'pre-load',
    'isGaiaAvoided': true,
    'web_location': 1315873,
    if (videoType != 'ugc' && !fallback) ...{
      'biz_type': ?bizType,
      'scene': 'normal',
      'from_client': 'BROWSER',
      'drm_tech_type': 0,
      'app_id': 100,
    },
    if (tryLook) 'try_look': 1,
    'dm_img_list': '[]',
    'dm_img_str': dmImgStr,
    'dm_cover_img_str': dmCoverImgStr,
    'dm_img_inter': '{"ds":[],"wh":[0,0,0],"of":[0,0,0]}',
    'cur_language': ?language,
  };
}

void main() {
  // 占位 key，与 WBI fixture 同源，非真实凭据。
  const imgKey = '7cd084941338484aae1ad9425b84077c';
  const subKey = '4932caff0ff746eab6f01bf08b70ac45';
  final mixinKey = getMixinKey(imgKey + subKey);

  // 固定的风控 token：形状取自基线（去掉末尾两字符的 base64），
  // 取值固定以便对照签名。
  const dmImgStr = 'QUJDREVGR0hJSktMTU5PUFFSU1RVVld';
  const dmCoverImgStr = 'YWJjZGVmZ2hpamtsbW5vcHFyc3R1dnd4eXowMTIzNDU2Nzg5';
  const wts = 1702204169;

  final cases = <Map<String, Object?>>[
    {
      'name': 'ugc_minimal',
      'input': {
        'bvid': 'BV1L94y1H7CV',
        'cid': 1304392106,
        'qn': 80,
        'video_type': 'ugc',
        'try_look': false,
        'voice_balance': false,
        'fallback': false,
      },
    },
    {
      'name': 'ugc_try_look_with_avid',
      'input': {
        'avid': 844620406,
        'bvid': 'BV1L94y1H7CV',
        'cid': 1304392106,
        'qn': 112,
        'video_type': 'ugc',
        'try_look': true,
        'voice_balance': true,
        'fallback': false,
      },
    },
    {
      'name': 'pgc_primary',
      'input': {
        'ep_id': 374717,
        'season_id': 28843,
        'cid': 221404201,
        'qn': 80,
        'video_type': 'pgc',
        'try_look': false,
        'voice_balance': false,
        'fallback': false,
      },
    },
    {
      'name': 'pgc_fallback_drops_biz_group',
      'input': {
        'ep_id': 374717,
        'season_id': 28843,
        'cid': 221404201,
        'qn': 80,
        'video_type': 'pgc',
        'try_look': false,
        'voice_balance': false,
        'fallback': true,
      },
    },
    {
      'name': 'pgc_with_language',
      'input': {
        'ep_id': 374717,
        'cid': 221404201,
        'qn': 80,
        'video_type': 'pgc',
        'try_look': true,
        'voice_balance': false,
        'fallback': false,
        'language': 'zh',
      },
    },
    {
      'name': 'pugv_primary',
      'input': {
        'season_id': 1234,
        'cid': 999888,
        'qn': 64,
        'video_type': 'pugv',
        'try_look': false,
        'voice_balance': false,
        'fallback': false,
      },
    },
    {
      'name': 'pugv_fallback',
      'input': {
        'ep_id': 5678,
        'cid': 999888,
        'qn': 64,
        'video_type': 'pugv',
        'try_look': false,
        'voice_balance': false,
        'fallback': true,
      },
    },
  ];

  final out = <String, Object>{
    'protocol_version': protocolVersion,
    'generated_from': 'lib/http/video.dart::videoUrl + lib/utils/wbi_sign.dart',
    'img_key': imgKey,
    'sub_key': subKey,
    'mixin_key': mixinKey,
    'wts': wts,
    'dm_img_str': dmImgStr,
    'dm_cover_img_str': dmCoverImgStr,
    'cases': [
      for (final c in cases)
        () {
          final input = c['input']! as Map<String, Object?>;
          final videoType = input['video_type']! as String;
          final fallback = input['fallback']! as bool;
          final params = buildParams(
            avid: input['avid'] as int?,
            bvid: input['bvid'] as String?,
            epid: input['ep_id'] as int?,
            seasonId: input['season_id'] as int?,
            cid: input['cid']! as int,
            qn: input['qn']! as int,
            tryLook: input['try_look']! as bool,
            videoType: videoType,
            language: input['language'] as String?,
            voiceBalance: input['voice_balance']! as bool,
            fallback: fallback,
            dmImgStr: dmImgStr,
            dmCoverImgStr: dmCoverImgStr,
          );
          params['wts'] = wts;
          final wRid = encWbi(params, mixinKey);
          final sortedKeys = params.keys.toList()..sort();
          return {
            'name': c['name'],
            'input': input,
            'path': fallback
                ? _apis[videoType]!.fallbackApi
                : _apis[videoType]!.api,
            // 按 key 升序 + 末尾 w_rid：与 Rust 的 PreparedRequest.query
            // 同一口径。签名覆盖的是排序后的串，顺序是签名的一部分。
            'query': [
              for (final k in sortedKeys) [k, params[k].toString()],
              ['w_rid', wRid],
            ],
          };
        }(),
    ],
  };
  print(const JsonEncoder.withIndent('  ').convert(out));
}
