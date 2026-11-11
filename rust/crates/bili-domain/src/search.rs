//! 搜索业务实体。
//!
//! ## 富文本与纯文本一起给
//!
//! 搜索接口把命中的关键词用 `<em class="keyword">…</em>` 包起来。拆成
//! [`TitleSpan`] 是**对协议的理解**（ADR-011 决策二），放 Rust；而「命中
//! 部分用什么颜色画」是界面，留 Dart。
//!
//! 每个带富文本的实体同时给 `title_spans` 与 `title`：后者是前者拼接出
//! 的纯文本。基线在几处各自拼过一遍（`SearchVideoItemModel.title`、
//! `SearchArticleItemModel.subTitle`），拼接规则一致，没有理由让每个
//! 调用点各写一遍。
//!
//! ## 取值开放的字段保持字符串
//!
//! `item_type`（`video` / `ketang` / `live_room` …）、`season_type_name`
//! 这类字段服务端随时会加值，按 ADR-011 决策三保持 `String`，未知值交给
//! UI 忽略。
//!
//! ## 界面措辞不在这里
//!
//! 基线按 `type` 给出角标文案（课堂 / 直播 / 合作）并置 `isPugv` /
//! `isLive`。那是界面措辞，本层只带回 `item_type` 与 `is_union_video`
//! 这两个协议字段，由 Dart 决定画成什么。

use serde::Serialize;

/// 一段标题文本。`is_em` 为真表示它被服务端标成了命中关键词。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TitleSpan {
    pub is_em: bool,
    pub text: String,
}

/// 把富文本段落拼成纯文本。
#[must_use]
pub fn spans_to_plain(spans: &[TitleSpan]) -> String {
    spans.iter().map(|s| s.text.as_str()).collect()
}

// ---------------------------------------------------------------- 搜索建议

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SearchSuggestions {
    pub tag: Option<Vec<SearchSuggestion>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SearchSuggestion {
    pub term: Option<String>,
    #[serde(rename = "name")]
    pub text_rich: String,
}

// ---------------------------------------------------------------- 公共片段

/// UP 主。对应基线 `SearchOwner`，取的是扁平在条目上的 `mid` / `author`
/// / `upic` 三个字段，不是嵌套的 `owner` 对象。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SearchOwner {
    pub mid: Option<i64>,
    pub name: Option<String>,
    pub face: Option<String>,
}

/// 条目统计。字段名取基线 `SearchStat` 的语义，不是接口的原始键名
/// （接口用 `play` / `review`，这里叫 `view` / `reply`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SearchStat {
    pub view: Option<i64>,
    pub danmaku: Option<i64>,
    pub favorite: Option<i64>,
    pub reply: Option<i64>,
    pub like: Option<i64>,
}

/// 认证信息。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OfficialVerify {
    pub kind: Option<i64>,
    pub desc: Option<String>,
}

// ---------------------------------------------------------------- 视频结果

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SearchVideoItem {
    pub id: Option<i64>,
    pub aid: Option<i64>,
    pub bvid: Option<String>,
    pub arcurl: Option<String>,
    /// 富文本标题。
    pub title_spans: Vec<TitleSpan>,
    /// `title_spans` 拼接出的纯文本。
    pub title: String,
    pub description: Option<String>,
    pub pic: Option<String>,
    pub pubdate: Option<i64>,
    /// 基线把它读进 `ctime`，接口键名是 `senddate`。
    pub senddate: Option<i64>,
    /// **秒**。接口给的是 `"12:34"` 这种字符串，这里按基线
    /// `DurationUtils.parseDuration` 折算。
    pub duration_secs: i64,
    pub owner: SearchOwner,
    pub stat: SearchStat,
    /// `video` / `ketang` / `live_room` …。见模块文档：不收成枚举。
    pub item_type: Option<String>,
    /// `item_type == "live_room"` 时才有意义。
    pub roomid: Option<i64>,
    /// 接口给的是 `0` / `1`，基线判 `== 1` 画「合作」角标。
    pub is_union_video: Option<i64>,
    pub tag: Option<String>,
}

// ---------------------------------------------------------------- 用户结果

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SearchUserItem {
    pub item_type: Option<String>,
    pub mid: Option<i64>,
    pub uname: Option<String>,
    pub usign: Option<String>,
    pub fans: Option<i64>,
    pub videos: Option<i64>,
    pub upic: Option<String>,
    pub face_nft: Option<i64>,
    pub face_nft_type: Option<i64>,
    pub verify_info: Option<String>,
    pub level: Option<i64>,
    pub gender: Option<i64>,
    pub is_up_user: Option<i64>,
    pub is_live: Option<i64>,
    pub room_id: Option<i64>,
    pub official_verify: Option<OfficialVerify>,
    pub is_senior_member: Option<i64>,
}

/// 综合搜索里的用户卡片。比 [`SearchUserItem`] 多挂几条代表作，少几个
/// 字段——是接口的两个不同形状，不要合并。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SearchUserCard {
    pub mid: Option<i64>,
    pub uname: Option<String>,
    pub usign: Option<String>,
    pub fans: Option<i64>,
    pub videos: Option<i64>,
    pub upic: Option<String>,
    pub verify_info: Option<String>,
    pub level: Option<i64>,
    pub gender: Option<i64>,
    pub is_up_user: Option<i64>,
    pub is_live: Option<i64>,
    pub room_id: Option<i64>,
    pub official_verify: Option<OfficialVerify>,
    pub is_senior_member: Option<i64>,
    /// 代表作。
    pub res: Vec<SearchVideoItem>,
}

// ---------------------------------------------------------------- 直播结果

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SearchLiveItem {
    pub rank_offset: Option<i64>,
    pub uid: Option<i64>,
    pub tags: Option<String>,
    pub live_time: Option<String>,
    pub uname: Option<String>,
    pub uface: Option<String>,
    pub user_cover: Option<String>,
    pub item_type: Option<String>,
    pub title_spans: Vec<TitleSpan>,
    pub title: String,
    pub cover: Option<String>,
    pub online: Option<i64>,
    pub rank_index: Option<i64>,
    pub rank_score: Option<i64>,
    pub roomid: Option<i64>,
    pub attentions: Option<i64>,
    /// 已按基线 `Em.regCate` 剥掉外层标签。
    pub cate_name: Option<String>,
}

// ---------------------------------------------------------------- 番剧结果

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SearchPgcItem {
    pub item_type: Option<String>,
    pub media_id: Option<i64>,
    pub title_spans: Vec<TitleSpan>,
    pub title: String,
    pub org_title: Option<String>,
    pub media_type: Option<i64>,
    pub cv: Option<String>,
    pub staff: Option<String>,
    pub season_id: Option<i64>,
    pub is_avid: Option<bool>,
    pub hit_epids: Option<String>,
    pub season_type: Option<i64>,
    pub season_type_name: Option<String>,
    pub url: Option<String>,
    pub button_text: Option<String>,
    pub is_follow: Option<i64>,
    pub is_selection: Option<i64>,
    pub cover: Option<String>,
    pub areas: Option<String>,
    pub styles: Option<String>,
    pub goto_url: Option<String>,
    pub desc: Option<String>,
    pub pubtime: Option<i64>,
    pub media_mode: Option<i64>,
    /// 评分。接口给的是 `{"score":9.7,"user_count":123}`，形状不稳定，
    /// 基线原样存成 `Map` 交给 UI。这里保持同样的不解释，给成 JSON 串。
    pub media_score_json: Option<String>,
    pub index_show: Option<String>,
}

// ---------------------------------------------------------------- 专栏结果

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SearchArticleItem {
    pub pub_time: Option<i64>,
    pub like: Option<i64>,
    pub title_spans: Vec<TitleSpan>,
    pub title: String,
    pub rank_offset: Option<i64>,
    pub mid: Option<i64>,
    pub image_urls: Vec<String>,
    pub id: Option<i64>,
    pub category_id: Option<i64>,
    pub view: Option<i64>,
    pub reply: Option<i64>,
    pub desc: Option<String>,
    pub rank_score: Option<i64>,
    pub item_type: Option<String>,
    pub template_id: Option<i64>,
    pub category_name: Option<String>,
}

// ---------------------------------------------------------------- 综合搜索

/// 综合搜索里的活动卡片。
///
/// 基线只保留 `url` 以直播域名开头的那些，其余丢弃——那是**界面决策**
/// （App 只能打开直播活动页），因此过滤留在 Dart，本层原样带回。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SearchActivityItem {
    pub id: Option<i64>,
    pub title: Option<String>,
    pub desc: Option<String>,
    pub cover: Option<String>,
    pub url: Option<String>,
    pub state: Option<i64>,
    pub status: Option<i64>,
    pub author: Option<String>,
}

/// 综合搜索的结果。各分区互不依赖，缺哪块就是空。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct SearchAllResult {
    pub num_results: Option<i64>,
    pub videos: Vec<SearchVideoItem>,
    pub users: Vec<SearchUserCard>,
    pub medias: Vec<SearchPgcItem>,
    pub activities: Vec<SearchActivityItem>,
    /// 赛事卡片的原始 JSON。
    ///
    /// 不在本层展开：它的实体是 `MatchContest`，而那个模型还有一个
    /// 未迁的生产者（`lib/http/match.dart`，S15）。按 ADR-011 决策六，
    /// 共享模型要等所有生产者迁完才能动，这里先原样带过去。
    pub esports_json: Option<String>,
}

// ------------------------------------------------------------ 热搜 / 默认词

/// 热搜条目。热搜榜与搜索页默认词用的是同一个形状。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SearchTrendingItem {
    pub keyword: String,
    /// 缺失时回落到 `keyword`，与基线一致。
    pub show_name: String,
    pub icon: Option<String>,
    pub show_live_icon: Option<bool>,
    /// 基线把首个中点换成空格再展示。那是**排版**，留在 Dart，本层给原值。
    pub recommend_reason: Option<String>,
}

/// 热搜榜。`top_count` 为置顶条数，置顶项已排在 `list` 前面。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct SearchTrending {
    pub list: Vec<SearchTrendingItem>,
    pub top_count: i64,
}
