//! 历史记录、稍后再看与播放列表的 Dart 导出（S7 / issue #57）。
//!
//! 基线对应的 `UserHttp` 剩下十一个函数已随本次迁移删除，按 ADR-011
//! 决策四不留降级开关。
//!
//! ## 导出结构里没有 Widget，也没有 `checked`
//!
//! 基线 `HistoryItemModel` / `LaterItemModel` 上挂着返回 Widget 的 getter
//! （角标、进度条）与多选用的可变 `checked`。前者是渲染，后者是界面状态，
//! 两者都留在宿主（AGENTS.md §2）；这里只出判据所需的原始值。
//!
//! ## 账号由 Dart 给对
//!
//! 历史那几条在基线里显式走 `Accounts.history`，不是按路径查表的那个。
//! Rust 不认账号，Cookie 与 csrf 都按请求传进来——迁移时最容易丢的就是
//! 这一条。

use std::time::Duration;

use bili_api::history;
use bili_api::http::RequestContext;
use bili_api::transport_reqwest::ReqwestTransport;
use bili_domain::error::DomainError;
use bili_domain::redact::Redacted;

use crate::api::init;
use crate::api::transport::TransportSettings;
use crate::api::write::WriteResult;

/// 一页观看历史。
#[derive(Debug, Clone)]
pub struct HistoryPage {
    /// 顶部的业务分页签。只有首次拉取给。
    pub tabs: Vec<HistoryTab>,
    pub list: Vec<HistoryItem>,
}

/// 一个业务分页签。`kind` 对应协议的 `type`。
#[derive(Debug, Clone)]
pub struct HistoryTab {
    pub kind: Option<String>,
    pub name: Option<String>,
}

/// 一条观看历史。
#[derive(Debug, Clone)]
pub struct HistoryItem {
    pub title: Option<String>,
    pub cover: Option<String>,
    /// 多封面（合集类条目）。`cover` 为空时取第一张。
    pub covers: Vec<String>,
    pub uri: Option<String>,
    pub videos: Option<i64>,
    pub author_name: Option<String>,
    pub author_mid: Option<i64>,
    /// 观看时间，Unix 秒。
    pub view_at: Option<i64>,
    /// 观看进度，**秒**；`-1` 已看完。续播位置用 [`playback_progress_ms`]。
    pub progress: Option<i64>,
    pub badge: Option<String>,
    pub show_title: Option<String>,
    pub duration: Option<i64>,
    /// `1` 已收藏。界面优先显示「已收藏」角标而不是 `badge`。
    pub is_fav: Option<i64>,
    /// 历史条目 id，删除时用。
    pub kid: Option<i64>,
    pub tag_name: Option<String>,
    /// 直播条目的开播状态，`1` 为直播中。角标颜色按它决定。
    pub live_status: Option<i64>,
    pub history: HistoryRef,
}

/// 历史条目指向的那个东西。`cid` 的 0 已归一成 `None`。
#[derive(Debug, Clone)]
pub struct HistoryRef {
    pub oid: Option<i64>,
    pub epid: Option<i64>,
    pub bvid: Option<String>,
    pub page: Option<i64>,
    pub cid: Option<i64>,
    pub business: Option<String>,
}

/// 一页稍后再看。
#[derive(Debug, Clone)]
pub struct LaterPage {
    pub count: Option<i64>,
    pub list: Vec<LaterItem>,
}

/// 一条稍后再看。
#[derive(Debug, Clone)]
pub struct LaterItem {
    pub aid: Option<i64>,
    pub bvid: Option<String>,
    pub pic: Option<String>,
    pub title: Option<String>,
    /// 番剧条目去掉季名前缀之后的分集名。非番剧为 `None`。
    pub subtitle: Option<String>,
    pub pubdate: Option<i64>,
    pub duration: Option<i64>,
    pub redirect_url: Option<String>,
    /// `1` 联合投稿，界面显示「合作」角标。
    pub is_cooperation: Option<i64>,
    pub owner_mid: Option<i64>,
    pub owner_name: Option<String>,
    pub owner_face: Option<String>,
    pub view: Option<i64>,
    pub danmaku: Option<i64>,
    pub season_title: Option<String>,
    pub ep_id: Option<i64>,
    pub cid: Option<i64>,
    /// 观看进度，**秒**；`-1` 已看完。
    pub progress: Option<i64>,
    pub is_pgc: Option<bool>,
    /// 空串已归一成 `None`。
    pub pgc_label: Option<String>,
    pub is_pugv: Option<bool>,
    /// 充电专属。判据是 `charging_pay.level` 存在与否。
    pub is_charging: bool,
    /// **已按 `rotate` 摆正**的宽高，见领域层。
    pub width: Option<i64>,
    pub height: Option<i64>,
}

/// 播放列表，一页。
#[derive(Debug, Clone)]
pub struct MediaList {
    pub list: Vec<MediaListItem>,
}

/// 播放列表里的一项。
#[derive(Debug, Clone)]
pub struct MediaListItem {
    pub aid: Option<i64>,
    pub bvid: Option<String>,
    pub title: Option<String>,
    pub intro: Option<String>,
    pub cover: Option<String>,
    pub duration: Option<i64>,
    pub badge: Option<String>,
    /// `kind` 对应协议的 `type`。
    pub kind: Option<i64>,
    pub upper_mid: Option<i64>,
    pub upper_name: Option<String>,
    pub upper_face: Option<String>,
    pub play: Option<i64>,
    pub danmaku: Option<i64>,
    /// 第一个分 P 的 cid。
    pub first_page_cid: Option<i64>,
}

/// 播放列表请求的业务入参。
///
/// 做成结构而不是九个形参：它们没有自然顺序（三个布尔连在一起），调用点
/// 写错两个的位置不会报错，只会取回另一段列表。
#[derive(Debug, Clone)]
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

/// 一页历史的结果。
#[derive(Debug, Clone)]
pub struct HistoryPageResult {
    pub data: Option<HistoryPage>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

/// 「历史是否已暂停」的结果。
#[derive(Debug, Clone)]
pub struct HistoryStatusResult {
    pub data: Option<bool>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

/// 一页稍后再看的结果。
#[derive(Debug, Clone)]
pub struct LaterPageResult {
    pub data: Option<LaterPage>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

/// 播放列表的结果。
#[derive(Debug, Clone)]
pub struct MediaListResult {
    pub data: Option<MediaList>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

/// 一次请求所需的公共参数。
#[derive(Debug, Clone)]
pub struct HistoryContext {
    pub base_url: String,
    /// 完整的 Cookie 头值。**历史那几条要的是 `Accounts.history` 的**，
    /// 由 Dart 选对账号之后传进来。
    pub cookie: Option<String>,
    pub headers: Vec<(String, String)>,
    /// CSRF token。只有写操作要传，且同样来自对应的那个账号。
    pub csrf: Option<String>,
    pub timeout_ms: u64,
    pub transport: TransportSettings,
    /// WBI mixin key。只有稍后再看列表签名，其余传空串即可。
    pub mixin_key: String,
    /// Unix 秒，由 Dart 注入。
    pub wts: i64,
}

impl HistoryContext {
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

// ------------------------------------------------------------------ 只读

/// 取一页观看历史。
pub async fn fetch_history_list(
    context: HistoryContext,
    kind: String,
    page_size: i64,
    max: Option<i64>,
    view_at: Option<i64>,
) -> HistoryPageResult {
    let (ctx, transport, _, _) = context.split();
    let joined = init::runtime()
        .spawn(async move {
            history::fetch_history_list(&transport, &ctx, &kind, page_size, max, view_at).await
        })
        .await;
    history_page_result(flatten(joined))
}

/// 搜索历史记录。响应与 [`fetch_history_list`] 同形。
pub async fn fetch_search_history(
    context: HistoryContext,
    page: i64,
    keyword: String,
) -> HistoryPageResult {
    let (ctx, transport, _, _) = context.split();
    let joined = init::runtime()
        .spawn(async move { history::fetch_search_history(&transport, &ctx, page, &keyword).await })
        .await;
    history_page_result(flatten(joined))
}

/// 取「历史记录是否已暂停」。
pub async fn fetch_history_status(context: HistoryContext) -> HistoryStatusResult {
    let (ctx, transport, _, _) = context.split();
    let joined = init::runtime()
        .spawn(async move { history::fetch_history_status(&transport, &ctx).await })
        .await;

    match flatten(joined) {
        Ok(paused) => HistoryStatusResult {
            data: Some(paused),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => HistoryStatusResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 取一页稍后再看。WBI 签名。
pub async fn fetch_see_you_later(
    context: HistoryContext,
    page: i64,
    page_size: i64,
    viewed: i64,
    keyword: String,
    asc: bool,
) -> LaterPageResult {
    let (ctx, transport, mixin_key, wts) = context.split();
    let joined = init::runtime()
        .spawn(async move {
            history::fetch_see_you_later(
                &transport, &ctx, page, page_size, viewed, &keyword, asc, &mixin_key, wts,
            )
            .await
        })
        .await;

    match flatten(joined) {
        Ok(p) => LaterPageResult {
            data: Some(LaterPage {
                count: p.count,
                list: p.list.into_iter().map(later_item).collect(),
            }),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => LaterPageResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 取播放列表一页。
pub async fn fetch_media_list(context: HistoryContext, params: MediaListParams) -> MediaListResult {
    let (ctx, transport, _, _) = context.split();
    let joined = init::runtime()
        .spawn(async move {
            let p = history::MediaListParams {
                kind: params.kind,
                biz_id: params.biz_id,
                oid: params.oid,
                otype: params.otype,
                page_size: params.page_size,
                direction: params.direction,
                desc: params.desc,
                sort_field: params.sort_field,
                with_current: params.with_current,
            };
            history::fetch_media_list(&transport, &ctx, &p).await
        })
        .await;

    match flatten(joined) {
        Ok(l) => MediaListResult {
            data: Some(MediaList {
                list: l
                    .list
                    .into_iter()
                    .map(|i| MediaListItem {
                        aid: i.aid,
                        bvid: i.bvid,
                        title: i.title,
                        intro: i.intro,
                        cover: i.cover,
                        duration: i.duration,
                        badge: i.badge,
                        kind: i.kind,
                        upper_mid: i.upper_mid,
                        upper_name: i.upper_name,
                        upper_face: i.upper_face,
                        play: i.play,
                        danmaku: i.danmaku,
                        first_page_cid: i.first_page_cid,
                    })
                    .collect(),
            }),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => MediaListResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

// -------------------------------------------------------------------- 写

/// 暂停/恢复记录历史。置为语义，重发安全。
pub async fn pause_history(context: HistoryContext, paused: bool) -> WriteResult {
    let (ctx, transport, _, _) = context.split();
    let joined = init::runtime()
        .spawn(async move { history::pause_history(&transport, &ctx, paused).await })
        .await;
    WriteResult::from_result(flatten(joined))
}

/// 清空历史记录。**不可逆**，失败不自动重发。
pub async fn clear_history(context: HistoryContext) -> WriteResult {
    let (ctx, transport, _, _) = context.split();
    let joined = init::runtime()
        .spawn(async move { history::clear_history(&transport, &ctx).await })
        .await;
    WriteResult::from_result(flatten(joined))
}

/// 删除一条历史记录。`kid` 形如 `archive_123456`。
pub async fn del_history(context: HistoryContext, kid: String) -> WriteResult {
    let (ctx, transport, _, _) = context.split();
    let joined = init::runtime()
        .spawn(async move { history::del_history(&transport, &ctx, &kid).await })
        .await;
    WriteResult::from_result(flatten(joined))
}

/// 加入稍后再看。`aid` 与 `bvid` 至少给一个。
pub async fn to_view_later(
    context: HistoryContext,
    aid: Option<i64>,
    bvid: Option<String>,
) -> WriteResult {
    let (ctx, transport, _, _) = context.split();
    let joined = init::runtime()
        .spawn(async move { history::to_view_later(&transport, &ctx, aid, bvid.as_deref()).await })
        .await;
    WriteResult::from_result(flatten(joined))
}

/// 从稍后再看移除。`resources` 是逗号分隔的 aid 串。
pub async fn to_view_del(context: HistoryContext, resources: String) -> WriteResult {
    let (ctx, transport, _, _) = context.split();
    let joined = init::runtime()
        .spawn(async move { history::to_view_del(&transport, &ctx, &resources).await })
        .await;
    WriteResult::from_result(flatten(joined))
}

/// 清空稍后再看。**不可逆**，失败不自动重发。
pub async fn to_view_clear(context: HistoryContext, clean_type: Option<i64>) -> WriteResult {
    let (ctx, transport, _, _) = context.split();
    let joined = init::runtime()
        .spawn(async move { history::to_view_clear(&transport, &ctx, clean_type).await })
        .await;
    WriteResult::from_result(flatten(joined))
}

// ------------------------------------------------------------------ 换算

/// 续播位置，**毫秒**。
///
/// 两条规则都在 Rust：历史接口以秒计而播放器要毫秒；`-1`（已看完）要换成
/// `0`，否则重新打开会从末尾续播。`#[frb(sync)]`——它是纯算术，在列表项
/// 点击的路径上，异步化会让调用点也得变异步（ADR-010）。
#[must_use]
#[flutter_rust_bridge::frb(sync)]
pub fn playback_progress_ms(progress: Option<i64>) -> Option<i64> {
    bili_domain::user::HistoryItem {
        progress,
        ..Default::default()
    }
    .playback_progress_ms()
}

fn history_page_result(
    r: Result<bili_domain::user::HistoryPage, DomainError>,
) -> HistoryPageResult {
    match r {
        Ok(p) => HistoryPageResult {
            data: Some(HistoryPage {
                tabs: p
                    .tabs
                    .into_iter()
                    .map(|t| HistoryTab {
                        kind: t.kind,
                        name: t.name,
                    })
                    .collect(),
                list: p
                    .list
                    .into_iter()
                    .map(|i| HistoryItem {
                        title: i.title,
                        cover: i.cover,
                        covers: i.covers,
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
                        history: HistoryRef {
                            oid: i.history.oid,
                            epid: i.history.epid,
                            bvid: i.history.bvid,
                            page: i.history.page,
                            cid: i.history.cid,
                            business: i.history.business,
                        },
                    })
                    .collect(),
            }),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => HistoryPageResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

fn later_item(i: bili_domain::user::LaterItem) -> LaterItem {
    LaterItem {
        aid: i.aid,
        bvid: i.bvid,
        pic: i.pic,
        title: i.title,
        subtitle: i.subtitle,
        pubdate: i.pubdate,
        duration: i.duration,
        redirect_url: i.redirect_url,
        is_cooperation: i.is_cooperation,
        owner_mid: i.owner_mid,
        owner_name: i.owner_name,
        owner_face: i.owner_face,
        view: i.view,
        danmaku: i.danmaku,
        season_title: i.season_title,
        ep_id: i.ep_id,
        cid: i.cid,
        progress: i.progress,
        is_pgc: i.is_pgc,
        pgc_label: i.pgc_label,
        is_pugv: i.is_pugv,
        is_charging: i.is_charging,
        width: i.width,
        height: i.height,
    }
}

/// 把 `JoinHandle` 的两层结果压平。任务 panic 不传给 Dart。
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
