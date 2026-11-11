import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

/// 直播只读接口迁移的**基线对照**（S11a 第一批 / issue #61，机制见
/// ADR-011 决策五与其中 S9 补充二）。
///
/// 被取代的 `fromJson` 随本 PR 从 `lib/` 删除，借不到活着的基线，所以
/// 下面 `_Baseline` 里是迁移前那一版的**逐字拷贝**：
///
/// - `lib/models_new/live/live_area_list/{area_list,area_item}.dart`
/// - `lib/models_new/live/live_feed_index/{data,card_list,card_data,
///   card_data_item,card_data_list_item,feedback}.dart`
/// - `lib/models_new/live/live_second_list/{data,tag}.dart`
/// - `lib/models_new/live/live_follow/{data,item}.dart`
/// - `lib/models_new/live/live_room_info_h5/{data,room_info,anchor_info,
///   base_info}.dart`
/// - `lib/utils/parse_string.dart` 的 `nonNullOrEmptyString`
///
/// 它不引用 `lib/` 的任何东西，写下之后不再改——和 `*.rust.json` 快照一样
/// 是迁移当时行为的冻结证据。两者由不同实现产出，任何一边变了都会红。
///
/// ## 三处**有意**与基线不同，这里断言的是差异本身
///
/// 1. **分区 id 归一成整数。** 基线 `AreaItem.id` 是 `dynamic`，
///    `room_area_list` 回的是字符串。保持原样会让调用点
///    `e.id == areaId` 恒为假（详见 `bili-api/src/live.rs`）。对照时比的
///    是 `toString()` 后相等。
/// 2. **卡片不再分两种。** 基线首页推荐的元素套着两层 `card_data`，
///    这里拆到和分区二级列表同一种，所以对照时要先把基线那层剥掉。
/// 3. **H5 房间信息摊平成一层。** 基线分 `room_info` /
///    `anchor_info.base_info` / `watched_show` 三层。
void main() {
  const dir = 'rust/crates/bili-api/tests/fixtures/live';

  Map<String, dynamic> readRaw(String name) =>
      jsonDecode(File('$dir/$name.raw.json').readAsStringSync())
          as Map<String, dynamic>;

  dynamic readSnapshot(String name) =>
      jsonDecode(File('$dir/$name.rust.json').readAsStringSync());

  /// 卡片的逐字段对照。两条接口共用。
  void expectCard(
    Map<String, dynamic> rust,
    Map<String, dynamic> baseline,
    String where,
  ) {
    expect(rust['roomid'], baseline['roomid'], reason: '$where: roomid');
    expect(rust['uid'], baseline['uid'], reason: '$where: uid');
    expect(rust['uname'], baseline['uname'], reason: '$where: uname');
    expect(rust['face'], baseline['face'], reason: '$where: face');
    expect(rust['cover'], baseline['cover'], reason: '$where: 封面');
    expect(
      rust['system_cover'],
      baseline['systemCover'],
      reason: '$where: 首帧封面（空串回落到 cover）',
    );
    expect(rust['title'], baseline['title'], reason: '$where: 标题');
    expect(rust['area_name'], baseline['areaName'], reason: '$where: 分区名');
    expect(rust['area_v2_id'], baseline['areaV2Id'], reason: '$where: 二级分区');
    expect(
      rust['area_v2_parent_id'],
      baseline['areaV2ParentId'],
      reason: '$where: 一级分区',
    );
    expect(
      rust['watched_text'],
      baseline['watchedShowTextLarge'],
      reason: '$where: 看过人数',
    );

    final rustFeedback = rust['feedback'] as List;
    final baseFeedback = (baseline['feedback'] as List?) ?? const [];
    expect(rustFeedback.length, baseFeedback.length, reason: '$where: 反馈组数');
    for (var i = 0; i < baseFeedback.length; i++) {
      final r = rustFeedback[i] as Map<String, dynamic>;
      final b = baseFeedback[i] as Map<String, dynamic>;
      expect(r['title'], b['title'], reason: '$where: 反馈 $i 标题');
      expect(r['subtitle'], b['subtitle'], reason: '$where: 反馈 $i 副标题');
      expect(r['kind'], b['type'], reason: '$where: 反馈 $i type');

      final rustReasons = r['reasons'] as List;
      final baseReasons = (b['reasons'] as List?) ?? const [];
      expect(
        rustReasons.length,
        baseReasons.length,
        reason: '$where: 反馈 $i 理由数',
      );
      for (var j = 0; j < baseReasons.length; j++) {
        final rr = rustReasons[j] as Map<String, dynamic>;
        final br = baseReasons[j] as Map<String, dynamic>;
        expect(rr['id'], br['id'], reason: '$where: 理由 $j id');
        expect(rr['name'], br['name'], reason: '$where: 理由 $j 名称');
        expect(rr['id_type'], br['idType'], reason: '$where: 理由 $j idType');
        expect(
          rr['reason_id'],
          br['reasonId'],
          reason: '$where: 理由 $j reasonId',
        );
      }
    }
  }

  group('全部分区', () {
    test('Rust 快照与 Dart 基线逐字段一致', () {
      final raw = readRaw('area_list');
      final snap = readSnapshot('area_list') as List;
      final data = raw['data'] as Map<String, dynamic>;

      final baseline = (data['list'] as List)
          .map((e) => _Baseline.areaList(e as Map<String, dynamic>))
          .toList();
      expect(snap.length, baseline.length, reason: '分组数');

      for (var i = 0; i < baseline.length; i++) {
        final b = baseline[i];
        final r = snap[i] as Map<String, dynamic>;
        expect(r['name'], b['name'], reason: '第 $i 组: 名称');

        final rustAreas = r['areas'] as List;
        final baseAreas = (b['areaList'] as List?) ?? const [];
        expect(rustAreas.length, baseAreas.length, reason: '第 $i 组: 分区数');
        for (var j = 0; j < baseAreas.length; j++) {
          final ra = rustAreas[j] as Map<String, dynamic>;
          final ba = baseAreas[j] as Map<String, dynamic>;
          final where = '第 $i 组第 $j 个';
          // 有意差异 1：id 归一成整数，比 toString() 后的值。
          expect('${ra['id']}', '${ba['id']}', reason: '$where: id');
          expect(
            '${ra['parent_id']}',
            '${ba['parentId']}',
            reason: '$where: parentId',
          );
          expect(ra['name'], ba['name'], reason: '$where: 名称');
          expect(ra['pic'], ba['pic'], reason: '$where: 图标');
          expect(ra['parent_name'], ba['parentName'], reason: '$where: 父分区名');
        }
      }
    });
  });

  group('二级分区', () {
    test('Rust 快照与 Dart 基线逐字段一致', () {
      final raw = readRaw('room_area_list');
      final snap = readSnapshot('room_area_list') as List;
      final baseline = (raw['data'] as List)
          .map((e) => _Baseline.areaItem(e as Map<String, dynamic>))
          .toList();
      expect(snap.length, baseline.length, reason: '条数');

      for (var i = 0; i < baseline.length; i++) {
        final b = baseline[i];
        final r = snap[i] as Map<String, dynamic>;
        // 有意差异 1。这份 fixture 里前两条的 id 是字符串、第三条是整数，
        // 基线会原样保留两种类型——正是那个 bug 的来源。
        expect('${r['id']}', '${b['id']}', reason: '第 $i 条: id');
        expect(
          '${r['parent_id']}',
          '${b['parentId']}',
          reason: '第 $i 条: parentId',
        );
        expect(r['name'], b['name'], reason: '第 $i 条: 名称');
        expect(r['pic'], b['pic'], reason: '第 $i 条: 图标');
        expect(r['parent_name'], b['parentName'], reason: '第 $i 条: 父分区名');
      }

      // 把差异本身钉住：基线确实给出了两种运行时类型。
      final rawIds = (raw['data'] as List)
          .map((e) => (e as Map<String, dynamic>)['id'])
          .toList();
      expect(rawIds.whereType<String>(), isNotEmpty, reason: '样本含字符串型 id');
      expect(rawIds.whereType<int>(), isNotEmpty, reason: '样本含整数型 id');
      expect(
        snap.every((e) => (e as Map<String, dynamic>)['id'] is int),
        isTrue,
        reason: 'Rust 侧一律是整数',
      );
    });
  });

  group('收藏分区', () {
    test('Rust 快照与 Dart 基线逐字段一致', () {
      final raw = readRaw('fav_tag');
      final snap = readSnapshot('fav_tag') as List;
      final data = raw['data'] as Map<String, dynamic>;
      final baseline = (data['tags'] as List)
          .map((e) => _Baseline.areaItem(e as Map<String, dynamic>))
          .toList();
      expect(snap.length, baseline.length, reason: '条数');

      for (var i = 0; i < baseline.length; i++) {
        final b = baseline[i];
        final r = snap[i] as Map<String, dynamic>;
        expect('${r['id']}', '${b['id']}', reason: '第 $i 条: id');
        expect(r['name'], b['name'], reason: '第 $i 条: 名称');
        // 这条接口不回 parent_id。基线是 null，Rust 用 -1 占位——
        // 真实分区 id 不会是负数，0 则会与「人气」撞上。
        if (b['parentId'] == null) {
          expect(r['parent_id'], -1, reason: '第 $i 条: 缺失的 parentId');
        } else {
          expect(
            '${r['parent_id']}',
            '${b['parentId']}',
            reason: '第 $i 条: parentId',
          );
        }
      }
    });
  });

  group('首页推荐', () {
    test('Rust 快照与 Dart 基线逐字段一致', () {
      final raw = readRaw('feed_index');
      final snap = readSnapshot('feed_index') as Map<String, dynamic>;
      final data = raw['data'] as Map<String, dynamic>;
      final baseline = _Baseline.indexData(data);

      // 基线是 0/1，Rust 直接给布尔。
      expect(snap['has_more'], baseline['hasMore'] != 0, reason: 'has_more');

      // 有意差异 2：基线 cardList 的元素套着 card_data.small_card_v1。
      final baseCards = (baseline['cardList'] as List?) ?? const [];
      final rustCards = snap['cards'] as List;
      expect(rustCards.length, baseCards.length, reason: '正文卡片数');
      for (var i = 0; i < baseCards.length; i++) {
        expectCard(
          rustCards[i] as Map<String, dynamic>,
          ((baseCards[i] as Map<String, dynamic>)['cardData']
                  as Map<String, dynamic>)['smallCardV1']
              as Map<String, dynamic>,
          '正文第 $i 张',
        );
      }

      final baseFollow =
          ((baseline['followItem'] as Map<String, dynamic>?)?['cardData']
                  as Map<String, dynamic>?)?['myIdolV1']
              as Map<String, dynamic>?;
      final rustFollow = snap['follow'] as Map<String, dynamic>?;
      expect(rustFollow != null, baseFollow != null, reason: '关注区块是否存在');
      if (baseFollow != null && rustFollow != null) {
        expect(
          rustFollow['total_count'],
          (baseFollow['extraInfo'] as Map<String, dynamic>?)?['totalCount'] ??
              0,
          reason: '关注区块: 总数',
        );
        final baseItems = (baseFollow['list'] as List?) ?? const [];
        final rustItems = rustFollow['items'] as List;
        expect(rustItems.length, baseItems.length, reason: '关注区块: 条数');
        for (var i = 0; i < baseItems.length; i++) {
          expectCard(
            rustItems[i] as Map<String, dynamic>,
            baseItems[i] as Map<String, dynamic>,
            '关注第 $i 位',
          );
        }
      }

      final baseArea =
          ((baseline['areaItem'] as Map<String, dynamic>?)?['cardData']
                  as Map<String, dynamic>?)?['areaEntranceV3']
              as Map<String, dynamic>?;
      final baseAreaList = (baseArea?['list'] as List?) ?? const [];
      final rustArea = snap['area_entrance'] as List;
      expect(rustArea.length, baseAreaList.length, reason: '分区入口数');
      for (var i = 0; i < baseAreaList.length; i++) {
        expectCard(
          rustArea[i] as Map<String, dynamic>,
          baseAreaList[i] as Map<String, dynamic>,
          '分区入口第 $i 个',
        );
      }
    });

    test('未知卡片类型两侧都被忽略', () {
      final raw = readRaw('feed_index');
      final data = raw['data'] as Map<String, dynamic>;
      final types = (data['card_list'] as List)
          .map((e) => (e as Map<String, dynamic>)['card_type'])
          .toList();
      expect(types, contains('banner_v2'), reason: '样本含基线注释掉的卡片');
      expect(
        types,
        contains('brand_new_card_type_v9'),
        reason: '样本含一个虚构的新卡片类型',
      );

      final snap = readSnapshot('feed_index') as Map<String, dynamic>;
      final smallCards = types.where((t) => t == 'small_card_v1').length;
      expect(
        (snap['cards'] as List).length,
        smallCards,
        reason: '正文只收 small_card_v1',
      );
    });
  });

  group('分区二级列表', () {
    test('Rust 快照与 Dart 基线逐字段一致', () {
      final raw = readRaw('second_list');
      final snap = readSnapshot('second_list') as Map<String, dynamic>;
      final baseline = _Baseline.secondData(
        raw['data'] as Map<String, dynamic>,
      );

      expect(snap['count'], baseline['count'], reason: 'count');

      final baseCards = (baseline['cardList'] as List?) ?? const [];
      final rustCards = snap['cards'] as List;
      expect(rustCards.length, baseCards.length, reason: '卡片数');
      for (var i = 0; i < baseCards.length; i++) {
        expectCard(
          rustCards[i] as Map<String, dynamic>,
          baseCards[i] as Map<String, dynamic>,
          '第 $i 张',
        );
      }

      final baseTags = (baseline['newTags'] as List?) ?? const [];
      final rustTags = snap['tags'] as List;
      expect(rustTags.length, baseTags.length, reason: '标签数');
      for (var i = 0; i < baseTags.length; i++) {
        final r = rustTags[i] as Map<String, dynamic>;
        final b = baseTags[i] as Map<String, dynamic>;
        expect(r['name'], b['name'], reason: '标签 $i 名称');
        expect(r['sort_type'], b['sortType'], reason: '标签 $i sortType');
      }
    });
  });

  group('关注的直播', () {
    test('Rust 快照与 Dart 基线逐字段一致', () {
      final raw = readRaw('follow');
      final snap = readSnapshot('follow') as Map<String, dynamic>;
      final baseline = _Baseline.followData(
        raw['data'] as Map<String, dynamic>,
      );

      expect(snap['title'], baseline['title'], reason: '标题');
      expect(snap['page_size'], baseline['pageSize'], reason: 'pageSize');
      expect(snap['total_page'], baseline['totalPage'], reason: 'totalPage');
      expect(snap['count'], baseline['count'], reason: 'count');
      expect(snap['live_count'], baseline['liveCount'], reason: 'liveCount');

      final baseItems = (baseline['list'] as List?) ?? const [];
      final rustItems = snap['items'] as List;
      expect(rustItems.length, baseItems.length, reason: '条数（已过滤未开播）');
      // 样本里有 4 条，其中 2 条未开播/缺 live_status。
      expect(
        (raw['data'] as Map<String, dynamic>)['list'] as List,
        hasLength(greaterThan(rustItems.length)),
        reason: '确实过滤掉了一些',
      );

      for (var i = 0; i < baseItems.length; i++) {
        final r = rustItems[i] as Map<String, dynamic>;
        final b = baseItems[i] as Map<String, dynamic>;
        expect(r['roomid'], b['roomid'], reason: '第 $i 条: roomid');
        expect(r['uname'], b['uname'], reason: '第 $i 条: uname');
        expect(r['title'], b['title'], reason: '第 $i 条: 标题');
        expect(r['area_name'], b['areaName'], reason: '第 $i 条: 分区名');
        expect(r['text_small'], b['textSmall'], reason: '第 $i 条: 人数');
        expect(r['room_cover'], b['roomCover'], reason: '第 $i 条: 封面');
      }
    });
  });

  group('H5 房间信息', () {
    test('Rust 快照与 Dart 基线逐字段一致', () {
      final raw = readRaw('room_h5');
      final snap = readSnapshot('room_h5') as Map<String, dynamic>;
      final baseline = _Baseline.roomInfoH5(
        raw['data'] as Map<String, dynamic>,
      );

      // 有意差异 3：三层摊平成一层。
      final room = baseline['roomInfo'] as Map<String, dynamic>?;
      final base =
          (baseline['anchorInfo'] as Map<String, dynamic>?)?['baseInfo']
              as Map<String, dynamic>?;
      final watched = baseline['watchedShow'] as Map<String, dynamic>?;

      expect(snap['uid'], room?['uid'], reason: 'uid');
      expect(snap['title'], room?['title'], reason: '标题');
      expect(snap['cover'], room?['cover'], reason: '封面');
      expect(
        snap['app_background'],
        room?['appBackground'],
        reason: 'app 背景图',
      );
      expect(snap['anchor_name'], base?['uname'], reason: '主播昵称');
      expect(snap['anchor_face'], base?['face'], reason: '主播头像');
      expect(snap['watched_text'], watched?['textLarge'], reason: '看过人数');
    });
  });
}

/// 迁移前那一版 `fromJson` 的逐字拷贝。产出 Map 而非对象，免得把被删掉的
/// 模型类再写一遍；字段名沿用基线的属性名，对照时一眼能看出对的是哪个。
abstract final class _Baseline {
  /// `lib/utils/parse_string.dart`
  static String? nonNullOrEmptyString(String? value) {
    if (value == null || value.isEmpty) return null;
    return value;
  }

  /// `live_area_list/area_item.dart`
  static Map<String, dynamic> areaItem(Map<String, dynamic> json) => {
    'id': json['id'],
    'name': json['name'] as String?,
    'pic': json['pic'] as String?,
    'parentId': json['parent_id'],
    'parentName': json['parent_name'] as String?,
  };

  /// `live_area_list/area_list.dart`
  static Map<String, dynamic> areaList(Map<String, dynamic> json) => {
    'name': json['name'] ?? '',
    'areaList': (json['area_list'] as List<dynamic>?)
        ?.map((e) => areaItem(e as Map<String, dynamic>))
        .toList(),
  };

  /// `live_feed_index/feedback.dart`
  static Map<String, dynamic> feedback(Map<String, dynamic> json) => {
    'title': json['title'],
    'subtitle': json['subtitle'],
    'type': json['type'],
    'reasons': (json['reasons'] as List?)
        ?.map((x) => reason(x as Map<String, dynamic>))
        .toList(),
  };

  static Map<String, dynamic> reason(Map<String, dynamic> json) => {
    'id': json['id'],
    'name': json['name'],
    'idType': json['id_type'],
    'reasonId': json['reason_id'],
  };

  /// `live_feed_index/card_data_list_item.dart`（含 `systemCover` getter）
  static Map<String, dynamic> cardLiveItem(Map<String, dynamic> json) {
    final cover = json['cover'] as String?;
    final systemCover = nonNullOrEmptyString(json['system_cover'] as String?);
    return {
      'roomid': json['roomid'] ?? json['id'],
      'uid': json['uid'] as int?,
      'uname': json['uname'] as String?,
      'face': json['face'] as String?,
      'cover': cover,
      // getter: `_systemCover ?? cover`
      'systemCover': systemCover ?? cover,
      'title': json['title'] as String?,
      'areaName': json['area_name'] as String?,
      'areaV2Id': json['area_v2_id'] as int?,
      'areaV2ParentId': json['area_v2_parent_id'] as int?,
      'watchedShowTextLarge':
          (json['watched_show'] as Map<String, dynamic>?)?['text_large']
              as String?,
      'feedback': (json['feedback'] as List?)
          ?.map((x) => feedback(x as Map<String, dynamic>))
          .toList(),
    };
  }

  /// `live_feed_index/card_data_item.dart`
  static Map<String, dynamic> cardDataItem(Map<String, dynamic> json) => {
    'list': (json['list'] as List<dynamic>?)
        ?.map((e) => cardLiveItem(e as Map<String, dynamic>))
        .toList(),
    'extraInfo': json['extra_info'] == null
        ? null
        : {
            'totalCount':
                (json['extra_info'] as Map<String, dynamic>)['total_count'],
          },
  };

  /// `live_feed_index/card_data.dart`（`banner_v2` 在基线里是注释掉的）
  static Map<String, dynamic> cardData(Map<String, dynamic> json) => {
    'myIdolV1': json['my_idol_v1'] == null
        ? null
        : cardDataItem(json['my_idol_v1'] as Map<String, dynamic>),
    'areaEntranceV3': json['area_entrance_v3'] == null
        ? null
        : cardDataItem(json['area_entrance_v3'] as Map<String, dynamic>),
    'smallCardV1': json['small_card_v1'] == null
        ? null
        : cardLiveItem(json['small_card_v1'] as Map<String, dynamic>),
  };

  /// `live_feed_index/card_list.dart`
  static Map<String, dynamic> cardList(Map<String, dynamic> json) => {
    'cardType': json['card_type'] as String?,
    'cardData': json['card_data'] == null
        ? null
        : cardData(json['card_data'] as Map<String, dynamic>),
  };

  /// `live_feed_index/card_data_item.dart` 的 `kAreaEntrance`
  static Map<String, dynamic> get kAreaEntrance => {
    'cardData': {
      'areaEntranceV3': {
        'list': [
          for (final e in const [
            [0, 0, '人气'],
            [0, 9, '虚拟主播'],
            [0, 14, '聊天室'],
            [145, 1, '颜值'],
            [0, 1, '娱乐'],
            [86, 2, '英雄联盟'],
            [35, 3, '王者荣耀'],
            [0, 5, '电台'],
            [89, 2, 'CS2'],
            [0, 15, '互动玩法'],
            [0, 6, '单机游戏'],
            [21, 1, '视频唱见'],
            [301000, 301, '帮我玩'],
            [1013, 1, '团播'],
          ])
            {
              'roomid': null,
              'uid': null,
              'uname': null,
              'face': null,
              'cover': null,
              'systemCover': null,
              'title': e[2],
              'areaName': null,
              'areaV2Id': e[0],
              'areaV2ParentId': e[1],
              'watchedShowTextLarge': null,
              'feedback': null,
            },
        ],
      },
    },
  };

  /// `live_feed_index/data.dart`
  static Map<String, dynamic> indexData(Map<String, dynamic> json) {
    Map<String, dynamic>? followItem;
    Map<String, dynamic>? areaItem;
    List<Map<String, dynamic>>? cards;

    if (json['card_list'] case final List list when list.isNotEmpty) {
      for (final item in list) {
        final card = item as Map<String, dynamic>;
        switch (card['card_type']) {
          case 'my_idol_v1':
            followItem = cardList(card);
          case 'area_entrance_v3':
            areaItem = cardList(card);
          case 'small_card_v1':
            (cards ??= <Map<String, dynamic>>[]).add(cardList(card));
        }
      }
    }
    areaItem ??= kAreaEntrance;

    return {
      'cardList': cards,
      'hasMore': json['has_more'] as int?,
      'followItem': followItem,
      'areaItem': areaItem,
    };
  }

  /// `live_second_list/tag.dart`
  static Map<String, dynamic> secondTag(Map<String, dynamic> json) => {
    'name': json['name'],
    'sortType': json['sort_type'],
  };

  /// `live_second_list/data.dart`
  static Map<String, dynamic> secondData(Map<String, dynamic> json) => {
    'count': json['count'] as int?,
    'cardList': (json['list'] as List<dynamic>?)
        ?.map((e) => cardLiveItem(e as Map<String, dynamic>))
        .toList(),
    'newTags': (json['new_tags'] as List<dynamic>?)
        ?.map((e) => secondTag(e as Map<String, dynamic>))
        .toList(),
  };

  /// `live_follow/item.dart`
  static Map<String, dynamic> followItem(Map<String, dynamic> json) => {
    'roomid': json['roomid'] as int?,
    'uname': json['uname'] as String?,
    'title': json['title'] as String?,
    'areaName': json['area_name'] as String?,
    'textSmall': json['text_small'] as String?,
    'roomCover': json['room_cover'] as String?,
  };

  /// `live_follow/data.dart`（`fromJson` 里就按 live_status 过滤了）
  static Map<String, dynamic> followData(Map<String, dynamic> json) => {
    'title': json['title'] as String?,
    'pageSize': json['pageSize'] as int?,
    'totalPage': json['totalPage'] as int?,
    'list': (json['list'] as List<dynamic>?)
        ?.cast<Map<String, dynamic>>()
        .where((i) => i['live_status'] == 1)
        .map(followItem)
        .toList(),
    'count': json['count'] as int?,
    'liveCount': json['live_count'] as int?,
  };

  /// `live_room_info_h5/{data,room_info,anchor_info,base_info}.dart`
  static Map<String, dynamic> roomInfoH5(Map<String, dynamic> json) => {
    'roomInfo': json['room_info'] == null
        ? null
        : {
            'uid': (json['room_info'] as Map<String, dynamic>)['uid'] as int?,
            'title':
                (json['room_info'] as Map<String, dynamic>)['title'] as String?,
            'cover':
                (json['room_info'] as Map<String, dynamic>)['cover'] as String?,
            'appBackground':
                (json['room_info'] as Map<String, dynamic>)['app_background']
                    as String?,
          },
    'anchorInfo': json['anchor_info'] == null
        ? null
        : {
            'baseInfo':
                (json['anchor_info'] as Map<String, dynamic>)['base_info'] ==
                    null
                ? null
                : {
                    'uname':
                        ((json['anchor_info']
                                    as Map<String, dynamic>)['base_info']
                                as Map<String, dynamic>)['uname']
                            as String?,
                    'face':
                        ((json['anchor_info']
                                    as Map<String, dynamic>)['base_info']
                                as Map<String, dynamic>)['face']
                            as String?,
                  },
          },
    'watchedShow': json['watched_show'] == null
        ? null
        : {
            'textLarge':
                (json['watched_show'] as Map<String, dynamic>)['text_large']
                    as String?,
          },
  };
}
