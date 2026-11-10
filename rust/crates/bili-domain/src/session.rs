//! 请求代际与取消。
//!
//! RFC-0001 §2.2：事件须携带 `request_id`/`generation`，在路由切换、
//! Seek、搜索输入变更时支持取消并**丢弃过期结果**。
//!
//! 基线（Dart）完全没有这套机制：过期事件靠 `_playerCount == 0` 这类
//! 零散判断兜底（见 BASELINE.md §5）。典型症状是快速切换视频时，先发
//! 出的请求后返回，把新视频的状态覆盖掉。
//!
//! 这里把代际做成类型而非约定：[`Generation`] 单调递增，
//! [`GenerationGate`] 负责判定一个结果是否已过期。

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// 会话标识。一次播放、一个页面或一条搜索序列对应一个会话。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SessionId(pub u64);

/// 单调递增的代际号。每次「作废先前请求」的操作使其 +1。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Generation(pub u64);

/// 代际闸门：发起请求前取号，结果回来时判断是否仍然有效。
///
/// 可跨线程共享（`Arc` 内部可变），因为取消通常发生在 UI 线程，
/// 而结果到达于后台任务。
#[derive(Debug, Clone)]
pub struct GenerationGate {
    session: SessionId,
    current: Arc<AtomicU64>,
}

impl GenerationGate {
    #[must_use]
    pub fn new(session: SessionId) -> Self {
        Self {
            session,
            current: Arc::new(AtomicU64::new(0)),
        }
    }

    #[must_use]
    pub const fn session(&self) -> SessionId {
        self.session
    }

    /// 当前代际。
    #[must_use]
    pub fn current(&self) -> Generation {
        Generation(self.current.load(Ordering::Acquire))
    }

    /// 作废此前所有在途请求，返回新代际。
    ///
    /// 在路由切换、Seek、搜索输入变更时调用。
    pub fn bump(&self) -> Generation {
        Generation(self.current.fetch_add(1, Ordering::AcqRel) + 1)
    }

    /// 某代际的结果是否仍应被采纳。
    ///
    /// 严格相等而非 `>=`：代际落后说明已被作废，代际超前说明调用方
    /// 用错了闸门，两者都不该被采纳。
    #[must_use]
    pub fn is_current(&self, generation: Generation) -> bool {
        self.current() == generation
    }
}

/// 携带代际的结果。消费前必须经 [`Stamped::accept`] 校验。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stamped<T> {
    pub session: SessionId,
    pub generation: Generation,
    pub value: T,
}

impl<T> Stamped<T> {
    pub const fn new(session: SessionId, generation: Generation, value: T) -> Self {
        Self {
            session,
            generation,
            value,
        }
    }

    /// 仍然有效则返回值，已过期则返回 `None`。
    ///
    /// 会话不匹配同样视为过期——跨会话的结果绝不应被采纳。
    #[must_use]
    pub fn accept(self, gate: &GenerationGate) -> Option<T> {
        if self.session == gate.session() && gate.is_current(self.generation) {
            Some(self.value)
        } else {
            None
        }
    }
}
