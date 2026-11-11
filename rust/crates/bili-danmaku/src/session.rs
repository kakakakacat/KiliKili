//! 弹幕会话：一个 cid 的分段缓存与分桶结果。
//!
//! 对应基线 `PlDanmakuController` 的 `_dmSegMap` + `_requestedSeg`，
//! 但**只保留已处理的结果**，不保留原始分段：原始字节解完就没有用处，
//! 一个热门视频的一段弹幕有数千条，留着只是占内存。
//!
//! ## 所有权与失效（issue #42 / ADR-003）
//!
//! 一个会话 = 一个 [`SessionId`] = 基线的一个 `PlDanmakuController`
//! 实例 = 一个 cid（切 P 换 cid，也就换会话）。失效的规则很简单：
//! **会话由创建者关闭，关闭后它的 id 不再复用**。切 P、切视频、页面
//! 销毁都走同一条路——关掉旧会话，建一个新的。
//!
//! 这与 ADR-003 的 `session_id` + `generation` 是同一套语义的退化形式：
//! 弹幕会话里没有「同一会话内要作废在途请求」的情形（seek 不会让已解出
//! 的弹幕失效，它只是换个桶取），因此不需要在会话内再分代际。在途的
//! 分段响应如果在关闭之后才回来，它持有的 id 已经找不到会话，自然被
//! 丢弃——这正是代际要达到的效果。

use std::collections::BTreeSet;

use bili_domain::danmaku::DanmakuSegment;
use bili_domain::session::SessionId;

use crate::engine::{Buckets, EngineOptions, ProcessedElem, ingest};

/// 一次 `ingest_segment` 的结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SegmentOutcome {
    /// 该分段此前已并入过，本次什么都没做。
    ///
    /// 基线用 `_requestedSeg` 在**请求前**去重；这里是第二道闸门，
    /// 防止重复并入导致同一条弹幕出现两次。两道闸门不冲突：前者省掉
    /// 请求，后者保证结果正确。
    pub already_loaded: bool,
    /// 本次分段解码出的弹幕条数（过滤前）。
    pub decoded: usize,
    /// 并入后这个会话已入桶的总条数（过滤后）。
    pub total: usize,
    /// 服务端报告的分段状态。基线在 `state == 1` 时提示「弹幕已关闭」。
    pub state: i32,
}

/// 一个 cid 的弹幕会话。
#[derive(Debug)]
pub struct DanmakuSession {
    id: SessionId,
    options: EngineOptions,
    buckets: Buckets,
    loaded: BTreeSet<i64>,
}

impl DanmakuSession {
    #[must_use]
    pub fn new(id: SessionId, options: EngineOptions) -> Self {
        Self {
            id,
            options,
            buckets: Buckets::default(),
            loaded: BTreeSet::new(),
        }
    }

    #[must_use]
    pub const fn id(&self) -> SessionId {
        self.id
    }

    /// 该分段是否已并入。
    #[must_use]
    pub fn is_loaded(&self, segment_index: i64) -> bool {
        self.loaded.contains(&segment_index)
    }

    /// 并入一个已解码的分段。重复并入同一分段是**无操作**。
    pub fn ingest_segment(
        &mut self,
        segment_index: i64,
        segment: DanmakuSegment,
    ) -> SegmentOutcome {
        if !self.loaded.insert(segment_index) {
            return SegmentOutcome {
                already_loaded: true,
                decoded: 0,
                total: self.buckets.elem_count(),
                state: 0,
            };
        }
        let decoded = segment.elems.len();
        let state = segment.state;
        ingest(&self.options, &mut self.buckets, segment.elems);
        SegmentOutcome {
            already_loaded: false,
            decoded,
            total: self.buckets.elem_count(),
            state,
        }
    }

    /// 取某个播放进度所在的 100ms 桶。
    ///
    /// 这是**每 100ms 调用一次**的热路径（渲染侧按整百毫秒推进），
    /// 因此它只做一次 map 查询，不做任何过滤或排序——该做的在并入时
    /// 已经做完了。
    #[must_use]
    pub fn bucket(&self, progress_ms: i32) -> Option<&[ProcessedElem]> {
        self.buckets.at(progress_ms)
    }

    /// 已入桶的弹幕总条数。
    #[must_use]
    pub fn elem_count(&self) -> usize {
        self.buckets.elem_count()
    }

    /// 已并入的分段数。
    #[must_use]
    pub fn segment_count(&self) -> usize {
        self.loaded.len()
    }

    #[must_use]
    pub const fn options(&self) -> &EngineOptions {
        &self.options
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::DanmakuSession;
    use crate::engine::EngineOptions;
    use bili_domain::danmaku::{DanmakuElem, DanmakuSegment};
    use bili_domain::session::SessionId;

    fn segment(elems: &[(i32, &str)], state: i32) -> DanmakuSegment {
        DanmakuSegment {
            elems: elems
                .iter()
                .map(|(progress, content)| DanmakuElem {
                    progress: *progress,
                    content: (*content).to_owned(),
                    ..DanmakuElem::default()
                })
                .collect(),
            state,
        }
    }

    fn session() -> DanmakuSession {
        DanmakuSession::new(SessionId(1), EngineOptions::default())
    }

    #[test]
    fn a_segment_is_ingested_once() {
        let mut s = session();
        let first = s.ingest_segment(0, segment(&[(1_000, "一")], 0));
        assert!(!first.already_loaded);
        assert_eq!(first.decoded, 1);
        assert_eq!(first.total, 1);

        let again = s.ingest_segment(0, segment(&[(1_000, "一")], 0));
        assert!(again.already_loaded);
        assert_eq!(s.elem_count(), 1, "重复并入不得让弹幕出现两次");
    }

    #[test]
    fn segments_accumulate() {
        let mut s = session();
        s.ingest_segment(0, segment(&[(1_000, "一")], 0));
        s.ingest_segment(1, segment(&[(400_000, "二")], 0));
        assert_eq!(s.segment_count(), 2);
        assert_eq!(s.elem_count(), 2);
        assert!(s.is_loaded(1));
        assert!(!s.is_loaded(2));
    }

    #[test]
    fn closed_state_is_reported_back() {
        let mut s = session();
        let outcome = s.ingest_segment(0, segment(&[], 1));
        assert_eq!(outcome.state, 1);
    }

    #[test]
    fn bucket_lookup_is_by_progress() {
        let mut s = session();
        s.ingest_segment(0, segment(&[(61_050, "前方高能")], 0));
        assert_eq!(
            s.bucket(61_000).expect("桶存在")[0].elem.content,
            "前方高能"
        );
        assert!(s.bucket(61_100).is_none());
    }
}
