import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

/// S12（issue #63）的一次性基线对照，机制见 ADR-011 决策五。
///
/// `MsgReplyData`、`MsgAtData`、`MsgLikeData`、`MsgLikeDetailData`、
/// `MsgSysItem`、`SingleUnreadData`、`MsgFeedUnreadData`、`ImUserInfosData`、
/// `SessionSsData`、`UidSetting` 都已随本次迁移删除，没有「还活着的基线
/// 模型」可借，于是本文件自带 oracle——下面的 `_Baseline*` 是迁移前那批
/// `fromJson` 的**逐行转写**，取自合并前的 `main`（`ceabd1672`）：
///
/// - `lib/models_new/msg/msg_reply/{data,item,content,cursor,user}.dart`
/// - `lib/models_new/msg/msg_at/{data,item,content,cursor,user}.dart`
/// - `lib/models_new/msg/msg_like/{data,latest,total,item,content,cursor,user}.dart`
/// - `lib/models_new/msg/msg_like_detail/{data,page,card,item,user}.dart`
/// - `lib/models_new/msg/msg_sys/data.dart`
/// - `lib/models_new/msg/{im_user_infos/datum,session_ss/data,msg_dnd/uid_setting}.dart`
/// - `lib/models_new/{single_unread,msgfeed_unread}/data.dart`
///
/// 转写时不许顺手修正任何怪异之处：它要证明的正是 Rust 与迁移前行为一致。
///
/// **本文件刻意不 import `lib/`**，这样它在没打 Flutter SDK 补丁的机器上
/// 也跑得起来（AGENTS.md §3）。
///
/// ## 两处刻意的不一致，在断言里显式点名
///
/// 1. **`ImUserInfosData` 摊平了三棵子树。** 基线复用
///    `model_avatar.dart` 的 `Vip`/`Pendant`/`BaseOfficialVerify`，那几个
///    类型还有别的切片在用，搬过来就得和 S6/S7 抢所有权。迁移后只取这张
///    卡片实际显示的四个值（`vipStatus`/`vipType`/`pendantImage`/
///    `officialType`），并把**空串归一成 null**——基线是每个调用点各写
///    一遍 `?.isNotEmpty == true`。
/// 2. **未读数多了一个 `badge`。** 那是红点的求和规则，基线写在
///    `lib/pages/main/controller.dart` 里，按 AGENTS.md §2 下沉到 Rust。
///    基线没有对应字段，所以这里对着基线的那段加法单独断言。
///
/// fixture 全部是手工构造的，原因写在
/// `rust/crates/bili-api/tests/msg_golden.rs` 的文件头：这一片没有一条
/// 匿名可取的接口。它们证明的是映射一致，不能证明我们对协议的理解正确
/// ——那要等 Phase 2 的真机验证。
void main() {
  const dir = 'rust/crates/bili-api/tests/fixtures/msg';

  Map<String, dynamic> read(String name) =>
      jsonDecode(File('$dir/$name').readAsStringSync()) as Map<String, dynamic>;

  List<dynamic> readList(String name) =>
      jsonDecode(File('$dir/$name').readAsStringSync()) as List<dynamic>;

  void expectUser(Map<String, dynamic>? snapshot, _BaselineUser? b) {
    if (b == null) {
      expect(snapshot, isNull);
      return;
    }
    expect(snapshot, isNotNull);
    expect(snapshot!['mid'], b.mid);
    expect(snapshot['nickname'], b.nickname);
    expect(snapshot['avatar'], b.avatar);
  }

  void expectCursor(Map<String, dynamic>? snapshot, _BaselineCursor? b) {
    if (b == null) {
      expect(snapshot, isNull);
      return;
    }
    expect(snapshot, isNotNull);
    expect(snapshot!['is_end'], b.isEnd);
    expect(snapshot['id'], b.id);
    expect(snapshot['time'], b.time);
  }

  test('回复我的：Rust 快照与基线逐字段一致', () {
    final data = read('reply_feed.raw.json')['data'] as Map<String, dynamic>;
    final snapshot = read('reply_feed.rust.json');
    final b = _BaselineReplyData.fromJson(data);

    expectCursor(snapshot['cursor'] as Map<String, dynamic>?, b.cursor);
    expect(snapshot['last_view_at'], b.lastViewAt);

    final items = snapshot['items'] as List;
    expect(items.length, b.items!.length);
    for (var i = 0; i < items.length; i++) {
      final s = items[i] as Map<String, dynamic>;
      final e = b.items![i];
      expect(s['id'], e.id);
      expect(s['counts'], e.counts);
      expect(s['is_multi'], e.isMulti);
      expect(s['reply_time'], e.replyTime);
      expectUser(s['user'] as Map<String, dynamic>?, e.user);

      final content = s['item'] as Map<String, dynamic>?;
      expect(content != null, e.item != null);
      if (content != null) {
        expect(content['subject_id'], e.item!.subjectId);
        expect(content['business_id'], e.item!.businessId);
        expect(content['business'], e.item!.business);
        expect(content['native_uri'], e.item!.nativeUri);
        expect(content['root_reply_content'], e.item!.rootReplyContent);
        expect(content['source_content'], e.item!.sourceContent);
        expect(content['target_reply_content'], e.item!.targetReplyContent);
      }
    }
  });

  test('@我的：Rust 快照与基线逐字段一致', () {
    final data = read('at_feed.raw.json')['data'] as Map<String, dynamic>;
    final snapshot = read('at_feed.rust.json');
    final b = _BaselineAtData.fromJson(data);

    expectCursor(snapshot['cursor'] as Map<String, dynamic>?, b.cursor);

    final items = snapshot['items'] as List;
    expect(items.length, b.items!.length);
    for (var i = 0; i < items.length; i++) {
      final s = items[i] as Map<String, dynamic>;
      final e = b.items![i];
      expect(s['id'], e.id);
      expect(s['at_time'], e.atTime);
      expectUser(s['user'] as Map<String, dynamic>?, e.user);

      final content = s['item'] as Map<String, dynamic>?;
      expect(content != null, e.item != null);
      if (content != null) {
        expect(content['business'], e.item!.business);
        expect(content['image'], e.item!.image);
        expect(content['source_content'], e.item!.sourceContent);
        expect(content['native_uri'], e.item!.nativeUri);
      }
    }
  });

  test('收到的赞：两段结构与基线逐字段一致', () {
    final data = read('like_feed.raw.json')['data'] as Map<String, dynamic>;
    final snapshot = read('like_feed.rust.json');
    final b = _BaselineLikeData.fromJson(data);

    void expectLikeItems(List? items, List<_BaselineLikeItem>? expected) {
      expect(items?.length, expected?.length);
      if (items == null || expected == null) return;
      for (var i = 0; i < items.length; i++) {
        final s = items[i] as Map<String, dynamic>;
        final e = expected[i];
        expect(s['id'], e.id);
        expect(s['counts'], e.counts, reason: 'counts 是总人数，可大于 users 长度');
        expect(s['like_time'], e.likeTime);
        expect(s['notice_state'], e.noticeState);

        final users = s['users'] as List;
        expect(users.length, e.users!.length);
        for (var j = 0; j < users.length; j++) {
          expectUser(users[j] as Map<String, dynamic>, e.users![j]);
        }

        final content = s['item'] as Map<String, dynamic>?;
        expect(content != null, e.item != null);
        if (content != null) {
          expect(content['business'], e.item!.business);
          expect(content['title'], e.item!.title);
          expect(content['image'], e.item!.image);
          expect(content['native_uri'], e.item!.nativeUri);
        }
      }
    }

    final latest = snapshot['latest'] as Map<String, dynamic>?;
    expect(latest != null, b.latest != null);
    expect(latest!['last_view_at'], b.latest!.lastViewAt);
    expectLikeItems(latest['items'] as List?, b.latest!.items);

    final total = snapshot['total'] as Map<String, dynamic>?;
    expect(total != null, b.total != null);
    expectCursor(total!['cursor'] as Map<String, dynamic>?, b.total!.cursor);
    expectLikeItems(total['items'] as List?, b.total!.items);
  });

  test('赞的明细：Rust 快照与基线逐字段一致', () {
    final data = read('like_detail.raw.json')['data'] as Map<String, dynamic>;
    final snapshot = read('like_detail.rust.json');
    final b = _BaselineLikeDetailData.fromJson(data);

    expect((snapshot['page'] as Map<String, dynamic>)['is_end'], b.page!.isEnd);
    final card = snapshot['card'] as Map<String, dynamic>;
    expect(card['business'], b.card!.business);
    expect(card['title'], b.card!.title);

    final items = snapshot['items'] as List;
    expect(items.length, b.items!.length);
    for (var i = 0; i < items.length; i++) {
      final s = items[i] as Map<String, dynamic>;
      expect(s['like_time'], b.items![i].likeTime);
      expectUser(s['user'] as Map<String, dynamic>?, b.items![i].user);
    }
  });

  test('系统通知：content 的三种形态与基线一致', () {
    final data = read('sys_notify.raw.json')['data'] as List<dynamic>;
    final snapshot = readList('sys_notify.rust.json');
    final b = data
        .map((e) => _BaselineSysMsgItem.fromJson(e as Map<String, dynamic>))
        .toList();

    expect(snapshot.length, b.length);
    for (var i = 0; i < snapshot.length; i++) {
      final s = snapshot[i] as Map<String, dynamic>;
      expect(s['id'], b[i].id);
      expect(s['cursor'], b[i].cursor);
      expect(s['title'], b[i].title);
      expect(s['time_at'], b[i].timeAt);
      expect(
        s['content'],
        b[i].content,
        reason: '能解成 JSON 且有 web 时取 web，否则原样保留——基线把解析异常吞掉也是这个效果',
      );
    }
  });

  test('私信未读数：八个桶与基线一致，badge 对上基线的那段加法', () {
    final data = read('single_unread.raw.json')['data'] as Map<String, dynamic>;
    final snapshot = read('single_unread.rust.json');
    final b = _BaselineSingleUnread.fromJson(data);

    expect(snapshot['unfollow_unread'], b.unfollowUnread);
    expect(snapshot['follow_unread'], b.followUnread);
    expect(snapshot['unfollow_push_msg'], b.unfollowPushMsg);
    expect(snapshot['dustbin_push_msg'], b.dustbinPushMsg);
    expect(snapshot['dustbin_unread'], b.dustbinUnread);
    expect(snapshot['biz_msg_unfollow_unread'], b.bizMsgUnfollowUnread);
    expect(snapshot['biz_msg_follow_unread'], b.bizMsgFollowUnread);
    expect(snapshot['custom_unread'], b.customUnread);

    // 基线 `lib/pages/main/controller.dart` 的 `_msgUnread`：六项相加，
    // 两个 dustbin_* 不算。快照里的 `badge` 是 Rust 算的同一件事
    // （`SingleUnread::badge_count`），这一条就是那条规则在跨语言这一层
    // 唯一的守卫。
    final baselineBadge =
        b.followUnread +
        b.unfollowUnread +
        b.bizMsgFollowUnread +
        b.bizMsgUnfollowUnread +
        b.unfollowPushMsg +
        b.customUnread;
    expect(snapshot['badge'], baselineBadge);
  });

  test('消息中心未读数：四个 Tab 与基线一致', () {
    final data = read('feed_unread.raw.json')['data'] as Map<String, dynamic>;
    final snapshot = read('feed_unread.rust.json');
    final b = _BaselineFeedUnread.fromJson(data);

    expect(snapshot['at'], b.at);
    expect(snapshot['like'], b.like);
    expect(snapshot['reply'], b.reply);
    expect(snapshot['sys_msg'], b.sysMsg);

    // 快照的 badge 按四项全勾算。基线 `_msgFeedUnread` 在四项都勾、且
    // disableLikeMsg 为 false 时，走的正是这个加法。
    expect(snapshot['badge'], b.at + b.like + b.reply + b.sysMsg);
  });

  test('IM 用户资料：摊平的四个值与基线的三棵子树一致', () {
    final data = read('im_user_infos.raw.json')['data'] as List<dynamic>;
    final snapshot = readList('im_user_infos.rust.json');
    final b = data
        .map((e) => _BaselineImUserInfo.fromJson(e as Map<String, dynamic>))
        .toList();

    expect(snapshot.length, b.length);
    for (var i = 0; i < snapshot.length; i++) {
      final s = snapshot[i] as Map<String, dynamic>;
      expect(s['mid'], b[i].mid);
      expect(s['name'], b[i].name);
      expect(s['face'], b[i].face);
      expect(s['sign'], b[i].sign, reason: 'sign 的空串刻意不归一：它是正文不是开关');
      expect(s['vip_status'], b[i].vipStatus);
      expect(s['vip_type'], b[i].vipType);
      expect(
        s['official_type'],
        b[i].officialType,
        reason: '基线 BaseOfficialVerify 读的是 type，不是 role',
      );
      // 刻意的不一致：空串归一成 null，省得每个调用点各写一遍 isNotEmpty。
      expect(
        s['pendant_image'],
        b[i].pendantImage?.isNotEmpty == true ? b[i].pendantImage : null,
      );
    }
  });

  test('会话设置与免打扰：与基线逐字段一致', () {
    final ss = read('session_ss.raw.json')['data'] as Map<String, dynamic>;
    final ssSnapshot = read('session_ss.rust.json');
    final bss = _BaselineSessionSs.fromJson(ss);
    expect(ssSnapshot['follow_status'], bss.followStatus);
    expect(ssSnapshot['push_setting'], bss.pushSetting);
    expect(ssSnapshot['show_push_setting'], bss.showPushSetting);

    final dnd = read('msg_dnd.raw.json')['data'] as Map<String, dynamic>;
    final dndSnapshot = readList('msg_dnd.rust.json');
    final bdnd = (dnd['uid_settings'] as List)
        .map((e) => _BaselineUidSetting.fromJson(e as Map<String, dynamic>))
        .toList();
    expect(dndSnapshot.length, bdnd.length);
    for (var i = 0; i < dndSnapshot.length; i++) {
      expect((dndSnapshot[i] as Map<String, dynamic>)['setting'], bdnd[i].setting);
    }
  });
}

// ===================================================== 迁移前模型的逐行转写

class _BaselineCursor {
  _BaselineCursor({this.isEnd, this.id, this.time});

  final bool? isEnd;
  final int? id;
  final int? time;

  factory _BaselineCursor.fromJson(Map<String, dynamic> json) => _BaselineCursor(
    isEnd: json['is_end'] as bool?,
    id: json['id'] as int?,
    time: json['time'] as int?,
  );
}

class _BaselineUser {
  _BaselineUser({this.mid, this.nickname, this.avatar});

  final int? mid;
  final String? nickname;
  final String? avatar;

  factory _BaselineUser.fromJson(Map<String, dynamic> json) => _BaselineUser(
    mid: json['mid'] as int?,
    nickname: json['nickname'] as String?,
    avatar: json['avatar'] as String?,
  );
}

class _BaselineReplyContent {
  _BaselineReplyContent({
    this.subjectId,
    this.businessId,
    this.business,
    this.nativeUri,
    this.rootReplyContent,
    this.sourceContent,
    this.targetReplyContent,
  });

  final int? subjectId;
  final int? businessId;
  final String? business;
  final String? nativeUri;
  final String? rootReplyContent;
  final String? sourceContent;
  final String? targetReplyContent;

  factory _BaselineReplyContent.fromJson(Map<String, dynamic> json) {
    return _BaselineReplyContent(
      subjectId: json['subject_id'] as int?,
      businessId: json['business_id'] as int?,
      business: json['business'] as String?,
      nativeUri: json['native_uri'] as String?,
      rootReplyContent: json['root_reply_content'] as String?,
      sourceContent: json['source_content'] as String?,
      targetReplyContent: json['target_reply_content'] as String?,
    );
  }
}

class _BaselineReplyItem {
  _BaselineReplyItem({
    this.id,
    this.user,
    this.item,
    this.counts,
    this.isMulti,
    this.replyTime,
  });

  final int? id;
  final _BaselineUser? user;
  final _BaselineReplyContent? item;
  final int? counts;
  final int? isMulti;
  final int? replyTime;

  factory _BaselineReplyItem.fromJson(Map<String, dynamic> json) =>
      _BaselineReplyItem(
        id: json['id'] as int?,
        user: json['user'] == null
            ? null
            : _BaselineUser.fromJson(json['user'] as Map<String, dynamic>),
        item: json['item'] == null
            ? null
            : _BaselineReplyContent.fromJson(
                json['item'] as Map<String, dynamic>,
              ),
        counts: json['counts'] as int?,
        isMulti: json['is_multi'] as int?,
        replyTime: json['reply_time'] as int?,
      );
}

class _BaselineReplyData {
  _BaselineReplyData({this.cursor, this.items, this.lastViewAt});

  final _BaselineCursor? cursor;
  final List<_BaselineReplyItem>? items;
  final int? lastViewAt;

  factory _BaselineReplyData.fromJson(Map<String, dynamic> json) =>
      _BaselineReplyData(
        cursor: json['cursor'] == null
            ? null
            : _BaselineCursor.fromJson(json['cursor'] as Map<String, dynamic>),
        items: (json['items'] as List<dynamic>?)
            ?.map((e) => _BaselineReplyItem.fromJson(e as Map<String, dynamic>))
            .toList(),
        lastViewAt: json['last_view_at'] as int?,
      );
}

class _BaselineAtContent {
  _BaselineAtContent({
    this.business,
    this.image,
    this.sourceContent,
    this.nativeUri,
  });

  final String? business;
  final String? image;
  final String? sourceContent;
  final String? nativeUri;

  factory _BaselineAtContent.fromJson(Map<String, dynamic> json) =>
      _BaselineAtContent(
        business: json['business'] as String?,
        image: json['image'] as String?,
        sourceContent: json['source_content'] as String?,
        nativeUri: json['native_uri'] as String?,
      );
}

class _BaselineAtItem {
  _BaselineAtItem({this.id, this.user, this.item, this.atTime});

  final int? id;
  final _BaselineUser? user;
  final _BaselineAtContent? item;
  final int? atTime;

  factory _BaselineAtItem.fromJson(Map<String, dynamic> json) =>
      _BaselineAtItem(
        id: json['id'] as int?,
        user: json['user'] == null
            ? null
            : _BaselineUser.fromJson(json['user'] as Map<String, dynamic>),
        item: json['item'] == null
            ? null
            : _BaselineAtContent.fromJson(json['item'] as Map<String, dynamic>),
        atTime: json['at_time'] as int?,
      );
}

class _BaselineAtData {
  _BaselineAtData({this.cursor, this.items});

  final _BaselineCursor? cursor;
  final List<_BaselineAtItem>? items;

  factory _BaselineAtData.fromJson(Map<String, dynamic> json) =>
      _BaselineAtData(
        cursor: json['cursor'] == null
            ? null
            : _BaselineCursor.fromJson(json['cursor'] as Map<String, dynamic>),
        items: (json['items'] as List<dynamic>?)
            ?.map((e) => _BaselineAtItem.fromJson(e as Map<String, dynamic>))
            .toList(),
      );
}

class _BaselineLikeContent {
  _BaselineLikeContent({this.business, this.title, this.image, this.nativeUri});

  final String? business;
  final String? title;
  final String? image;
  final String? nativeUri;

  factory _BaselineLikeContent.fromJson(Map<String, dynamic> json) {
    return _BaselineLikeContent(
      business: json['business'] as String?,
      title: json['title'] as String?,
      image: json['image'] as String?,
      nativeUri: json['native_uri'] as String?,
    );
  }
}

class _BaselineLikeItem {
  _BaselineLikeItem({
    this.id,
    this.users,
    this.item,
    this.counts,
    this.likeTime,
    this.noticeState,
  });

  final int? id;
  final List<_BaselineUser>? users;
  final _BaselineLikeContent? item;
  final int? counts;
  final int? likeTime;
  final int? noticeState;

  factory _BaselineLikeItem.fromJson(Map<String, dynamic> json) =>
      _BaselineLikeItem(
        id: json['id'] as int?,
        users: (json['users'] as List<dynamic>?)
            ?.map((e) => _BaselineUser.fromJson(e as Map<String, dynamic>))
            .toList(),
        item: json['item'] == null
            ? null
            : _BaselineLikeContent.fromJson(
                json['item'] as Map<String, dynamic>,
              ),
        counts: json['counts'] as int?,
        likeTime: json['like_time'] as int?,
        noticeState: json['notice_state'] as int?,
      );
}

class _BaselineLatest {
  _BaselineLatest({this.items, this.lastViewAt});

  final List<_BaselineLikeItem>? items;
  final int? lastViewAt;

  factory _BaselineLatest.fromJson(Map<String, dynamic> json) =>
      _BaselineLatest(
        items: (json['items'] as List<dynamic>?)
            ?.map((e) => _BaselineLikeItem.fromJson(e))
            .toList(),
        lastViewAt: json['last_view_at'] as int?,
      );
}

class _BaselineTotal {
  _BaselineTotal({this.cursor, this.items});

  final _BaselineCursor? cursor;
  final List<_BaselineLikeItem>? items;

  factory _BaselineTotal.fromJson(Map<String, dynamic> json) => _BaselineTotal(
    cursor: json['cursor'] == null
        ? null
        : _BaselineCursor.fromJson(json['cursor'] as Map<String, dynamic>),
    items: (json['items'] as List<dynamic>?)
        ?.map((e) => _BaselineLikeItem.fromJson(e as Map<String, dynamic>))
        .toList(),
  );
}

class _BaselineLikeData {
  _BaselineLikeData({this.latest, this.total});

  final _BaselineLatest? latest;
  final _BaselineTotal? total;

  factory _BaselineLikeData.fromJson(Map<String, dynamic> json) =>
      _BaselineLikeData(
        latest: json['latest'] == null
            ? null
            : _BaselineLatest.fromJson(json['latest'] as Map<String, dynamic>),
        total: json['total'] == null
            ? null
            : _BaselineTotal.fromJson(json['total'] as Map<String, dynamic>),
      );
}

class _BaselineLikeDetailPage {
  _BaselineLikeDetailPage({this.isEnd});

  final bool? isEnd;

  factory _BaselineLikeDetailPage.fromJson(Map<String, dynamic> json) =>
      _BaselineLikeDetailPage(isEnd: json['is_end'] as bool?);
}

class _BaselineLikeDetailCard {
  _BaselineLikeDetailCard({this.business, this.title});

  final String? business;
  final String? title;

  factory _BaselineLikeDetailCard.fromJson(Map<String, dynamic> json) =>
      _BaselineLikeDetailCard(
        business: json['business'] as String?,
        title: json['title'] as String?,
      );
}

class _BaselineLikeDetailItem {
  _BaselineLikeDetailItem({this.user, this.likeTime});

  final _BaselineUser? user;
  final int? likeTime;

  factory _BaselineLikeDetailItem.fromJson(Map<String, dynamic> json) =>
      _BaselineLikeDetailItem(
        user: json['user'] == null
            ? null
            : _BaselineUser.fromJson(json['user'] as Map<String, dynamic>),
        likeTime: json['like_time'] as int?,
      );
}

class _BaselineLikeDetailData {
  _BaselineLikeDetailData({this.page, this.card, this.items});

  final _BaselineLikeDetailPage? page;
  final _BaselineLikeDetailCard? card;
  final List<_BaselineLikeDetailItem>? items;

  factory _BaselineLikeDetailData.fromJson(Map<String, dynamic> json) =>
      _BaselineLikeDetailData(
        page: json['page'] == null
            ? null
            : _BaselineLikeDetailPage.fromJson(
                json['page'] as Map<String, dynamic>,
              ),
        card: json['card'] == null
            ? null
            : _BaselineLikeDetailCard.fromJson(
                json['card'] as Map<String, dynamic>,
              ),
        items: (json['items'] as List<dynamic>?)
            ?.map(
              (e) => _BaselineLikeDetailItem.fromJson(e as Map<String, dynamic>),
            )
            .toList(),
      );
}

class _BaselineSysMsgItem {
  int? id;
  int? cursor;
  String? title;
  String? content;
  String? timeAt;

  _BaselineSysMsgItem.fromJson(Map<String, dynamic> json) {
    id = json['id'] as int?;
    cursor = json['cursor'] as int?;
    title = json['title'] as String?;
    content = json['content'] as String?;
    if (content != null) {
      try {
        dynamic json = jsonDecode(content!);
        if (json?['web'] != null) {
          content = json['web'];
        }
      } catch (_) {}
    }
    timeAt = json['time_at'] as String?;
  }
}

class _BaselineSingleUnread {
  _BaselineSingleUnread({
    required this.unfollowUnread,
    required this.followUnread,
    required this.unfollowPushMsg,
    required this.dustbinPushMsg,
    required this.dustbinUnread,
    required this.bizMsgUnfollowUnread,
    required this.bizMsgFollowUnread,
    required this.customUnread,
  });

  final int unfollowUnread;
  final int followUnread;
  final int unfollowPushMsg;
  final int dustbinPushMsg;
  final int dustbinUnread;
  final int bizMsgUnfollowUnread;
  final int bizMsgFollowUnread;
  final int customUnread;

  factory _BaselineSingleUnread.fromJson(Map<String, dynamic> json) =>
      _BaselineSingleUnread(
        unfollowUnread: json['unfollow_unread'] ?? 0,
        followUnread: json['follow_unread'] ?? 0,
        unfollowPushMsg: json['unfollow_push_msg'] ?? 0,
        dustbinPushMsg: json['dustbin_push_msg'] ?? 0,
        dustbinUnread: json['dustbin_unread'] ?? 0,
        bizMsgUnfollowUnread: json['biz_msg_unfollow_unread'] ?? 0,
        bizMsgFollowUnread: json['biz_msg_follow_unread'] ?? 0,
        customUnread: json['custom_unread'] ?? 0,
      );
}

class _BaselineFeedUnread {
  _BaselineFeedUnread({
    required this.at,
    required this.like,
    required this.reply,
    required this.sysMsg,
  });

  final int at;
  final int like;
  final int reply;
  final int sysMsg;

  factory _BaselineFeedUnread.fromJson(Map<String, dynamic> json) =>
      _BaselineFeedUnread(
        at: json['at'] ?? 0,
        like: json['like'] ?? 0,
        reply: json['reply'] ?? 0,
        sysMsg: json['sys_msg'] ?? 0,
      );
}

/// 基线的 `ImUserInfosData` + `model_avatar.dart` 的三棵子树，合并转写。
///
/// `Vip.fromJson` 读的是 `json['type'] ?? json['vipType']` 与
/// `json['status'] ?? json['vipStatus'] ?? 0`——两种键名都认。IM 这条接口
/// 给的是前者，但转写时照抄，不简化。
class _BaselineImUserInfo {
  _BaselineImUserInfo({
    this.mid,
    this.name,
    this.face,
    this.sign,
    this.vipStatus,
    this.vipType,
    this.pendantImage,
    this.officialType,
  });

  final int? mid;
  final String? name;
  final String? face;
  final String? sign;
  final int? vipStatus;
  final int? vipType;
  final String? pendantImage;
  final int? officialType;

  factory _BaselineImUserInfo.fromJson(Map<String, dynamic> json) {
    final vip = json['vip'] as Map<String, dynamic>?;
    final pendant = json['pendant'] as Map<String, dynamic>?;
    final official = json['official'] as Map<String, dynamic>?;
    return _BaselineImUserInfo(
      mid: json['mid'] as int?,
      name: json['name'] as String?,
      face: json['face'] as String?,
      sign: json['sign'] as String?,
      vipStatus: vip == null
          ? null
          : (vip['status'] ?? vip['vipStatus'] ?? 0) as int,
      vipType: vip == null ? null : (vip['type'] ?? vip['vipType']) as int?,
      pendantImage: pendant?['image'] as String?,
      officialType: official?['type'] as int?,
    );
  }
}

class _BaselineSessionSs {
  _BaselineSessionSs({
    this.followStatus,
    this.pushSetting,
    this.showPushSetting,
  });

  final int? followStatus;
  final int? pushSetting;
  final int? showPushSetting;

  factory _BaselineSessionSs.fromJson(Map<String, dynamic> json) =>
      _BaselineSessionSs(
        followStatus: json['follow_status'] as int?,
        pushSetting: json['push_setting'] as int?,
        showPushSetting: json['show_push_setting'] as int?,
      );
}

class _BaselineUidSetting {
  _BaselineUidSetting({this.setting});

  final int? setting;

  factory _BaselineUidSetting.fromJson(Map<String, dynamic> json) =>
      _BaselineUidSetting(setting: json['setting'] as int?);
}
