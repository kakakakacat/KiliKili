/// 取流 typed 结构的宿主侧便利层（issue #79）。
///
/// ## 这里**不放逻辑**
///
/// 基线 `PlayUrlModel` 上那些派生行为——音轨排序、候选 URL 顺序、CDN
/// 选路、补档的缺档计算与去重合并、loudnorm 参数串、片头片尾转片段——
/// 按 RFC §2.3 的判据全部下沉到了 Rust（`bili-core` 的 `cdn` /
/// `playback`，经 `bridge_flutter::api::playback` 导出）。换掉 Flutter
/// 这层时它们一个字都不用重写。
///
/// 本文件剩下两类东西，都不满足那条判据：
///
/// 1. **转发**：把 FRB 的自由函数挂成扩展，让调用点读起来还是
///    `stream.playUrls` 而不是 `rustApi.streamPlayUrls(stream: stream)`。
///    转发不是逻辑，删掉它只是调用点变啰嗦。
/// 2. **宿主的静态表与文案**：`VideoQuality` / `AudioQuality` 是客户端
///    对码率号的命名（见 `base_quality.dart`），「（AI）」是中文 UI 后缀。
///
/// ## 那两张静态表为什么还在 Dart
///
/// 它们确实满足「换宿主要重抄一遍」，但只有十来行常量，而拔出来要连带
/// 改设置页的画质/音质下拉、`Pref` 的默认值和一串 `Rxn<VideoQuality>`
/// ——那是设置与播放器 UI 的改造，不该塞进起播路径这一片。
/// **随画质设置项迁移时一并下沉**，届时本文件只剩转发。
library;

import 'package:kilikili/models/common/video/base_quality.dart';
import 'package:kilikili/rust/api/play_url.dart';
import 'package:kilikili/rust/api/playback.dart' as rust_api;
import 'package:kilikili/rust/api/sponsor_block.dart' show SkipSegment;
// FRB 自带的 Int64List，与 dart:typed_data 的同名类不是一个类型。
import 'package:flutter_rust_bridge/flutter_rust_bridge.dart' show Int64List;

// ------------------------------------------------------------------ 取流

extension PlayUrlExt on PlayUrl {
  /// 低于最高档、但 DASH 里没给出的最高一档清晰度；没有则 `null`。
  int? get missingVideoQualityBelowHighest {
    final videos = dash?.video;
    final formats = supportFormats;
    if (videos == null || videos.isEmpty || formats == null) return null;
    return rust_api.missingVideoQualityBelowHighest(
      videoIds: Int64List.fromList([for (final v in videos) v.id]),
      supportQualities: Int64List.fromList([
        for (final f in formats)
          if (f.quality case final q?) q,
      ]),
    );
  }

  /// 音轨列表。顺序在 Rust 解析时就排好了（中文在前、AI 在后）。
  List<LanguageItem>? get languageItems => language?.items;

  /// 换掉 `dash`，其余字段原样保留。
  ///
  /// 补档要把新清晰度并进来，而 FRB 结构不可变。FRB 不生成 `copyWith`，
  /// 又犯不上为一个字段生成一整套，所以只开这一个口子——其余字段都是
  /// 本次取流的结果，不该被补档那次请求的响应覆盖。
  PlayUrl copyWithDash(Dash dash) => PlayUrl(
    from: from,
    result: result,
    message: message,
    quality: quality,
    format: format,
    timeLength: timeLength,
    acceptFormat: acceptFormat,
    acceptDescription: acceptDescription,
    acceptQuality: acceptQuality,
    videoCodecid: videoCodecid,
    seekParam: seekParam,
    seekType: seekType,
    supportFormats: supportFormats,
    dash: dash,
    durl: durl,
    volume: volume,
    lastPlayTime: lastPlayTime,
    lastPlayCid: lastPlayCid,
    curLanguage: curLanguage,
    language: language,
    clipInfoList: clipInfoList,
  );
}

extension LanguageItemExt on LanguageItem {
  /// 展示用标题。AI 配音的判定在 Rust（`is_ai`），这里只负责把中文后缀
  /// 拼上去——换一套 UI 语言时要改的正是这一行。
  String get displayTitle => '$title${isAi ? '（AI）' : ''}';
}

// ------------------------------------------------------------------ 流

extension MediaStreamExt on MediaStream {
  /// 可播地址的候选顺序：主地址在前，备用地址依次在后。
  List<String> get playUrls => rust_api.streamPlayUrls(stream: this);

  /// 清晰度。查宿主的静态表，响应给了名字就用响应的。
  VideoQuality videoQuality(List<PlayFormat>? formats) =>
      videoQualityOf(id, formats);

  /// 音质名。
  String get audioQualityDesc => AudioQuality.fromCode(id).desc;
}

extension DurlExt on Durl {
  /// 同 [MediaStreamExt.playUrls]。
  List<String> get playUrls =>
      rust_api.durlPlayUrls(url: url, backupUrl: backupUrl);
}

extension MediaStreamListExt on List<MediaStream> {
  /// 列表里出现过的清晰度档位。
  ///
  /// 这是一次投影，不是规则——没有任何判断可言，不值得跨一次 FFI。
  Set<int> get availableVideoQualities => {for (final i in this) i.id};

  /// 并入补档结果，按清晰度从高到低排好。去重与排序在 Rust。
  List<MediaStream> merged(List<MediaStream>? other) => other == null
      ? this
      : rust_api.mergeVideoStreams(base: this, extra: other);
}

/// 查一档清晰度的展示名。
///
/// 三级回退：响应的 `support_formats` 命中 → 用它给的名字；没命中 →
/// 查宿主静态表；表里也没有 → 名字退化成码率号。
///
/// 留在宿主的理由见本文件开头：回退链本身该下沉，但它查的那张表暂时
/// 还在这边，拆成两处反而更糟。
VideoQuality videoQualityOf(int code, List<PlayFormat>? formats) {
  if (formats != null) {
    for (final format in formats) {
      if (format.quality == code) {
        final newDesc = format.newDesc;
        final displayDesc = format.displayDesc;
        return VideoQuality(
          code,
          newDesc ?? displayDesc ?? code.toString(),
          displayDesc ?? newDesc ?? code.toString(),
        );
      }
    }
  }
  return VideoQuality.fromCode(code);
}

// ------------------------------------------------------------------ 音量

extension VolumeExt on Volume {
  /// 服务端是否给了有效的均衡参数。
  bool get isNotEmpty => rust_api.hasVolumeMeasurement(volume: this);

  /// 拼出 FFmpeg `loudnorm` 的参数串。
  String format(Map<String, num> config) => rust_api.loudnormParams(
    volume: this,
    config: rust_api.LoudnormConfig(
      lra: config['lra']?.toDouble(),
      i: config['i']?.toDouble(),
      tp: config['tp']?.toDouble(),
      offset: config['offset']?.toDouble(),
    ),
  );

  /// 下载条目的**磁盘格式**。键名必须与基线 `Volume.toJson` 一字不差：
  /// 用户机器上已有的 `entry.json` 要能继续读出来。
  ///
  /// 这条没下沉：它是下载条目持久化格式的一部分，而那整套记录的读写
  /// 仍在宿主侧（`BiliDownloadEntryInfo`）。随下载迁移时一并处理。
  Map<String, dynamic> toJson() => {
    'measured_i': measuredI,
    'measured_lra': measuredLra,
    'measured_tp': measuredTp,
    'measured_threshold': measuredThreshold,
    'target_offset': targetOffset,
    'target_i': targetI,
    'target_tp': targetTp,
  };
}

/// [Volume] 的静态成员。扩展里放不了静态量，单独开一个。
abstract final class Volumes {
  /// 从磁盘上的下载条目读回。坏数据按「没有」处理，不让一条历史记录
  /// 把下载列表整个打不开——基线 `Volume.fromCache` 就是这个行为。
  static Volume? fromCache(dynamic json) {
    if (json == null) return null;
    try {
      final map = json as Map;
      double value(String key) => ((map[key] as num?) ?? 0).toDouble();
      return Volume(
        measuredI: value('measured_i'),
        measuredLra: value('measured_lra'),
        measuredTp: value('measured_tp'),
        measuredThreshold: value('measured_threshold'),
        targetOffset: value('target_offset'),
        targetI: value('target_i'),
        targetTp: value('target_tp'),
      );
    } catch (_) {}
    return null;
  }
}

// ------------------------------------------------------------------ 跳过片段

/// PGC 的片头片尾 → 可跳过片段。
///
/// 返回 SponsorBlock 那条路径的同一个类型，宿主因此只需要一条消费路径
/// （`sponsor_block_compat.dart` 把它转成 `SegmentItemModel`）。
List<SkipSegment> pgcClipSegments(
  List<ClipInfo>? clipInfoList,
  int? timeLength,
) => clipInfoList == null || clipInfoList.isEmpty
    ? const []
    : rust_api.pgcClipSegments(clips: clipInfoList, timeLengthMs: timeLength);
