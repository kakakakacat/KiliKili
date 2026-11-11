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

/// 粉丝勋章。
///
/// **跨切片共享**：本模块的贡献榜与勋章墙产出它，而 SuperChat 与弹幕
/// 长连接里的每条消息也带同一结构（S11b / #62，阻塞在 ADR-012）。
/// Dart 的 `UinfoMedal` 因此删不掉，由 `live_rest_compat.dart` 单向
/// 适配，随 S11b 迁完一并删。
#[derive(Debug, Clone)]
pub struct LiveMedal {
    pub name: Option<String>,
    pub level: Option<i64>,
    pub id: Option<i64>,
    pub ruid: Option<i64>,
    pub color_start: Option<String>,
    pub color_text: Option<String>,
}

/// 直播间表情包里的一个表情。
#[derive(Debug, Clone)]
pub struct LiveEmoticon {
    pub emoji: Option<String>,
    pub url: Option<String>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub emoticon_unique: Option<String>,
}

/// 一个表情包。
#[derive(Debug, Clone)]
pub struct LiveEmotePackage {
    pub emoticons: Vec<LiveEmoticon>,
    pub pkg_type: Option<i64>,
    pub current_cover: Option<String>,
}

/// 直播搜索结果里的一个直播间。
#[derive(Debug, Clone)]
pub struct LiveSearchRoom {
    pub roomid: Option<i64>,
    pub cover: Option<String>,
    pub title: Option<String>,
    pub name: Option<String>,
    pub face: Option<String>,
    /// `watched_show.text_large`，壳已摊平。
    pub watched_text: Option<String>,
}

/// 直播搜索结果里的一个主播。
#[derive(Debug, Clone)]
pub struct LiveSearchUser {
    pub roomid: Option<i64>,
    pub name: Option<String>,
    pub face: Option<String>,
    pub live_status: Option<i64>,
    pub area_name: Option<String>,
    pub fans_num: Option<i64>,
}

/// 直播搜索的一页。同一次请求只会有 `rooms` 或 `users` 其中一侧非空。
#[derive(Debug, Clone)]
pub struct LiveSearchData {
    pub rooms: Vec<LiveSearchRoom>,
    pub total_room: Option<i64>,
    pub users: Vec<LiveSearchUser>,
    pub total_user: Option<i64>,
}

/// 被屏蔽的一个用户。
///
/// 增删屏蔽用户的写操作仍在 Dart（归 ADR-013 / #50），但它**直接构造
/// 本结构的 Dart 绑定**，读写共用一个类型。
#[derive(Debug, Clone)]
pub struct LiveShieldUser {
    pub uid: i64,
    pub uname: String,
}

/// 房间的屏蔽设置。
#[derive(Debug, Clone)]
pub struct LiveShieldInfo {
    pub shield_users: Vec<LiveShieldUser>,
    pub keywords: Vec<String>,
}

/// 贡献榜的查询条件。
///
/// 打包成结构体而不是五个平铺参数，是因为它们**两两同型且相邻**：
/// `ruid` 与 `room_id` 都是 i64，`kind` 与 `switch_name` 都是 String。
/// 平铺时传反了编译器一声不吭，服务端也只回一个空榜——表现成「这个榜
/// 没人」，从现象反推不出是参数顺序错了。Dart 侧用命名参数构造本结构，
/// 顺序就不再有意义。
#[derive(Debug, Clone)]
pub struct LiveRankQuery {
    /// 主播 uid。
    pub ruid: i64,
    pub room_id: i64,
    pub page: i64,
    /// `LiveContributionRankType.name`，如 `online_rank`。
    pub kind: String,
    /// 同一枚举的 `sw1tch`，如 `contribution_rank`。**必须与 [`kind`]
    /// 成对**，只带其一服务端回空榜而不是报错。
    ///
    /// [`kind`]: Self::kind
    pub switch_name: String,
}

/// 贡献榜的一项。
#[derive(Debug, Clone)]
pub struct LiveContributionRankItem {
    pub uid: Option<i64>,
    pub name: Option<String>,
    pub face: Option<String>,
    pub score: Option<i64>,
    pub medal: Option<LiveMedal>,
}

/// 勋章墙的一项。
#[derive(Debug, Clone)]
pub struct LiveMedalWallItem {
    /// `medal_info.wearing_status`，1 为正在佩戴。壳已摊平。
    pub wearing_status: Option<i64>,
    pub target_name: Option<String>,
    pub target_icon: Option<String>,
    pub link: Option<String>,
    pub live_status: Option<i64>,
    pub official: Option<i64>,
    pub medal: Option<LiveMedal>,
}

/// 某个用户的勋章墙。
#[derive(Debug, Clone)]
pub struct LiveMedalWall {
    pub list: Vec<LiveMedalWallItem>,
    pub count: Option<i64>,
    pub name: Option<String>,
    pub icon: Option<String>,
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

/// 房间表情包。字段语义同 [`LiveAreaGroupsResult`]。
#[derive(Debug, Clone)]
pub struct LiveEmotePackagesResult {
    pub data: Option<Vec<LiveEmotePackage>>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

/// 直播搜索。字段语义同 [`LiveAreaGroupsResult`]。
#[derive(Debug, Clone)]
pub struct LiveSearchResult {
    pub data: Option<LiveSearchData>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

/// 屏蔽设置。字段语义同 [`LiveAreaGroupsResult`]。
#[derive(Debug, Clone)]
pub struct LiveShieldInfoResult {
    pub data: Option<LiveShieldInfo>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

/// 贡献榜。字段语义同 [`LiveAreaGroupsResult`]。
#[derive(Debug, Clone)]
pub struct LiveContributionRankResult {
    pub data: Option<Vec<LiveContributionRankItem>>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

/// 勋章墙。字段语义同 [`LiveAreaGroupsResult`]。
#[derive(Debug, Clone)]
pub struct LiveMedalWallResult {
    pub data: Option<LiveMedalWall>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

/// 主播的粉丝团勋章名。
///
/// **这个结果没有 `data` 字段**，与上面几个不同：勋章名本身就是可空的
/// （主播没开粉丝团），而 `Option<Option<String>>` 是 FRB 不支持的类型
/// ——codegen 会直接 panic，不是生成一个将就能用的绑定。
///
/// 于是改用 `error_kind` 区分两种 `None`：它为 `None` 表示请求成功，
/// 此时 `name` 为 `None` 意味着该主播没有粉丝团（基线把这种情况也报成
/// 了错误提示）。
#[derive(Debug, Clone)]
pub struct LiveMedalNameResult {
    pub name: Option<String>,
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

fn medal(value: bili_domain::live::LiveMedal) -> LiveMedal {
    LiveMedal {
        name: value.name,
        level: value.level,
        id: value.id,
        ruid: value.ruid,
        color_start: value.color_start,
        color_text: value.color_text,
    }
}

fn emote_package(value: bili_domain::live::LiveEmotePackage) -> LiveEmotePackage {
    LiveEmotePackage {
        emoticons: value
            .emoticons
            .into_iter()
            .map(|e| LiveEmoticon {
                emoji: e.emoji,
                url: e.url,
                width: e.width,
                height: e.height,
                emoticon_unique: e.emoticon_unique,
            })
            .collect(),
        pkg_type: value.pkg_type,
        current_cover: value.current_cover,
    }
}

fn search_page(value: bili_domain::live::LiveSearchData) -> LiveSearchData {
    LiveSearchData {
        rooms: value
            .rooms
            .into_iter()
            .map(|r| LiveSearchRoom {
                roomid: r.roomid,
                cover: r.cover,
                title: r.title,
                name: r.name,
                face: r.face,
                watched_text: r.watched_text,
            })
            .collect(),
        total_room: value.total_room,
        users: value
            .users
            .into_iter()
            .map(|u| LiveSearchUser {
                roomid: u.roomid,
                name: u.name,
                face: u.face,
                live_status: u.live_status,
                area_name: u.area_name,
                fans_num: u.fans_num,
            })
            .collect(),
        total_user: value.total_user,
    }
}

fn shield_info(value: bili_domain::live::LiveShieldInfo) -> LiveShieldInfo {
    LiveShieldInfo {
        shield_users: value
            .shield_users
            .into_iter()
            .map(|u| LiveShieldUser {
                uid: u.uid,
                uname: u.uname,
            })
            .collect(),
        keywords: value.keywords,
    }
}

fn contribution_rank_item(
    value: bili_domain::live::LiveContributionRankItem,
) -> LiveContributionRankItem {
    LiveContributionRankItem {
        uid: value.uid,
        name: value.name,
        face: value.face,
        score: value.score,
        medal: value.medal.map(medal),
    }
}

fn medal_wall(value: bili_domain::live::LiveMedalWall) -> LiveMedalWall {
    LiveMedalWall {
        list: value
            .list
            .into_iter()
            .map(|i| LiveMedalWallItem {
                wearing_status: i.wearing_status,
                target_name: i.target_name,
                target_icon: i.target_icon,
                link: i.link,
                live_status: i.live_status,
                official: i.official,
                medal: i.medal.map(medal),
            })
            .collect(),
        count: value.count,
        name: value.name,
        icon: value.icon,
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

/// 取房间表情包。不签名。
pub async fn fetch_live_emoticons(context: LiveContext, room_id: i64) -> LiveEmotePackagesResult {
    if let Err(e) = live::check_room_id(room_id) {
        return LiveEmotePackagesResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        };
    }

    let (ctx, transport) = context.split();

    let joined = init::runtime()
        .spawn(async move { live::fetch_emoticons(&transport, &ctx, room_id).await })
        .await;

    match flatten(joined) {
        Ok(packages) => LiveEmotePackagesResult {
            data: Some(packages.into_iter().map(emote_package).collect()),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => LiveEmotePackagesResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 直播搜索。**APP 签名**。
///
/// `kind` 取 `room` 或 `user`，与 Dart 的 `LiveSearchType.name` 对应。
/// 不在这里定义枚举：它同时是 Dart 侧 Tab 的标识，两边各定义一份就会
/// 漂移。
pub async fn fetch_live_search(
    context: LiveContext,
    identity: LiveAppIdentity,
    page: i64,
    keyword: String,
    kind: String,
) -> LiveSearchResult {
    let (ctx, transport) = context.split();
    let id = identity.into_api();

    let joined = init::runtime()
        .spawn(
            async move { live::fetch_search(&transport, &ctx, &id, page, &keyword, &kind).await },
        )
        .await;

    match flatten(joined) {
        Ok(page) => LiveSearchResult {
            data: Some(search_page(page)),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => LiveSearchResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 取房间的屏蔽设置。**WBI 签名**。
pub async fn fetch_live_shield_info(
    context: LiveContext,
    room_id: i64,
    mixin_key: String,
    wts: i64,
) -> LiveShieldInfoResult {
    if let Err(e) = live::check_room_id(room_id) {
        return LiveShieldInfoResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        };
    }

    let (ctx, transport) = context.split();

    let joined =
        init::runtime()
            .spawn(async move {
                live::fetch_shield_info(&transport, &ctx, room_id, &mixin_key, wts).await
            })
            .await;

    match flatten(joined) {
        Ok(info) => LiveShieldInfoResult {
            data: Some(shield_info(info)),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => LiveShieldInfoResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 取贡献榜。**WBI 签名**。
///
/// `kind` 与 `switch_name` 成对出现（在线榜 `online_rank` /
/// `contribution_rank`、日榜 `daily_rank` / `today_rank`…），只带其一
/// 服务端会回一个空榜而不是报错。
pub async fn fetch_live_contribution_rank(
    context: LiveContext,
    query: LiveRankQuery,
    mixin_key: String,
    wts: i64,
) -> LiveContributionRankResult {
    let LiveRankQuery {
        ruid,
        room_id,
        page,
        kind,
        switch_name,
    } = query;

    if let Err(e) = live::check_room_id(room_id) {
        return LiveContributionRankResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        };
    }

    let (ctx, transport) = context.split();

    let joined = init::runtime()
        .spawn(async move {
            live::fetch_contribution_rank(
                &transport,
                &ctx,
                ruid,
                room_id,
                page,
                &kind,
                &switch_name,
                &mixin_key,
                wts,
            )
            .await
        })
        .await;

    match flatten(joined) {
        Ok(items) => LiveContributionRankResult {
            data: Some(items.into_iter().map(contribution_rank_item).collect()),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => LiveContributionRankResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 取某个用户的勋章墙。不签名。
pub async fn fetch_live_medal_wall(context: LiveContext, target_id: i64) -> LiveMedalWallResult {
    let (ctx, transport) = context.split();

    let joined = init::runtime()
        .spawn(async move { live::fetch_medal_wall(&transport, &ctx, target_id).await })
        .await;

    match flatten(joined) {
        Ok(wall) => LiveMedalWallResult {
            data: Some(medal_wall(wall)),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => LiveMedalWallResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 取主播的粉丝团勋章名。不签名。
///
/// 成功但没有粉丝团时 `data` 是 `Some(None)`，不是错误——基线把这种
/// 情况也报成了错误提示。
pub async fn fetch_live_medal_name(context: LiveContext, uid: i64) -> LiveMedalNameResult {
    let (ctx, transport) = context.split();

    let joined = init::runtime()
        .spawn(async move { live::fetch_medal_name(&transport, &ctx, uid).await })
        .await;

    match flatten(joined) {
        Ok(name) => LiveMedalNameResult {
            name,
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => LiveMedalNameResult {
            name: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}
