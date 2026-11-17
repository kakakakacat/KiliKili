import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

/// 用户空间那一族的**一次性基线对照**（S6 #56，机制见 ADR-011 决策五）。
///
/// ## 为什么是逐行转写，而不是 import 基线模型
///
/// 这一批的基线模型在**本 PR 里已经删除**（`lib/models/member/info.dart`、
/// `lib/models_new/member_card_info/`、`lib/models_new/member/coin_like_arc/`、
/// `lib/models_new/member_guard/`、`lib/models_new/upower_rank/`、
/// `lib/models_new/space/space_season_series/`）——它们的生产者全部迁进了
/// Rust，照 ADR-011 决策四不留旧实现。被删掉的东西 import 不进来。
///
/// 下面的 `_Baseline*` 是那几个 `fromJson` 的逐行转写，取自本 PR 的父提交，
/// 写下之后不再改，也不许顺手修正任何怪异之处——包括：
/// - `Vip.fromJson` 的 `type ?? vipType` 与 `status ?? vipStatus ?? 0`；
/// - `Card.fromJson` 读的是**大写开头**的 `Official`，而 `mid` 是字符串。
///
/// `MemberHttp.memberStat` / `memberView` 在基线里**没有模型**，直接把
/// `res.data['data']` 这个 `Map` 塞进 `RxMap`，界面按字符串键去取。所以
/// 这两条的对照写成「Rust 的字段与原始 JSON 里那几个键相等」——基线的
/// 「解析」本来就只有取键这一步。
///
/// ## 这个测试的寿命
///
/// 一次性。基线实现已经不在仓库里，本文件证明的是「删掉它们的那一刻，
/// Rust 的输出与它们逐字段一致」。**下一个碰这一族的 PR 可以连同它一起
/// 删**，长期留在仓库里的是
/// `rust/crates/bili-api/tests/fixtures/space/*.rust.json` 那几份快照。
void main() {
  const dir = 'rust/crates/bili-api/tests/fixtures/space';

  dynamic read(String name) =>
      jsonDecode(File('$dir/$name').readAsStringSync());

  test('用户资料：Rust 快照与基线逐字段一致', () {
    for (final name in ['member_info', 'member_info_alt_vip_keys']) {
      final data = read('$name.raw.json')['data'] as Map<String, dynamic>;
      final snapshot = read('$name.rust.json') as Map<String, dynamic>;
      final baseline = _BaselineMemberInfo.fromJson(data);

      expect(snapshot['mid'], baseline.mid, reason: '$name mid');
      expect(snapshot['name'], baseline.name, reason: '$name name');
      expect(snapshot['face'], baseline.face, reason: '$name face');
      expect(snapshot['level'], baseline.level, reason: '$name level');
      expect(
        snapshot['is_followed'],
        baseline.isFollowed,
        reason: '$name isFollowed',
      );
      expect(snapshot['vip_status'], baseline.vipStatus, reason: '$name vip.status');
      expect(snapshot['vip_type'], baseline.vipType, reason: '$name vip.type');
      expect(
        snapshot['is_senior_member'],
        baseline.isSeniorMember,
        reason: '$name isSeniorMember',
      );
    }
  });

  test('大会员信息的两套键名都认 —— 照基线的 `type ?? vipType`', () {
    final alt =
        read('member_info_alt_vip_keys.rust.json') as Map<String, dynamic>;
    expect(alt['vip_type'], 1, reason: '只给了 vipType');
    expect(alt['vip_status'], 1, reason: '只给了 vipStatus');
  });

  test('用户名片：Rust 快照与基线逐字段一致', () {
    for (final name in ['member_card', 'member_card_no_card']) {
      final data = read('$name.raw.json')['data'] as Map<String, dynamic>;
      final snapshot = read('$name.rust.json') as Map<String, dynamic>;
      final baseline = _BaselineMemberCard.fromJson(data);

      expect(snapshot['mid'], baseline.mid, reason: '$name card.mid');
      expect(snapshot['name'], baseline.name, reason: '$name card.name');
      expect(snapshot['face'], baseline.face, reason: '$name card.face');
      expect(
        snapshot['official_type'],
        baseline.officialType,
        reason: '$name card.Official.type',
      );
      expect(snapshot['vip_status'], baseline.vipStatus, reason: '$name vip.status');
      expect(snapshot['vip_type'], baseline.vipType, reason: '$name vip.type');
      expect(
        snapshot['archive_count'],
        baseline.archiveCount,
        reason: '$name archiveCount',
      );
      expect(snapshot['follower'], baseline.follower, reason: '$name follower');
    }
  });

  test('名片的 mid 是字符串 —— 基线的 `Card.mid` 也是 String?', () {
    expect(
      (read('member_card.rust.json') as Map<String, dynamic>)['mid'],
      isA<String>(),
    );
  });

  test('关注关系计数：基线只是取键，Rust 取的是同样两个键', () {
    final data =
        read('member_relation_stat.raw.json')['data'] as Map<String, dynamic>;
    final snapshot =
        read('member_relation_stat.rust.json') as Map<String, dynamic>;
    expect(snapshot['following'], data['following'], reason: 'following');
    expect(snapshot['follower'], data['follower'], reason: 'follower');
    expect(snapshot['following'], -1, reason: '隐私设置下的 -1 要原样留着');
  });

  test('合集和列表：Rust 快照与基线逐字段一致', () {
    // 基线取的是 `res.data['data']?['items_lists'] ?? {}`。
    final lists =
        (read('season_series.raw.json')['data']
            as Map<String, dynamic>)['items_lists']
        as Map<String, dynamic>;
    final snapshot = read('season_series.rust.json') as Map<String, dynamic>;

    expect(snapshot['total'], lists['page']['total'], reason: 'page.total');

    for (final (key, rustKey) in [
      ('seasons_list', 'seasons'),
      ('series_list', 'series'),
    ]) {
      final baseline = (lists[key] as List)
          .map((e) => _BaselineSeasonSeries.fromJson(e as Map<String, dynamic>))
          .toList();
      final got = snapshot[rustKey] as List;
      expect(got.length, baseline.length, reason: '$key 条数');
      for (var i = 0; i < got.length; i++) {
        final item = got[i] as Map<String, dynamic>;
        expect(item['cover'], baseline[i].cover, reason: '$key[$i] cover');
        expect(item['name'], baseline[i].name, reason: '$key[$i] name');
        expect(item['ptime'], baseline[i].ptime, reason: '$key[$i] ptime');
        expect(item['total'], baseline[i].total, reason: '$key[$i] total');
        expect(
          item['season_id'],
          baseline[i].seasonId,
          reason: '$key[$i] seasonId',
        );
        expect(
          item['series_id'],
          baseline[i].seriesId,
          reason: '$key[$i] seriesId',
        );
      }
    }
  });

  test('整块 items_lists 缺失时是空页，不是错误 —— 照基线的 `?? {}`', () {
    final snapshot =
        read('season_series_empty.rust.json') as Map<String, dynamic>;
    expect(snapshot['seasons'], isEmpty);
    expect(snapshot['series'], isEmpty);
    expect(snapshot['total'], isNull);
  });

  test('投币/点赞列表：Rust 快照与基线逐字段一致', () {
    final data = read('coin_like_arc.raw.json')['data'] as Map<String, dynamic>;
    final snapshot = read('coin_like_arc.rust.json') as Map<String, dynamic>;
    expect(snapshot['count'], data['count'], reason: 'count');

    final baseline = (data['item'] as List)
        .map((e) => _BaselineCoinLikeArcItem.fromJson(e as Map<String, dynamic>))
        .toList();
    final list = snapshot['items'] as List;
    expect(list.length, baseline.length, reason: '条数');
    for (var i = 0; i < list.length; i++) {
      final got = list[i] as Map<String, dynamic>;
      final want = baseline[i];
      expect(got['title'], want.title, reason: '第 $i 条 title');
      expect(got['cover'], want.cover, reason: '第 $i 条 cover');
      expect(got['uri'], want.uri, reason: '第 $i 条 uri');
      expect(got['param'], want.param, reason: '第 $i 条 param');
      expect(got['duration'], want.duration, reason: '第 $i 条 duration');
      expect(got['is_steins'], want.isSteins, reason: '第 $i 条 isSteins');
      expect(
        got['is_cooperation'],
        want.isCooperation,
        reason: '第 $i 条 isCooperation',
      );
      expect(got['is_pgc'], want.isPgc, reason: '第 $i 条 isPgc');
      expect(got['play'], want.play, reason: '第 $i 条 play');
      expect(got['danmaku'], want.danmaku, reason: '第 $i 条 danmaku');
      expect(got['ctime'], want.ctime, reason: '第 $i 条 ctime');
    }
  });

  test('大航海名单：Rust 快照与基线逐字段一致（缺字段的那项除外）', () {
    final data = read('member_guard.raw.json')['data'] as Map<String, dynamic>;
    final snapshot = read('member_guard.rust.json') as Map<String, dynamic>;
    expect(snapshot['has_more'], data['has_more'], reason: 'hasMore');

    final raw = data['guard_top_list'] as List;
    final list = snapshot['items'] as List;
    expect(list.length, raw.length, reason: '条数');
    for (var i = 0; i < list.length; i++) {
      final got = list[i] as Map<String, dynamic>;
      final src = raw[i] as Map<String, dynamic>;
      // 基线的 `GuardItem.fromJson` 就是四次取键（字段声明成非空，少一个
      // 会抛）。这里逐键比，并单独验那条只有 uid 的不再是崩溃。
      expect(got['uid'], src['uid'], reason: '第 $i 条 uid');
      expect(got['username'], src['username'], reason: '第 $i 条 username');
      expect(got['face'], src['face'], reason: '第 $i 条 face');
      expect(
        got['guard_level'],
        src['guard_level'],
        reason: '第 $i 条 guardLevel',
      );
    }
    expect(
      (list[2] as Map<String, dynamic>)['username'],
      isNull,
      reason: '只有 uid 的那条：基线会抛，我们给 null',
    );
  });

  test('充电排行：Rust 快照与基线逐字段一致', () {
    final data = read('upower_rank.raw.json')['data'] as Map<String, dynamic>;
    final snapshot = read('upower_rank.rust.json') as Map<String, dynamic>;

    expect(
      snapshot['privilege_type'],
      data['privilege_type'],
      reason: 'privilegeType',
    );
    expect(snapshot['tabs'], data['tabs'], reason: 'tabs');

    final rawLevels = data['level_info'] as List;
    final levels = snapshot['levels'] as List;
    expect(levels.length, rawLevels.length, reason: '档位数');
    for (var i = 0; i < levels.length; i++) {
      final got = levels[i] as Map<String, dynamic>;
      final src = rawLevels[i] as Map<String, dynamic>;
      expect(got['privilege_type'], src['privilege_type'], reason: '档位 $i');
      expect(got['name'], src['name'], reason: '档位 $i name');
      expect(got['member_total'], src['member_total'], reason: '档位 $i 人数');
    }

    final rawRank = data['rank_info'] as List;
    final items = snapshot['items'] as List;
    expect(items.length, rawRank.length, reason: '条数');
    for (var i = 0; i < items.length; i++) {
      final got = items[i] as Map<String, dynamic>;
      final src = rawRank[i] as Map<String, dynamic>;
      expect(got['mid'], src['mid'], reason: '第 $i 条 mid');
      expect(got['nickname'], src['nickname'], reason: '第 $i 条 nickname');
      expect(got['avatar'], src['avatar'], reason: '第 $i 条 avatar');
      expect(got['day'], src['day'], reason: '第 $i 条 day');
    }
  });

  test('播放与获赞总数：基线取的是同样三个（嵌套）键', () {
    final data = read('member_up_stat.raw.json')['data'] as Map<String, dynamic>;
    final snapshot = read('member_up_stat.rust.json') as Map<String, dynamic>;
    expect(snapshot['archive_view'], data['archive']['view'], reason: '稿件播放');
    expect(snapshot['article_view'], data['article']['view'], reason: '专栏阅读');
    expect(snapshot['likes'], data['likes'], reason: '获赞');
  });
}

// --------------------------------------------------------- 基线逐行转写

/// 基线 `Vip.fromJson`：同一个结构在不同接口下有两套键名，两套都认。
({int? type, int status}) _baselineVip(Map<String, dynamic>? json) {
  if (json == null) return (type: null, status: 0);
  return (
    type: json['type'] ?? json['vipType'],
    status: json['status'] ?? json['vipStatus'] ?? 0,
  );
}

class _BaselineMemberInfo {
  _BaselineMemberInfo.fromJson(Map<String, dynamic> json)
    : mid = json['mid'],
      name = json['name'],
      face = json['face'],
      level = json['level'],
      isFollowed = json['is_followed'],
      isSeniorMember = json['is_senior_member'],
      // 基线无条件 `Vip.fromJson(json['vip'])`——`vip` 缺了会抛。这里
      // 不复刻那个崩法；fixture 两份都带着 `vip`，对照不受影响。
      _vip = _baselineVip(json['vip'] as Map<String, dynamic>?);

  final int? mid;
  final String? name;
  final String? face;
  final int? level;
  final bool? isFollowed;
  final int? isSeniorMember;
  final ({int? type, int status}) _vip;

  int? get vipType => _vip.type;
  int get vipStatus => _vip.status;
}

/// 基线 `SpaceSsMeta.fromJson`。基线的 `SpaceSsModel` 只是裹了一层 `meta`。
class _BaselineSeasonSeries {
  _BaselineSeasonSeries.fromJson(Map<String, dynamic> json)
    : cover = json['meta']?['cover'],
      name = json['meta']?['name'],
      ptime = json['meta']?['ptime'],
      total = json['meta']?['total'],
      seasonId = json['meta']?['season_id'],
      seriesId = json['meta']?['series_id'];

  final String? cover;
  final String? name;
  final int? ptime;
  final int? total;
  final int? seasonId;
  final int? seriesId;
}

/// 基线 `CoinLikeArcItem.fromJson`。
class _BaselineCoinLikeArcItem {
  _BaselineCoinLikeArcItem.fromJson(Map<String, dynamic> json)
    : title = json['title'] as String?,
      cover = json['cover'] as String?,
      uri = json['uri'] as String?,
      param = json['param'] as String?,
      duration = json['duration'] as int?,
      isSteins = json['is_steins'] as bool?,
      isCooperation = json['is_cooperation'] as bool?,
      isPgc = json['is_pgc'] as bool?,
      play = json['play'] as int?,
      danmaku = json['danmaku'] as int?,
      ctime = json['ctime'] as int?;

  final String? title;
  final String? cover;
  final String? uri;
  final String? param;
  final int? duration;
  final bool? isSteins;
  final bool? isCooperation;
  final bool? isPgc;
  final int? play;
  final int? danmaku;
  final int? ctime;
}

class _BaselineMemberCard {
  _BaselineMemberCard.fromJson(Map<String, dynamic> json)
    : mid = json['card']?['mid'] as String?,
      name = json['card']?['name'] as String?,
      face = json['card']?['face'] as String?,
      // 这条接口的键是大写开头的 `Official`。
      officialType = json['card']?['Official']?['type'],
      _vip = json['card']?['vip'] == null
          ? null
          : _baselineVip(json['card']['vip'] as Map<String, dynamic>),
      archiveCount = json['archive_count'] as int?,
      follower = json['follower'] as int?;

  final String? mid;
  final String? name;
  final String? face;
  final int? officialType;
  final ({int? type, int status})? _vip;
  final int? archiveCount;
  final int? follower;

  /// 基线的 `Card.vip` 是可空的（`json['vip'] == null ? null : ...`），
  /// 整块缺失时界面读到的是 `null`，不是 0。
  int? get vipType => _vip?.type;
  int? get vipStatus => _vip?.status;
}
