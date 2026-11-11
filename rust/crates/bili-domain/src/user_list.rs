//! 黑名单列表的领域实体（S15 / issue #66）。
//!
//! 对应基线 `lib/models_new/blacklist/`。
//!
//! 模块叫 `user_list` 而不是 `blacklist`：同样「一页用户」的形状在关注、
//! 粉丝、同好里还会再出现（S6 #56 / S7 #57），届时这里是它们的去处。本
//! 切片只放黑名单一个生产者——关注与粉丝的 `FollowItemModel` 另有四个
//! 未迁的生产者，按 ADR-011 决策六不在这里动。

use serde::Serialize;

/// 一页黑名单。
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
pub struct BlackList {
    pub list: Vec<BlackListItem>,
    pub total: Option<i64>,
}

/// 黑名单里的一个人。
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
pub struct BlackListItem {
    pub mid: Option<i64>,
    /// 拉黑时间，Unix 秒。
    pub mtime: Option<i64>,
    pub uname: Option<String>,
    pub face: Option<String>,
}
