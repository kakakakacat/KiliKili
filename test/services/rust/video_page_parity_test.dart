import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

/// S3 第二批（issue #53）的一次性基线对照，机制见 ADR-011 决策五。
///
/// 与第一批同样的处境：`AiConclusionData`、`VideoShotData`、`PlayInfoData`、
/// `VideoNoteData` 都已随本次迁移删除，没有「还活着的基线模型」可借，于是
/// 本文件自带 oracle——下面的 `_Baseline*` 是迁移前那几个 `fromJson` 的
/// **逐行转写**，取自合并前的 `main`（`096357a08`）：
///
/// - `lib/models_new/video/video_ai_conclusion/*.dart`
/// - `lib/models_new/video/video_shot/data.dart`（含 `http2https`）
/// - `lib/models_new/video/video_play_info/{data,subtitle_info,subtitle,
///   view_point,interaction}.dart`
/// - `lib/models_new/video/video_note_list/*.dart`（含 `Vip.fromJson` 的
///   `status ?? 0`）
///
/// 转写时不许顺手修正任何怪异之处：它要证明的正是 Rust 与迁移前行为一致。
///
/// **两处刻意的不一致**，在断言里显式换算，不是漏网：
///
/// 1. 字幕的 `lanDoc`：基线在模型里就补了「（AI）」后缀，迁移后那一步留在
///    Dart（`VideoPageCompat`），所以快照里是服务端的原值。
/// 2. 字幕顺序：基线构造完就按「中文优先、AI 靠后」排序，迁移后同样留在
///    Dart，快照保持服务端顺序。因此这里按 id 配对比较，不比顺序。
///
/// `ai_conclusion` 与 `play_info_rich` 两份 fixture 是**手工构造**的
/// （匿名拿不到登录态才有的那几段），对照它们证明的是映射一致，不能证明
/// 我们对协议的理解正确——那要等 Phase 2 的真机验证。
void main() {
  const dir = 'rust/crates/bili-api/tests/fixtures/video_page';

  Map<String, dynamic> read(String name) =>
      jsonDecode(File('$dir/$name').readAsStringSync()) as Map<String, dynamic>;

  test('AI 总结：Rust 快照与基线逐字段一致', () {
    final data = read('ai_conclusion.raw.json')['data'] as Map<String, dynamic>;
    final snapshot = read('ai_conclusion.rust.json');
    final baseline = _BaselineAiConclusion.fromJson(data);

    expect(snapshot['summary'], baseline.summary);
    final outline = snapshot['outline'] as List?;
    expect(outline?.length, baseline.outline?.length);
    for (var i = 0; i < (baseline.outline?.length ?? 0); i++) {
      final b = baseline.outline![i];
      final r = outline![i] as Map<String, dynamic>;
      expect(r['title'], b.title, reason: '第 $i 段标题');
      final parts = r['part_outline'] as List?;
      expect(parts?.length, b.partOutline?.length, reason: '第 $i 段条数');
      for (var j = 0; j < (b.partOutline?.length ?? 0); j++) {
        expect(parts![j]['timestamp'], b.partOutline![j].timestamp);
        expect(parts[j]['content'], b.partOutline![j].content);
      }
    }
  });

  test('预览缩略图：Rust 快照与基线逐字段一致（含 http2https）', () {
    final data = read('video_shot.raw.json')['data'] as Map<String, dynamic>;
    final snapshot = read('video_shot.rust.json');
    final b = _BaselineVideoShot.fromJson(data);

    expect(snapshot['pvdata'], b.pvdata, reason: 'pvdata 基线不做 https 转换');
    expect(snapshot['img_x_len'], b.imgXLen);
    expect(snapshot['img_y_len'], b.imgYLen);
    expect(snapshot['img_x_size'], b.imgXSize);
    expect(snapshot['img_y_size'], b.imgYSize);
    expect(snapshot['image'] ?? snapshot['images'], b.image);
    expect((snapshot['index'] as List).cast<int>(), b.index);
  });

  test('播放信息（真实样本）：Rust 快照与基线逐字段一致', () {
    _expectPlayInfoMatches(
      read('play_info.raw.json')['data'] as Map<String, dynamic>,
      read('play_info.rust.json'),
    );
  });

  test('播放信息（手工构造，带字幕/看点/互动）：与基线逐字段一致', () {
    _expectPlayInfoMatches(
      read('play_info_rich.raw.json')['data'] as Map<String, dynamic>,
      read('play_info_rich.rust.json'),
    );
  });

  test('笔记列表：Rust 快照与基线逐字段一致', () {
    final data = read('note_list.raw.json')['data'] as Map<String, dynamic>;
    final snapshot = read('note_list.rust.json');
    final b = _BaselineNoteList.fromJson(data);

    expect(snapshot['total'], b.total);
    final list = snapshot['list'] as List?;
    expect(list?.length, b.list?.length);
    for (var i = 0; i < (b.list?.length ?? 0); i++) {
      final item = b.list![i];
      final r = list![i] as Map<String, dynamic>;
      final where = '第 $i 条（${item.cvid}）';
      expect(r['cvid'], item.cvid, reason: '$where: cvid');
      expect(r['summary'], item.summary, reason: '$where: 摘要');
      expect(r['pubtime'], item.pubtime, reason: '$where: 时间');

      final a = r['author'] as Map<String, dynamic>?;
      expect(a?['mid'], item.mid, reason: '$where: mid');
      expect(a?['name'], item.name, reason: '$where: 昵称');
      expect(a?['face'], item.face, reason: '$where: 头像');
      expect(a?['level'], item.level, reason: '$where: 等级');
      expect(a?['is_senior_member'], item.isSeniorMember, reason: '$where: 硬核');
      expect(a?['vip_type'], item.vipType, reason: '$where: 会员类型');
      // 基线 Vip.fromJson 缺 status 时填 0，不是 null。
      expect(a?['vip_status'], item.vipStatus, reason: '$where: 会员状态');
      expect(a?['vip_label_text'], item.vipLabelText, reason: '$where: 会员标签');
      expect(a?['pendant_image'], item.pendantImage, reason: '$where: 挂件');
      expect(a?['official_type'], item.officialType, reason: '$where: 认证类型');
      expect(a?['official_desc'], item.officialDesc, reason: '$where: 认证说明');
    }
  });
}

void _expectPlayInfoMatches(
  Map<String, dynamic> raw,
  Map<String, dynamic> snapshot,
) {
  final b = _BaselinePlayInfo.fromJson(raw);

  expect(snapshot['last_play_cid'], b.lastPlayCid, reason: '续播 cid');

  // 看点：顺序与字段都应一致。
  final vps = snapshot['view_points'] as List?;
  expect(vps?.length, b.viewPoints?.length, reason: '看点条数');
  for (var i = 0; i < (b.viewPoints?.length ?? 0); i++) {
    final bv = b.viewPoints![i];
    final r = vps![i] as Map<String, dynamic>;
    expect(r['kind'], bv.type, reason: '第 $i 个看点类型');
    expect(r['from'], bv.from, reason: '第 $i 个看点起点');
    expect(r['to'], bv.to, reason: '第 $i 个看点终点');
    expect(r['content'], bv.content, reason: '第 $i 个看点文案');
    expect(r['img_url'], bv.imgUrl, reason: '第 $i 个看点图');
  }

  // 字幕：按 id 配对比，不比顺序（排序留在 Dart，见文件头）。
  final subs = (snapshot['subtitles'] as List?) ?? const [];
  expect(subs.length, b.subtitles?.length ?? 0, reason: '字幕条数');
  for (final bs in b.subtitles ?? const <_BaselineSubtitle>[]) {
    final r = subs.cast<Map<String, dynamic>>().firstWhere(
      (e) => e['id'] == bs.id,
    );
    expect(r['lan'], bs.lan, reason: '字幕 ${bs.id} 语言');
    expect(r['kind'], bs.type, reason: '字幕 ${bs.id} 类型');
    expect(r['subtitle_url'], bs.subtitleUrl, reason: '字幕 ${bs.id} 地址');
    // 快照里是服务端原值，基线补过「（AI）」；在这里补回来再比。
    final isAi = r['kind'] == 1;
    expect(
      '${r['lan_doc']}${isAi ? '（AI）' : ''}',
      bs.lanDoc,
      reason: '字幕 ${bs.id} 语言名',
    );
  }

  final interaction = snapshot['interaction'] as Map<String, dynamic>?;
  expect(interaction?['graph_version'], b.graphVersion, reason: '剧情树版本');
  final node = interaction?['history_node'] as Map<String, dynamic>?;
  expect(node?['node_id'], b.nodeId, reason: '剧情节点 id');
  expect(node?['title'], b.nodeTitle, reason: '剧情节点标题');
  expect(node?['cid'], b.nodeCid, reason: '剧情节点 cid');
}

// ---------------------------------------------------------------------------
// 以下是迁移前 fromJson 的逐行转写，见文件头。
// ---------------------------------------------------------------------------

final _http = RegExp("^(http:)?//", caseSensitive: false);
String _http2https(String? s) => s?.replaceFirst(_http, "https://") ?? '';

class _BaselineAiConclusion {
  String? summary;
  List<_BaselineOutline>? outline;

  _BaselineAiConclusion.fromJson(Map<String, dynamic> json) {
    final result = json['model_result'] as Map<String, dynamic>?;
    summary = result?['summary'] as String?;
    outline = (result?['outline'] as List<dynamic>?)
        ?.map((e) => _BaselineOutline.fromJson(e as Map<String, dynamic>))
        .toList();
  }
}

class _BaselineOutline {
  String? title;
  List<_BaselinePartOutline>? partOutline;

  _BaselineOutline.fromJson(Map<String, dynamic> json) {
    title = json['title'] as String?;
    partOutline = (json['part_outline'] as List<dynamic>?)
        ?.map((e) => _BaselinePartOutline.fromJson(e as Map<String, dynamic>))
        .toList();
  }
}

class _BaselinePartOutline {
  int? timestamp;
  String? content;

  _BaselinePartOutline.fromJson(Map<String, dynamic> json) {
    timestamp = json['timestamp'] as int?;
    content = json['content'] as String?;
  }
}

class _BaselineVideoShot {
  String? pvdata;
  late int imgXLen;
  late int imgYLen;
  late double imgXSize;
  late double imgYSize;
  late List<String> image;
  late List<int> index;

  _BaselineVideoShot.fromJson(Map<String, dynamic> json) {
    pvdata = json["pvdata"];
    imgXLen = json["img_x_len"];
    imgYLen = json["img_y_len"];
    imgXSize = (json["img_x_size"] as num).toDouble();
    imgYSize = (json["img_y_size"] as num).toDouble();
    image = (json["image"] as List)
        .map((e) => _http2https(e as String))
        .toList();
    index = (json["index"] as List).cast<int>();
  }
}

class _BaselinePlayInfo {
  int? lastPlayCid;
  List<_BaselineSubtitle>? subtitles;
  List<_BaselineViewPoint>? viewPoints;
  int? graphVersion;
  int? nodeId;
  String? nodeTitle;
  int? nodeCid;

  _BaselinePlayInfo.fromJson(Map<String, dynamic> json) {
    lastPlayCid = json['last_play_cid'] as int?;

    // SubtitleInfo.fromJson：只有非空列表才构造，并就地排序。
    if ((json['subtitle'] as Map<String, dynamic>?)?['subtitles']
        case final List list when list.isNotEmpty) {
      subtitles =
          list
              .map((e) => _BaselineSubtitle.fromJson(e as Map<String, dynamic>))
              .toList()
            ..sort();
    }

    viewPoints = (json['view_points'] as List?)
        ?.map((e) => _BaselineViewPoint.fromJson(e as Map<String, dynamic>))
        .toList();

    final interaction = json['interaction'] as Map<String, dynamic>?;
    graphVersion = interaction?['graph_version'] as int?;
    final node = interaction?['history_node'] as Map<String, dynamic>?;
    nodeId = node?['node_id'] as int?;
    nodeTitle = node?['title'] as String?;
    nodeCid = node?['cid'] as int?;
  }
}

class _BaselineSubtitle implements Comparable<_BaselineSubtitle> {
  int? id;
  late String lan;
  String? lanDoc;
  String? subtitleUrl;
  int? type;
  bool isAi = false;

  _BaselineSubtitle.fromJson(Map<String, dynamic> json) {
    id = json["id"];
    lan = json["lan"];
    type = json["type"];
    isAi = type == 1;
    lanDoc = '${json["lan_doc"]}${isAi ? '（AI）' : ''}';
    subtitleUrl = json["subtitle_url"];
  }

  @override
  int compareTo(_BaselineSubtitle other) {
    final thisHasZh = lan.contains('zh');
    final otherHasZh = other.lan.contains('zh');
    if (thisHasZh != otherHasZh) return thisHasZh ? -1 : 1;
    if (isAi != other.isAi) return isAi ? 1 : -1;
    return 0;
  }
}

class _BaselineViewPoint {
  int? type;
  int? from;
  int? to;
  String? content;
  String? imgUrl;

  _BaselineViewPoint.fromJson(Map<String, dynamic> json) {
    type = json["type"];
    from = json["from"];
    to = json["to"];
    content = json["content"];
    imgUrl = json["imgUrl"];
  }
}

class _BaselineNoteList {
  int? total;
  List<_BaselineNoteItem>? list;

  _BaselineNoteList.fromJson(Map<String, dynamic> json) {
    total = (json['page'] as Map<String, dynamic>?)?['total'] as int?;
    list = (json['list'] as List<dynamic>?)
        ?.map((e) => _BaselineNoteItem.fromJson(e as Map<String, dynamic>))
        .toList();
  }
}

class _BaselineNoteItem {
  int? cvid;
  String? summary;
  String? pubtime;
  int? mid;
  String? name;
  String? face;
  int? level;
  int? isSeniorMember;
  int? vipType;
  int? vipStatus;
  String? vipLabelText;
  String? pendantImage;
  int? officialType;
  String? officialDesc;

  _BaselineNoteItem.fromJson(Map<String, dynamic> json) {
    cvid = json['cvid'] as int?;
    summary = json['summary'] as String?;
    pubtime = json['pubtime'] as String?;

    final author = json['author'] as Map<String, dynamic>?;
    mid = author?['mid'] as int?;
    name = author?['name'] as String?;
    face = author?['face'] as String?;
    level = author?['level'] as int?;
    isSeniorMember = author?['is_senior_member'] as int?;

    final vip = author?['vip_info'] as Map<String, dynamic>?;
    // Vip.fromJson：type 兼容 vipType，status 缺省填 0。
    vipType = (vip?['type'] ?? vip?['vipType']) as int?;
    vipStatus = vip == null
        ? null
        : (vip['status'] ?? vip['vipStatus'] ?? 0) as int?;
    vipLabelText = (vip?['label'] as Map<String, dynamic>?)?['text'] as String?;

    pendantImage =
        (author?['pendant'] as Map<String, dynamic>?)?['image'] as String?;
    final official = author?['official'] as Map<String, dynamic>?;
    officialType = official?['type'] as int?;
    officialDesc = official?['desc'] as String?;
  }
}
