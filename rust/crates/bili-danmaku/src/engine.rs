//! 引擎：标记自己 → 合并 → 过滤 → 分桶。
//!
//! 基线是 `lib/pages/danmaku/controller.dart` 的 `handleDanmaku`，**顺序
//! 不能错**（issue #42）：
//!
//! 1. 登录态下先按 `midHash` 标记 `isSelf`
//! 2. 自己发的弹幕既不参与合并也不参与过滤
//! 3. 合并（开关开启时）按 `content` 去重，首条留下并累加 `count`
//! 4. 过滤（关键词 / 正则 / UID）
//! 5. 按 `progress ~/ 100` 分桶
//!
//! 顺序为什么要紧：合并在过滤之前，所以**被过滤掉的内容也会占住合并
//! 表的位置**——首条命中规则被丢弃后，后续同内容的弹幕撞上合并表直接
//! `continue`，同样不会出现。反过来写（先过滤再合并）结果相同，但
//! `count` 会不同：那样第二条同内容弹幕会成为「首条」，计数从头开始。
//!
//! `weight` 阈值过滤**不在这里**：基线在渲染时按用户设置比较
//! （`lib/pages/danmaku/view.dart`），同一份分桶在用户改阈值后要能立刻
//! 生效，不能在入桶时就把低权重的弹幕丢掉。
//!
//! 字段 `count` / `is_self` 是**引擎的产物**，不是解码的产物：服务端从
//! 不下发它们（protobuf 字段 100/101 是上游手工加进生成代码的客户端
//! 字段，见 `bili_domain::danmaku` 的模块文档）。

use std::collections::BTreeMap;
use std::collections::HashMap;

use bili_domain::danmaku::DanmakuElem;
use serde::Serialize;

use crate::rule::FilterRules;

/// 分桶粒度：100 毫秒。基线 `progress ~/ 100`，渲染侧也按整百毫秒取桶
/// （`currentPosition -= currentPosition % 100`）。
pub const BUCKET_LENGTH_MS: i32 = 100;

/// 某个播放进度（毫秒）落在第几个桶。
///
/// 截断方向与 Dart 的 `~/` 一致（向零取整）。负的 `progress` 不会从
/// 服务端来，但 fixture 和将来的本地弹幕可能造出来，截断方向一致就不会
/// 在边界上分岔。
#[must_use]
pub const fn bucket_index(progress_ms: i32) -> i32 {
    progress_ms / BUCKET_LENGTH_MS
}

/// 引擎处理过的一条弹幕：服务端字段 + 客户端字段。
///
/// 序列化成**扁平**对象，与基线规范化（`lib/services/rust/danmaku_normalize.dart`
/// 的 `normalizeProcessedElem`）逐键对应，golden 才能直接比对。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProcessedElem {
    #[serde(flatten)]
    pub elem: DanmakuElem,
    /// 合并后的重复条数。未参与合并时为 0——与基线一致：渲染侧按
    /// `count > 1` 才显示角标，0 和 1 的表现相同。
    pub count: i32,
    /// 是否是当前登录账号自己发的。未登录时恒为 false。
    pub is_self: bool,
}

/// 引擎参数。全部来自用户设置，一次会话内不变。
#[derive(Debug, Clone, Default)]
pub struct EngineOptions {
    /// 未登录时不做 `isSelf` 标记（基线 `_isLogin` 为假则整段跳过）。
    pub is_login: bool,
    /// 当前账号 mid 的 crc32 十六进制串，与弹幕的 `mid_hash` 同源。
    pub self_mid_hash: String,
    /// 合并同内容弹幕（基线 `Pref.mergeDanmaku`）。
    pub merge: bool,
    pub rules: FilterRules,
}

/// 按 100ms 分桶的弹幕。跨分段累积，与基线的 `_dmSegMap` 对应。
///
/// 用 `BTreeMap` 而非 `HashMap`：取桶是按播放进度的单点查询，两者都
/// 够快；但**序列化顺序**只有前者是确定的，golden 比对不该依赖哈希序。
#[derive(Debug, Clone, Default, Serialize)]
pub struct Buckets(BTreeMap<i32, Vec<ProcessedElem>>);

impl Buckets {
    /// 取某个播放进度所在的桶。没有弹幕时返回 `None`，与基线
    /// `_dmSegMap[progress ~/ 100]` 取不到时的行为一致。
    #[must_use]
    pub fn at(&self, progress_ms: i32) -> Option<&[ProcessedElem]> {
        self.0.get(&bucket_index(progress_ms)).map(Vec::as_slice)
    }

    /// 非空桶的个数。
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// 已入桶的弹幕总条数（过滤掉的不计）。
    #[must_use]
    pub fn elem_count(&self) -> usize {
        self.0.values().map(Vec::len).sum()
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }
}

/// 把一个分段的弹幕并入 `buckets`。
///
/// 合并表是**每次调用**新建的，不跨分段——基线的 `uniques` 是
/// `handleDanmaku` 的局部变量，一个分段 6 分钟，跨分段的同内容弹幕不
/// 合并。这一点必须保持：否则一个视频从头看到尾，重复弹幕会越合越多，
/// 与基线的显示不同。
pub fn ingest(options: &EngineOptions, buckets: &mut Buckets, elems: Vec<DanmakuElem>) {
    if elems.is_empty() {
        return;
    }
    let should_filter = !options.rules.is_empty();
    // content -> 首条所在的位置；`None` 表示首条被过滤掉了，没有落桶。
    // 基线此处存的是对象引用，`count++` 直接改那个对象；我们存位置，
    // 效果相同，而且不需要内部可变性。
    let mut uniques: HashMap<String, Option<(i32, usize)>> = HashMap::new();

    for elem in elems {
        let is_self = options.is_login && elem.mid_hash == options.self_mid_hash;
        let mut count = 0;

        if !is_self {
            if options.merge {
                match uniques.get(&elem.content) {
                    // 已有首条：累加计数，这一条不单独显示。
                    Some(&Some((bucket, index))) => {
                        if let Some(first) =
                            buckets.0.get_mut(&bucket).and_then(|b| b.get_mut(index))
                        {
                            first.count += 1;
                        }
                        continue;
                    }
                    // 首条被过滤掉了：同内容的后续一并不显示。
                    Some(&None) => continue,
                    None => count = 1,
                }
            }

            if should_filter && options.rules.removes(&elem) {
                if options.merge {
                    uniques.insert(elem.content, None);
                }
                continue;
            }
        }

        let bucket = bucket_index(elem.progress);
        let slot = buckets.0.entry(bucket).or_default();
        if options.merge && !is_self {
            uniques.insert(elem.content.clone(), Some((bucket, slot.len())));
        }
        slot.push(ProcessedElem {
            elem,
            count,
            is_self,
        });
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::{Buckets, EngineOptions, bucket_index, ingest};
    use crate::rule::FilterRules;
    use bili_domain::danmaku::DanmakuElem;

    fn elem(progress: i32, content: &str, mid_hash: &str) -> DanmakuElem {
        DanmakuElem {
            progress,
            content: content.to_owned(),
            mid_hash: mid_hash.to_owned(),
            ..DanmakuElem::default()
        }
    }

    fn contents(buckets: &Buckets, progress_ms: i32) -> Vec<String> {
        buckets
            .at(progress_ms)
            .unwrap_or_default()
            .iter()
            .map(|p| p.elem.content.clone())
            .collect()
    }

    fn run(options: &EngineOptions, elems: Vec<DanmakuElem>) -> Buckets {
        let mut buckets = Buckets::default();
        ingest(options, &mut buckets, elems);
        buckets
    }

    #[test]
    fn bucket_boundaries_match_the_baseline() {
        assert_eq!(bucket_index(0), 0);
        assert_eq!(bucket_index(99), 0);
        assert_eq!(bucket_index(100), 1, "整百毫秒属于下一个桶");
        assert_eq!(bucket_index(61_050), 610);
        // Dart 的 ~/ 向零取整，不是向下取整。
        assert_eq!(bucket_index(-150), -1);
    }

    #[test]
    fn elements_keep_their_order_inside_a_bucket() {
        let buckets = run(
            &EngineOptions::default(),
            vec![
                elem(1_000, "一", "a"),
                elem(1_050, "二", "b"),
                elem(1_099, "三", "c"),
            ],
        );
        assert_eq!(contents(&buckets, 1_000), ["一", "二", "三"]);
    }

    #[test]
    fn merge_keeps_the_first_and_counts_the_rest() {
        let options = EngineOptions {
            merge: true,
            ..EngineOptions::default()
        };
        let buckets = run(
            &options,
            vec![
                elem(1_000, "草", "a"),
                elem(2_000, "草", "b"),
                elem(3_000, "草", "c"),
                elem(2_000, "别的", "d"),
            ],
        );
        // 后两条不单独入桶，计数记在首条上。
        assert!(buckets.at(2_000).is_some_and(|b| b.len() == 1));
        assert_eq!(contents(&buckets, 2_000), ["别的"]);
        let first = &buckets.at(1_000).expect("首条在桶里")[0];
        assert_eq!(first.elem.content, "草");
        assert_eq!(first.count, 3);
    }

    #[test]
    fn merge_off_keeps_every_duplicate_with_count_zero() {
        let buckets = run(
            &EngineOptions::default(),
            vec![elem(1_000, "草", "a"), elem(1_010, "草", "b")],
        );
        let bucket = buckets.at(1_000).expect("两条都在桶里");
        assert_eq!(bucket.len(), 2);
        assert!(bucket.iter().all(|p| p.count == 0), "不合并时计数恒为 0");
    }

    #[test]
    fn filtered_content_also_swallows_its_duplicates() {
        // 合并在过滤之前：首条被过滤后仍占着合并表，后续同内容一并消失。
        let options = EngineOptions {
            merge: true,
            rules: FilterRules::compile(vec!["广告".to_owned()], &[], []).expect("可编译"),
            ..EngineOptions::default()
        };
        let buckets = run(
            &options,
            vec![
                elem(1_000, "广告内容", "a"),
                elem(2_000, "广告内容", "b"),
                elem(2_000, "正常弹幕", "c"),
            ],
        );
        assert_eq!(contents(&buckets, 1_000), Vec::<String>::new());
        assert_eq!(contents(&buckets, 2_000), ["正常弹幕"]);
    }

    #[test]
    fn self_danmaku_is_exempt_from_merge_and_filter() {
        let options = EngineOptions {
            is_login: true,
            self_mid_hash: "me".to_owned(),
            merge: true,
            rules: FilterRules::compile(vec!["广告".to_owned()], &[], []).expect("可编译"),
        };
        let buckets = run(
            &options,
            vec![
                elem(1_000, "广告内容", "me"),
                elem(1_000, "广告内容", "me"),
                elem(1_000, "广告内容", "other"),
            ],
        );
        let bucket = buckets.at(1_000).expect("自己的两条都在");
        assert_eq!(bucket.len(), 2, "自己发的不合并、不过滤");
        assert!(bucket.iter().all(|p| p.is_self));
        assert!(bucket.iter().all(|p| p.count == 0));
    }

    #[test]
    fn self_marking_requires_login() {
        let options = EngineOptions {
            is_login: false,
            self_mid_hash: "me".to_owned(),
            merge: true,
            rules: FilterRules::compile(vec!["广告".to_owned()], &[], []).expect("可编译"),
        };
        // 未登录：midHash 相同也不算自己，照样过滤。
        let buckets = run(&options, vec![elem(1_000, "广告内容", "me")]);
        assert!(buckets.is_empty());
    }

    #[test]
    fn uid_rule_matches_the_mid_hash() {
        let options = EngineOptions {
            rules: FilterRules::compile(vec![], &[], ["blocked".to_owned()]).expect("可编译"),
            ..EngineOptions::default()
        };
        let buckets = run(
            &options,
            vec![elem(1_000, "内容", "blocked"), elem(1_000, "内容", "ok")],
        );
        assert_eq!(buckets.elem_count(), 1);
    }

    #[test]
    fn weight_is_not_filtered_here() {
        // 阈值过滤属于渲染侧，低权重弹幕必须照常入桶。
        let buckets = run(
            &EngineOptions::default(),
            vec![DanmakuElem {
                progress: 1_000,
                weight: 1,
                content: "低权重".to_owned(),
                ..DanmakuElem::default()
            }],
        );
        assert_eq!(buckets.elem_count(), 1);
    }

    #[test]
    fn merge_table_does_not_cross_calls() {
        // 基线的 uniques 是 handleDanmaku 的局部变量：跨分段不合并。
        let options = EngineOptions {
            merge: true,
            ..EngineOptions::default()
        };
        let mut buckets = Buckets::default();
        ingest(&options, &mut buckets, vec![elem(1_000, "草", "a")]);
        ingest(&options, &mut buckets, vec![elem(400_000, "草", "b")]);
        assert_eq!(buckets.elem_count(), 2);
        assert_eq!(buckets.at(1_000).expect("首段")[0].count, 1);
    }

    #[test]
    fn empty_input_changes_nothing() {
        let mut buckets = Buckets::default();
        ingest(&EngineOptions::default(), &mut buckets, vec![]);
        assert!(buckets.is_empty());
    }
}
