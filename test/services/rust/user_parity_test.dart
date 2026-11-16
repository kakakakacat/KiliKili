import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

/// 「我的」那一族的**一次性基线对照**（S7 #57，机制见 ADR-011 决策五）。
///
/// ## 为什么是逐行转写，而不是 import 基线模型
///
/// 这一族的基线模型在**本 PR 里已经删除**（`lib/models/user/`、
/// `lib/models_new/{coin_log,login_log,space_setting,user_real_name}`、
/// `lib/models_new/video/video_tag/`）——它们的生产者全部迁进了 Rust，照
/// ADR-011 决策四不留旧实现。被删掉的东西 import 不进来。
///
/// 下面的 `_Baseline*` 是那几个 `fromJson` 的逐行转写，取自本 PR 的父提交，
/// 写下之后不再改，也不许顺手修正任何怪异之处——包括 `UserInfoData` 里
/// `money` 那句 `is int ? toDouble() : ...` 和 `LevelInfo` 的满级特判。
///
/// `SubItemModel` **没有**转写：它还活着（合集详情页那条接口还在 Dart，
/// 见 `lib/services/rust/sub_compat.dart`），对照放在那条切片迁完时做更有
/// 意义；这里只验 Rust 自己的几处摊平不丢字段。
///
/// ## 这个测试的寿命
///
/// 一次性。基线实现已经不在仓库里，本文件证明的是「删掉它们的那一刻，
/// Rust 的输出与它们逐字段一致」。**下一个碰这一族的 PR 可以连同它一起
/// 删**，长期留在仓库里的是
/// `rust/crates/bili-api/tests/fixtures/user/*.rust.json` 那几份快照。
void main() {
  const dir = 'rust/crates/bili-api/tests/fixtures/user';

  dynamic read(String name) =>
      jsonDecode(File('$dir/$name').readAsStringSync());

  test('登录用户信息：Rust 快照与基线逐字段一致', () {
    for (final name in ['user_info', 'user_info_max_level']) {
      final data = read('$name.raw.json')['data'] as Map<String, dynamic>;
      final snapshot = read('$name.rust.json') as Map<String, dynamic>;
      final baseline = _BaselineUserInfo.fromJson(data);

      expect(snapshot['is_login'], baseline.isLogin, reason: '$name isLogin');
      expect(snapshot['mid'], baseline.mid, reason: '$name mid');
      expect(snapshot['uname'], baseline.uname, reason: '$name uname');
      expect(snapshot['face'], baseline.face, reason: '$name face');
      expect(snapshot['money'], baseline.money, reason: '$name money');
      expect(snapshot['vip_status'], baseline.vipStatus, reason: '$name vipStatus');
      expect(snapshot['vip_type'], baseline.vipType, reason: '$name vipType');
      expect(
        snapshot['is_senior_member'],
        baseline.isSeniorMember,
        reason: '$name isSeniorMember',
      );

      final level = snapshot['level_info'] as Map<String, dynamic>?;
      expect(level?['current_level'], baseline.currentLevel, reason: '$name 等级');
      expect(level?['current_min'], baseline.currentMin, reason: '$name 本级下限');
      expect(level?['current_exp'], baseline.currentExp, reason: '$name 当前经验');
      expect(level?['next_exp'], baseline.nextExp, reason: '$name 下一级经验');
    }
  });

  test('满级时下一级经验取当前经验 —— 照基线的特判', () {
    final snapshot = read('user_info_max_level.rust.json') as Map<String, dynamic>;
    final level = snapshot['level_info'] as Map<String, dynamic>;
    expect(level['current_level'], 6);
    expect(
      level['next_exp'],
      level['current_exp'],
      reason: '服务端给的 next_exp 是 0，照用会让进度条分母为 0',
    );
  });

  test('三个计数：Rust 快照与基线逐字段一致', () {
    final data = read('user_stat.raw.json')['data'] as Map<String, dynamic>;
    final snapshot = read('user_stat.rust.json') as Map<String, dynamic>;
    expect(snapshot['following'], data['following']);
    expect(snapshot['follower'], data['follower']);
    expect(snapshot['dynamic_count'], data['dynamic_count']);
  });

  test('硬币流水：delta 的格式化与基线一致', () {
    final data = read('coin_log.raw.json')['data']['list'] as List;
    final snapshot = read('coin_log.rust.json') as List;
    final baseline = data
        .map((e) => _BaselineCoinLogItem.fromJson(e as Map<String, dynamic>))
        .toList();

    expect(snapshot.length, baseline.length);
    for (var i = 0; i < baseline.length; i++) {
      final got = snapshot[i] as Map<String, dynamic>;
      expect(got['time'], baseline[i].time, reason: '第 $i 条时间');
      expect(got['delta'], baseline[i].delta, reason: '第 $i 条变化');
      expect(got['reason'], baseline[i].reason, reason: '第 $i 条原因');
    }
    expect(baseline[0].delta, '2', reason: 'fixture 第一条就是整数');
  });

  test('登录日志：缺字段落到空串，照基线', () {
    final data = read('login_log.raw.json')['data']['list'] as List;
    final snapshot = read('login_log.rust.json') as List;
    final baseline = data
        .map((e) => _BaselineLoginLogItem.fromJson(e as Map<String, dynamic>))
        .toList();

    for (var i = 0; i < baseline.length; i++) {
      final got = snapshot[i] as Map<String, dynamic>;
      expect(got['ip'], baseline[i].ip, reason: '第 $i 条 ip');
      expect(got['time_at'], baseline[i].timeAt, reason: '第 $i 条时间');
      expect(got['geo'], baseline[i].geo, reason: '第 $i 条位置');
    }
    expect(baseline[1].geo, '', reason: 'fixture 第二条没有 geo');
  });

  test('隐私设置：十五项的 key、反相与分组都和基线一致', () {
    final data =
        read('space_setting.raw.json')['data']['privacy'] as Map<String, dynamic>;
    final snapshot = (read('space_setting.rust.json') as List)
        .cast<Map<String, dynamic>>();
    final baseline = _baselinePrivacy(data);

    expect(snapshot.length, baseline.length, reason: '十五项一个都不能少');
    for (var i = 0; i < baseline.length; i++) {
      expect(snapshot[i]['key'], baseline[i].key, reason: '第 $i 项 key');
      expect(snapshot[i]['value'], baseline[i].value, reason: '第 $i 项值');
      expect(
        snapshot[i]['reversed'],
        baseline[i].isReverse,
        reason: '第 $i 项是否反相 —— 读错它就是把隐私设置显示反',
      );
      expect(snapshot[i]['group'], baseline[i].group, reason: '第 $i 项分组');
    }
  });

  test('服务端没下发的开关是 null，不替它填默认值', () {
    final snapshot = (read('space_setting.rust.json') as List)
        .cast<Map<String, dynamic>>();
    final school =
        snapshot.firstWhere((e) => e['key'] == 'disable_show_school');
    expect(
      school['value'],
      isNull,
      reason: 'fixture 里故意没给这一项；填 0 或 1 都等于替用户做了隐私决定',
    );
  });

  test('视频标签：Rust 快照与基线逐字段一致', () {
    final data = read('video_tags.raw.json')['data'] as List;
    final snapshot = (read('video_tags.rust.json') as List)
        .cast<Map<String, dynamic>>();
    final baseline = data
        .map((e) => _BaselineVideoTag.fromJson(e as Map<String, dynamic>))
        .toList();

    for (var i = 0; i < baseline.length; i++) {
      expect(snapshot[i]['tag_id'], baseline[i].tagId, reason: '第 $i 个 id');
      expect(snapshot[i]['tag_name'], baseline[i].tagName, reason: '第 $i 个名字');
      expect(snapshot[i]['tag_type'], baseline[i].tagType, reason: '第 $i 个类型');
      expect(snapshot[i]['music_id'], baseline[i].musicId, reason: '第 $i 个曲目');
    }
  });

  test('实名信息：Rust 快照与基线逐字段一致', () {
    final data = read('user_real_name.raw.json')['data'] as Map<String, dynamic>;
    final snapshot = read('user_real_name.rust.json') as Map<String, dynamic>;
    expect(snapshot['name'], data['name']);
    final reject = snapshot['reject_page'] as Map<String, dynamic>?;
    expect(reject?['title'], data['reject_page']['title']);
    expect(reject?['text'], data['reject_page']['text']);
  });

  test('订阅列表：摊平的作者与计数没有丢字段', () {
    // `SubItemModel` 还活着（见文件头），所以这里不转写它，只验 Rust 把
    // `upper` 与 `cnt_info` 两层摊平之后每个值都还在原位。
    final data = read('sub_list.raw.json')['data'] as Map<String, dynamic>;
    final snapshot = read('sub_list.rust.json') as Map<String, dynamic>;
    final first = (snapshot['list'] as List).first as Map<String, dynamic>;
    final raw = (data['list'] as List).first as Map<String, dynamic>;

    expect(snapshot['count'], data['count']);
    expect(snapshot['has_more'], data['has_more']);
    expect(first['upper_mid'], raw['upper']['mid']);
    expect(first['upper_name'], raw['upper']['name']);
    expect(first['upper_face'], raw['upper']['face']);
    expect(first['play_count'], raw['cnt_info']['play']);
    expect(first['collect_count'], raw['cnt_info']['danmaku']);
    expect(first['kind'], raw['type'], reason: '协议里叫 type，Rust 侧避开关键字叫 kind');

    final second = (snapshot['list'] as List)[1] as Map<String, dynamic>;
    expect(second['upper_mid'], isNull, reason: '没有 upper 不是错误');
    expect(second['play_count'], isNull, reason: '没有 cnt_info 不是错误');
  });
}

/// 已删除的 `lib/models/user/info.dart` 的 `UserInfoData.fromJson` 与
/// `LevelInfo.fromJson` 的逐行转写，两层摊平成一个类。
class _BaselineUserInfo {
  bool? isLogin;
  String? face;
  int? mid;
  double? money;
  String? uname;
  int? vipStatus;
  int? vipType;
  int? isSeniorMember;
  int? currentLevel;
  int? currentMin;
  int? currentExp;
  int? nextExp;

  _BaselineUserInfo.fromJson(Map<String, dynamic> json) {
    isLogin = json['isLogin'] ?? false;
    face = json['face'];
    mid = json['mid'];
    money = json['money'] is int ? json['money'].toDouble() : json['money'];
    uname = json['uname'];
    vipStatus = json['vipStatus'];
    vipType = json['vipType'];
    isSeniorMember = json['is_senior_member'];

    final level = json['level_info'];
    if (level != null) {
      currentLevel = level['current_level'];
      currentMin = level['current_min'];
      currentExp = level['current_exp'];
      nextExp = level['current_level'] == 6
          ? level['current_exp']
          : level['next_exp'];
    }
  }
}

/// 已删除的 `lib/models_new/coin_log/list.dart` 的逐行转写。
class _BaselineCoinLogItem {
  final String time;
  final String delta;
  final String reason;

  _BaselineCoinLogItem.fromJson(Map<String, dynamic> json)
    : time = json['time'],
      delta = (json['delta'] as num).toString(),
      reason = json['reason'];
}

/// 已删除的 `lib/models_new/login_log/list.dart` 的逐行转写。
class _BaselineLoginLogItem {
  final String ip;
  final String timeAt;
  final String geo;

  _BaselineLoginLogItem.fromJson(Map<String, dynamic> json)
    : ip = json['ip'] ?? '',
      timeAt = json['time_at'] ?? '',
      geo = json['geo'] ?? '';
}

/// 已删除的 `lib/models_new/space_setting/privacy.dart` 的一项。
class _BaselinePrivacyItem {
  _BaselinePrivacyItem(this.key, this.value, this.isReverse, this.group);
  final String key;
  final int? value;
  final bool isReverse;
  final int group;
}

/// `Privacy.fromJson` 的逐行转写。基线把标题也写在这里，标题现在归界面，
/// 所以转写里只留 key / 值 / 反相 / 分组这四样——它们才是被比对的东西。
List<_BaselinePrivacyItem> _baselinePrivacy(Map<String, dynamic> json) => [
  _BaselinePrivacyItem('fav_video', json['fav_video'], false, 1),
  _BaselinePrivacyItem('bangumi', json['bangumi'], false, 1),
  _BaselinePrivacyItem('comic', json['comic'], false, 1),
  _BaselinePrivacyItem('coins_video', json['coins_video'], false, 1),
  _BaselinePrivacyItem('likes_video', json['likes_video'], false, 1),
  _BaselinePrivacyItem('played_game', json['played_game'], false, 1),
  _BaselinePrivacyItem('dress_up', json['dress_up'], false, 1),
  _BaselinePrivacyItem('disable_following', json['disable_following'], true, 1),
  _BaselinePrivacyItem('disable_show_fans', json['disable_show_fans'], true, 1),
  _BaselinePrivacyItem('close_space_medal', json['close_space_medal'], true, 2),
  _BaselinePrivacyItem('only_show_wearing', json['only_show_wearing'], true, 2),
  _BaselinePrivacyItem(
    'disable_show_school',
    json['disable_show_school'],
    true,
    2,
  ),
  _BaselinePrivacyItem('live_playback', json['live_playback'], false, 3),
  _BaselinePrivacyItem('charge_video', json['charge_video'], false, 3),
  _BaselinePrivacyItem('lesson_video', json['lesson_video'], false, 3),
];

/// 已删除的 `lib/models_new/video/video_tag/data.dart` 的逐行转写。
class _BaselineVideoTag {
  int? tagId;
  String? tagName;
  String? tagType;
  String? musicId;

  _BaselineVideoTag.fromJson(Map<String, dynamic> json)
    : tagId = json['tag_id'],
      tagName = json['tag_name'],
      tagType = json['tag_type'],
      musicId = json['music_id'];
}
