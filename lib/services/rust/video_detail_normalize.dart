/// 视频详情响应的**规范化**：把 `/x/web-interface/wbi/view` 的 `data`
/// 转成与 Rust 侧 `bili_domain::video::VideoDetail` 逐字段对应的 Map。
///
/// 两个使用者：
///
/// 1. `tool/fixture-gen/video_detail_fixture.dart` —— 产出 golden fixture
///    的期望值；
/// 2. `lib/services/rust/video_detail_shadow.dart` —— 线上 shadow mode
///    与 Rust 输出比对。
///
/// 二者必须用**同一份**规范化规则，否则 fixture 绿着、线上却天天报差异，
/// 或者反过来，差异在 CI 里永远看不见。
///
/// ## 它与 `VideoDetailData.fromJson` 的关系
///
/// 本文件是那套模型解析规则（`lib/models_new/video/video_detail/`、
/// `lib/utils/parse_int.dart`、`lib/utils/parse_string.dart`）的**转写**，
/// 而不是对模型实例的序列化。原因：模型依赖 `package:flutter` 与
/// `hive_ce`（`Owner` 是 HiveType），fixture 生成器是纯 dart 脚本，跑不起
/// 那些依赖。
///
/// 转写意味着可能与模型脱节。兜底在
/// `test/services/rust/video_detail_normalize_test.dart`：它用**真正的**
/// `VideoDetailData.fromJson` 解析同一份 fixture，逐字段核对本文件的输出。
/// 改模型时若忘了同步这里，那个测试会红。
///
/// 本文件**不得** import flutter 或 hive——一旦引入，fixture 生成器就跑
/// 不了了。
library;

// ── 基线工具函数（转写自 lib/utils/） ──────────────────────────────

int? _safeToInt(dynamic value) => switch (value) {
  int _ => value,
  String _ => int.tryParse(value),
  num _ => value.toInt(),
  _ => null,
};

String? _nonNullOrEmptyString(String? value) {
  if (value == null || value.isEmpty) return null;
  return value;
}

// ── 规范化（转写自 lib/models_new/video/video_detail/） ─────────────

Map<String, dynamic>? _dimension(dynamic json) {
  if (json == null) return null;
  // dimension.dart: rotate == 1 时交换宽高
  final rotated = json['rotate'] == 1;
  return {
    'width': (rotated ? json['height'] : json['width']) as int?,
    'height': (rotated ? json['width'] : json['height']) as int?,
  };
}

Map<String, dynamic>? _owner(dynamic json) {
  if (json == null) return null;
  // model_owner.dart: mid 走 safeToInt
  return {
    'mid': _safeToInt(json['mid']),
    'name': json['name'] as String?,
    'face': json['face'] as String?,
  };
}

Map<String, dynamic>? _stat(dynamic json) {
  if (json == null) return null;
  // stat.dart: favorite / coin / like 缺失时为 0，其余保持可空
  return {
    'view': json['view'] as int?,
    'danmaku': json['danmaku'] as int?,
    'reply': json['reply'] as int?,
    'favorite': (json['favorite'] as int?) ?? 0,
    // coin 在基线里是 num：服务端若给浮点，Dart 的 toInt() 是截断。
    'coin': ((json['coin'] as num?) ?? 0).toInt(),
    'share': json['share'] as int?,
    'like': (json['like'] as int?) ?? 0,
  };
}

Map<String, dynamic>? _part(dynamic json) {
  if (json == null) return null;
  // page.dart
  return {
    'cid': json['cid'] as int?,
    'page': json['page'] as int?,
    'from': json['from'] as String?,
    'part': json['part'] as String?,
    'duration': json['duration'] as int?,
    'vid': json['vid'] as String?,
    'dimension': _dimension(json['dimension']),
    'ctime': json['ctime'] as int?,
    'first_frame': json['first_frame'] as String?,
    'badge': json['badge'] as String?,
  };
}

Map<String, dynamic>? _arc(dynamic json) {
  if (json == null) return null;
  // arc.dart
  return {
    'aid': json['aid'] as int?,
    'pic': json['pic'] as String?,
    'title': json['title'] as String?,
    'pubdate': json['pubdate'] as int?,
    'duration': json['duration'] as int?,
    'author': _owner(json['author']),
    'stat': _stat(json['stat']),
    'dimension': _dimension(json['dimension']),
  };
}

Map<String, dynamic> _episode(dynamic json) {
  // episode.dart
  return {
    'season_id': json['season_id'] as int?,
    'section_id': json['section_id'] as int?,
    'id': json['id'] as int?,
    'aid': json['aid'] as int?,
    'bvid': json['bvid'] as String?,
    'cid': json['cid'] as int?,
    'title': json['title'] as String?,
    'attribute': json['attribute'] as int?,
    'badge': json['badge'] as String?,
    'arc': _arc(json['arc']),
    'page': _part(json['page']),
    'pages': (json['pages'] as List<dynamic>?)?.map(_part).toList(),
  };
}

Map<String, dynamic>? _ugcSeason(dynamic json) {
  if (json == null) return null;
  // ugc_season.dart + section.dart
  return {
    'id': json['id'] as int?,
    'title': json['title'] as String?,
    'cover': json['cover'] as String?,
    'mid': json['mid'] as int?,
    'sections': (json['sections'] as List<dynamic>?)
        ?.map(
          (s) => {
            'season_id': s['season_id'] as int?,
            'id': s['id'] as int?,
            'title': s['title'] as String?,
            'type': s['type'] as int?,
            'episodes': (s['episodes'] as List<dynamic>?)
                ?.map(_episode)
                .toList(),
          },
        )
        .toList(),
  };
}

Map<String, dynamic> _staff(dynamic json) {
  // staff.dart + model_avatar.dart(Vip / BaseOfficialVerify)
  final vip = json['vip'];
  final official = json['official'];
  return {
    // 基线此字段是 dynamic，规范化时收敛为整数（见 bili-domain::video::Staff）
    'mid': _safeToInt(json['mid']),
    'title': json['title'] as String?,
    'name': json['name'] as String?,
    'face': json['face'] as String?,
    'vip_type': vip == null ? null : (vip['type'] ?? vip['vipType']) as int?,
    'vip_status': vip == null
        ? 0
        : ((vip['status'] ?? vip['vipStatus']) as int?) ?? 0,
    'official_type': official == null ? null : official['type'] as int?,
    'official_desc': official == null ? null : official['desc'] as String?,
  };
}

/// 规范化一份详情 `data`。入参是服务端响应里 `data` 字段的原始 Map。
Map<String, dynamic> normalizeVideoDetail(Map<String, dynamic> data) {
  // data.dart
  return {
    'bvid': data['bvid'] as String?,
    'aid': data['aid'] as int?,
    'videos': data['videos'] as int?,
    'copyright': data['copyright'] as int?,
    'pic': data['pic'] as String?,
    'title': data['title'] as String?,
    'pubdate': data['pubdate'] as int?,
    'ctime': data['ctime'] as int?,
    'desc': data['desc'] as String?,
    'desc_v2': (data['desc_v2'] as List<dynamic>?)
        ?.map(
          (e) => {
            'raw_text': e['raw_text'] as String?,
            'type': e['type'] as int?,
            'biz_id': e['biz_id'] as int?,
          },
        )
        .toList(),
    'duration': data['duration'] as int?,
    // rights.dart 只取了这一个字段
    'is_stein_gate': data['rights'] == null
        ? null
        : data['rights']['is_stein_gate'] as int?,
    'owner': _owner(data['owner']),
    'stat': _stat(data['stat']),
    // argue_info.dart 只取了这一个字段
    'argue_msg': data['argue_info'] == null
        ? null
        : data['argue_info']['argue_msg'] as String?,
    'cid': data['cid'] as int?,
    'dimension': _dimension(data['dimension']),
    'season_id': data['season_id'] as int?,
    'is_upower_exclusive': data['is_upower_exclusive'] as bool?,
    'pages': (data['pages'] as List<dynamic>?)?.map(_part).toList(),
    'ugc_season': _ugcSeason(data['ugc_season']),
    'staff': (data['staff'] as List<dynamic>?)?.map(_staff).toList(),
    'redirect_url': _nonNullOrEmptyString(data['redirect_url'] as String?),
  };
}
