import 'dart:convert';
import 'dart:io';

import 'package:crypto/crypto.dart';
import 'package:flutter_test/flutter_test.dart';

/// 直播取流迁移的**基线对照**（S11a 第二批 / issue #61，机制见 ADR-011
/// 决策五与其中 S9 补充二）。
///
/// 被取代的 `fromJson` 随本 PR 从 `lib/` 删除，借不到活着的基线，所以
/// 下面 `_Baseline` 里是迁移前那一版的**逐字拷贝**：
///
/// - `lib/models_new/live/live_room_play_info/{data,playurl_info,playurl,
///   stream,format,codec,url_info}.dart`
/// - `lib/models/common/video/live_quality.dart` 的静态表与三个构造
///   （`fromJson` / `fromCode` / `fromFormat`）
///
/// 它不引用 `lib/` 的任何东西，写下之后不再改——和 `*.rust.json` 快照
/// 一样是迁移当时行为的冻结证据。两者由不同实现产出，任何一边变了都
/// 会红。
///
/// ## 对照的是「Rust + 适配层」整体
///
/// 画质的名字由两边合作产出：Rust 只认响应里的 `g_qn_desc`，命中不了
/// 就给 `desc: null`，由 `lib/services/rust/live_playurl_compat.dart`
/// 的 `toLiveQuality` 查客户端静态表补上（理由见那个文件）。所以下面
/// 比的是 `_applyCompat(rust项)` 与基线 `fromFormat` 的结果——拆开比
/// 任何一半都证明不了迁移没改行为。
///
/// `_applyCompat` 是 `toLiveQuality` 的逐字拷贝，同样不 import `lib/`。
///
/// ## 两处**有意**与基线不同，这里断言的是差异本身
///
/// 1. **`playurl_info.playurl` 两层空壳摊平成 `streams`。** 基线要写
///    `response.playurlInfo?.playurl` 再判 null；摊平后「没有 playurl」
///    与「stream 是空数组」都表现为空列表。见 `room_play_info_offline`
///    那个用例。
/// 2. **`desc` 的分工**：见上一节。
void main() {
  const dir = 'rust/crates/bili-api/tests/fixtures/live';

  Map<String, dynamic> readRaw(String name) =>
      jsonDecode(File('$dir/$name.raw.json').readAsStringSync())
          as Map<String, dynamic>;

  dynamic readSnapshot(String name) =>
      jsonDecode(File('$dir/$name.rust.json').readAsStringSync());

  /// 把 Rust 的一档画质按适配层的规则补全，再和基线那一档逐字段比。
  void expectQn(
    Map<String, dynamic> rust,
    _Qn baseline,
    String where,
  ) {
    final applied = _applyCompat(rust);
    expect(applied.code, baseline.code, reason: '$where: code');
    expect(applied.desc, baseline.desc, reason: '$where: desc');
  }

  void expectCodec(
    Map<String, dynamic> rust,
    _CodecItem baseline,
    String where,
  ) {
    expect(
      rust['codec_name'],
      baseline.codecName,
      reason: '$where: codec_name',
    );
    expect(rust['base_url'], baseline.baseUrl, reason: '$where: base_url');

    expectQn(
      rust['current_qn'] as Map<String, dynamic>,
      baseline.currentQn,
      '$where: current_qn',
    );

    final rustAccept = rust['accept_qn'] as List;
    expect(
      rustAccept.length,
      baseline.acceptQn.length,
      reason: '$where: accept_qn 档数',
    );
    for (var i = 0; i < rustAccept.length; i++) {
      expectQn(
        rustAccept[i] as Map<String, dynamic>,
        baseline.acceptQn[i],
        '$where: accept_qn[$i]',
      );
    }

    final rustUrls = rust['url_info'] as List;
    expect(
      rustUrls.length,
      baseline.urlInfo.length,
      reason: '$where: url_info 条数',
    );
    for (var i = 0; i < rustUrls.length; i++) {
      final u = rustUrls[i] as Map<String, dynamic>;
      expect(u['host'], baseline.urlInfo[i].host, reason: '$where: host[$i]');
      expect(
        u['extra'],
        baseline.urlInfo[i].extra,
        reason: '$where: extra[$i]',
      );
    }
  }

  test('取流：整棵 stream 树与基线逐字段一致', () {
    final raw = readRaw('room_play_info');
    final baseline = _Baseline.roomPlayInfo(raw['data'] as Map<String, dynamic>);
    final rust = readSnapshot('room_play_info') as Map<String, dynamic>;

    expect(rust['room_id'], baseline.roomId, reason: 'room_id');
    expect(rust['short_id'], baseline.shortId, reason: 'short_id');
    expect(rust['uid'], baseline.uid, reason: 'uid');
    expect(rust['is_portrait'], baseline.isPortrait, reason: 'is_portrait');
    expect(rust['live_status'], baseline.liveStatus, reason: 'live_status');
    expect(rust['live_time'], baseline.liveTime, reason: 'live_time');

    // 差异 1：基线是 playurlInfo?.playurl?.stream，Rust 摊平成 streams。
    final baselineStreams = baseline.playurlInfo?.playurl?.stream ?? const [];
    final rustStreams = rust['streams'] as List;
    expect(rustStreams.length, baselineStreams.length, reason: 'stream 条数');
    expect(rustStreams, isNotEmpty, reason: '这份样本应当有流');

    for (var i = 0; i < rustStreams.length; i++) {
      final rs = rustStreams[i] as Map<String, dynamic>;
      final bs = baselineStreams[i];
      expect(
        rs['protocol_name'],
        bs.protocolName,
        reason: 'stream[$i]: protocol_name',
      );

      final rustFormats = rs['format'] as List;
      expect(
        rustFormats.length,
        bs.format.length,
        reason: 'stream[$i]: format 条数',
      );

      for (var j = 0; j < rustFormats.length; j++) {
        final rf = rustFormats[j] as Map<String, dynamic>;
        final bf = bs.format[j];
        expect(
          rf['format_name'],
          bf.formatName,
          reason: 'stream[$i].format[$j]: format_name',
        );

        final rustCodecs = rf['codec'] as List;
        expect(
          rustCodecs.length,
          bf.codec.length,
          reason: 'stream[$i].format[$j]: codec 条数',
        );

        for (var k = 0; k < rustCodecs.length; k++) {
          expectCodec(
            rustCodecs[k] as Map<String, dynamic>,
            bf.codec[k],
            'stream[$i].format[$j].codec[$k]',
          );
        }
      }
    }
  });

  test('取流：g_qn_desc 的四种情况都与基线一致', () {
    final raw = readRaw('room_play_info');
    final baseline = _Baseline.roomPlayInfo(raw['data'] as Map<String, dynamic>);
    final rust = readSnapshot('room_play_info') as Map<String, dynamic>;

    // 这份样本刻意覆盖了四种：
    //   10000 在 g_qn_desc 里，有 desc        → 用表里的「原画」
    //   150   不在 g_qn_desc 里               → 静态表补「高清」
    //   7777  在表里但那项缺 desc             → 退到 qn 的十进制串「7777」
    //   30000 不在表里，静态表里有            → 补「杜比」
    final flvCodec =
        ((rust['streams'] as List)[0]
                as Map<String, dynamic>)['format']
            as List;
    final avc =
        ((flvCodec[0] as Map<String, dynamic>)['codec'] as List)[0]
            as Map<String, dynamic>;
    final accept = avc['accept_qn'] as List;

    final baselineAvc = baseline
        .playurlInfo!
        .playurl!
        .stream[0]
        .format[0]
        .codec[0];

    expect(
      _applyCompat(accept[0] as Map<String, dynamic>).desc,
      '原画',
      reason: '命中 g_qn_desc',
    );
    expect(
      _applyCompat(accept[3] as Map<String, dynamic>).desc,
      '高清',
      reason: '未命中 g_qn_desc，由静态表补',
    );
    // 和基线对齐：基线此处走 fromCode，结果同样是「高清」。
    expect(baselineAvc.acceptQn[3].desc, '高清');

    final hevc =
        (((rust['streams'] as List)[1] as Map<String, dynamic>)['format']
                as List)[1]
            as Map<String, dynamic>;
    final hevcCodec = (hevc['codec'] as List)[0] as Map<String, dynamic>;
    final hevcAccept = hevcCodec['accept_qn'] as List;

    expect(
      _applyCompat(hevcAccept[0] as Map<String, dynamic>).desc,
      '7777',
      reason: 'g_qn_desc 里有这一项但缺 desc，退到 qn 的十进制串',
    );
    expect(
      _applyCompat(hevcAccept[1] as Map<String, dynamic>).desc,
      '杜比',
      reason: '未命中 g_qn_desc，由静态表补',
    );

    final baselineHevc = baseline
        .playurlInfo!
        .playurl!
        .stream[1]
        .format[1]
        .codec[0];
    expect(baselineHevc.acceptQn[0].desc, '7777');
    expect(baselineHevc.acceptQn[1].desc, '杜比');
  });

  test('取流请求：参数集与 w_rid 和基线 WbiSign 一致', () {
    final cases =
        jsonDecode(
              File('$dir/room_play_info_request.rust.json').readAsStringSync(),
            )
            as List;

    // 两个用例与 Rust 侧 live_golden.rs 里的一一对应。
    final baselines = <Map<String, Object>>[
      _Baseline.liveRoomInfoParams(roomId: 100001, qn: 10000, wts: 1762800000),
      _Baseline.liveRoomInfoParams(
        roomId: 100002,
        onlyAudio: true,
        wts: 1762800000,
      ),
    ];

    expect(cases.length, baselines.length);

    for (var i = 0; i < cases.length; i++) {
      final c = cases[i] as Map<String, dynamic>;
      final where = c['case'];
      final rustQuery = <String, String>{
        for (final e in c['query'] as List) (e as List)[0] as String: e[1],
      };
      final baseline = baselines[i];

      expect(
        c['path'],
        '/xlive/web-room/v2/index/getRoomPlayInfo',
        reason: '$where: path',
      );

      // 1. 参数集：键一致，且 Rust 的字符串等于基线那个值的 toString()。
      //    这条钉住的是 `web_location` —— 基线写的是 double 字面量
      //    444.8，Rust 侧是字符串 "444.8"，两者必须逐字相同。
      final rustKeys = rustQuery.keys.where((k) => k != 'w_rid').toSet();
      expect(rustKeys, baseline.keys.toSet(), reason: '$where: 参数键集合');
      for (final entry in baseline.entries) {
        expect(
          rustQuery[entry.key],
          entry.value.toString(),
          reason: '$where: ${entry.key} 的值',
        );
      }

      // 2. w_rid：用基线 encWbi 的逐字拷贝独立算一遍。
      expect(
        _Baseline.wRid(baseline, c['mixin_key'] as String),
        rustQuery['w_rid'],
        reason: '$where: w_rid',
      );
    }
  });

  test('取流：未开播时基线的 playurl 为 null，Rust 给空 streams', () {
    final raw = readRaw('room_play_info_offline');
    final baseline = _Baseline.roomPlayInfo(raw['data'] as Map<String, dynamic>);
    final rust = readSnapshot('room_play_info_offline') as Map<String, dynamic>;

    expect(rust['room_id'], baseline.roomId);
    expect(rust['live_status'], baseline.liveStatus);
    expect(rust['is_portrait'], baseline.isPortrait);
    expect(rust['live_time'], baseline.liveTime);

    // 差异 1：基线这里是 `playurlInfo!.playurl == null`，调用点据此报
    // 「无法获取播放地址」；Rust 摊平成空列表，调用点判一次空即可。
    expect(baseline.playurlInfo, isNotNull);
    expect(baseline.playurlInfo!.playurl, isNull);
    expect(rust['streams'], isEmpty);
  });
}

// ---------------------------------------------------------------- 基线拷贝

/// `LiveQuality` 的迁移前实现，只保留取流用得到的部分。
class _Qn {
  final int code;
  final String desc;

  const _Qn(this.code, this.desc);

  _Qn.fromId(this.code) : desc = '$code';

  /// `LiveQuality.fromJson`：`qn` 缺失时 -1，`desc` 缺失时退到
  /// `json['qn'].toString()`——注意 `qn` 也缺时 Dart 算的是
  /// `null.toString()`，即字面量 `'null'`。
  factory _Qn.fromJson(Map<String, dynamic> json) =>
      _Qn(json['qn'] ?? -1, json['desc'] ?? '${json['qn']}');

  static const List<_Qn> values = [
    _Qn(30000, '杜比'),
    _Qn(25000, '4K 原画'),
    _Qn(20000, '4K'),
    _Qn(15000, '2K'),
    _Qn(10000, '原画'),
    _Qn(400, '蓝光'),
    _Qn(250, '超清'),
    _Qn(150, '高清'),
    _Qn(80, '流畅'),
  ];

  static final Map<int, _Qn> _codeMap = {for (final i in values) i.code: i};

  factory _Qn.fromCode(int code) => _codeMap[code] ?? _Qn.fromId(code);

  /// `LiveQuality.fromFormat` 的三级回退。
  factory _Qn.fromFormat(int code, List<_Qn>? liveQn) {
    if (liveQn != null && liveQn.isNotEmpty) {
      for (final qn in liveQn) {
        if (qn.code == code) {
          return qn;
        }
      }
    }
    return _Qn.fromCode(code);
  }
}

/// `live_playurl_compat.dart` 的 `toLiveQuality` 的逐字拷贝。
_Qn _applyCompat(Map<String, dynamic> rustQn) {
  final desc = rustQn['desc'] as String?;
  final code = rustQn['code'] as int;
  return desc == null ? _Qn.fromCode(code) : _Qn(code, desc);
}

class _UrlInfo {
  final String host;
  final String extra;

  _UrlInfo({required this.host, required this.extra});

  factory _UrlInfo.fromJson(Map<String, dynamic> json) => _UrlInfo(
    host: json['host'] as String,
    extra: json['extra'] as String,
  );
}

class _CodecItem {
  final String? codecName;
  final _Qn currentQn;
  final List<_Qn> acceptQn;
  final String baseUrl;
  final List<_UrlInfo> urlInfo;

  _CodecItem({
    this.codecName,
    required this.currentQn,
    required this.acceptQn,
    required this.baseUrl,
    required this.urlInfo,
  });

  factory _CodecItem.fromJson(Map<String, dynamic> json, List<_Qn>? liveQn) =>
      _CodecItem(
        codecName: json['codec_name'],
        currentQn: _Qn.fromFormat(json['current_qn'] as int, liveQn),
        acceptQn: (json['accept_qn'] as List)
            .map((e) => _Qn.fromFormat(e, liveQn))
            .toList(),
        baseUrl: json['base_url'] as String,
        urlInfo: (json['url_info'] as List<dynamic>)
            .map((e) => _UrlInfo.fromJson(e as Map<String, dynamic>))
            .toList(),
      );
}

class _Format {
  final String? formatName;
  final List<_CodecItem> codec;

  _Format({this.formatName, required this.codec});

  factory _Format.fromJson(Map<String, dynamic> json, List<_Qn>? liveQn) =>
      _Format(
        formatName: json['format_name'],
        codec: (json['codec'] as List<dynamic>)
            .map((e) => _CodecItem.fromJson(e as Map<String, dynamic>, liveQn))
            .toList(),
      );
}

class _Stream {
  final String? protocolName;
  final List<_Format> format;

  _Stream({this.protocolName, required this.format});

  factory _Stream.fromJson(Map<String, dynamic> json, List<_Qn>? liveQn) =>
      _Stream(
        protocolName: json['protocol_name'],
        format: (json['format'] as List<dynamic>)
            .map((e) => _Format.fromJson(e as Map<String, dynamic>, liveQn))
            .toList(),
      );
}

class _Playurl {
  late List<_Stream> stream;

  _Playurl.fromJson(Map<String, dynamic> json) {
    final liveQn = (json['g_qn_desc'] as List<dynamic>?)
        ?.map((e) => _Qn.fromJson(e as Map<String, dynamic>))
        .toList();
    stream = (json['stream'] as List<dynamic>)
        .map((e) => _Stream.fromJson(e as Map<String, dynamic>, liveQn))
        .toList();
  }
}

class _PlayurlInfo {
  final _Playurl? playurl;

  _PlayurlInfo({this.playurl});

  factory _PlayurlInfo.fromJson(Map<String, dynamic> json) => _PlayurlInfo(
    playurl: json['playurl'] == null
        ? null
        : _Playurl.fromJson(json['playurl'] as Map<String, dynamic>),
  );
}

class _RoomPlayInfoData {
  final int? roomId;
  final int? shortId;
  final int? uid;
  final bool? isPortrait;
  final int? liveStatus;
  final int? liveTime;
  final _PlayurlInfo? playurlInfo;

  _RoomPlayInfoData({
    this.roomId,
    this.shortId,
    this.uid,
    this.isPortrait,
    this.liveStatus,
    this.liveTime,
    this.playurlInfo,
  });
}

abstract final class _Baseline {
  /// `LiveHttp.liveRoomInfo` 的查询参数，**连同类型**逐字拷贝。
  ///
  /// 类型是这里的关键：`web_location` 在基线里是 double 字面量
  /// `444.8`，`ptype`/`dolby`/`panorama` 是 int。`encWbi` 用
  /// `params[i].toString()` 取值，类型变了字符串就变，签名跟着变。
  static Map<String, Object> liveRoomInfoParams({
    required int roomId,
    int? qn,
    bool onlyAudio = false,
    required int wts,
  }) => {
    'room_id': roomId,
    'protocol': '0,1',
    'format': '0,1,2',
    'codec': '0,1,2',
    if (qn != null) 'qn': qn,
    'platform': 'web',
    'ptype': 8,
    'dolby': 5,
    'panorama': 1,
    if (onlyAudio) 'only_audio': 1,
    'web_location': 444.8,
    // 基线由 `encWbi` 自己放入，这里显式给定以便复现。
    'wts': wts,
  };

  /// `WbiSign.encWbi` 里算 `w_rid` 的那几行，逐字拷贝。
  static String wRid(Map<String, Object> params, String mixinKey) {
    final chrFilter = RegExp(r"[!\'\(\)\*]");
    final keys = params.keys.toList()..sort();
    final queryStr = keys
        .map(
          (i) =>
              '${Uri.encodeComponent(i)}='
              '${Uri.encodeComponent(params[i].toString().replaceAll(chrFilter, ''))}',
        )
        .join('&');
    return md5.convert(utf8.encode(queryStr + mixinKey)).toString();
  }

  static _RoomPlayInfoData roomPlayInfo(Map<String, dynamic> json) =>
      _RoomPlayInfoData(
        roomId: json['room_id'] as int?,
        shortId: json['short_id'] as int?,
        uid: json['uid'] as int?,
        isPortrait: json['is_portrait'] as bool?,
        liveStatus: json['live_status'] as int?,
        liveTime: json['live_time'] as int?,
        playurlInfo: json['playurl_info'] == null
            ? null
            : _PlayurlInfo.fromJson(
                json['playurl_info'] as Map<String, dynamic>,
              ),
      );
}
