import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

/// S3（issue #53）的**一次性基线对照**，机制见 ADR-011 决策五。
///
/// 五条视频流接口已迁到 Rust，`HotVideoItemModel.fromJson` 与
/// `PopularSeriesOneData` 等模型随之删除——于是没有「还活着的基线模型」
/// 可借，和 SponsorBlock 样板那次不同。
///
/// 因此本文件自带 oracle：下面的 `_Baseline*` 是迁移前那几个 `fromJson`
/// 的**逐行转写**，取自合并前的 `main`（`9df6de51a`）：
///
/// - `lib/models/model_hot_video_item.dart` 的 `HotVideoItemModel.fromJson`
///   与 `HotStat.fromJson`
/// - `lib/models/model_owner.dart` 的 `Owner.fromJson`
/// - `lib/models_new/video/video_detail/dimension.dart` 的
///   `Dimension.fromJson`
/// - `lib/models_new/popular/*` 的三个 `fromJson`
///
/// 转写时**不许顺手修正**任何怪异之处（例如 `safeToInt` 对字符串 mid 的
/// 容忍）：它要证明的正是 Rust 与迁移前的行为一致。
///
/// **这个测试是有寿命的**：它与 `rust/crates/bili-api/tests/fixtures/
/// video_feed/*.rust.json` 快照配对，证明快照不是「Rust 的自我验证」。
/// 快照是长期资产，本文件不是——S3 的行为一旦在 Phase 2 真机回归里确认
/// 过，就可以删掉它。
void main() {
  const dir = 'rust/crates/bili-api/tests/fixtures/video_feed';

  Map<String, dynamic> readJson(String name) =>
      jsonDecode(File('$dir/$name').readAsStringSync()) as Map<String, dynamic>;

  List<dynamic> readList(String name) =>
      jsonDecode(File('$dir/$name').readAsStringSync()) as List<dynamic>;

  void expectItemsMatch(List<dynamic> rawItems, List<dynamic> snapshot) {
    final baseline = rawItems
        .map((e) => _BaselineItem.fromJson(e as Map<String, dynamic>))
        .toList();

    expect(snapshot.length, baseline.length, reason: '条数不同');

    for (var i = 0; i < baseline.length; i++) {
      final b = baseline[i];
      final r = snapshot[i] as Map<String, dynamic>;
      final where = '第 $i 条（${b.bvid}）';

      expect(r['aid'], b.aid, reason: '$where: aid');
      expect(r['cid'], b.cid, reason: '$where: cid');
      expect(r['bvid'], b.bvid, reason: '$where: bvid');
      expect(r['videos'], b.videos, reason: '$where: 分P数');
      expect(r['tid'], b.tid, reason: '$where: 分区 id');
      expect(r['tname'], b.tname, reason: '$where: 分区名');
      expect(r['copyright'], b.copyright, reason: '$where: 版权');
      expect(r['cover'], b.cover, reason: '$where: 封面');
      expect(r['title'], b.title, reason: '$where: 标题');
      expect(r['pubdate'], b.pubdate, reason: '$where: 发布时间');
      expect(r['ctime'], b.ctime, reason: '$where: 投稿时间');
      expect(r['desc'], b.desc, reason: '$where: 简介');
      expect(r['state'], b.state, reason: '$where: 状态');
      expect(r['first_frame'], b.firstFrame, reason: '$where: 首帧');
      expect(r['pub_location'], b.pubLocation, reason: '$where: 发布地区');
      expect(r['redirect_url'], b.redirectUrl, reason: '$where: 跳转地址');
      expect(r['progress'], b.progress, reason: '$where: 进度');

      // 基线缺字段时填 -1，Rust 用 null 表示同一件事；换算关系钉死在这里。
      expect(
        (r['duration'] as num?)?.toInt() ?? -1,
        b.duration,
        reason: '$where: 时长',
      );

      final owner = r['owner'] as Map<String, dynamic>;
      expect(owner['mid'], b.ownerMid, reason: '$where: UP mid');
      expect(owner['name'], b.ownerName, reason: '$where: UP 昵称');
      expect(owner['face'], b.ownerFace, reason: '$where: UP 头像');

      final stat = r['stat'] as Map<String, dynamic>;
      expect(stat['view'], b.view, reason: '$where: 播放量');
      expect(stat['like'], b.like, reason: '$where: 点赞');
      expect(stat['danmaku'], b.danmaku, reason: '$where: 弹幕');
      expect(stat['reply'], b.reply, reason: '$where: 评论');
      expect(stat['favorite'], b.favorite, reason: '$where: 收藏');
      expect(stat['coin'], b.coin, reason: '$where: 硬币');
      expect(stat['share'], b.share, reason: '$where: 分享');
      expect(stat['now_rank'], b.nowRank, reason: '$where: 当前排名');
      expect(stat['his_rank'], b.hisRank, reason: '$where: 历史排名');
      expect(stat['dislike'], b.dislike, reason: '$where: 点踩');
      expect(stat['vt'], b.vt, reason: '$where: vt');
      expect(stat['vv'], b.vv, reason: '$where: vv');

      final dim = r['dimension'] as Map<String, dynamic>?;
      expect(dim?['width'], b.width, reason: '$where: 宽（已按 rotate 归正）');
      expect(dim?['height'], b.height, reason: '$where: 高（已按 rotate 归正）');

      // 基线把三级优先级压成一个 badge 字符串；Rust 带回的是「哪一级」，
      // 文案留给 Dart。两者的映射在 HotVideoItemModel.fromRust 里，这里
      // 按同一张表比对。
      final badge = switch (r['badge_kind'] as String?) {
        'charging' => '充电专属',
        'cooperation' => '合作',
        _ => r['pgc_label'] as String?,
      };
      expect(badge, b.badge, reason: '$where: 角标');
    }
  }

  test('热门：Rust 快照与基线逐字段一致', () {
    expectItemsMatch(
      readJson('hot.raw.json')['data']['list'] as List<dynamic>,
      readList('hot.rust.json'),
    );
  });

  test('排行：Rust 快照与基线逐字段一致', () {
    expectItemsMatch(
      readJson('rank.raw.json')['data']['list'] as List<dynamic>,
      readList('rank.rust.json'),
    );
  });

  test('相关推荐：Rust 快照与基线逐字段一致', () {
    expectItemsMatch(
      readJson('related.raw.json')['data'] as List<dynamic>,
      readList('related.rust.json'),
    );
  });

  test('每周必看一期：config / reminder / list 与基线一致', () {
    final data =
        readJson('series_one.raw.json')['data'] as Map<String, dynamic>;
    final snapshot = readJson('series_one.rust.json');

    // 基线 PopularSeriesOneData.fromJson
    final config = data['config'] as Map<String, dynamic>?;
    final snapConfig = snapshot['config'] as Map<String, dynamic>?;
    expect(snapConfig?['name'], config?['name'] as String?);
    expect(snapConfig?['label'], config?['label'] as String?);
    expect(snapConfig?['media_id'], config?['media_id'] as int?);
    expect(snapshot['reminder'], data['reminder'] as String?);

    expectItemsMatch(
      data['list'] as List<dynamic>,
      snapshot['list'] as List<dynamic>,
    );
  });

  test('每周必看期号列表：与基线一致', () {
    final list = readJson('series_list.raw.json')['data']['list'] as List;
    final snapshot = readList('series_list.rust.json');
    expect(snapshot.length, list.length);
    for (var i = 0; i < list.length; i++) {
      final raw = list[i] as Map<String, dynamic>;
      final r = snapshot[i] as Map<String, dynamic>;
      // 基线 PopularSeriesListItem 只取 number 与 name；subject/status 是
      // Rust 多带回来的，没有基线可比，只断言它们没把那两个字段挤掉。
      expect(r['number'], raw['number'] as int?, reason: '第 $i 期: 期号');
      expect(r['name'], raw['name'] as String?, reason: '第 $i 期: 名称');
    }
  });

  test('入站必刷：media_id 与 list 与基线一致', () {
    final data = readJson('precious.raw.json')['data'] as Map<String, dynamic>;
    final snapshot = readJson('precious.rust.json');
    expect(snapshot['media_id'], data['media_id'] as int?);
    expectItemsMatch(
      data['list'] as List<dynamic>,
      snapshot['list'] as List<dynamic>,
    );
  });
}

/// 迁移前 `HotVideoItemModel.fromJson` + `Owner.fromJson` +
/// `HotStat.fromJson` + `Dimension.fromJson` 的逐行转写（见文件头）。
class _BaselineItem {
  int? aid;
  int? cid;
  String? bvid;
  int? videos;
  int? tid;
  String? tname;
  int? copyright;
  String? cover;
  String? title;
  int? pubdate;
  int? ctime;
  String? desc;
  int? state;
  int duration = -1;
  int? ownerMid;
  String? ownerName;
  String? ownerFace;
  int? view;
  int? like;
  int? danmaku;
  int? reply;
  int? favorite;
  num? coin;
  int? share;
  int? nowRank;
  int? hisRank;
  int? dislike;
  int? vt;
  int? vv;
  int? width;
  int? height;
  String? firstFrame;
  String? pubLocation;
  String? redirectUrl;
  num? progress;
  String? badge;

  _BaselineItem.fromJson(Map<String, dynamic> json) {
    aid = json['aid'];
    cid = json['cid'];
    bvid = json['bvid'];
    videos = json['videos'];
    tid = json['tid'];
    tname = json['tname'];
    copyright = json['copyright'];
    cover = json['pic'];
    title = json['title'];
    pubdate = json['pubdate'];
    ctime = json['ctime'];
    desc = json['desc'];
    state = json['state'];
    duration = json['duration'] ?? -1;

    final owner = json['owner'] as Map<String, dynamic>?;
    // 基线 Owner.fromJson 用 safeToInt：mid 偶尔是字符串。
    ownerMid = _safeToInt(owner?['mid']);
    ownerName = owner?['name'];
    ownerFace = owner?['face'];

    final stat = json['stat'] as Map<String, dynamic>?;
    view = _safeToInt(stat?['view']);
    like = stat?['like'];
    danmaku = stat?['danmaku'];
    reply = stat?['reply'];
    favorite = stat?['favorite'];
    coin = stat?['coin'];
    share = stat?['share'];
    nowRank = stat?['now_rank'];
    hisRank = stat?['his_rank'];
    dislike = stat?['dislike'];
    vt = stat?['vt'];
    vv = stat?['vv'];

    final dimension = json['dimension'] as Map<String, dynamic>?;
    if (dimension != null) {
      if (dimension['rotate'] == 1) {
        width = dimension['height'] as int?;
        height = dimension['width'] as int?;
      } else {
        width = dimension['width'] as int?;
        height = dimension['height'] as int?;
      }
    }

    firstFrame = json['first_frame'];
    pubLocation = json['pub_location'];
    redirectUrl = json['redirect_url'];
    progress = json['progress'];

    if (json['charging_pay']?['level'] != null) {
      badge = '充电专属';
    } else if (json['rights']?['is_cooperation'] == 1) {
      badge = '合作';
    } else {
      badge = json['pgc_label'];
    }
  }

  /// 基线 `lib/utils/parse_int.dart` 的 `safeToInt`。
  static int? _safeToInt(dynamic value) => switch (value) {
    int() => value,
    num() => value.toInt(),
    String() => int.tryParse(value),
    _ => null,
  };
}
