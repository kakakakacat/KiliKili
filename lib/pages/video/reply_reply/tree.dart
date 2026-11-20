import 'dart:collection';

import 'package:kilikili/services/rust/rust_reply_grpc.dart';

/// 无法显示父评论的原因分类
enum ReasonType {
  deleted('已删除'),
  blocked('已屏蔽'),
  ;

  final String desc;
  const ReasonType(this.desc);
}

/// DFS 序里的一行（含合成占位）。
class ReplyTreeNode {
  ReplyTreeNode({
    required this.reply,
    required this.depth,
    this.parent,
    this.reason,
  });

  final ReplyEntity reply;
  final int depth;

  int collapsed = 0;
  ReasonType? reason;

  final ReplyTreeNode? parent;
  List<ReplyTreeNode>? _children;

  bool get isLeaf => _children == null || _children!.isEmpty;
  bool get hasChildren => _children != null && _children!.isNotEmpty;

  void _addChild(ReplyTreeNode node) {
    if (_children case final kids?) {
      kids.add(node);
    } else {
      _children = [node];
    }
  }

  // @override
  // int get hashCode => identityHashCode(reply);

  // @override
  // bool operator ==(Object other) {
  //   return other is ReplyTreeNode && identical(reply, other.reply);
  // }

  @override
  String toString({bool deep = false}) =>
      'ReplyTreeNode('
      'replyId=${reply.id}'
      ',depth=$depth'
      '${reason == null ? "" : ",suppressReason=${reason!.name}"}'
      '${isLeaf ? "" : ",children=[${deep ? _children!.map((i) => i.toString(deep: true)).join(',') : '...'}]"}'
      ')';
}

/// 树 + 折叠状态下的可见投影 [_vis]。
///
/// - 树结构：[ReplyTreeNode.parent] / [ReplyTreeNode._children] 指针。
/// - 可见序：[_vis] 是树的 DFS 前序投影，折叠时摘掉折叠节点的整棵子树，
///   展开时按 [ReplyTreeNode._children] 顺序（ctime 升序）插回。
extension type ReplyTree._(List<ReplyTreeNode> _vis)
    implements Iterable<ReplyTreeNode> {
  @pragma('vm:prefer-inline')
  ReplyTreeNode operator [](int index) => _vis[index];

  /// ctime 升序，相同再比 id。
  ///
  /// 排序本身在 Rust（`bili_domain::reply_thread::reply_compare`），这里
  /// 留的这一份**只给 [appendLeaf] 的断言用**：宿主插新回复时要自己保证
  /// 插在末尾，而那是宿主的不变量，不是协议的。两处都改才算改对。
  static int replyCompare(ReplyEntity a, ReplyEntity b) {
    final ctimeCmp = a.ctime.compareTo(b.ctime);
    if (ctimeCmp != 0) return ctimeCmp;
    return a.id.compareTo(b.id);
  }

  /// 从原始数据构建完整 DFS 序。
  ///
  /// 结构与合成都在 `bili_domain::reply_thread`（#179）：上溯链、缺失父
  /// 评论的合成、「回复了谁」的四级优先级、排序与自动折叠层级。这里只把
  /// 返回的行接成 [ReplyTreeNode]，并挑出这一刻可见的那些。
  ///
  /// [data] 需先按 [ReplyEntity.id] 去重。[root] 可不提供。
  ///
  /// Rust 不把实体发回来（合成出来的占位除外），行里带的是来源——
  /// [remove] 按 `identical` 在原始列表里定位，换一个等值的新对象会让那
  /// 个判定失效。
  factory ReplyTree.build({
    required List<ReplyEntity> data,
    required int rootId,
    required Map<int, ReplyEntity> removed,
    int foldDepth = 0,
    ReplyTreeNode? root,
  }) {
    assert(
      root == null || root.reply.id == rootId,
      'rootId: $rootId 与 reply: ${root.reply.id} 不等',
    );
    assert(
      data.map((i) => i.id).toSet().length == data.length,
      'data未去重',
    );

    final rows = RustReplyGrpc.thread(
      data: data,
      removed: removed,
      rootId: rootId,
      foldDepth: foldDepth,
    );

    // 行是 DFS 前序，父一定排在子前面，于是按下标回指即可。
    final nodes = <ReplyTreeNode>[];
    final vis = <ReplyTreeNode>[];

    for (final row in rows) {
      final reply = switch (row.source) {
        ReplyRowSource.data => data[row.index!],
        ReplyRowSource.removed => removed[row.id]!,
        ReplyRowSource.synthetic => row.placeholder!,
      };
      final parent = row.parent == null ? root : nodes[row.parent!];
      final node = ReplyTreeNode(
        reply: reply,
        depth: row.depth,
        parent: parent,
        reason: switch (row.reason) {
          ReplyRowReason.none => null,
          ReplyRowReason.deleted => ReasonType.deleted,
          ReplyRowReason.blocked => ReasonType.blocked,
        },
      )..collapsed = row.collapsed;

      parent?._addChild(node);
      nodes.add(node);
      if (row.visible) vis.add(node);
    }

    return ReplyTree._(vis);
  }

  /// 返回 [index] 处节点的可见子树在 [_vis] 中的末尾下标（不含）。
  ///
  /// 或许可以考虑多峰有平台的二分，但更复杂。
  int _visibleSubtreeEnd(int index) {
    final a = _vis[index];
    assert(a.collapsed == 0, '折叠的节点无可见子树');
    final d = a.depth;
    var end = index + (a._children?.length ?? 0) + 1;
    while (end < _vis.length && _vis[end].depth > d) {
      end++;
    }
    return end;
  }

  /// 切换 [index] 处的折叠状态。
  int toggleCollapse(int index) {
    final a = _vis[index];
    if (a.isLeaf) return 0;

    final start = index + 1;
    if (a.collapsed == 0) {
      final end = _visibleSubtreeEnd(index);
      _vis.removeRange(start, end);
      return a.collapsed = end - start;
    } else {
      final kids = a._children;
      if (kids == null || kids.isEmpty) return 0;
      final out = <ReplyTreeNode>[];

      /// 按 [ReplyTreeNode._children] 顺序 DFS，跳过嵌套折叠的子树。
      void add(ReplyTreeNode n) {
        out.add(n);
        if (n.collapsed != 0) return;
        final cs = n._children;
        if (cs == null || cs.isEmpty) return;
        for (final c in cs) {
          add(c);
        }
      }

      for (final c in kids) {
        add(c);
      }
      _vis.insertAll(start, out);
      a.collapsed = 0;
      return out.length;
    }
  }

  /// 向 [index] 处节点插入新回复 [c]。
  ///
  /// 前置条件：祖先均处于展开态，否则 [ReplyTreeNode.collapsed] 状态错误。
  /// [c] 是该节点所有子节点里 ctime 最大的。
  int appendLeaf(ReplyEntity c, int index) {
    final parent = _vis[index];
    assert(
      parent.isLeaf || replyCompare(parent._children!.last.reply, c) < 0,
      '回复：${c.id} 必须是 parent 所有子节点里 ctime 最大的',
    );
    assert(parent._debugAncestorsExpanded(), '祖先未处于展开态');

    final node = ReplyTreeNode(
      reply: c,
      depth: parent.depth + 1,
      parent: parent,
    );

    // 先扫再插children，避免叶子节点错位
    int insertIndex = -1;
    if (parent.collapsed == 0) {
      insertIndex = _visibleSubtreeEnd(index);
      _vis.insert(insertIndex, node);
    } else {
      parent.collapsed++;
    }

    parent._addChild(node);
    return insertIndex;
  }

  /// 删除 [index] 处的评论。
  ///
  /// - 有后代：使用「已删除」占位，结构不动（保留子树可见性）。
  /// - 叶子：从 [ReplyTreeNode._children] 与 [_vis] 移除。
  /// 前置条件：祖先均处于展开态，否则 [ReplyTreeNode.collapsed] 状态错误。
  ///
  /// 原始数据的同步删除使用 [ReplyList.reverseIdentityIndexOf] 定位：
  /// [ReplyEntity] 的 `==` 是按值比较（FRB 生成的实现逐字段比），同一页里
  /// 两条内容完全相同的评论会被当成同一个，不应使用 [List.remove]。
  ReplyTreeNode remove(int index) {
    final node = _vis[index];
    assert(node._debugAncestorsExpanded(), '祖先未处于展开态');

    if (node.hasChildren) {
      node.reason = ReasonType.deleted;
      return node;
    }

    _vis.removeAt(index);

    if (node.parent case final p?) {
      if (p._children case final kids?) {
        final kidsIdx = kids.reverseIdentityIndexOf(node);
        if (kidsIdx >= 0) {
          kids.removeAt(kidsIdx);
        } else {
          assert(false);
        }
      }
    }

    return node;
  }

  bool isParentFirstChild(int index) {
    if (index == 0) return true;
    return identical(_vis[index].parent, _vis[index - 1]);
  }

  bool isParentLastChild(int index) {
    if (index == _vis.length - 1) return true;
    return _vis[index + 1].depth < _vis[index].depth;
  }

}

extension ReplyTreeNodeUi on ReplyTreeNode {
  /// 根据某节点的 [ancestors]，判断 [this] 该高亮拿一根线，高亮线如图。
  /// ```
  /// R
  /// ║ A
  /// │ ║ B
  /// │ ║ └ C
  /// │ ║   └ D
  /// │ ║ E
  /// │ │ ║ F
  /// │ │ │ ║ G ([ancestors] sources)
  /// │ │ │ │ └ H (调用方应当提前过滤 index > G，否则将产生错误高亮)
  /// ```
  int highlightLineOf(HashSet<ReplyTreeNode> ancestors) {
    var step = -1;
    for (var p = parent; p != null && !ancestors.contains(p); p = p.parent) {
      step++;
    }
    return depth - step;
  }

  ReplyTreeNode? lookupAncestorForDepth(int depth) {
    assert(0 <= depth && depth < this.depth, '祖先深度应小于当前深度且非负');
    var p = parent;
    while (p != null && p.depth > depth) {
      p = p.parent;
    }
    return p;
  }

  bool _debugAncestorsExpanded() {
    for (var p = parent; p != null; p = p.parent) {
      if (p.collapsed != 0) {
        return false;
      }
    }
    return true;
  }
}

extension ReplyList<T> on Iterable<T> {
  /// Do NOT invoke it on non [IndexedIterable] object.
  int reverseIdentityIndexOf(T element, [int? end]) {
    for (int i = end ?? length - 1; i >= 0; i--) {
      if (identical(element, elementAt(i))) {
        return i;
      }
    }
    return -1;
  }
}
