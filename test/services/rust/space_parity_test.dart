import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:kilikili/grpc/bilibili/app/interfaces/v1.pb.dart'
    show SearchArchiveReply;
import 'package:kilikili/utils/em.dart';

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

  // ------------------------------------------------------------ 空间主页

  test('空间主页：Rust 快照与基线逐字段一致', () {
    for (final name in ['space_page', 'space_page_alt']) {
      final data = read('$name.raw.json')['data'] as Map<String, dynamic>;
      final snapshot = read('$name.rust.json') as Map<String, dynamic>;
      final baseline = _BaselineSpaceData(data);

      expect(snapshot['relation'], baseline.relation, reason: '$name relation');
      expect(
        snapshot['guest_relation'],
        baseline.guestRelation,
        reason: '$name guestRelation',
      );
      expect(snapshot['medal'], baseline.medal, reason: '$name medal');
      expect(
        snapshot['default_tab'],
        baseline.defaultTab,
        reason: '$name defaultTab',
      );
      expect(
        snapshot['rel_special'],
        baseline.relSpecial,
        reason: '$name relSpecial',
      );
      // 基线的 `hasItem` 是 `bool?`，八块全空时它是 false。
      expect(
        snapshot['has_item'],
        baseline.hasItem,
        reason: '$name hasItem',
      );

      final card = data['card'] as Map<String, dynamic>?;
      final snapCard = snapshot['card'] as Map<String, dynamic>?;
      expect(snapCard == null, card == null, reason: '$name card 有无');
      if (card != null && snapCard != null) {
        expect(snapCard['mid'], card['mid'], reason: '$name card.mid');
        expect(snapCard['name'], card['name'], reason: '$name card.name');
        expect(snapCard['face'], card['face'], reason: '$name card.face');
        expect(snapCard['sign'], card['sign'], reason: '$name card.sign');
        expect(snapCard['fans'], card['fans'], reason: '$name card.fans');
        expect(
          snapCard['attention'],
          card['attention'],
          reason: '$name card.attention',
        );
        expect(
          snapCard['like_num'],
          card['likes']?['like_num'],
          reason: '$name card.likes.likeNum',
        );
        expect(
          snapCard['level'],
          card['level_info']?['current_level'],
          reason: '$name card.levelInfo.currentLevel',
        );
        expect(
          snapCard['level_identity'],
          card['level_info']?['identity'],
          reason: '$name card.levelInfo.identity',
        );
        expect(
          snapCard['official_type'],
          card['official_verify']?['type'],
          reason: '$name card.officialVerify.type',
        );
        expect(
          snapCard['official_desc'],
          card['official_verify']?['desc'],
          reason: '$name card.officialVerify.desc',
        );
        expect(
          snapCard['official_splice_title'],
          card['official_verify']?['splice_title'],
          reason: '$name card.officialVerify.spliceTitle',
        );
        final vip = _baselineVip(card['vip'] as Map<String, dynamic>?);
        expect(
          snapCard['vip_status'],
          card['vip'] == null ? null : vip.status,
          reason: '$name card.vip.status',
        );
        expect(
          snapCard['vip_type'],
          card['vip'] == null ? null : vip.type,
          reason: '$name card.vip.type',
        );
        expect(
          snapCard['silence'],
          card['silence'],
          reason: '$name card.silence',
        );
        // 基线 `SpaceCard.spaceTag` 只留这两类。
        final tags = (card['space_tag'] as List?)
            ?.where(
              (e) => const [
                'location',
                'real_name',
              ].contains(e['type']),
            )
            .toList();
        expect(
          (snapCard['tags'] as List).length,
          tags?.length ?? 0,
          reason: '$name card.spaceTag 过滤后条数',
        );
      }

      final live = data['live'] as Map<String, dynamic>?;
      final snapLive = snapshot['live'] as Map<String, dynamic>?;
      expect(snapLive == null, live == null, reason: '$name live 有无');
      if (live != null && snapLive != null) {
        expect(snapLive['room_status'], live['roomStatus']);
        expect(snapLive['live_status'], live['liveStatus']);
        expect(snapLive['room_id'], live['roomid']);
      }

      // 基线的 `Elec.list` 映射成 `ElecItem`，只取昵称与头像（它的构造
      // 函数压根不收 mid）。Rust 也不给 mid，理由写在 `into_domain_anonymous`。
      final elec = data['elec'] as Map<String, dynamic>?;
      final snapElec = snapshot['elec'] as Map<String, dynamic>?;
      expect(snapElec == null, elec == null, reason: '$name elec 有无');
      if (elec != null && snapElec != null) {
        expect(snapElec['total'], elec['total'], reason: '$name elec.total');
        final list = (elec['list'] as List?) ?? const [];
        final snapList = snapElec['list'] as List;
        expect(snapList.length, list.length, reason: '$name elec.list 条数');
        for (var i = 0; i < list.length; i++) {
          expect(snapList[i]['name'], list[i]['uname']);
          expect(snapList[i]['face'], list[i]['avatar']);
          expect(snapList[i]['mid'], isNull, reason: '基线的 ElecItem 没有 mid');
        }
      }

      // 基线 `Guard` 的人数是正则从 `desc` 开头抠出来的字符串。
      final guard = data['guard'] as Map<String, dynamic>?;
      final snapGuard = snapshot['guard'] as Map<String, dynamic>?;
      expect(snapGuard == null, guard == null, reason: '$name guard 有无');
      if (guard != null && snapGuard != null) {
        final desc = guard['desc'] as String?;
        expect(
          snapGuard['count'],
          desc == null ? null : RegExp(r'^(\d+)').firstMatch(desc)?.group(1),
          reason: '$name guard.count',
        );
      }

      // 五个视频块共用 `SpaceArchiveItem`。
      for (final (key, snapKey) in const [
        ('archive', 'archive'),
        ('season', 'season'),
        ('coin_archive', 'coin_archive'),
        ('like_archive', 'like_archive'),
        ('comic', 'comic'),
      ]) {
        final block = data[key] as Map<String, dynamic>?;
        final snapBlock = snapshot[snapKey] as Map<String, dynamic>?;
        expect(snapBlock == null, block == null, reason: '$name $key 有无');
        if (block == null || snapBlock == null) continue;
        expect(snapBlock['count'], block['count'], reason: '$name $key.count');
        final items = (block['item'] as List?) ?? const [];
        final snapItems = snapBlock['items'] as List;
        expect(snapItems.length, items.length, reason: '$name $key 条数');
        for (var i = 0; i < items.length; i++) {
          _expectArchiveItem(snapItems[i], items[i], '$name $key[$i]');
        }
      }
    }
  });

  test('空间主页：预约那一块与基线逐字段一致', () {
    final data = read('space_reservation.raw.json')['data'];
    final snapshot = read('space_reservation.rust.json') as Map<String, dynamic>;
    final items = data['reservation_card_list'] as List;
    final snapItems = snapshot['reservations'] as List;
    expect(snapItems.length, items.length);
    for (var i = 0; i < items.length; i++) {
      final e = items[i] as Map<String, dynamic>;
      final s = snapItems[i] as Map<String, dynamic>;
      expect(s['sid'], e['sid']);
      expect(s['name'], e['name']);
      // 基线：`total: json['total'] ?? 0`、`isFollow: json['is_follow'] == 1`。
      expect(s['total'], e['total'] ?? 0);
      expect(s['is_follow'], e['is_follow'] == 1);
      // 基线用 `nonNullOrEmptyString`，空串按没有。
      final desc = e['desc_text_1']?['text'] as String?;
      expect(s['desc_text'], (desc == null || desc.isEmpty) ? null : desc);
      expect(s['dynamic_id'], e['dynamic_id']);
      final lottery = e['lottery_prize_info'] as Map<String, dynamic>?;
      expect((s['lottery'] == null), lottery == null);
      if (lottery != null) {
        expect(s['lottery']['text'], lottery['text']);
        expect(s['lottery']['jump_url'], lottery['jump_url']);
      }
    }
  });

  test('投稿列表：Rust 快照与基线逐字段一致', () {
    for (final name in [
      'space_archive',
      'space_archive_extras',
      'space_archive_empty',
    ]) {
      final data = read('$name.raw.json')['data'] as Map<String, dynamic>;
      final snapshot = read('$name.rust.json') as Map<String, dynamic>;

      expect(snapshot['count'], data['count'], reason: '$name count');
      expect(snapshot['has_next'], data['has_next'], reason: '$name hasNext');
      expect(snapshot['has_prev'], data['has_prev'], reason: '$name hasPrev');
      expect(snapshot['next'], data['next'], reason: '$name next');

      final button = data['episodic_button'] as Map<String, dynamic>?;
      final snapButton = snapshot['episodic_button'] as Map<String, dynamic>?;
      expect(snapButton == null, button == null, reason: '$name 按钮有无');
      if (button != null && snapButton != null) {
        expect(snapButton['text'], button['text']);
        expect(snapButton['uri'], button['uri']);
      }

      final items = (data['item'] as List?) ?? const [];
      final snapItems = snapshot['items'] as List;
      expect(snapItems.length, items.length, reason: '$name 条数');
      for (var i = 0; i < items.length; i++) {
        _expectArchiveItem(snapItems[i], items[i], '$name[$i]');
      }
    }
  });

  // --------------------------------------- 网页端的投稿搜索与合集/列表

  test('投稿搜索：Rust 快照与基线逐字段一致', () {
    final data = read('space_archive_search.raw.json')['data'];
    final snapshot = read('space_archive_search.rust.json');

    expect(snapshot['count'], data['page']['count'], reason: 'page.count');

    // 基线 `SearchArchiveList.fromJson`：先 slist，再从 tlist 里挑 196。
    final tags = [
      ...((data['list']['slist'] as List?) ?? const []),
      if (data['list']['tlist']?['196'] case final lesson?) lesson,
    ];
    final snapTags = snapshot['tags'] as List;
    expect(snapTags.length, tags.length, reason: '标签条数');
    for (var i = 0; i < tags.length; i++) {
      expect(snapTags[i]['tid'], tags[i]['tid']);
      expect(snapTags[i]['count'], tags[i]['count']);
      expect(snapTags[i]['name'], tags[i]['name']);
      expect(snapTags[i]['special_type'], tags[i]['special_type']);
    }

    final vlist = data['list']['vlist'] as List;
    final snapItems = snapshot['items'] as List;
    expect(snapItems.length, vlist.length);
    for (var i = 0; i < vlist.length; i++) {
      final baseline = _BaselineVListItem(vlist[i] as Map<String, dynamic>);
      final s = snapItems[i] as Map<String, dynamic>;
      expect(s['aid'], baseline.aid, reason: 'vlist[$i] aid');
      expect(s['bvid'], baseline.bvid, reason: 'vlist[$i] bvid');
      expect(s['cover'], baseline.cover, reason: 'vlist[$i] cover');
      expect(s['title'], baseline.title, reason: 'vlist[$i] title');
      expect(s['desc'], baseline.desc, reason: 'vlist[$i] desc');
      expect(s['pubdate'], baseline.pubdate, reason: 'vlist[$i] pubdate');
      expect(s['duration'], baseline.duration, reason: 'vlist[$i] duration');
      expect(s['view'], baseline.view, reason: 'vlist[$i] play');
      expect(s['danmaku'], baseline.danmu, reason: 'vlist[$i] video_review');
      expect(s['owner_mid'], baseline.ownerMid, reason: 'vlist[$i] mid');
      expect(s['owner_name'], baseline.ownerName, reason: 'vlist[$i] author');
      expect(s['season_id'], baseline.seasonId, reason: 'vlist[$i] season_id');
      expect(
        s['redirect_url'],
        baseline.redirectUrl,
        reason: 'vlist[$i] jump_url',
      );
      expect(s['progress'], baseline.progress, reason: 'vlist[$i] progress');
      // 基线在模型里直接写死中文；Rust 只给类别，文案在
      // `space_web_compat.dart`。这里比的是「类别对不对」。
      expect(
        switch (s['badge']) {
          'Lesson' => '课堂',
          'Charging' => '充电专属',
          'Union' => '合作',
          _ => null,
        },
        baseline.badge,
        reason: 'vlist[$i] badge',
      );
    }
  });

  test('合集与列表：Rust 快照与基线逐字段一致', () {
    for (final name in ['space_season_web', 'space_series_web']) {
      final data = read('$name.raw.json')['data'] as Map<String, dynamic>;
      final snapshot = read('$name.rust.json') as Map<String, dynamic>;
      expect(snapshot['total'], data['page']['total'], reason: '$name total');

      final archives = data['archives'] as List;
      final snapItems = snapshot['items'] as List;
      expect(snapItems.length, archives.length, reason: '$name 条数');
      for (var i = 0; i < archives.length; i++) {
        final e = archives[i] as Map<String, dynamic>;
        final s = snapItems[i] as Map<String, dynamic>;
        // 基线 `SeasonArchive.fromJson` 的逐行转写。
        expect(s['aid'], e['aid'], reason: '$name[$i] aid');
        expect(s['bvid'], e['bvid'], reason: '$name[$i] bvid');
        expect(s['cover'], e['pic'], reason: '$name[$i] pic');
        expect(s['title'], e['title'], reason: '$name[$i] title');
        expect(s['pubdate'], e['pubdate'], reason: '$name[$i] pubdate');
        expect(s['duration'], e['duration'], reason: '$name[$i] duration');
        expect(s['view'], e['stat']?['view'], reason: '$name[$i] stat.view');
        expect(
          s['danmaku'],
          e['stat']?['danmaku'],
          reason: '$name[$i] stat.danmaku',
        );
        // 基线读的是 `upMid`，当前响应里没有这个键，所以两边都是空的。
        expect(s['owner_mid'], e['upMid'], reason: '$name[$i] upMid');
      }
    }
  });

  // ------------------------------------------------- gRPC 投稿搜索

  test('gRPC 投稿搜索：Rust 快照与基线的 protobuf 解码逐字段一致', () {
    const dir = 'rust/crates/bili-api/tests/fixtures/space_grpc';
    final reply = SearchArchiveReply.fromBuffer(
      File('$dir/search_archive.raw.pb').readAsBytesSync(),
    );
    final snapshot =
        jsonDecode(File('$dir/search_archive.rust.json').readAsStringSync())
            as Map<String, dynamic>;

    expect(snapshot['total'], reply.total.toInt(), reason: 'total');
    final snapItems = snapshot['items'] as List;
    expect(snapItems.length, reply.archives.length, reason: '条数');
    for (var i = 0; i < reply.archives.length; i++) {
      final item = reply.archives[i];
      final arc = item.archive;
      final s = snapItems[i] as Map<String, dynamic>;
      expect(s['aid'], arc.hasAid() ? arc.aid.toInt() : null, reason: '[$i] aid');
      expect(s['cover'], arc.hasPic() ? arc.pic : null, reason: '[$i] pic');
      expect(
        s['pubdate'],
        arc.hasPubdate() ? arc.pubdate.toInt() : null,
        reason: '[$i] pubdate',
      );
      expect(
        s['duration'],
        arc.hasDuration() ? arc.duration.toInt() : null,
        reason: '[$i] duration',
      );
      expect(
        s['first_cid'],
        arc.hasFirstCid() ? arc.firstCid.toInt() : null,
        reason: '[$i] firstCid',
      );
      expect(
        s['view'],
        arc.hasStat() && arc.stat.hasView() ? arc.stat.view : null,
        reason: '[$i] stat.view',
      );
      expect(
        s['danmaku'],
        arc.hasStat() && arc.stat.hasDanmaku() ? arc.stat.danmaku : null,
        reason: '[$i] stat.danmaku',
      );
      expect(
        s['author_name'],
        arc.hasAuthor() && arc.author.hasName() ? arc.author.name : null,
        reason: '[$i] author.name',
      );
      expect(s['is_pugv'], item.isPugv, reason: '[$i] isPugv');
      // 基线判的是 `hasRedirectUrl()`，而空串与未设置在 wire 上是一回事。
      expect(
        s['redirect_url'],
        arc.redirectUrl.isEmpty ? null : arc.redirectUrl,
        reason: '[$i] redirectUrl',
      );
      expect(s['uri'], item.uri.isEmpty ? null : item.uri, reason: '[$i] uri');
      // 基线 `DimensionExt.isVertical` 的逐行转写。
      final d = arc.dimension;
      expect(
        s['is_vertical'],
        d.rotate.toInt() == 1 ? d.width > d.height : d.height > d.width,
        reason: '[$i] isVertical',
      );
      // 基线 `Em.regTitle` 的结果：Rust 的分段与它一致。
      expect(
        (s['title_spans'] as List)
            .map((e) => (isEm: e['is_em'] as bool, text: e['text'] as String))
            .toList(),
        Em.regTitle(arc.title),
        reason: '[$i] titleSpans',
      );
    }
  });
}

/// 基线 `VListItemModel.fromJson` 的逐行转写。
class _BaselineVListItem {
  _BaselineVListItem(Map<String, dynamic> json)
    : cover = json['pic'],
      desc = json['description'],
      title = json['title'],
      pubdate = json['created'],
      duration = json['length'] == null
          ? 0
          : _parseDuration(json['length'] as String),
      aid = json['aid'],
      bvid = json['bvid'],
      view = json['play'],
      danmu = json['video_review'],
      ownerMid = json['mid'],
      ownerName = json['author'],
      seasonId = json['season_id'],
      redirectUrl = json['jump_url'] {
    if (json['is_lesson_video'] == 1) {
      badge = '课堂';
    } else if (json['is_charging_arc'] == true) {
      badge = '充电专属';
    } else if (json['is_union_video'] == 1) {
      badge = '合作';
    }
    final position = json['playback_position'] as num?;
    if (position != null) {
      progress = position == 100 ? -1 : ((position / 100) * duration).round();
    }
  }

  final String? cover;
  final String? desc;
  final String? title;
  final int? pubdate;
  final int duration;
  final int? aid;
  final String? bvid;
  final int? view;
  final int? danmu;
  final int? ownerMid;
  final String? ownerName;
  final int? seasonId;
  final String? redirectUrl;
  String? badge;
  int? progress;
}

/// 基线 `DurationUtils.parseDuration` 的逐行转写：`"1:02:03"` → 秒。
int _parseDuration(String value) {
  final parts = value.split(':').map(int.parse).toList();
  var seconds = 0;
  for (final part in parts) {
    seconds = seconds * 60 + part;
  }
  return seconds;
}

/// 基线 `SpaceArchiveItem.fromJson` 的逐字段对照。
///
/// 两处**刻意与基线不同**，各自有理由，不在这里比：
/// - `owner`：基线造 `Owner(mid: 0, name: author)` 这个假 owner，Rust 只
///   给 `author`（见 `bili_domain::space::SpaceArchiveItem`）；
/// - `badges`：基线保留 `text` 为空的角标再 join，Rust 把它们丢掉。
void _expectArchiveItem(dynamic snap, dynamic raw, String where) {
  expect(snap['title'], raw['title'], reason: '$where title');
  expect(snap['cover'], raw['cover'], reason: '$where cover');
  expect(snap['uri'], raw['uri'], reason: '$where uri');
  expect(snap['param'], raw['param'], reason: '$where param');
  expect(snap['goto'], raw['goto'], reason: '$where goto');
  expect(snap['bvid'], raw['bvid'], reason: '$where bvid');
  // 基线：`cid = json['first_cid']`。
  expect(snap['cid'], raw['first_cid'], reason: '$where cid');
  // 基线：`duration = json['duration'] ?? -1`。
  expect(snap['duration'], raw['duration'] ?? -1, reason: '$where duration');
  // 基线的 `PlayStat.fromJson` 读的是 `play` 与 `danmaku`。
  expect(snap['play'], raw['play'], reason: '$where play');
  expect(snap['danmaku'], raw['danmaku'], reason: '$where danmaku');
  expect(snap['author'], raw['author'], reason: '$where author');
  expect(
    snap['publish_time_text'],
    raw['publish_time_text'],
    reason: '$where publishTimeText',
  );
  expect(snap['is_steins'], raw['is_steins'], reason: '$where isSteins');
  expect(
    snap['is_cooperation'],
    raw['is_cooperation'],
    reason: '$where isCooperation',
  );
  expect(snap['is_pgc'], raw['is_pgc'], reason: '$where isPgc');
  expect(snap['is_pugv'], raw['is_pugv'], reason: '$where isPugv');
  expect(snap['styles'], raw['styles'], reason: '$where styles');
  expect(snap['label'], raw['label'], reason: '$where label');
  expect(
    snap['season_mtime'],
    raw['season']?['mtime'],
    reason: '$where season.mtime',
  );
  final history = raw['history'] as Map<String, dynamic>?;
  expect(snap['history'] == null, history == null, reason: '$where history 有无');
  if (history != null) {
    expect(snap['history']['progress'], history['progress']);
    expect(snap['history']['duration'], history['duration']);
  }
  // 基线保留空文案的角标，Rust 丢掉；这里只比非空的那些。
  final badges = ((raw['badges'] as List?) ?? const [])
      .map((e) => e['text'] as String?)
      .where((e) => e != null && e.isNotEmpty)
      .toList();
  expect(snap['badges'], badges, reason: '$where badges');
}

/// 基线 `SpaceData.fromJson` 里那几个**算出来**的值的逐行转写。
class _BaselineSpaceData {
  _BaselineSpaceData(Map<String, dynamic> json)
    : relation = json['relation'] as int?,
      guestRelation = json['guest_relation'] as int?,
      medal = json['medal'] as int?,
      defaultTab = json['default_tab'] as String?,
      relSpecial = (json['rel_special'] as num?)?.toInt(),
      hasItem =
          json['archive']?['item']?.isNotEmpty == true ||
          json['favourite2']?['item']?.isNotEmpty == true ||
          json['coin_archive']?['item']?.isNotEmpty == true ||
          json['like_archive']?['item']?.isNotEmpty == true ||
          json['article']?['item']?.isNotEmpty == true ||
          json['audios']?['item']?.isNotEmpty == true ||
          json['comic']?['item']?.isNotEmpty == true ||
          json['season']?['item']?.isNotEmpty == true;

  final int? relation;
  final int? guestRelation;
  final int? medal;
  final String? defaultTab;
  final int? relSpecial;
  final bool hasItem;
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
