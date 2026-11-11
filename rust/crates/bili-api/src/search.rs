//! 搜索接口：请求构造、响应 DTO 与到领域实体的映射。
//!
//! 覆盖基线 `lib/http/search.dart` 里属于**搜索**的那几个接口：综合搜索、
//! 分类型搜索、热搜榜、搜索页默认词。同文件里的 `ab2c`、`pgcInfo`、
//! `pugvInfo`、`topicPubSearch` 分别属于 S3 / S10 / S5，不在本模块。
//!
//! ## 一个接口，六种响应形状
//!
//! `search_type` 决定 `data.result` 长什么样：
//!
//! | `search_type` | 路径 | `result` |
//! |---|---|---|
//! | `all` | `/search/all/v2` | 分区数组：`[{result_type, data}, …]` |
//! | 其余五种 | `/search/type` | 条目数组 |
//!
//! 因此解析函数按**形状**分，不按 `search_type` 分：综合搜索一个
//! （[`parse_all`]），其余五种各一个。调用方知道自己要什么形状。
//!
//! ## 风控
//!
//! `data.v_voucher` 存在即表示本次请求被风控拦下，`result` 不可信。基线
//! 在这里直接判失败（`lib/http/search.dart` 里那句 `return const
//! Error('触发风控')`，其后的验证流程是注释掉的）。本模块保持同样的判定，
//! 归为 [`ErrorKind::Auth`]——重试只会继续被拦。
//!
//! ## 为什么这里不用 `#[derive(Deserialize)]` 的 DTO
//!
//! 本 crate 的其余模块都用 DTO（见 crate 文档），搜索是例外。原因是这个
//! 接口的容错粒度不同：同一个键在不同 `search_type` 下类型会变（统计字段
//! 有时是数字有时是字符串，基线为此专门有个 `safeToInt`），而 derive 出来
//! 的 `Deserialize` 一个字段类型不符就让**整条记录**失败。搜索结果一页
//! 二十条，为一条的播放量是字符串而丢掉整条不划算，更不该丢掉整页。
//!
//! 因此本模块在 [`serde_json::Value`] 上逐字段取值，取不到就是 `None`。
//! 代价是字段名写成了字符串字面量，拼错不会在编译期发现——由下面的单元
//! 测试与 golden fixture 兜住。

use bili_domain::error::{DomainError, ErrorKind};
use bili_domain::search::{
    OfficialVerify, SearchActivityItem, SearchAllResult, SearchArticleItem, SearchLiveItem,
    SearchOwner, SearchPgcItem, SearchStat, SearchTrending, SearchTrendingItem, SearchUserCard,
    SearchUserItem, SearchVideoItem,
};
use serde_json::Value;

use crate::http::{HttpTransport, RequestContext, check_status};
use crate::request::PreparedRequest;
use crate::search_text::{parse_duration_secs, parse_title_spans, plain_title, strip_tags_first};

/// 接口路径。与基线 `lib/http/api.dart` 保持一致。
pub mod path {
    /// 综合搜索。
    pub const SEARCH_ALL: &str = "/x/web-interface/wbi/search/all/v2";
    /// 分类型搜索。
    pub const SEARCH_BY_TYPE: &str = "/x/web-interface/wbi/search/type";
    /// 热搜榜。
    pub const SEARCH_TRENDING: &str = "/x/v2/search/trending/ranking";
    /// 搜索页默认词。
    pub const SEARCH_RECOMMEND: &str = "/x/web-interface/wbi/search/square";
}

/// 分类型搜索的业务入参。与基线 `SearchHttp.searchByType` 的签名一一对应。
///
/// 成结构而不是摊平成十来个参数：筛选项还会增加（基线已经有时长、分区、
/// 发布时间、用户类型、专栏分区五组），每加一个都改一遍签名与四个调用点
/// 不划算。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SearchParams {
    /// `all` / `video` / `media_bangumi` / `media_ft` / `live_room` /
    /// `bili_user` / `article`。原样进查询参数。
    pub search_type: String,
    pub keyword: String,
    pub page: i64,
    /// 排序。基线只在**非空**时才放进参数，空串表示默认排序。
    pub order: Option<String>,
    pub duration: Option<i64>,
    pub tids: Option<i64>,
    pub order_sort: Option<i64>,
    pub user_type: Option<i64>,
    pub category_id: Option<i64>,
    pub pub_begin: Option<i64>,
    pub pub_end: Option<i64>,
    /// 风控校验票据。基线把它同时放进查询参数与 Cookie。
    pub gaia_vtoken: Option<String>,
}

impl SearchParams {
    /// 综合搜索走 `/search/all/v2`，其余走 `/search/type`。
    #[must_use]
    pub fn path(&self) -> &'static str {
        if self.search_type == "all" {
            path::SEARCH_ALL
        } else {
            path::SEARCH_BY_TYPE
        }
    }
}

/// 构造分类型搜索请求。
///
/// 固定参数（`page_size` / `platform` / `web_location`）在这里补齐而不是
/// 由调用方传：它们是协议要求的常量，调用方没有理由有机会忘掉。
#[must_use]
pub fn build_search_request(params: &SearchParams, mixin_key: &str, wts: i64) -> PreparedRequest {
    let mut query: Vec<(String, String)> = vec![
        ("search_type".to_owned(), params.search_type.clone()),
        ("keyword".to_owned(), params.keyword.clone()),
        ("page".to_owned(), params.page.to_string()),
        ("page_size".to_owned(), "20".to_owned()),
        ("platform".to_owned(), "pc".to_owned()),
        ("web_location".to_owned(), "1430654".to_owned()),
        ("wts".to_owned(), wts.to_string()),
    ];

    // 基线：`if (order != null && order.isNotEmpty)`。空串不进参数——
    // 放一个空的 order 会被服务端当成非法排序而不是默认排序。
    if let Some(order) = params.order.as_ref().filter(|o| !o.is_empty()) {
        query.push(("order".to_owned(), order.clone()));
    }
    push_opt(&mut query, "duration", params.duration);
    push_opt(&mut query, "tids", params.tids);
    push_opt(&mut query, "order_sort", params.order_sort);
    push_opt(&mut query, "user_type", params.user_type);
    push_opt(&mut query, "category_id", params.category_id);
    push_opt(&mut query, "pubtime_begin_s", params.pub_begin);
    push_opt(&mut query, "pubtime_end_s", params.pub_end);
    if let Some(token) = &params.gaia_vtoken {
        query.push(("gaia_vtoken".to_owned(), token.clone()));
    }

    query.sort_by(|a, b| a.0.cmp(&b.0));
    let w_rid = bili_protocol::wbi::sign(&query, mixin_key);
    query.push(("w_rid".to_owned(), w_rid));

    PreparedRequest {
        path: params.path(),
        query,
    }
}

fn push_opt(query: &mut Vec<(String, String)>, key: &str, value: Option<i64>) {
    if let Some(value) = value {
        query.push((key.to_owned(), value.to_string()));
    }
}

// ------------------------------------------------------------------ 取数

/// 发一条搜索请求并返回响应体。
///
/// # Errors
/// 传输失败为 `Network`；非 2xx 见 [`check_status`]。
async fn send<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    prepared: &PreparedRequest,
) -> Result<String, DomainError> {
    let url = format!(
        "{}{}?{}",
        ctx.base_url.trim_end_matches('/'),
        prepared.path,
        crate::client::encode_query(&prepared.query)
    );

    // 只记 path：查询串里带着用户输入的关键词，进日志既无益于诊断也有损
    // 隐私（RFC-0001 §2.2）。
    tracing::debug!(path = prepared.path, "发送搜索请求");

    let resp = transport
        .get(crate::http::HttpGet {
            url,
            headers: ctx.all_headers(),
            timeout: ctx.timeout,
        })
        .await?;

    check_status(resp.status)?;
    Ok(resp.body)
}

/// 取一次搜索的原始响应体。
///
/// 解析交给调用方按形状选函数——同一个请求路径有六种响应形状，在这里
/// 分支的话返回类型就得是个并集，而调用方本来就知道自己要哪一种。
///
/// # Errors
/// 见 [`send`]。
pub async fn fetch_search<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    params: &SearchParams,
    mixin_key: &str,
    wts: i64,
) -> Result<String, DomainError> {
    let prepared = build_search_request(params, mixin_key, wts);
    send(transport, ctx, &prepared).await
}

/// 取热搜榜。`limit` 为条数上限。
///
/// # Errors
/// 见 [`send`]；解析失败见 [`parse_trending`]。
pub async fn fetch_trending<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    limit: i64,
    needs_top: bool,
) -> Result<SearchTrending, DomainError> {
    let prepared = PreparedRequest {
        path: path::SEARCH_TRENDING,
        query: vec![("limit".to_owned(), limit.to_string())],
    };
    let body = send(transport, ctx, &prepared).await?;
    parse_trending(&body, needs_top)
}

/// 取搜索页默认词。
///
/// 查询参数是基线写死的一组 App 端标识（`build` / `mobi_app` / …）：
/// 这个接口在 Web 端标识下会返回空列表，所以不能省。
///
/// # Errors
/// 见 [`send`]；解析失败见 [`parse_recommend`]。
pub async fn fetch_recommend<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
) -> Result<Vec<SearchTrendingItem>, DomainError> {
    let prepared = PreparedRequest {
        path: path::SEARCH_RECOMMEND,
        query: vec![
            ("build".to_owned(), "8430300".to_owned()),
            ("channel".to_owned(), "master".to_owned()),
            ("version".to_owned(), "8.43.0".to_owned()),
            ("c_locale".to_owned(), "zh_CN".to_owned()),
            ("mobi_app".to_owned(), "android".to_owned()),
            ("platform".to_owned(), "android".to_owned()),
            ("s_locale".to_owned(), "zh_CN".to_owned()),
            ("from".to_owned(), "2".to_owned()),
        ],
    };
    let body = send(transport, ctx, &prepared).await?;
    parse_recommend(&body)
}

// ------------------------------------------------------------------ 解析

/// 解开信封并挡下风控。
///
/// # Errors
/// 信封失败见 [`crate::envelope`]；`v_voucher` 存在时为 [`ErrorKind::Auth`]。
fn unwrap_search_data(body: &str) -> Result<Value, DomainError> {
    let data: Value = crate::envelope::unwrap_envelope(body)?;

    // 基线：`if (vVoucher != null) return const Error('触发风控')`。
    // 归 Auth 而不是 Remote：原样重试只会再被拦一次，必须走验证流程。
    if data.get("v_voucher").is_some_and(|v| !v.is_null()) {
        return Err(DomainError::new(
            ErrorKind::Auth,
            "触发风控，需要完成验证后重试",
        ));
    }
    Ok(data)
}

/// `numResults` 字段。基线读的是 `(json['numResults'] as num?)?.toInt()`。
fn num_results(data: &Value) -> Option<i64> {
    data.get("numResults").and_then(Value::as_i64)
}

/// 解析综合搜索。
///
/// 按 `result_type` 分发到各分区；未知分区整块忽略——服务端加新卡片类型
/// 时，老客户端应当少显示一块，而不是整页搜不出来。
///
/// # Errors
/// 见 [`unwrap_search_data`]。
pub fn parse_all(body: &str) -> Result<SearchAllResult, DomainError> {
    let data = unwrap_search_data(body)?;
    let mut out = SearchAllResult {
        num_results: num_results(&data),
        ..SearchAllResult::default()
    };

    let Some(blocks) = data.get("result").and_then(Value::as_array) else {
        return Ok(out);
    };

    for block in blocks {
        let Some(items) = block.get("data").and_then(Value::as_array) else {
            continue;
        };
        match block.get("result_type").and_then(Value::as_str) {
            Some("video") => out.videos = items.iter().filter_map(video_item).collect(),
            Some("bili_user") => out.users = items.iter().filter_map(user_card).collect(),
            Some("media_bangumi" | "media_ft") => {
                out.medias.extend(items.iter().filter_map(pgc_item));
            }
            Some("activity") => out.activities = items.iter().filter_map(activity_item).collect(),
            // 赛事卡片原样带过去，理由见 `SearchAllResult::esports_json`。
            Some("esports") => {
                out.esports_json = items.first().and_then(|v| serde_json::to_string(v).ok());
            }
            _ => {}
        }
    }
    Ok(out)
}

/// 一次分类型搜索的结果。
#[derive(Debug, Clone, PartialEq)]
pub struct SearchPage<T> {
    pub num_results: Option<i64>,
    pub items: Vec<T>,
}

/// 解析视频搜索。
///
/// # Errors
/// 见 [`unwrap_search_data`]。
pub fn parse_video(body: &str) -> Result<SearchPage<SearchVideoItem>, DomainError> {
    parse_typed(body, video_item)
}

/// 解析用户搜索。
///
/// # Errors
/// 见 [`unwrap_search_data`]。
pub fn parse_user(body: &str) -> Result<SearchPage<SearchUserItem>, DomainError> {
    parse_typed(body, user_item)
}

/// 解析直播间搜索。
///
/// # Errors
/// 见 [`unwrap_search_data`]。
pub fn parse_live(body: &str) -> Result<SearchPage<SearchLiveItem>, DomainError> {
    parse_typed(body, live_item)
}

/// 解析番剧/影视搜索。
///
/// # Errors
/// 见 [`unwrap_search_data`]。
pub fn parse_pgc(body: &str) -> Result<SearchPage<SearchPgcItem>, DomainError> {
    parse_typed(body, pgc_item)
}

/// 解析专栏搜索。
///
/// # Errors
/// 见 [`unwrap_search_data`]。
pub fn parse_article(body: &str) -> Result<SearchPage<SearchArticleItem>, DomainError> {
    parse_typed(body, article_item)
}

fn parse_typed<T>(
    body: &str,
    map: impl Fn(&Value) -> Option<T>,
) -> Result<SearchPage<T>, DomainError> {
    let data = unwrap_search_data(body)?;
    let items = data
        .get("result")
        .and_then(Value::as_array)
        .map(|list| list.iter().filter_map(map).collect())
        .unwrap_or_default();
    Ok(SearchPage {
        num_results: num_results(&data),
        items,
    })
}

/// 解析热搜榜。
///
/// `needs_top` 为真时把 `top_list` 拼在 `list` 前面，并记下置顶条数——
/// 与基线 `SearchTrendingData.fromJson` 的行为一致。
///
/// # Errors
/// 见 [`crate::envelope`]。
pub fn parse_trending(body: &str, needs_top: bool) -> Result<SearchTrending, DomainError> {
    let data: Value = crate::envelope::unwrap_envelope(body)?;
    let mut list = trending_list(data.get("list"));
    let mut top_count = 0i64;

    if needs_top {
        let mut top = trending_list(data.get("top_list"));
        top_count = i64::try_from(top.len()).unwrap_or(i64::MAX);
        top.append(&mut list);
        list = top;
    }

    Ok(SearchTrending { list, top_count })
}

/// 解析搜索页默认词。
///
/// # Errors
/// 见 [`crate::envelope`]。
pub fn parse_recommend(body: &str) -> Result<Vec<SearchTrendingItem>, DomainError> {
    let data: Value = crate::envelope::unwrap_envelope(body)?;
    Ok(trending_list(data.get("list")))
}

fn trending_list(value: Option<&Value>) -> Vec<SearchTrendingItem> {
    value
        .and_then(Value::as_array)
        .map(|list| list.iter().filter_map(trending_item).collect())
        .unwrap_or_default()
}

/// 缺 `keyword` 的条目直接丢掉。
///
/// 基线这里是 `json['keyword'] as String`，缺失即抛异常、整个热搜榜
/// 拿不到。丢掉单条更合适——与 ADR-011 决策三同向。
fn trending_item(value: &Value) -> Option<SearchTrendingItem> {
    let keyword = value.get("keyword")?.as_str()?.to_owned();
    Some(SearchTrendingItem {
        // 基线：`json['show_name'] ?? json['keyword']`。
        show_name: str_field(value, "show_name").unwrap_or_else(|| keyword.clone()),
        keyword,
        icon: str_field(value, "icon"),
        show_live_icon: value.get("show_live_icon").and_then(Value::as_bool),
        recommend_reason: str_field(value, "recommend_reason"),
    })
}

// ------------------------------------------------------------ 条目级映射

fn video_item(value: &Value) -> Option<SearchVideoItem> {
    let title_spans = parse_title_spans(str_field(value, "title").unwrap_or_default().as_str());
    Some(SearchVideoItem {
        id: i64_field(value, "id"),
        aid: i64_field(value, "aid"),
        bvid: str_field(value, "bvid"),
        arcurl: str_field(value, "arcurl"),
        title: bili_domain::search::spans_to_plain(&title_spans),
        title_spans,
        description: str_field(value, "description"),
        pic: str_field(value, "pic"),
        pubdate: i64_field(value, "pubdate"),
        senddate: i64_field(value, "senddate"),
        duration_secs: parse_duration_secs(
            str_field(value, "duration").unwrap_or_default().as_str(),
        ),
        owner: SearchOwner {
            mid: i64_field(value, "mid"),
            name: str_field(value, "author"),
            face: str_field(value, "upic"),
        },
        stat: SearchStat {
            // 基线这几个字段走 `safeToInt`：接口偶尔给字符串。
            view: lenient_i64(value, "play"),
            danmaku: lenient_i64(value, "danmaku"),
            favorite: lenient_i64(value, "favorite"),
            reply: lenient_i64(value, "review"),
            like: lenient_i64(value, "like"),
        },
        item_type: str_field(value, "type"),
        roomid: i64_field(value, "roomid"),
        is_union_video: i64_field(value, "is_union_video"),
        tag: str_field(value, "tag"),
    })
}

fn user_item(value: &Value) -> Option<SearchUserItem> {
    Some(SearchUserItem {
        item_type: str_field(value, "type"),
        mid: i64_field(value, "mid"),
        uname: str_field(value, "uname"),
        usign: str_field(value, "usign"),
        fans: i64_field(value, "fans"),
        videos: i64_field(value, "videos"),
        upic: str_field(value, "upic"),
        face_nft: i64_field(value, "face_nft"),
        face_nft_type: i64_field(value, "face_nft_type"),
        verify_info: str_field(value, "verify_info"),
        level: i64_field(value, "level"),
        gender: i64_field(value, "gender"),
        is_up_user: i64_field(value, "is_upuser"),
        is_live: i64_field(value, "is_live"),
        room_id: i64_field(value, "room_id"),
        official_verify: official_verify(value.get("official_verify")),
        is_senior_member: i64_field(value, "is_senior_member"),
    })
}

fn user_card(value: &Value) -> Option<SearchUserCard> {
    Some(SearchUserCard {
        mid: i64_field(value, "mid"),
        uname: str_field(value, "uname"),
        usign: str_field(value, "usign"),
        fans: i64_field(value, "fans"),
        videos: i64_field(value, "videos"),
        upic: str_field(value, "upic"),
        verify_info: str_field(value, "verify_info"),
        level: i64_field(value, "level"),
        gender: i64_field(value, "gender"),
        is_up_user: i64_field(value, "is_upuser"),
        is_live: i64_field(value, "is_live"),
        room_id: i64_field(value, "room_id"),
        official_verify: official_verify(value.get("official_verify")),
        is_senior_member: i64_field(value, "is_senior_member"),
        res: value
            .get("res")
            .and_then(Value::as_array)
            .map(|list| list.iter().filter_map(video_item).collect())
            .unwrap_or_default(),
    })
}

fn live_item(value: &Value) -> Option<SearchLiveItem> {
    let title_spans = parse_title_spans(str_field(value, "title").unwrap_or_default().as_str());
    Some(SearchLiveItem {
        rank_offset: i64_field(value, "rank_offset"),
        uid: i64_field(value, "uid"),
        tags: str_field(value, "tags"),
        live_time: str_field(value, "live_time"),
        uname: str_field(value, "uname"),
        uface: str_field(value, "uface"),
        user_cover: str_field(value, "user_cover"),
        item_type: str_field(value, "type"),
        title: bili_domain::search::spans_to_plain(&title_spans),
        title_spans,
        cover: str_field(value, "cover"),
        online: i64_field(value, "online"),
        rank_index: i64_field(value, "rank_index"),
        rank_score: i64_field(value, "rank_score"),
        roomid: i64_field(value, "roomid"),
        attentions: i64_field(value, "attentions"),
        cate_name: str_field(value, "cate_name").map(|c| strip_tags_first(&c)),
    })
}

fn pgc_item(value: &Value) -> Option<SearchPgcItem> {
    let title_spans = parse_title_spans(str_field(value, "title").unwrap_or_default().as_str());
    Some(SearchPgcItem {
        item_type: str_field(value, "type"),
        media_id: i64_field(value, "media_id"),
        title: bili_domain::search::spans_to_plain(&title_spans),
        title_spans,
        org_title: str_field(value, "org_title"),
        media_type: i64_field(value, "media_type"),
        cv: str_field(value, "cv"),
        staff: str_field(value, "staff"),
        season_id: i64_field(value, "season_id"),
        is_avid: value.get("is_avid").and_then(Value::as_bool),
        hit_epids: str_field(value, "hit_epids"),
        season_type: i64_field(value, "season_type"),
        season_type_name: str_field(value, "season_type_name"),
        url: str_field(value, "url"),
        button_text: str_field(value, "button_text"),
        is_follow: i64_field(value, "is_follow"),
        is_selection: i64_field(value, "is_selection"),
        cover: str_field(value, "cover"),
        areas: str_field(value, "areas"),
        styles: str_field(value, "styles"),
        goto_url: str_field(value, "goto_url"),
        desc: str_field(value, "desc"),
        pubtime: i64_field(value, "pubtime"),
        media_mode: i64_field(value, "media_mode"),
        media_score_json: value
            .get("media_score")
            .filter(|v| !v.is_null())
            .and_then(|v| serde_json::to_string(v).ok()),
        index_show: str_field(value, "index_show"),
    })
}

fn article_item(value: &Value) -> Option<SearchArticleItem> {
    let raw_title = str_field(value, "title").unwrap_or_default();
    let title_spans = parse_title_spans(&raw_title);
    Some(SearchArticleItem {
        pub_time: i64_field(value, "pub_time"),
        like: i64_field(value, "like"),
        title: plain_title(&raw_title),
        title_spans,
        rank_offset: i64_field(value, "rank_offset"),
        mid: i64_field(value, "mid"),
        image_urls: value
            .get("image_urls")
            .and_then(Value::as_array)
            .map(|list| {
                list.iter()
                    .filter_map(|v| v.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default(),
        id: i64_field(value, "id"),
        category_id: i64_field(value, "category_id"),
        view: i64_field(value, "view"),
        reply: i64_field(value, "reply"),
        desc: str_field(value, "desc"),
        rank_score: i64_field(value, "rank_score"),
        item_type: str_field(value, "type"),
        // 基线这里读的是 `templateId`（驼峰），不是 `template_id`。
        template_id: i64_field(value, "templateId"),
        category_name: str_field(value, "category_name"),
    })
}

fn activity_item(value: &Value) -> Option<SearchActivityItem> {
    Some(SearchActivityItem {
        id: i64_field(value, "id"),
        title: str_field(value, "title"),
        desc: str_field(value, "desc"),
        cover: str_field(value, "cover"),
        url: str_field(value, "url"),
        state: i64_field(value, "state"),
        status: i64_field(value, "status"),
        author: str_field(value, "author"),
    })
}

fn official_verify(value: Option<&Value>) -> Option<OfficialVerify> {
    let value = value.filter(|v| !v.is_null())?;
    Some(OfficialVerify {
        kind: i64_field(value, "type"),
        desc: str_field(value, "desc"),
    })
}

// ------------------------------------------------------------------ 取值

fn str_field(value: &Value, key: &str) -> Option<String> {
    value.get(key)?.as_str().map(str::to_owned)
}

fn i64_field(value: &Value, key: &str) -> Option<i64> {
    value.get(key)?.as_i64()
}

/// 数字或数字字符串都认。对应基线的 `safeToInt`：搜索接口的统计字段
/// 偶尔会给成字符串，收紧成只认数字会让那一条的播放量变成空。
fn lenient_i64(value: &Value, key: &str) -> Option<i64> {
    match value.get(key)? {
        Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)),
        Value::String(s) => s.parse().ok(),
        _ => None,
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    fn envelope(data: &str) -> String {
        format!(r#"{{"code":0,"message":"0","ttl":1,"data":{data}}}"#)
    }

    #[test]
    fn video_results_carry_rich_title_and_seconds() {
        let body = envelope(
            r#"{"numResults":42,"result":[{
                "id":1,"aid":100,"bvid":"BV1xx","type":"video",
                "title":"前<em class=\"keyword\">命中</em>后",
                "duration":"12:34","mid":7,"author":"UP","upic":"//face",
                "play":1234,"danmaku":"56","review":7,"favorite":8,"like":9,
                "is_union_video":1
            }]}"#,
        );
        let page = parse_video(&body).expect("应当解析成功");
        assert_eq!(page.num_results, Some(42));
        let item = &page.items[0];
        assert_eq!(item.title, "前命中后");
        assert_eq!(item.title_spans.len(), 3);
        assert!(item.title_spans[1].is_em);
        assert_eq!(item.duration_secs, 754);
        assert_eq!(item.owner.name.as_deref(), Some("UP"));
        assert_eq!(item.stat.view, Some(1234));
        // 基线 safeToInt：字符串形态的数字也要读出来。
        assert_eq!(item.stat.danmaku, Some(56));
        assert_eq!(item.is_union_video, Some(1));
    }

    #[test]
    fn risk_control_voucher_is_an_auth_error() {
        // 基线：`if (vVoucher != null) return const Error('触发风控')`。
        let body = envelope(r#"{"v_voucher":"voucher_xxx","result":[]}"#);
        let err = parse_video(&body).expect_err("必须判为失败");
        assert_eq!(err.kind(), ErrorKind::Auth, "重试只会继续被拦");
        assert!(err.detail().contains("风控"));
    }

    #[test]
    fn missing_result_is_an_empty_page_not_an_error() {
        // 搜不到东西是正常结果，不是协议错误。
        let body = envelope(r#"{"numResults":0}"#);
        let page = parse_video(&body).expect("应当解析成功");
        assert!(page.items.is_empty());
        assert_eq!(page.num_results, Some(0));
    }

    #[test]
    fn search_all_dispatches_by_result_type() {
        let body = envelope(
            r#"{"numResults":9,"result":[
                {"result_type":"video","data":[{"title":"v","duration":"0:30"}]},
                {"result_type":"bili_user","data":[{"mid":5,"uname":"u",
                    "res":[{"title":"代表作","duration":"1:00"}]}]},
                {"result_type":"media_bangumi","data":[{"title":"番","media_id":3}]},
                {"result_type":"media_ft","data":[{"title":"影","media_id":4}]},
                {"result_type":"activity","data":[{"id":1,"url":"https://live.bilibili.com/x"}]},
                {"result_type":"brand_new_block","data":[{"whatever":1}]}
            ]}"#,
        );
        let all = parse_all(&body).expect("应当解析成功");
        assert_eq!(all.num_results, Some(9));
        assert_eq!(all.videos.len(), 1);
        assert_eq!(all.users.len(), 1);
        assert_eq!(all.users[0].res.len(), 1, "代表作也要解析出来");
        // media_bangumi 与 media_ft 合并进同一个列表，与基线一致。
        assert_eq!(all.medias.len(), 2);
        assert_eq!(all.activities.len(), 1);
    }

    #[test]
    fn unknown_result_block_is_ignored_not_fatal() {
        // 服务端加新卡片类型时，老客户端应当少显示一块，而不是整页搜不出来。
        let body = envelope(r#"{"result":[{"result_type":"brand_new","data":[{}]}]}"#);
        let all = parse_all(&body).expect("未知分区不该让解析失败");
        assert!(all.videos.is_empty());
    }

    #[test]
    fn activity_url_filtering_is_left_to_the_caller() {
        // 「只能打开直播活动页」是界面决策，本层原样带回两条。
        let body = envelope(
            r#"{"result":[{"result_type":"activity","data":[
                {"id":1,"url":"https://live.bilibili.com/x"},
                {"id":2,"url":"https://www.bilibili.com/y"}
            ]}]}"#,
        );
        let all = parse_all(&body).expect("应当解析成功");
        assert_eq!(all.activities.len(), 2);
    }

    #[test]
    fn esports_block_is_passed_through_as_json() {
        // MatchContest 还有一个未迁的生产者（S15），按 ADR-011 决策六不展开。
        let body = envelope(
            r#"{"result":[{"result_type":"esports","data":[{"ID":77,"gameStage":"决赛"}]}]}"#,
        );
        let all = parse_all(&body).expect("应当解析成功");
        let raw = all.esports_json.expect("应当带回原始 JSON");
        assert!(raw.contains("\"ID\":77"));
    }

    #[test]
    fn live_cate_name_is_unwrapped() {
        let body = envelope(r#"{"result":[{"title":"直播","cate_name":"<span>网游</span>"}]}"#);
        let page = parse_live(&body).expect("应当解析成功");
        assert_eq!(page.items[0].cate_name.as_deref(), Some("网游"));
    }

    #[test]
    fn article_reads_the_camel_case_template_id() {
        // 接口这个字段是驼峰的 `templateId`，不是 `template_id`。
        let body = envelope(r#"{"result":[{"title":"文","templateId":4}]}"#);
        let page = parse_article(&body).expect("应当解析成功");
        assert_eq!(page.items[0].template_id, Some(4));
    }

    #[test]
    fn trending_prepends_the_top_list_and_counts_it() {
        let body = envelope(
            r#"{"list":[{"keyword":"b"},{"keyword":"c"}],"top_list":[{"keyword":"a"}]}"#,
        );
        let top = parse_trending(&body, true).expect("应当解析成功");
        assert_eq!(top.top_count, 1);
        let words: Vec<&str> = top.list.iter().map(|i| i.keyword.as_str()).collect();
        assert_eq!(words, ["a", "b", "c"], "置顶项排在前面");

        // needs_top 为假时不取 top_list，计数为 0。
        let plain = parse_trending(&body, false).expect("应当解析成功");
        assert_eq!(plain.top_count, 0);
        assert_eq!(plain.list.len(), 2);
    }

    #[test]
    fn trending_show_name_falls_back_to_keyword() {
        let body = envelope(r#"{"list":[{"keyword":"关键词"}]}"#);
        let t = parse_trending(&body, false).expect("应当解析成功");
        assert_eq!(t.list[0].show_name, "关键词");
    }

    #[test]
    fn trending_item_without_keyword_is_dropped() {
        // 基线此处 `as String` 会抛异常，整个热搜榜拿不到。
        let body = envelope(r#"{"list":[{"show_name":"没有 keyword"},{"keyword":"好的"}]}"#);
        let t = parse_trending(&body, false).expect("应当解析成功");
        assert_eq!(t.list.len(), 1);
        assert_eq!(t.list[0].keyword, "好的");
    }

    #[test]
    fn recommend_reason_is_not_reformatted_here() {
        // 把中点换成空格是排版，留给 Dart。
        let body = envelope(r#"{"list":[{"keyword":"k","recommend_reason":"热点·解读"}]}"#);
        let items = parse_recommend(&body).expect("应当解析成功");
        assert_eq!(items[0].recommend_reason.as_deref(), Some("热点·解读"));
    }

    #[test]
    fn remote_error_code_is_carried_through() {
        let body = r#"{"code":-412,"message":"请求被拦截","data":null}"#;
        let err = parse_video(body).expect_err("必须失败");
        assert_eq!(err.code(), Some(-412));
    }

    #[test]
    fn empty_order_is_not_sent() {
        // 空 order 会被服务端当成非法排序而不是默认排序。
        let params = SearchParams {
            search_type: "video".to_owned(),
            keyword: "测试".to_owned(),
            page: 1,
            order: Some(String::new()),
            ..SearchParams::default()
        };
        let req = build_search_request(&params, "mixin", 1_700_000_000);
        assert!(!req.query.iter().any(|(k, _)| k == "order"));
    }

    #[test]
    fn all_search_uses_a_different_path() {
        let mut params = SearchParams {
            search_type: "all".to_owned(),
            keyword: "k".to_owned(),
            page: 1,
            ..SearchParams::default()
        };
        assert_eq!(build_search_request(&params, "m", 1).path, path::SEARCH_ALL);
        params.search_type = "video".to_owned();
        assert_eq!(
            build_search_request(&params, "m", 1).path,
            path::SEARCH_BY_TYPE
        );
    }

    #[test]
    fn request_is_sorted_and_signed() {
        let params = SearchParams {
            search_type: "video".to_owned(),
            keyword: "k".to_owned(),
            page: 2,
            duration: Some(1),
            ..SearchParams::default()
        };
        let req = build_search_request(&params, "mixinkey", 1_700_000_000);

        // w_rid 必须排在最后（它不参与签名），其余按 key 升序。
        let (last_key, _) = req.query.last().expect("非空");
        assert_eq!(last_key, "w_rid");
        let keys: Vec<&str> = req.query[..req.query.len() - 1]
            .iter()
            .map(|(k, _)| k.as_str())
            .collect();
        let mut sorted = keys.clone();
        sorted.sort_unstable();
        assert_eq!(keys, sorted, "签名覆盖的是排序后的查询串");

        // 协议要求的常量不能漏。
        for key in ["page_size", "platform", "web_location", "wts"] {
            assert!(req.query.iter().any(|(k, _)| k == key), "缺少 {key}");
        }
    }
}
