//! 视频流的五条只读接口：热门、排行、相关推荐、每周必看、入站必刷。
//!
//! 对应基线 `VideoHttp` 的 `hotVideoList`、`getRankVideoList`、
//! `relatedVideoList`、`popularSeriesList` / `popularSeriesOne`、
//! `popularPrecious`（`lib/http/video.dart`）。
//!
//! 五条接口在基线里共用 `HotVideoItemModel`，这里同样共用
//! [`bili_domain::video_feed::FeedVideoItem`]，DTO 也只有一份。
//!
//! ## 解析与过滤是两步
//!
//! [`parse_feed_list`] 等解析函数**不过滤**，过滤在 `fetch_*` 里用
//! [`FeedFilter`] 完成。拆开是为了让 golden 快照固定的是「协议解析」
//! 本身——快照若把用户设置也烘进去，设置一变快照就红，而它要证明的恰恰
//! 是解析没变。
//!
//! ## 请求头
//!
//! 每周必看与入站必刷在基线里额外带了桌面端 UA、`origin` 与 `referer`
//! （不带会被风控拦）。这些值的真相源在 Dart（`BrowserUa.pc`、
//! `HttpString.baseUrl`），由调用方放进 [`RequestContext`] 传入，Rust
//! 不另存一份。

use bili_domain::error::DomainError;
use bili_domain::video_feed::{
    FeedDimension, FeedFilter, FeedOwner, FeedStat, FeedVideoItem, PopularPrecious,
    PopularSeriesConfig, PopularSeriesListItem, PopularSeriesOne,
};
use bili_protocol::wbi;
use serde::Deserialize;

use crate::http::{HttpTransport, RequestContext};
use crate::request::PreparedRequest;

/// 接口路径。与基线 `lib/http/api.dart` 保持一致。
pub mod path {
    pub const HOT: &str = "/x/web-interface/popular";
    pub const RELATED: &str = "/x/web-interface/archive/related";
    pub const RANK: &str = "/x/web-interface/ranking/v2";
    pub const POPULAR_SERIES_LIST: &str = "/x/web-interface/popular/series/list";
    pub const POPULAR_SERIES_ONE: &str = "/x/web-interface/popular/series/one";
    pub const POPULAR_PRECIOUS: &str = "/x/web-interface/popular/precious";
}

/// 基线给每周必看/入站必刷带的 `web_location`。值照抄，不求甚解——它是
/// 风控用的页面标识，换一个值会被拒。
const WEB_LOCATION: &str = "333.934";

/// 热门列表请求。不签名（基线也没签）。
#[must_use]
pub fn hot_request(pn: i64, ps: i64) -> PreparedRequest {
    PreparedRequest {
        path: path::HOT,
        query: vec![
            ("pn".to_owned(), pn.to_string()),
            ("ps".to_owned(), ps.to_string()),
        ],
    }
}

/// 相关推荐请求。不签名。
#[must_use]
pub fn related_request(bvid: &str) -> PreparedRequest {
    PreparedRequest {
        path: path::RELATED,
        query: vec![("bvid".to_owned(), bvid.to_owned())],
    }
}

/// 排行榜请求。`type=all` 是基线固定值。
#[must_use]
pub fn rank_request(rid: i64, mixin_key: &str, wts: i64) -> PreparedRequest {
    signed(
        path::RANK,
        vec![
            ("rid".to_owned(), rid.to_string()),
            ("type".to_owned(), "all".to_owned()),
        ],
        mixin_key,
        wts,
    )
}

/// 每周必看的期号列表请求。
#[must_use]
pub fn popular_series_list_request(mixin_key: &str, wts: i64) -> PreparedRequest {
    signed(
        path::POPULAR_SERIES_LIST,
        vec![("web_location".to_owned(), WEB_LOCATION.to_owned())],
        mixin_key,
        wts,
    )
}

/// 每周必看某一期的请求。
#[must_use]
pub fn popular_series_one_request(number: i64, mixin_key: &str, wts: i64) -> PreparedRequest {
    signed(
        path::POPULAR_SERIES_ONE,
        vec![
            ("number".to_owned(), number.to_string()),
            ("web_location".to_owned(), WEB_LOCATION.to_owned()),
        ],
        mixin_key,
        wts,
    )
}

/// 入站必刷的请求。`page_size` 在基线里固定 100。
#[must_use]
pub fn popular_precious_request(page: i64, mixin_key: &str, wts: i64) -> PreparedRequest {
    signed(
        path::POPULAR_PRECIOUS,
        vec![
            ("page".to_owned(), page.to_string()),
            ("page_size".to_owned(), "100".to_owned()),
            ("web_location".to_owned(), WEB_LOCATION.to_owned()),
        ],
        mixin_key,
        wts,
    )
}

/// 补上 `wts` 与 `w_rid`。与 [`crate::request::video_detail`] 同一套做法：
/// 先排序再签名，签名覆盖的就是最终要发出去的那个串。
fn signed(
    path: &'static str,
    mut query: Vec<(String, String)>,
    mixin_key: &str,
    wts: i64,
) -> PreparedRequest {
    query.push(("wts".to_owned(), wts.to_string()));
    query.sort_by(|a, b| a.0.cmp(&b.0));
    let w_rid = wbi::sign(&query, mixin_key);
    query.push(("w_rid".to_owned(), w_rid));
    PreparedRequest { path, query }
}

/// 取热门列表。
///
/// # Errors
/// 传输、状态码、信封三类失败见各自的文档；屏蔽词正则不被支持时为
/// [`ErrorKind::Unsupported`]。
pub async fn fetch_hot<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    filter: &FeedFilter,
    pn: i64,
    ps: i64,
) -> Result<Vec<FeedVideoItem>, DomainError> {
    let body = crate::client::send(transport, ctx, &hot_request(pn, ps)).await?;
    let items = parse_list_in_data(&body)?;
    Ok(retain(items, |i| filter.keeps_hot_or_rank(i)))
}

/// 取排行榜。
///
/// # Errors
/// 同 [`fetch_hot`]。
pub async fn fetch_rank<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    filter: &FeedFilter,
    rid: i64,
    mixin_key: &str,
    wts: i64,
) -> Result<Vec<FeedVideoItem>, DomainError> {
    let body = crate::client::send(transport, ctx, &rank_request(rid, mixin_key, wts)).await?;
    let items = parse_list_in_data(&body)?;
    Ok(retain(items, |i| filter.keeps_hot_or_rank(i)))
}

/// 取相关推荐。
///
/// # Errors
/// 同 [`fetch_hot`]。
pub async fn fetch_related<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    filter: &FeedFilter,
    bvid: &str,
) -> Result<Vec<FeedVideoItem>, DomainError> {
    let body = crate::client::send(transport, ctx, &related_request(bvid)).await?;
    let items = parse_related(&body)?;
    Ok(retain(items, |i| filter.keeps_related(i)))
}

/// 取每周必看的期号列表。
///
/// # Errors
/// 传输、状态码、信封三类失败见各自的文档。
pub async fn fetch_popular_series_list<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    mixin_key: &str,
    wts: i64,
) -> Result<Vec<PopularSeriesListItem>, DomainError> {
    let req = popular_series_list_request(mixin_key, wts);
    let body = crate::client::send(transport, ctx, &req).await?;
    parse_series_list(&body)
}

/// 取每周必看的某一期。基线对这条**不做任何过滤**，这里也不做。
///
/// # Errors
/// 同 [`fetch_popular_series_list`]。
pub async fn fetch_popular_series_one<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    number: i64,
    mixin_key: &str,
    wts: i64,
) -> Result<PopularSeriesOne, DomainError> {
    let req = popular_series_one_request(number, mixin_key, wts);
    let body = crate::client::send(transport, ctx, &req).await?;
    parse_series_one(&body)
}

/// 取入站必刷。基线对这条同样不过滤。
///
/// # Errors
/// 同 [`fetch_popular_series_list`]。
pub async fn fetch_popular_precious<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    page: i64,
    mixin_key: &str,
    wts: i64,
) -> Result<PopularPrecious, DomainError> {
    let req = popular_precious_request(page, mixin_key, wts);
    let body = crate::client::send(transport, ctx, &req).await?;
    parse_precious(&body)
}

fn retain(items: Vec<FeedVideoItem>, keep: impl Fn(&FeedVideoItem) -> bool) -> Vec<FeedVideoItem> {
    items.into_iter().filter(|i| keep(i)).collect()
}

/// 解析 `data.list` 形态的列表（热门、排行）。
///
/// # Errors
/// 信封失败沿用统一分类；结构不符为 `Protocol`。
pub fn parse_list_in_data(body: &str) -> Result<Vec<FeedVideoItem>, DomainError> {
    #[derive(Debug, Default, Deserialize)]
    #[serde(default)]
    struct ListDto {
        list: Option<Vec<ItemDto>>,
    }
    let dto: ListDto = crate::envelope::unwrap_envelope(body)?;
    Ok(into_items(dto.list))
}

/// 解析相关推荐：`data` 直接就是数组。
///
/// # Errors
/// 同 [`parse_list_in_data`]。
pub fn parse_related(body: &str) -> Result<Vec<FeedVideoItem>, DomainError> {
    let dto: Option<Vec<ItemDto>> = crate::envelope::unwrap_envelope(body)?;
    Ok(into_items(dto))
}

/// 解析每周必看的期号列表。
///
/// # Errors
/// 同 [`parse_list_in_data`]。
pub fn parse_series_list(body: &str) -> Result<Vec<PopularSeriesListItem>, DomainError> {
    #[derive(Debug, Default, Deserialize)]
    #[serde(default)]
    struct Dto {
        list: Option<Vec<SeriesListItemDto>>,
    }
    let dto: Dto = crate::envelope::unwrap_envelope(body)?;
    Ok(dto
        .list
        .unwrap_or_default()
        .into_iter()
        .map(SeriesListItemDto::into_domain)
        .collect())
}

/// 解析每周必看的一期。
///
/// # Errors
/// 同 [`parse_list_in_data`]。
pub fn parse_series_one(body: &str) -> Result<PopularSeriesOne, DomainError> {
    #[derive(Debug, Default, Deserialize)]
    #[serde(default)]
    struct Dto {
        config: Option<ConfigDto>,
        reminder: Option<String>,
        list: Option<Vec<ItemDto>>,
    }
    let dto: Dto = crate::envelope::unwrap_envelope(body)?;
    Ok(PopularSeriesOne {
        config: dto.config.map(ConfigDto::into_domain),
        reminder: dto.reminder,
        // 这里保留「没有 list」与「空 list」的区别：前者是协议形态变了，
        // 后者是这一期真的没有内容，UI 的空态文案不同。
        list: dto.list.map(|l| into_items(Some(l))),
    })
}

/// 解析入站必刷。
///
/// # Errors
/// 同 [`parse_list_in_data`]。
pub fn parse_precious(body: &str) -> Result<PopularPrecious, DomainError> {
    #[derive(Debug, Default, Deserialize)]
    #[serde(default)]
    struct Dto {
        media_id: Option<i64>,
        list: Option<Vec<ItemDto>>,
    }
    let dto: Dto = crate::envelope::unwrap_envelope(body)?;
    Ok(PopularPrecious {
        media_id: dto.media_id,
        list: dto.list.map(|l| into_items(Some(l))),
    })
}

fn into_items(dtos: Option<Vec<ItemDto>>) -> Vec<FeedVideoItem> {
    dtos.unwrap_or_default()
        .into_iter()
        .map(ItemDto::into_domain)
        .collect()
}

/// 宽松整数。与 [`crate::video_detail`] 的同名函数同义：服务端对 mid 之
/// 类的字段混用过数字与字符串，基线的 `safeToInt` 两种都收。
fn flexible_i64(v: Option<&serde_json::Value>) -> Option<i64> {
    match v? {
        serde_json::Value::Number(n) => n.as_i64().or_else(|| {
            #[allow(clippy::cast_possible_truncation)]
            n.as_f64().map(|f| f.trunc() as i64)
        }),
        serde_json::Value::String(s) => s.parse().ok(),
        _ => None,
    }
}

// 全字段 Option + `#[serde(default)]`：五条接口给的字段并不一致，少一个
// 不该让整条列表作废。
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ItemDto {
    aid: Option<i64>,
    cid: Option<i64>,
    bvid: Option<String>,
    videos: Option<i64>,
    tid: Option<i64>,
    tname: Option<String>,
    copyright: Option<i64>,
    pic: Option<String>,
    title: Option<String>,
    pubdate: Option<i64>,
    ctime: Option<i64>,
    desc: Option<String>,
    state: Option<i64>,
    duration: Option<i64>,
    owner: Option<OwnerDto>,
    stat: Option<StatDto>,
    dimension: Option<DimensionDto>,
    first_frame: Option<String>,
    pub_location: Option<String>,
    redirect_url: Option<String>,
    progress: Option<f64>,
    charging_pay: Option<ChargingPayDto>,
    rights: Option<RightsDto>,
    pgc_label: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct OwnerDto {
    mid: Option<serde_json::Value>,
    name: Option<String>,
    face: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct StatDto {
    view: Option<serde_json::Value>,
    like: Option<i64>,
    danmaku: Option<i64>,
    reply: Option<i64>,
    favorite: Option<i64>,
    coin: Option<f64>,
    share: Option<i64>,
    now_rank: Option<i64>,
    his_rank: Option<i64>,
    dislike: Option<i64>,
    vt: Option<i64>,
    vv: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct DimensionDto {
    width: Option<i64>,
    height: Option<i64>,
    rotate: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ChargingPayDto {
    level: Option<serde_json::Value>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RightsDto {
    is_cooperation: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ConfigDto {
    name: Option<String>,
    label: Option<String>,
    media_id: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SeriesListItemDto {
    number: Option<i64>,
    subject: Option<String>,
    status: Option<i64>,
    name: Option<String>,
}

impl ItemDto {
    fn into_domain(self) -> FeedVideoItem {
        // 基线 `HotVideoItemModel.fromJson` 的三级优先级：充电专属 >
        // 合作 > 服务端给的 pgc_label。这里只带回「是哪一级」，文案留给
        // 界面（ADR-011 决策二）。
        let badge_kind = if self.charging_pay.and_then(|c| c.level).is_some() {
            Some("charging".to_owned())
        } else if self.rights.and_then(|r| r.is_cooperation) == Some(1) {
            Some("cooperation".to_owned())
        } else {
            None
        };

        let owner = self.owner.unwrap_or_default();
        let stat = self.stat.unwrap_or_default();

        FeedVideoItem {
            aid: self.aid,
            cid: self.cid,
            bvid: self.bvid,
            videos: self.videos,
            tid: self.tid,
            tname: self.tname,
            copyright: self.copyright,
            cover: self.pic,
            title: self.title,
            pubdate: self.pubdate,
            ctime: self.ctime,
            desc: self.desc,
            state: self.state,
            duration: self.duration,
            owner: FeedOwner {
                mid: flexible_i64(owner.mid.as_ref()),
                name: owner.name,
                face: owner.face,
            },
            stat: FeedStat {
                // 播放量在「不公开」时是字符串 "--"，基线 `json["view"]`
                // 赋给 `int?` 会抛，整条列表作废。这里按 safeToInt 收成
                // None，与过滤规则的「view 为 null 则不过滤」接上。
                view: flexible_i64(stat.view.as_ref()),
                like: stat.like,
                danmaku: stat.danmaku,
                reply: stat.reply,
                favorite: stat.favorite,
                coin: stat.coin,
                share: stat.share,
                now_rank: stat.now_rank,
                his_rank: stat.his_rank,
                dislike: stat.dislike,
                vt: stat.vt,
                vv: stat.vv,
            },
            dimension: self.dimension.map(DimensionDto::into_domain),
            first_frame: self.first_frame,
            pub_location: self.pub_location,
            redirect_url: self.redirect_url,
            progress: self.progress,
            badge_kind,
            pgc_label: self.pgc_label,
        }
    }
}

impl DimensionDto {
    fn into_domain(self) -> FeedDimension {
        // 基线 `Dimension.fromJson`：rotate == 1 时宽高互换。
        if self.rotate == Some(1) {
            FeedDimension {
                width: self.height,
                height: self.width,
            }
        } else {
            FeedDimension {
                width: self.width,
                height: self.height,
            }
        }
    }
}

impl ConfigDto {
    fn into_domain(self) -> PopularSeriesConfig {
        PopularSeriesConfig {
            name: self.name,
            label: self.label,
            media_id: self.media_id,
        }
    }
}

impl SeriesListItemDto {
    fn into_domain(self) -> PopularSeriesListItem {
        PopularSeriesListItem {
            number: self.number,
            subject: self.subject,
            status: self.status,
            name: self.name,
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::{
        hot_request, parse_list_in_data, parse_related, parse_series_one, popular_precious_request,
        rank_request, related_request,
    };

    fn envelope(data: &str) -> String {
        format!(r#"{{"code":0,"message":"0","ttl":1,"data":{data}}}"#)
    }

    #[test]
    fn hot_request_is_not_signed() {
        let req = hot_request(2, 20);
        assert_eq!(req.path, super::path::HOT);
        assert_eq!(
            req.query,
            vec![
                ("pn".to_owned(), "2".to_owned()),
                ("ps".to_owned(), "20".to_owned())
            ]
        );
    }

    #[test]
    fn related_request_carries_only_the_bvid() {
        let req = related_request("BV1Fixture01");
        assert_eq!(req.path, super::path::RELATED);
        assert_eq!(req.query.len(), 1);
    }

    #[test]
    fn signed_requests_sort_before_signing_and_append_w_rid() {
        let req = rank_request(0, "0123456789abcdef0123456789abcdef", 1_700_000_000);
        let keys: Vec<&str> = req.query.iter().map(|(k, _)| k.as_str()).collect();
        // 排序覆盖 wts，w_rid 在最后——签名覆盖的串与实际发送的一致。
        assert_eq!(keys, vec!["rid", "type", "wts", "w_rid"]);
        let (_, w_rid) = req.query.last().expect("有 w_rid");
        assert_eq!(w_rid.len(), 32, "w_rid 是 md5 十六进制");
    }

    #[test]
    fn precious_request_pins_the_baseline_page_size() {
        let req = popular_precious_request(1, "0123456789abcdef0123456789abcdef", 1);
        let page_size = req
            .query
            .iter()
            .find(|(k, _)| k == "page_size")
            .expect("有 page_size");
        assert_eq!(page_size.1, "100");
    }

    #[test]
    fn hidden_view_count_does_not_fail_the_whole_list() {
        // 播放量不公开时服务端给的是字符串。基线把它交给 int? 会抛异常，
        // 整条列表作废；这里按 safeToInt 收成 None。
        let body = envelope(r#"{"list":[{"aid":1,"stat":{"view":"--","like":3}}]}"#);
        let items = parse_list_in_data(&body).expect("不该整条失败");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].stat.view, None);
        assert_eq!(items[0].stat.like, Some(3));
    }

    #[test]
    fn string_mid_is_accepted() {
        let body = envelope(r#"[{"owner":{"mid":"20001","name":"UP主"}}]"#);
        let items = parse_related(&body).expect("可解析");
        assert_eq!(items[0].owner.mid, Some(20001));
    }

    #[test]
    fn rotated_dimension_is_normalised() {
        let body = envelope(r#"[{"dimension":{"width":1920,"height":1080,"rotate":1}}]"#);
        let items = parse_related(&body).expect("可解析");
        let dim = items[0].dimension.as_ref().expect("有 dimension");
        assert_eq!((dim.width, dim.height), (Some(1080), Some(1920)));
    }

    #[test]
    fn badge_priority_matches_the_baseline() {
        let body = envelope(
            r#"[{"charging_pay":{"level":1},"rights":{"is_cooperation":1},"pgc_label":"独家"},
                {"rights":{"is_cooperation":1},"pgc_label":"独家"},
                {"pgc_label":"独家"}]"#,
        );
        let items = parse_related(&body).expect("可解析");
        assert_eq!(items[0].badge_kind.as_deref(), Some("charging"));
        assert_eq!(items[1].badge_kind.as_deref(), Some("cooperation"));
        assert_eq!(items[2].badge_kind, None);
        // 三条都带着服务端文案，取不取由界面按 badge_kind 决定。
        assert_eq!(items[2].pgc_label.as_deref(), Some("独家"));
    }

    #[test]
    fn missing_list_is_distinguished_from_an_empty_one() {
        let absent = parse_series_one(&envelope(r#"{"reminder":"x"}"#)).expect("可解析");
        assert!(absent.list.is_none(), "没有 list 是协议形态变了");

        let empty = parse_series_one(&envelope(r#"{"list":[]}"#)).expect("可解析");
        assert_eq!(
            empty.list.as_deref(),
            Some(&[][..]),
            "空 list 是这一期没内容"
        );
    }

    #[test]
    fn business_failure_keeps_the_code() {
        let body = r#"{"code":-404,"message":"啥都木有","ttl":1}"#;
        let err = parse_list_in_data(body).expect_err("业务失败");
        assert_eq!(err.code(), Some(-404));
    }
}
