import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

/// S1（issue #51）的一次性基线对照，机制见 ADR-011 决策五。
///
/// 与前几批同样的处境：`ReplyData`、`ReplyReplyData`、`ReplyInteractData`、
/// `EmoteModelData` 都已随本次迁移删除，没有「还活着的基线模型」可借，于是
/// 本文件自带 oracle——下面的 `_Baseline*` 是迁移前那几个 `fromJson` 的
/// **逐行转写**，取自合并前的 `main`（`24ebcae9a`）：
///
/// - `lib/models_new/reply/{data,reply,member,content,cursor,
///   pagination_reply,top,upper}.dart`
/// - `lib/models_new/reply2reply/{data,page,root}.dart`
/// - `lib/models_new/reply_interaction/{data,interact_status}.dart`
/// - `lib/models_new/emote/{data,package,emote,meta}.dart`
///
/// 转写时不许顺手修正任何怪异之处：它要证明的正是 Rust 与迁移前行为一致。
///
/// **一处刻意的不一致**，在断言里显式换算，不是漏网：基线
/// `ReplyMember.mid` 是**字符串**（服务端为绕开 JS 精度而下发的形态），
/// 迁移后收成整数，所以这里按 `int.parse` 之后比较。
///
/// **本测试只覆盖被迁移的字段**。基线那 20 个模型里的勋章、挂件、大会员
/// 标签、等级、认证、折叠、UP 互动等装饰子树在迁移前就没有消费者（评论区
/// UI 走 gRPC），本切片不映射它们，这里自然也不对照——它们随模型一起删掉，
/// 不存在「两边不一致」的可能。
///
/// `interaction` 与 `emote_panel` 两份 fixture 是**手工构造**的（两条都要
/// 登录态：匿名请求开关状态是 `code -101`，表情包面板返回 `packages: null`），
/// 对照它们证明的是映射一致，不能证明我们对协议的理解正确——那要等 Phase 2
/// 的真机验证。
void main() {
  const dir = 'rust/crates/bili-api/tests/fixtures/reply';

  Map<String, dynamic> read(String name) =>
      jsonDecode(File('$dir/$name').readAsStringSync()) as Map<String, dynamic>;

  void expectItemMatches(
    Map<String, dynamic>? snapshot,
    _BaselineReply? baseline, {
    required String where,
  }) {
    if (baseline == null) {
      expect(snapshot, isNull, reason: '$where: 基线没有这条，快照也不该有');
      return;
    }
    expect(snapshot, isNotNull, reason: '$where: 基线有这条');
    final r = snapshot!;
    expect(r['rpid'], baseline.rpid, reason: '$where: rpid');
    expect(r['oid'], baseline.oid, reason: '$where: oid');
    expect(r['kind'], baseline.type, reason: '$where: type');
    expect(r['mid'], baseline.mid, reason: '$where: mid');
    expect(r['root'], baseline.root, reason: '$where: root');
    expect(r['parent'], baseline.parent, reason: '$where: parent');
    expect(r['dialog'], baseline.dialog, reason: '$where: dialog');
    expect(r['count'], baseline.count, reason: '$where: 楼中楼总数');
    expect(r['rcount'], baseline.rcount, reason: '$where: 楼中楼可见数');
    expect(r['state'], baseline.state, reason: '$where: state');
    expect(r['fansgrade'], baseline.fansgrade, reason: '$where: fansgrade');
    expect(r['attr'], baseline.attr, reason: '$where: attr');
    expect(r['ctime'], baseline.ctime, reason: '$where: ctime');
    expect(r['like'], baseline.like, reason: '$where: like');
    expect(r['action'], baseline.action, reason: '$where: action');
    expect(r['invisible'], baseline.invisible, reason: '$where: invisible');
    expect(r['message'], baseline.message, reason: '$where: 正文');

    final member = r['member'] as Map<String, dynamic>?;
    expect(member?['uname'], baseline.uname, reason: '$where: 昵称');
    expect(member?['avatar'], baseline.avatar, reason: '$where: 头像');
    // 基线这里是字符串，迁移后是整数。
    expect(
      member?['mid'],
      baseline.memberMid == null ? null : int.parse(baseline.memberMid!),
      reason: '$where: 发布者 mid',
    );
  }

  void expectListMatches(String rawName, String snapshotName) {
    final data = read(rawName)['data'] as Map<String, dynamic>;
    final snapshot = read(snapshotName);
    final b = _BaselineReplyData.fromJson(data);

    final replies = (snapshot['replies'] as List).cast<Map<String, dynamic>>();
    expect(replies.length, b.replies?.length ?? 0, reason: '评论条数');
    for (var i = 0; i < replies.length; i++) {
      expectItemMatches(replies[i], b.replies![i], where: '第 $i 条评论');
    }

    final tops = (snapshot['top_replies'] as List).cast<Map<String, dynamic>>();
    expect(tops.length, b.topReplies?.length ?? 0, reason: '置顶条数');
    for (var i = 0; i < tops.length; i++) {
      expectItemMatches(tops[i], b.topReplies![i], where: '第 $i 条置顶');
    }

    expectItemMatches(
      snapshot['top_upper'] as Map<String, dynamic>?,
      b.topUpper,
      where: 'top.upper',
    );
    expect(snapshot['upper_mid'], b.upperMid, reason: 'UP 主 mid');

    final cursor = snapshot['cursor'] as Map<String, dynamic>?;
    if (b.cursor == null) {
      expect(cursor, isNull, reason: '基线没有 cursor');
    } else {
      final c = b.cursor!;
      expect(cursor?['is_begin'], c.isBegin, reason: 'is_begin');
      expect(cursor?['is_end'], c.isEnd, reason: 'is_end');
      expect(cursor?['prev'], c.prev, reason: 'prev');
      expect(cursor?['next'], c.next, reason: 'next');
      expect(cursor?['next_offset'], c.nextOffset, reason: 'next_offset');
      expect(cursor?['mode'], c.mode, reason: 'mode');
      expect(cursor?['mode_text'], c.modeText, reason: 'mode_text');
      expect(cursor?['all_count'], c.allCount, reason: 'all_count');
      expect(cursor?['name'], c.name, reason: 'name');
    }

    final page = snapshot['page'] as Map<String, dynamic>?;
    if (b.page == null) {
      expect(page, isNull, reason: '基线没有 page');
    } else {
      expect(page?['num'], b.page!.num, reason: 'page.num');
      expect(page?['size'], b.page!.size, reason: 'page.size');
      expect(page?['count'], b.page!.count, reason: 'page.count');
    }
  }

  test('根评论列表（匿名形态，真实样本）：Rust 快照与基线逐字段一致', () {
    expectListMatches(
      'reply_list_cursor.raw.json',
      'reply_list_cursor.rust.json',
    );
  });

  test('根评论列表（登录形态，真实样本）：Rust 快照与基线逐字段一致', () {
    expectListMatches(
      'reply_list_paged.raw.json',
      'reply_list_paged.rust.json',
    );
  });

  test('楼中楼（真实样本）：Rust 快照与基线逐字段一致', () {
    final data = read('reply_reply.raw.json')['data'] as Map<String, dynamic>;
    final snapshot = read('reply_reply.rust.json');
    final b = _BaselineReplyReplyData.fromJson(data);

    final replies = (snapshot['replies'] as List).cast<Map<String, dynamic>>();
    expect(replies.length, b.replies?.length ?? 0, reason: '楼中楼条数');
    for (var i = 0; i < replies.length; i++) {
      expectItemMatches(replies[i], b.replies![i], where: '第 $i 条楼中楼');
    }

    expectItemMatches(
      snapshot['root'] as Map<String, dynamic>?,
      b.root,
      where: '根评论',
    );
    expect(snapshot['upper_mid'], b.upperMid, reason: 'UP 主 mid');

    final page = snapshot['page'] as Map<String, dynamic>?;
    expect(page?['num'], b.page?.num, reason: 'page.num');
    expect(page?['size'], b.page?.size, reason: 'page.size');
    expect(page?['count'], b.page?.count, reason: 'page.count');
  });

  test('评论区开关状态：Rust 快照与基线逐字段一致', () {
    final data = read('interaction.raw.json')['data'] as Map<String, dynamic>;
    final snapshot = read('interaction.rust.json');
    final b = _BaselineInteract.fromJson(data);

    final selection = snapshot['up_reply_selection'] as Map<String, dynamic>;
    expect(selection['can_modify'], b.selectionCanModify);
    expect(selection['status'], b.selectionStatus);

    final reply = snapshot['up_reply'] as Map<String, dynamic>;
    expect(reply['can_modify'], b.replyCanModify);
    expect(reply['status'], b.replyStatus);
  });

  test('表情包面板：Rust 快照与基线逐字段一致', () {
    final data = read('emote_panel.raw.json')['data'] as Map<String, dynamic>;
    // 这一份快照的顶层是数组：Rust 侧直接返回 `Vec<EmotePackage>`，没有
    // 再包一层 data——基线的 `EmoteModelData` 只是为了取出 `packages`。
    final packages = (jsonDecode(
      File('$dir/emote_panel.rust.json').readAsStringSync(),
    ) as List).cast<Map<String, dynamic>>();
    final b = _BaselineEmoteData.fromJson(data);

    expect(packages.length, b.packages?.length ?? 0, reason: '表情包组数');
    for (var i = 0; i < packages.length; i++) {
      final r = packages[i];
      final p = b.packages![i];
      expect(r['url'], p.url, reason: '第 $i 组图标');
      // 基线叫 type，迁移后叫 kind（type 在 Rust 里是关键字）。
      expect(r['kind'], p.type, reason: '第 $i 组类型');

      final emotes = (r['emote'] as List).cast<Map<String, dynamic>>();
      expect(emotes.length, p.emote?.length ?? 0, reason: '第 $i 组表情数');
      for (var j = 0; j < emotes.length; j++) {
        final e = p.emote![j];
        expect(emotes[j]['text'], e.text, reason: '第 $i 组第 $j 个文本');
        expect(emotes[j]['url'], e.url, reason: '第 $i 组第 $j 个图');
        final meta = emotes[j]['meta'] as Map<String, dynamic>?;
        expect(meta?['size'], e.size, reason: '第 $i 组第 $j 个尺寸');
        expect(meta?['alias'], e.alias, reason: '第 $i 组第 $j 个别名');
      }
    }
  });
}

// ---------------------------------------------------------------------------
// 以下是迁移前 fromJson 的逐行转写，见文件头。只转写被迁移的字段：没被
// 迁移的那些装饰字段在基线里也只是「存下来没人读」。
// ---------------------------------------------------------------------------

class _BaselineReply {
  int? rpid;
  int? oid;
  int? type;
  int? mid;
  int? root;
  int? parent;
  int? dialog;
  int? count;
  int? rcount;
  int? state;
  int? fansgrade;
  int? attr;
  int? ctime;
  int? like;
  int? action;
  bool? invisible;
  String? message;
  String? memberMid;
  String? uname;
  String? avatar;

  _BaselineReply.fromJson(Map<String, dynamic> json) {
    rpid = json['rpid'] as int?;
    oid = json['oid'] as int?;
    type = json['type'] as int?;
    mid = json['mid'] as int?;
    root = json['root'] as int?;
    parent = json['parent'] as int?;
    dialog = json['dialog'] as int?;
    count = json['count'] as int?;
    rcount = json['rcount'] as int?;
    state = json['state'] as int?;
    fansgrade = json['fansgrade'] as int?;
    attr = json['attr'] as int?;
    ctime = json['ctime'] as int?;
    like = json['like'] as int?;
    action = json['action'] as int?;
    invisible = json['invisible'] as bool?;
    final content = json['content'] as Map<String, dynamic>?;
    message = content?['message'] as String?;
    final member = json['member'] as Map<String, dynamic>?;
    memberMid = member?['mid'] as String?;
    uname = member?['uname'] as String?;
    avatar = member?['avatar'] as String?;
  }
}

class _BaselineCursor {
  bool? isBegin;
  bool? isEnd;
  int? prev;
  int? next;
  String? nextOffset;
  int? mode;
  String? modeText;
  int? allCount;
  String? name;

  _BaselineCursor.fromJson(Map<String, dynamic> json) {
    isBegin = json['is_begin'] as bool?;
    isEnd = json['is_end'] as bool?;
    prev = json['prev'] as int?;
    next = json['next'] as int?;
    final pagination = json['pagination_reply'] as Map<String, dynamic>?;
    nextOffset = pagination?['next_offset'] as String?;
    mode = json['mode'] as int?;
    modeText = json['mode_text'] as String?;
    allCount = json['all_count'] as int?;
    name = json['name'] as String?;
  }
}

class _BaselinePage {
  int? num;
  int? size;
  int? count;

  _BaselinePage.fromJson(Map<String, dynamic> json) {
    num = json['num'] as int?;
    size = json['size'] as int?;
    count = json['count'] as int?;
  }
}

class _BaselineReplyData {
  _BaselineCursor? cursor;
  _BaselinePage? page;
  List<_BaselineReply>? replies;
  List<_BaselineReply>? topReplies;
  _BaselineReply? topUpper;
  int? upperMid;

  _BaselineReplyData.fromJson(Map<String, dynamic> json) {
    cursor = json['cursor'] == null
        ? null
        : _BaselineCursor.fromJson(json['cursor'] as Map<String, dynamic>);
    page = json['page'] == null
        ? null
        : _BaselinePage.fromJson(json['page'] as Map<String, dynamic>);
    replies = (json['replies'] as List<dynamic>?)
        ?.map((e) => _BaselineReply.fromJson(e as Map<String, dynamic>))
        .toList();
    topReplies = (json['top_replies'] as List<dynamic>?)
        ?.map((e) => _BaselineReply.fromJson(e as Map<String, dynamic>))
        .toList();
    final top = json['top'] as Map<String, dynamic>?;
    topUpper = top?['upper'] == null
        ? null
        : _BaselineReply.fromJson(top!['upper'] as Map<String, dynamic>);
    final upper = json['upper'] as Map<String, dynamic>?;
    upperMid = upper?['mid'] as int?;
  }
}

class _BaselineReplyReplyData {
  _BaselinePage? page;
  _BaselineReply? root;
  List<_BaselineReply>? replies;
  int? upperMid;

  _BaselineReplyReplyData.fromJson(Map<String, dynamic> json) {
    page = json['page'] == null
        ? null
        : _BaselinePage.fromJson(json['page'] as Map<String, dynamic>);
    root = json['root'] == null
        ? null
        : _BaselineReply.fromJson(json['root'] as Map<String, dynamic>);
    replies = (json['replies'] as List<dynamic>?)
        ?.map((e) => _BaselineReply.fromJson(e as Map<String, dynamic>))
        .toList();
    final upper = json['upper'] as Map<String, dynamic>?;
    upperMid = upper?['mid'] as int?;
  }
}

class _BaselineInteract {
  late bool selectionCanModify;
  late int selectionStatus;
  late bool replyCanModify;
  late int replyStatus;

  _BaselineInteract.fromJson(Map<String, dynamic> json) {
    // 基线 InteractStatus.fromJson 对缺字段是直接抛异常的，这里照样写。
    selectionCanModify = json['up_reply_selection']['can_modify'] as bool;
    selectionStatus = json['up_reply_selection']['status'] as int;
    replyCanModify = json['up_reply']['can_modify'] as bool;
    replyStatus = json['up_reply']['status'] as int;
  }
}

class _BaselineEmoteData {
  List<_BaselinePackage>? packages;

  _BaselineEmoteData.fromJson(Map<String, dynamic> json) {
    packages = (json['packages'] as List<dynamic>?)
        ?.map((e) => _BaselinePackage.fromJson(e as Map<String, dynamic>))
        .toList();
  }
}

class _BaselinePackage {
  String? url;
  int? type;
  List<_BaselineEmote>? emote;

  _BaselinePackage.fromJson(Map<String, dynamic> json) {
    url = json['url'] as String?;
    type = json['type'] as int?;
    emote = (json['emote'] as List<dynamic>?)
        ?.map((e) => _BaselineEmote.fromJson(e as Map<String, dynamic>))
        .toList();
  }
}

class _BaselineEmote {
  String? text;
  String? url;
  int? size;
  String? alias;

  _BaselineEmote.fromJson(Map<String, dynamic> json) {
    text = json['text'] as String?;
    url = json['url'] as String?;
    final meta = json['meta'] as Map<String, dynamic>?;
    size = meta?['size'] as int?;
    alias = meta?['alias'] as String?;
  }
}
