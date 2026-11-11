//! SponsorBlock（片段跳过）实体。
//!
//! 与其余实体的不同之处：这是**第三方 API**
//! （<https://github.com/hanydd/BilibiliSponsorBlock/wiki/API>），不走
//! bilibili 的 `{"code":0,"data":…}` 信封，成败只看 HTTP 状态码。因此
//! 它的错误分类在 `bili-api` 侧单独写了一份，没有复用 `envelope`。
//!
//! ## 为什么 `category` 是字符串而不是枚举
//!
//! 服务端随时可以加新分类（上游 wiki 的分类表这一年就加过两次）。收成
//! Rust 枚举的话，新分类会变成一个解析错误——整条片段列表拿不到，用户
//! 看到的是"跳过功能坏了"。保持字符串，未知分类由 UI 自行忽略，这与基线
//! `SegmentType.values.byName` 的失败面一致，但失败范围只限于那一条片段。
//!
//! 这条结论对后续切片同样适用：**协议里取值开放的字段，不要在 Rust 收成
//! 枚举**，除非服务端承诺封闭。

use serde::Serialize;

/// 一个可跳过片段。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SkipSegment {
    pub uuid: String,
    /// 分类名（`sponsor` / `selfpromo` / `interaction` …）。见模块文档。
    pub category: String,
    /// 动作（`skip` / `mute` / `full` / `poi`）。
    pub action_type: Option<String>,
    /// 起止（**毫秒**）。接口给的是秒（浮点），这里按基线
    /// `SegmentItemModel.fromJson` 的 `(e * 1000).round()` 归一。
    pub start_ms: i64,
    pub end_ms: i64,
    /// 视频总时长（毫秒）。基线此处是 `* 1000` 不取整，保持一致。
    pub video_duration_ms: Option<f64>,
    pub votes: Option<i64>,
    pub cid: Option<String>,
}

/// 提交者的统计信息。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SponsorUserInfo {
    pub view_count: i64,
    pub minutes_saved: f64,
    pub segment_count: i64,
}
