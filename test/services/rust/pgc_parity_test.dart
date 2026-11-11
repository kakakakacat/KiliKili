import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

/// PGC 迁移的**基线对照**（S10 第一批 / issue #60，机制见 ADR-011 决策五
/// 与其中 S9 补充二）。
///
/// 被取代的 `fromJson` 随本 PR 从 `lib/` 删除，借不到活着的基线，所以
/// 下面 `_Baseline` 里是迁移前那一版的**逐字拷贝**：
///
/// - `lib/models_new/pgc/pgc_index_result/{data,list}.dart`
/// - `lib/models_new/pgc/pgc_index_condition/{data,sort,value}.dart`
/// - `lib/models_new/pgc/pgc_timeline/{pgc_timeline,result,episode}.dart`
/// - `lib/models_new/pgc/pgc_review/{data,list,author,stat}.dart`
/// - `lib/models_new/pgc/pgc_rank/{pgc_rank_item_model,new_ep,stat}.dart`
///
/// 它不引用 `lib/` 的任何东西，写下之后不再改——和 `*.rust.json` 快照一样
/// 是迁移当时行为的冻结证据。两者由不同实现产出，任何一边变了都会红。
void main() {
  const dir = 'rust/crates/bili-api/tests/fixtures/pgc';

  Map<String, dynamic> readRaw(String name) =>
      jsonDecode(File('$dir/$name.raw.json').readAsStringSync())
          as Map<String, dynamic>;

  dynamic readSnapshot(String name) =>
      jsonDecode(File('$dir/$name.rust.json').readAsStringSync());

  group('索引结果', () {
    test('Rust 快照与 Dart 基线逐字段一致', () {
      final raw = readRaw('index');
      final snap = readSnapshot('index') as Map<String, dynamic>;
      final data = raw['data'] as Map<String, dynamic>;

      expect(snap['has_next'], data['has_next'], reason: 'has_next');

      final baseline = (data['list'] as List)
          .map((e) => _Baseline.indexItem(e as Map<String, dynamic>))
          .toList();
      final items = snap['list'] as List;
      expect(items.length, baseline.length, reason: '条数');

      for (var i = 0; i < baseline.length; i++) {
        final b = baseline[i];
        final r = items[i] as Map<String, dynamic>;
        final where = '第 $i 条';
        expect(r['badge'], b['badge'], reason: '$where: badge');
        expect(r['cover'], b['cover'], reason: '$where: 封面');
        expect(r['index_show'], b['indexShow'], reason: '$where: index_show');
        expect(r['order'], b['order'], reason: '$where: order');
        expect(r['season_id'], b['seasonId'], reason: '$where: season_id');
        expect(r['title'], b['title'], reason: '$where: 标题');
      }
    });
  });

  group('索引筛选条件', () {
    test('Rust 快照与 Dart 基线逐字段一致', () {
      final raw = readRaw('index_condition');
      final snap = readSnapshot('index_condition') as Map<String, dynamic>;
      final data = raw['data'] as Map<String, dynamic>;

      final filters = (data['filter'] as List)
          .map((e) => _Baseline.conditionFilter(e as Map<String, dynamic>))
          .toList();
      final rustFilters = snap['filter'] as List;
      expect(rustFilters.length, filters.length, reason: 'filter 组数');

      for (var i = 0; i < filters.length; i++) {
        final b = filters[i];
        final r = rustFilters[i] as Map<String, dynamic>;
        expect(r['field'], b['field'], reason: 'filter $i: field');
        expect(r['name'], b['name'], reason: 'filter $i: name');

        final bv = b['values'] as List;
        final rv = r['values'] as List;
        expect(rv.length, bv.length, reason: 'filter $i: 取值数');
        for (var j = 0; j < bv.length; j++) {
          // keyword 要原样回传给索引接口，改一个字都会让筛选失效。
          expect(
            (rv[j] as Map)['keyword'],
            (bv[j] as Map)['keyword'],
            reason: 'filter $i 取值 $j: keyword',
          );
          expect(
            (rv[j] as Map)['name'],
            (bv[j] as Map)['name'],
            reason: 'filter $i 取值 $j: name',
          );
        }
      }

      final orders = (data['order'] as List)
          .map((e) => _Baseline.conditionOrder(e as Map<String, dynamic>))
          .toList();
      final rustOrders = snap['order'] as List;
      expect(rustOrders.length, orders.length, reason: 'order 组数');
      for (var i = 0; i < orders.length; i++) {
        expect((rustOrders[i] as Map)['field'], orders[i]['field']);
        expect((rustOrders[i] as Map)['name'], orders[i]['name']);
        expect((rustOrders[i] as Map)['sort'], orders[i]['sort']);
      }
    });
  });

  group('放送时间表', () {
    test('Rust 快照与 Dart 基线逐字段一致', () {
      final raw = readRaw('timeline');
      final snap = readSnapshot('timeline') as List;

      // 基线 PgcTimeline.fromJson 读的是顶层 result，不是 data。
      final baseline = (raw['result'] as List)
          .map((e) => _Baseline.timelineDay(e as Map<String, dynamic>))
          .toList();
      expect(snap.length, baseline.length, reason: '天数');

      for (var i = 0; i < baseline.length; i++) {
        final b = baseline[i];
        final r = snap[i] as Map<String, dynamic>;
        final where = '第 $i 天（${b['date']}）';
        expect(r['date'], b['date'], reason: '$where: date');
        expect(r['date_ts'], b['dateTs'], reason: '$where: date_ts');
        expect(r['day_of_week'], b['dayOfWeek'], reason: '$where: 星期');
        expect(r['is_today'], b['isToday'], reason: '$where: is_today');

        final be = b['episodes'] as List;
        final re = r['episodes'] as List;
        expect(re.length, be.length, reason: '$where: 集数');
        for (var j = 0; j < be.length; j++) {
          final bj = be[j] as Map;
          final rj = re[j] as Map;
          expect(rj['episode_id'], bj['episodeId'], reason: '$where 第 $j 集');
          expect(rj['cover'], bj['cover']);
          expect(rj['follow'], bj['follow']);
          expect(rj['pub_index'], bj['pubIndex']);
          expect(rj['pub_time'], bj['pubTime']);
          expect(rj['season_id'], bj['seasonId']);
          expect(rj['title'], bj['title']);
        }
      }
    });

    test('没有新番的那天也要保留', () {
      // 界面按天排版，少一天会让日期错位。
      final snap = readSnapshot('timeline') as List;
      expect(snap.length, 3);
      expect((snap[2] as Map)['episodes'], isEmpty);
    });
  });

  group('评论列表', () {
    test('Rust 快照与 Dart 基线逐字段一致', () {
      final raw = readRaw('review');
      final snap = readSnapshot('review') as Map<String, dynamic>;
      final data = raw['data'] as Map<String, dynamic>;

      // 基线 `json['count'] ?? json['total']`。
      expect(snap['count'], data['count'] ?? data['total'], reason: 'count');
      expect(snap['next'], data['next'], reason: 'next');

      final baseline = (data['list'] as List)
          .map((e) => _Baseline.reviewItem(e as Map<String, dynamic>))
          .toList();
      final items = snap['list'] as List;
      expect(items.length, baseline.length, reason: '条数');

      for (var i = 0; i < baseline.length; i++) {
        final b = baseline[i];
        final r = items[i] as Map<String, dynamic>;
        final where = '第 $i 条（${b['reviewId']}）';

        expect(r['review_id'], b['reviewId'], reason: '$where: review_id');
        expect(r['article_id'], b['articleId'], reason: '$where: article_id');
        expect(r['title'], b['title'], reason: '$where: 标题');
        expect(r['content'], b['content'], reason: '$where: 正文');
        expect(
          r['push_time_str'],
          b['pushTimeStr'],
          reason: '$where: 时间文案',
        );
        // 0–10 整除成 0–5。差一分就是差半颗星。
        expect(r['score'], b['score'], reason: '$where: 分数');

        final ba = b['author'] as Map?;
        final ra = r['author'] as Map?;
        if (ba == null) {
          expect(ra, isNull, reason: '$where: 应当没有作者');
        } else {
          expect(ra, isNotNull, reason: '$where: 应当有作者');
          expect(ra!['mid'], ba['mid'], reason: '$where: mid');
          expect(ra['uname'], ba['uname'], reason: '$where: 昵称');
          expect(ra['avatar'], ba['avatar'], reason: '$where: 头像');
          expect(ra['level'], ba['level'], reason: '$where: 等级');
          // 基线把 vip 包成共享模型；迁移后摊平成两个整数，值要一致。
          expect(ra['vip_type'], ba['vipType'], reason: '$where: vip type');
          expect(
            ra['vip_status'],
            ba['vipStatus'],
            reason: '$where: vip status',
          );
        }

        final bs = b['stat'] as Map?;
        final rs = r['stat'] as Map?;
        if (bs == null) {
          expect(rs, isNull, reason: '$where: 应当没有 stat');
        } else {
          expect(rs!['likes'], bs['likes'], reason: '$where: 赞数');
          expect(rs['liked'], bs['liked'], reason: '$where: 已赞');
          expect(rs['disliked'], bs['disliked'], reason: '$where: 已踩');
        }
      }
    });

    test('分数是整除不是四舍五入', () {
      // 基线 `~/ 2`：10 → 5，9 → 4。改成四舍五入会让一整批九分短评多出
      // 半颗星，而这种偏差没人会报 bug，只会慢慢被当成「本来就这样」。
      expect(_Baseline.reviewScore(10), 5);
      expect(_Baseline.reviewScore(9), 4);
      expect(_Baseline.reviewScore(1), 0);
      expect(_Baseline.reviewScore(null), 0);
    });
  });

  group('排行榜', () {
    test('web 变体与 Dart 基线逐字段一致', () {
      final raw = readRaw('rank_web');
      final snap = readSnapshot('rank_web') as List;
      // 基线 pgcRankList 读的是 result.list。
      final baseline = ((raw['result'] as Map)['list'] as List)
          .map((e) => _Baseline.rankItem(e as Map<String, dynamic>))
          .toList();
      _expectRank(snap, baseline);
    });

    test('season 变体与 Dart 基线逐字段一致', () {
      final raw = readRaw('rank_season');
      final snap = readSnapshot('rank_season') as List;
      // 基线 pgcSeasonRankList 读的是 data.list——与 web 变体不同。
      final baseline = ((raw['data'] as Map)['list'] as List)
          .map((e) => _Baseline.rankItem(e as Map<String, dynamic>))
          .toList();
      _expectRank(snap, baseline);
    });

    test('缺 stat 的条目计数为 0 而不是空', () {
      // 基线 `(json['follow'] as int?) ?? 0`。给 null 会让界面的数字
      // 格式化函数拿到空值。
      final snap = readSnapshot('rank_web') as List;
      final second = snap[1] as Map<String, dynamic>;
      expect(second['follow'], 0);
      expect(second['view'], 0);
    });
  });

  group('追番状态', () {
    test('Rust 快照与 Dart 基线一致', () {
      final raw = readRaw('season_status');
      final snap = readSnapshot('season_status') as Map<String, dynamic>;
      // 基线把整个 result 当 Map 丢给 UI，只读这两个键。
      final result = raw['result'] as Map<String, dynamic>;
      expect(snap['follow'], result['follow']);
      expect(snap['follow_status'], result['follow_status']);
    });
  });
}

void _expectRank(List snap, List<Map<String, dynamic>> baseline) {
  expect(snap.length, baseline.length, reason: '条数');
  for (var i = 0; i < baseline.length; i++) {
    final b = baseline[i];
    final r = snap[i] as Map<String, dynamic>;
    final where = '第 $i 条';
    expect(r['title'], b['title'], reason: '$where: 标题');
    expect(r['cover'], b['cover'], reason: '$where: 封面');
    expect(r['url'], b['url'], reason: '$where: url');
    // 基线嵌在 new_ep.index_show，迁移后摊平。
    expect(
      r['new_ep_index_show'],
      b['newEpIndexShow'],
      reason: '$where: index_show',
    );
    expect(r['follow'], b['follow'], reason: '$where: 追番数');
    expect(r['view'], b['view'], reason: '$where: 播放数');
  }
}

/// 迁移前的 Dart 实现，逐字拷贝，**不要改**。来源见文件头部。
abstract final class _Baseline {
  static Map<String, dynamic> indexItem(Map<String, dynamic> json) => {
    'badge': json['badge'] as String?,
    'cover': json['cover'] as String?,
    'indexShow': json['index_show'] as String?,
    'order': json['order'] as String?,
    'seasonId': json['season_id'] as int?,
    'title': json['title'] as String?,
  };

  static Map<String, dynamic> conditionFilter(Map<String, dynamic> json) => {
    'field': json['field'] as String?,
    'name': json['name'] as String?,
    'values':
        (json['values'] as List<dynamic>?)
            ?.map((e) => {'keyword': e['keyword'], 'name': e['name']})
            .toList() ??
        const [],
  };

  static Map<String, dynamic> conditionOrder(Map<String, dynamic> json) => {
    'field': json['field'] as String?,
    'name': json['name'] as String?,
    'sort': json['sort'] as String?,
  };

  static Map<String, dynamic> timelineDay(Map<String, dynamic> json) => {
    'date': json['date'] as String?,
    'dateTs': json['date_ts'] as int?,
    'dayOfWeek': json['day_of_week'] as int?,
    'isToday': json['is_today'] as int?,
    'episodes':
        (json['episodes'] as List<dynamic>?)
            ?.map(
              (e) => {
                'cover': e['cover'] as String?,
                'episodeId': e['episode_id'] as int?,
                'follow': e['follow'] as int?,
                'pubIndex': e['pub_index'] as String?,
                'pubTime': e['pub_time'] as String?,
                'seasonId': e['season_id'] as int?,
                'title': e['title'] as String?,
              },
            )
            .toList() ??
        const [],
  };

  /// 基线 `json['score'] == null ? 0 : json['score'] ~/ 2`。
  static int reviewScore(Object? score) =>
      score == null ? 0 : (score as int) ~/ 2;

  static Map<String, dynamic> reviewItem(Map<String, dynamic> json) => {
    'articleId': json['article_id'],
    'title': json['title'] as String?,
    'content': json['content'] as String?,
    'pushTimeStr': json['push_time_str'] as String?,
    'reviewId': json['review_id'] as int?,
    'score': reviewScore(json['score']),
    'author': json['author'] == null
        ? null
        : {
            'avatar': json['author']['avatar'] as String?,
            'level': json['author']['level'] as int?,
            'mid': json['author']['mid'] as int?,
            'uname': json['author']['uname'] as String?,
            // 基线 Vip.fromJson：`type` 直取，`status ?? 0`。
            'vipType': json['author']['vip']?['type'] as int?,
            'vipStatus': (json['author']['vip']?['status'] as int?) ?? 0,
          },
    'stat': json['stat'] == null
        ? null
        : {
            'disliked': json['stat']['disliked'] as int?,
            'liked': json['stat']['liked'] as int?,
            'likes': json['stat']['likes'] as int?,
          },
  };

  static Map<String, dynamic> rankItem(Map<String, dynamic> json) => {
    'cover': json['cover'] as String?,
    'title': json['title'] as String?,
    'url': json['url'] as String?,
    'newEpIndexShow': json['new_ep'] == null
        ? null
        : json['new_ep']['index_show'] as String?,
    // 基线 Stat.fromJson：`(json['follow'] as int?) ?? 0`。注意 stat 整个
    // 缺失时基线的 PgcRankItemModel.stat 为 null，界面用 `stat!` 会炸；
    // 迁移后摊平成必有值的 0，这是有意的收紧。
    'follow': json['stat'] == null
        ? 0
        : (json['stat']['follow'] as int?) ?? 0,
    'view': json['stat'] == null ? 0 : (json['stat']['view'] as int?) ?? 0,
  };
}
