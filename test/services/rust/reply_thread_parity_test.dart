import 'dart:collection';
import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

/// S2 续单（issue #179）线程化视图合成的**一次性基线对照**，机制见
/// ADR-011 决策五。
///
/// 合成已迁到 `bili_domain::reply_thread`，`ReplyTree.build` 里的上溯、
/// 合成、排序与拍平随之删除——于是没有「还活着的基线实现」可借。
///
/// 因此本文件自带 oracle：下面的 `_baselineBuild` 是迁移前那段代码的
/// **逐行转写**，取自合并前的 `main`（`de0899f81`）的
/// `lib/pages/video/reply_reply/tree.dart`。转写时**不许顺手修正**任何
/// 怪异之处——包括那几条 `assert(false)` 之后仍然继续往下走的分支：
/// release 包里它们就是这么跑的。
///
/// 两侧跑同一份文件
/// （`rust/crates/bili-domain/tests/fixtures/reply_thread/cases.json`）：
/// 本测试是判据，`reply_thread_golden.rs` 是被验的那一侧。
///
/// **这个测试是有寿命的**：行为在 Phase 2 真机回归里确认过之后就可以删掉
/// 它，fixture 与 Rust 侧长期留下。
///
/// ## 转写时动了什么
///
/// 1. 基线的节点是 `ReplyEntity`（FRB 生成类），这里换成只带建树要读的
///    那几个字段的 `_Item`。判定逻辑一字不差地读同名字段。
/// 2. 基线的 `rows` 只收**可见**节点，折叠起来的那些只挂在
///    `ReplyTreeNode._children` 上；Rust 一次给出全部节点加一个 `visible`
///    标记。所以转写时 `walk` 把每个节点都记一行，`visible` 照原样传。
///    可见的那个子序列与基线的 `rows` 逐行相同。
/// 3. `_placeholder` 造的是一条完整的 `ReplyEntity`，这里只记它真正带
///    信息的那几个字段（parent / dialog / ctime / 作者的 mid 与昵称）。
void main() {
  const path = 'rust/crates/bili-domain/tests/fixtures/reply_thread/cases.json';
  final fixture =
      jsonDecode(File(path).readAsStringSync()) as Map<String, dynamic>;

  group('线程化视图合成与迁移前的基线一致', () {
    final cases = fixture['cases'] as List<dynamic>;

    test('用例不为空，否则本测试形同虚设', () {
      expect(cases, isNotEmpty);
    });

    for (final raw in cases) {
      final c = raw as Map<String, dynamic>;
      final name = c['name'] as String;

      test(name, () {
        final data = ((c['data'] as List<dynamic>?) ?? const [])
            .cast<Map<String, dynamic>>()
            .toList()
            .asMap()
            .entries
            .map((e) => _Item.fromJson(e.value, source: 'data', index: e.key))
            .toList();
        final removed = <int, _Item>{
          for (final r in ((c['removed'] as List<dynamic>?) ?? const [])
              .cast<Map<String, dynamic>>())
            r['id'] as int: _Item.fromJson(r, source: 'removed'),
        };

        final rows = _baselineBuild(
          data: data,
          removed: removed,
          rootId: c['root_id'] as int,
          foldDepth: c['fold_depth'] as int? ?? 0,
        );

        expect(
          rows.map((r) => r.toJson()).toList(),
          c['rows'] as List<dynamic>,
          reason: 'fixture 的期望值与迁移前的基线不符',
        );
      });
    }
  });
}

// ─────────────────────────── fixture 的投影 ───────────────────────────

class _User {
  const _User(this.mid, this.name);

  final int mid;
  final String name;
}

class _ParentMember {
  const _ParentMember(this.mid, this.name);

  /// 古早评论可能只有昵称没有 mid，这个 null 是有意义的。
  final int? mid;
  final String name;
}

class _Item {
  _Item({
    required this.id,
    required this.parent,
    required this.dialog,
    required this.ctime,
    required this.member,
    required this.parentMember,
    required this.members,
    required this.source,
    this.index,
  });

  final int id;
  final int parent;
  final int dialog;
  final int ctime;
  final _User member;
  final _ParentMember? parentMember;
  final Map<String, _User> members;

  /// 'data' / 'removed' / 'synthetic'。
  final String source;
  final int? index;

  factory _Item.fromJson(
    Map<String, dynamic> json, {
    required String source,
    int? index,
  }) {
    final member = json['member'] as Map<String, dynamic>?;
    final parentMember = json['parent_member'] as Map<String, dynamic>?;
    return _Item(
      id: json['id'] as int,
      parent: json['parent'] as int,
      dialog: json['dialog'] as int,
      ctime: json['ctime'] as int,
      member: member == null
          ? const _User(0, '')
          : _User(member['mid'] as int? ?? 0, member['name'] as String? ?? ''),
      parentMember: parentMember == null
          ? null
          : _ParentMember(
              parentMember['mid'] as int?,
              parentMember['name'] as String? ?? '',
            ),
      members: ((json['members'] as Map<String, dynamic>?) ?? const {}).map(
        (key, value) => MapEntry(
          key,
          _User(
            (value as Map<String, dynamic>)['mid'] as int? ?? 0,
            value['name'] as String? ?? '',
          ),
        ),
      ),
      source: source,
      index: index,
    );
  }
}

/// 一行。字段与 fixture 一一对应，等于默认值的不写出来。
class _Row {
  _Row({
    required this.item,
    required this.depth,
    required this.parent,
    required this.visible,
    this.reason,
  });

  final _Item item;
  final int depth;
  final int? parent;
  final bool visible;
  String? reason;
  int collapsed = 0;

  Map<String, dynamic> toJson() => {
    'source': item.source,
    if (item.index != null) 'index': item.index,
    'id': item.id,
    'depth': depth,
    if (parent != null) 'parent': parent,
    if (reason != null) 'reason': reason,
    if (collapsed != 0) 'collapsed': collapsed,
    if (!visible) 'visible': false,
    if (item.source == 'synthetic')
      'placeholder': {
        'parent': item.parent,
        'dialog': item.dialog,
        'ctime': item.ctime,
        'mid': item.member.mid,
        'name': item.member.name,
      },
  };
}

// ────────────────── 迁移前 `ReplyTree.build` 的逐行转写 ──────────────────

/// `ctime` 的上下界。基线用 `Int64.MAX_VALUE` / `MIN_VALUE` 做扫描初值。
const _maxCtime = 0x7fffffffffffffff;
const _minCtime = -0x8000000000000000;

/// 基线 `_placeholder`。
_Item _placeholder({
  required int id,
  required int parent,
  required int dialog,
  required int ctime,
  required _User? member,
}) => _Item(
  id: id,
  parent: parent,
  dialog: dialog,
  ctime: ctime,
  member: member ?? const _User(0, ''),
  parentMember: null,
  members: const {},
  source: 'synthetic',
);

/// 基线 `ReplyTree.replyCompare`。
int _replyCompare(_Item a, _Item b) {
  final ctimeCmp = a.ctime.compareTo(b.ctime);
  if (ctimeCmp != 0) return ctimeCmp;
  return a.id.compareTo(b.id);
}

/// 基线 `_extractMention`。
(int, _User?) _extractMention(_Item reply) {
  final parent = reply.parentMember;
  if (parent == null) return const (1 << 30, null);
  final members = reply.members;

  if (members[parent.name] case final m?) {
    return (1, m);
  }
  if (parent.mid case final mid?) {
    for (final m in members.values) {
      if (m.mid == mid) return (1, m);
    }
  }
  if (parent.name == reply.member.name) {
    return (2, reply.member);
  }
  return (3, _User(parent.mid ?? 0, parent.name));
}

List<_Row> _baselineBuild({
  required List<_Item> data,
  required Map<int, _Item> removed,
  required int rootId,
  required int foldDepth,
}) {
  // 基线由调用点按 id 去重后把 `seen` 一并转移进来。
  final seen = HashSet<int>()..addAll(data.map((i) => i.id));

  if (foldDepth == 0) foldDepth = -1;

  final children = HashMap<int, List<_Item>>();
  final suppressed = HashMap<int, String>();

  void addNode(_Item r) {
    (children[r.parent] ??= []).add(r);
    seen.add(r.id);
  }

  // ── 1. 上溯 + 建桶 ──
  final toSynthDialogs = <int>{};
  final toSynthParents = <int>{};

  for (final r in data) {
    (children[r.parent] ??= []).add(r);

    final d = r.dialog;
    if (r.id != d && !seen.contains(d) && !toSynthDialogs.contains(d)) {
      if (removed[d] case final dialog?) {
        addNode(dialog);
        suppressed[d] = 'blocked';
      } else {
        toSynthDialogs.add(d);
      }
    }

    for (var p = r.parent, i = 0; p != rootId && i < 512; i++) {
      if (seen.contains(p)) break;
      final anc = removed[p];
      if (anc == null) {
        toSynthParents.add(p);
        break;
      }
      addNode(anc);
      suppressed[p] = 'blocked';
      p = anc.parent;
    }
  }

  // ── 2. 合成缺失父 ──
  final deferred = <int>[];

  _Item synthReply(int p, List<_Item> kids, [bool needAnchor = false]) {
    var mKidCtime = _maxCtime;

    _User? mention;
    var mPri = 1 << 30;
    var mMentCtime = _minCtime;

    int? anchor;

    for (final r in kids) {
      if (r.ctime < mKidCtime) mKidCtime = r.ctime;

      final (pri, m) = _extractMention(r);
      if (pri < mPri || (pri == mPri && r.ctime > mMentCtime)) {
        mention = m;
        mPri = pri;
        mMentCtime = r.ctime;
      }

      if (needAnchor && anchor == null) {
        final d = r.dialog;
        if (d != p && seen.contains(d)) anchor = d;
      }
    }

    return _placeholder(
      id: p,
      parent: anchor ?? rootId,
      dialog: anchor ?? p,
      ctime: mKidCtime,
      member: mention,
    );
  }

  // ── 2a. 合成缺失 dialog ──
  for (final p in toSynthDialogs) {
    toSynthParents.remove(p);
    if (seen.contains(p)) {
      continue;
    }

    if (children[p] case final kids?) {
      addNode(synthReply(p, kids));
      suppressed[p] = 'deleted';
    } else {
      seen.add(p);
      deferred.add(p);
    }
  }
  toSynthDialogs.clear();

  // ── 2b. 合成缺失父 ──
  for (final p in toSynthParents) {
    if (children[p] case final kids?) {
      addNode(synthReply(p, kids, true));
      suppressed[p] = 'deleted';
    }
  }
  toSynthParents.clear();

  // ── 2c. 实体化 deferred ──
  for (final p in deferred) {
    if (children[p] case final kids?) {
      addNode(synthReply(p, kids));
      suppressed[p] = 'deleted';
    }
  }
  deferred.clear();

  // ── 3. DFS 前序 ──
  final rows = <_Row>[];
  final visited = HashSet<int>();

  int walk(_Item r, int depth, int? parent, bool visible) {
    if (!visited.add(r.id)) {
      return -1;
    }

    final rowIndex = rows.length;
    rows.add(
      _Row(
        item: r,
        depth: depth,
        parent: parent,
        visible: visible,
        reason: suppressed[r.id],
      ),
    );

    final kids = children[r.id];
    var count = 0;
    if (kids != null) {
      kids.sort(_replyCompare);
      final autoFold = depth == foldDepth;
      final childVisible = visible && !autoFold;
      for (final k in kids) {
        if (identical(k, r)) {
          continue;
        }
        count += walk(k, depth + 1, rowIndex, childVisible) + 1;
      }
      if (autoFold) rows[rowIndex].collapsed = count;
    }
    return count;
  }

  final rootKids = children[rootId];
  if (rootKids != null) {
    rootKids.sort(_replyCompare);
    for (final i in rootKids) {
      walk(i, 0, null, true);
    }
  }

  return rows;
}
