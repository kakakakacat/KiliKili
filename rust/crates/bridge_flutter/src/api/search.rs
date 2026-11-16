//! 搜索的 Dart 导出（S9 / issue #59）。
//!
//! 照 `sponsor_block` 的 typed 直连样板写（ADR-011）：结果用**值**表达
//! 失败而不是 `Result`，实体字段保持 `Option`，设置与凭据按请求传入。
//!
//! ## 为什么是六个函数而不是一个
//!
//! 同一个 `search_type` 参数决定了响应有六种形状。做成一个函数就得返回
//! 一个并集结构（六个 `Option<Vec<…>>`），Dart 侧每个调用点都要先判空再
//! 取自己那一支——把类型系统本来能表达的东西退化成运行时检查。调用方
//! 知道自己点的是哪个标签页，直接调对应的函数。
//!
//! ## 分页信息与条目一起返回
//!
//! `num_results` 用来画「约 N 条结果」并驱动分页，和条目来自同一次响应。
//! 拆成两次调用等于把一次请求的结果撕成两半。
//!
//! ## 风控
//!
//! 服务端用 `v_voucher` 拦下请求时，返回 `error_kind: "auth"`，文案里带
//! 「风控」二字。Dart 侧据此提示用户去完成验证，而不是当成网络错误重试
//! ——重试只会继续被拦。

use std::time::Duration;

use bili_api::http::RequestContext;
use bili_api::search::{self, SearchParams as ApiSearchParams};
use bili_api::transport_reqwest::ReqwestTransport;
use bili_domain::error::DomainError;
use bili_domain::redact::Redacted;

use crate::api::init;
use crate::api::transport::TransportSettings;

// ------------------------------------------------------------------ 入参

/// 一次搜索请求的公共参数。
///
/// 单独成结构而不是摊平：本模块有八个导出函数，每个都要这一组，摊平的话
/// 加一个字段要改八处签名，还得同步改 Dart 的八个调用点。
#[derive(Debug, Clone)]
pub struct SearchContext {
    /// 接口基地址，如 `https://api.bilibili.com`。
    pub base_url: String,
    /// 完整的 Cookie 头值。未登录时为 `None`。**Rust 不保存**，用完即弃。
    pub cookie: Option<String>,
    /// 其余请求头（User-Agent、Referer、origin 等）。搜索接口对 Referer
    /// 与浏览器 UA 敏感，由 Dart 的账号体系与 `BrowserUa` 产出。
    pub headers: Vec<(String, String)>,
    /// WBI 的 `mixin_key`。由 Dart 侧按日缓存（`WbiSign.getWbiKeys`）。
    pub mixin_key: String,
    /// 签名用的 Unix 秒。由调用方注入，使请求构造保持可被 fixture 固定。
    pub wts: i64,
    pub timeout_ms: u64,
    pub transport: TransportSettings,
}

impl SearchContext {
    fn split(self) -> (RequestContext, ReqwestTransport, String, i64) {
        let transport = ReqwestTransport::new(self.transport.to_config());
        let ctx = RequestContext {
            base_url: self.base_url,
            cookie: self.cookie.map(Redacted::new),
            // 只读路径：csrf 只有写操作用得上（ADR-013 决策四）。
            csrf: None,
            headers: self.headers,
            timeout: Duration::from_millis(self.timeout_ms),
        };
        (ctx, transport, self.mixin_key, self.wts)
    }
}

/// 分类型搜索的筛选条件。与基线 `SearchHttp.searchByType` 的参数一一对应。
#[derive(Debug, Clone)]
pub struct SearchQuery {
    /// `all` / `video` / `media_bangumi` / `media_ft` / `live_room` /
    /// `bili_user` / `article`。
    pub search_type: String,
    pub keyword: String,
    pub page: i64,
    /// 排序。空串表示默认排序，不会进查询参数。
    pub order: Option<String>,
    pub duration: Option<i64>,
    pub tids: Option<i64>,
    pub order_sort: Option<i64>,
    pub user_type: Option<i64>,
    pub category_id: Option<i64>,
    pub pub_begin: Option<i64>,
    pub pub_end: Option<i64>,
    pub gaia_vtoken: Option<String>,
}

impl SearchQuery {
    fn to_api(&self) -> ApiSearchParams {
        ApiSearchParams {
            search_type: self.search_type.clone(),
            keyword: self.keyword.clone(),
            page: self.page,
            order: self.order.clone(),
            duration: self.duration,
            tids: self.tids,
            order_sort: self.order_sort,
            user_type: self.user_type,
            category_id: self.category_id,
            pub_begin: self.pub_begin,
            pub_end: self.pub_end,
            gaia_vtoken: self.gaia_vtoken.clone(),
        }
    }
}

// ------------------------------------------------------------------ 实体

/// 一段标题文本。`is_em` 为真表示服务端把它标成了命中关键词。
#[derive(Debug, Clone)]
pub struct TitleSpan {
    pub is_em: bool,
    pub text: String,
}

/// UP 主。
#[derive(Debug, Clone)]
pub struct SearchOwner {
    pub mid: Option<i64>,
    pub name: Option<String>,
    pub face: Option<String>,
}

/// 条目统计。
#[derive(Debug, Clone)]
pub struct SearchStat {
    pub view: Option<i64>,
    pub danmaku: Option<i64>,
    pub favorite: Option<i64>,
    pub reply: Option<i64>,
    pub like: Option<i64>,
}

/// 认证信息。
#[derive(Debug, Clone)]
pub struct OfficialVerify {
    pub kind: Option<i64>,
    pub desc: Option<String>,
}

/// 视频条目。
#[derive(Debug, Clone)]
pub struct SearchVideoItem {
    pub id: Option<i64>,
    pub aid: Option<i64>,
    pub bvid: Option<String>,
    pub arcurl: Option<String>,
    pub title_spans: Vec<TitleSpan>,
    /// `title_spans` 拼接出的纯文本。
    pub title: String,
    pub description: Option<String>,
    pub pic: Option<String>,
    pub pubdate: Option<i64>,
    pub senddate: Option<i64>,
    /// **秒**。接口给的是 `"12:34"`，读不出来时为 0。
    pub duration_secs: i64,
    pub owner: SearchOwner,
    pub stat: SearchStat,
    /// `video` / `ketang` / `live_room` …。刻意不是枚举：服务端随时会加
    /// 新值，收成枚举会让新值变成解析错误。
    pub item_type: Option<String>,
    pub roomid: Option<i64>,
    pub is_union_video: Option<i64>,
    pub tag: Option<String>,
}

/// 用户条目（用户标签页）。
#[derive(Debug, Clone)]
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

/// 综合搜索里的用户卡片。比 [`SearchUserItem`] 多挂代表作，少几个字段
/// ——是接口的两个不同形状，不要合并。
#[derive(Debug, Clone)]
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
    pub res: Vec<SearchVideoItem>,
}

/// 直播间条目。
#[derive(Debug, Clone)]
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
    /// 已剥掉外层标签。
    pub cate_name: Option<String>,
}

/// 番剧/影视条目。
#[derive(Debug, Clone)]
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
    /// 评分。
    pub media_score: Option<MediaScore>,
    pub index_show: Option<String>,
}

/// 番剧评分。
#[derive(Debug, Clone)]
pub struct MediaScore {
    pub score: Option<f64>,
    pub user_count: Option<i64>,
}

/// 专栏条目。
#[derive(Debug, Clone)]
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

/// 综合搜索里的活动卡片。
///
/// 「只保留直播域名的活动」是界面决策（App 只能打开那种），过滤留在
/// Dart，这里原样带回。
#[derive(Debug, Clone)]
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

/// 热搜/默认词条目。
#[derive(Debug, Clone)]
pub struct SearchTrendingItem {
    pub keyword: String,
    pub show_name: String,
    pub icon: Option<String>,
    pub show_live_icon: Option<bool>,
    /// 原值。基线把首个中点换成空格再展示，那是排版，留给 Dart。
    pub recommend_reason: Option<String>,
}

// ------------------------------------------------------------------ 结果

/// 综合搜索的结果。
#[derive(Debug, Clone)]
pub struct SearchAllData {
    pub num_results: Option<i64>,
    pub videos: Vec<SearchVideoItem>,
    pub users: Vec<SearchUserCard>,
    pub medias: Vec<SearchPgcItem>,
    pub activities: Vec<SearchActivityItem>,
    /// 赛事卡片。
    ///
    /// 类型来自 [`crate::api::misc`]：赛事详情（S15 / #66）与这里是同一个
    /// 实体的两套字段名。S9 当时只能原样带回 JSON 串，因为 `MatchContest`
    /// 还有一个未迁的生产者；S15 迁完后按 ADR-011 决策六在这里展开。
    pub esports: Vec<crate::api::misc::MatchContest>,
}

// 下面七个结果结构的字段完全相同，只有 `data` 的类型不同。**刻意逐个
// 写开而不是用 macro_rules! 收掉**：flutter_rust_bridge 的 codegen 用 syn
// 解析源码，并不展开宏——宏生成的结构体与导出函数它一个都看不见，生成的
// 绑定会悄悄少掉这几个类型。重复在这里是代价最低的那一种。

/// 综合搜索的结果。
///
/// `num_results` 与 `data.num_results` 同值；外层那份是为了让七个结果
/// 类型形状一致，调用点取分页数不必先判 `data` 是否为空。
///
/// 失败时 `data` 为 `None`，错误以**值**的形式带回（ADR-011 决策一）。
#[derive(Debug, Clone)]
pub struct SearchAllResult {
    pub data: Option<SearchAllData>,
    /// 约有多少条结果。驱动分页与「约 N 条」文案。
    pub num_results: Option<i64>,
    /// `network|remote|protocol|auth|storage|player|cancelled`。
    pub error_kind: Option<String>,
    /// 可直接展示的文案，不含凭据、不含完整 URL。
    pub error_detail: Option<String>,
    /// 服务端业务码。风控拦截时为 `None`，靠 `error_kind == "auth"` 与
    /// 文案区分。
    pub error_code: Option<i64>,
}

/// 视频搜索的结果。字段语义同 [`SearchAllResult`]。
#[derive(Debug, Clone)]
pub struct SearchVideoResult {
    pub data: Option<Vec<SearchVideoItem>>,
    pub num_results: Option<i64>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

/// 用户搜索的结果。字段语义同 [`SearchAllResult`]。
#[derive(Debug, Clone)]
pub struct SearchUserResult {
    pub data: Option<Vec<SearchUserItem>>,
    pub num_results: Option<i64>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

/// 直播间搜索的结果。字段语义同 [`SearchAllResult`]。
#[derive(Debug, Clone)]
pub struct SearchLiveResult {
    pub data: Option<Vec<SearchLiveItem>>,
    pub num_results: Option<i64>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

/// 番剧/影视搜索的结果。字段语义同 [`SearchAllResult`]。
#[derive(Debug, Clone)]
pub struct SearchPgcResult {
    pub data: Option<Vec<SearchPgcItem>>,
    pub num_results: Option<i64>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

/// 专栏搜索的结果。字段语义同 [`SearchAllResult`]。
#[derive(Debug, Clone)]
pub struct SearchArticleResult {
    pub data: Option<Vec<SearchArticleItem>>,
    pub num_results: Option<i64>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

/// 热搜榜/默认词的结果。
///
/// 热搜榜那一支里 `num_results` 带的是**置顶条数**（置顶项已排在列表
/// 前面）；默认词那一支没有这个概念，为 `None`。
#[derive(Debug, Clone)]
pub struct SearchTrendingResult {
    pub data: Option<Vec<SearchTrendingItem>>,
    pub num_results: Option<i64>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

// ------------------------------------------------------------------ 导出

// 下面六个导出函数的骨架相同（spawn 到自己的 Tokio 运行时、压平
// JoinHandle、失败转成值），同样因为 codegen 不展开宏而逐个写开。

/// 视频搜索。
pub async fn search_video(context: SearchContext, query: SearchQuery) -> SearchVideoResult {
    let params = query.to_api();
    let (ctx, transport, mixin_key, wts) = context.split();

    let joined = init::runtime()
        .spawn(async move {
            let body = search::fetch_search(&transport, &ctx, &params, &mixin_key, wts).await?;
            search::parse_video(&body)
        })
        .await;

    match flatten(joined) {
        Ok(page) => SearchVideoResult {
            num_results: page.num_results,
            data: Some(page.items.into_iter().map(video_item).collect()),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => SearchVideoResult {
            data: None,
            num_results: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 用户搜索。
pub async fn search_user(context: SearchContext, query: SearchQuery) -> SearchUserResult {
    let params = query.to_api();
    let (ctx, transport, mixin_key, wts) = context.split();

    let joined = init::runtime()
        .spawn(async move {
            let body = search::fetch_search(&transport, &ctx, &params, &mixin_key, wts).await?;
            search::parse_user(&body)
        })
        .await;

    match flatten(joined) {
        Ok(page) => SearchUserResult {
            num_results: page.num_results,
            data: Some(page.items.into_iter().map(user_item).collect()),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => SearchUserResult {
            data: None,
            num_results: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 直播间搜索。
pub async fn search_live(context: SearchContext, query: SearchQuery) -> SearchLiveResult {
    let params = query.to_api();
    let (ctx, transport, mixin_key, wts) = context.split();

    let joined = init::runtime()
        .spawn(async move {
            let body = search::fetch_search(&transport, &ctx, &params, &mixin_key, wts).await?;
            search::parse_live(&body)
        })
        .await;

    match flatten(joined) {
        Ok(page) => SearchLiveResult {
            num_results: page.num_results,
            data: Some(page.items.into_iter().map(live_item).collect()),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => SearchLiveResult {
            data: None,
            num_results: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 番剧/影视搜索。
pub async fn search_pgc(context: SearchContext, query: SearchQuery) -> SearchPgcResult {
    let params = query.to_api();
    let (ctx, transport, mixin_key, wts) = context.split();

    let joined = init::runtime()
        .spawn(async move {
            let body = search::fetch_search(&transport, &ctx, &params, &mixin_key, wts).await?;
            search::parse_pgc(&body)
        })
        .await;

    match flatten(joined) {
        Ok(page) => SearchPgcResult {
            num_results: page.num_results,
            data: Some(page.items.into_iter().map(pgc_item).collect()),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => SearchPgcResult {
            data: None,
            num_results: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 专栏搜索。
pub async fn search_article(context: SearchContext, query: SearchQuery) -> SearchArticleResult {
    let params = query.to_api();
    let (ctx, transport, mixin_key, wts) = context.split();

    let joined = init::runtime()
        .spawn(async move {
            let body = search::fetch_search(&transport, &ctx, &params, &mixin_key, wts).await?;
            search::parse_article(&body)
        })
        .await;

    match flatten(joined) {
        Ok(page) => SearchArticleResult {
            num_results: page.num_results,
            data: Some(page.items.into_iter().map(article_item).collect()),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => SearchArticleResult {
            data: None,
            num_results: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 综合搜索。响应是分区数组，形状与其余五种不同。
pub async fn search_all(context: SearchContext, query: SearchQuery) -> SearchAllResult {
    let params = query.to_api();
    let (ctx, transport, mixin_key, wts) = context.split();

    let joined = init::runtime()
        .spawn(async move {
            let body = search::fetch_search(&transport, &ctx, &params, &mixin_key, wts).await?;
            search::parse_all(&body)
        })
        .await;

    match flatten(joined) {
        Ok(all) => SearchAllResult {
            num_results: all.num_results,
            data: Some(SearchAllData {
                num_results: all.num_results,
                videos: all.videos.into_iter().map(video_item).collect(),
                users: all.users.into_iter().map(user_card).collect(),
                medias: all.medias.into_iter().map(pgc_item).collect(),
                activities: all.activities.into_iter().map(activity_item).collect(),
                esports: all
                    .esports
                    .into_iter()
                    .map(crate::api::misc::match_contest)
                    .collect(),
            }),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => SearchAllResult {
            data: None,
            num_results: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 热搜榜。
///
/// `needs_top` 为真时把置顶项拼在列表前面，置顶条数由 `num_results`
/// 带回——复用这个字段而不是新开一个，是为了让八个结果形状保持一致。
pub async fn search_trending(
    context: SearchContext,
    limit: i64,
    needs_top: bool,
) -> SearchTrendingResult {
    let (ctx, transport, _, _) = context.split();

    let joined = init::runtime()
        .spawn(async move { search::fetch_trending(&transport, &ctx, limit, needs_top).await })
        .await;

    match flatten(joined) {
        Ok(trending) => SearchTrendingResult {
            num_results: Some(trending.top_count),
            data: Some(trending.list.into_iter().map(trending_item).collect()),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => SearchTrendingResult {
            data: None,
            num_results: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 搜索页默认词。
pub async fn search_recommend(
    context: SearchContext,
    appkey: String,
    appsec: String,
    access_key: Option<String>,
) -> SearchTrendingResult {
    let (ctx, transport, _, ts) = context.split();

    let joined = init::runtime()
        .spawn(async move {
            search::fetch_recommend(
                &transport,
                &ctx,
                &appkey,
                &appsec,
                access_key.as_deref(),
                ts,
            )
            .await
        })
        .await;

    match flatten(joined) {
        Ok(items) => SearchTrendingResult {
            num_results: None,
            data: Some(items.into_iter().map(trending_item).collect()),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => SearchTrendingResult {
            data: None,
            num_results: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
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

// -------------------------------------------------------------- 实体映射

fn spans(list: Vec<bili_domain::search::TitleSpan>) -> Vec<TitleSpan> {
    list.into_iter()
        .map(|s| TitleSpan {
            is_em: s.is_em,
            text: s.text,
        })
        .collect()
}

fn verify(value: Option<bili_domain::search::OfficialVerify>) -> Option<OfficialVerify> {
    value.map(|v| OfficialVerify {
        kind: v.kind,
        desc: v.desc,
    })
}

fn video_item(v: bili_domain::search::SearchVideoItem) -> SearchVideoItem {
    SearchVideoItem {
        id: v.id,
        aid: v.aid,
        bvid: v.bvid,
        arcurl: v.arcurl,
        title_spans: spans(v.title_spans),
        title: v.title,
        description: v.description,
        pic: v.pic,
        pubdate: v.pubdate,
        senddate: v.senddate,
        duration_secs: v.duration_secs,
        owner: SearchOwner {
            mid: v.owner.mid,
            name: v.owner.name,
            face: v.owner.face,
        },
        stat: SearchStat {
            view: v.stat.view,
            danmaku: v.stat.danmaku,
            favorite: v.stat.favorite,
            reply: v.stat.reply,
            like: v.stat.like,
        },
        item_type: v.item_type,
        roomid: v.roomid,
        is_union_video: v.is_union_video,
        tag: v.tag,
    }
}

fn user_item(u: bili_domain::search::SearchUserItem) -> SearchUserItem {
    SearchUserItem {
        item_type: u.item_type,
        mid: u.mid,
        uname: u.uname,
        usign: u.usign,
        fans: u.fans,
        videos: u.videos,
        upic: u.upic,
        face_nft: u.face_nft,
        face_nft_type: u.face_nft_type,
        verify_info: u.verify_info,
        level: u.level,
        gender: u.gender,
        is_up_user: u.is_up_user,
        is_live: u.is_live,
        room_id: u.room_id,
        official_verify: verify(u.official_verify),
        is_senior_member: u.is_senior_member,
    }
}

fn user_card(u: bili_domain::search::SearchUserCard) -> SearchUserCard {
    SearchUserCard {
        mid: u.mid,
        uname: u.uname,
        usign: u.usign,
        fans: u.fans,
        videos: u.videos,
        upic: u.upic,
        verify_info: u.verify_info,
        level: u.level,
        gender: u.gender,
        is_up_user: u.is_up_user,
        is_live: u.is_live,
        room_id: u.room_id,
        official_verify: verify(u.official_verify),
        is_senior_member: u.is_senior_member,
        res: u.res.into_iter().map(video_item).collect(),
    }
}

fn live_item(l: bili_domain::search::SearchLiveItem) -> SearchLiveItem {
    SearchLiveItem {
        rank_offset: l.rank_offset,
        uid: l.uid,
        tags: l.tags,
        live_time: l.live_time,
        uname: l.uname,
        uface: l.uface,
        user_cover: l.user_cover,
        item_type: l.item_type,
        title_spans: spans(l.title_spans),
        title: l.title,
        cover: l.cover,
        online: l.online,
        rank_index: l.rank_index,
        rank_score: l.rank_score,
        roomid: l.roomid,
        attentions: l.attentions,
        cate_name: l.cate_name,
    }
}

fn pgc_item(p: bili_domain::search::SearchPgcItem) -> SearchPgcItem {
    SearchPgcItem {
        item_type: p.item_type,
        media_id: p.media_id,
        title_spans: spans(p.title_spans),
        title: p.title,
        org_title: p.org_title,
        media_type: p.media_type,
        cv: p.cv,
        staff: p.staff,
        season_id: p.season_id,
        is_avid: p.is_avid,
        hit_epids: p.hit_epids,
        season_type: p.season_type,
        season_type_name: p.season_type_name,
        url: p.url,
        button_text: p.button_text,
        is_follow: p.is_follow,
        is_selection: p.is_selection,
        cover: p.cover,
        areas: p.areas,
        styles: p.styles,
        goto_url: p.goto_url,
        desc: p.desc,
        pubtime: p.pubtime,
        media_mode: p.media_mode,
        media_score: p.media_score.map(|m| MediaScore {
            score: m.score,
            user_count: m.user_count,
        }),
        index_show: p.index_show,
    }
}

fn article_item(a: bili_domain::search::SearchArticleItem) -> SearchArticleItem {
    SearchArticleItem {
        pub_time: a.pub_time,
        like: a.like,
        title_spans: spans(a.title_spans),
        title: a.title,
        rank_offset: a.rank_offset,
        mid: a.mid,
        image_urls: a.image_urls,
        id: a.id,
        category_id: a.category_id,
        view: a.view,
        reply: a.reply,
        desc: a.desc,
        rank_score: a.rank_score,
        item_type: a.item_type,
        template_id: a.template_id,
        category_name: a.category_name,
    }
}

fn activity_item(a: bili_domain::search::SearchActivityItem) -> SearchActivityItem {
    SearchActivityItem {
        id: a.id,
        title: a.title,
        desc: a.desc,
        cover: a.cover,
        url: a.url,
        state: a.state,
        status: a.status,
        author: a.author,
    }
}

fn trending_item(t: bili_domain::search::SearchTrendingItem) -> SearchTrendingItem {
    SearchTrendingItem {
        keyword: t.keyword,
        show_name: t.show_name,
        icon: t.icon,
        show_live_icon: t.show_live_icon,
        recommend_reason: t.recommend_reason,
    }
}
