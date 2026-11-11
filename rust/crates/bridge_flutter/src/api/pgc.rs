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

// ------------------------------------------------------------------ 入参

/// 一次 PGC 请求的公共参数。
#[derive(Debug, Clone)]
pub struct PgcContext {
    pub base_url: String,
    /// 完整的 Cookie 头值。未登录时为 `None`。**Rust 不保存**，用完即弃。
    pub cookie: Option<String>,
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
