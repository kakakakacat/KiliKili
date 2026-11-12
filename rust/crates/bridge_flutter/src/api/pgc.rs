//! PGC 的 Dart 导出（S10 第一批 / issue #60）。
//!
//! 照 ADR-011 的 typed 直连样板写：结果用**值**表达失败而不是 `Result`，
//! 实体字段保持 `Option`，凭据按请求传入。
//!
//! ## 逐个写开，不要用宏收
//!
//! 下面七个结果结构的字段完全相同，八个取数函数的骨架也相同。**刻意不用
//! `macro_rules!`**：flutter_rust_bridge 的 codegen 用 `syn` 解析源码、
//! 不展开宏，宏生成的结构体与导出函数它一个都看不见，而且**不报错**
//! ——生成的绑定会静默少掉这些类型。详见 ADR-011 的 S9 补充一。
//!
//! ## 账号按接口分两个桶
//!
//! `ApiType.apiTypeSet` 把索引与两种排行归在 `recommend`，时间表、评论、
//! 追番状态没有列进去、落到默认的 `main`。Dart 侧按 path 查表选账号，
//! 不在这里猜。

use std::time::Duration;

use bili_api::http::RequestContext;
use bili_api::pgc::{self, IndexParams};
use bili_api::transport_reqwest::ReqwestTransport;
use bili_domain::error::DomainError;
use bili_domain::redact::Redacted;

use crate::api::init;
use crate::api::transport::TransportSettings;
use crate::api::write::WriteResult;
use bili_api::request::PreparedWrite;

// ------------------------------------------------------------------ 入参

/// 一次 PGC 请求的公共参数。
#[derive(Debug, Clone)]
pub struct PgcContext {
    pub base_url: String,
    /// 完整的 Cookie 头值。未登录时为 `None`。**Rust 不保存**，用完即弃。
    pub cookie: Option<String>,
    /// CSRF token（cookie 里的 `bili_jct`）。**只有短评的五条写操作要传**，
    /// 详情与列表那些只读接口传 `None`（ADR-013 决策四）。
    pub csrf: Option<String>,
    pub headers: Vec<(String, String)>,
    /// WBI 的 `mixin_key`。只有两种排行要签名，其余接口传空串。
    pub mixin_key: String,
    /// 签名用的 Unix 秒，由调用方注入以便 fixture 固定。
    pub wts: i64,
    pub timeout_ms: u64,
    pub transport: TransportSettings,
}

impl PgcContext {
    fn split(self) -> (RequestContext, ReqwestTransport, String, i64) {
        let transport = ReqwestTransport::new(self.transport.to_config());
        let ctx = RequestContext {
            base_url: self.base_url,
            cookie: self.cookie.map(Redacted::new),
            csrf: self.csrf.map(Redacted::new),
            headers: self.headers,
            timeout: Duration::from_millis(self.timeout_ms),
        };
        (ctx, transport, self.mixin_key, self.wts)
    }
}

/// 索引结果的筛选条件。
#[derive(Debug, Clone)]
pub struct PgcIndexQuery {
    pub page: i64,
    /// 用户选中的筛选项。键与取值都来自 [`fetch_pgc_index_condition`]
    /// 返回的 `field` / `keyword`，原样透传——它们是服务端定义的。
    pub filters: Vec<(String, String)>,
    pub season_type: Option<i64>,
    pub index_type: Option<i64>,
    /// 接口里这个参数就叫 `type`，与 `index_type` 不是一回事。
    pub type_: Option<i64>,
}

// ------------------------------------------------------------------ 实体

/// 索引结果里的一条。
#[derive(Debug, Clone)]
pub struct PgcIndexItem {
    pub badge: Option<String>,
    pub cover: Option<String>,
    pub index_show: Option<String>,
    /// 排序依据的展示值，随 `order` 参数变化（播放量/追番数…）。
    pub order: Option<String>,
    pub season_id: Option<i64>,
    pub title: Option<String>,
}

/// 一页索引结果。
#[derive(Debug, Clone)]
pub struct PgcIndexPage {
    /// 接口给的是 0/1，不是布尔。
    pub has_next: Option<i64>,
    pub list: Vec<PgcIndexItem>,
}

/// 一个筛选项的取值。
#[derive(Debug, Clone)]
pub struct PgcConditionValue {
    pub keyword: Option<String>,
    pub name: Option<String>,
}

/// 一组筛选项。
#[derive(Debug, Clone)]
pub struct PgcConditionFilter {
    pub field: Option<String>,
    pub name: Option<String>,
    pub values: Vec<PgcConditionValue>,
}

/// 一个排序方式。
#[derive(Debug, Clone)]
pub struct PgcConditionOrder {
    pub field: Option<String>,
    pub name: Option<String>,
    pub sort: Option<String>,
}

/// 索引页的筛选条件。
#[derive(Debug, Clone)]
pub struct PgcIndexCondition {
    pub filter: Vec<PgcConditionFilter>,
    pub order: Vec<PgcConditionOrder>,
}

/// 时间表里的一集。
#[derive(Debug, Clone)]
pub struct TimelineEpisode {
    pub cover: Option<String>,
    pub episode_id: Option<i64>,
    /// 0/1，是否已追。
    pub follow: Option<i64>,
    pub pub_index: Option<String>,
    /// 形如 `"19:00"`。时区与展示格式属界面，这里不折算。
    pub pub_time: Option<String>,
    pub season_id: Option<i64>,
    pub title: Option<String>,
}

/// 时间表的一天。没有新番的那天也会出现，界面按天排版。
#[derive(Debug, Clone)]
pub struct TimelineDay {
    pub date: Option<String>,
    pub date_ts: Option<i64>,
    pub day_of_week: Option<i64>,
    pub episodes: Vec<TimelineEpisode>,
    /// 0/1。
    pub is_today: Option<i64>,
}

/// 评论作者。
///
/// `vip_type` / `vip_status` 是摊平的，没有包成共享的 `Vip`：界面只用它
/// 判一句「是不是年度大会员」，而 `Vip` 在 Dart 侧还有五个未迁的生产者。
#[derive(Debug, Clone)]
pub struct PgcReviewAuthor {
    pub avatar: Option<String>,
    pub level: Option<i64>,
    pub mid: Option<i64>,
    pub uname: Option<String>,
    pub vip_type: Option<i64>,
    /// 缺失时为 0——界面判的是 `> 0`，给空会让那句判断炸掉。
    pub vip_status: i64,
}

/// 评论的互动计数。
#[derive(Debug, Clone)]
pub struct PgcReviewStat {
    pub disliked: Option<i64>,
    pub liked: Option<i64>,
    pub likes: Option<i64>,
}

/// 一条短评或长评。
#[derive(Debug, Clone)]
pub struct PgcReviewItem {
    /// 长评才有。
    pub article_id: Option<i64>,
    pub author: Option<PgcReviewAuthor>,
    pub title: Option<String>,
    pub content: Option<String>,
    pub push_time_str: Option<String>,
    pub review_id: Option<i64>,
    /// **0–5**。接口给 0–10，已按基线整除折半（9 分是 4 不是 5）。
    pub score: i64,
    pub stat: Option<PgcReviewStat>,
}

/// 一页评论。
#[derive(Debug, Clone)]
pub struct PgcReviewPage {
    pub list: Vec<PgcReviewItem>,
    /// 游标。为空表示没有下一页。
    pub next: Option<String>,
    pub count: Option<i64>,
}

/// 追番状态。
#[derive(Debug, Clone)]
pub struct SeasonStatus {
    /// 0/1，是否已追。
    pub follow: Option<i64>,
    /// 追番状态细分（想看/在看/看过）。
    pub follow_status: Option<i64>,
}

/// 排行榜里的一条。
#[derive(Debug, Clone)]
pub struct PgcRankItem {
    pub cover: Option<String>,
    /// 基线嵌在 `new_ep.index_show`，只有这一个字段，摊平。
    pub new_ep_index_show: Option<String>,
    /// 缺失时为 0，不是空。
    pub follow: i64,
    pub view: i64,
    pub title: Option<String>,
    pub url: Option<String>,
}

// ------------------------------------------------------------------ 结果

/// 索引结果。失败时 `data` 为 `None`，错误以值的形式带回。
#[derive(Debug, Clone)]
pub struct PgcIndexResult {
    pub data: Option<PgcIndexPage>,
    /// `network|remote|protocol|auth|storage|player|cancelled`。
    pub error_kind: Option<String>,
    /// 可直接展示的文案，不含凭据、不含完整 URL。
    pub error_detail: Option<String>,
    /// 服务端业务码。
    pub error_code: Option<i64>,
}

/// 索引筛选条件。字段语义同 [`PgcIndexResult`]。
#[derive(Debug, Clone)]
pub struct PgcIndexConditionResult {
    pub data: Option<PgcIndexCondition>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

/// 放送时间表。字段语义同 [`PgcIndexResult`]。
#[derive(Debug, Clone)]
pub struct PgcTimelineResult {
    pub data: Option<Vec<TimelineDay>>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

/// 一页评论。字段语义同 [`PgcIndexResult`]。
#[derive(Debug, Clone)]
pub struct PgcReviewResult {
    pub data: Option<PgcReviewPage>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

/// 追番状态。字段语义同 [`PgcIndexResult`]。
#[derive(Debug, Clone)]
pub struct SeasonStatusResult {
    pub data: Option<SeasonStatus>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

/// 排行榜。字段语义同 [`PgcIndexResult`]。
#[derive(Debug, Clone)]
pub struct PgcRankResult {
    pub data: Option<Vec<PgcRankItem>>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

// ------------------------------------------------------------------ 导出

/// 取一页索引结果。
pub async fn fetch_pgc_index(context: PgcContext, query: PgcIndexQuery) -> PgcIndexResult {
    let params = IndexParams {
        page: query.page,
        filters: query.filters,
        season_type: query.season_type,
        index_type: query.index_type,
        type_: query.type_,
    };
    let (ctx, transport, _, _) = context.split();

    let joined = init::runtime()
        .spawn(async move { pgc::fetch_index(&transport, &ctx, &params).await })
        .await;

    match flatten(joined) {
        Ok(page) => PgcIndexResult {
            data: Some(index_page(page)),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => PgcIndexResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 取「番剧索引」入口的一页。
///
/// 那十几个表示「不限」的 `-1` 由 Rust 侧补齐（整组一起才有意义，少一个
/// 服务端就会按自己的默认值过滤），调用方只给 `page` 与 `index_type`。
pub async fn fetch_pgc_default_index(
    context: PgcContext,
    page: Option<i64>,
    index_type: Option<i64>,
) -> PgcIndexResult {
    let (ctx, transport, _, _) = context.split();

    let joined = init::runtime()
        .spawn(async move { pgc::fetch_default_index(&transport, &ctx, page, index_type).await })
        .await;

    match flatten(joined) {
        Ok(page) => PgcIndexResult {
            data: Some(index_page(page)),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => PgcIndexResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 取索引的筛选条件与排序方式。
pub async fn fetch_pgc_index_condition(
    context: PgcContext,
    season_type: Option<i64>,
    type_: i64,
    index_type: Option<i64>,
) -> PgcIndexConditionResult {
    let (ctx, transport, _, _) = context.split();

    let joined = init::runtime()
        .spawn(async move {
            pgc::fetch_index_condition(&transport, &ctx, season_type, type_, index_type).await
        })
        .await;

    match flatten(joined) {
        Ok(c) => PgcIndexConditionResult {
            data: Some(PgcIndexCondition {
                filter: c
                    .filter
                    .into_iter()
                    .map(|f| PgcConditionFilter {
                        field: f.field,
                        name: f.name,
                        values: f
                            .values
                            .into_iter()
                            .map(|v| PgcConditionValue {
                                keyword: v.keyword,
                                name: v.name,
                            })
                            .collect(),
                    })
                    .collect(),
                order: c
                    .order
                    .into_iter()
                    .map(|o| PgcConditionOrder {
                        field: o.field,
                        name: o.name,
                        sort: o.sort,
                    })
                    .collect(),
            }),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => PgcIndexConditionResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 取放送时间表。`types`：1 番剧 / 3 电影 / 4 国创。
pub async fn fetch_pgc_timeline(
    context: PgcContext,
    types: i64,
    before: i64,
    after: i64,
) -> PgcTimelineResult {
    let (ctx, transport, _, _) = context.split();

    let joined = init::runtime()
        .spawn(async move { pgc::fetch_timeline(&transport, &ctx, types, before, after).await })
        .await;

    match flatten(joined) {
        Ok(days) => PgcTimelineResult {
            data: Some(
                days.into_iter()
                    .map(|d| TimelineDay {
                        date: d.date,
                        date_ts: d.date_ts,
                        day_of_week: d.day_of_week,
                        episodes: d
                            .episodes
                            .into_iter()
                            .map(|e| TimelineEpisode {
                                cover: e.cover,
                                episode_id: e.episode_id,
                                follow: e.follow,
                                pub_index: e.pub_index,
                                pub_time: e.pub_time,
                                season_id: e.season_id,
                                title: e.title,
                            })
                            .collect(),
                        is_today: d.is_today,
                    })
                    .collect(),
            ),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => PgcTimelineResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 取一页评论。`long` 为真取长评，否则取短评。
pub async fn fetch_pgc_reviews(
    context: PgcContext,
    long: bool,
    media_id: i64,
    sort: i64,
    cursor: Option<String>,
) -> PgcReviewResult {
    let (ctx, transport, _, _) = context.split();

    let joined = init::runtime()
        .spawn(async move {
            pgc::fetch_reviews(&transport, &ctx, long, media_id, sort, cursor.as_deref()).await
        })
        .await;

    match flatten(joined) {
        Ok(page) => PgcReviewResult {
            data: Some(PgcReviewPage {
                next: page.next,
                count: page.count,
                list: page
                    .list
                    .into_iter()
                    .map(|r| PgcReviewItem {
                        article_id: r.article_id,
                        author: r.author.map(|a| PgcReviewAuthor {
                            avatar: a.avatar,
                            level: a.level,
                            mid: a.mid,
                            uname: a.uname,
                            vip_type: a.vip_type,
                            vip_status: a.vip_status,
                        }),
                        title: r.title,
                        content: r.content,
                        push_time_str: r.push_time_str,
                        review_id: r.review_id,
                        score: r.score,
                        stat: r.stat.map(|s| PgcReviewStat {
                            disliked: s.disliked,
                            liked: s.liked,
                            likes: s.likes,
                        }),
                    })
                    .collect(),
            }),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => PgcReviewResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 取追番状态。
pub async fn fetch_season_status(context: PgcContext, season_id: i64) -> SeasonStatusResult {
    let (ctx, transport, _, _) = context.split();

    let joined = init::runtime()
        .spawn(async move { pgc::fetch_season_status(&transport, &ctx, season_id).await })
        .await;

    match flatten(joined) {
        Ok(s) => SeasonStatusResult {
            data: Some(SeasonStatus {
                follow: s.follow,
                follow_status: s.follow_status,
            }),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => SeasonStatusResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 取排行榜。
///
/// `season` 为真走 season 变体（载荷在 `data`），否则走 web 变体
/// （载荷在 `result`）。两条都要 WBI 签名，`context.mixin_key` 不能为空。
pub async fn fetch_pgc_rank(
    context: PgcContext,
    season: bool,
    day: i64,
    season_type: i64,
) -> PgcRankResult {
    let (ctx, transport, mixin_key, wts) = context.split();

    let joined = init::runtime()
        .spawn(async move {
            pgc::fetch_rank(&transport, &ctx, season, day, season_type, &mixin_key, wts).await
        })
        .await;

    match flatten(joined) {
        Ok(items) => PgcRankResult {
            data: Some(
                items
                    .into_iter()
                    .map(|r| PgcRankItem {
                        cover: r.cover,
                        new_ep_index_show: r.new_ep_index_show,
                        follow: r.follow,
                        view: r.view,
                        title: r.title,
                        url: r.url,
                    })
                    .collect(),
            ),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => PgcRankResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

// ------------------------------------------------------------------ 内部

fn index_page(page: bili_domain::pgc::PgcIndexPage) -> PgcIndexPage {
    PgcIndexPage {
        has_next: page.has_next,
        list: page
            .list
            .into_iter()
            .map(|i| PgcIndexItem {
                badge: i.badge,
                cover: i.cover,
                index_show: i.index_show,
                order: i.order,
                season_id: i.season_id,
                title: i.title,
            })
            .collect(),
    }
}

/// 把 `JoinHandle` 的两层结果压平。
///
/// 任务 panic 或运行时关闭时**不**把 panic 传给 Dart：宿主 App 不该因为
/// 一条取数路径而崩溃。
fn flatten<T>(
    joined: Result<Result<T, DomainError>, tokio::task::JoinError>,
) -> Result<T, DomainError> {
    match joined {
        Ok(inner) => inner,
        Err(e) => Err(DomainError::new(
            bili_domain::error::ErrorKind::Network,
            format!("取数任务未完成: {e}"),
        )),
    }
}

// ------------------------------------------------- 番剧详情（S10 第二批）

/// 一集。
///
/// **不是** `lib/models_new/pgc/pgc_info_model/episode.dart` 的替代品：
/// 那个类继承共享的 `BaseEpisodeItem`，剧集面板按基类做多态消费，而
/// `BaseEpisodeItem` 还有四个未迁的生产者（UGC 分 P、合集、媒体列表、
/// 互动视频）。按 ADR-011 决策六，共享模型要等**所有**生产者迁完才能删，
/// 所以 Dart 侧留了一个单向适配 `lib/services/rust/pgc_detail_compat.dart`，
/// 只在需要那条多态列表的地方把本结构转过去。
#[derive(Debug, Clone)]
pub struct PgcEpisode {
    pub id: Option<i64>,
    pub aid: Option<i64>,
    pub cid: Option<i64>,
    pub ep_id: Option<i64>,
    pub bvid: Option<String>,
    pub badge: Option<String>,
    pub title: Option<String>,
    pub cover: Option<String>,
    /// 已按 `rotate` 校正：响应在 `rotate == 1` 时给的是交换过的宽高。
    pub width: Option<i64>,
    pub height: Option<i64>,
    /// **单位随接口变**：番剧是毫秒，课堂是秒。按 `from == "pugv"` 分流，
    /// 与基线一致。
    pub duration: Option<i64>,
    pub from: Option<String>,
    pub link: Option<String>,
    pub long_title: Option<String>,
    pub pub_time: Option<i64>,
    pub share_copy: Option<String>,
    pub share_url: Option<String>,
    pub show_title: Option<String>,
    pub play: Option<i64>,
}

/// 正片之外的一组（PV、花絮、特别篇…）。空组也会保留。
#[derive(Debug, Clone)]
pub struct PgcSection {
    pub title: Option<String>,
    pub episodes: Vec<PgcEpisode>,
}

/// 季度统计。
///
/// 前三个字段缺失时为 0，后四个保持 `None`——基线就是这样不对称的，
/// 界面对两组的处理不同（前三个总显示数字，后四个缺失时整块隐藏）。
#[derive(Debug, Clone)]
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
#[derive(Debug, Clone)]
pub struct PgcUpInfo {
    pub mid: Option<i64>,
    pub avatar: Option<String>,
    pub uname: Option<String>,
}

/// 当前用户对这一季的状态。
#[derive(Debug, Clone)]
pub struct PgcUserStatus {
    /// 0/1，是否已追。
    pub favored: Option<i64>,
    pub last_ep_id: Option<i64>,
}

/// 参与的创作者。
#[derive(Debug, Clone)]
pub struct PgcCooperator {
    pub mid: Option<i64>,
    pub avatar: Option<String>,
    pub nick_name: Option<String>,
    pub role: Option<String>,
}

/// 简介里的一张图。`aspect_ratio` 缺失时为 1，不是 0。
#[derive(Debug, Clone)]
pub struct PgcBriefImage {
    pub aspect_ratio: f64,
    pub url: Option<String>,
}

/// 同系列的另一季。
#[derive(Debug, Clone)]
pub struct PgcSeasonBrief {
    pub media_id: Option<i64>,
    pub season_id: Option<i64>,
    pub season_title: Option<String>,
}

/// 番剧 / 课堂详情。
///
/// 基线 `PgcInfoModel` 的五个单字段壳（`Area`/`NewEp`/`Publish`/
/// `Rating`/`Brief`）已摊平；`stat`/`up`/`user_status` 保留结构。
#[derive(Debug, Clone)]
pub struct PgcSeasonDetail {
    pub actors: Option<String>,
    pub areas: Vec<String>,
    pub cover: Option<String>,
    pub episodes: Vec<PgcEpisode>,
    pub evaluate: Option<String>,
    pub media_id: Option<i64>,
    pub new_ep_desc: Option<String>,
    pub new_ep_title: Option<String>,
    pub pub_time_show: Option<String>,
    pub rating_score: Option<f64>,
    pub season_id: Option<i64>,
    pub season_title: Option<String>,
    pub sections: Vec<PgcSection>,
    pub stat: Option<PgcSeasonStat>,
    pub subtitle: Option<String>,
    pub title: Option<String>,
    /// 响应里叫 `type`。
    pub kind: Option<i64>,
    pub up: Option<PgcUpInfo>,
    pub user_status: Option<PgcUserStatus>,
    pub cooperators: Vec<PgcCooperator>,
    pub brief_images: Vec<PgcBriefImage>,
    pub seasons: Vec<PgcSeasonBrief>,
}

/// 番剧 / 课堂详情。字段语义同 [`PgcIndexResult`]。
#[derive(Debug, Clone)]
pub struct PgcSeasonDetailResult {
    pub data: Option<PgcSeasonDetail>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

fn episode(value: bili_domain::pgc::PgcEpisode) -> PgcEpisode {
    PgcEpisode {
        id: value.id,
        aid: value.aid,
        cid: value.cid,
        ep_id: value.ep_id,
        bvid: value.bvid,
        badge: value.badge,
        title: value.title,
        cover: value.cover,
        width: value.width,
        height: value.height,
        duration: value.duration,
        from: value.from,
        link: value.link,
        long_title: value.long_title,
        pub_time: value.pub_time,
        share_copy: value.share_copy,
        share_url: value.share_url,
        show_title: value.show_title,
        play: value.play,
    }
}

fn season_detail(value: bili_domain::pgc::PgcSeasonDetail) -> PgcSeasonDetail {
    PgcSeasonDetail {
        actors: value.actors,
        areas: value.areas,
        cover: value.cover,
        episodes: value.episodes.into_iter().map(episode).collect(),
        evaluate: value.evaluate,
        media_id: value.media_id,
        new_ep_desc: value.new_ep_desc,
        new_ep_title: value.new_ep_title,
        pub_time_show: value.pub_time_show,
        rating_score: value.rating_score,
        season_id: value.season_id,
        season_title: value.season_title,
        sections: value
            .sections
            .into_iter()
            .map(|s| PgcSection {
                title: s.title,
                episodes: s.episodes.into_iter().map(episode).collect(),
            })
            .collect(),
        stat: value.stat.map(|s| PgcSeasonStat {
            coin: s.coin,
            favorite: s.favorite,
            like: s.like,
            danmaku: s.danmaku,
            reply: s.reply,
            share: s.share,
            view: s.view,
        }),
        subtitle: value.subtitle,
        title: value.title,
        kind: value.kind,
        up: value.up.map(|u| PgcUpInfo {
            mid: u.mid,
            avatar: u.avatar,
            uname: u.uname,
        }),
        user_status: value.user_status.map(|u| PgcUserStatus {
            favored: u.favored,
            last_ep_id: u.last_ep_id,
        }),
        cooperators: value
            .cooperators
            .into_iter()
            .map(|c| PgcCooperator {
                mid: c.mid,
                avatar: c.avatar,
                nick_name: c.nick_name,
                role: c.role,
            })
            .collect(),
        brief_images: value
            .brief_images
            .into_iter()
            .map(|i| PgcBriefImage {
                aspect_ratio: i.aspect_ratio,
                url: i.url,
            })
            .collect(),
        seasons: value
            .seasons
            .into_iter()
            .map(|s| PgcSeasonBrief {
                media_id: s.media_id,
                season_id: s.season_id,
                season_title: s.season_title,
            })
            .collect(),
    }
}

/// 取番剧详情（`pgcInfo`，载荷在 `result`）。
///
/// `season_id` 与 `ep_id` 至少要给一个，两个都缺时不发请求，直接返回
/// `protocol` 错误。
pub async fn fetch_pgc_season_detail(
    context: PgcContext,
    season_id: Option<i64>,
    ep_id: Option<i64>,
) -> PgcSeasonDetailResult {
    fetch_season_detail_of(context, pgc::SeasonKind::Pgc, season_id, ep_id).await
}

/// 取课堂详情（`pugvInfo`，载荷在 `data`）。
pub async fn fetch_pugv_season_detail(
    context: PgcContext,
    season_id: Option<i64>,
    ep_id: Option<i64>,
) -> PgcSeasonDetailResult {
    fetch_season_detail_of(context, pgc::SeasonKind::Pugv, season_id, ep_id).await
}

async fn fetch_season_detail_of(
    context: PgcContext,
    kind: pgc::SeasonKind,
    season_id: Option<i64>,
    ep_id: Option<i64>,
) -> PgcSeasonDetailResult {
    let params = pgc::SeasonDetailParams { season_id, ep_id };
    let (ctx, transport, _, _) = context.split();

    let joined = init::runtime()
        .spawn(async move { pgc::fetch_season_detail(&transport, &ctx, kind, params).await })
        .await;

    match flatten(joined) {
        Ok(detail) => PgcSeasonDetailResult {
            data: Some(season_detail(detail)),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => PgcSeasonDetailResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

// --------------------------------------------------- 短评的写操作（#60）

/// 给一条短评点赞。
///
/// **不自动重试。** 基线的请求里没有状态位，从调用点看不出服务端是「置为」
/// 还是「切换」，所以按更保守的切换处理（ADR-013 决策三定级为可判定）：
/// 失败了由用户看着列表里的赞踩状态决定要不要再点一次。
pub async fn pgc_review_like(context: PgcContext, media_id: i64, review_id: i64) -> WriteResult {
    send_review(context, pgc::review_like_request(media_id, review_id)).await
}

/// 给一条短评点踩。与点赞只有路径不同，见 [`pgc_review_like`]。
pub async fn pgc_review_dislike(context: PgcContext, media_id: i64, review_id: i64) -> WriteResult {
    send_review(context, pgc::review_dislike_request(media_id, review_id)).await
}

/// 发表一条短评。
///
/// **绝不自动重发**（ADR-013 定级为不可重发）：重发 = 用户看得见的两条
/// 相同短评。`share_feed` 为 false 时整个字段都不发，与基线一致。
pub async fn pgc_review_post(
    context: PgcContext,
    media_id: i64,
    score: i64,
    content: String,
    share_feed: bool,
) -> WriteResult {
    send_review(
        context,
        pgc::review_post_request(media_id, score, &content, share_feed),
    )
    .await
}

/// 修改一条已有的短评。带 `review_id`，是置为语义。
pub async fn pgc_review_modify(
    context: PgcContext,
    media_id: i64,
    review_id: i64,
    score: i64,
    content: String,
) -> WriteResult {
    send_review(
        context,
        pgc::review_modify_request(media_id, review_id, score, &content),
    )
    .await
}

/// 删除一条短评。
pub async fn pgc_review_del(context: PgcContext, media_id: i64, review_id: i64) -> WriteResult {
    send_review(context, pgc::review_del_request(media_id, review_id)).await
}

/// 五条短评写操作共用的发送路径。
///
/// 不导出给 Dart（没有 `pub`），只是免得上面五个函数各自抄一遍
/// spawn + flatten + 折结果——抄错一处的表现是「失败被当成了成功」。
async fn send_review(context: PgcContext, prepared: PreparedWrite) -> WriteResult {
    let (ctx, transport, _, _) = context.split();
    let joined = init::runtime()
        .spawn(async move { pgc::send_review_write(&transport, &ctx, &prepared).await })
        .await;

    WriteResult::from_result(flatten(joined))
}
