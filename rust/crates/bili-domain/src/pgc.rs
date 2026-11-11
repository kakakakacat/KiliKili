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

// ------------------------------------------------------------ 番剧详情

/// 番剧 / 课堂详情（`pgcInfo` / `pugvInfo`，S10 第二批 / issue #60）。
///
/// ## 单字段包装一律摊平
///
/// 基线有五个只装着一个字段的壳：`Area{name}`、`NewEp{desc,title}`、
/// `Publish{pub_time_show}`、`Rating{score}`、`Brief{img}`。它们在协议上
/// 没有各自的含义，摊平后调用点少一层 `?.`，也少五个只为穿透而存在的
/// Dart 模型。
///
/// ## 没有摊平的那几个
///
/// `stat` / `up` / `user_status` 保留结构：它们各自有多个字段，摊平会让
/// 根结构多出十来个前缀字段，反而看不出哪些是一组。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PgcSeasonDetail {
    pub actors: Option<String>,
    /// 基线是 `List<Area>`，而 `Area` 只有 `name`。
    pub areas: Vec<String>,
    pub cover: Option<String>,
    /// 正片。`sections` 是正片之外的 PV、花絮等。
    pub episodes: Vec<PgcEpisode>,
    pub evaluate: Option<String>,
    pub media_id: Option<i64>,
    pub new_ep_desc: Option<String>,
    pub new_ep_title: Option<String>,
    pub pub_time_show: Option<String>,
    pub rating_score: Option<f64>,
    pub season_id: Option<i64>,
    pub season_title: Option<String>,
    /// 响应里的键是 **`section`**（单数），不是 `sections`。
    pub sections: Vec<PgcSection>,
    pub stat: Option<PgcSeasonStat>,
    pub subtitle: Option<String>,
    pub title: Option<String>,
    /// 响应里叫 `type`，Rust 的关键字。番剧/国创/电影…的区分。
    pub kind: Option<i64>,
    pub up: Option<PgcUpInfo>,
    pub user_status: Option<PgcUserStatus>,
    pub cooperators: Vec<PgcCooperator>,
    /// 基线 `brief.img`。
    pub brief_images: Vec<PgcBriefImage>,
    /// 同系列的其他季。
    pub seasons: Vec<PgcSeasonBrief>,
}

/// 一集。
///
/// `width` / `height` **已按 `rotate` 校正**，见
/// [`crate::pgc::apply_rotate`]。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PgcEpisode {
    pub id: Option<i64>,
    pub aid: Option<i64>,
    pub cid: Option<i64>,
    pub ep_id: Option<i64>,
    pub bvid: Option<String>,
    pub badge: Option<String>,
    pub title: Option<String>,
    pub cover: Option<String>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    /// **单位随接口变**：`pgcInfo` 是毫秒，`pugvInfo` 是秒。基线原样透传
    /// 并在界面上按 `from == 'pugv'` 分流，这里照搬——在这里统一成一种
    /// 单位需要知道自己是哪条接口，而那是调用方才知道的事。
    pub duration: Option<i64>,
    pub from: Option<String>,
    pub link: Option<String>,
    pub long_title: Option<String>,
    /// 基线 `pub_time ?? release_date`：两个键表示同一件事。
    pub pub_time: Option<i64>,
    pub share_copy: Option<String>,
    pub share_url: Option<String>,
    pub show_title: Option<String>,
    pub play: Option<i64>,
}

/// 正片之外的一组（PV、花絮、特别篇…）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PgcSection {
    pub title: Option<String>,
    pub episodes: Vec<PgcEpisode>,
}

/// 季度统计。
///
/// 基线 `PgcStat.fromJson` 对七个字段的处理**不对称**：`coins` /
/// `favorite` / `likes` 缺失时取 0，`danmakus` / `reply` / `share` /
/// `views` 缺失时留 null。照搬——界面对这两组的处理不同（前三个总是
/// 显示数字，后四个缺失时整块隐藏），统一成 0 会让隐藏逻辑失效。
///
/// 另注意键名：是 `coins` / `likes` / `views` 的复数形式，而
/// `favorite` / `reply` / `share` 是单数。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PgcSeasonStat {
    pub coin: i64,
    pub favorite: i64,
    pub like: i64,
    pub danmaku: Option<i64>,
    pub reply: Option<i64>,
    pub share: Option<i64>,
    pub view: Option<i64>,
}

/// 出品方 / UP 主。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PgcUpInfo {
    pub mid: Option<i64>,
    pub avatar: Option<String>,
    pub uname: Option<String>,
}

/// 当前用户对这一季的状态。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PgcUserStatus {
    /// 0/1，是否已收藏（追番）。
    pub favored: Option<i64>,
    /// 基线 `progress.last_ep_id`，只有这一个字段，摊平。
    pub last_ep_id: Option<i64>,
}

/// 参与的创作者。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PgcCooperator {
    pub mid: Option<i64>,
    pub avatar: Option<String>,
    pub nick_name: Option<String>,
    pub role: Option<String>,
}

/// 简介里的一张图。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PgcBriefImage {
    /// 缺失时为 1，与基线 `?? 1` 一致：0 会让布局除零。
    pub aspect_ratio: f64,
    pub url: Option<String>,
}

/// 同系列的另一季。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PgcSeasonBrief {
    pub media_id: Option<i64>,
    pub season_id: Option<i64>,
    pub season_title: Option<String>,
}

/// 按 `rotate` 校正宽高。
///
/// 移植自基线 `Dimension.fromJson`：`rotate == 1` 时响应里的 `width` 与
/// `height` 是**交换过的**，要换回来。不校正的话竖屏片源会被当成横屏，
/// 播放器按错误的比例铺画面——这种错不会报错，只是画面比例不对。
#[must_use]
pub fn apply_rotate(
    width: Option<i64>,
    height: Option<i64>,
    rotate: Option<i64>,
) -> (Option<i64>, Option<i64>) {
    if rotate == Some(1) {
        (height, width)
    } else {
        (width, height)
    }
}
