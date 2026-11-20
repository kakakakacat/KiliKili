//! 线程化评论视图的合成（S2 续单 / issue #179）。
//!
//! 对应基线 `lib/pages/video/reply_reply/tree.dart` 里的 `ReplyTree.build`：
//! 把一页楼中楼接成一棵树，沿途**补出没有随页下发的父评论**，最后拍成
//! DFS 前序。
//!
//! ## 为什么在领域层
//!
//! 这一段是 AGENTS.md §2 点名的那几类之一：排序规则（`reply_compare`）、
//! 合并与去重（上溯链、交叉 id）、阈值判断（`fold_depth` 的自动折叠），
//! 外加一整套「协议没规定、但两端必须一致」的决定——占位父评论的 `ctime`
//! 取子评论最小值、`parent`/`dialog` 的 anchor 怎么选、「回复了谁」按四级
//! 优先级认人。换一个宿主框架，这些一条都不会变。
//!
//! 基线文件里那条「为什么留在宿主」的例外注释随本片作废：它当时写的退出
//! 条件就是「随线程化评论下沉那一片一起走」。
//!
//! ## 什么没有下来
//!
//! 折叠态与可见投影（`toggleCollapse` / `appendLeaf` / `remove`）留在宿主。
//! 它们是视图状态，不是算法：哪一行被用户点开了，换个框架本来就要重写。
//! 本模块因此一次给出**全部节点**（含折叠起来看不见的），由宿主自己决定
//! 这一刻画哪些。
//!
//! ## 为什么不把实体发回去
//!
//! 宿主删一条评论时用 `identical` 在原始列表里定位它
//! （`ReplyList.reverseIdentityIndexOf`）——FRB 生成的 `==` 是逐字段比，
//! 同一页里两条内容完全相同的评论会被当成同一个。所以行里带的是**来源**
//! （`data` 的下标 / `removed` 的键），由宿主解引用回原来的那个对象。
//! 只有合成出来的占位是新造的，那种才整条发过去。
//!
//! ## 顺手定死的两处迭代序
//!
//! 与 [`crate::reply_richtext`] 同一个毛病：`content.members` 是
//! `HashMap`，而基线的「改名后按 mid 认人」要取**第一个** mid 相同的成员。
//! 哈希序下这个「第一个」是不确定的，这里按昵称字典序取。
//! 待合成的 id 集合则按**登记顺序**处理（基线 `Set<int>` 是 LinkedHashSet），
//! 它会影响 anchor 落在哪一条子评论上。

use std::collections::{HashMap, HashSet};

use crate::reply_grpc::{ReplyEntity, ReplyMember, ReplyParentMember};

/// 上溯链的最大长度。基线写死 512，超过就当 `removed` 里有环。
const MAX_ANCESTOR_WALK: usize = 512;

/// 没有实体化的 mention 的优先级初值。基线 `1 << 30`。
const NO_MENTION: i32 = 1 << 30;

/// 建树要读的那几个字段，从 `ReplyEntity` 投影而来。
///
/// 形状照 [`crate::reply_filter::ReplyFilterItem`]：正文、配图、点赞数
/// 跟建树无关，不必跨过来。
#[derive(Debug, Clone, Default)]
pub struct ReplyThreadItem {
    pub id: i64,
    pub parent: i64,
    pub dialog: i64,
    /// Unix 秒。排序与占位的 `ctime` 都用它。
    pub ctime: i64,
    /// 这条评论的作者。
    pub member: ReplyMember,
    /// 被回复者。合成占位父评论时用它认人。
    pub parent_member: Option<ReplyParentMember>,
    /// 正文里的成员表（昵称 → 成员）。
    pub members: HashMap<String, ReplyMember>,
}

/// 一行的内容从哪来。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum ReplyRowSource {
    /// 传进来的那一页，`index` 是下标。
    Data,
    /// 被过滤规则删掉、靠 `removed` 捞回来的，`id` 是它的键。
    Removed,
    /// 合成出来的占位，整条在 `placeholder` 里。
    Synthetic,
}

/// 这一行为什么显示不出原文。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum ReplyRowReason {
    /// 正常评论。
    None,
    /// 服务端没下发，按「已删除」占位。
    Deleted,
    /// 被本地过滤规则删掉，按「已屏蔽」占位。
    Blocked,
}

/// DFS 前序里的一行。
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct ReplyThreadRow {
    pub source: ReplyRowSource,
    /// [`ReplyRowSource::Data`]：`data` 的下标。其余为 `None`。
    pub index: Option<usize>,
    /// [`ReplyRowSource::Removed`] / [`ReplyRowSource::Synthetic`]：评论 id。
    pub id: i64,
    /// [`ReplyRowSource::Synthetic`]：合成出来的那条占位。
    pub placeholder: Option<ReplyEntity>,
    /// 0 是根评论的直接子节点（基线的 `depth`）。
    pub depth: i32,
    /// 父**行**的下标；`None` 表示挂在根评论下。
    pub parent: Option<usize>,
    pub reason: ReplyRowReason,
    /// 这一行被自动折叠起来的后代数量。0 表示没折叠。
    pub collapsed: i32,
    /// 这一刻画不画得出来。折叠节点的整棵子树为 `false`。
    pub visible: bool,
}

/// 指向一个节点：传进来的那一页、`removed`、或者本次合成出来的占位。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NodeRef {
    Data(usize),
    Removed(i64),
    Synth(usize),
}

/// 合成出来的占位父评论。
#[derive(Debug, Clone)]
struct SynthNode {
    id: i64,
    parent: i64,
    dialog: i64,
    ctime: i64,
    /// 「回复了谁」。认不出来时为 `None`，基线此时给一个全空的作者。
    member: Option<ReplyMember>,
}

struct Builder<'a> {
    data: &'a [ReplyThreadItem],
    removed: &'a HashMap<i64, ReplyThreadItem>,
    root_id: i64,
    synth: Vec<SynthNode>,
    children: HashMap<i64, Vec<NodeRef>>,
    seen: HashSet<i64>,
    suppressed: HashMap<i64, ReplyRowReason>,
}

impl<'a> Builder<'a> {
    fn item(&self, node: NodeRef) -> Option<&'a ReplyThreadItem> {
        match node {
            NodeRef::Data(index) => self.data.get(index),
            NodeRef::Removed(id) => self.removed.get(&id),
            NodeRef::Synth(_) => None,
        }
    }

    fn id_of(&self, node: NodeRef) -> i64 {
        match node {
            NodeRef::Synth(index) => self.synth[index].id,
            other => self.item(other).map_or(0, |i| i.id),
        }
    }

    fn parent_of(&self, node: NodeRef) -> i64 {
        match node {
            NodeRef::Synth(index) => self.synth[index].parent,
            other => self.item(other).map_or(0, |i| i.parent),
        }
    }

    fn dialog_of(&self, node: NodeRef) -> i64 {
        match node {
            NodeRef::Synth(index) => self.synth[index].dialog,
            other => self.item(other).map_or(0, |i| i.dialog),
        }
    }

    fn ctime_of(&self, node: NodeRef) -> i64 {
        match node {
            NodeRef::Synth(index) => self.synth[index].ctime,
            other => self.item(other).map_or(0, |i| i.ctime),
        }
    }

    /// 基线 `addNode`：进桶 + 记进 `seen`。
    fn add_node(&mut self, node: NodeRef) {
        let parent = self.parent_of(node);
        let id = self.id_of(node);
        self.children.entry(parent).or_default().push(node);
        self.seen.insert(id);
    }

    /// 基线 `synthReply`。
    fn synth_reply(&self, p: i64, kids: &[NodeRef], need_anchor: bool) -> SynthNode {
        let mut min_kid_ctime = i64::MAX;

        let mut mention: Option<ReplyMember> = None;
        let mut min_pri = NO_MENTION;
        let mut max_mention_ctime = i64::MIN;

        let mut anchor: Option<i64> = None;

        for kid in kids {
            let ctime = self.ctime_of(*kid);
            if ctime < min_kid_ctime {
                min_kid_ctime = ctime;
            }

            let (pri, m) = extract_mention(self.item(*kid));
            if pri < min_pri || (pri == min_pri && ctime > max_mention_ctime) {
                mention = m;
                min_pri = pri;
                max_mention_ctime = ctime;
            }

            if need_anchor && anchor.is_none() {
                let d = self.dialog_of(*kid);
                if d != p && self.seen.contains(&d) {
                    anchor = Some(d);
                }
            }
        }

        SynthNode {
            id: p,
            parent: anchor.unwrap_or(self.root_id),
            dialog: anchor.unwrap_or(p),
            ctime: min_kid_ctime,
            member: mention,
        }
    }

    fn push_synth(&mut self, node: SynthNode) {
        let id = node.id;
        self.synth.push(node);
        self.add_node(NodeRef::Synth(self.synth.len() - 1));
        self.suppressed.insert(id, ReplyRowReason::Deleted);
    }
}

/// 基线 `ReplyTree.replyCompare`：ctime 升序，相同再比 id。
fn reply_compare(a: (i64, i64), b: (i64, i64)) -> std::cmp::Ordering {
    a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1))
}

/// 基线 `_extractMention`：从被回复者认出一个人，顺带给一个优先级。
///
/// 合成出来的占位没有 `parent_member`，走最低优先级那一支（基线同）。
fn extract_mention(item: Option<&ReplyThreadItem>) -> (i32, Option<ReplyMember>) {
    let Some(item) = item else {
        return (NO_MENTION, None);
    };
    let Some(parent) = item.parent_member.as_ref() else {
        return (NO_MENTION, None);
    };
    let members = &item.members;

    // 1) 名字命中成员表：直接用。
    if let Some(m) = members.get(&parent.name) {
        return (1, Some(m.clone()));
    }
    // 2) 改名：原名字不在成员表，但 mid 相同，认为是同一人。
    //    基线取「第一个」，而成员表是哈希表、没有第一个可言；按昵称字典序
    //    取，至少同一份输入永远给同一个答案。
    if let Some(mid) = parent.mid {
        let mut names: Vec<&String> = members.keys().collect();
        names.sort_unstable();
        if let Some(m) = names
            .into_iter()
            .filter_map(|name| members.get(name))
            .find(|m| m.mid == mid)
        {
            return (1, Some(m.clone()));
        }
    }
    // 3) 自回复：被回复者没有 mid 属性。
    if parent.name == item.member.name {
        return (2, Some(item.member.clone()));
    }
    // 4) 可能是自回复但改了名，也可能是古早回复没有成员信息。
    (
        3,
        Some(ReplyMember {
            mid: parent.mid.unwrap_or(0),
            name: parent.name.clone(),
            ..Default::default()
        }),
    )
}

/// 把一页楼中楼接成 DFS 前序。
///
/// `data` 必须**已按 id 去重**（基线在调用点 `retainWhere` 做，那是宿主
/// 对自己那份列表的改写，没有跟着下来）。`fold_depth` 为 0 表示不折叠。
///
/// 返回的行含折叠起来看不见的那些，见 [`ReplyThreadRow::visible`]。
#[must_use]
pub fn build(
    data: &[ReplyThreadItem],
    removed: &HashMap<i64, ReplyThreadItem>,
    root_id: i64,
    fold_depth: i32,
) -> Vec<ReplyThreadRow> {
    // 基线：`if (foldDepth == 0) foldDepth = -1;`——0 档等于不折叠，而
    // depth 从 0 起算，不改的话 0 会把第一层全折起来。
    let fold_depth = if fold_depth == 0 { -1 } else { fold_depth };

    let mut b = Builder {
        data,
        removed,
        root_id,
        synth: Vec::new(),
        children: HashMap::new(),
        // 调用方保证 `data` 已去重，于是这一页的 id 就是初始的 `seen`。
        seen: data.iter().map(|i| i.id).collect(),
        suppressed: HashMap::new(),
    };

    // ── 1. 上溯 + 建桶 ──
    let mut to_synth_dialogs = OrderedIdSet::default();
    let mut to_synth_parents = OrderedIdSet::default();

    for (index, r) in data.iter().enumerate() {
        b.children
            .entry(r.parent)
            .or_default()
            .push(NodeRef::Data(index));

        // dialog：是 root 的直接子节点，不必上溯。
        let d = r.dialog;
        if r.id != d && !b.seen.contains(&d) && !to_synth_dialogs.contains(d) {
            if b.removed.contains_key(&d) {
                b.add_node(NodeRef::Removed(d));
                b.suppressed.insert(d, ReplyRowReason::Blocked);
            } else {
                to_synth_dialogs.insert(d);
            }
        }

        // parent：沿 removed 上溯保留被屏蔽祖先，否则登记待合成。
        let mut p = r.parent;
        for _ in 0..MAX_ANCESTOR_WALK {
            if p == root_id || b.seen.contains(&p) {
                break;
            }
            match b.removed.get(&p) {
                None => {
                    to_synth_parents.insert(p);
                    break;
                }
                Some(anc) => {
                    let next = anc.parent;
                    b.add_node(NodeRef::Removed(p));
                    b.suppressed.insert(p, ReplyRowReason::Blocked);
                    p = next;
                }
            }
        }
    }

    // ── 2a. 合成缺失 dialog ──
    // 有 kids：必为真实/被屏蔽的节点，直接实体化，ctime 已经定得下来。
    // 无 kids：先记进 seen 与 deferred，等 2b 把子节点挂上来再说。
    let mut deferred: Vec<i64> = Vec::new();
    for p in to_synth_dialogs.drain() {
        // 交叉 id：已按 dialog 语义处理，2b 不再合成。
        to_synth_parents.remove(p);
        if b.seen.contains(&p) {
            continue;
        }
        match b.children.get(&p) {
            Some(kids) => {
                let node = b.synth_reply(p, &kids.clone(), false);
                b.push_synth(node);
            }
            None => {
                b.seen.insert(p);
                deferred.push(p);
            }
        }
    }

    // ── 2b. 合成缺失父 ──
    // seen 此时已含全部 dialog（含 deferred），anchor 的判定没有顺序依赖。
    for p in to_synth_parents.drain() {
        if let Some(kids) = b.children.get(&p) {
            let node = b.synth_reply(p, &kids.clone(), true);
            b.push_synth(node);
        }
        // else：幽灵节点，基线在这里 assert(false) 后什么也不做。
    }

    // ── 2c. 实体化 deferred：2b 已经把子节点挂上来了 ──
    for p in deferred {
        if let Some(kids) = b.children.get(&p) {
            let node = b.synth_reply(p, &kids.clone(), false);
            b.push_synth(node);
        }
    }

    // ── 3. DFS 前序 ──
    let mut rows: Vec<ReplyThreadRow> = Vec::new();
    let mut visited: HashSet<i64> = HashSet::new();

    if let Some(root_kids) = b.children.get(&root_id) {
        let mut kids = root_kids.clone();
        sort_kids(&b, &mut kids);
        for kid in kids {
            walk(&b, kid, 0, None, true, fold_depth, &mut rows, &mut visited);
        }
    }

    rows
}

fn sort_kids(b: &Builder<'_>, kids: &mut [NodeRef]) {
    kids.sort_by(|x, y| {
        reply_compare((b.ctime_of(*x), b.id_of(*x)), (b.ctime_of(*y), b.id_of(*y)))
    });
}

/// 基线 `walk`：前序拍平，返回后代数量。
#[allow(clippy::too_many_arguments)]
fn walk(
    b: &Builder<'_>,
    node: NodeRef,
    depth: i32,
    parent: Option<usize>,
    visible: bool,
    fold_depth: i32,
    rows: &mut Vec<ReplyThreadRow>,
    visited: &mut HashSet<i64>,
) -> i32 {
    let id = b.id_of(node);
    if !visited.insert(id) {
        // 基线在这里 assert(false) 后返回 -1：有环的话整棵子树不要了。
        return -1;
    }

    let row_index = rows.len();
    rows.push(ReplyThreadRow {
        source: match node {
            NodeRef::Data(_) => ReplyRowSource::Data,
            NodeRef::Removed(_) => ReplyRowSource::Removed,
            NodeRef::Synth(_) => ReplyRowSource::Synthetic,
        },
        index: match node {
            NodeRef::Data(index) => Some(index),
            _ => None,
        },
        id,
        placeholder: match node {
            NodeRef::Synth(index) => Some(placeholder_of(&b.synth[index], b.root_id)),
            _ => None,
        },
        depth,
        parent,
        reason: b
            .suppressed
            .get(&id)
            .copied()
            .unwrap_or(ReplyRowReason::None),
        collapsed: 0,
        visible,
    });

    let mut count = 0;
    if let Some(kids) = b.children.get(&id) {
        let mut kids = kids.clone();
        sort_kids(b, &mut kids);
        let auto_fold = depth == fold_depth;
        let child_visible = visible && !auto_fold;
        for kid in kids {
            if kid == node {
                // 自引用。基线 assert(false) 后跳过。
                continue;
            }
            count += walk(
                b,
                kid,
                depth + 1,
                Some(row_index),
                child_visible,
                fold_depth,
                rows,
                visited,
            ) + 1;
        }
        if auto_fold {
            rows[row_index].collapsed = count;
        }
    }
    count
}

/// 基线 `_placeholder`：只要身份、一个用于排序的 `ctime`，以及一个用来
/// 显示「回复了谁」的作者。
fn placeholder_of(node: &SynthNode, root_id: i64) -> ReplyEntity {
    ReplyEntity {
        id: node.id,
        mid: node.member.as_ref().map_or(0, |m| m.mid),
        root: root_id,
        parent: node.parent,
        dialog: node.dialog,
        ctime: node.ctime,
        member: node.member.clone().unwrap_or_default(),
        ..Default::default()
    }
}

/// 按登记顺序迭代的 id 集合。
///
/// 基线用的是 Dart `Set<int>`，也就是 LinkedHashSet——顺序会影响 anchor
/// 落在哪一条子评论上，所以不能换成哈希序。
#[derive(Debug, Default)]
struct OrderedIdSet {
    order: Vec<i64>,
    set: HashSet<i64>,
}

impl OrderedIdSet {
    fn contains(&self, id: i64) -> bool {
        self.set.contains(&id)
    }

    fn insert(&mut self, id: i64) {
        if self.set.insert(id) {
            self.order.push(id);
        }
    }

    fn remove(&mut self, id: i64) {
        if self.set.remove(&id) {
            self.order.retain(|i| *i != id);
        }
    }

    fn drain(&mut self) -> Vec<i64> {
        self.set.clear();
        std::mem::take(&mut self.order)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    /// 一条自成一轮对话的评论（`dialog` 就是它自己，也就是 root 的直接
    /// 子节点）。
    fn item(id: i64, parent: i64, ctime: i64) -> ReplyThreadItem {
        in_dialog(id, parent, id, ctime)
    }

    /// 挂在某一轮对话下的评论。`dialog` 指向那一轮的头一条。
    fn in_dialog(id: i64, parent: i64, dialog: i64, ctime: i64) -> ReplyThreadItem {
        ReplyThreadItem {
            id,
            parent,
            dialog,
            ctime,
            ..Default::default()
        }
    }

    fn shape(rows: &[ReplyThreadRow]) -> Vec<(i64, i32, bool)> {
        rows.iter().map(|r| (r.id, r.depth, r.visible)).collect()
    }

    #[test]
    fn children_are_sorted_by_ctime_then_id() {
        let data = vec![item(3, 1, 200), item(2, 1, 100), item(4, 1, 100)];
        let rows = build(&data, &HashMap::new(), 1, 0);
        assert_eq!(shape(&rows), vec![(2, 0, true), (4, 0, true), (3, 0, true)]);
    }

    #[test]
    fn missing_parent_is_synthesized_with_the_smallest_child_ctime() {
        // 9 没有随页下发，也不在 removed 里 → 合成一条占位。
        let data = vec![in_dialog(5, 9, 9, 300), in_dialog(6, 9, 9, 200)];
        let rows = build(&data, &HashMap::new(), 1, 0);

        assert_eq!(rows[0].source, ReplyRowSource::Synthetic);
        assert_eq!(rows[0].reason, ReplyRowReason::Deleted);
        let placeholder = rows[0].placeholder.as_ref().expect("占位要带实体");
        assert_eq!(placeholder.id, 9);
        assert_eq!(placeholder.parent, 1, "dialog 是 root 的直接子节点");
        assert_eq!(placeholder.ctime, 200, "占位的 ctime 取子评论最小值");
        assert_eq!(shape(&rows), vec![(9, 0, true), (6, 1, true), (5, 1, true)]);
    }

    #[test]
    fn blocked_ancestor_is_kept_from_removed() {
        let mut removed = HashMap::new();
        removed.insert(7, item(7, 1, 50));
        let data = vec![in_dialog(8, 7, 7, 100)];
        let rows = build(&data, &removed, 1, 0);

        assert_eq!(rows[0].source, ReplyRowSource::Removed);
        assert_eq!(rows[0].reason, ReplyRowReason::Blocked);
        assert_eq!(rows[0].id, 7);
        assert_eq!(rows[1].id, 8);
    }

    #[test]
    fn fold_depth_hides_the_subtree_and_counts_it() {
        //  10 ─ 11 ─ 12
        let data = vec![
            item(10, 1, 100),
            in_dialog(11, 10, 10, 200),
            in_dialog(12, 11, 10, 300),
        ];
        let rows = build(&data, &HashMap::new(), 1, 1);

        assert_eq!(
            shape(&rows),
            vec![(10, 0, true), (11, 1, true), (12, 2, false)],
            "foldDepth = 1 时第 1 层的子树收起来"
        );
        assert_eq!(rows[1].collapsed, 1);
        assert_eq!(rows[0].collapsed, 0);
    }

    #[test]
    fn fold_depth_zero_never_folds() {
        let data = vec![item(10, 1, 100), in_dialog(11, 10, 10, 200)];
        let rows = build(&data, &HashMap::new(), 1, 0);
        assert!(rows.iter().all(|r| r.visible && r.collapsed == 0));
    }

    #[test]
    fn a_cycle_in_removed_orphans_the_subtree_instead_of_hanging() {
        // removed 里 20 ↔ 21 互为父节点，谁也到不了 root。基线在这里
        // `assert(false)`（debug 崩、release 继续），结果同样是这棵子树
        // 接不到 root 上、一行都画不出来。这条守的是**不死循环**。
        let mut removed = HashMap::new();
        removed.insert(20, in_dialog(20, 21, 20, 10));
        removed.insert(21, in_dialog(21, 20, 20, 10));
        let data = vec![in_dialog(22, 20, 20, 100)];

        assert!(build(&data, &removed, 1, 0).is_empty());
    }

    #[test]
    fn an_over_long_ancestor_chain_stops_at_the_cap() {
        // 上溯上限 512：链比它还长时，上半截永远接不到 root。宁可丢一棵
        // 子树，也不在一条坏数据上转到天荒地老——基线就是这么写的。
        let mut removed = HashMap::new();
        for id in 2..=601 {
            removed.insert(id, in_dialog(id, id + 1, 2, 10));
        }
        removed.insert(602, in_dialog(602, 1, 2, 10));
        let data = vec![in_dialog(700, 2, 2, 100)];

        assert!(build(&data, &removed, 1, 0).is_empty());
    }

    #[test]
    fn mention_prefers_the_member_table() {
        let mut kid = in_dialog(31, 30, 30, 100);
        kid.parent_member = Some(ReplyParentMember {
            mid: Some(42),
            name: "甲".to_owned(),
        });
        kid.members.insert(
            "甲".to_owned(),
            ReplyMember {
                mid: 42,
                name: "甲".to_owned(),
                face: "https://i0.example/face.png".to_owned(),
                ..Default::default()
            },
        );

        let rows = build(&[kid], &HashMap::new(), 1, 0);
        let placeholder = rows[0].placeholder.as_ref().expect("占位要带实体");
        assert_eq!(placeholder.member.mid, 42);
        assert_eq!(placeholder.member.face, "https://i0.example/face.png");
    }

    #[test]
    fn mention_recognizes_a_renamed_member_by_mid() {
        let mut kid = in_dialog(31, 30, 30, 100);
        kid.parent_member = Some(ReplyParentMember {
            mid: Some(42),
            name: "旧名字".to_owned(),
        });
        kid.members.insert(
            "新名字".to_owned(),
            ReplyMember {
                mid: 42,
                name: "新名字".to_owned(),
                ..Default::default()
            },
        );

        let rows = build(&[kid], &HashMap::new(), 1, 0);
        let placeholder = rows[0].placeholder.as_ref().expect("占位要带实体");
        assert_eq!(placeholder.member.name, "新名字");
    }

    #[test]
    fn mention_falls_back_to_a_name_only_member() {
        let mut kid = in_dialog(31, 30, 30, 100);
        kid.parent_member = Some(ReplyParentMember {
            mid: None,
            name: "查无此人".to_owned(),
        });

        let rows = build(&[kid], &HashMap::new(), 1, 0);
        let placeholder = rows[0].placeholder.as_ref().expect("占位要带实体");
        assert_eq!(placeholder.member.name, "查无此人");
        assert_eq!(placeholder.member.mid, 0);
    }
}
