//! PGC 接口：请求构造、响应解析与到领域实体的映射。
//!
//! 覆盖 S10 第一批的八条只读接口：索引结果、索引筛选条件、按条件取索引、
//! 放送时间表、短评/长评列表、追番状态、两种排行榜。
//!
//! **不在本模块**：
//! - 番剧详情 `pgcInfo` / `pugvInfo`——`PgcInfoModel` 深入播放页，留到
//!   S10 第二批（issue #60）；
//! - 短评的五个写操作（点赞/点踩/发表/修改/删除）——归 ADR-013（#50），
//!   写操作的幂等与 CSRF 没定之前不碰；
//! - PGC 的取流分支——已在 [`crate::playurl`] 里。
//!
//! ## 载荷键不统一
//!
//! | 接口 | 载荷键 |
//! |---|---|
//! | 索引结果 / 索引条件 / 评论列表 / season 排行 | `data` |
//! | 放送时间表 / 追番状态 / web 排行 | `result` |
//!
//! 两个解包器见 [`crate::envelope`]。别为了"统一"去猜另一个键存不存在。
//!
//! ## 为什么这里用 DTO 而搜索那边用 `Value`
//!
//! 搜索接口同一个键在不同 `search_type` 下类型会变，需要字段级容错，
//! 所以 S9 直接在 `serde_json::Value` 上取值。PGC 这几条形状稳定，
//! `#[derive(Deserialize)]` 的 DTO 足够，而且字段名拼错编译期就能发现
//! ——这是本 crate 的默认写法（见 crate 文档），搜索那边才是例外。

use bili_domain::error::DomainError;
use bili_domain::pgc::{
    PgcBriefImage, PgcConditionFilter, PgcConditionOrder, PgcConditionValue, PgcCooperator,
    PgcEpisode, PgcIndexCondition, PgcIndexItem, PgcIndexPage, PgcRankItem, PgcReviewAuthor,
    PgcReviewItem, PgcReviewPage, PgcReviewStat, PgcSeasonBrief, PgcSeasonDetail, PgcSeasonStat,
    PgcSection, PgcUpInfo, PgcUserStatus, SeasonStatus, TimelineDay, TimelineEpisode,
};
use serde::Deserialize;

use crate::envelope::{unwrap_envelope, unwrap_result_envelope};
use crate::http::{HttpGet, HttpTransport, RequestContext, check_status};
use crate::request::PreparedRequest;

/// 接口路径。与基线 `lib/http/api.dart` 保持一致。
pub mod path {
    /// 索引结果。`pgcIndex`（固定筛选条件的那个入口）用的也是它。
    pub const INDEX_RESULT: &str = "/pgc/season/index/result";
    /// 索引的筛选条件与排序方式。
    pub const INDEX_CONDITION: &str = "/pgc/season/index/condition";
    /// 放送时间表。
    pub const TIMELINE: &str = "/pgc/web/timeline";
    /// 长评列表。
    pub const REVIEW_LONG: &str = "/pgc/review/long/list";
    /// 短评列表。
    pub const REVIEW_SHORT: &str = "/pgc/review/short/list";
    /// 追番状态。
    pub const SEASON_STATUS: &str = "/pgc/view/web/season/user/status";
    /// 排行榜（web）。载荷在 `result`。
    pub const RANK: &str = "/pgc/web/rank/list";
    /// 排行榜（season）。载荷在 `data`。
    pub const SEASON_RANK: &str = "/pgc/season/rank/web/list";
    /// 番剧 / 国创 / 电影详情。载荷在 `result`。
    pub const SEASON_DETAIL: &str = "/pgc/view/web/season";
    /// 课堂详情。载荷在 `data`。
    pub const PUGV_SEASON_DETAIL: &str = "/pugv/view/web/season";
}

// ------------------------------------------------------------ 请求构造

/// 索引结果的查询参数。
///
/// `filters` 是用户在筛选面板里选中的那些键值对（地区、年份、风格…），
/// 取值来自 [`fetch_index_condition`] 返回的 `keyword`，原样透传——
/// 它们是服务端定义的，客户端没有理由去解释。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IndexParams {
    pub page: i64,
    pub filters: Vec<(String, String)>,
    pub season_type: Option<i64>,
    pub index_type: Option<i64>,
    /// 基线这个参数名就叫 `type`，与 `index_type` 是两个不同的东西。
    pub type_: Option<i64>,
}

/// 构造索引结果请求。
///
/// `pagesize=21` 是基线写死的，补在这里而不是让调用方传：它是分页约定，
/// 调用方没有理由有机会填错。
#[must_use]
pub fn build_index_request(params: &IndexParams) -> PreparedRequest {
    let mut query: Vec<(String, String)> = params.filters.clone();
    push_opt(&mut query, "season_type", params.season_type);
    push_opt(&mut query, "type", params.type_);
    push_opt(&mut query, "index_type", params.index_type);
    query.push(("page".to_owned(), params.page.to_string()));
    query.push(("pagesize".to_owned(), "21".to_owned()));

    PreparedRequest {
        path: path::INDEX_RESULT,
        query,
    }
}

/// 「番剧索引」入口的固定参数。
///
/// 基线 `PgcHttp.pgcIndex` 把这十几个 `-1` 写死在调用点上。`-1` 在这个
/// 接口里表示「不限」，整组一起才有意义（少一个，服务端会按它自己的默认
/// 值过滤），所以收在这里，调用方只给 `page` 与 `index_type`。
#[must_use]
pub fn build_default_index_request(page: Option<i64>, index_type: Option<i64>) -> PreparedRequest {
    let mut query: Vec<(String, String)> = [
        ("st", "1"),
        ("order", "3"),
        ("season_version", "-1"),
        ("spoken_language_type", "-1"),
        ("area", "-1"),
        ("is_finish", "-1"),
        ("copyright", "-1"),
        ("season_status", "-1"),
        ("season_month", "-1"),
        ("year", "-1"),
        ("style_id", "-1"),
        ("sort", "0"),
        ("season_type", "1"),
        ("pagesize", "20"),
        ("type", "1"),
    ]
    .iter()
    .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
    .collect();

    push_opt(&mut query, "page", page);
    push_opt(&mut query, "index_type", index_type);

    PreparedRequest {
        path: path::INDEX_RESULT,
        query,
    }
}

fn push_opt(query: &mut Vec<(String, String)>, key: &str, value: Option<i64>) {
    if let Some(value) = value {
        query.push((key.to_owned(), value.to_string()));
    }
}

// ------------------------------------------------------------------ 取数

async fn send<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    prepared: &PreparedRequest,
) -> Result<String, DomainError> {
    let query = if prepared.query.is_empty() {
        String::new()
    } else {
        format!("?{}", crate::client::encode_query(&prepared.query))
    };
    let url = format!(
        "{}{}{}",
        ctx.base_url.trim_end_matches('/'),
        prepared.path,
        query
    );

    tracing::debug!(path = prepared.path, "发送 PGC 请求");

    let resp = transport
        .get(HttpGet {
            url,
            headers: ctx.all_headers(),
            timeout: ctx.timeout,
        })
        .await?;

    check_status(resp.status)?;
    Ok(resp.body)
}

/// 取一页索引结果。
///
/// # Errors
/// 传输失败为 `Network`；非 2xx 见 [`check_status`]；业务失败见
/// [`crate::envelope`]。
pub async fn fetch_index<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    params: &IndexParams,
) -> Result<PgcIndexPage, DomainError> {
    let body = send(transport, ctx, &build_index_request(params)).await?;
    parse_index(&body)
}

/// 取「番剧索引」入口的一页。参数固定，见 [`build_default_index_request`]。
///
/// # Errors
/// 同 [`fetch_index`]。
pub async fn fetch_default_index<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    page: Option<i64>,
    index_type: Option<i64>,
) -> Result<PgcIndexPage, DomainError> {
    let prepared = build_default_index_request(page, index_type);
    let body = send(transport, ctx, &prepared).await?;
    parse_index(&body)
}

/// 取索引的筛选条件与排序方式。
///
/// # Errors
/// 同 [`fetch_index`]。
pub async fn fetch_index_condition<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    season_type: Option<i64>,
    type_: i64,
    index_type: Option<i64>,
) -> Result<PgcIndexCondition, DomainError> {
    let mut query = Vec::new();
    push_opt(&mut query, "season_type", season_type);
    query.push(("type".to_owned(), type_.to_string()));
    push_opt(&mut query, "index_type", index_type);

    let body = send(
        transport,
        ctx,
        &PreparedRequest {
            path: path::INDEX_CONDITION,
            query,
        },
    )
    .await?;
    parse_index_condition(&body)
}

/// 取放送时间表。
///
/// `types`：1 番剧 / 3 电影 / 4 国创。`before` / `after` 为相对今天的天数。
///
/// # Errors
/// 同 [`fetch_index`]。
pub async fn fetch_timeline<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    types: i64,
    before: i64,
    after: i64,
) -> Result<Vec<TimelineDay>, DomainError> {
    let body = send(
        transport,
        ctx,
        &PreparedRequest {
            path: path::TIMELINE,
            query: vec![
                ("types".to_owned(), types.to_string()),
                ("before".to_owned(), before.to_string()),
                ("after".to_owned(), after.to_string()),
            ],
        },
    )
    .await?;
    parse_timeline(&body)
}

/// 取一页评论。`long` 为真取长评，否则取短评。
///
/// # Errors
/// 同 [`fetch_index`]。
pub async fn fetch_reviews<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    long: bool,
    media_id: i64,
    sort: i64,
    cursor: Option<&str>,
) -> Result<PgcReviewPage, DomainError> {
    let mut query = vec![
        ("media_id".to_owned(), media_id.to_string()),
        ("ps".to_owned(), "20".to_owned()),
        ("sort".to_owned(), sort.to_string()),
    ];
    if let Some(cursor) = cursor {
        query.push(("cursor".to_owned(), cursor.to_owned()));
    }
    query.push(("web_location".to_owned(), "666.19".to_owned()));

    let body = send(
        transport,
        ctx,
        &PreparedRequest {
            path: if long {
                path::REVIEW_LONG
            } else {
                path::REVIEW_SHORT
            },
            query,
        },
    )
    .await?;
    parse_reviews(&body)
}

/// 取追番状态。
///
/// # Errors
/// 同 [`fetch_index`]。
pub async fn fetch_season_status<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    season_id: i64,
) -> Result<SeasonStatus, DomainError> {
    let body = send(
        transport,
        ctx,
        &PreparedRequest {
            path: path::SEASON_STATUS,
            query: vec![("season_id".to_owned(), season_id.to_string())],
        },
    )
    .await?;
    parse_season_status(&body)
}

/// 取排行榜。
///
/// `season` 为真走 `/pgc/season/rank/web/list`（载荷在 `data`），否则走
/// `/pgc/web/rank/list`（载荷在 `result`）。两条都要 WBI 签名。
///
/// # Errors
/// 同 [`fetch_index`]。
pub async fn fetch_rank<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    season: bool,
    day: i64,
    season_type: i64,
    mixin_key: &str,
    wts: i64,
) -> Result<Vec<PgcRankItem>, DomainError> {
    let mut query = vec![
        ("day".to_owned(), day.to_string()),
        ("season_type".to_owned(), season_type.to_string()),
        ("wts".to_owned(), wts.to_string()),
    ];
    query.sort_by(|a, b| a.0.cmp(&b.0));
    let w_rid = bili_protocol::wbi::sign(&query, mixin_key);
    query.push(("w_rid".to_owned(), w_rid));

    let body = send(
        transport,
        ctx,
        &PreparedRequest {
            path: if season {
                path::SEASON_RANK
            } else {
                path::RANK
            },
            query,
        },
    )
    .await?;
    parse_rank(&body, season)
}

// ------------------------------------------------------------------ 解析

#[derive(Deserialize)]
struct IndexPageDto {
    has_next: Option<i64>,
    list: Option<Vec<IndexItemDto>>,
}

#[derive(Deserialize)]
struct IndexItemDto {
    badge: Option<String>,
    cover: Option<String>,
    index_show: Option<String>,
    order: Option<String>,
    season_id: Option<i64>,
    title: Option<String>,
}

/// 解析索引结果。载荷在 `data`。
///
/// # Errors
/// 见 [`crate::envelope`]。
pub fn parse_index(body: &str) -> Result<PgcIndexPage, DomainError> {
    let dto: IndexPageDto = unwrap_envelope(body)?;
    Ok(PgcIndexPage {
        has_next: dto.has_next,
        list: dto
            .list
            .unwrap_or_default()
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
    })
}

#[derive(Deserialize)]
struct ConditionDto {
    filter: Option<Vec<FilterDto>>,
    order: Option<Vec<OrderDto>>,
}

#[derive(Deserialize)]
struct FilterDto {
    field: Option<String>,
    name: Option<String>,
    values: Option<Vec<ValueDto>>,
}

#[derive(Deserialize)]
struct ValueDto {
    keyword: Option<String>,
    name: Option<String>,
}

#[derive(Deserialize)]
struct OrderDto {
    field: Option<String>,
    name: Option<String>,
    sort: Option<String>,
}

/// 解析索引筛选条件。载荷在 `data`。
///
/// # Errors
/// 见 [`crate::envelope`]。
pub fn parse_index_condition(body: &str) -> Result<PgcIndexCondition, DomainError> {
    let dto: ConditionDto = unwrap_envelope(body)?;
    Ok(PgcIndexCondition {
        filter: dto
            .filter
            .unwrap_or_default()
            .into_iter()
            .map(|f| PgcConditionFilter {
                field: f.field,
                name: f.name,
                values: f
                    .values
                    .unwrap_or_default()
                    .into_iter()
                    .map(|v| PgcConditionValue {
                        keyword: v.keyword,
                        name: v.name,
                    })
                    .collect(),
            })
            .collect(),
        order: dto
            .order
            .unwrap_or_default()
            .into_iter()
            .map(|o| PgcConditionOrder {
                field: o.field,
                name: o.name,
                sort: o.sort,
            })
            .collect(),
    })
}

#[derive(Deserialize)]
struct TimelineDayDto {
    date: Option<String>,
    date_ts: Option<i64>,
    day_of_week: Option<i64>,
    episodes: Option<Vec<TimelineEpisodeDto>>,
    is_today: Option<i64>,
}

#[derive(Deserialize)]
struct TimelineEpisodeDto {
    cover: Option<String>,
    episode_id: Option<i64>,
    follow: Option<i64>,
    pub_index: Option<String>,
    pub_time: Option<String>,
    season_id: Option<i64>,
    title: Option<String>,
}

/// 解析放送时间表。载荷在 **`result`**。
///
/// # Errors
/// 见 [`crate::envelope`]。
pub fn parse_timeline(body: &str) -> Result<Vec<TimelineDay>, DomainError> {
    let dto: Vec<TimelineDayDto> = unwrap_result_envelope(body)?;
    Ok(dto
        .into_iter()
        .map(|d| TimelineDay {
            date: d.date,
            date_ts: d.date_ts,
            day_of_week: d.day_of_week,
            episodes: d
                .episodes
                .unwrap_or_default()
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
        .collect())
}

#[derive(Deserialize)]
struct ReviewPageDto {
    list: Option<Vec<ReviewItemDto>>,
    next: Option<String>,
    /// 长评给这个。
    count: Option<i64>,
    /// 短评给这个。基线是 `json['count'] ?? json['total']`。
    total: Option<i64>,
}

#[derive(Deserialize)]
struct ReviewItemDto {
    article_id: Option<i64>,
    author: Option<AuthorDto>,
    title: Option<String>,
    content: Option<String>,
    push_time_str: Option<String>,
    review_id: Option<i64>,
    score: Option<i64>,
    stat: Option<ReviewStatDto>,
}

#[derive(Deserialize)]
struct AuthorDto {
    avatar: Option<String>,
    level: Option<i64>,
    mid: Option<i64>,
    uname: Option<String>,
    vip: Option<VipDto>,
}

#[derive(Deserialize)]
struct VipDto {
    #[serde(rename = "type")]
    kind: Option<i64>,
    status: Option<i64>,
}

#[derive(Deserialize)]
struct ReviewStatDto {
    disliked: Option<i64>,
    liked: Option<i64>,
    likes: Option<i64>,
}

/// 解析一页评论。载荷在 `data`。
///
/// # Errors
/// 见 [`crate::envelope`]。
pub fn parse_reviews(body: &str) -> Result<PgcReviewPage, DomainError> {
    let dto: ReviewPageDto = unwrap_envelope(body)?;
    Ok(PgcReviewPage {
        // 基线：`json['count'] ?? json['total']`。
        count: dto.count.or(dto.total),
        next: dto.next,
        list: dto
            .list
            .unwrap_or_default()
            .into_iter()
            .map(|r| PgcReviewItem {
                article_id: r.article_id,
                author: r.author.map(|a| PgcReviewAuthor {
                    avatar: a.avatar,
                    level: a.level,
                    mid: a.mid,
                    uname: a.uname,
                    vip_type: a.vip.as_ref().and_then(|v| v.kind),
                    // 基线 `json['status'] ?? 0`。
                    vip_status: a.vip.and_then(|v| v.status).unwrap_or(0),
                }),
                title: r.title,
                content: r.content,
                push_time_str: r.push_time_str,
                review_id: r.review_id,
                // 基线：`json['score'] == null ? 0 : json['score'] ~/ 2`。
                // 整数除法，不是四舍五入——9 分显示成 4 星半里的 4。
                score: r.score.map_or(0, |s| s / 2),
                stat: r.stat.map(|s| PgcReviewStat {
                    disliked: s.disliked,
                    liked: s.liked,
                    likes: s.likes,
                }),
            })
            .collect(),
    })
}

#[derive(Deserialize)]
struct SeasonStatusDto {
    follow: Option<i64>,
    follow_status: Option<i64>,
}

/// 解析追番状态。载荷在 **`result`**。
///
/// # Errors
/// 见 [`crate::envelope`]。
pub fn parse_season_status(body: &str) -> Result<SeasonStatus, DomainError> {
    let dto: SeasonStatusDto = unwrap_result_envelope(body)?;
    Ok(SeasonStatus {
        follow: dto.follow,
        follow_status: dto.follow_status,
    })
}

#[derive(Deserialize)]
struct RankListDto {
    list: Option<Vec<RankItemDto>>,
}

#[derive(Deserialize)]
struct RankItemDto {
    cover: Option<String>,
    new_ep: Option<RankNewEpDto>,
    stat: Option<RankStatDto>,
    title: Option<String>,
    url: Option<String>,
}

#[derive(Deserialize)]
struct RankNewEpDto {
    index_show: Option<String>,
}

#[derive(Deserialize)]
struct RankStatDto {
    follow: Option<i64>,
    view: Option<i64>,
}

/// 解析排行榜。
///
/// `season` 决定载荷键：season 变体在 `data`，web 变体在 `result`。
/// 这是接口的现状，不是可以统一掉的东西——基线两个函数也是各读各的。
///
/// # Errors
/// 见 [`crate::envelope`]。
pub fn parse_rank(body: &str, season: bool) -> Result<Vec<PgcRankItem>, DomainError> {
    let dto: RankListDto = if season {
        unwrap_envelope(body)?
    } else {
        unwrap_result_envelope(body)?
    };
    Ok(dto
        .list
        .unwrap_or_default()
        .into_iter()
        .map(|r| PgcRankItem {
            cover: r.cover,
            new_ep_index_show: r.new_ep.and_then(|n| n.index_show),
            // 基线 `?? 0`。
            follow: r.stat.as_ref().and_then(|s| s.follow).unwrap_or(0),
            view: r.stat.and_then(|s| s.view).unwrap_or(0),
            title: r.title,
            url: r.url,
        })
        .collect())
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    fn data_envelope(payload: &str) -> String {
        format!(r#"{{"code":0,"message":"success","ttl":1,"data":{payload}}}"#)
    }

    fn result_envelope(payload: &str) -> String {
        format!(r#"{{"code":0,"message":"success","ttl":1,"result":{payload}}}"#)
    }

    #[test]
    fn index_page_carries_has_next_and_items() {
        let body = data_envelope(
            r#"{"has_next":1,"list":[
                {"badge":"会员","cover":"//c","index_show":"全12话",
                 "order":"1.2万追番","season_id":7,"title":"示例番剧"}
            ]}"#,
        );
        let page = parse_index(&body).expect("应解析成功");
        assert_eq!(page.has_next, Some(1));
        assert_eq!(page.list.len(), 1);
        assert_eq!(page.list[0].season_id, Some(7));
        assert_eq!(page.list[0].order.as_deref(), Some("1.2万追番"));
    }

    #[test]
    fn missing_list_is_an_empty_page_not_an_error() {
        let page = parse_index(&data_envelope(r#"{"has_next":0}"#)).expect("应解析成功");
        assert!(page.list.is_empty());
    }

    #[test]
    fn index_condition_keeps_server_keywords_verbatim() {
        // filter 的 keyword 要原样回传给索引接口，客户端不解释。
        let body = data_envelope(
            r#"{"filter":[{"field":"area","name":"地区","values":[
                    {"keyword":"-1","name":"全部"},{"keyword":"2","name":"日本"}]}],
                "order":[{"field":"3","name":"追番人数","sort":"0"}]}"#,
        );
        let c = parse_index_condition(&body).expect("应解析成功");
        assert_eq!(c.filter.len(), 1);
        assert_eq!(c.filter[0].values.len(), 2);
        assert_eq!(c.filter[0].values[1].keyword.as_deref(), Some("2"));
        assert_eq!(c.order[0].field.as_deref(), Some("3"));
    }

    #[test]
    fn timeline_comes_from_the_result_key() {
        // 时间表的载荷在 result，不是 data。读错键会得到一个空列表而不是
        // 报错，所以这条要钉住。
        let body = result_envelope(
            r#"[{"date":"11-11","date_ts":1762790400,"day_of_week":2,"is_today":1,
                 "episodes":[{"cover":"//c","episode_id":1,"follow":0,
                              "pub_index":"第5话","pub_time":"19:00",
                              "season_id":9,"title":"示例"}]}]"#,
        );
        let days = parse_timeline(&body).expect("应解析成功");
        assert_eq!(days.len(), 1);
        assert_eq!(days[0].is_today, Some(1));
        assert_eq!(days[0].episodes[0].pub_time.as_deref(), Some("19:00"));
    }

    #[test]
    fn timeline_read_from_the_data_key_would_fail() {
        // 反向确认：同一份载荷放在 data 下解不出来。这就是「别为了统一
        // 去猜另一个键」的理由。
        let body = data_envelope(r#"[{"date":"11-11"}]"#);
        assert!(parse_timeline(&body).is_err());
    }

    #[test]
    fn review_score_is_halved_with_integer_division() {
        // 基线 `json['score'] ~/ 2`：接口 0-10，界面 0-5。整数除法，
        // 9 分变 4 而不是 5。
        let body = data_envelope(
            r#"{"total":3,"next":"cur","list":[
                {"review_id":1,"score":10,"content":"满分"},
                {"review_id":2,"score":9,"content":"九分"},
                {"review_id":3,"content":"没有分"}]}"#,
        );
        let page = parse_reviews(&body).expect("应解析成功");
        assert_eq!(page.list[0].score, 5);
        assert_eq!(page.list[1].score, 4, "整数除法，不是四舍五入");
        assert_eq!(page.list[2].score, 0, "缺分数按 0");
        assert_eq!(page.count, Some(3), "短评给的是 total");
        assert_eq!(page.next.as_deref(), Some("cur"));
    }

    #[test]
    fn review_count_prefers_count_over_total() {
        // 基线 `json['count'] ?? json['total']`：长评给 count。
        let body = data_envelope(r#"{"count":7,"total":99,"list":[]}"#);
        assert_eq!(parse_reviews(&body).expect("应解析成功").count, Some(7));
    }

    #[test]
    fn review_author_vip_is_flattened() {
        let body = data_envelope(
            r#"{"list":[{"review_id":1,"author":{"mid":5,"uname":"甲","level":6,
                 "vip":{"type":2,"status":1}}}]}"#,
        );
        let page = parse_reviews(&body).expect("应解析成功");
        let author = page.list[0].author.as_ref().expect("有作者");
        assert_eq!(author.vip_type, Some(2));
        assert_eq!(author.vip_status, 1);
    }

    #[test]
    fn review_author_without_vip_defaults_status_to_zero() {
        // 基线 `json['status'] ?? 0`。界面判的是 `status > 0`，给 null
        // 会让那句判断炸掉。
        let body = data_envelope(r#"{"list":[{"review_id":1,"author":{"mid":5}}]}"#);
        let page = parse_reviews(&body).expect("应解析成功");
        let author = page.list[0].author.as_ref().expect("有作者");
        assert_eq!(author.vip_status, 0);
        assert_eq!(author.vip_type, None);
    }

    #[test]
    fn season_status_comes_from_the_result_key() {
        let body = result_envelope(r#"{"follow":1,"follow_status":2}"#);
        let s = parse_season_status(&body).expect("应解析成功");
        assert_eq!(s.follow, Some(1));
        assert_eq!(s.follow_status, Some(2));
    }

    #[test]
    fn the_two_rank_variants_read_different_payload_keys() {
        // web 排行在 result，season 排行在 data。基线两个函数各读各的，
        // 这里保持一致。
        let web = result_envelope(
            r#"{"list":[{"title":"甲","cover":"//c","url":"//u",
                 "new_ep":{"index_show":"更新至第5话"},
                 "stat":{"follow":100,"view":200}}]}"#,
        );
        let items = parse_rank(&web, false).expect("web 变体应解析成功");
        assert_eq!(items[0].new_ep_index_show.as_deref(), Some("更新至第5话"));
        assert_eq!(items[0].follow, 100);

        let season = data_envelope(r#"{"list":[{"title":"乙"}]}"#);
        let items = parse_rank(&season, true).expect("season 变体应解析成功");
        assert_eq!(items[0].title.as_deref(), Some("乙"));
        // 基线 `?? 0`：没有 stat 时计数为 0 而不是空。
        assert_eq!(items[0].follow, 0);
        assert_eq!(items[0].view, 0);

        // 读错键解不出来。
        assert!(parse_rank(&web, true).is_err());
    }

    #[test]
    fn remote_error_code_is_carried_through() {
        let body = r#"{"code":-404,"message":"啥都木有","data":null}"#;
        let err = parse_index(body).expect_err("必须失败");
        assert_eq!(err.code(), Some(-404));
    }

    #[test]
    fn default_index_request_keeps_the_whole_unlimited_param_set() {
        // 这十几个 -1 表示「不限」，少一个服务端就会按自己的默认值过滤，
        // 结果静默变少。整组一起才有意义。
        let req = build_default_index_request(Some(2), None);
        for key in [
            "st",
            "order",
            "season_version",
            "spoken_language_type",
            "area",
            "is_finish",
            "copyright",
            "season_status",
            "season_month",
            "year",
            "style_id",
            "sort",
            "season_type",
            "pagesize",
            "type",
        ] {
            assert!(req.query.iter().any(|(k, _)| k == key), "缺少 {key}");
        }
        assert!(req.query.iter().any(|(k, v)| k == "page" && v == "2"));
        assert!(
            !req.query.iter().any(|(k, _)| k == "index_type"),
            "未给值的可选参数不该出现"
        );
    }

    #[test]
    fn index_request_passes_filters_through_and_fixes_pagesize() {
        let params = IndexParams {
            page: 3,
            filters: vec![("area".to_owned(), "2".to_owned())],
            season_type: Some(1),
            ..IndexParams::default()
        };
        let req = build_index_request(&params);
        assert!(req.query.iter().any(|(k, v)| k == "area" && v == "2"));
        assert!(req.query.iter().any(|(k, v)| k == "pagesize" && v == "21"));
        assert!(req.query.iter().any(|(k, v)| k == "page" && v == "3"));
    }
}

// ------------------------------------------------------ 番剧详情（第二批）

/// 番剧详情的请求参数。
///
/// `season_id` 与 `ep_id` 至少要有一个：基线把两个都声明成 `dynamic` 并
/// 用「null 则不写入」跳过，两个都缺时请求照发，换回一个业务错误码，
/// 从调用链上很难回溯到「两个 id 都没取到」。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SeasonDetailParams {
    pub season_id: Option<i64>,
    pub ep_id: Option<i64>,
}

impl SeasonDetailParams {
    const fn identifies_a_season(self) -> bool {
        self.season_id.is_some() || self.ep_id.is_some()
    }
}

/// 番剧详情走 `pgcInfo`，课堂详情走 `pugvInfo`。
///
/// 两条接口的响应结构相同，但**载荷键不同**：前者在 `result`，后者在
/// `data`。这正是第一批给 [`crate::envelope`] 加 `unwrap_result_envelope`
/// 的那组差异之一。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeasonKind {
    /// 番剧 / 国创 / 电影，载荷在 `result`。
    Pgc,
    /// 课堂，载荷在 `data`。
    Pugv,
}

impl SeasonKind {
    #[must_use]
    pub const fn path(self) -> &'static str {
        match self {
            Self::Pgc => path::SEASON_DETAIL,
            Self::Pugv => path::PUGV_SEASON_DETAIL,
        }
    }
}

/// 构造番剧 / 课堂详情请求。
///
/// 返回 `None` 表示两个 id 都没给，见 [`SeasonDetailParams`]。
#[must_use]
pub fn build_season_detail_request(
    kind: SeasonKind,
    params: SeasonDetailParams,
) -> Option<PreparedRequest> {
    if !params.identifies_a_season() {
        return None;
    }
    let mut query = Vec::with_capacity(2);
    push_opt(&mut query, "season_id", params.season_id);
    push_opt(&mut query, "ep_id", params.ep_id);
    Some(PreparedRequest {
        path: kind.path(),
        query,
    })
}

#[derive(Deserialize)]
struct SeasonDetailDto {
    #[serde(default)]
    actors: Option<String>,
    #[serde(default)]
    areas: Vec<NamedDto>,
    #[serde(default)]
    cover: Option<String>,
    #[serde(default)]
    episodes: Vec<EpisodeDto>,
    #[serde(default)]
    evaluate: Option<String>,
    #[serde(default)]
    media_id: Option<i64>,
    #[serde(default)]
    new_ep: Option<NewEpDto>,
    #[serde(default)]
    publish: Option<PublishDto>,
    #[serde(default)]
    rating: Option<RatingDto>,
    #[serde(default)]
    season_id: Option<i64>,
    #[serde(default)]
    season_title: Option<String>,
    /// 单数。写成 `sections` 会静默拿不到花絮那一组。
    #[serde(default)]
    section: Vec<SectionDto>,
    #[serde(default)]
    stat: Option<SeasonStatDto>,
    #[serde(default)]
    subtitle: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default, rename = "type")]
    kind: Option<i64>,
    #[serde(default)]
    up_info: Option<UpInfoDto>,
    #[serde(default)]
    user_status: Option<UserStatusDto>,
    #[serde(default)]
    cooperators: Vec<CooperatorDto>,
    #[serde(default)]
    brief: Option<BriefDto>,
    #[serde(default)]
    seasons: Vec<SeasonBriefDto>,
}

#[derive(Deserialize)]
struct NamedDto {
    #[serde(default)]
    name: Option<String>,
}

#[derive(Deserialize)]
struct NewEpDto {
    #[serde(default)]
    desc: Option<String>,
    #[serde(default)]
    title: Option<String>,
}

#[derive(Deserialize)]
struct PublishDto {
    #[serde(default)]
    pub_time_show: Option<String>,
}

#[derive(Deserialize)]
struct RatingDto {
    #[serde(default)]
    score: Option<f64>,
}

#[derive(Deserialize)]
struct DimensionDto {
    #[serde(default)]
    width: Option<i64>,
    #[serde(default)]
    height: Option<i64>,
    #[serde(default)]
    rotate: Option<i64>,
}

#[derive(Deserialize)]
struct EpisodeDto {
    #[serde(default)]
    id: Option<i64>,
    #[serde(default)]
    aid: Option<i64>,
    #[serde(default)]
    cid: Option<i64>,
    #[serde(default)]
    ep_id: Option<i64>,
    #[serde(default)]
    bvid: Option<String>,
    #[serde(default)]
    badge: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    cover: Option<String>,
    #[serde(default)]
    dimension: Option<DimensionDto>,
    #[serde(default)]
    duration: Option<i64>,
    #[serde(default)]
    from: Option<String>,
    #[serde(default)]
    link: Option<String>,
    #[serde(default)]
    long_title: Option<String>,
    #[serde(default)]
    pub_time: Option<i64>,
    /// 基线取的是 `pub_time ?? release_date`：两个键表示同一件事。
    #[serde(default)]
    release_date: Option<i64>,
    #[serde(default)]
    share_copy: Option<String>,
    #[serde(default)]
    share_url: Option<String>,
    #[serde(default)]
    show_title: Option<String>,
    #[serde(default)]
    play: Option<i64>,
}

#[derive(Deserialize)]
struct SectionDto {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    episodes: Vec<EpisodeDto>,
}

#[derive(Deserialize)]
struct SeasonStatDto {
    #[serde(default)]
    coins: Option<i64>,
    #[serde(default)]
    favorite: Option<i64>,
    #[serde(default)]
    likes: Option<i64>,
    #[serde(default)]
    danmakus: Option<i64>,
    #[serde(default)]
    reply: Option<i64>,
    #[serde(default)]
    share: Option<i64>,
    #[serde(default)]
    views: Option<i64>,
}

#[derive(Deserialize)]
struct UpInfoDto {
    #[serde(default)]
    mid: Option<i64>,
    #[serde(default)]
    avatar: Option<String>,
    #[serde(default)]
    uname: Option<String>,
}

#[derive(Deserialize)]
struct UserProgressDto {
    #[serde(default)]
    last_ep_id: Option<i64>,
}

#[derive(Deserialize)]
struct UserStatusDto {
    #[serde(default)]
    favored: Option<i64>,
    #[serde(default)]
    progress: Option<UserProgressDto>,
}

#[derive(Deserialize)]
struct CooperatorDto {
    #[serde(default)]
    mid: Option<i64>,
    #[serde(default)]
    avatar: Option<String>,
    #[serde(default)]
    nick_name: Option<String>,
    #[serde(default)]
    role: Option<String>,
}

#[derive(Deserialize)]
struct BriefImageDto {
    #[serde(default)]
    aspect_ratio: Option<f64>,
    #[serde(default)]
    url: Option<String>,
}

#[derive(Deserialize)]
struct BriefDto {
    #[serde(default)]
    img: Vec<BriefImageDto>,
}

#[derive(Deserialize)]
struct SeasonBriefDto {
    #[serde(default)]
    media_id: Option<i64>,
    #[serde(default)]
    season_id: Option<i64>,
    #[serde(default)]
    season_title: Option<String>,
}

impl EpisodeDto {
    fn into_domain(self) -> PgcEpisode {
        let (width, height) = self.dimension.map_or((None, None), |d| {
            bili_domain::pgc::apply_rotate(d.width, d.height, d.rotate)
        });
        PgcEpisode {
            id: self.id,
            aid: self.aid,
            cid: self.cid,
            ep_id: self.ep_id,
            bvid: self.bvid,
            badge: self.badge,
            title: self.title,
            cover: self.cover,
            width,
            height,
            duration: self.duration,
            from: self.from,
            link: self.link,
            long_title: self.long_title,
            pub_time: self.pub_time.or(self.release_date),
            share_copy: self.share_copy,
            share_url: self.share_url,
            show_title: self.show_title,
            play: self.play,
        }
    }
}

fn season_detail_from(dto: SeasonDetailDto) -> PgcSeasonDetail {
    PgcSeasonDetail {
        actors: dto.actors,
        areas: dto.areas.into_iter().filter_map(|a| a.name).collect(),
        cover: dto.cover,
        episodes: dto
            .episodes
            .into_iter()
            .map(EpisodeDto::into_domain)
            .collect(),
        evaluate: dto.evaluate,
        media_id: dto.media_id,
        new_ep_desc: dto.new_ep.as_ref().and_then(|e| e.desc.clone()),
        new_ep_title: dto.new_ep.and_then(|e| e.title),
        pub_time_show: dto.publish.and_then(|p| p.pub_time_show),
        rating_score: dto.rating.and_then(|r| r.score),
        season_id: dto.season_id,
        season_title: dto.season_title,
        sections: dto
            .section
            .into_iter()
            .map(|s| PgcSection {
                title: s.title,
                episodes: s
                    .episodes
                    .into_iter()
                    .map(EpisodeDto::into_domain)
                    .collect(),
            })
            .collect(),
        stat: dto.stat.map(|s| PgcSeasonStat {
            coin: s.coins.unwrap_or_default(),
            favorite: s.favorite.unwrap_or_default(),
            like: s.likes.unwrap_or_default(),
            danmaku: s.danmakus,
            reply: s.reply,
            share: s.share,
            view: s.views,
        }),
        subtitle: dto.subtitle,
        title: dto.title,
        kind: dto.kind,
        up: dto.up_info.map(|u| PgcUpInfo {
            mid: u.mid,
            avatar: u.avatar,
            uname: u.uname,
        }),
        user_status: dto.user_status.map(|u| PgcUserStatus {
            favored: u.favored,
            last_ep_id: u.progress.and_then(|p| p.last_ep_id),
        }),
        cooperators: dto
            .cooperators
            .into_iter()
            .map(|c| PgcCooperator {
                mid: c.mid,
                avatar: c.avatar,
                nick_name: c.nick_name,
                role: c.role,
            })
            .collect(),
        brief_images: dto
            .brief
            .map(|b| {
                b.img
                    .into_iter()
                    .map(|i| PgcBriefImage {
                        aspect_ratio: i.aspect_ratio.unwrap_or(1.0),
                        url: i.url,
                    })
                    .collect()
            })
            .unwrap_or_default(),
        seasons: dto
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

/// 解析番剧 / 课堂详情。载荷键按 `kind` 分流。
///
/// # Errors
/// 信封解包失败或结构不符。
pub fn parse_season_detail(body: &str, kind: SeasonKind) -> Result<PgcSeasonDetail, DomainError> {
    let dto: SeasonDetailDto = match kind {
        SeasonKind::Pgc => unwrap_result_envelope(body)?,
        SeasonKind::Pugv => unwrap_envelope(body)?,
    };
    Ok(season_detail_from(dto))
}

/// 取番剧 / 课堂详情。
///
/// # Errors
/// 参数不足、传输、状态码、信封或解析失败。
pub async fn fetch_season_detail<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    kind: SeasonKind,
    params: SeasonDetailParams,
) -> Result<PgcSeasonDetail, DomainError> {
    let Some(request) = build_season_detail_request(kind, params) else {
        return Err(DomainError::new(
            bili_domain::error::ErrorKind::Protocol,
            "season_id 与 ep_id 至少要有一个",
        ));
    };
    let body = send(transport, ctx, &request).await?;
    parse_season_detail(&body, kind)
}

#[cfg(test)]
// 测试里 expect 是正确做法：前提不成立时就该崩掉并指出原因。
#[allow(clippy::expect_used)]
mod season_detail_tests {
    use super::*;

    fn pgc(body: &str) -> PgcSeasonDetail {
        parse_season_detail(body, SeasonKind::Pgc).expect("样本应解析成功")
    }

    #[test]
    fn payload_key_differs_between_the_two_endpoints() {
        let in_result = r#"{"code":0,"message":"0","result":{"title":"甲"}}"#;
        let in_data = r#"{"code":0,"message":"0","data":{"title":"甲"}}"#;

        assert_eq!(pgc(in_result).title.as_deref(), Some("甲"));
        assert_eq!(
            parse_season_detail(in_data, SeasonKind::Pugv)
                .expect("课堂详情在 data 里")
                .title
                .as_deref(),
            Some("甲")
        );

        // 反向：读错键必须报错而不是静默返回一个空壳。空壳在界面上是
        // 「这一季什么都没有」，比报错难查得多。
        assert!(parse_season_detail(in_data, SeasonKind::Pgc).is_err());
        assert!(parse_season_detail(in_result, SeasonKind::Pugv).is_err());
    }

    #[test]
    fn sections_come_from_the_singular_key() {
        let body = r#"{"code":0,"message":"0","result":{
            "section":[{"title":"PV","episodes":[{"ep_id":1}]}],
            "sections":[{"title":"写成复数就拿不到","episodes":[{"ep_id":2}]}]
        }}"#;
        let detail = pgc(body);
        assert_eq!(detail.sections.len(), 1);
        assert_eq!(detail.sections[0].title.as_deref(), Some("PV"));
        assert_eq!(detail.sections[0].episodes[0].ep_id, Some(1));
    }

    #[test]
    fn rotate_one_swaps_width_and_height() {
        let body = r#"{"code":0,"message":"0","result":{"episodes":[
            {"ep_id":1,"dimension":{"width":1920,"height":1080,"rotate":0}},
            {"ep_id":2,"dimension":{"width":1920,"height":1080,"rotate":1}},
            {"ep_id":3,"dimension":{"width":1920,"height":1080}}
        ]}}"#;
        let eps = pgc(body).episodes;
        assert_eq!((eps[0].width, eps[0].height), (Some(1920), Some(1080)));
        assert_eq!(
            (eps[1].width, eps[1].height),
            (Some(1080), Some(1920)),
            "rotate==1 时响应里的宽高是交换过的"
        );
        assert_eq!((eps[2].width, eps[2].height), (Some(1920), Some(1080)));
    }

    #[test]
    fn pub_time_falls_back_to_release_date() {
        let body = r#"{"code":0,"message":"0","result":{"episodes":[
            {"ep_id":1,"pub_time":100},
            {"ep_id":2,"release_date":200},
            {"ep_id":3,"pub_time":300,"release_date":400},
            {"ep_id":4}
        ]}}"#;
        let eps = pgc(body).episodes;
        assert_eq!(eps[0].pub_time, Some(100));
        assert_eq!(eps[1].pub_time, Some(200));
        assert_eq!(eps[2].pub_time, Some(300), "两个都有时以 pub_time 为准");
        assert_eq!(eps[3].pub_time, None);
    }

    /// 基线 `PgcStat.fromJson` 对七个字段的处理不对称，而且键名有单复数
    /// 之分。统一成 0 会让界面「缺失时整块隐藏」的逻辑失效。
    #[test]
    fn stat_defaults_are_asymmetric_and_keys_are_plural() {
        let full = r#"{"code":0,"message":"0","result":{"stat":{
            "coins":1,"favorite":2,"likes":3,"danmakus":4,"reply":5,"share":6,"views":7
        }}}"#;
        let s = pgc(full).stat.expect("有 stat");
        assert_eq!((s.coin, s.favorite, s.like), (1, 2, 3));
        assert_eq!(
            (s.danmaku, s.reply, s.share, s.view),
            (Some(4), Some(5), Some(6), Some(7))
        );

        let empty = r#"{"code":0,"message":"0","result":{"stat":{}}}"#;
        let s = pgc(empty).stat.expect("有 stat");
        assert_eq!((s.coin, s.favorite, s.like), (0, 0, 0), "这三个缺失取 0");
        assert_eq!(
            (s.danmaku, s.reply, s.share, s.view),
            (None, None, None, None),
            "这四个缺失保持 null，界面据此整块隐藏"
        );

        // 单数键拼成复数（或反之）都拿不到值——这组键名没有规律，
        // 只能逐个对着响应抄。
        let wrong = r#"{"code":0,"message":"0","result":{"stat":{
            "coin":1,"favorites":2,"like":3,"danmaku":4,"views":7
        }}}"#;
        let s = pgc(wrong).stat.expect("有 stat");
        assert_eq!((s.coin, s.favorite, s.like), (0, 0, 0));
        assert_eq!(s.danmaku, None);
        assert_eq!(s.view, Some(7));
    }

    #[test]
    fn single_field_wrappers_are_flattened() {
        let body = r#"{"code":0,"message":"0","result":{
            "areas":[{"name":"日本"},{"name":"中国"},{}],
            "new_ep":{"desc":"全12话","title":"12"},
            "publish":{"pub_time_show":"2026-01-01 24:00"},
            "rating":{"score":9.7},
            "brief":{"img":[{"aspect_ratio":1.5,"url":"u1"},{"url":"u2"}]},
            "user_status":{"favored":1,"progress":{"last_ep_id":42}}
        }}"#;
        let d = pgc(body);
        assert_eq!(
            d.areas,
            vec!["日本".to_owned(), "中国".to_owned()],
            "没有 name 的条目丢弃"
        );
        assert_eq!(d.new_ep_desc.as_deref(), Some("全12话"));
        assert_eq!(d.new_ep_title.as_deref(), Some("12"));
        assert_eq!(d.pub_time_show.as_deref(), Some("2026-01-01 24:00"));
        assert!((d.rating_score.expect("有评分") - 9.7).abs() < f64::EPSILON);
        assert_eq!(d.brief_images.len(), 2);
        assert!(
            (d.brief_images[1].aspect_ratio - 1.0).abs() < f64::EPSILON,
            "缺失取 1，不是 0"
        );
        let us = d.user_status.expect("有 user_status");
        assert_eq!((us.favored, us.last_ep_id), (Some(1), Some(42)));
    }

    #[test]
    fn missing_ids_are_rejected_before_sending() {
        assert!(
            build_season_detail_request(SeasonKind::Pgc, SeasonDetailParams::default()).is_none()
        );

        let by_season = build_season_detail_request(
            SeasonKind::Pgc,
            SeasonDetailParams {
                season_id: Some(1),
                ep_id: None,
            },
        )
        .expect("有 season_id 就够");
        assert_eq!(by_season.path, path::SEASON_DETAIL);
        assert_eq!(by_season.query_string(), "season_id=1");

        let by_ep = build_season_detail_request(
            SeasonKind::Pugv,
            SeasonDetailParams {
                season_id: None,
                ep_id: Some(2),
            },
        )
        .expect("有 ep_id 也够");
        assert_eq!(by_ep.path, path::PUGV_SEASON_DETAIL);
        assert_eq!(by_ep.query_string(), "ep_id=2");
    }

    #[test]
    fn an_empty_season_is_not_an_error() {
        let d = pgc(r#"{"code":0,"message":"0","result":{}}"#);
        assert!(d.episodes.is_empty());
        assert!(d.sections.is_empty());
        assert!(d.seasons.is_empty());
        assert!(d.cooperators.is_empty());
        assert!(d.brief_images.is_empty());
        assert!(d.stat.is_none());
    }

    #[test]
    fn remote_error_code_is_carried_through() {
        let body = r#"{"code":-404,"message":"啥都木有","result":null}"#;
        let err = parse_season_detail(body, SeasonKind::Pgc).expect_err("业务错误码必须变成错误");
        assert_eq!(err.code(), Some(-404));
    }
}
