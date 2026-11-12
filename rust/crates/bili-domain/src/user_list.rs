//! 「一页用户」的领域实体（S15 / issue #66）：黑名单、关注列表、粉丝列表。
//!
//! 对应基线 `lib/models_new/blacklist/` 与 `lib/models_new/follow/`。
//!
//! 模块叫 `user_list` 而不是 `blacklist`，就是为了等关注、粉丝、同好这几个
//! 同形状的列表搬进来。黑名单先到（#66 第一批），关注与粉丝随后（#66 收尾）。
//!
//! 同好列表（`/x/relation/same/followings`）、已关注的 up（`followed_upper`）
//! 与关注分组（`/x/relation/tag`）还在 Dart（S6 #56 / S7 #57）。它们的响应
//! 形状也是 [`FollowList`]，迁的时候直接复用，不要再造一个。

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

/// 一页关注或粉丝。
///
/// 关注列表与粉丝列表是**同一个响应形状**，服务端只换了路径；基线也用
/// 同一个 `FollowData` 承载两边（`lib/http/follow.dart`、`lib/http/fan.dart`）。
/// 这里不按路径拆两个实体，拆了只会得到两个字段完全相同的结构。
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
pub struct FollowList {
    pub list: Vec<FollowUser>,
    /// 粉丝/关注总数。分组列表（`/x/relation/tag`）那条路径不给，为 `None`。
    pub total: Option<i64>,
}

/// 列表里的一个人。
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
pub struct FollowUser {
    /// 缺失时为 0，与基线 `FollowItemModel.fromJson` 一致——界面拿它拼
    /// 个人页路由，null 和 0 在那里是一回事。
    pub mid: i64,
    /// 关注关系位掩码。基线只用它判 `!= -1`（是否已关注）。
    pub attribute: Option<i64>,
    pub uname: Option<String>,
    pub face: Option<String>,
    pub sign: Option<String>,
    pub official_verify: Option<OfficialVerify>,
}

/// 官方认证。`kind` 对应协议里的 `type`（Rust 关键字，改了名）。
///
/// 定义在这里而不是各自的模块里：搜索结果的用户卡片（`crate::search`）读的
/// 是同一个 `official_verify` 子树，`crate::search::OfficialVerify` 现在是
/// 本类型的再导出。接口里凡是「一个用户」的地方都会带它，再抄一遍只会多
/// 一个要同步维护的点。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct OfficialVerify {
    pub kind: Option<i64>,
    pub desc: Option<String>,
}
