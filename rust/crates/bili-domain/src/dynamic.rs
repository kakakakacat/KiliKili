//! 动态业务实体。协议消息只在 `bili-api` 内部出现。

use serde::Serialize;

/// 首页动态红点的有效更新数。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct UnreadCount {
    pub count: i64,
}
