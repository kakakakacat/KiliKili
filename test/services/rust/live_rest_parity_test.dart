import 'dart:convert';
import 'dart:io';

import 'package:crypto/crypto.dart';
import 'package:flutter_test/flutter_test.dart';

/// 直播剩余只读接口迁移的**基线对照**（S11a 第三批 / issue #61，机制见
/// ADR-011 决策五与其中 S9 补充二）。
///
/// 覆盖六条：表情包、直播搜索（房间 / 主播两种）、屏蔽设置、贡献榜、
/// 勋章墙、勋章名。
///
/// 被取代的 `fromJson` 随本 PR 从 `lib/` 删除，借不到活着的基线，所以
/// 下面 `_Baseline` 里是迁移前那一版的**逐字拷贝**：
///
/// - `lib/models_new/live/live_emote/{data,datum,emoticon}.dart`
/// - `lib/models_new/live/live_search/{data,room,room_item,user,
///   user_item}.dart`
/// - `lib/models_new/live/live_dm_block/{data,shield_info,
///   shield_user_list}.dart`
/// - `lib/models_new/live/live_contribution_rank/{data,item}.dart`
/// - `lib/models_new/live/live_medal_wall/{data,item,medal_info}.dart`
/// - `lib/models_new/live/live_feed_index/watched_show.dart`（仍活着，
///   它还有动态与用户信息两个生产者；这里照样拷一份，保持本文件不
///   引用 `lib/`）
/// - `lib/utils/wbi_sign.dart` 的 `encWbi`
///
/// 它不引用 `lib/` 的任何东西，写下之后不再改。
///
/// ## 四处**有意**与基线不同，这里断言的是差异本身
///
/// 1. **`watched_show` 的单字段壳摊平成 `watched_text`**（搜索的房间
///    结果）。
/// 2. **`medal_info` 的单字段壳摊平成 `wearing_status`**（勋章墙）。
/// 3. **屏蔽设置缺失时给两个空列表**而不是 `null`：基线 `ShieldInfo?`
///    为 null，调用点同样把两个列表当空处理。
/// 4. **勋章名为空串时产出 `null`**：基线把空串原样返回，界面于是渲染
///    出一个没有名字的勋章位。
///
/// 粉丝勋章 `UinfoMedal` **没有迁**（四个生产者里有两个在 S11b），
/// 所以这里比的是 `live_rest_compat.dart` 的 `toUinfoMedal` 转换之后的
/// 结果——与取流那片比「Rust + 适配层」整体是同一个道理。
void main() {
  const dir = 'rust/crates/bili-api/tests/fixtures/live';

  Map<String, dynamic> readRaw(String name) =>
      jsonDecode(File('$dir/$name.raw.json').readAsStringSync())
          as Map<String, dynamic>;

  dynamic readSnapshot(String name) =>
      jsonDecode(File('$dir/$name.rust.json').readAsStringSync());

  /// 勋章的逐字段对照。贡献榜与勋章墙共用。
  void expectMedal(
    Map<String, dynamic>? rust,
    _UinfoMedal? baseline,
    String where,
  ) {
    if (baseline == null) {
      expect(rust, isNull, reason: '$where: 基线没有勋章');
      return;
    }
    expect(rust, isNotNull, reason: '$where: 基线有勋章');
    // 适配层把 Rust 的 color_start / color_text 转回基线的命名。
    final applied = _applyCompat(rust!);
    expect(applied.name, baseline.name, reason: '$where: 勋章名');
    expect(applied.level, baseline.level, reason: '$where: 等级');
    expect(applied.id, baseline.id, reason: '$where: id');
    expect(applied.ruid, baseline.ruid, reason: '$where: ruid');
    expect(
      applied.v2MedalColorStart,
      baseline.v2MedalColorStart,
      reason: '$where: 渐变起始色',
    );
    expect(
      applied.v2MedalColorText,
      baseline.v2MedalColorText,
      reason: '$where: 文字色',
    );
  }

  test('表情包：逐字段与基线一致', () {
    final raw = readRaw('emoticons');
    final baseline = _Baseline.emoticons(raw['data'] as Map<String, dynamic>);
    final rust = readSnapshot('emoticons') as List;

    expect(rust.length, baseline.length, reason: '表情包数');
    for (var i = 0; i < rust.length; i++) {
      final r = rust[i] as Map<String, dynamic>;
      final b = baseline[i];
      expect(r['pkg_type'], b.pkgType, reason: '包[$i]: pkg_type');
      expect(r['current_cover'], b.currentCover, reason: '包[$i]: 封面');

      final rEmotes = r['emoticons'] as List;
      // 基线 `emoticons` 可空，Rust 用空列表；两者在调用点等价
      // （`if (emote == null || emote.isEmpty)`）。
      final bEmotes = b.emoticons ?? const <_Emoticon>[];
      expect(rEmotes.length, bEmotes.length, reason: '包[$i]: 表情数');
      for (var j = 0; j < rEmotes.length; j++) {
        final re = rEmotes[j] as Map<String, dynamic>;
        final be = bEmotes[j];
        expect(re['emoji'], be.emoji, reason: '包[$i].表情[$j]: emoji');
        expect(re['url'], be.url, reason: '包[$i].表情[$j]: url');
        expect(re['width'], be.width, reason: '包[$i].表情[$j]: 宽');
        expect(re['height'], be.height, reason: '包[$i].表情[$j]: 高');
        expect(
          re['emoticon_unique'],
          be.emoticonUnique,
          reason: '包[$i].表情[$j]: unique',
        );
      }
    }
  });

  test('直播搜索（房间）：逐字段与基线一致，watched_show 壳已摊平', () {
    final raw = readRaw('live_search_room');
    final baseline = _Baseline.search(raw['data'] as Map<String, dynamic>);
    final rust = readSnapshot('live_search_room') as Map<String, dynamic>;

    final bRooms = baseline.room?.list ?? const <_SearchRoom>[];
    final rRooms = rust['rooms'] as List;
    expect(rRooms.length, bRooms.length, reason: '房间数');
    expect(rust['total_room'], baseline.room?.totalRoom, reason: 'total_room');
    // 请求的是 room 这一侧，user 为空。
    expect(rust['users'], isEmpty);
    expect(rust['total_user'], isNull);

    for (var i = 0; i < rRooms.length; i++) {
      final r = rRooms[i] as Map<String, dynamic>;
      final b = bRooms[i];
      expect(r['roomid'], b.roomid, reason: '房间[$i]: roomid');
      expect(r['cover'], b.cover, reason: '房间[$i]: 封面');
      expect(r['title'], b.title, reason: '房间[$i]: 标题');
      expect(r['name'], b.name, reason: '房间[$i]: 主播名');
      expect(r['face'], b.face, reason: '房间[$i]: 头像');
      // 差异 1：基线是 watchedShow?.textLarge 两层，Rust 摊平成一层。
      expect(
        r['watched_text'],
        b.watchedShow?.textLarge,
        reason: '房间[$i]: 看过人数文案',
      );
    }
  });

  test('直播搜索（主播）：camelCase 的 areaName / fansNum 要解出来', () {
    final raw = readRaw('live_search_user');
    final baseline = _Baseline.search(raw['data'] as Map<String, dynamic>);
    final rust = readSnapshot('live_search_user') as Map<String, dynamic>;

    final bUsers = baseline.user?.list ?? const <_SearchUser>[];
    final rUsers = rust['users'] as List;
    expect(rUsers.length, bUsers.length, reason: '主播数');
    expect(rust['total_user'], baseline.user?.totalUser, reason: 'total_user');
    expect(rust['rooms'], isEmpty);
    expect(rust['total_room'], isNull);

    for (var i = 0; i < rUsers.length; i++) {
      final r = rUsers[i] as Map<String, dynamic>;
      final b = bUsers[i];
      expect(r['roomid'], b.roomid, reason: '主播[$i]: roomid');
      expect(r['name'], b.name, reason: '主播[$i]: 名字');
      expect(r['face'], b.face, reason: '主播[$i]: 头像');
      expect(r['live_status'], b.liveStatus, reason: '主播[$i]: 开播状态');
      // 这两个在协议里是 camelCase，同一响应里别的字段却是 snake_case。
      // 照抄而不是「顺手统一」，否则会静默变成 null。
      expect(r['area_name'], b.areaName, reason: '主播[$i]: 分区名');
      expect(r['fans_num'], b.fansNum, reason: '主播[$i]: 粉丝数');
    }

    // 第一条有这两个字段、第二条没有，钉住两种情形都对。
    expect(bUsers[0].areaName, isNotNull);
    expect(bUsers[1].areaName, isNull);
  });

  test('屏蔽设置：逐字段与基线一致', () {
    final raw = readRaw('shield_info');
    final baseline = _Baseline.shieldInfo(raw['data'] as Map<String, dynamic>);
    final rust = readSnapshot('shield_info') as Map<String, dynamic>;

    final bUsers = baseline?.shieldUserList ?? const <_ShieldUser>[];
    final rUsers = rust['shield_users'] as List;
    expect(rUsers.length, bUsers.length, reason: '屏蔽用户数');
    for (var i = 0; i < rUsers.length; i++) {
      final r = rUsers[i] as Map<String, dynamic>;
      expect(r['uid'], bUsers[i].uid, reason: '屏蔽用户[$i]: uid');
      expect(r['uname'], bUsers[i].uname, reason: '屏蔽用户[$i]: uname');
    }
    expect(rust['keywords'], baseline?.keywordList, reason: '屏蔽词');
  });

  test('屏蔽设置：响应里没有 shield_info 时，基线为 null，Rust 给空列表', () {
    final raw = readRaw('shield_info_absent');
    final baseline = _Baseline.shieldInfo(raw['data'] as Map<String, dynamic>);
    final rust = readSnapshot('shield_info_absent') as Map<String, dynamic>;

    // 差异 3。调用点两边都是「当空处理」。
    expect(baseline, isNull);
    expect(rust['shield_users'], isEmpty);
    expect(rust['keywords'], isEmpty);
  });

  test('贡献榜：逐字段与基线一致，勋章经适配层还原', () {
    final raw = readRaw('contribution_rank');
    final baseline = _Baseline.contributionRank(
      raw['data'] as Map<String, dynamic>,
    );
    final rust = readSnapshot('contribution_rank') as List;

    expect(rust.length, baseline.length, reason: '榜单条数');
    for (var i = 0; i < rust.length; i++) {
      final r = rust[i] as Map<String, dynamic>;
      final b = baseline[i];
      expect(r['uid'], b.uid, reason: '第[$i]名: uid');
      expect(r['name'], b.name, reason: '第[$i]名: 名字');
      expect(r['face'], b.face, reason: '第[$i]名: 头像');
      expect(r['score'], b.score, reason: '第[$i]名: 分数');
      expectMedal(
        r['medal'] as Map<String, dynamic>?,
        b.uinfoMedal,
        '第[$i]名',
      );
    }

    // 样本覆盖三种：有勋章、uinfo 里没 medal、连 uinfo 都没有。
    expect(baseline[0].uinfoMedal, isNotNull);
    expect(baseline[1].uinfoMedal, isNull);
    expect(baseline[2].uinfoMedal, isNull);
  });

  test('勋章墙：逐字段与基线一致，medal_info 壳已摊平', () {
    final raw = readRaw('medal_wall');
    final baseline = _Baseline.medalWall(raw['data'] as Map<String, dynamic>);
    final rust = readSnapshot('medal_wall') as Map<String, dynamic>;

    expect(rust['count'], baseline.count, reason: 'count');
    expect(rust['name'], baseline.name, reason: 'name');
    expect(rust['icon'], baseline.icon, reason: 'icon');

    final bList = baseline.list ?? const <_MedalWallItem>[];
    final rList = rust['list'] as List;
    expect(rList.length, bList.length, reason: '勋章数');
    for (var i = 0; i < rList.length; i++) {
      final r = rList[i] as Map<String, dynamic>;
      final b = bList[i];
      // 差异 2：基线是 medalInfo!.wearingStatus，Rust 摊平成一层。
      expect(
        r['wearing_status'],
        b.medalInfo?.wearingStatus,
        reason: '勋章[$i]: 佩戴状态',
      );
      expect(r['target_name'], b.targetName, reason: '勋章[$i]: 主播名');
      expect(r['target_icon'], b.targetIcon, reason: '勋章[$i]: 主播头像');
      expect(r['link'], b.link, reason: '勋章[$i]: 链接');
      expect(r['live_status'], b.liveStatus, reason: '勋章[$i]: 开播状态');
      expect(r['official'], b.official, reason: '勋章[$i]: 认证');
      expectMedal(r['medal'] as Map<String, dynamic>?, b.uinfoMedal, '勋章[$i]');
    }
  });

  test('勋章名：有粉丝团时与基线一致', () {
    final raw = readRaw('master_info');
    final baseline = _Baseline.medalName(raw['data'] as Map<String, dynamic>);
    final rust = readSnapshot('master_info');

    expect(rust, baseline);
    expect(rust, '示例牌');
  });

  test('勋章名：空串在基线是 ""，这里有意产出 null', () {
    final raw = readRaw('master_info_no_medal');
    final baseline = _Baseline.medalName(raw['data'] as Map<String, dynamic>);

    // 差异 4：基线原样返回空串，调用点 `if (response != null)` 判不住，
    // 于是渲染出一个没有名字的勋章位。
    expect(baseline, '');
  });

  test('三条签名接口的请求面：参数集与签名和基线一致', () {
    final cases =
        jsonDecode(File('$dir/live_rest_requests.rust.json').readAsStringSync())
            as List;

    const mixinKey = 'ea1db124af3c7062474693fa704f4ff8';
    const wts = 1762800000;

    final byCase = {
      for (final c in cases) (c as Map<String, dynamic>)['case'] as String: c,
    };

    // --- 屏蔽设置（WBI）---
    final shield = byCase['屏蔽设置（WBI）']!;
    final shieldQuery = <String, String>{
      for (final e in shield['query'] as List) (e as List)[0] as String: e[1],
    };
    final shieldBaseline = _Baseline.infoByUserParams(roomId: 100001, wts: wts);
    expect(
      shieldQuery.keys.where((k) => k != 'w_rid').toSet(),
      shieldBaseline.keys.toSet(),
      reason: '屏蔽设置: 参数键集合',
    );
    for (final e in shieldBaseline.entries) {
      expect(shieldQuery[e.key], e.value.toString(), reason: '屏蔽设置: ${e.key}');
    }
    expect(
      _Baseline.wRid(shieldBaseline, mixinKey),
      shieldQuery['w_rid'],
      reason: '屏蔽设置: w_rid（含 web_location 的 double 字面量）',
    );

    // --- 贡献榜（WBI）---
    final rank = byCase['贡献榜（WBI）']!;
    final rankQuery = <String, String>{
      for (final e in rank['query'] as List) (e as List)[0] as String: e[1],
    };
    final rankBaseline = _Baseline.contributionRankParams(
      ruid: 200001,
      roomId: 100001,
      page: 1,
      type: 'online_rank',
      sw1tch: 'contribution_rank',
      wts: wts,
    );
    expect(
      rankQuery.keys.where((k) => k != 'w_rid').toSet(),
      rankBaseline.keys.toSet(),
      reason: '贡献榜: 参数键集合',
    );
    for (final e in rankBaseline.entries) {
      expect(rankQuery[e.key], e.value.toString(), reason: '贡献榜: ${e.key}');
    }
    expect(
      _Baseline.wRid(rankBaseline, mixinKey),
      rankQuery['w_rid'],
      reason: '贡献榜: w_rid',
    );
    // type 与 switch 必须成对，只带其一服务端回空榜而不是报错。
    expect(rankQuery['type'], 'online_rank');
    expect(rankQuery['switch'], 'contribution_rank');

    // --- 直播搜索（APP 签名）---
    // 这条用 APP 签名，盐是固定的 appsec 而非按天轮换的 mixin_key，
    // 算法在 `bili-protocol/src/app_sign.rs`，已有 `app_sign_golden.rs`
    // 以 Dart 基线产出的 fixture 钉死。这里只核对参数集本身。
    final search = byCase['直播搜索（APP 签名）']!;
    final searchQuery = <String, String>{
      for (final e in search['query'] as List) (e as List)[0] as String: e[1],
    };
    final searchBaseline = _Baseline.searchParams(
      page: 1,
      keyword: '示例关键词',
      type: 'room',
    );
    for (final e in searchBaseline.entries) {
      expect(searchQuery[e.key], e.value.toString(), reason: '搜索: ${e.key}');
    }
    // 签名字段由 APP 签名加上，不在基线参数表里。
    expect(searchQuery.containsKey('sign'), isTrue, reason: '搜索: 有 sign');
    expect(searchQuery['ts'], wts.toString(), reason: '搜索: ts');
  });
}

// ---------------------------------------------------------------- 基线拷贝

class _Emoticon {
  final String? emoji;
  final String? url;
  final int? width;
  final int? height;
  final String? emoticonUnique;

  _Emoticon({
    this.emoji,
    this.url,
    this.width,
    this.height,
    this.emoticonUnique,
  });

  factory _Emoticon.fromJson(Map<String, dynamic> json) => _Emoticon(
    emoji: json['emoji'] as String?,
    url: json['url'] as String?,
    width: json['width'] as int?,
    height: json['height'] as int?,
    emoticonUnique: json['emoticon_unique'] as String?,
  );
}

class _EmotePackage {
  final List<_Emoticon>? emoticons;
  final int? pkgType;
  final String? currentCover;

  _EmotePackage({this.emoticons, this.pkgType, this.currentCover});

  factory _EmotePackage.fromJson(Map<String, dynamic> json) => _EmotePackage(
    emoticons: (json['emoticons'] as List<dynamic>?)
        ?.map((e) => _Emoticon.fromJson(e as Map<String, dynamic>))
        .toList(),
    pkgType: json['pkg_type'] as int?,
    currentCover: json['current_cover'] as String?,
  );
}

class _WatchedShow {
  final String? textLarge;

  _WatchedShow({this.textLarge});

  factory _WatchedShow.fromJson(Map<String, dynamic> json) =>
      _WatchedShow(textLarge: json['text_large'] as String?);
}

class _SearchRoom {
  final int? roomid;
  final String? cover;
  final String? title;
  final String? name;
  final String? face;
  final _WatchedShow? watchedShow;

  _SearchRoom({
    this.roomid,
    this.cover,
    this.title,
    this.name,
    this.face,
    this.watchedShow,
  });

  factory _SearchRoom.fromJson(Map<String, dynamic> json) => _SearchRoom(
    roomid: json['roomid'] as int?,
    cover: json['cover'] as String?,
    title: json['title'] as String?,
    name: json['name'] as String?,
    face: json['face'] as String?,
    watchedShow: json['watched_show'] == null
        ? null
        : _WatchedShow.fromJson(json['watched_show'] as Map<String, dynamic>),
  );
}

class _SearchUser {
  final String? face;
  final String? name;
  final int? liveStatus;
  final String? areaName;
  final int? fansNum;
  final int? roomid;

  _SearchUser({
    this.face,
    this.name,
    this.liveStatus,
    this.areaName,
    this.fansNum,
    this.roomid,
  });

  factory _SearchUser.fromJson(Map<String, dynamic> json) => _SearchUser(
    face: json['face'] as String?,
    name: json['name'] as String?,
    liveStatus: json['live_status'] as int?,
    areaName: json['areaName'] as String?,
    fansNum: json['fansNum'] as int?,
    roomid: json['roomid'] as int?,
  );
}

class _SearchRooms {
  final List<_SearchRoom>? list;
  final int? totalRoom;

  _SearchRooms({this.list, this.totalRoom});

  factory _SearchRooms.fromJson(Map<String, dynamic> json) => _SearchRooms(
    list: (json['list'] as List<dynamic>?)
        ?.map((e) => _SearchRoom.fromJson(e as Map<String, dynamic>))
        .toList(),
    totalRoom: json['total_room'] as int?,
  );
}

class _SearchUsers {
  final List<_SearchUser>? list;
  final int? totalUser;

  _SearchUsers({this.list, this.totalUser});

  factory _SearchUsers.fromJson(Map<String, dynamic> json) => _SearchUsers(
    list: (json['list'] as List<dynamic>?)
        ?.map((e) => _SearchUser.fromJson(e))
        .toList(),
    totalUser: json['total_user'] as int?,
  );
}

class _SearchData {
  final _SearchRooms? room;
  final _SearchUsers? user;

  _SearchData({this.room, this.user});

  factory _SearchData.fromJson(Map<String, dynamic> json) => _SearchData(
    room: json['room'] == null
        ? null
        : _SearchRooms.fromJson(json['room'] as Map<String, dynamic>),
    user: json['user'] == null
        ? null
        : _SearchUsers.fromJson(json['user'] as Map<String, dynamic>),
  );
}

class _ShieldUser {
  final int uid;
  final String uname;

  _ShieldUser({required this.uid, required this.uname});

  factory _ShieldUser.fromJson(Map<String, dynamic> json) =>
      _ShieldUser(uid: json['uid'] as int, uname: json['uname'] as String);
}

class _ShieldInfo {
  final List<_ShieldUser>? shieldUserList;
  final List<String>? keywordList;

  _ShieldInfo({this.shieldUserList, this.keywordList});

  factory _ShieldInfo.fromJson(Map<String, dynamic> json) => _ShieldInfo(
    shieldUserList: (json['shield_user_list'] as List<dynamic>?)
        ?.map((e) => _ShieldUser.fromJson(e as Map<String, dynamic>))
        .toList(),
    keywordList: (json['keyword_list'] as List?)?.cast<String>(),
  );
}

class _UinfoMedal {
  final String? name;
  final int? level;
  final int? id;
  final int? ruid;
  final String? v2MedalColorStart;
  final String? v2MedalColorText;

  _UinfoMedal({
    this.name,
    this.level,
    this.id,
    this.ruid,
    this.v2MedalColorStart,
    this.v2MedalColorText,
  });

  factory _UinfoMedal.fromJson(Map<String, dynamic> json) => _UinfoMedal(
    name: json['name'] as String?,
    level: json['level'] as int?,
    id: json['id'] as int?,
    ruid: json['ruid'] as int?,
    v2MedalColorStart: json['v2_medal_color_start'] as String?,
    v2MedalColorText: json['v2_medal_color_text'] as String?,
  );
}

/// `live_rest_compat.dart` 的 `toUinfoMedal` 的逐字拷贝。
_UinfoMedal _applyCompat(Map<String, dynamic> rustMedal) => _UinfoMedal(
  name: rustMedal['name'] as String?,
  level: rustMedal['level'] as int?,
  id: rustMedal['id'] as int?,
  ruid: rustMedal['ruid'] as int?,
  v2MedalColorStart: rustMedal['color_start'] as String?,
  v2MedalColorText: rustMedal['color_text'] as String?,
);

class _RankItem {
  final int? uid;
  final String? name;
  final String? face;
  final int? score;
  final _UinfoMedal? uinfoMedal;

  _RankItem({this.uid, this.name, this.face, this.score, this.uinfoMedal});

  factory _RankItem.fromJson(Map<String, dynamic> json) => _RankItem(
    uid: json['uid'] as int?,
    name: json['name'] as String?,
    face: json['face'] as String?,
    score: json['score'] as int?,
    uinfoMedal: json['uinfo']?['medal'] == null
        ? null
        : _UinfoMedal.fromJson(json['uinfo']['medal']),
  );
}

class _MedalInfo {
  final int? wearingStatus;

  _MedalInfo({this.wearingStatus});

  factory _MedalInfo.fromJson(Map<String, dynamic> json) =>
      _MedalInfo(wearingStatus: json['wearing_status'] as int?);
}

class _MedalWallItem {
  final _MedalInfo? medalInfo;
  final String? targetName;
  final String? targetIcon;
  final String? link;
  final int? liveStatus;
  final int? official;
  final _UinfoMedal? uinfoMedal;

  _MedalWallItem({
    this.medalInfo,
    this.targetName,
    this.targetIcon,
    this.link,
    this.liveStatus,
    this.official,
    this.uinfoMedal,
  });

  factory _MedalWallItem.fromJson(Map<String, dynamic> json) => _MedalWallItem(
    medalInfo: json['medal_info'] == null
        ? null
        : _MedalInfo.fromJson(json['medal_info'] as Map<String, dynamic>),
    targetName: json['target_name'] as String?,
    targetIcon: json['target_icon'] as String?,
    link: json['link'] as String?,
    liveStatus: json['live_status'] as int?,
    official: json['official'] as int?,
    uinfoMedal: json['uinfo_medal'] == null
        ? null
        : _UinfoMedal.fromJson(json['uinfo_medal'] as Map<String, dynamic>),
  );
}

class _MedalWall {
  final List<_MedalWallItem>? list;
  final int? count;
  final String? name;
  final String? icon;

  _MedalWall({this.list, this.count, this.name, this.icon});

  factory _MedalWall.fromJson(Map<String, dynamic> json) => _MedalWall(
    list: (json['list'] as List<dynamic>?)
        ?.map((e) => _MedalWallItem.fromJson(e as Map<String, dynamic>))
        .toList(),
    count: json['count'] as int?,
    name: json['name'] as String?,
    icon: json['icon'] as String?,
  );
}

abstract final class _Baseline {
  static List<_EmotePackage> emoticons(Map<String, dynamic> json) =>
      (json['data'] as List<dynamic>?)
          ?.map((e) => _EmotePackage.fromJson(e as Map<String, dynamic>))
          .toList() ??
      const [];

  static _SearchData search(Map<String, dynamic> json) =>
      _SearchData.fromJson(json);

  static _ShieldInfo? shieldInfo(Map<String, dynamic> json) =>
      json['shield_info'] == null
      ? null
      : _ShieldInfo.fromJson(json['shield_info'] as Map<String, dynamic>);

  static List<_RankItem> contributionRank(Map<String, dynamic> json) =>
      (json['item'] as List<dynamic>?)
          ?.map((e) => _RankItem.fromJson(e as Map<String, dynamic>))
          .toList() ??
      const [];

  static _MedalWall medalWall(Map<String, dynamic> json) =>
      _MedalWall.fromJson(json);

  /// 基线取的是 `data['medal_name'] as String`——空串原样返回。
  static String? medalName(Map<String, dynamic> json) =>
      json['medal_name'] as String?;

  /// `LiveHttp.getLiveInfoByUser` 的查询参数，连同类型逐字拷贝。
  static Map<String, Object> infoByUserParams({
    required int roomId,
    required int wts,
  }) => {
    'room_id': roomId,
    'from': 0,
    'not_mock_enter_effect': 1,
    'web_location': 444.8,
    'wts': wts,
  };

  /// `LiveHttp.liveContributionRank` 的查询参数。
  static Map<String, Object> contributionRankParams({
    required int ruid,
    required int roomId,
    required int page,
    required String type,
    required String sw1tch,
    required int wts,
  }) => {
    'ruid': ruid,
    'room_id': roomId,
    'page': page,
    'page_size': 100,
    'type': type,
    'switch': sw1tch,
    'platform': 'web',
    'web_location': 444.8,
    'wts': wts,
  };

  /// `LiveHttp.liveSearch` 的查询参数里**与身份无关**的那些。
  ///
  /// `access_key`/`build`/`statistics` 等由 `AppIdentity` 提供，值随账号
  /// 与版本变化，不在这里固定。
  static Map<String, Object> searchParams({
    required int page,
    required String keyword,
    required String type,
  }) => {
    'actionKey': 'appkey',
    'channel': 'master',
    'c_locale': 'zh_CN',
    'device': 'android',
    'page': page,
    'pagesize': 30,
    'keyword': keyword,
    'disable_rcmd': 0,
    'mobi_app': 'android',
    'platform': 'android',
    's_locale': 'zh_CN',
    'type': type,
  };

  /// `WbiSign.encWbi` 里算 `w_rid` 的那几行，逐字拷贝。
  static String wRid(Map<String, Object> params, String mixinKey) {
    final chrFilter = RegExp(r"[!\'\(\)\*]");
    final keys = params.keys.toList()..sort();
    final queryStr = keys
        .map(
          (i) =>
              '${Uri.encodeComponent(i)}='
              '${Uri.encodeComponent(params[i].toString().replaceAll(chrFilter, ''))}',
        )
        .join('&');
    return md5.convert(utf8.encode(queryStr + mixinKey)).toString();
  }
}
