import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

/// 取流迁移的**基线对照**（issue #79，机制见 ADR-011 决策五）。
///
/// 被取代的 `PlayUrlModel` 整棵树随本 PR 从 `lib/` 删除，借不到活着的
/// 基线，所以下面 `_Baseline` 里是迁移前那一版的**逐字拷贝**：
///
/// - `lib/models/video/play/url.dart` 的 `PlayUrlModel.fromJson` 及其
///   全部子模型（`Dash` / `BaseItem` / `VideoItem` / `AudioItem` /
///   `Durl` / `FormatItem` / `Volume` / `Language` / `LanguageItem`）；
/// - `lib/models/common/video/video_quality.dart` 的 `fromFormat` 三级
///   回退与那张静态表里本测试用得到的几档；
/// - `lib/models_new/sponsor_block/segment_item.dart` 的 `fromPgcJson`。
///
/// 它**不 import `lib/` 的任何东西**：一是 ADR-011 要求对照的是冻结下来
/// 的基线而不是会跟着改的现实现，二是这样它在没打 Flutter SDK 补丁的
/// 机器上也跑得起来。写下之后不再改。
///
/// 另一侧是 Rust 的快照 `*.rust.json`
/// （`rust/crates/bili-api/tests/fixtures/play_url/`，由
/// `play_url_golden.rs` 冻结）。两者由完全不同的实现产出，任何一边改了
/// 行为都会在这里变红。**通过之后本文件连同快照里的 Dart 痕迹一并退场**
/// ——它证明的是「迁移这一刻没有改行为」，不是长期回归。
///
/// ## 四处**有意**与基线不同，这里断言的是差异本身
///
/// 1. **音轨排序下沉。** 基线在 `Language.fromJson` 里排（中文在前、AI
///    在后），现在 Rust 解析时就排好。下面对基线那一侧照样排一遍，比的
///    是两边排完的顺序一致。
/// 2. **`is_ai` 是新增字段。** 基线没有这个字段，它把 AI 判定的结果直接
///    拼进了 `title`（`「国语（AI）」`）。所以比的是
///    `rust.title + (rust.is_ai ? '（AI）' : '')` 与基线的 `title`。
/// 3. **`clip_info_list` 保持接口单位（秒）。** 基线在模型里就转成了
///    毫秒的 `SegmentItemModel`；转换现在在 Rust 的 `pgc_clip_segments`
///    里，不在解析里。下面按同样的规则转一遍再比。
/// 4. **流上不再挂 `quality`。** 基线 `VideoItem.quality` 是
///    `VideoQuality.fromFormat(id, formats)` 的结果，与流的 `id` 恒等
///    （名字可能来自响应，码永远是 id）。下面对每条流算一遍基线的名字，
///    确认它能从快照里的 `support_formats` 原样重建。
void main() {
  const dir = 'rust/crates/bili-api/tests/fixtures/play_url';

  Map<String, dynamic> readRaw(String name) =>
      jsonDecode(File('$dir/$name.raw.json').readAsStringSync())
          as Map<String, dynamic>;

  Map<String, dynamic> readSnapshot(String name) =>
      jsonDecode(File('$dir/$name.rust.json').readAsStringSync())
          as Map<String, dynamic>;

  /// 从整份响应里取出基线 `fromJson` 真正吃的那一层。
  ///
  /// 四条接口路径的外层各不相同，基线在 `VideoHttp.videoUrl` 的 switch
  /// 里拆；这里按同样的规则拆，顺带把两处 `lastPlayTime` 的来源也照搬。
  _Baseline parseBaseline(Map<String, dynamic> body, String shape) {
    switch (shape) {
      case 'ugc':
        return _Baseline.fromJson(body['data'] as Map<String, dynamic>);
      case 'pgc':
        final result = body['data'] as Map<String, dynamic>;
        final videoInfo = result['video_info'] as Map<String, dynamic>;
        final model = _Baseline.fromJson(videoInfo);
        final progress = result['watch_progress'];
        if (progress != null) {
          model.lastPlayTime =
              progress['current_watch_progress'] ??
              progress['current_progress'];
        }
        return model;
      case 'pugv-fallback':
        final result = body['data'] as Map<String, dynamic>;
        return _Baseline.fromJson(result)
          ..lastPlayTime =
              result['play_view_business_info']?['user_status']?['watch_progress']?['current_watch_progress'];
      default:
        throw ArgumentError('未知形状 $shape');
    }
  }

  void expectStream(
    Map<String, dynamic> rust,
    _BaseItem baseline,
    String where,
  ) {
    expect(rust['id'], baseline.id, reason: '$where: id');
    expect(rust['base_url'], baseline.baseUrl, reason: '$where: base_url');
    expect(
      (rust['backup_url'] as List?)?.cast<String>(),
      baseline.backupUrl,
      reason: '$where: backup_url',
    );
    expect(rust['band_width'], baseline.bandWidth, reason: '$where: bandwidth');
    expect(rust['mime_type'], baseline.mimeType, reason: '$where: mime_type');
    expect(rust['codecs'], baseline.codecs, reason: '$where: codecs');
    expect(rust['width'], baseline.width, reason: '$where: width');
    expect(rust['height'], baseline.height, reason: '$where: height');
    expect(
      rust['frame_rate'],
      baseline.frameRate,
      reason: '$where: frame_rate',
    );
    expect(rust['sar'], baseline.sar, reason: '$where: sar');
    expect(
      rust['start_with_sap'],
      baseline.startWithSap,
      reason: '$where: start_with_sap',
    );
    expect(rust['codecid'], baseline.codecid, reason: '$where: codecid');

    final segmentBase = rust['segment_base'] as Map<String, dynamic>?;
    if (baseline.segmentBase == null) {
      expect(segmentBase, isNull, reason: '$where: segment_base');
    } else {
      expect(
        segmentBase?['initialization'],
        baseline.segmentBase!['initialization'],
        reason: '$where: segment_base.initialization',
      );
      expect(
        segmentBase?['index_range'],
        baseline.segmentBase!['index_range'],
        reason: '$where: segment_base.index_range',
      );
    }

    // 候选地址的顺序：主地址在前、备用依次在后。规则现在在 Rust，
    // 但它只用 base_url / backup_url，所以能从快照原样重建。
    final playUrls = [
      if (rust['base_url'] case final String u when u.isNotEmpty) u,
      ...?(rust['backup_url'] as List?)?.cast<String>(),
    ];
    expect(playUrls, baseline.playUrls.toList(), reason: '$where: playUrls');
  }

  void checkCase(String name, String shape) {
    final rust = readSnapshot(name);
    final baseline = parseBaseline(readRaw(name), shape);

    expect(rust['from'], baseline.from, reason: '$name: from');
    expect(rust['result'], baseline.result, reason: '$name: result');
    expect(rust['message'], baseline.message, reason: '$name: message');
    expect(rust['quality'], baseline.quality, reason: '$name: quality');
    expect(rust['format'], baseline.format, reason: '$name: format');
    expect(
      rust['time_length'],
      baseline.timeLength,
      reason: '$name: timelength',
    );
    expect(
      rust['accept_format'],
      baseline.acceptFormat,
      reason: '$name: accept_format',
    );
    expect(
      (rust['accept_description'] as List?)?.cast<String>(),
      baseline.acceptDesc?.cast<String>(),
      reason: '$name: accept_description',
    );
    expect(
      (rust['accept_quality'] as List?)?.cast<int>(),
      baseline.acceptQuality,
      reason: '$name: accept_quality',
    );
    expect(
      rust['video_codecid'],
      baseline.videoCodecid,
      reason: '$name: video_codecid',
    );
    expect(rust['seek_param'], baseline.seekParam, reason: '$name: seek_param');
    expect(rust['seek_type'], baseline.seekType, reason: '$name: seek_type');
    expect(
      rust['last_play_time'],
      baseline.lastPlayTime,
      reason: '$name: last_play_time',
    );
    expect(
      rust['last_play_cid'],
      baseline.lastPlayCid,
      reason: '$name: last_play_cid',
    );
    expect(
      rust['cur_language'],
      baseline.curLanguage,
      reason: '$name: cur_language',
    );

    // --- support_formats
    final rustFormats = (rust['support_formats'] as List?)
        ?.cast<Map<String, dynamic>>();
    expect(
      rustFormats?.length,
      baseline.supportFormats?.length,
      reason: '$name: support_formats 条数',
    );
    if (rustFormats != null) {
      for (var i = 0; i < rustFormats.length; i++) {
        final r = rustFormats[i];
        final b = baseline.supportFormats![i];
        expect(r['quality'], b.quality, reason: '$name: formats[$i].quality');
        expect(r['format'], b.format, reason: '$name: formats[$i].format');
        expect(r['new_desc'], b.newDesc, reason: '$name: formats[$i].new_desc');
        expect(
          r['display_desc'],
          b.displayDesc,
          reason: '$name: formats[$i].display_desc',
        );
        expect(
          (r['codecs'] as List?)?.cast<String>(),
          b.codecs,
          reason: '$name: formats[$i].codecs',
        );
      }
    }

    // --- dash
    final rustDash = rust['dash'] as Map<String, dynamic>?;
    if (baseline.dash == null) {
      expect(rustDash, isNull, reason: '$name: dash');
    } else {
      expect(
        rustDash?['duration'],
        baseline.dash!.duration,
        reason: '$name: dash.duration',
      );
      expect(
        rustDash?['min_buffer_time'],
        baseline.dash!.minBufferTime,
        reason: '$name: dash.min_buffer_time',
      );

      final rustVideo = (rustDash?['video'] as List?)
          ?.cast<Map<String, dynamic>>();
      expect(
        rustVideo?.length,
        baseline.dash!.video?.length,
        reason: '$name: dash.video 条数',
      );
      if (rustVideo != null) {
        for (var i = 0; i < rustVideo.length; i++) {
          expectStream(
            rustVideo[i],
            baseline.dash!.video![i],
            '$name: video[$i]',
          );
          // 差异 4：流上不再挂 quality，但它能从 support_formats 重建。
          final rebuilt = _videoQualityOf(
            rustVideo[i]['id'] as int,
            rustFormats,
          );
          final b = baseline.dash!.video![i].quality!;
          expect(rebuilt.code, b.code, reason: '$name: video[$i].quality.code');
          expect(rebuilt.desc, b.desc, reason: '$name: video[$i].quality.desc');
          expect(
            rebuilt.shortDesc,
            b.shortDesc,
            reason: '$name: video[$i].quality.shortDesc',
          );
        }
      }

      // 音轨已在解析时按 flac → dolby → 普通合并，两边同规则。
      final rustAudio = (rustDash?['audio'] as List?)
          ?.cast<Map<String, dynamic>>();
      expect(
        rustAudio?.length,
        baseline.dash!.audio?.length,
        reason: '$name: dash.audio 条数',
      );
      if (rustAudio != null) {
        for (var i = 0; i < rustAudio.length; i++) {
          expectStream(
            rustAudio[i],
            baseline.dash!.audio![i],
            '$name: audio[$i]',
          );
        }
      }
    }

    // --- durl
    final rustDurl = (rust['durl'] as List?)?.cast<Map<String, dynamic>>();
    expect(
      rustDurl?.length,
      baseline.durl?.length,
      reason: '$name: durl 条数',
    );
    if (rustDurl != null) {
      for (var i = 0; i < rustDurl.length; i++) {
        final r = rustDurl[i];
        final b = baseline.durl![i];
        expect(r['order'], b.order, reason: '$name: durl[$i].order');
        expect(r['length'], b.length, reason: '$name: durl[$i].length');
        expect(r['size'], b.size, reason: '$name: durl[$i].size');
        expect(r['ahead'], b.ahead, reason: '$name: durl[$i].ahead');
        expect(r['vhead'], b.vhead, reason: '$name: durl[$i].vhead');
        expect(r['url'], b.url, reason: '$name: durl[$i].url');
        expect(
          (r['backup_url'] as List?)?.cast<String>(),
          b.backupUrl,
          reason: '$name: durl[$i].backup_url',
        );
        final playUrls = [
          if (r['url'] case final String u when u.isNotEmpty) u,
          ...?(r['backup_url'] as List?)?.cast<String>(),
        ];
        expect(
          playUrls,
          b.playUrls.toList(),
          reason: '$name: durl[$i].playUrls',
        );
      }
    }

    // --- volume
    final rustVolume = rust['volume'] as Map<String, dynamic>?;
    if (baseline.volume == null) {
      expect(rustVolume, isNull, reason: '$name: volume');
    } else {
      final b = baseline.volume!;
      expect(
        rustVolume?['measured_i'],
        b.measuredI,
        reason: '$name: measured_i',
      );
      expect(
        rustVolume?['measured_lra'],
        b.measuredLra,
        reason: '$name: measured_lra',
      );
      expect(
        rustVolume?['measured_tp'],
        b.measuredTp,
        reason: '$name: measured_tp',
      );
      expect(
        rustVolume?['measured_threshold'],
        b.measuredThreshold,
        reason: '$name: measured_threshold',
      );
      expect(
        rustVolume?['target_offset'],
        b.targetOffset,
        reason: '$name: target_offset',
      );
      expect(rustVolume?['target_i'], b.targetI, reason: '$name: target_i');
      expect(rustVolume?['target_tp'], b.targetTp, reason: '$name: target_tp');
    }

    // --- language（差异 1、2）
    final rustLang = rust['language'] as Map<String, dynamic>?;
    if (baseline.language == null) {
      expect(rustLang, isNull, reason: '$name: language');
    } else {
      expect(
        rustLang?['support'],
        baseline.language!.support,
        reason: '$name: language.support',
      );
      final rustItems = (rustLang?['items'] as List?)
          ?.cast<Map<String, dynamic>>();
      expect(
        rustItems?.length,
        baseline.language!.items?.length,
        reason: '$name: language.items 条数',
      );
      if (rustItems != null) {
        for (var i = 0; i < rustItems.length; i++) {
          final r = rustItems[i];
          final b = baseline.language!.items![i];
          expect(r['lang'], b.lang, reason: '$name: lang[$i].lang');
          expect(
            r['subtitle_lang'],
            b.subtitleLang,
            reason: '$name: lang[$i].subtitle_lang',
          );
          expect(r['is_ai'], b.isAi, reason: '$name: lang[$i].is_ai');
          // 基线把后缀拼进了 title，Rust 保留原始 title + is_ai。
          final displayTitle =
              '${r['title']}${r['is_ai'] == true ? '（AI）' : ''}';
          expect(displayTitle, b.title, reason: '$name: lang[$i].title');
        }
      }
    }

    // --- clip_info_list（差异 3）
    final rustClips = (rust['clip_info_list'] as List?)
        ?.cast<Map<String, dynamic>>();
    expect(
      rustClips?.length,
      baseline.clipInfoList?.length,
      reason: '$name: clip_info_list 条数',
    );
    if (rustClips != null) {
      for (var i = 0; i < rustClips.length; i++) {
        final r = rustClips[i];
        final b = baseline.clipInfoList![i];
        expect(
          _segmentCategory(r['clip_type'] as String?),
          b.category,
          reason: '$name: clip[$i].category',
        );
        expect(
          ((r['start'] as num) * 1000).round(),
          b.segment[0],
          reason: '$name: clip[$i].start',
        );
        expect(
          ((r['end'] as num) * 1000).round(),
          b.segment[1],
          reason: '$name: clip[$i].end',
        );
        expect(
          baseline.timeLength,
          b.videoDuration,
          reason: '$name: clip[$i].videoDuration',
        );
      }
    }
  }

  test('UGC DASH（未登录）与基线逐字段一致', () {
    checkCase('dash_anonymous.ugc', 'ugc');
  });

  test('OGV 带观看进度与多语言音轨与基线逐字段一致', () {
    checkCase('ogv_watch_progress.pgc', 'pgc');
  });

  test('PUGV 降级的整段流与基线逐字段一致', () {
    checkCase('durl_progress.pugv-fallback', 'pugv-fallback');
  });
}

// ===================================================================
// 以下全部是迁移前 `lib/` 里那一版的逐字拷贝。不要「顺手改好」：
// 它的价值正在于**冻结**，改了就不再是基线。
// ===================================================================

String _segmentCategory(String? clipType) => switch (clipType) {
  'CLIP_TYPE_OP' => 'intro',
  'CLIP_TYPE_ED' => 'outro',
  _ => 'sponsor',
};

class _Segment {
  _Segment({
    required this.category,
    required this.segment,
    required this.videoDuration,
  });

  final String category;
  final List<int> segment;
  final num? videoDuration;

  /// `SegmentItemModel.fromPgcJson` 的拷贝。
  factory _Segment.fromPgcJson(Map<String, dynamic> json, num? videoDuration) =>
      _Segment(
        category: _segmentCategory(json['clipType'] as String?),
        segment: [
          ((json['start'] as num) * 1000).round(),
          ((json['end'] as num) * 1000).round(),
        ],
        videoDuration: videoDuration,
      );
}

/// `VideoQuality` 的拷贝，只含本测试用得到的部分。
class _VideoQuality {
  const _VideoQuality(this.code, this.desc, this.shortDesc);

  final int code;
  final String desc;
  final String shortDesc;

  static const List<_VideoQuality> values = [
    _VideoQuality(129, 'HDR Vivid', 'HDR Vivid'),
    _VideoQuality(127, '8K 超高清', '8K'),
    _VideoQuality(126, '杜比视界', '杜比'),
    _VideoQuality(125, 'HDR 真彩', 'HDR'),
    _VideoQuality(120, '4K 超高清', '4K'),
    _VideoQuality(116, '1080P 60帧', '1080P60'),
    _VideoQuality(112, '1080P 高码率', '1080P+'),
    _VideoQuality(100, '智能修复', '智能修复'),
    _VideoQuality(80, '1080P 高清', '1080P'),
    _VideoQuality(74, '720P 60帧', '720P60'),
    _VideoQuality(64, '720P 准高清', '720P'),
    _VideoQuality(32, '480P 标清', '480P'),
    _VideoQuality(16, '360P 流畅', '360P'),
    _VideoQuality(6, '240P 极速', '240P'),
  ];

  static _VideoQuality fromCode(int code) => values.firstWhere(
    (e) => e.code == code,
    orElse: () => _VideoQuality(code, code.toString(), code.toString()),
  );
}

/// `VideoQuality.fromFormat` 的拷贝（三级回退）。
_VideoQuality _videoQualityOf(int code, List<Map<String, dynamic>>? formats) {
  if (formats != null && formats.isNotEmpty) {
    for (final format in formats) {
      if (format['quality'] == code) {
        final newDesc = format['new_desc'] as String?;
        final displayDesc = format['display_desc'] as String?;
        return _VideoQuality(
          code,
          newDesc ?? displayDesc ?? code.toString(),
          displayDesc ?? newDesc ?? code.toString(),
        );
      }
    }
  }
  return _VideoQuality.fromCode(code);
}

class _Baseline {
  String? from;
  String? result;
  String? message;
  int? quality;
  String? format;
  int? timeLength;
  String? acceptFormat;
  List<dynamic>? acceptDesc;
  List<int>? acceptQuality;
  int? videoCodecid;
  String? seekParam;
  String? seekType;
  _Dash? dash;
  List<_Durl>? durl;
  List<_Format>? supportFormats;
  _Volume? volume;

  int _lastPlayTime = 0;
  int get lastPlayTime => _lastPlayTime;
  set lastPlayTime(int? value) {
    if (value != null && value > 0) {
      _lastPlayTime = value;
    } else {
      _lastPlayTime = 0;
    }
  }

  int? lastPlayCid;
  String? curLanguage;
  _Language? language;
  List<_Segment>? clipInfoList;

  _Baseline.fromJson(Map<String, dynamic> json) {
    from = json['from'];
    result = json['result'];
    message = json['message'];
    quality = json['quality'];
    format = json['format'];
    timeLength = json['timelength'];
    acceptFormat = json['accept_format'];
    acceptDesc = json['accept_description'];
    acceptQuality = (json['accept_quality'] as List?)
        ?.map<int>((e) => e as int)
        .toList();
    videoCodecid = json['video_codecid'];
    seekParam = json['seek_param'];
    seekType = json['seek_type'];
    supportFormats = (json['support_formats'] as List?)
        ?.map<_Format>((e) => _Format.fromJson(e))
        .toList();
    dash = json['dash'] != null
        ? _Dash.fromJson(json['dash'], supportFormats)
        : null;
    durl = (json['durl'] as List?)
        ?.map<_Durl>((e) => _Durl.fromJson(e))
        .toList();
    volume = json['volume'] == null ? null : _Volume.fromJson(json['volume']);
    lastPlayTime = json['last_play_time'];
    lastPlayCid = json['last_play_cid'];
    curLanguage = json['cur_language'];
    language = json['language'] == null
        ? null
        : _Language.fromJson(json['language']);
    final List? clips = json['clip_info_list'];
    if (clips != null && clips.isNotEmpty) {
      clipInfoList = clips
          .map(
            (e) => _Segment.fromPgcJson(e as Map<String, dynamic>, timeLength),
          )
          .toList();
    }
  }
}

class _Language {
  bool? support;
  List<_LanguageItem>? items;

  _Language.fromJson(Map<String, dynamic> json) {
    support = json['support'];
    items =
        (json['items'] as List?)?.map((e) => _LanguageItem.fromJson(e)).toList()
          ?..sort(_sort);
  }
}

int _sort(_LanguageItem a, _LanguageItem b) {
  final aHasZh = a.lang?.contains('zh') ?? false;
  final bHasZh = b.lang?.contains('zh') ?? false;
  if (aHasZh != bHasZh) return aHasZh ? -1 : 1;
  if (a.isAi != b.isAi) return a.isAi ? 1 : -1;
  return 0;
}

class _LanguageItem {
  String? lang;
  String? title;
  String? subtitleLang;
  bool isAi = false;

  _LanguageItem.fromJson(Map<String, dynamic> json) {
    lang = json['lang'];
    isAi = json['production_type'] == 2;
    title = '${json['title']}${isAi ? '（AI）' : ''}';
    subtitleLang = json['subtitle_lang'];
  }
}

class _Dash {
  int? duration;
  double? minBufferTime;
  List<_BaseItem>? video;
  List<_BaseItem>? audio;

  _Dash.fromJson(Map<String, dynamic> json, List<_Format>? supportFormats) {
    duration = json['duration'];
    minBufferTime = json['minBufferTime'];
    video = (json['video'] as List?)
        ?.map<_BaseItem>((e) => _BaseItem.fromJson(e, supportFormats))
        .toList();
    final audio = [
      if (json['flac']?['audio'] case Map<String, dynamic> flac)
        _BaseItem.fromJson(flac, null),
      if (json['dolby']?['audio'] case List list)
        ...list.map((e) => _BaseItem.fromJson(e, null)),
      if (json['audio'] case List list)
        ...list.map((e) => _BaseItem.fromJson(e, null)),
    ];
    this.audio = audio.isEmpty ? null : audio;
  }
}

class _BaseItem {
  late int id;
  String? baseUrl;
  List<String>? backupUrl;
  int? bandWidth;
  String? mimeType;
  String? codecs;
  int? width;
  int? height;
  String? frameRate;
  String? sar;
  int? startWithSap;
  Map? segmentBase;
  int? codecid;
  _VideoQuality? quality;

  _BaseItem.fromJson(Map<String, dynamic> json, List<_Format>? formats) {
    id = json['id'];
    baseUrl = json['baseUrl'] ?? json['base_url'];
    backupUrl = ((json['backupUrl'] ?? json['backup_url']) as List?)
        ?.cast<String>();
    bandWidth = json['bandWidth'] ?? json['bandwidth'];
    mimeType = json['mime_type'];
    codecs = json['codecs'];
    width = json['width'];
    height = json['height'];
    frameRate = json['frameRate'] ?? json['frame_rate'];
    sar = json['sar'];
    startWithSap = json['startWithSap'] ?? json['start_with_sap'];
    segmentBase = json['segmentBase'] ?? json['segment_base'];
    codecid = json['codecid'];
    if (formats != null) {
      quality = _videoQualityOf(id, [
        for (final f in formats)
          {
            'quality': f.quality,
            'new_desc': f.newDesc,
            'display_desc': f.displayDesc,
          },
      ]);
    }
  }

  Iterable<String> get playUrls sync* {
    if (baseUrl?.isNotEmpty == true) yield baseUrl!;
    if (backupUrl?.isNotEmpty == true) yield* backupUrl!;
  }
}

class _Durl {
  int? order;
  int? length;
  int? size;
  String? ahead;
  String? vhead;
  String? url;
  List<String>? backupUrl;

  _Durl.fromJson(Map<String, dynamic> json)
    : order = json['order'],
      length = json['length'],
      size = json['size'],
      ahead = json['ahead'],
      vhead = json['vhead'],
      url = json['url'],
      backupUrl = (json['backup_url'] as List?)?.cast<String>();

  Iterable<String> get playUrls sync* {
    if (url?.isNotEmpty == true) yield url!;
    if (backupUrl?.isNotEmpty == true) yield* backupUrl!;
  }
}

class _Format {
  int? quality;
  String? format;
  String? newDesc;
  String? displayDesc;
  List<String>? codecs;

  _Format.fromJson(Map<String, dynamic> json) {
    quality = json['quality'];
    format = json['format'];
    newDesc = json['new_description'];
    displayDesc = json['display_desc'];
    codecs = (json['codecs'] as List?)?.cast<String>();
  }
}

class _Volume {
  _Volume({
    required this.measuredI,
    required this.measuredLra,
    required this.measuredTp,
    required this.measuredThreshold,
    required this.targetOffset,
    required this.targetI,
    required this.targetTp,
  });

  final num measuredI;
  final num measuredLra;
  final num measuredTp;
  final num measuredThreshold;
  final num targetOffset;
  final num targetI;
  final num targetTp;

  factory _Volume.fromJson(Map<String, dynamic> json) => _Volume(
    measuredI: json['measured_i'] ?? 0,
    measuredLra: json['measured_lra'] ?? 0,
    measuredTp: json['measured_tp'] ?? 0,
    measuredThreshold: json['measured_threshold'] ?? 0,
    targetOffset: json['target_offset'] ?? 0,
    targetI: json['target_i'] ?? 0,
    targetTp: json['target_tp'] ?? 0,
  );
}
