//! 视频流五条只读接口的 Dart 导出（S3 / issue #53）。
//!
//! 形态照 `sponsor_block.rs` 这块样板：typed 结构直连，失败用值不用异常，
//! 设置与凭据按请求传入（ADR-011）。本模块与样板的三点不同：
//!
//! 1. **走 bilibili 的业务信封**，因此 `error_code` 是业务 `code`
//!    （如 -404「啥都木有」），不是 HTTP 状态码。
//! 2. **带 WBI 签名**。`mixin_key` 由 Dart 的按日缓存提供，`wts` 由 Dart
//!    注入——与已迁移的详情/取流路径同一套做法，请求构造因此可被 fixture
//!    固定。
//! 3. **带过滤规则**。热门/排行/相关推荐要按用户设置过滤，规则在
//!    `bili_domain::video_feed`，取值随每次请求传入。
//!
//! ## 屏蔽词正则编不过时为什么整条请求失败
//!
//! typed 直连没有降级开关（ADR-011 决策四）。忽略一条编不过的规则意味着
//! 用户明确屏蔽过的内容照常出现，而且没有任何提示；整条失败至少能让界面
//! 说清「屏蔽词规则不被支持，请修改设置」。少看一屏可以重试，看到拉黑的
//! 内容不能撤回。这类失败的 `error_kind` 是 `unsupported`。

use std::time::Duration;

use bili_api::http::RequestContext;
use bili_api::transport_reqwest::ReqwestTransport;
use bili_api::video_feed;
use bili_domain::error::DomainError;
use bili_domain::redact::Redacted;
use bili_domain::video_feed as domain;

use crate::api::init;
use crate::api::transport::TransportSettings;

/// 一条视频流条目。字段语义见 `bili_domain::video_feed::FeedVideoItem`。
#[derive(Debug, Clone)]
pub struct FeedVideoItem {
    pub aid: Option<i64>,
    pub cid: Option<i64>,
    pub bvid: Option<String>,
    pub videos: Option<i64>,
    pub tid: Option<i64>,
    pub tname: Option<String>,
    pub copyright: Option<i64>,
    pub cover: Option<String>,
    pub title: Option<String>,
    pub pubdate: Option<i64>,
    pub ctime: Option<i64>,
    pub desc: Option<String>,
    pub state: Option<i64>,
    /// 秒。未知时为 `None`（基线用 -1 表示同一件事）。
    pub duration: Option<i64>,
    pub owner_mid: Option<i64>,
    pub owner_name: Option<String>,
    pub owner_face: Option<String>,
    pub stat: FeedStat,
    /// 已按 `rotate` 归正。
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub first_frame: Option<String>,
    pub pub_location: Option<String>,
    pub redirect_url: Option<String>,
    pub progress: Option<f64>,
    /// 角标来源：`charging` / `cooperation`。文案由 Dart 决定。
    pub badge_kind: Option<String>,
    /// 服务端给的角标文案，仅在 `badge_kind` 为空时使用。
    pub pgc_label: Option<String>,
}

/// 计数。
#[derive(Debug, Clone)]
pub struct FeedStat {
    /// 播放量。不公开时服务端给的是字符串，这里为 `None`。
    pub view: Option<i64>,
    pub like: Option<i64>,
    pub danmaku: Option<i64>,
    pub reply: Option<i64>,
    pub favorite: Option<i64>,
    pub coin: Option<f64>,
    pub share: Option<i64>,
    pub now_rank: Option<i64>,
    pub his_rank: Option<i64>,
    pub dislike: Option<i64>,
    pub vt: Option<i64>,
    pub vv: Option<i64>,
}

/// 每周必看的一期。
#[derive(Debug, Clone)]
pub struct PopularSeriesOne {
    pub config: Option<PopularSeriesConfig>,
    pub reminder: Option<String>,
    /// `None` 表示响应里根本没有 list（协议形态变了），空列表表示这一期
    /// 没有内容——UI 的空态文案不同。
    pub list: Option<Vec<FeedVideoItem>>,
}

/// 每周必看一期的标题信息。
#[derive(Debug, Clone)]
pub struct PopularSeriesConfig {
    pub name: Option<String>,
    pub label: Option<String>,
    pub media_id: Option<i64>,
}

/// 每周必看的期号。
#[derive(Debug, Clone)]
pub struct PopularSeriesListItem {
    pub number: Option<i64>,
    pub subject: Option<String>,
    pub status: Option<i64>,
    pub name: Option<String>,
}

/// 入站必刷。
#[derive(Debug, Clone)]
pub struct PopularPrecious {
    pub media_id: Option<i64>,
    pub list: Option<Vec<FeedVideoItem>>,
}

/// 列表结果。`error_code` 是业务 code。
#[derive(Debug, Clone)]
pub struct FeedListResult {
    pub data: Option<Vec<FeedVideoItem>>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

/// 每周必看期号列表的结果。
#[derive(Debug, Clone)]
pub struct PopularSeriesListResult {
    pub data: Option<Vec<PopularSeriesListItem>>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

/// 每周必看某一期的结果。
#[derive(Debug, Clone)]
pub struct PopularSeriesOneResult {
    pub data: Option<PopularSeriesOne>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

/// 入站必刷的结果。
#[derive(Debug, Clone)]
pub struct PopularPreciousResult {
    pub data: Option<PopularPrecious>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

/// 过滤规则的取值，取自 Dart 的 `Pref` 与 `GlobalData().blackMids`。
#[derive(Debug, Clone)]
pub struct FeedFilterSettings {
    pub blocked_mids: Vec<i64>,
    /// 推荐屏蔽词正则源串。空串=不启用。
    pub ban_word_rcmd: String,
    /// 分区屏蔽词正则源串。空串=不启用。
    pub ban_word_zone: String,
    pub min_duration: i64,
    pub min_play: i64,
    pub min_like_ratio: i64,
    pub apply_to_related: bool,
}

/// 一次请求所需的公共参数。
///
/// 单独成结构而不是摊平：本模块有六个导出，每个都要这一组，摊平后加一个
/// 字段要改六处签名，还得同步改 Dart 的六个调用点。
#[derive(Debug, Clone)]
pub struct FeedContext {
    pub base_url: String,
    /// 完整的 Cookie 头值。未登录时为 `None`。
    pub cookie: Option<String>,
    /// 其余请求头。每周必看与入站必刷要的桌面端 UA / origin / referer 也
    /// 走这里——那几个值的真相源在 Dart。
    pub headers: Vec<(String, String)>,
    pub timeout_ms: u64,
    pub transport: TransportSettings,
    /// WBI mixin key，由 Dart 的按日缓存提供。未签名的接口（热门、相关
    /// 推荐）用不到，传空串即可。
    pub mixin_key: String,
    /// Unix 秒，由 Dart 注入。
    pub wts: i64,
    pub filter: FeedFilterSettings,
}

impl FeedContext {
    fn split(
        self,
    ) -> (
        RequestContext,
        ReqwestTransport,
        String,
        i64,
        FeedFilterSettings,
    ) {
        let transport = ReqwestTransport::new(self.transport.to_config());
        let ctx = RequestContext {
            base_url: self.base_url,
            cookie: self.cookie.map(Redacted::new),
            headers: self.headers,
            timeout: Duration::from_millis(self.timeout_ms),
        };
        (ctx, transport, self.mixin_key, self.wts, self.filter)
    }
}

/// 取热门列表。
pub async fn fetch_hot_videos(context: FeedContext, pn: i64, ps: i64) -> FeedListResult {
    let (ctx, transport, _, _, filter) = context.split();
    let joined = init::runtime()
        .spawn(async move {
            let filter = compile(&filter)?;
            video_feed::fetch_hot(&transport, &ctx, &filter, pn, ps).await
        })
        .await;
    list_result(flatten(joined))
}

/// 取排行榜。`rid` 为分区 id，0 是全站。
pub async fn fetch_rank_videos(context: FeedContext, rid: i64) -> FeedListResult {
    let (ctx, transport, mixin_key, wts, filter) = context.split();
    let joined = init::runtime()
        .spawn(async move {
            let filter = compile(&filter)?;
            video_feed::fetch_rank(&transport, &ctx, &filter, rid, &mixin_key, wts).await
        })
        .await;
    list_result(flatten(joined))
}

/// 取相关推荐。
pub async fn fetch_related_videos(context: FeedContext, bvid: String) -> FeedListResult {
    let (ctx, transport, _, _, filter) = context.split();
    let joined = init::runtime()
        .spawn(async move {
            let filter = compile(&filter)?;
            video_feed::fetch_related(&transport, &ctx, &filter, &bvid).await
        })
        .await;
    list_result(flatten(joined))
}

/// 取每周必看的期号列表。
pub async fn fetch_popular_series_list(context: FeedContext) -> PopularSeriesListResult {
    let (ctx, transport, mixin_key, wts, _) = context.split();
    let joined = init::runtime()
        .spawn(async move {
            video_feed::fetch_popular_series_list(&transport, &ctx, &mixin_key, wts).await
        })
        .await;

    match flatten(joined) {
        Ok(list) => PopularSeriesListResult {
            data: Some(
                list.into_iter()
                    .map(PopularSeriesListItem::from_domain)
                    .collect(),
            ),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => PopularSeriesListResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 取每周必看的某一期。基线对这条不做过滤，这里也不做。
pub async fn fetch_popular_series_one(context: FeedContext, number: i64) -> PopularSeriesOneResult {
    let (ctx, transport, mixin_key, wts, _) = context.split();
    let joined = init::runtime()
        .spawn(async move {
            video_feed::fetch_popular_series_one(&transport, &ctx, number, &mixin_key, wts).await
        })
        .await;

    match flatten(joined) {
        Ok(one) => PopularSeriesOneResult {
            data: Some(PopularSeriesOne {
                config: one.config.map(|c| PopularSeriesConfig {
                    name: c.name,
                    label: c.label,
                    media_id: c.media_id,
                }),
                reminder: one.reminder,
                list: one
                    .list
                    .map(|l| l.into_iter().map(FeedVideoItem::from_domain).collect()),
            }),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => PopularSeriesOneResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 取入站必刷。基线对这条同样不过滤。
pub async fn fetch_popular_precious(context: FeedContext, page: i64) -> PopularPreciousResult {
    let (ctx, transport, mixin_key, wts, _) = context.split();
    let joined = init::runtime()
        .spawn(async move {
            video_feed::fetch_popular_precious(&transport, &ctx, page, &mixin_key, wts).await
        })
        .await;

    match flatten(joined) {
        Ok(precious) => PopularPreciousResult {
            data: Some(PopularPrecious {
                media_id: precious.media_id,
                list: precious
                    .list
                    .map(|l| l.into_iter().map(FeedVideoItem::from_domain).collect()),
            }),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => PopularPreciousResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

fn compile(settings: &FeedFilterSettings) -> Result<domain::FeedFilter, DomainError> {
    domain::FeedFilter::compile(&domain::FeedFilterSettings {
        blocked_mids: settings.blocked_mids.clone(),
        ban_word_rcmd: settings.ban_word_rcmd.clone(),
        ban_word_zone: settings.ban_word_zone.clone(),
        min_duration: settings.min_duration,
        min_play: settings.min_play,
        min_like_ratio: settings.min_like_ratio,
        apply_to_related: settings.apply_to_related,
    })
}

fn list_result(result: Result<Vec<domain::FeedVideoItem>, DomainError>) -> FeedListResult {
    match result {
        Ok(items) => FeedListResult {
            data: Some(items.into_iter().map(FeedVideoItem::from_domain).collect()),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => FeedListResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 把 `JoinHandle` 的两层结果压平。任务 panic 不传给 Dart——宿主 App 不该
/// 因为一次取数而崩溃。
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

impl FeedVideoItem {
    fn from_domain(i: domain::FeedVideoItem) -> Self {
        Self {
            aid: i.aid,
            cid: i.cid,
            bvid: i.bvid,
            videos: i.videos,
            tid: i.tid,
            tname: i.tname,
            copyright: i.copyright,
            cover: i.cover,
            title: i.title,
            pubdate: i.pubdate,
            ctime: i.ctime,
            desc: i.desc,
            state: i.state,
            duration: i.duration,
            owner_mid: i.owner.mid,
            owner_name: i.owner.name,
            owner_face: i.owner.face,
            stat: FeedStat {
                view: i.stat.view,
                like: i.stat.like,
                danmaku: i.stat.danmaku,
                reply: i.stat.reply,
                favorite: i.stat.favorite,
                coin: i.stat.coin,
                share: i.stat.share,
                now_rank: i.stat.now_rank,
                his_rank: i.stat.his_rank,
                dislike: i.stat.dislike,
                vt: i.stat.vt,
                vv: i.stat.vv,
            },
            width: i.dimension.as_ref().and_then(|d| d.width),
            height: i.dimension.as_ref().and_then(|d| d.height),
            first_frame: i.first_frame,
            pub_location: i.pub_location,
            redirect_url: i.redirect_url,
            progress: i.progress,
            badge_kind: i.badge_kind,
            pgc_label: i.pgc_label,
        }
    }
}

impl PopularSeriesListItem {
    fn from_domain(i: domain::PopularSeriesListItem) -> Self {
        Self {
            number: i.number,
            subject: i.subject,
            status: i.status,
            name: i.name,
        }
    }
}
