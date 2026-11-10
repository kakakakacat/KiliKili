import 'dart:convert';
import 'dart:io';

import 'package:PiliPlus/models/video/play/url.dart';
import 'package:PiliPlus/services/rust/play_url_denormalize.dart';
import 'package:PiliPlus/services/rust/play_url_normalize.dart';
import 'package:flutter_test/flutter_test.dart';

/// `play_url_normalize.dart` 是 `PlayUrlModel.fromJson` 那套解析规则的
/// 转写（理由见该文件的文档注释）。转写会脱节，本测试是兜底：用**真正的
/// 模型**解析同一份 fixture，再核对「规范化 → 还原 → 模型」是否等价。
///
/// 下面的 `_fromModel` 读的是**模型实例的字段**，不是 JSON：模型改了
/// 解析行为而规范化器或还原器没跟上时，两边的值会对不上。
void main() {
  const fixtureDir = 'rust/crates/bili-api/tests/fixtures/play_url';

  late List<File> raws;

  setUpAll(() {
    raws =
        Directory(fixtureDir)
            .listSync()
            .whereType<File>()
            .where((f) => f.path.endsWith('.raw.json'))
            .toList()
          ..sort((a, b) => a.path.compareTo(b.path));
  });

  test('fixture 目录非空，否则本测试形同虚设', () {
    expect(raws, isNotEmpty);
  });

  test('规范化 → 还原 → 模型，与直接解析原始响应等价', () {
    // Rust 取流路径返回的是规范化 JSON，要经 denormalize 才交给模型。
    // 这一来一回若丢了字段，表现为「某个清晰度不见了」「音量均衡失效」
    // 这类很难反推的现象——所以在这里钉死。
    for (final raw in raws) {
      final name = raw.uri.pathSegments.last;
      final shape = _shapeOf(name);
      expect(shape, isNotNull, reason: '$name 的文件名没有指出响应形状');

      final body = jsonDecode(raw.readAsStringSync()) as Map<String, dynamic>;
      final normalized = normalizePlayUrl(
        body,
        PlayUrlShape.of(shape!.$1, shape.$2),
      );
      expect(normalized, isNotNull, reason: '$name: 规范化为空');

      final direct = _fromModel(_modelFromRaw(body, shape.$1, shape.$2));
      final roundTrip = _fromModel(
        PlayUrlModel.fromJson(denormalizePlayUrl(normalized!)),
      );

      expect(roundTrip, direct, reason: '${raw.path}: 规范化往返丢了字段');
    }
  });
}

(String, bool)? _shapeOf(String fileName) {
  final match = RegExp(
    r'\.(ugc|pgc|pugv)(-fallback)?\.raw\.json$',
  ).firstMatch(fileName);
  if (match == null) return null;
  return (match.group(1)!, match.group(2) != null);
}

/// 按 `VideoHttp.videoUrl` 的分支从原始响应构造模型。
///
/// 这里复制的是那个 switch 的取数位置，不是整段逻辑——重试分支与错误
/// 文案不属于规范化的对照范围。
PlayUrlModel _modelFromRaw(
  Map<String, dynamic> body,
  String videoType,
  bool fallback,
) {
  switch (PlayUrlShape.of(videoType, fallback)) {
    case PlayUrlShape.ugc:
      return PlayUrlModel.fromJson(body['data']);
    case PlayUrlShape.playView:
      final data = body['data'];
      return PlayUrlModel.fromJson(data['video_info'])
        ..lastPlayTime =
            data['watch_progress']?['current_watch_progress'] ??
            data['watch_progress']?['current_progress'];
    case PlayUrlShape.pgcFallback:
      final result = body['result'];
      return PlayUrlModel.fromJson(result['video_info'])
        ..lastPlayTime =
            result['play_view_business_info']?['user_status']?['watch_progress']?['current_watch_progress'];
    case PlayUrlShape.pugvFallback:
      final data = body['data'];
      return PlayUrlModel.fromJson(data)
        ..lastPlayTime =
            data['play_view_business_info']?['user_status']?['watch_progress']?['current_watch_progress'];
  }
}

/// 模型实例 → 可比对的纯值。
///
/// 清晰度描述（`VideoItem.quality` / `AudioItem.quality`）也一并读出来：
/// 它们由模型从 id 与 support_formats 推导，若规范化往返弄丢了
/// `support_formats`，正是这里会先暴露。
Map<String, dynamic> _fromModel(PlayUrlModel m) => {
  'from': m.from,
  'result': m.result,
  'message': m.message,
  'quality': m.quality,
  'format': m.format,
  'timeLength': m.timeLength,
  'acceptFormat': m.acceptFormat,
  'acceptDesc': m.acceptDesc,
  'acceptQuality': m.acceptQuality,
  'videoCodecid': m.videoCodecid,
  'seekParam': m.seekParam,
  'seekType': m.seekType,
  'lastPlayTime': m.lastPlayTime,
  'lastPlayCid': m.lastPlayCid,
  'curLanguage': m.curLanguage,
  'supportFormats': m.supportFormats
      ?.map(
        (f) => [f.quality, f.format, f.newDesc, f.displayDesc, f.codecs],
      )
      .toList(),
  'dash': m.dash == null
      ? null
      : [
          m.dash!.duration,
          m.dash!.minBufferTime,
          m.dash!.video?.map(_item).toList(),
          m.dash!.audio?.map(_item).toList(),
        ],
  'durl': m.durl
      ?.map((d) => [d.order, d.length, d.size, d.ahead, d.vhead, d.url, d.backupUrl])
      .toList(),
  'volume': m.volume == null
      ? null
      : [
          m.volume!.measuredI,
          m.volume!.measuredLra,
          m.volume!.measuredTp,
          m.volume!.measuredThreshold,
          m.volume!.targetOffset,
          m.volume!.targetI,
          m.volume!.targetTp,
        ],
  'language': m.language == null
      ? null
      : [
          m.language!.support,
          m.language!.items
              ?.map((i) => [i.lang, i.title, i.subtitleLang, i.isAi])
              .toList(),
        ],
  'clipInfoList': m.clipInfoList
      ?.map((c) => [c.category, c.segment, c.videoDuration])
      .toList(),
};

List<dynamic> _item(BaseItem i) => [
  i.id,
  i.baseUrl,
  i.backupUrl,
  i.bandWidth,
  i.mimeType,
  i.codecs,
  i.width,
  i.height,
  i.frameRate,
  i.sar,
  i.startWithSap,
  i.segmentBase,
  i.codecid,
  switch (i) {
    VideoItem() => i.quality.toString(),
    AudioItem() => i.quality,
    _ => null,
  },
];
