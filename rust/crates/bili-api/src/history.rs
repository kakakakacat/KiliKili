//! 历史记录、稍后再看与播放列表（S7 / issue #57）。
//!
//! 对应基线 `lib/http/user.dart` 剩下的十一条：观看历史的读/搜/删/清空/
//! 暂停开关，稍后再看的读/加/删/清空，以及播放列表。
//!
//! ## 为什么和「我的」那一族分开
//!
//! 它们的基线模型（`HistoryItemModel` / `LaterItemModel`）里混着返回
//! Widget 的 getter 与多选用的 `checked` 字段，迁移要连同几个列表页的 item
//! widget 一起改。协议侧其实没有特别之处，分开只是为了让那一批界面改动
//! 单独可读。
//!
//! ## 历史相关接口走的是另一个账号
//!
//! 基线对这一族显式传 `Accounts.history`（`lib/http/user.dart` 的
//! `options: Options(extra: {'account': account ?? Accounts.history})`），
//! **不是** `ApiType.apiTypeSet` 查出来的那个。Rust 侧不认账号，由 Dart
//! 在 `RequestContext` 里给对应的 Cookie 与 csrf——迁移时最容易丢的就是
//! 这一条（AGENTS.md §3）。

use bili_domain::error::DomainError;
use bili_domain::user::{
    HistoryItem, HistoryPage, HistoryRef, HistoryTab, LaterItem, LaterPage, MediaList,
    MediaListItem,
};
use serde::Deserialize;

use crate::http::{HttpTransport, Idempotency, RequestContext};
use crate::request::{PreparedRequest, PreparedWrite};

/// 接口路径。与基线 `lib/http/api.dart` 保持一致。
pub mod path {
    pub const HISTORY_LIST: &str = "/x/web-interface/history/cursor";
    pub const SEARCH_HISTORY: &str = "/x/web-interface/history/search";
    /// 基线的常量里带着 `?jsonp=jsonp`，这里拆开：查询参数由
    /// [`history_status_request`] 给，路径就是路径。
    pub const HISTORY_STATUS: &str = "/x/v2/history/shadow";
    pub const PAUSE_HISTORY: &str = "/x/v2/history/shadow/set";
    pub const CLEAR_HISTORY: &str = "/x/v2/history/clear";
    pub const DEL_HISTORY: &str = "/x/v2/history/delete";

    pub const SEE_YOU_LATER: &str = "/x/v2/history/toview/web";
    pub const TO_VIEW_LATER: &str = "/x/v2/history/toview/add";
    pub const TO_VIEW_DEL: &str = "/x/v2/history/toview/v2/dels";
    pub const TO_VIEW_CLEAR: &str = "/x/v2/history/toview/clear";

    pub const MEDIA_LIST: &str = "/x/v2/medialist/resource/list";
}

// ------------------------------------------------------------ 历史：只读

/// 一页观看历史的请求。
///
/// `max` 与 `view_at` 是游标，首页传 0——基线用的是 `max ?? 0`，不是省略。
#[must_use]
pub fn history_list_request(
    kind: &str,
    page_size: i64,
    max: Option<i64>,
    view_at: Option<i64>,
) -> PreparedRequest {
    PreparedRequest {
        path: path::HISTORY_LIST,
        query: vec![
            ("type".to_owned(), kind.to_owned()),
            ("ps".to_owned(), page_size.to_string()),
            ("max".to_owned(), max.unwrap_or(0).to_string()),
            ("view_at".to_owned(), view_at.unwrap_or(0).to_string()),
        ],
    }
}

/// 搜索历史记录的请求。`business=all` 是基线写死的。
#[must_use]
pub fn search_history_request(page: i64, keyword: &str) -> PreparedRequest {
    PreparedRequest {
        path: path::SEARCH_HISTORY,
        query: vec![
            ("pn".to_owned(), page.to_string()),
            ("keyword".to_owned(), keyword.to_owned()),
            ("business".to_owned(), "all".to_owned()),
        ],
    }
}

/// 历史记录是否已暂停的请求。
#[must_use]
pub fn history_status_request() -> PreparedRequest {
    PreparedRequest {
        path: path::HISTORY_STATUS,
        query: vec![("jsonp".to_owned(), "jsonp".to_owned())],
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct HistoryPageDto {
    tab: Option<Vec<HistoryTabDto>>,
    list: Option<Vec<HistoryItemDto>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct HistoryTabDto {
    #[serde(rename = "type")]
    kind: Option<String>,
    name: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct HistoryItemDto {
    title: Option<String>,
    cover: Option<String>,
    covers: Option<Vec<String>>,
    uri: Option<String>,
    history: Option<HistoryRefDto>,
    videos: Option<i64>,
    author_name: Option<String>,
    author_mid: Option<i64>,
    view_at: Option<i64>,
    progress: Option<i64>,
    badge: Option<String>,
    show_title: Option<String>,
    duration: Option<i64>,
    is_fav: Option<i64>,
    kid: Option<i64>,
    tag_name: Option<String>,
    live_status: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct HistoryRefDto {
    oid: Option<i64>,
    epid: Option<i64>,
    bvid: Option<String>,
    page: Option<i64>,
    cid: Option<i64>,
    business: Option<String>,
}

/// 解析一页观看历史。搜索历史走的是同一个形状。
///
/// # Errors
/// 信封失败沿用统一分类。`list` 缺失按空列表处理：翻到底与这一页没有数据
/// 在基线里都是 `list == null`，不是错误。
pub fn parse_history_page(body: &str) -> Result<HistoryPage, DomainError> {
    let dto: HistoryPageDto = crate::envelope::unwrap_envelope(body)?;
    Ok(HistoryPage {
        tabs: dto
            .tab
            .unwrap_or_default()
            .into_iter()
            .map(|t| HistoryTab {
                kind: t.kind,
                name: t.name,
            })
            .collect(),
        list: dto
            .list
            .unwrap_or_default()
            .into_iter()
            .map(|i| HistoryItem {
                title: i.title,
                cover: i.cover,
                covers: i.covers.unwrap_or_default(),
                uri: i.uri,
                videos: i.videos,
                author_name: i.author_name,
                author_mid: i.author_mid,
                view_at: i.view_at,
                progress: i.progress,
                badge: i.badge,
                show_title: i.show_title,
                duration: i.duration,
                is_fav: i.is_fav,
                kid: i.kid,
                tag_name: i.tag_name,
                live_status: i.live_status,
                history: i.history.map_or_else(HistoryRef::default, |h| HistoryRef {
                    oid: h.oid,
                    epid: h.epid,
                    bvid: h.bvid,
                    page: h.page,
                    // 0 不是合法 cid，照搬会让播放器拿它去请求。见实体文档。
                    cid: h.cid.filter(|&c| c != 0),
                    business: h.business,
                }),
            })
            .collect(),
    })
}

/// 解析「历史记录是否已暂停」。
///
/// # Errors
/// 信封失败沿用统一分类。`data` 直接就是一个布尔值。
pub fn parse_history_status(body: &str) -> Result<bool, DomainError> {
    crate::envelope::unwrap_envelope(body)
}

/// 取一页观看历史。
///
/// # Errors
/// 传输、状态码、信封三类失败，见各自的文档。
pub async fn fetch_history_list<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    kind: &str,
    page_size: i64,
    max: Option<i64>,
    view_at: Option<i64>,
) -> Result<HistoryPage, DomainError> {
    let req = history_list_request(kind, page_size, max, view_at);
    let body = crate::client::send(transport, ctx, &req).await?;
    parse_history_page(&body)
}

/// 搜索历史记录。
///
/// # Errors
/// 同 [`fetch_history_list`]。
pub async fn fetch_search_history<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    page: i64,
    keyword: &str,
) -> Result<HistoryPage, DomainError> {
    let req = search_history_request(page, keyword);
    let body = crate::client::send(transport, ctx, &req).await?;
    parse_history_page(&body)
}

/// 取「历史记录是否已暂停」。
///
/// # Errors
/// 同 [`fetch_history_list`]。
pub async fn fetch_history_status<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
) -> Result<bool, DomainError> {
    let body = crate::client::send(transport, ctx, &history_status_request()).await?;
    parse_history_status(&body)
}

// ------------------------------------------------------------- 历史：写

/// 暂停/恢复记录历史。`paused` 为 true 表示暂停。
///
/// 置为语义，重发安全。`jsonp=jsonp` 是基线写死的老参数。
#[must_use]
pub fn pause_history_request(paused: bool) -> PreparedWrite {
    PreparedWrite::form_only(
        path::PAUSE_HISTORY,
        vec![
            ("switch".to_owned(), paused.to_string()),
            ("jsonp".to_owned(), "jsonp".to_owned()),
        ],
        Idempotency::Idempotent,
    )
}

/// 清空全部历史记录。
///
/// 定级 [`Idempotency::Unsafe`]：**不可逆**。重发本身落到同一个状态（都是
/// 空的），但传输层的自动重发意味着「用户以为失败、实际已经清空」，而这条
/// 没有撤销。按 ADR-013，不可逆的一律 Unsafe。
#[must_use]
pub fn clear_history_request() -> PreparedWrite {
    PreparedWrite::form_only(
        path::CLEAR_HISTORY,
        vec![("jsonp".to_owned(), "jsonp".to_owned())],
        Idempotency::Unsafe,
    )
}

/// 删除一条历史记录。`kid` 形如 `archive_123456`，由调用方拼好。
///
/// 重发打到同一个 kid，结果仍是「它不在了」，所以是幂等。
#[must_use]
pub fn del_history_request(kid: &str) -> PreparedWrite {
    PreparedWrite::form_only(
        path::DEL_HISTORY,
        vec![
            ("kid".to_owned(), kid.to_owned()),
            ("jsonp".to_owned(), "jsonp".to_owned()),
        ],
        Idempotency::Idempotent,
    )
}

/// 暂停/恢复记录历史。
///
/// # Errors
/// 传输、状态码、信封三类失败。
pub async fn pause_history<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    paused: bool,
) -> Result<(), DomainError> {
    let body = crate::client::send_write(transport, ctx, &pause_history_request(paused)).await?;
    crate::envelope::check_envelope(&body)
}

/// 清空历史记录。
///
/// # Errors
/// 传输、状态码、信封三类失败。
pub async fn clear_history<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
) -> Result<(), DomainError> {
    let body = crate::client::send_write(transport, ctx, &clear_history_request()).await?;
    crate::envelope::check_envelope(&body)
}

/// 删除一条历史记录。
///
/// # Errors
/// 传输、状态码、信封三类失败。
pub async fn del_history<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    kid: &str,
) -> Result<(), DomainError> {
    let body = crate::client::send_write(transport, ctx, &del_history_request(kid)).await?;
    crate::envelope::check_envelope(&body)
}

// --------------------------------------------------------- 稍后再看：只读

/// 一页稍后再看的请求。WBI 签名。
///
/// `need_split=true` 与 `web_location=333.881` 是基线写死的固定值。
#[must_use]
pub fn see_you_later_request(
    page: i64,
    page_size: i64,
    viewed: i64,
    keyword: &str,
    asc: bool,
    mixin_key: &str,
    wts: i64,
) -> PreparedRequest {
    let mut query = vec![
        ("pn".to_owned(), page.to_string()),
        ("ps".to_owned(), page_size.to_string()),
        ("viewed".to_owned(), viewed.to_string()),
        ("key".to_owned(), keyword.to_owned()),
        ("asc".to_owned(), asc.to_string()),
        ("need_split".to_owned(), "true".to_owned()),
        ("web_location".to_owned(), "333.881".to_owned()),
        ("wts".to_owned(), wts.to_string()),
    ];
    query.sort_by(|a, b| a.0.cmp(&b.0));
    let w_rid = bili_protocol::wbi::sign(&query, mixin_key);
    query.push(("w_rid".to_owned(), w_rid));
    PreparedRequest {
        path: path::SEE_YOU_LATER,
        query,
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct LaterPageDto {
    count: Option<i64>,
    list: Option<Vec<LaterItemDto>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct LaterItemDto {
    aid: Option<i64>,
    bvid: Option<String>,
    pic: Option<String>,
    title: Option<String>,
    pubdate: Option<i64>,
    duration: Option<i64>,
    redirect_url: Option<String>,
    rights: Option<RightsDto>,
    owner: Option<OwnerDto>,
    stat: Option<StatDto>,
    bangumi: Option<BangumiDto>,
    cid: Option<i64>,
    progress: Option<i64>,
    is_pgc: Option<bool>,
    pgc_label: Option<String>,
    is_pugv: Option<bool>,
    charging_pay: Option<ChargingPayDto>,
    dimension: Option<DimensionDto>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RightsDto {
    is_cooperation: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct OwnerDto {
    mid: Option<i64>,
    name: Option<String>,
    face: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct StatDto {
    view: Option<i64>,
    danmaku: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct BangumiDto {
    ep_id: Option<i64>,
    season: Option<SeasonDto>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SeasonDto {
    title: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ChargingPayDto {
    level: Option<serde_json::Value>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct DimensionDto {
    width: Option<i64>,
    height: Option<i64>,
    rotate: Option<i64>,
}

/// 解析一页稍后再看。
///
/// # Errors
/// 信封失败沿用统一分类。`list` 缺失按空列表处理。
///
/// 两处规则照搬基线，都不是渲染：
/// - `subtitle` 是把季名前缀从标题里切掉之后的分集名，只对番剧条目有；
/// - `is_charging` 的判据是 `charging_pay.level` **存在与否**，不是某个
///   布尔字段——服务端对非充电视频整个 `charging_pay` 都不给。
pub fn parse_later_page(body: &str) -> Result<LaterPage, DomainError> {
    let dto: LaterPageDto = crate::envelope::unwrap_envelope(body)?;
    Ok(LaterPage {
        count: dto.count,
        list: dto
            .list
            .unwrap_or_default()
            .into_iter()
            .map(|i| {
                let season_title = i
                    .bangumi
                    .as_ref()
                    .and_then(|b| b.season.as_ref().and_then(|s| s.title.clone()));
                let subtitle = match (&season_title, &i.title) {
                    (Some(season), Some(title)) => {
                        Some(title.replacen(&format!("{season} "), "", 1))
                    }
                    _ => None,
                };
                LaterItem {
                    aid: i.aid,
                    bvid: i.bvid,
                    pic: i.pic,
                    title: i.title,
                    subtitle,
                    pubdate: i.pubdate,
                    duration: i.duration,
                    redirect_url: i.redirect_url,
                    is_cooperation: i.rights.and_then(|r| r.is_cooperation),
                    owner_mid: i.owner.as_ref().and_then(|o| o.mid),
                    owner_name: i.owner.as_ref().and_then(|o| o.name.clone()),
                    owner_face: i.owner.and_then(|o| o.face),
                    view: i.stat.as_ref().and_then(|s| s.view),
                    danmaku: i.stat.and_then(|s| s.danmaku),
                    ep_id: i.bangumi.and_then(|b| b.ep_id),
                    season_title,
                    cid: i.cid,
                    progress: i.progress,
                    is_pgc: i.is_pgc,
                    // 空串当没有角标，与基线一致。
                    pgc_label: i.pgc_label.filter(|s| !s.is_empty()),
                    is_pugv: i.is_pugv,
                    is_charging: i
                        .charging_pay
                        .is_some_and(|c| c.level.is_some_and(|v| !v.is_null())),
                    // rotate == 1 时宽高对调，照基线 `Dimension.fromJson`。
                    width: i.dimension.as_ref().and_then(|d| {
                        if d.rotate == Some(1) {
                            d.height
                        } else {
                            d.width
                        }
                    }),
                    height: i.dimension.as_ref().and_then(|d| {
                        if d.rotate == Some(1) {
                            d.width
                        } else {
                            d.height
                        }
                    }),
                }
            })
            .collect(),
    })
}

/// 取一页稍后再看。
///
/// # Errors
/// 同 [`fetch_history_list`]。
#[allow(clippy::too_many_arguments)]
pub async fn fetch_see_you_later<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    page: i64,
    page_size: i64,
    viewed: i64,
    keyword: &str,
    asc: bool,
    mixin_key: &str,
    wts: i64,
) -> Result<LaterPage, DomainError> {
    let req = see_you_later_request(page, page_size, viewed, keyword, asc, mixin_key, wts);
    let body = crate::client::send(transport, ctx, &req).await?;
    parse_later_page(&body)
}

// ---------------------------------------------------------- 稍后再看：写

/// 加入稍后再看。`aid` 与 `bvid` 至少给一个。
///
/// 置为语义（同一条加两次还是一条），重发安全。
#[must_use]
pub fn to_view_later_request(aid: Option<i64>, bvid: Option<&str>) -> PreparedWrite {
    let mut form = Vec::with_capacity(2);
    if let Some(aid) = aid {
        form.push(("aid".to_owned(), aid.to_string()));
    }
    if let Some(bvid) = bvid {
        form.push(("bvid".to_owned(), bvid.to_owned()));
    }
    PreparedWrite::form_only(path::TO_VIEW_LATER, form, Idempotency::Idempotent)
}

/// 从稍后再看移除。`resources` 是逗号分隔的 aid 串。
///
/// 重发打到同一批 id，结果仍是「它们不在了」，所以是幂等。
#[must_use]
pub fn to_view_del_request(resources: &str) -> PreparedWrite {
    PreparedWrite::form_only(
        path::TO_VIEW_DEL,
        vec![("resources".to_owned(), resources.to_owned())],
        Idempotency::Idempotent,
    )
}

/// 清空稍后再看。`clean_type`：省略＝全部，1＝失效的，2＝已看过的。
///
/// 定级 [`Idempotency::Unsafe`]：与清空历史同理，**不可逆**。
#[must_use]
pub fn to_view_clear_request(clean_type: Option<i64>) -> PreparedWrite {
    let mut form = Vec::with_capacity(1);
    if let Some(clean_type) = clean_type {
        form.push(("clean_type".to_owned(), clean_type.to_string()));
    }
    PreparedWrite::form_only(path::TO_VIEW_CLEAR, form, Idempotency::Unsafe)
}

/// 加入稍后再看。
///
/// # Errors
/// 传输、状态码、信封三类失败。
pub async fn to_view_later<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    aid: Option<i64>,
    bvid: Option<&str>,
) -> Result<(), DomainError> {
    let body = crate::client::send_write(transport, ctx, &to_view_later_request(aid, bvid)).await?;
    crate::envelope::check_envelope(&body)
}

/// 从稍后再看移除。
///
/// # Errors
/// 传输、状态码、信封三类失败。
pub async fn to_view_del<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    resources: &str,
) -> Result<(), DomainError> {
    let body = crate::client::send_write(transport, ctx, &to_view_del_request(resources)).await?;
    crate::envelope::check_envelope(&body)
}

/// 清空稍后再看。
///
/// # Errors
/// 传输、状态码、信封三类失败。
pub async fn to_view_clear<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    clean_type: Option<i64>,
) -> Result<(), DomainError> {
    let body =
        crate::client::send_write(transport, ctx, &to_view_clear_request(clean_type)).await?;
    crate::envelope::check_envelope(&body)
}

// ------------------------------------------------------------- 播放列表

/// 播放列表一页的请求。
///
/// `mobi_app=web`、`tid=0` 是基线写死的；`otype` 的取值 ugc 为 2、pgc 为 24。
#[must_use]
pub fn media_list_request(params: &MediaListParams) -> PreparedRequest {
    let mut query = vec![
        ("mobi_app".to_owned(), "web".to_owned()),
        ("type".to_owned(), params.kind.to_string()),
        ("biz_id".to_owned(), params.biz_id.to_string()),
    ];
    if let Some(oid) = params.oid {
        query.push(("oid".to_owned(), oid.to_string()));
    }
    if let Some(otype) = params.otype {
        query.push(("otype".to_owned(), otype.to_string()));
    }
    query.push(("ps".to_owned(), params.page_size.to_string()));
    query.push(("direction".to_owned(), params.direction.to_string()));
    query.push(("desc".to_owned(), params.desc.to_string()));
    query.push(("sort_field".to_owned(), params.sort_field.to_string()));
    query.push(("tid".to_owned(), "0".to_owned()));
    query.push(("with_current".to_owned(), params.with_current.to_string()));
    PreparedRequest {
        path: path::MEDIA_LIST,
        query,
    }
}

/// 播放列表请求的业务入参。
///
/// 做成结构而不是九个形参：它们**没有一个有自然顺序**（三个布尔连在一起），
/// 调用点写错两个的位置不会报错，只会取回另一段列表。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaListParams {
    pub kind: i64,
    pub biz_id: i64,
    pub oid: Option<i64>,
    /// ugc 为 2，pgc 为 24。
    pub otype: Option<i64>,
    pub page_size: i64,
    pub direction: bool,
    pub desc: bool,
    pub sort_field: i64,
    pub with_current: bool,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct MediaListDto {
    media_list: Option<Vec<MediaListItemDto>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct MediaListItemDto {
    id: Option<i64>,
    bv_id: Option<String>,
    title: Option<String>,
    intro: Option<String>,
    cover: Option<String>,
    duration: Option<i64>,
    badge: Option<BadgeDto>,
    #[serde(rename = "type")]
    kind: Option<i64>,
    upper: Option<OwnerDto>,
    cnt_info: Option<CntInfoDto>,
    pages: Option<Vec<PageDto>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct BadgeDto {
    text: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct CntInfoDto {
    play: Option<i64>,
    danmaku: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct PageDto {
    id: Option<i64>,
}

/// 解析播放列表。
///
/// # Errors
/// 信封失败沿用统一分类。`media_list` 缺失按空列表处理，与基线的
/// `?? <MediaListItemModel>[]` 一致。
///
/// 稿件 id 在这条接口里叫 `id`、bvid 叫 `bv_id`（别处是 `aid`/`bvid`）
/// ——这是服务端的事实，不是笔误。
pub fn parse_media_list(body: &str) -> Result<MediaList, DomainError> {
    let dto: MediaListDto = crate::envelope::unwrap_envelope(body)?;
    Ok(MediaList {
        list: dto
            .media_list
            .unwrap_or_default()
            .into_iter()
            .map(|i| MediaListItem {
                aid: i.id,
                bvid: i.bv_id,
                title: i.title,
                intro: i.intro,
                cover: i.cover,
                duration: i.duration,
                badge: i.badge.and_then(|b| b.text),
                kind: i.kind,
                upper_mid: i.upper.as_ref().and_then(|u| u.mid),
                upper_name: i.upper.as_ref().and_then(|u| u.name.clone()),
                upper_face: i.upper.and_then(|u| u.face),
                play: i.cnt_info.as_ref().and_then(|c| c.play),
                danmaku: i.cnt_info.and_then(|c| c.danmaku),
                first_page_cid: i.pages.and_then(|p| p.first().and_then(|x| x.id)),
            })
            .collect(),
    })
}

/// 取播放列表一页。
///
/// # Errors
/// 同 [`fetch_history_list`]。
pub async fn fetch_media_list<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    params: &MediaListParams,
) -> Result<MediaList, DomainError> {
    let body = crate::client::send(transport, ctx, &media_list_request(params)).await?;
    parse_media_list(&body)
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::{
        clear_history_request, history_list_request, parse_history_page, parse_history_status,
        parse_later_page, parse_media_list, see_you_later_request, to_view_clear_request,
        to_view_later_request,
    };
    use crate::http::Idempotency;

    #[test]
    fn history_cursor_defaults_to_zero_not_absent() {
        let q = history_list_request("archive", 20, None, None).query_string();
        assert_eq!(
            q, "type=archive&ps=20&max=0&view_at=0",
            "基线用的是 `max ?? 0`，不是省略 — {q}"
        );
    }

    #[test]
    fn a_zero_cid_is_normalised_away() {
        let page = parse_history_page(
            r#"{"code":0,"message":"0","data":{"list":[{"title":"甲","history":{"oid":1,"cid":0,"bvid":"BV1"}}]}}"#,
        )
        .expect("应解析成功");
        assert_eq!(
            page.list[0].history.cid, None,
            "0 不是合法 cid，照搬会让播放器拿它去请求"
        );
    }

    #[test]
    fn playback_progress_converts_seconds_and_resets_when_finished() {
        let page = parse_history_page(
            r#"{"code":0,"message":"0","data":{"list":[{"progress":90},{"progress":-1},{}]}}"#,
        )
        .expect("应解析成功");
        assert_eq!(
            page.list[0].playback_progress_ms(),
            Some(90_000),
            "秒转毫秒"
        );
        assert_eq!(
            page.list[1].playback_progress_ms(),
            Some(0),
            "已看完要从头播，不是跳到末尾"
        );
        assert_eq!(page.list[2].playback_progress_ms(), None);
    }

    #[test]
    fn history_status_is_a_bare_bool() {
        assert!(parse_history_status(r#"{"code":0,"message":"0","data":true}"#).expect("ok"));
        assert!(!parse_history_status(r#"{"code":0,"message":"0","data":false}"#).expect("ok"));
    }

    #[test]
    fn the_season_prefix_is_stripped_from_the_episode_title() {
        let page = parse_later_page(
            r#"{"code":0,"message":"0","data":{"list":[{"title":"示例番剧 第1话 开始","bangumi":{"ep_id":9,"season":{"title":"示例番剧"}}},{"title":"普通视频"}]}}"#,
        )
        .expect("应解析成功");
        assert_eq!(page.list[0].subtitle.as_deref(), Some("第1话 开始"));
        assert_eq!(page.list[0].season_title.as_deref(), Some("示例番剧"));
        assert_eq!(page.list[1].subtitle, None, "非番剧条目没有分集名");
    }

    #[test]
    fn charging_is_decided_by_the_presence_of_a_level() {
        let page = parse_later_page(
            r#"{"code":0,"message":"0","data":{"list":[{"charging_pay":{"level":1}},{"charging_pay":{}},{}]}}"#,
        )
        .expect("应解析成功");
        assert!(page.list[0].is_charging);
        assert!(!page.list[1].is_charging, "有 charging_pay 但没有 level");
        assert!(!page.list[2].is_charging, "整个 charging_pay 都没有");
    }

    #[test]
    fn a_rotated_dimension_is_swapped() {
        let page = parse_later_page(
            r#"{"code":0,"message":"0","data":{"list":[{"dimension":{"width":1920,"height":1080,"rotate":1}},{"dimension":{"width":1920,"height":1080,"rotate":0}}]}}"#,
        )
        .expect("应解析成功");
        assert_eq!(
            (page.list[0].width, page.list[0].height),
            (Some(1080), Some(1920)),
            "竖屏稿件要对调"
        );
        assert_eq!(
            (page.list[1].width, page.list[1].height),
            (Some(1920), Some(1080))
        );
    }

    #[test]
    fn an_empty_pgc_label_is_treated_as_absent() {
        let page =
            parse_later_page(r#"{"code":0,"message":"0","data":{"list":[{"pgc_label":""}]}}"#)
                .expect("应解析成功");
        assert_eq!(page.list[0].pgc_label, None, "空串当没有角标");
    }

    #[test]
    fn see_you_later_is_signed_and_carries_the_fixed_params() {
        let q = see_you_later_request(1, 20, 0, "", false, "key", 1_700_000_000).query_string();
        for expect in [
            "pn=1",
            "ps=20",
            "viewed=0",
            "key=",
            "asc=false",
            "need_split=true",
            "web_location=333.881",
        ] {
            assert!(q.contains(expect), "缺 {expect} — {q}");
        }
        assert!(q.contains("w_rid="), "必须签名 — {q}");
    }

    #[test]
    fn media_list_uses_id_and_bv_id_not_aid_and_bvid() {
        let list = parse_media_list(
            r#"{"code":0,"message":"0","data":{"media_list":[{"id":123,"bv_id":"BV1xx","badge":{"text":"充电专属"},"pages":[{"id":900},{"id":901}],"cnt_info":{"play":10,"danmaku":2}}]}}"#,
        )
        .expect("应解析成功");
        assert_eq!(list.list[0].aid, Some(123));
        assert_eq!(list.list[0].bvid.as_deref(), Some("BV1xx"));
        assert_eq!(list.list[0].badge.as_deref(), Some("充电专属"));
        assert_eq!(
            list.list[0].first_page_cid,
            Some(900),
            "基线取的是第一个分 P 的 id"
        );
        assert_eq!(list.list[0].play, Some(10));
    }

    #[test]
    fn the_two_clear_operations_are_unsafe() {
        assert_eq!(
            clear_history_request().idempotency,
            Idempotency::Unsafe,
            "不可逆：自动重发会让「以为失败、实际已清空」"
        );
        assert_eq!(to_view_clear_request(None).idempotency, Idempotency::Unsafe);
        assert_eq!(
            to_view_clear_request(None).form_string(),
            "",
            "不传 clean_type 表示全部清空，整条参数丢掉"
        );
        assert_eq!(to_view_clear_request(Some(2)).form_string(), "clean_type=2");
    }

    #[test]
    fn to_view_later_accepts_either_identifier() {
        assert_eq!(
            to_view_later_request(None, Some("BV1xx")).form_string(),
            "bvid=BV1xx"
        );
        assert_eq!(to_view_later_request(Some(9), None).form_string(), "aid=9");
        assert_eq!(
            to_view_later_request(None, Some("BV1xx")).idempotency,
            Idempotency::Idempotent,
            "同一条加两次还是一条"
        );
    }
}
