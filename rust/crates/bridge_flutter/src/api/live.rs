//! 直播只读接口的 Dart 导出（S11a 第一、二批 / issue #61）。
//!
//! 照 ADR-011 的 typed 直连样板写：结果用**值**表达失败而不是 `Result`，
//! 实体字段保持 `Option`，凭据按请求传入。
//!
//! ## 逐个写开，不要用宏收
//!
//! 下面八个结果结构的字段完全相同，八个取数函数的骨架也相同。**刻意不用
//! `macro_rules!`**：flutter_rust_bridge 的 codegen 用 `syn` 解析源码、
//! 不展开宏，宏生成的结构体与导出函数它一个都看不见，而且**不报错**
//! ——生成的绑定会静默少掉这些类型。详见 ADR-011 的 S9 补充一。
//!
//! ## 账号按接口分三个桶
//!
//! `ApiType.apiTypeSet` 把 `liveAreaList` / `liveFeedIndex` /
//! `liveSecondList` / `liveRoomAreaList` 归在 `recommend`，
//! `liveRoomInfoH5` 与 `liveRoomInfo`（取流）归在 `heartbeat`，
//! `getLiveFavTag` 与 `liveFollow` 没有列进去、落到默认的 `main`。
//! Dart 侧按 path 查表选账号，不在这里猜。
//!
//! 取流归 `heartbeat` 而不是 `video` 是容易弄错的一处：`ApiType` 里
//! `video` 那个桶装的是**视频**取流（`ugcUrl`/`pgcUrl`/…），直播取流
//! 不在其中。按 path 查表就不会错，硬编码迟早错，且错了不报错——只是
//! 悄悄换了一个账号的 Cookie。

use std::time::Duration;

use bili_api::http::RequestContext;
use bili_api::live::{self, AppIdentity};
use bili_api::transport_reqwest::ReqwestTransport;
use bili_domain::error::DomainError;
use bili_domain::redact::Redacted;

use crate::api::init;
use crate::api::transport::TransportSettings;

// ------------------------------------------------------------------ 入参

/// 一次直播请求的公共参数。
///
/// `base_url` 是 `HttpString.liveBaseUrl`（`api.live.bilibili.com`），
/// 与其余接口的 `api.bilibili.com` 不是同一个主机，由 Dart 侧给出。
#[derive(Debug, Clone)]
pub struct LiveContext {
    pub base_url: String,
    /// 完整的 Cookie 头值。未登录时为 `None`。**Rust 不保存**，用完即弃。
    pub cookie: Option<String>,
    pub headers: Vec<(String, String)>,
    pub timeout_ms: u64,
    pub transport: TransportSettings,
}

impl LiveContext {
    fn split(self) -> (RequestContext, ReqwestTransport) {
        let transport = ReqwestTransport::new(self.transport.to_config());
        let ctx = RequestContext {
            base_url: self.base_url,
            cookie: self.cookie.map(Redacted::new),
            headers: self.headers,
            timeout: Duration::from_millis(self.timeout_ms),
        };
        (ctx, transport)
    }
}

/// 本机/本次会话的身份值。
///
/// 这些值的真相源在 Dart（`lib/common/constants.dart` 与账号体系），
/// 还有若干未迁接口在用，复制到 Rust 会出现两个真相源。`ts` 由 Dart
/// 注入，使请求构造保持纯函数。
#[derive(Debug, Clone)]
pub struct LiveAppIdentity {
    pub appkey: String,
    pub appsec: String,
    pub build: i64,
    pub version: String,
    /// `Constants.statisticsApp`，一段 JSON。
    pub statistics: String,
    /// `Constants.userAgentApp`。只有两条 feed 接口用得到。
    pub user_agent: String,
    pub buvid: String,
    /// `Constants.traceId`。
    pub trace_id: String,
    /// 未登录时为 `None`，此时整个 `access_key` 参数不出现。
    pub access_key: Option<String>,
    /// 是否已登录。只影响首页推荐的 `relation_page`，不要用
    /// `access_key != null` 代替。
    pub logged_in: bool,
    pub ts: i64,
}

impl LiveAppIdentity {
    fn into_api(self) -> AppIdentity {
        AppIdentity {
            appkey: self.appkey,
            appsec: self.appsec,
            build: self.build,
            version: self.version,
            statistics: self.statistics,
            user_agent: self.user_agent,
            buvid: self.buvid,
            trace_id: self.trace_id,
            access_key: self.access_key,
            logged_in: self.logged_in,
            ts: self.ts,
        }
    }
}

// ------------------------------------------------------------------ 实体

/// 直播分区。
///
/// `id` / `parent_id` 已归一成整数——接口在不同入口下给字符串或整数，
/// 基线原样透传导致相等比较恒为假，见 `bili-api` 侧的说明。
/// `parent_id` 为 `-1` 表示该接口没有回这个字段（收藏分区）。
#[derive(Debug, Clone)]
pub struct LiveArea {
    pub id: i64,
    pub name: Option<String>,
    pub pic: Option<String>,
    pub parent_id: i64,
    pub parent_name: Option<String>,
}

/// 一级分区及其下的二级分区。
#[derive(Debug, Clone)]
pub struct LiveAreaGroup {
    pub name: String,
    pub areas: Vec<LiveArea>,
}

/// 反馈里的一条理由。
#[derive(Debug, Clone)]
pub struct LiveFeedbackReason {
    pub id: Option<i64>,
    pub name: Option<String>,
    pub id_type: Option<String>,
    pub reason_id: Option<i64>,
}

/// 卡片上的「不感兴趣」入口。
#[derive(Debug, Clone)]
pub struct LiveFeedback {
    pub title: Option<String>,
    pub subtitle: Option<String>,
    /// 响应里的键是 `type`。
    pub kind: Option<String>,
    pub reasons: Vec<LiveFeedbackReason>,
}

/// 直播间卡片。首页推荐、分区二级列表、分区入口、关注区块共用一种。
#[derive(Debug, Clone)]
pub struct LiveRoomCard {
    pub roomid: Option<i64>,
    pub uid: Option<i64>,
    pub uname: Option<String>,
    pub face: Option<String>,
    pub cover: Option<String>,
    /// 已按基线 getter 的口径回落到 `cover`（空串算缺失）。
    pub system_cover: Option<String>,
    pub title: Option<String>,
    pub area_name: Option<String>,
    pub area_v2_id: Option<i64>,
    pub area_v2_parent_id: Option<i64>,
    /// 形如「1.2万人看过」。
    pub watched_text: Option<String>,
    pub feedback: Vec<LiveFeedback>,
}

/// 首页的「我的关注」区块。
#[derive(Debug, Clone)]
pub struct LiveFollowSection {
    pub total_count: i64,
    pub items: Vec<LiveRoomCard>,
}

/// 首页推荐的一屏。
#[derive(Debug, Clone)]
pub struct LiveIndexPage {
    pub cards: Vec<LiveRoomCard>,
    pub has_more: bool,
    /// 未登录或没有关注时为 `None`。
    pub follow: Option<LiveFollowSection>,
    /// 服务端不下发时已回落到内置表，这里一定非空。
    pub area_entrance: Vec<LiveRoomCard>,
}

/// 分区二级列表的排序标签。
#[derive(Debug, Clone)]
pub struct LiveSecondTag {
    pub name: Option<String>,
    pub sort_type: Option<String>,
}

/// 分区二级列表的一屏。
#[derive(Debug, Clone)]
pub struct LiveSecondPage {
    pub count: Option<i64>,
    pub cards: Vec<LiveRoomCard>,
    pub tags: Vec<LiveSecondTag>,
}

/// 关注列表里的一条。
#[derive(Debug, Clone)]
pub struct LiveFollowItem {
    pub roomid: Option<i64>,
    pub uname: Option<String>,
    pub title: Option<String>,
    pub area_name: Option<String>,
    pub text_small: Option<String>,
    pub room_cover: Option<String>,
}

/// 关注的直播整页。`items` 只含正在直播的。
#[derive(Debug, Clone)]
pub struct LiveFollowingPage {
    pub title: Option<String>,
    pub page_size: Option<i64>,
    pub total_page: Option<i64>,
    pub items: Vec<LiveFollowItem>,
    pub count: Option<i64>,
    pub live_count: Option<i64>,
}

/// H5 房间信息。基线的三层嵌套已摊平。
#[derive(Debug, Clone)]
pub struct LiveRoomH5 {
    pub uid: Option<i64>,
    pub title: Option<String>,
    pub cover: Option<String>,
    pub app_background: Option<String>,
    pub anchor_name: Option<String>,
    pub anchor_face: Option<String>,
    pub watched_text: Option<String>,
}

/// 一档画质。
///
/// `desc` 为 `None` 表示本次响应的 `g_qn_desc` 里没有这一档，**不是**
/// 没有名字：Dart 侧用内置静态表补（`live_playurl_compat.dart` 的
/// `toLiveQuality`）。那张表同时被设置页与 `Pref.liveQuality` 使用，
/// 与取流无关，所以留在 Dart——ADR-011 决策六的单向适配。
#[derive(Debug, Clone)]
pub struct LiveQn {
    pub code: i64,
    pub desc: Option<String>,
}

/// 一个 CDN 节点。拼流地址时 `host + base_url + extra` 三段相连。
#[derive(Debug, Clone)]
pub struct LiveUrlInfo {
    pub host: String,
    pub extra: String,
}

/// 一种编码（avc / hevc / av1）下的全部可选地址。
#[derive(Debug, Clone)]
pub struct LiveCodec {
    pub codec_name: Option<String>,
    /// 服务端实际给的那一档，**以它为准**而非请求里的 `qn`。
    pub current_qn: LiveQn,
    pub accept_qn: Vec<LiveQn>,
    pub base_url: String,
    pub url_info: Vec<LiveUrlInfo>,
}

/// 一种封装格式（flv / ts / fmp4）。
#[derive(Debug, Clone)]
pub struct LiveFormat {
    pub format_name: Option<String>,
    pub codec: Vec<LiveCodec>,
}

/// 一种传输协议（http_stream / http_hls）。
#[derive(Debug, Clone)]
pub struct LiveStream {
    pub protocol_name: Option<String>,
    pub format: Vec<LiveFormat>,
}

/// 取流响应。
///
/// 协议上的 `playurl_info.playurl` 两层空壳已经摊平：`streams` 为空即
/// 「没有可播的地址」，调用方只需判一次空。理由见
/// [`bili_domain::live::LiveRoomPlayInfo`]。
#[derive(Debug, Clone)]
pub struct LiveRoomPlayInfo {
    pub room_id: Option<i64>,
    pub short_id: Option<i64>,
    pub uid: Option<i64>,
    pub is_portrait: Option<bool>,
    /// 1 为直播中。其余值都不可取流。
    pub live_status: Option<i64>,
    pub live_time: Option<i64>,
    pub streams: Vec<LiveStream>,
}

// ------------------------------------------------------------------ 结果

/// 全部分区。失败时 `data` 为 `None`，错误以值的形式带回。
#[derive(Debug, Clone)]
pub struct LiveAreaGroupsResult {
    pub data: Option<Vec<LiveAreaGroup>>,
    /// `network|remote|protocol|auth|storage|player|cancelled`。
    pub error_kind: Option<String>,
    /// 可直接展示的文案，不含凭据、不含完整 URL。
    pub error_detail: Option<String>,
    /// 服务端业务码。
    pub error_code: Option<i64>,
}

/// 一组分区（二级分区、收藏分区）。字段语义同 [`LiveAreaGroupsResult`]。
#[derive(Debug, Clone)]
pub struct LiveAreasResult {
    pub data: Option<Vec<LiveArea>>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

/// 首页推荐。字段语义同 [`LiveAreaGroupsResult`]。
#[derive(Debug, Clone)]
pub struct LiveIndexResult {
    pub data: Option<LiveIndexPage>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

/// 分区二级列表。字段语义同 [`LiveAreaGroupsResult`]。
#[derive(Debug, Clone)]
pub struct LiveSecondResult {
    pub data: Option<LiveSecondPage>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

/// 关注的直播。字段语义同 [`LiveAreaGroupsResult`]。
#[derive(Debug, Clone)]
pub struct LiveFollowResult {
    pub data: Option<LiveFollowingPage>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

/// H5 房间信息。字段语义同 [`LiveAreaGroupsResult`]。
#[derive(Debug, Clone)]
pub struct LiveRoomH5Result {
    pub data: Option<LiveRoomH5>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

/// 取流。字段语义同 [`LiveAreaGroupsResult`]。
#[derive(Debug, Clone)]
pub struct LiveRoomPlayInfoResult {
    pub data: Option<LiveRoomPlayInfo>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

// ------------------------------------------------------------------ 转换

fn area(value: bili_domain::live::LiveArea) -> LiveArea {
    LiveArea {
        id: value.id,
        name: value.name,
        pic: value.pic,
        parent_id: value.parent_id,
        parent_name: value.parent_name,
    }
}

fn area_group(value: bili_domain::live::LiveAreaGroup) -> LiveAreaGroup {
    LiveAreaGroup {
        name: value.name,
        areas: value.areas.into_iter().map(area).collect(),
    }
}

fn feedback(value: bili_domain::live::LiveFeedback) -> LiveFeedback {
    LiveFeedback {
        title: value.title,
        subtitle: value.subtitle,
        kind: value.kind,
        reasons: value
            .reasons
            .into_iter()
            .map(|r| LiveFeedbackReason {
                id: r.id,
                name: r.name,
                id_type: r.id_type,
                reason_id: r.reason_id,
            })
            .collect(),
    }
}

fn card(value: bili_domain::live::LiveRoomCard) -> LiveRoomCard {
    LiveRoomCard {
        roomid: value.roomid,
        uid: value.uid,
        uname: value.uname,
        face: value.face,
        cover: value.cover,
        system_cover: value.system_cover,
        title: value.title,
        area_name: value.area_name,
        area_v2_id: value.area_v2_id,
        area_v2_parent_id: value.area_v2_parent_id,
        watched_text: value.watched_text,
        feedback: value.feedback.into_iter().map(feedback).collect(),
    }
}

fn index_page(value: bili_domain::live::LiveIndexPage) -> LiveIndexPage {
    LiveIndexPage {
        cards: value.cards.into_iter().map(card).collect(),
        has_more: value.has_more,
        follow: value.follow.map(|f| LiveFollowSection {
            total_count: f.total_count,
            items: f.items.into_iter().map(card).collect(),
        }),
        area_entrance: value.area_entrance.into_iter().map(card).collect(),
    }
}

fn second_page(value: bili_domain::live::LiveSecondPage) -> LiveSecondPage {
    LiveSecondPage {
        count: value.count,
        cards: value.cards.into_iter().map(card).collect(),
        tags: value
            .tags
            .into_iter()
            .map(|t| LiveSecondTag {
                name: t.name,
                sort_type: t.sort_type,
            })
            .collect(),
    }
}

fn follow_page(value: bili_domain::live::LiveFollowingPage) -> LiveFollowingPage {
    LiveFollowingPage {
        title: value.title,
        page_size: value.page_size,
        total_page: value.total_page,
        items: value
            .items
            .into_iter()
            .map(|i| LiveFollowItem {
                roomid: i.roomid,
                uname: i.uname,
                title: i.title,
                area_name: i.area_name,
                text_small: i.text_small,
                room_cover: i.room_cover,
            })
            .collect(),
        count: value.count,
        live_count: value.live_count,
    }
}

fn room_h5(value: bili_domain::live::LiveRoomH5) -> LiveRoomH5 {
    LiveRoomH5 {
        uid: value.uid,
        title: value.title,
        cover: value.cover,
        app_background: value.app_background,
        anchor_name: value.anchor_name,
        anchor_face: value.anchor_face,
        watched_text: value.watched_text,
    }
}

fn quality(value: bili_domain::live::LiveQn) -> LiveQn {
    LiveQn {
        code: value.code,
        desc: value.desc,
    }
}

fn room_play_info(value: bili_domain::live::LiveRoomPlayInfo) -> LiveRoomPlayInfo {
    LiveRoomPlayInfo {
        room_id: value.room_id,
        short_id: value.short_id,
        uid: value.uid,
        is_portrait: value.is_portrait,
        live_status: value.live_status,
        live_time: value.live_time,
        streams: value
            .streams
            .into_iter()
            .map(|s| LiveStream {
                protocol_name: s.protocol_name,
                format: s
                    .format
                    .into_iter()
                    .map(|f| LiveFormat {
                        format_name: f.format_name,
                        codec: f
                            .codec
                            .into_iter()
                            .map(|c| LiveCodec {
                                codec_name: c.codec_name,
                                current_qn: quality(c.current_qn),
                                accept_qn: c.accept_qn.into_iter().map(quality).collect(),
                                base_url: c.base_url,
                                url_info: c
                                    .url_info
                                    .into_iter()
                                    .map(|u| LiveUrlInfo {
                                        host: u.host,
                                        extra: u.extra,
                                    })
                                    .collect(),
                            })
                            .collect(),
                    })
                    .collect(),
            })
            .collect(),
    }
}

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

// ------------------------------------------------------------------ 取数

/// 取全部分区（按一级分区分组）。
pub async fn fetch_live_area_list(
    context: LiveContext,
    identity: LiveAppIdentity,
) -> LiveAreaGroupsResult {
    let id = identity.into_api();
    let (ctx, transport) = context.split();

    let joined = init::runtime()
        .spawn(async move { live::fetch_area_list(&transport, &ctx, &id).await })
        .await;

    match flatten(joined) {
        Ok(groups) => LiveAreaGroupsResult {
            data: Some(groups.into_iter().map(area_group).collect()),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => LiveAreaGroupsResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 取某一级分区下的二级分区。
pub async fn fetch_live_room_area_list(
    context: LiveContext,
    identity: LiveAppIdentity,
    parent_id: i64,
) -> LiveAreasResult {
    let id = identity.into_api();
    let (ctx, transport) = context.split();

    let joined = init::runtime()
        .spawn(async move { live::fetch_room_area_list(&transport, &ctx, &id, parent_id).await })
        .await;

    match flatten(joined) {
        Ok(areas) => LiveAreasResult {
            data: Some(areas.into_iter().map(area).collect()),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => LiveAreasResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 取收藏的分区。
pub async fn fetch_live_fav_tags(
    context: LiveContext,
    identity: LiveAppIdentity,
) -> LiveAreasResult {
    let id = identity.into_api();
    let (ctx, transport) = context.split();

    let joined = init::runtime()
        .spawn(async move { live::fetch_fav_tags(&transport, &ctx, &id).await })
        .await;

    match flatten(joined) {
        Ok(areas) => LiveAreasResult {
            data: Some(areas.into_iter().map(area).collect()),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => LiveAreasResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 取首页推荐的一屏。
///
/// `module_select` 只在刷新顶部区块时为真，正常翻页传 `false`。
pub async fn fetch_live_index(
    context: LiveContext,
    identity: LiveAppIdentity,
    page: i64,
    module_select: bool,
) -> LiveIndexResult {
    let id = identity.into_api();
    let (ctx, transport) = context.split();

    let joined = init::runtime()
        .spawn(async move { live::fetch_index(&transport, &ctx, &id, page, module_select).await })
        .await;

    match flatten(joined) {
        Ok(page) => LiveIndexResult {
            data: Some(index_page(page)),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => LiveIndexResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 取分区二级列表的一屏。
pub async fn fetch_live_second_list(
    context: LiveContext,
    identity: LiveAppIdentity,
    page: i64,
    area_id: Option<i64>,
    parent_area_id: Option<i64>,
    sort_type: Option<String>,
) -> LiveSecondResult {
    let id = identity.into_api();
    let (ctx, transport) = context.split();

    let joined = init::runtime()
        .spawn(async move {
            live::fetch_second_list(
                &transport,
                &ctx,
                &id,
                page,
                area_id,
                parent_area_id,
                sort_type.as_deref(),
            )
            .await
        })
        .await;

    match flatten(joined) {
        Ok(page) => LiveSecondResult {
            data: Some(second_page(page)),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => LiveSecondResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 取关注的直播。不签名，因此不需要 [`LiveAppIdentity`]。
pub async fn fetch_live_follow(context: LiveContext, page: i64) -> LiveFollowResult {
    let (ctx, transport) = context.split();

    let joined = init::runtime()
        .spawn(async move { live::fetch_follow(&transport, &ctx, page).await })
        .await;

    match flatten(joined) {
        Ok(page) => LiveFollowResult {
            data: Some(follow_page(page)),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => LiveFollowResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 取 H5 房间信息。不签名。
///
/// `room_id` 非正数时不发请求，直接返回 `protocol` 错误——基线把它原样
/// 拼进查询串，换回一个业务错误码，从调用链上很难回溯到「id 没取到」。
pub async fn fetch_live_room_h5(context: LiveContext, room_id: i64) -> LiveRoomH5Result {
    if let Err(e) = live::check_room_id(room_id) {
        return LiveRoomH5Result {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        };
    }

    let (ctx, transport) = context.split();

    let joined = init::runtime()
        .spawn(async move { live::fetch_room_h5(&transport, &ctx, room_id).await })
        .await;

    match flatten(joined) {
        Ok(room) => LiveRoomH5Result {
            data: Some(room_h5(room)),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => LiveRoomH5Result {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 取流。**WBI 签名**。
///
/// `mixin_key` 与 `wts` 由 Dart 注入：前者的真相源是 nav 接口加本地
/// 缓存（`WbiSign.getWbiKeys`，还有多条未迁接口在用），后者注入才能让
/// 请求构造保持纯函数、被 golden fixture 锁死。
///
/// 这两个参数**没有放进 [`LiveContext`]**：那个结构是另外七条接口共用
/// 的，它们都不签 WBI，为一条接口给它加两个恒为空的字段，等于让每个
/// 调用点都要填一遍无关的值。pgc 那边放进 context 是因为多条接口都要
/// 签。
///
/// `qn` 为 `None` 时由服务端选档；`only_audio` 为真时只要音频流。
/// `room_id` 非正数时不发请求，理由同 [`fetch_live_room_h5`]。
pub async fn fetch_live_room_play_info(
    context: LiveContext,
    room_id: i64,
    qn: Option<i64>,
    only_audio: bool,
    mixin_key: String,
    wts: i64,
) -> LiveRoomPlayInfoResult {
    if let Err(e) = live::check_room_id(room_id) {
        return LiveRoomPlayInfoResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        };
    }

    let (ctx, transport) = context.split();

    let joined = init::runtime()
        .spawn(async move {
            live::fetch_room_play_info(&transport, &ctx, room_id, qn, only_audio, &mixin_key, wts)
                .await
        })
        .await;

    match flatten(joined) {
        Ok(info) => LiveRoomPlayInfoResult {
            data: Some(room_play_info(info)),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => LiveRoomPlayInfoResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}
