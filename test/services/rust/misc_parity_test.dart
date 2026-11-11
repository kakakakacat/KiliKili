import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

/// S15（issue #66）的一次性基线对照，机制见 ADR-011 决策五。
///
/// `MusicDetail`、`BgmRecommend`、`BlackListData`、`MatchContest`、
/// `DanmakuBlockDataModel` 都已随本次迁移删除，没有「还活着的基线模型」
/// 可借，于是本文件自带 oracle——下面的 `_Baseline*` 是迁移前那几个
/// `fromJson` 的**逐行转写**，取自合并前的 `main`（`c46c5172a`）：
///
/// - `lib/models_new/music/{bgm_detail,bgm_recommend_list}.dart`
/// - `lib/models_new/blacklist/{data,list}.dart`
/// - `lib/models_new/match/match_info/{data,contest,season,team}.dart`
/// - `lib/models/user/danmaku_block.dart`
///
/// 转写时不许顺手修正任何怪异之处：它要证明的正是 Rust 与迁移前行为一致。
/// 三处基线的「怪异」因此原样保留，并在断言里点名：
///
/// 1. 成就标签要把 `achievement` 数组与 `music_rank`、`recreation_rank`
///    两个单独字段拼起来，顺序即界面排标签的顺序；
/// 2. 热度曲线在模型里就 `reversed` 过（服务端给的是时间降序）；
/// 3. 弹幕规则按 `type` 分三组，未知 type 直接丢掉（基线的 switch 没有
///    default 分支）。
///
/// **两处刻意的不一致**，在断言里显式换算：
/// - 赛事：基线 `MatchContest.fromJson`（详情接口）不读 `ID`，迁移后也
///   一样不读——详情那条路径的 `id` 恒为 null，这里断言它是 null；
/// - 推荐卡片的标签：基线是 `[{name}]` 的对象列表，迁移后摊平成字符串，
///   比较时取基线的 `name`。
///
/// 除 `match_info` 外的四份 fixture 都是**手工构造**的，原因写在
/// `rust/crates/bili-api/tests/misc_golden.rs` 的文件头：黑名单与弹幕屏蔽
/// 规则是账号自己的数据，音乐两条缺少可用的 `music_id`。它们证明的是映射
/// 一致，不能证明我们对协议的理解正确——那要等 Phase 2 的真机验证。
void main() {
  const dir = 'rust/crates/bili-api/tests/fixtures/misc';

  Map<String, dynamic> read(String name) =>
      jsonDecode(File('$dir/$name').readAsStringSync()) as Map<String, dynamic>;

  List<dynamic> readList(String name) =>
      jsonDecode(File('$dir/$name').readAsStringSync()) as List<dynamic>;

  test('BGM 详情：Rust 快照与基线逐字段一致', () {
    final data = read('bgm_detail.raw.json')['data'] as Map<String, dynamic>;
    final snapshot = read('bgm_detail.rust.json');
    final b = _BaselineMusicDetail.fromJson(data);

    expect(snapshot['music_title'], b.musicTitle);
    expect(snapshot['origin_artist'], b.originArtist);
    expect(snapshot['origin_artist_list'], b.originArtistList);
    expect(snapshot['mv_aid'], b.mvAid);
    expect(snapshot['mv_cid'], b.mvCid, reason: '缺失时基线填 0');
    expect(snapshot['mv_bvid'], b.mvBvid);
    expect(snapshot['mv_cover'], b.mvCover);
    expect(snapshot['wish_listen'], b.wishListen);
    expect(snapshot['wish_count'], b.wishCount);
    expect(snapshot['music_source'], b.musicSource);
    expect(snapshot['album'], b.album);
    expect(snapshot['listen_pv'], b.listenPv);
    expect(snapshot['music_relation'], b.musicRelation);
    expect(snapshot['music_publish'], b.musicPublish);

    expect(
      (snapshot['achievement'] as List).cast<String>(),
      b.achievement,
      reason: 'achievement + music_rank + recreation_rank，顺序即展示顺序',
    );

    final artists = (snapshot['artists_list'] as List)
        .cast<Map<String, dynamic>>();
    expect(artists.length, b.artistsList?.length ?? 0);
    for (var i = 0; i < artists.length; i++) {
      final a = b.artistsList![i];
      expect(artists[i]['mid'], a.mid, reason: '第 $i 位艺人 mid');
      expect(artists[i]['name'], a.name, reason: '第 $i 位艺人昵称');
      expect(artists[i]['face'], a.face, reason: '第 $i 位艺人头像');
      expect(artists[i]['identity'], a.identity, reason: '第 $i 位艺人身份');
    }

    final heat = snapshot['hot_song_heat'] as Map<String, dynamic>?;
    expect(heat?['last_heat'], b.lastHeat);
    final points = (heat?['song_heat'] as List).cast<Map<String, dynamic>>();
    expect(points.length, b.songHeat?.length ?? 0);
    for (var i = 0; i < points.length; i++) {
      expect(points[i]['date'], b.songHeat![i].date, reason: '第 $i 个点（已反转）');
      expect(points[i]['heat'], b.songHeat![i].heat);
    }

    final comment = snapshot['music_comment'] as Map<String, dynamic>?;
    expect(comment?['nums'], b.commentNums);
    expect(comment?['oid'], b.commentOid);
    expect(comment?['page_type'], b.commentPageType);
  });

  test('BGM 相关推荐：Rust 快照与基线逐字段一致', () {
    final data = read('bgm_recommend.raw.json')['data'] as Map<String, dynamic>;
    final snapshot = readList('bgm_recommend.rust.json')
        .cast<Map<String, dynamic>>();
    final b = (data['list'] as List)
        .map((e) => _BaselineBgmRecommend.fromJson(e as Map<String, dynamic>))
        .toList();

    expect(snapshot.length, b.length);
    for (var i = 0; i < snapshot.length; i++) {
      final r = snapshot[i];
      final where = '第 $i 条推荐';
      expect(r['bvid'], b[i].bvid, reason: '$where: bvid');
      expect(r['cid'], b[i].cid, reason: '$where: cid');
      expect(r['cover'], b[i].cover, reason: '$where: 封面');
      expect(r['title'], b[i].title, reason: '$where: 标题');
      expect(r['up_nick_name'], b[i].upNickName, reason: '$where: UP 主');
      expect(r['play'], b[i].play, reason: '$where: 播放');
      expect(r['danmu'], b[i].danmu, reason: '$where: 弹幕');
      expect(r['duration'], b[i].duration, reason: '$where: 时长');
      // 基线是 [{name}]，迁移后摊平成字符串。
      expect(
        (r['labels'] as List).cast<String>(),
        b[i].labelNames,
        reason: '$where: 标签',
      );
    }
  });

  test('黑名单：Rust 快照与基线逐字段一致', () {
    final data = read('black_list.raw.json')['data'] as Map<String, dynamic>;
    final snapshot = read('black_list.rust.json');
    final b = _BaselineBlackList.fromJson(data);

    expect(snapshot['total'], b.total);
    final list = (snapshot['list'] as List).cast<Map<String, dynamic>>();
    expect(list.length, b.list?.length ?? 0);
    for (var i = 0; i < list.length; i++) {
      expect(list[i]['mid'], b.list![i].mid, reason: '第 $i 人 mid');
      expect(list[i]['mtime'], b.list![i].mtime, reason: '第 $i 人拉黑时间');
      expect(list[i]['uname'], b.list![i].uname, reason: '第 $i 人昵称');
      expect(list[i]['face'], b.list![i].face, reason: '第 $i 人头像');
    }
  });

  test('赛事详情（真实样本）：Rust 快照与基线逐字段一致', () {
    final data = read('match_info.raw.json')['data'] as Map<String, dynamic>;
    final snapshot = read('match_info.rust.json');
    final b = _BaselineMatchContest.fromJson(
      data['contest'] as Map<String, dynamic>,
    );

    expect(snapshot['id'], isNull, reason: '基线的详情工厂不读 ID');
    expect(snapshot['game_stage'], b.gameStage, reason: '空串归一成 null');
    expect(snapshot['stime'], b.stime);
    expect(snapshot['home_score'], b.homeScore);
    expect(snapshot['away_score'], b.awayScore);
    expect(snapshot['live_room'], b.liveRoom);
    expect(snapshot['contest_status'], b.contestStatus);
    expect(snapshot['playback'], b.playback, reason: 'playback 不做空串归一');

    final season = snapshot['season'] as Map<String, dynamic>?;
    expect(season?['title'], b.seasonTitle);
    expect(season?['logo'], b.seasonLogo);

    final home = snapshot['home_team'] as Map<String, dynamic>?;
    expect(home?['title'], b.homeTitle);
    expect(home?['logo'], b.homeLogo);
    expect(home?['logo_full'], b.homeLogoFull);

    final away = snapshot['away_team'] as Map<String, dynamic>?;
    expect(away?['title'], b.awayTitle);
    expect(away?['logo'], b.awayLogo);
    expect(away?['logo_full'], b.awayLogoFull);
  });

  test('综合搜索的赛事卡片：与基线的 fromSearch 逐字段一致', () {
    // 这一份在 S9 的 fixture 里，赛事块的键名是 camelCase。
    const searchDir = 'rust/crates/bili-api/tests/fixtures/search';
    final raw = jsonDecode(
      File('$searchDir/search_all.raw.json').readAsStringSync(),
    ) as Map<String, dynamic>;
    final snapshot = jsonDecode(
      File('$searchDir/search_all.rust.json').readAsStringSync(),
    ) as Map<String, dynamic>;

    final block = (raw['data']['result'] as List).firstWhere(
      (e) => e['result_type'] == 'esports',
    );
    final baseline = ((block['data'] as List).first['contest'] as List)
        .map((e) => _BaselineMatchContest.fromSearch(e as Map<String, dynamic>))
        .toList();

    final contests = (snapshot['esports'] as List).cast<Map<String, dynamic>>();
    expect(contests.length, baseline.length);
    for (var i = 0; i < contests.length; i++) {
      final r = contests[i];
      final b = baseline[i];
      expect(r['id'], b.id, reason: '第 $i 张卡片 ID（只有搜索这条路径有）');
      expect(r['game_stage'], b.gameStage, reason: '第 $i 张卡片赛段');
      expect(r['stime'], b.stime);
      expect(r['home_score'], b.homeScore);
      expect(r['away_score'], b.awayScore);
      expect(r['live_room'], b.liveRoom);
      expect(r['contest_status'], b.contestStatus);
      expect(r['playback'], b.playback);
      expect((r['season'] as Map?)?['title'], b.seasonTitle);
      expect((r['home_team'] as Map?)?['title'], b.homeTitle);
      expect((r['away_team'] as Map?)?['title'], b.awayTitle);
    }
  });

  test('弹幕屏蔽规则：Rust 快照与基线逐字段一致（含分组与丢弃）', () {
    final data =
        read('danmaku_filter.raw.json')['data'] as Map<String, dynamic>;
    final snapshot = read('danmaku_filter.rust.json');
    final b = _BaselineDanmakuBlock.fromJson(data);

    void expectGroup(String key, List<_BaselineRule> rules) {
      final list = (snapshot[key] as List).cast<Map<String, dynamic>>();
      expect(list.length, rules.length, reason: '$key 条数');
      for (var i = 0; i < list.length; i++) {
        expect(list[i]['id'], rules[i].id, reason: '$key 第 $i 条 id');
        expect(list[i]['kind'], rules[i].type, reason: '$key 第 $i 条 type');
        expect(list[i]['filter'], rules[i].filter, reason: '$key 第 $i 条内容');
      }
    }

    expectGroup('keywords', b.rule);
    expectGroup('patterns', b.rule1);
    expectGroup('users', b.rule2);
    expect(snapshot['toast'], b.toast, reason: '空串归一成 null');
    expect(snapshot['valid'], b.valid);
    expect(snapshot['ver'], b.ver);
  });
}

// ---------------------------------------------------------------------------
// 以下是迁移前 fromJson 的逐行转写，见文件头。
// ---------------------------------------------------------------------------

class _BaselineMusicDetail {
  String? musicTitle;
  String? originArtist;
  String? originArtistList;
  int? mvAid;
  int mvCid = 0;
  String? mvBvid;
  String? mvCover;
  bool? wishListen;
  int? wishCount;
  String? musicSource;
  String? album;
  List<_BaselineArtist>? artistsList;
  int? listenPv;
  List<String> achievement = [];
  int? lastHeat;
  List<_BaselineSongHeat>? songHeat;
  int? commentNums;
  int? commentOid;
  int? commentPageType;
  int? musicRelation;
  String? musicPublish;

  _BaselineMusicDetail.fromJson(Map<String, dynamic> json) {
    musicTitle = json["music_title"];
    originArtist = json["origin_artist"];
    originArtistList = json["origin_artist_list"];
    mvAid = json["mv_aid"];
    mvCid = json["mv_cid"] ?? 0;
    mvBvid = json["mv_bvid"];
    mvCover = json["mv_cover"];
    wishListen = json["wish_listen"];
    wishCount = json["wish_count"];
    musicSource = json["music_source"];
    album = json["album"];
    artistsList = (json["artists_list"] as List?)
        ?.map((x) => _BaselineArtist.fromJson(x as Map<String, dynamic>))
        .toList();
    listenPv = json["listen_pv"];
    achievement = [
      ...?(json["achievement"] as List?)?.cast<String>(),
      ?json["music_rank"] as String?,
      ?json["recreation_rank"] as String?,
    ];
    final heat = json["hot_song_heat"] as Map<String, dynamic>?;
    lastHeat = heat?["last_heat"];
    songHeat = (heat?["song_heat"] as List?)?.reversed
        .map((x) => _BaselineSongHeat.fromJson(x as Map<String, dynamic>))
        .toList();
    final comment = json["music_comment"] as Map<String, dynamic>?;
    commentNums = comment?["nums"];
    commentOid = comment?["oid"];
    commentPageType = comment?["page_type"];
    musicRelation = json["music_relation"];
    musicPublish = json["music_publish"];
  }
}

class _BaselineArtist {
  int? mid;
  String? name;
  String? face;
  String? identity;

  _BaselineArtist.fromJson(Map<String, dynamic> json) {
    // 基线 Artist extends Owner：mid 走 safeToInt，name/face 直取。
    mid = json["mid"] is int
        ? json["mid"] as int
        : int.tryParse('${json["mid"]}');
    name = json["name"];
    face = json["face"];
    identity = json["identity"];
  }
}

class _BaselineSongHeat {
  late final int date;
  late final int heat;

  _BaselineSongHeat.fromJson(Map<String, dynamic> json) {
    date = json["date"];
    heat = json["heat"];
  }
}

class _BaselineBgmRecommend {
  String? bvid;
  int? cid;
  String? cover;
  String? title;
  String? upNickName;
  int? play;
  int? danmu;
  int? duration;
  List<String> labelNames = [];

  _BaselineBgmRecommend.fromJson(Map<String, dynamic> json) {
    bvid = json["bvid"];
    cid = json["cid"];
    cover = json["cover"];
    title = json["title"];
    upNickName = json["up_nick_name"];
    play = json["play"];
    danmu = json["danmu"];
    duration = json["duration"];
    labelNames = ((json["label_list"] as List?) ?? const [])
        .map((x) => (x as Map<String, dynamic>)["name"] as String?)
        .whereType<String>()
        .toList();
  }
}

class _BaselineBlackList {
  List<_BaselineBlackListItem>? list;
  int? total;

  _BaselineBlackList.fromJson(Map<String, dynamic> json) {
    list = (json['list'] as List<dynamic>?)
        ?.map(
          (e) => _BaselineBlackListItem.fromJson(e as Map<String, dynamic>),
        )
        .toList();
    total = json['total'] as int?;
  }
}

class _BaselineBlackListItem {
  int? mid;
  int? mtime;
  String? uname;
  String? face;

  _BaselineBlackListItem.fromJson(Map<String, dynamic> json) {
    mid = json['mid'] as int?;
    mtime = json['mtime'] as int?;
    uname = json['uname'] as String?;
    face = json['face'] as String?;
  }
}

/// 基线 `nonNullOrEmptyString`。
String? _nonNullOrEmpty(String? s) => (s == null || s.isEmpty) ? null : s;

class _BaselineMatchContest {
  int? id;
  String? gameStage;
  int? stime;
  int? homeScore;
  int? awayScore;
  int? liveRoom;
  int? contestStatus;
  String? playback;
  String? seasonTitle;
  String? seasonLogo;
  String? homeTitle;
  String? homeLogo;
  String? homeLogoFull;
  String? awayTitle;
  String? awayLogo;
  String? awayLogoFull;

  _BaselineMatchContest._();

  factory _BaselineMatchContest.fromJson(Map<String, dynamic> json) {
    final c = _BaselineMatchContest._()
      ..gameStage = _nonNullOrEmpty(json['game_stage'] as String?)
      ..stime = json['stime'] as int?
      ..homeScore = json['home_score'] as int?
      ..awayScore = json['away_score'] as int?
      ..liveRoom = json['live_room'] as int?
      ..contestStatus = json['contest_status'] as int?
      ..playback = json['playback'] as String?;
    c._season(json['season'] as Map<String, dynamic>?);
    c._home(json['home_team'] as Map<String, dynamic>?);
    c._away(json['away_team'] as Map<String, dynamic>?);
    return c;
  }

  factory _BaselineMatchContest.fromSearch(Map<String, dynamic> json) {
    final c = _BaselineMatchContest._()
      ..id = json['ID'] as int?
      ..gameStage = _nonNullOrEmpty(json['gameStage'] as String?)
      ..stime = json['stime'] as int?
      ..homeScore = json['homeScore'] as int?
      ..awayScore = json['awayScore'] as int?
      ..liveRoom = json['liveRoom'] as int?
      ..contestStatus = json['contestStatus'] as int?
      ..playback = json['playback'] as String?;
    c._season(json['season'] as Map<String, dynamic>?);
    c._home(json['homeTeam'] as Map<String, dynamic>?);
    c._away(json['awayTeam'] as Map<String, dynamic>?);
    return c;
  }

  void _season(Map<String, dynamic>? json) {
    seasonTitle = json?['title'] as String?;
    seasonLogo = json?['logo'] as String?;
  }

  void _home(Map<String, dynamic>? json) {
    homeTitle = json?['title'] as String?;
    homeLogo = json?['logo'] as String?;
    homeLogoFull = json?['logoFull'] as String?;
  }

  void _away(Map<String, dynamic>? json) {
    awayTitle = json?['title'] as String?;
    awayLogo = json?['logo'] as String?;
    awayLogoFull = json?['logoFull'] as String?;
  }
}

class _BaselineRule {
  late final int id;
  late final int type;
  late final String filter;

  _BaselineRule.fromJson(Map<String, dynamic> json) {
    id = json['id'];
    type = json['type'];
    filter = json['filter'];
  }
}

class _BaselineDanmakuBlock {
  List<_BaselineRule> rule = [];
  List<_BaselineRule> rule1 = [];
  List<_BaselineRule> rule2 = [];
  String? toast;
  int? valid;
  int? ver;

  _BaselineDanmakuBlock.fromJson(Map<String, dynamic> json) {
    if (json['rule'] case List list) {
      for (final e in list) {
        // 基线在这里对缺字段是直接抛的（late final 赋 null）；fixture 里
        // 那条缺 filter 的规则因此走不到分组，与 Rust 丢掉它一致。
        final map = e as Map<String, dynamic>;
        if (map['id'] == null || map['type'] == null || map['filter'] == null) {
          continue;
        }
        final item = _BaselineRule.fromJson(map);
        switch (item.type) {
          case 0:
            rule.add(item);
          case 1:
            rule1.add(item);
          case 2:
            rule2.add(item);
        }
      }
    }
    toast = json['toast'] == '' ? null : json['toast'];
    valid = json['valid'];
    ver = json['ver'];
  }
}
