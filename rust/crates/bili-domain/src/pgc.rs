//! PGC（番剧 / 影视 / 课堂）业务实体。
//!
//! 覆盖索引、放送时间表、短评与长评列表、追番状态、排行榜。**番剧详情
//! （`pgcInfo` / `pugvInfo`）不在这里**——它的 `PgcInfoModel` 深入播放页
//! （剧集列表、下载面板、简介页共八个消费点），留到 S10 第二批，等播放页
//! 那条分支落地后再动，理由见 issue #60 的讨论。
//!
//! ## 三种信封，不要合并
//!
//! PGC 这一组接口的载荷键**不统一**：索引与排行的 season 变体放在 `data`，
//! 时间表、追番状态与排行的 web 变体放在 `result`。这不是历史包袱而是
//! 现状，解析时按接口各取各的，别为了"统一"去猜。
//!
//! ## 评分除以二是协议，不是排版
//!
//! 短评接口给的 `score` 是 0–10，而界面与提交接口用的都是 0–5
//! （基线 `PgcReviewItemModel` 里那句 `json['score'] ~/ 2`）。换一套 UI
//! 这个折算仍然成立，所以按 ADR-011 决策二归 Rust。**整数除法**，不是
//! 四舍五入——基线用的是 `~/`。

use serde::Serialize;

// ------------------------------------------------------------------ 索引

/// 索引结果里的一条。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PgcIndexItem {
    pub badge: Option<String>,
    pub cover: Option<String>,
    pub index_show: Option<String>,
    /// 排序依据的展示值（播放量/追番数…），随 `order` 参数变化。
    pub order: Option<String>,
    pub season_id: Option<i64>,
    pub title: Option<String>,
}

/// 一页索引结果。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct PgcIndexPage {
    /// 接口给的是 0/1，不是布尔。原样带回。
    pub has_next: Option<i64>,
    pub list: Vec<PgcIndexItem>,
}

/// 一个筛选项的取值。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PgcConditionValue {
    pub keyword: Option<String>,
    pub name: Option<String>,
}

/// 一组筛选项（地区、年份、风格…）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PgcConditionFilter {
    pub field: Option<String>,
    pub name: Option<String>,
    pub values: Vec<PgcConditionValue>,
}

/// 一个排序方式。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PgcConditionOrder {
    pub field: Option<String>,
    pub name: Option<String>,
    pub sort: Option<String>,
}

/// 索引页的筛选条件。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct PgcIndexCondition {
    pub filter: Vec<PgcConditionFilter>,
    pub order: Vec<PgcConditionOrder>,
}

// -------------------------------------------------------------- 放送时间表

/// 时间表里的一集。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TimelineEpisode {
    pub cover: Option<String>,
    pub episode_id: Option<i64>,
    /// 0/1，是否已追。
    pub follow: Option<i64>,
    pub pub_index: Option<String>,
    /// 形如 `"19:00"`。接口给的就是字符串，不折算——时区与展示格式
    /// 属界面。
    pub pub_time: Option<String>,
    pub season_id: Option<i64>,
    pub title: Option<String>,
}

/// 时间表的一天。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TimelineDay {
    pub date: Option<String>,
    pub date_ts: Option<i64>,
    pub day_of_week: Option<i64>,
    pub episodes: Vec<TimelineEpisode>,
    /// 0/1。
    pub is_today: Option<i64>,
}

// ------------------------------------------------------------------ 评论

/// 短评/长评的作者。
///
/// `vip_type` / `vip_status` 刻意**摊平**，没有包成共享的 `Vip` 实体：
/// 这个界面只用它判一句「是不是年度大会员」（`status > 0 && type == 2`），
/// 而 `Vip` 在 Dart 侧还有五个未迁的生产者（ADR-011 决策六）。为两个
/// 整数去动那个共享模型不划算。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PgcReviewAuthor {
    pub avatar: Option<String>,
    pub level: Option<i64>,
    pub mid: Option<i64>,
    pub uname: Option<String>,
    pub vip_type: Option<i64>,
    /// 缺失时为 0，与基线 `json['status'] ?? 0` 一致。
    pub vip_status: i64,
}

/// 评论的互动计数。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PgcReviewStat {
    /// 0/1，当前用户是否点过踩。
    pub disliked: Option<i64>,
    /// 0/1，当前用户是否点过赞。
    pub liked: Option<i64>,
    pub likes: Option<i64>,
}

/// 一条短评或长评。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PgcReviewItem {
    /// 长评才有；短评为空。
    pub article_id: Option<i64>,
    pub author: Option<PgcReviewAuthor>,
    pub title: Option<String>,
    pub content: Option<String>,
    pub push_time_str: Option<String>,
    pub review_id: Option<i64>,
    /// **0–5**。接口给 0–10，这里按基线 `~/ 2` 折半（整数除法）。
    /// 缺失时为 0。
    pub score: i64,
    pub stat: Option<PgcReviewStat>,
}

/// 一页评论。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct PgcReviewPage {
    pub list: Vec<PgcReviewItem>,
    /// 游标。为空表示没有下一页。
    pub next: Option<String>,
    /// 总数。短评接口给 `total`，长评给 `count`，基线取二者之一。
    pub count: Option<i64>,
}

// -------------------------------------------------------------- 追番状态

/// 追番/追剧状态。
///
/// 基线把整个 `result` 当 `Map` 丢给 UI，而调用点只读两个字段
/// （`lib/pages/video/introduction/pgc/controller.dart`）。收成结构更诚实：
/// 哪天接口改了形状，解析会先红，而不是悄悄变成 `null`。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SeasonStatus {
    /// 0/1，是否已追。
    pub follow: Option<i64>,
    /// 追番状态细分（想看/在看/看过）。
    pub follow_status: Option<i64>,
}

// ------------------------------------------------------------------ 排行

/// 排行榜里的一条。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PgcRankItem {
    pub cover: Option<String>,
    /// 基线嵌在 `new_ep.index_show` 里，只有这一个字段，摊平。
    pub new_ep_index_show: Option<String>,
    /// 缺失时为 0，与基线 `?? 0` 一致。
    pub follow: i64,
    pub view: i64,
    pub title: Option<String>,
    pub url: Option<String>,
}
