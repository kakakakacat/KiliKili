/// 取流响应的**规范化**：把 playurl 四条接口路径的响应转成与 Rust 侧
/// `bili_domain::playurl::PlayUrl` 逐字段对应的 Map。
///
/// 两个使用者，必须共用同一份规则：
///
/// 1. `tool/fixture-gen/play_url_fixture.dart` —— 产出 golden fixture 的
///    期望值；
/// 2. `lib/services/rust/play_url_shadow.dart` —— 线上 shadow mode 与
///    Rust 输出比对。
///
/// 各写一份的话，fixture 可能常绿而线上天天报差异，或者反过来。
///
/// ## 它与 `PlayUrlModel.fromJson` 的关系
///
/// 本文件是那套模型解析规则（`lib/models/video/play/url.dart` 与
/// `lib/http/video.dart::videoUrl` 的分支）的**转写**，不是对模型实例的
/// 序列化：模型依赖 `package:flutter`，而 fixture 生成器是纯 dart 脚本。
///
/// 转写意味着可能与模型脱节。兜底在
/// `test/services/rust/play_url_normalize_test.dart`：它用**真正的**
/// `PlayUrlModel.fromJson` 解析同一份 fixture，逐字段核对本文件的输出。
///
/// 刻意**不**转写的部分：清晰度描述、音轨排序与「（AI）」后缀。那些是
/// 展示规则而非协议，Rust 侧同样没做（见 `bili-domain/src/playurl.rs`）。
///
/// 本文件**不得** import flutter——一旦引入，fixture 生成器就跑不了了。
library;

/// 响应里业务数据的位置，由 videoType 与是否降级决定。
///
/// 与 `lib/http/video.dart::videoUrl` 的那个 switch 一一对应；响应本身
/// 不足以区分（PUGV 降级与 UGC 的外层形状相同）。
enum PlayUrlShape {
  /// `data` 即流本体。UGC，以及 PUGV 降级（它另有业务信息给进度）。
  ugc,

  /// `data.video_info` + `data.watch_progress`。PGC/PUGV 主接口。
  playView,

  /// `result.video_info` + `result.play_view_business_info`。PGC 降级。
  pgcFallback,

  /// `data` 即流本体 + `data.play_view_business_info`。PUGV 降级。
  pugvFallback;

  /// 按基线的参数选择形状。
  static PlayUrlShape of(String videoType, bool fallback) =>
      switch ((videoType, fallback)) {
        ('ugc', _) => ugc,
        (_, false) => playView,
        ('pgc', true) => pgcFallback,
        _ => pugvFallback,
      };
}

/// 需要降级（DRM）时返回 null，与 Rust 的 `Outcome::NeedsFallback` 对应。
/// 响应本身无法解析（非 0 码、缺字段）时同样返回 null——规范化只负责
/// 成功响应，错误分类在 Rust 侧与 `VideoHttp` 各自处理。
Map<String, dynamic>? normalizePlayUrl(
  Map<String, dynamic> body,
  PlayUrlShape shape,
) {
  if (body['code'] != 0) return null;

  switch (shape) {
    case PlayUrlShape.ugc:
      final data = body['data'];
      if (data is! Map<String, dynamic>) return null;
      return _stream(data);

    case PlayUrlShape.playView:
      final data = body['data'];
      if (data is! Map<String, dynamic>) return null;
      final info = data['video_info'];
      if (info is! Map<String, dynamic>) return null;
      final out = _stream(info);
      final progress = data['watch_progress'];
      if (progress != null) {
        // 基线：外层进度覆盖流里的 last_play_time，且
        // current_watch_progress 优先于 current_progress。
        out['last_play_time'] = _progress(
          progress['current_watch_progress'] ?? progress['current_progress'],
        );
      }
      return out;

    case PlayUrlShape.pgcFallback:
      final result = body['result'];
      if (result is! Map<String, dynamic>) return null;
      final info = result['video_info'];
      final out = _stream(info is Map<String, dynamic> ? info : const {});
      out['last_play_time'] = _progress(_businessProgress(result));
      return out;

    case PlayUrlShape.pugvFallback:
      final data = body['data'];
      if (data is! Map<String, dynamic>) return null;
      final out = _stream(data);
      out['last_play_time'] = _progress(_businessProgress(data));
      return out;
  }
}

dynamic _businessProgress(Map<String, dynamic> holder) =>
    holder['play_view_business_info']?['user_status']?['watch_progress']?['current_watch_progress'];

/// 基线 `PlayUrlModel.lastPlayTime` 的 setter：null 与非正数一律为 0。
int _progress(dynamic value) => value is int && value > 0 ? value : 0;

Map<String, dynamic> _stream(Map<String, dynamic> json) => {
  'from': json['from'] as String?,
  'result': json['result'] as String?,
  'message': json['message'] as String?,
  'quality': json['quality'] as int?,
  'format': json['format'] as String?,
  'time_length': json['timelength'] as int?,
  'accept_format': json['accept_format'] as String?,
  'accept_description': (json['accept_description'] as List?)
      ?.map((e) => e as String)
      .toList(),
  'accept_quality': (json['accept_quality'] as List?)
      ?.map((e) => e as int)
      .toList(),
  'video_codecid': json['video_codecid'] as int?,
  'seek_param': json['seek_param'] as String?,
  'seek_type': json['seek_type'] as String?,
  'support_formats': (json['support_formats'] as List?)
      ?.map(_format)
      .toList(),
  'dash': _dash(json['dash']),
  'durl': (json['durl'] as List?)?.map(_durl).toList(),
  'volume': _volume(json['volume']),
  'last_play_time': _progress(json['last_play_time']),
  'last_play_cid': json['last_play_cid'] as int?,
  'cur_language': json['cur_language'] as String?,
  'language': _language(json['language']),
  'clip_info_list': (json['clip_info_list'] as List?)?.map(_clip).toList(),
};

Map<String, dynamic> _format(dynamic json) => {
  'quality': json['quality'] as int?,
  'format': json['format'] as String?,
  'new_desc': json['new_description'] as String?,
  'display_desc': json['display_desc'] as String?,
  'codecs': (json['codecs'] as List?)?.map((e) => e as String).toList(),
};

Map<String, dynamic>? _dash(dynamic json) {
  if (json == null) return null;
  // 基线 Dash.fromJson：flac → dolby → 普通，顺序即优先级（首项是默认
  // 音轨）。三处都空时整个 audio 为 null，而不是空列表。
  final audio = <Map<String, dynamic>>[
    if (json['flac']?['audio'] case Map<String, dynamic> flac) _mediaStream(flac),
    if (json['dolby']?['audio'] case List dolby) ...dolby.map(_mediaStream),
    if (json['audio'] case List plain) ...plain.map(_mediaStream),
  ];

  return {
    'duration': json['duration'] as int?,
    'min_buffer_time': (json['minBufferTime'] as num?)?.toDouble(),
    'video': (json['video'] as List?)?.map(_mediaStream).toList(),
    'audio': audio.isEmpty ? null : audio,
  };
}

/// 一路流。
///
/// 接口在 DASH 内层用 camelCase，在别处用 snake_case；基线对两者都认
/// （`BaseItem.fromJson` 的 `??` 链），Rust 侧用 serde 的 alias 做同样的
/// 事。
Map<String, dynamic> _mediaStream(dynamic json) => {
  // 基线把 id 声明成非空 late int，缺失时抛异常；Rust 用 0 兜底，让一路
  // 「未知清晰度」的流不至于毁掉整次取流。此处与 Rust 对齐。
  'id': (json['id'] as int?) ?? 0,
  'base_url': (json['baseUrl'] ?? json['base_url']) as String?,
  'backup_url': ((json['backupUrl'] ?? json['backup_url']) as List?)
      ?.map((e) => e as String)
      .toList(),
  'band_width': (json['bandWidth'] ?? json['bandwidth']) as int?,
  'mime_type': json['mime_type'] as String?,
  'codecs': json['codecs'] as String?,
  'width': json['width'] as int?,
  'height': json['height'] as int?,
  'frame_rate': (json['frameRate'] ?? json['frame_rate']) as String?,
  'sar': json['sar'] as String?,
  'start_with_sap': (json['startWithSap'] ?? json['start_with_sap']) as int?,
  'segment_base': _segmentBase(json['segmentBase'] ?? json['segment_base']),
  'codecid': json['codecid'] as int?,
};

Map<String, dynamic>? _segmentBase(dynamic json) {
  if (json == null) return null;
  return {
    'initialization': (json['Initialization'] ?? json['initialization'])
        as String?,
    'index_range': (json['indexRange'] ?? json['index_range']) as String?,
  };
}

Map<String, dynamic> _durl(dynamic json) => {
  'order': json['order'] as int?,
  'length': json['length'] as int?,
  'size': json['size'] as int?,
  'ahead': json['ahead'] as String?,
  'vhead': json['vhead'] as String?,
  'url': json['url'] as String?,
  'backup_url': (json['backup_url'] as List?)
      ?.map((e) => e as String)
      .toList(),
};

/// 基线对每个字段都用了 `?? 0`——`Volume.isNotEmpty` 正是靠「是否全为
/// 0」决定要不要启用均衡。
Map<String, dynamic>? _volume(dynamic json) {
  if (json == null) return null;
  double v(String key) => ((json[key] as num?) ?? 0).toDouble();
  return {
    'measured_i': v('measured_i'),
    'measured_lra': v('measured_lra'),
    'measured_tp': v('measured_tp'),
    'measured_threshold': v('measured_threshold'),
    'target_offset': v('target_offset'),
    'target_i': v('target_i'),
    'target_tp': v('target_tp'),
  };
}

Map<String, dynamic>? _language(dynamic json) {
  if (json == null) return null;
  return {
    'support': json['support'] as bool?,
    // 未排序：排序与「（AI）」后缀是展示规则，见库文档。
    'items': (json['items'] as List?)
        ?.map(
          (e) => {
            'lang': e['lang'] as String?,
            'title': e['title'] as String?,
            'subtitle_lang': e['subtitle_lang'] as String?,
            'production_type': e['production_type'] as int?,
          },
        )
        .toList(),
  };
}

Map<String, dynamic> _clip(dynamic json) => {
  // 保持接口单位（秒）。基线转毫秒是在模型里做的。
  'start': (json['start'] as num?)?.toDouble(),
  'end': (json['end'] as num?)?.toDouble(),
  'clip_type': (json['clipType'] ?? json['clip_type']) as String?,
};
