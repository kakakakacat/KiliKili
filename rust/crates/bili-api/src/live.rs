//! 直播只读接口：请求构造、响应解析与到领域实体的映射。
//!
//! 覆盖 S11a 第一批的七条只读接口：全部分区、某一级分区下的二级分区、
//! 收藏分区、首页推荐、分区二级列表、关注的直播、H5 房间信息；
//! 以及第二批的取流（`liveRoomInfo`）。
//!
//! **不在本模块**：弹幕长连接（S11b）、以及 `lib/http/live.dart` 里的
//! 全部写操作。边界与理由见 [`bili_domain::live`]。
//!
//! ## 三类接口，三套签名
//!
//! | 接口 | 主机 | 签名 |
//! |---|---|---|
//! | 分区三条、首页推荐、分区二级列表 | `api.live.bilibili.com` | APP 签名（`appkey`/`ts`/`sign`） |
//! | 关注的直播、H5 房间信息 | 同上 | 不签名 |
//! | 取流 | 同上 | **WBI**（`wts`/`w_rid`） |
//!
//! APP 签名的算法在 [`bili_protocol::app_sign`]，WBI 在
//! [`bili_protocol::wbi`]，两者不是一回事，前者那里有对照表。取流是本
//! 模块唯一走 WBI 的接口——它虽挂在直播主机上，用的却是 web 端那套参数
//! （`platform=web`、`web_location`），签名也跟着是 web 端的。
//!
//! ## 身份值从调用方传入，结构由本模块决定
//!
//! `appkey`/`appsec`/`statistics`/`User-Agent`/`buvid`/`access_key` 的
//! 真相源在 Dart（`lib/common/constants.dart` 与账号体系），且还有
//! `lib/http/login.dart`、`member.dart`、`fav.dart` 等未迁接口在用，
//! 复制一份到 Rust 就会出现两个真相源——与 [`crate::http::RequestContext`]
//! 不写死 `base_url` 是同一个理由。
//!
//! 反过来，**哪些参数要带、哪条接口要带 app 请求头**是协议形状，属于
//! 本切片，跟着接口一起迁过来。调用方不该有机会漏掉其中一项。

use bili_domain::error::{DomainError, ErrorKind};
use bili_domain::live::{
    LiveArea, LiveAreaGroup, LiveCodec, LiveFeedback, LiveFeedbackReason, LiveFollowItem,
    LiveFollowSection, LiveFollowingPage, LiveFormat, LiveIndexPage, LiveQn, LiveRoomCard,
    LiveRoomH5, LiveRoomPlayInfo, LiveSecondPage, LiveSecondTag, LiveStream, LiveUrlInfo,
    default_area_entrance,
};
use serde::Deserialize;

use crate::envelope::unwrap_envelope;
use crate::http::{HttpGet, HttpTransport, RequestContext, check_status};
use crate::request::PreparedRequest;

/// 接口路径。与基线 `lib/http/api.dart` 保持一致。
///
/// 都挂在 `HttpString.liveBaseUrl`（`api.live.bilibili.com`）下，不是
/// 其余接口用的 `api.bilibili.com`——base_url 由调用方给出，别在这里
/// 按习惯补成后者。
pub mod path {
    /// 全部分区（按一级分区分组）。
    pub const AREA_LIST: &str = "/xlive/app-interface/v2/index/getAreaList";
    /// 某一级分区下的二级分区。
    pub const ROOM_AREA_LIST: &str = "/room/v1/Area/getList";
    /// 收藏的分区。
    pub const FAV_TAG: &str = "/xlive/app-interface/v2/second/get_fav_tag";
    /// 首页推荐。
    pub const FEED_INDEX: &str = "/xlive/app-interface/v2/index/feed";
    /// 分区二级列表。
    pub const SECOND_LIST: &str = "/xlive/app-interface/v2/second/getList";
    /// 关注的直播。
    pub const FOLLOW: &str = "/xlive/web-ucenter/user/following";
    /// H5 房间信息。
    pub const ROOM_H5: &str = "/xlive/web-room/v1/index/getH5InfoByRoom";
    /// 取流。
    pub const ROOM_PLAY_INFO: &str = "/xlive/web-room/v2/index/getRoomPlayInfo";
}

// ------------------------------------------------------------ 请求构造

/// 本机/本次会话的身份值，由调用方提供。
///
/// 这些值**不是协议常量**，见本模块文档。`ts` 由调用方注入（Unix 秒）
/// 使请求构造保持纯函数，可被 golden fixture 锁死（ADR-008 决策五）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppIdentity {
    pub appkey: String,
    pub appsec: String,
    /// 基线写在各调用点上的 `build`（当前 8430300）。
    pub build: i64,
    /// 与 `build` 同属一套的版本号（当前 "8.43.0"）。
    pub version: String,
    /// 基线 `Constants.statisticsApp`，一段 JSON。
    pub statistics: String,
    /// 基线 `Constants.userAgentApp`。只用于需要 app 请求头的那两条。
    pub user_agent: String,
    pub buvid: String,
    /// 基线 `Constants.traceId`。
    pub trace_id: String,
    /// 账号的 `access_key`。未登录时为 `None`，此时整个参数不出现——
    /// 基线用的是 `'access_key': ?account.accessKey`（null 则不写入），
    /// 带一个空串上去会被当成无效凭据而不是匿名。
    pub access_key: Option<String>,
    /// 该账号是否已登录。只影响首页推荐的 `relation_page`。
    ///
    /// 不用 `access_key.is_some()` 代替：两者在基线里是不同的判断，
    /// 合并会让「有 key 但未登录」这种中间态悄悄改变请求形状。
    pub logged_in: bool,
    pub ts: i64,
}

fn put(query: &mut Vec<(String, String)>, key: &str, value: impl Into<String>) {
    query.push((key.to_owned(), value.into()));
}

/// 全部 app 接口共有的那组参数。
///
/// 整组一起才有意义：少一个，服务端会按它自己的默认值处理（比如
/// `disable_rcmd` 缺失时按开启个性化推荐算）。
fn app_base(id: &AppIdentity) -> Vec<(String, String)> {
    let mut query = Vec::with_capacity(16);
    if let Some(key) = &id.access_key {
        put(&mut query, "access_key", key.clone());
    }
    put(&mut query, "actionKey", "appkey");
    put(&mut query, "build", id.build.to_string());
    put(&mut query, "channel", "master");
    put(&mut query, "version", id.version.clone());
    put(&mut query, "c_locale", "zh_CN");
    put(&mut query, "device", "android");
    put(&mut query, "disable_rcmd", "0");
    put(&mut query, "mobi_app", "android");
    put(&mut query, "platform", "android");
    put(&mut query, "s_locale", "zh_CN");
    put(&mut query, "statistics", id.statistics.clone());
    query
}

/// 两条 feed 类接口额外带的一组。
fn feed_extras(query: &mut Vec<(String, String)>) {
    put(query, "device_name", "android");
    put(query, "device_type", "0");
    // 912 是一组位标志的并（基线原样写死），不是魔数。
    put(query, "fnval", "912");
    put(query, "https_url_req", "1");
    put(query, "network", "wifi");
    put(query, "scale", "2");
}

fn signed(query: Vec<(String, String)>, id: &AppIdentity, path: &'static str) -> PreparedRequest {
    PreparedRequest {
        path,
        query: bili_protocol::app_sign::sign(&query, &id.appkey, &id.appsec, id.ts),
    }
}

/// 全部分区。
#[must_use]
pub fn build_area_list_request(id: &AppIdentity) -> PreparedRequest {
    signed(app_base(id), id, path::AREA_LIST)
}

/// 收藏的分区。参数与 [`build_area_list_request`] 相同，差别只在
/// 调用方该用哪个账号（基线用 `Accounts.main`，分区列表用 `recommend`）。
#[must_use]
pub fn build_fav_tag_request(id: &AppIdentity) -> PreparedRequest {
    signed(app_base(id), id, path::FAV_TAG)
}

/// 某一级分区下的二级分区。
#[must_use]
pub fn build_room_area_list_request(id: &AppIdentity, parent_id: i64) -> PreparedRequest {
    let mut query = app_base(id);
    put(&mut query, "need_entrance", "1");
    put(&mut query, "parent_id", parent_id.to_string());
    put(&mut query, "source_id", "2");
    signed(query, id, path::ROOM_AREA_LIST)
}

/// 首页推荐。
///
/// `module_select` 只在刷新顶部区块时为真（基线 `queryTop`）：它让
/// 服务端连同关注区块与分区入口一起下发，正常翻页不需要。
#[must_use]
pub fn build_feed_index_request(
    id: &AppIdentity,
    page: i64,
    module_select: bool,
) -> PreparedRequest {
    let mut query = app_base(id);
    feed_extras(&mut query);
    if module_select {
        put(&mut query, "module_select", "1");
    }
    put(&mut query, "page", page.to_string());
    if id.logged_in {
        put(&mut query, "relation_page", "1");
    }
    signed(query, id, path::FEED_INDEX)
}

/// 分区二级列表。
///
/// `area_id` / `parent_area_id` 为 `None` 时整个参数不出现，与基线的
/// `'area_id': ?areaId` 一致。
#[must_use]
pub fn build_second_list_request(
    id: &AppIdentity,
    page: i64,
    area_id: Option<i64>,
    parent_area_id: Option<i64>,
    sort_type: Option<&str>,
) -> PreparedRequest {
    let mut query = app_base(id);
    feed_extras(&mut query);
    if let Some(area_id) = area_id {
        put(&mut query, "area_id", area_id.to_string());
    }
    if let Some(parent_area_id) = parent_area_id {
        put(&mut query, "parent_area_id", parent_area_id.to_string());
    }
    // 与首页推荐相反，这里固定为 0：顶部区块由首页那条接口负责。
    put(&mut query, "module_select", "0");
    put(&mut query, "page", page.to_string());
    put(&mut query, "page_size", "20");
    put(&mut query, "qn", "0");
    if let Some(sort_type) = sort_type {
        put(&mut query, "sort_type", sort_type.to_owned());
    }
    put(&mut query, "tag_version", "1");
    signed(query, id, path::SECOND_LIST)
}

/// 关注的直播。不签名。
#[must_use]
pub fn build_follow_request(page: i64) -> PreparedRequest {
    PreparedRequest {
        path: path::FOLLOW,
        query: vec![
            ("page".to_owned(), page.to_string()),
            ("page_size".to_owned(), "9".to_owned()),
            ("ignoreRecord".to_owned(), "1".to_owned()),
            // 基线传的是 Dart 的 `true`，序列化成字面量 "true"，不是 1。
            ("hit_ab".to_owned(), "true".to_owned()),
        ],
    }
}

/// H5 房间信息。不签名。
#[must_use]
pub fn build_room_h5_request(room_id: i64) -> PreparedRequest {
    PreparedRequest {
        path: path::ROOM_H5,
        query: vec![("room_id".to_owned(), room_id.to_string())],
    }
}

/// 取流请求。本模块唯一走 WBI 签名的一条。
///
/// 参数逐字复制自基线 `LiveHttp.liveRoomInfo`，包括几个看不出含义的
/// 常量——它们是服务端的能力声明，不是可调项：
///
/// - `protocol=0,1`：允许 http_stream 与 http_hls 两种传输；
/// - `format=0,1,2` / `codec=0,1,2`：允许 flv/ts/fmp4 与 avc/hevc/av1。
///   三者共同决定响应里 `stream → format → codec` 这棵树有多少枝；
/// - `ptype=8` / `dolby=5` / `panorama=1`：声明本端支持的播放器类型、
///   杜比与全景档位。少报一项，响应里对应的档位就不出现；
/// - `web_location=444.8`：**基线写的是浮点数**，`toString()` 后是
///   `"444.8"`。签名覆盖参数值，写成 `"444"` 或 `"444.80"` 都会让
///   `w_rid` 对不上而被服务端拒绝，且错误形式是业务码而非解析失败。
///
/// `qn` 为 `None` 时整个键不出现（基线的 `'qn': ?qn`），由服务端自行
/// 选档。`only_audio` 为假时同理不出现——基线用的是 `if (onlyAudio)`，
/// 不是传 0。
///
/// `wts` 由调用方注入（Unix 秒），使本函数保持纯函数可被 fixture 锁死。
/// 参数按 key 升序排列后再签名，`w_rid` 追加在末尾——与
/// [`PreparedRequest::query`] 的契约和其余 WBI 接口一致。基线那边顺序
/// 来自 Map 字面量，但签名两边都是排序后算的，`w_rid` 不受影响。
#[must_use]
pub fn build_room_play_info_request(
    room_id: i64,
    qn: Option<i64>,
    only_audio: bool,
    mixin_key: &str,
    wts: i64,
) -> PreparedRequest {
    let mut query = vec![
        ("room_id".to_owned(), room_id.to_string()),
        ("protocol".to_owned(), "0,1".to_owned()),
        ("format".to_owned(), "0,1,2".to_owned()),
        ("codec".to_owned(), "0,1,2".to_owned()),
        ("platform".to_owned(), "web".to_owned()),
        ("ptype".to_owned(), "8".to_owned()),
        ("dolby".to_owned(), "5".to_owned()),
        ("panorama".to_owned(), "1".to_owned()),
        ("web_location".to_owned(), "444.8".to_owned()),
        ("wts".to_owned(), wts.to_string()),
    ];
    if let Some(qn) = qn {
        query.push(("qn".to_owned(), qn.to_string()));
    }
    if only_audio {
        query.push(("only_audio".to_owned(), "1".to_owned()));
    }
    query.sort_by(|a, b| a.0.cmp(&b.0));

    let w_rid = bili_protocol::wbi::sign(&query, mixin_key);
    query.push(("w_rid".to_owned(), w_rid));

    PreparedRequest {
        path: path::ROOM_PLAY_INFO,
        query,
    }
}

/// 两条 feed 接口额外需要的请求头。
///
/// 基线只给首页推荐与分区二级列表加这一组（`lib/http/live.dart` 的
/// `Options.headers`），分区三条不加。值里有三个占位串
/// （`fp_local`/`fp_remote`/`session_id`），基线写死成全 1，照搬——
/// 它们是设备指纹字段，填真值等于主动多交一份可追踪信息。
#[must_use]
pub fn feed_headers(id: &AppIdentity) -> Vec<(String, String)> {
    const FINGERPRINT: &str = "1111111111111111111111111111111111111111111111111111111111111111";
    vec![
        ("buvid".to_owned(), id.buvid.clone()),
        ("fp_local".to_owned(), FINGERPRINT.to_owned()),
        ("fp_remote".to_owned(), FINGERPRINT.to_owned()),
        ("session_id".to_owned(), "11111111".to_owned()),
        ("env".to_owned(), "prod".to_owned()),
        ("app-key".to_owned(), "android".to_owned()),
        ("User-Agent".to_owned(), id.user_agent.clone()),
        ("x-bili-trace-id".to_owned(), id.trace_id.clone()),
        ("x-bili-aurora-eid".to_owned(), String::new()),
        ("x-bili-aurora-zone".to_owned(), String::new()),
        ("bili-http-engine".to_owned(), "cronet".to_owned()),
    ]
}

// ------------------------------------------------------------------ DTO

/// 分区 id 在不同接口里 JSON 类型不一致。
///
/// 基线把 `AreaItem.id` / `parentId` 声明成 `dynamic` 并直接透传，于是
/// `lib/pages/live_area_detail/view.dart` 里出现了
/// `LiveAreaDetailController(widget.areaId?.toString(), ...)`——一边转成
/// 字符串，另一边 `e.id == areaId` 拿原值比。两边类型只要不同，这个
/// 比较恒为假，表现是「从分区页进来，选中的 Tab 总是第一个」，不报错。
///
/// 统一成 `i64` 正是为了让这种比较重新成立。解析不出数字时按缺失处理，
/// 而不是塞 0——0 在这个接口里是「人气」这个真实分区。
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(untagged)]
enum NumberOrString {
    Number(i64),
    Text(String),
    #[default]
    Missing,
}

impl NumberOrString {
    fn to_i64(&self) -> Option<i64> {
        match self {
            Self::Number(n) => Some(*n),
            Self::Text(s) => s.parse().ok(),
            Self::Missing => None,
        }
    }
}

#[derive(Deserialize)]
struct AreaListDto {
    #[serde(default)]
    list: Vec<AreaGroupDto>,
}

#[derive(Deserialize)]
struct AreaGroupDto {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    area_list: Vec<AreaItemDto>,
}

#[derive(Deserialize)]
struct AreaItemDto {
    #[serde(default)]
    id: NumberOrString,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    pic: Option<String>,
    #[serde(default)]
    parent_id: NumberOrString,
    #[serde(default)]
    parent_name: Option<String>,
}

#[derive(Deserialize)]
struct FavTagDto {
    #[serde(default)]
    tags: Vec<AreaItemDto>,
}

#[derive(Deserialize)]
struct WatchedShowDto {
    #[serde(default)]
    text_large: Option<String>,
}

#[derive(Deserialize)]
struct RoomCardDto {
    #[serde(default)]
    roomid: Option<i64>,
    /// 分区二级列表里这张卡片的房间 id 键名是 `id`，首页推荐里是
    /// `roomid`。基线 `json['roomid'] ?? json['id']` 同样两个都认。
    #[serde(default)]
    id: Option<i64>,
    #[serde(default)]
    uid: Option<i64>,
    #[serde(default)]
    uname: Option<String>,
    #[serde(default)]
    face: Option<String>,
    #[serde(default)]
    cover: Option<String>,
    #[serde(default)]
    system_cover: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    area_name: Option<String>,
    #[serde(default)]
    area_v2_id: Option<i64>,
    #[serde(default)]
    area_v2_parent_id: Option<i64>,
    #[serde(default)]
    watched_show: Option<WatchedShowDto>,
    #[serde(default)]
    feedback: Vec<FeedbackDto>,
}

#[derive(Deserialize)]
struct FeedbackDto {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    subtitle: Option<String>,
    #[serde(default, rename = "type")]
    kind: Option<String>,
    #[serde(default)]
    reasons: Vec<FeedbackReasonDto>,
}

#[derive(Deserialize)]
struct FeedbackReasonDto {
    #[serde(default)]
    id: Option<i64>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    id_type: Option<String>,
    #[serde(default)]
    reason_id: Option<i64>,
}

#[derive(Deserialize)]
struct IndexDto {
    #[serde(default)]
    card_list: Vec<IndexCardDto>,
    #[serde(default)]
    has_more: Option<i64>,
}

#[derive(Deserialize)]
struct IndexCardDto {
    #[serde(default)]
    card_type: Option<String>,
    #[serde(default)]
    card_data: Option<IndexCardDataDto>,
}

#[derive(Deserialize)]
struct IndexCardDataDto {
    #[serde(default)]
    my_idol_v1: Option<CardSectionDto>,
    #[serde(default)]
    area_entrance_v3: Option<CardSectionDto>,
    #[serde(default)]
    small_card_v1: Option<RoomCardDto>,
}

#[derive(Deserialize)]
struct CardSectionDto {
    #[serde(default)]
    list: Vec<RoomCardDto>,
    #[serde(default)]
    extra_info: Option<ExtraInfoDto>,
}

#[derive(Deserialize)]
struct ExtraInfoDto {
    #[serde(default)]
    total_count: Option<i64>,
}

#[derive(Deserialize)]
struct SecondListDto {
    #[serde(default)]
    count: Option<i64>,
    #[serde(default)]
    list: Vec<RoomCardDto>,
    #[serde(default)]
    new_tags: Vec<SecondTagDto>,
}

#[derive(Deserialize)]
struct SecondTagDto {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    sort_type: Option<String>,
}

#[derive(Deserialize)]
struct FollowDto {
    #[serde(default)]
    title: Option<String>,
    #[serde(default, rename = "pageSize")]
    page_size: Option<i64>,
    #[serde(default, rename = "totalPage")]
    total_page: Option<i64>,
    #[serde(default)]
    list: Vec<FollowItemDto>,
    #[serde(default)]
    count: Option<i64>,
    #[serde(default)]
    live_count: Option<i64>,
}

#[derive(Deserialize)]
struct FollowItemDto {
    #[serde(default)]
    roomid: Option<i64>,
    #[serde(default)]
    uname: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    area_name: Option<String>,
    #[serde(default)]
    text_small: Option<String>,
    #[serde(default)]
    room_cover: Option<String>,
    #[serde(default)]
    live_status: Option<i64>,
}

#[derive(Deserialize)]
struct RoomH5Dto {
    #[serde(default)]
    room_info: Option<RoomH5RoomDto>,
    #[serde(default)]
    anchor_info: Option<RoomH5AnchorDto>,
    #[serde(default)]
    watched_show: Option<WatchedShowDto>,
}

#[derive(Deserialize)]
struct RoomH5RoomDto {
    #[serde(default)]
    uid: Option<i64>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    cover: Option<String>,
    #[serde(default)]
    app_background: Option<String>,
}

#[derive(Deserialize)]
struct RoomH5AnchorDto {
    #[serde(default)]
    base_info: Option<RoomH5BaseInfoDto>,
}

#[derive(Deserialize)]
struct RoomH5BaseInfoDto {
    #[serde(default)]
    uname: Option<String>,
    #[serde(default)]
    face: Option<String>,
}

// ------------------------------------------------------------------ 映射

/// 空串按缺失处理。基线 `nonNullOrEmptyString`。
fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|s| !s.is_empty())
}

impl AreaItemDto {
    fn into_domain(self) -> Option<LiveArea> {
        Some(LiveArea {
            id: self.id.to_i64()?,
            name: self.name,
            pic: self.pic,
            // 收藏分区那条接口不回 `parent_id`，基线此时是 `null`，
            // 而它只被用来做相等比较，取 0 会与「人气」撞上，故用 -1：
            // 真实分区 id 不会是负数。
            parent_id: self.parent_id.to_i64().unwrap_or(-1),
            parent_name: self.parent_name,
        })
    }
}

impl RoomCardDto {
    fn into_domain(self) -> LiveRoomCard {
        let cover = self.cover;
        let system_cover = non_empty(self.system_cover).or_else(|| cover.clone());
        LiveRoomCard {
            roomid: self.roomid.or(self.id),
            uid: self.uid,
            uname: self.uname,
            face: self.face,
            cover,
            system_cover,
            title: self.title,
            area_name: self.area_name,
            area_v2_id: self.area_v2_id,
            area_v2_parent_id: self.area_v2_parent_id,
            watched_text: self.watched_show.and_then(|w| w.text_large),
            feedback: self
                .feedback
                .into_iter()
                .map(FeedbackDto::into_domain)
                .collect(),
        }
    }
}

impl FeedbackDto {
    fn into_domain(self) -> LiveFeedback {
        LiveFeedback {
            title: self.title,
            subtitle: self.subtitle,
            kind: self.kind,
            reasons: self
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
}

// ------------------------------------------------------------------ 解析

/// 全部分区。
///
/// # Errors
/// 信封解包失败或结构不符。
pub fn parse_area_list(body: &str) -> Result<Vec<LiveAreaGroup>, DomainError> {
    let dto: AreaListDto = unwrap_envelope(body)?;
    Ok(dto
        .list
        .into_iter()
        .map(|group| LiveAreaGroup {
            name: group.name.unwrap_or_default(),
            areas: group
                .area_list
                .into_iter()
                .filter_map(AreaItemDto::into_domain)
                .collect(),
        })
        .collect())
}

/// 某一级分区下的二级分区。载荷直接就是数组。
///
/// # Errors
/// 同上。
pub fn parse_room_area_list(body: &str) -> Result<Vec<LiveArea>, DomainError> {
    let dto: Vec<AreaItemDto> = unwrap_envelope(body)?;
    Ok(dto
        .into_iter()
        .filter_map(AreaItemDto::into_domain)
        .collect())
}

/// 收藏的分区。
///
/// # Errors
/// 同上。
pub fn parse_fav_tags(body: &str) -> Result<Vec<LiveArea>, DomainError> {
    let dto: FavTagDto = unwrap_envelope(body)?;
    Ok(dto
        .tags
        .into_iter()
        .filter_map(AreaItemDto::into_domain)
        .collect())
}

/// 首页推荐。
///
/// `card_list` 里混着四种卡片，按 `card_type` 分流；基线只认其中三种
/// （`banner_v2` 被注释掉了），未知类型一律忽略——首页随时会多出新卡
/// 片，为此报错会让整页打不开。
///
/// # Errors
/// 同上。
pub fn parse_index(body: &str) -> Result<LiveIndexPage, DomainError> {
    let dto: IndexDto = unwrap_envelope(body)?;

    let mut cards = Vec::new();
    let mut follow = None;
    let mut area_entrance = None;

    for card in dto.card_list {
        let Some(data) = card.card_data else { continue };
        match card.card_type.as_deref() {
            Some("my_idol_v1") => {
                if let Some(section) = data.my_idol_v1 {
                    follow = Some(LiveFollowSection {
                        total_count: section
                            .extra_info
                            .and_then(|e| e.total_count)
                            .unwrap_or_default(),
                        items: section
                            .list
                            .into_iter()
                            .map(RoomCardDto::into_domain)
                            .collect(),
                    });
                }
            }
            Some("area_entrance_v3") => {
                if let Some(section) = data.area_entrance_v3 {
                    area_entrance = Some(
                        section
                            .list
                            .into_iter()
                            .map(RoomCardDto::into_domain)
                            .collect::<Vec<_>>(),
                    );
                }
            }
            Some("small_card_v1") => {
                if let Some(card) = data.small_card_v1 {
                    cards.push(card.into_domain());
                }
            }
            _ => {}
        }
    }

    Ok(LiveIndexPage {
        cards,
        has_more: dto.has_more.unwrap_or_default() != 0,
        follow,
        area_entrance: area_entrance
            .filter(|list| !list.is_empty())
            .unwrap_or_else(default_area_entrance),
    })
}

/// 分区二级列表。
///
/// # Errors
/// 同上。
pub fn parse_second_list(body: &str) -> Result<LiveSecondPage, DomainError> {
    let dto: SecondListDto = unwrap_envelope(body)?;
    Ok(LiveSecondPage {
        count: dto.count,
        cards: dto.list.into_iter().map(RoomCardDto::into_domain).collect(),
        tags: dto
            .new_tags
            .into_iter()
            .map(|t| LiveSecondTag {
                name: t.name,
                sort_type: t.sort_type,
            })
            .collect(),
    })
}

/// 关注的直播。只保留 `live_status == 1` 的，见
/// [`bili_domain::live::LiveFollowingPage::items`]。
///
/// # Errors
/// 同上。
pub fn parse_follow(body: &str) -> Result<LiveFollowingPage, DomainError> {
    let dto: FollowDto = unwrap_envelope(body)?;
    Ok(LiveFollowingPage {
        title: dto.title,
        page_size: dto.page_size,
        total_page: dto.total_page,
        items: dto
            .list
            .into_iter()
            .filter(|item| item.live_status == Some(1))
            .map(|item| LiveFollowItem {
                roomid: item.roomid,
                uname: item.uname,
                title: item.title,
                area_name: item.area_name,
                text_small: item.text_small,
                room_cover: item.room_cover,
            })
            .collect(),
        count: dto.count,
        live_count: dto.live_count,
    })
}

/// H5 房间信息。
///
/// # Errors
/// 同上。
pub fn parse_room_h5(body: &str) -> Result<LiveRoomH5, DomainError> {
    let dto: RoomH5Dto = unwrap_envelope(body)?;
    let room = dto.room_info;
    let base = dto.anchor_info.and_then(|a| a.base_info);
    Ok(LiveRoomH5 {
        uid: room.as_ref().and_then(|r| r.uid),
        title: room.as_ref().and_then(|r| r.title.clone()),
        cover: room.as_ref().and_then(|r| r.cover.clone()),
        app_background: room.and_then(|r| r.app_background),
        anchor_name: base.as_ref().and_then(|b| b.uname.clone()),
        anchor_face: base.and_then(|b| b.face),
        watched_text: dto.watched_show.and_then(|w| w.text_large),
    })
}

// --- 取流 ---
//
// 可空性逐字对着基线 `lib/models_new/live/live_room_play_info/*.dart`：
// 房间那几个字段在基线里是 `as int?`，而 `base_url` / `host` / `extra` /
// `current_qn` / `accept_qn` 以及三层列表都是不带 `?` 的强制转换——缺了
// 就抛，被 `liveRoomInfo` 的 `catch` 收成 `Error`。这里用必填字段复刻：
// serde 缺字段即 `Err`，同样走错误路径，而不是悄悄填个默认值。

#[derive(Deserialize)]
struct RoomPlayInfoDto {
    room_id: Option<i64>,
    short_id: Option<i64>,
    uid: Option<i64>,
    is_portrait: Option<bool>,
    live_status: Option<i64>,
    live_time: Option<i64>,
    playurl_info: Option<PlayurlInfoDto>,
}

#[derive(Deserialize)]
struct PlayurlInfoDto {
    playurl: Option<PlayurlDto>,
}

#[derive(Deserialize)]
struct PlayurlDto {
    /// 本次响应自带的码率描述表。基线是 `as List?`，可缺。
    g_qn_desc: Option<Vec<GQnDescDto>>,
    stream: Vec<StreamDto>,
}

#[derive(Deserialize)]
struct GQnDescDto {
    qn: Option<i64>,
    desc: Option<String>,
}

#[derive(Deserialize)]
struct StreamDto {
    protocol_name: Option<String>,
    format: Vec<FormatDto>,
}

#[derive(Deserialize)]
struct FormatDto {
    format_name: Option<String>,
    codec: Vec<CodecDto>,
}

#[derive(Deserialize)]
struct CodecDto {
    codec_name: Option<String>,
    current_qn: i64,
    accept_qn: Vec<i64>,
    base_url: String,
    url_info: Vec<UrlInfoDto>,
}

#[derive(Deserialize)]
struct UrlInfoDto {
    host: String,
    extra: String,
}

/// 把 `g_qn_desc` 一项转成领域的一档画质。
///
/// 复刻基线 `LiveQuality.fromJson`：`qn` 缺失时 code 取 `-1`，`desc`
/// 缺失时退到 `qn` 的十进制串。`qn` 与 `desc` 同时缺失时基线算的是
/// Dart 的 `null.toString()`，结果是字面量 `"null"`——照搬，因为这张表
/// 会被下面按 code 查，而 `-1` 这一档存在与否要和基线一致。
fn qn_from_desc_entry(dto: GQnDescDto) -> LiveQn {
    let code = dto.qn.unwrap_or(-1);
    let desc = dto.desc.unwrap_or_else(|| match dto.qn {
        Some(qn) => qn.to_string(),
        None => "null".to_owned(),
    });
    LiveQn {
        code,
        desc: Some(desc),
    }
}

/// 按 code 在 `g_qn_desc` 表里查一档画质。
///
/// 复刻基线 `LiveQuality.fromFormat` 的前半段：表非空且命中就用表里那
/// 一项。未命中时基线回退到客户端静态表，那张表没迁（见
/// [`bili_domain::live::LiveQn`]），所以这里产出 `desc: None` 交给
/// Dart 侧补——这是两边的分工点，不是信息丢失。
fn resolve_qn(code: i64, table: &[LiveQn]) -> LiveQn {
    table
        .iter()
        .find(|qn| qn.code == code)
        .cloned()
        .unwrap_or(LiveQn { code, desc: None })
}

/// 解析取流响应。
///
/// # Errors
/// 信封非 0、JSON 结构不符（含上面那几个必填字段缺失）。
pub fn parse_room_play_info(body: &str) -> Result<LiveRoomPlayInfo, DomainError> {
    let dto: RoomPlayInfoDto = unwrap_envelope(body)?;

    let playurl = dto.playurl_info.and_then(|info| info.playurl);
    let (table, stream_dtos) = match playurl {
        Some(p) => (
            p.g_qn_desc
                .unwrap_or_default()
                .into_iter()
                .map(qn_from_desc_entry)
                .collect::<Vec<_>>(),
            p.stream,
        ),
        None => (Vec::new(), Vec::new()),
    };

    let streams = stream_dtos
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
                            current_qn: resolve_qn(c.current_qn, &table),
                            accept_qn: c
                                .accept_qn
                                .into_iter()
                                .map(|code| resolve_qn(code, &table))
                                .collect(),
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
        .collect();

    Ok(LiveRoomPlayInfo {
        room_id: dto.room_id,
        short_id: dto.short_id,
        uid: dto.uid,
        is_portrait: dto.is_portrait,
        live_status: dto.live_status,
        live_time: dto.live_time,
        streams,
    })
}

// ------------------------------------------------------------------ 取数

async fn send<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    prepared: &PreparedRequest,
    extra_headers: Vec<(String, String)>,
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

    tracing::debug!(path = prepared.path, "发送直播请求");

    let mut headers = ctx.all_headers();
    // 追加而非替换：ctx 里那几条（cookie、referer、账号头）仍要带上。
    // 同名键以后者为准由传输层保证，这正是基线 Options.headers 的语义。
    headers.extend(extra_headers);

    let resp = transport
        .get(HttpGet {
            url,
            headers,
            timeout: ctx.timeout,
        })
        .await?;

    check_status(resp.status)?;
    Ok(resp.body)
}

/// 取全部分区。
///
/// # Errors
/// 传输、状态码、信封或解析失败。
pub async fn fetch_area_list<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    id: &AppIdentity,
) -> Result<Vec<LiveAreaGroup>, DomainError> {
    let body = send(transport, ctx, &build_area_list_request(id), Vec::new()).await?;
    parse_area_list(&body)
}

/// 取收藏的分区。
///
/// # Errors
/// 同上。
pub async fn fetch_fav_tags<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    id: &AppIdentity,
) -> Result<Vec<LiveArea>, DomainError> {
    let body = send(transport, ctx, &build_fav_tag_request(id), Vec::new()).await?;
    parse_fav_tags(&body)
}

/// 取某一级分区下的二级分区。
///
/// # Errors
/// 同上。
pub async fn fetch_room_area_list<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    id: &AppIdentity,
    parent_id: i64,
) -> Result<Vec<LiveArea>, DomainError> {
    let request = build_room_area_list_request(id, parent_id);
    let body = send(transport, ctx, &request, Vec::new()).await?;
    parse_room_area_list(&body)
}

/// 取首页推荐的一屏。
///
/// # Errors
/// 同上。
pub async fn fetch_index<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    id: &AppIdentity,
    page: i64,
    module_select: bool,
) -> Result<LiveIndexPage, DomainError> {
    let request = build_feed_index_request(id, page, module_select);
    let body = send(transport, ctx, &request, feed_headers(id)).await?;
    parse_index(&body)
}

/// 取分区二级列表的一屏。
///
/// # Errors
/// 同上。
pub async fn fetch_second_list<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    id: &AppIdentity,
    page: i64,
    area_id: Option<i64>,
    parent_area_id: Option<i64>,
    sort_type: Option<&str>,
) -> Result<LiveSecondPage, DomainError> {
    let request = build_second_list_request(id, page, area_id, parent_area_id, sort_type);
    let body = send(transport, ctx, &request, feed_headers(id)).await?;
    parse_second_list(&body)
}

/// 取关注的直播。
///
/// # Errors
/// 同上。
pub async fn fetch_follow<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    page: i64,
) -> Result<LiveFollowingPage, DomainError> {
    let body = send(transport, ctx, &build_follow_request(page), Vec::new()).await?;
    parse_follow(&body)
}

/// 取 H5 房间信息。
///
/// # Errors
/// 同上。
pub async fn fetch_room_h5<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    room_id: i64,
) -> Result<LiveRoomH5, DomainError> {
    let body = send(transport, ctx, &build_room_h5_request(room_id), Vec::new()).await?;
    parse_room_h5(&body)
}

/// 取流。
///
/// `qn` 是**期望**档位，不是保证：服务端会按房间实际可用的流降档，
/// 真正给到的那一档在 [`LiveCodec::current_qn`]。
///
/// # Errors
/// 传输失败、信封非 0、JSON 结构不符。
pub async fn fetch_room_play_info<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    room_id: i64,
    qn: Option<i64>,
    only_audio: bool,
    mixin_key: &str,
    wts: i64,
) -> Result<LiveRoomPlayInfo, DomainError> {
    let body = send(
        transport,
        ctx,
        &build_room_play_info_request(room_id, qn, only_audio, mixin_key, wts),
        Vec::new(),
    )
    .await?;
    parse_room_play_info(&body)
}

/// 房间 id 必须是正数。
///
/// 基线把 `roomId` 声明成 `Object` 并原样拼进查询串，`0` 或负数会换回
/// 一个业务错误码，从调用链上很难回溯到「id 没取到」。
///
/// # Errors
/// `room_id <= 0`。
pub fn check_room_id(room_id: i64) -> Result<(), DomainError> {
    if room_id > 0 {
        Ok(())
    } else {
        Err(DomainError::new(
            ErrorKind::Protocol,
            format!("房间 id 无效: {room_id}"),
        ))
    }
}

#[cfg(test)]
// 测试里 expect 是正确做法：前提不成立时就该崩掉并指出原因。
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    fn identity() -> AppIdentity {
        AppIdentity {
            appkey: "dfca71928277209b".to_owned(),
            appsec: "b5475a8825547a4fc26c7d518eaaa02e".to_owned(),
            build: 8_430_300,
            version: "8.43.0".to_owned(),
            statistics: r#"{"appId":1,"platform":3,"version":"8.43.0","abtest":""}"#.to_owned(),
            user_agent: "Mozilla/5.0 BiliDroid/8.43.0".to_owned(),
            buvid: "XY0000000000000000000000000000000".to_owned(),
            trace_id: "1111111111111111111111111111111:1111111111111111:0:0".to_owned(),
            access_key: None,
            logged_in: false,
            ts: 1_700_000_000,
        }
    }

    fn keys(req: &PreparedRequest) -> Vec<&str> {
        req.query.iter().map(|(k, _)| k.as_str()).collect()
    }

    fn value<'a>(req: &'a PreparedRequest, key: &str) -> Option<&'a str> {
        req.query
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    #[test]
    fn anonymous_requests_omit_access_key() {
        let req = build_area_list_request(&identity());
        assert!(
            !keys(&req).contains(&"access_key"),
            "未登录时不得带空的 access_key——空串会被当成无效凭据"
        );
    }

    #[test]
    fn logged_in_requests_carry_access_key() {
        let mut id = identity();
        id.access_key = Some("token".to_owned());
        let req = build_area_list_request(&id);
        assert_eq!(value(&req, "access_key"), Some("token"));
    }

    #[test]
    fn relation_page_tracks_login_not_access_key() {
        let mut id = identity();
        id.access_key = Some("token".to_owned());
        assert!(
            !keys(&build_feed_index_request(&id, 1, false)).contains(&"relation_page"),
            "有 access_key 但未登录时不该带 relation_page"
        );

        id.logged_in = true;
        assert_eq!(
            value(&build_feed_index_request(&id, 1, true), "relation_page"),
            Some("1")
        );
    }

    #[test]
    fn module_select_is_absent_unless_requested() {
        let id = identity();
        assert!(!keys(&build_feed_index_request(&id, 1, false)).contains(&"module_select"));
        assert_eq!(
            value(&build_feed_index_request(&id, 1, true), "module_select"),
            Some("1")
        );
        // 分区二级列表则固定为 0，不是"省略"。
        assert_eq!(
            value(
                &build_second_list_request(&id, 1, None, None, None),
                "module_select"
            ),
            Some("0")
        );
    }

    #[test]
    fn optional_area_filters_are_omitted_when_absent() {
        let id = identity();
        let req = build_second_list_request(&id, 1, None, None, None);
        let k = keys(&req);
        assert!(!k.contains(&"area_id"));
        assert!(!k.contains(&"parent_area_id"));
        assert!(!k.contains(&"sort_type"));

        let req = build_second_list_request(&id, 2, Some(145), Some(1), Some("online"));
        assert_eq!(value(&req, "area_id"), Some("145"));
        assert_eq!(value(&req, "parent_area_id"), Some("1"));
        assert_eq!(value(&req, "sort_type"), Some("online"));
        assert_eq!(value(&req, "page"), Some("2"));
    }

    #[test]
    fn signed_requests_end_with_sign() {
        let req = build_area_list_request(&identity());
        assert_eq!(keys(&req).last(), Some(&"sign"));
    }

    #[test]
    fn unsigned_requests_carry_no_app_credentials() {
        let req = build_follow_request(3);
        let k = keys(&req);
        assert!(!k.contains(&"appkey"), "关注列表走 web 接口，不加 APP 签名");
        assert!(!k.contains(&"sign"));
        // Dart 的 true 序列化成字面量，不是 1。
        assert_eq!(value(&req, "hit_ab"), Some("true"));
        assert_eq!(value(&req, "page_size"), Some("9"));
    }

    #[test]
    fn feed_headers_are_only_for_feed_endpoints() {
        let headers = feed_headers(&identity());
        let names: Vec<&str> = headers.iter().map(|(k, _)| k.as_str()).collect();
        assert!(names.contains(&"buvid"));
        assert!(names.contains(&"x-bili-trace-id"));
        // 指纹字段必须是占位值，不是真实设备信息。
        let fp = headers
            .iter()
            .find(|(k, _)| k == "fp_local")
            .map(|(_, v)| v.clone())
            .expect("有 fp_local");
        assert!(fp.chars().all(|c| c == '1'));
    }

    #[test]
    fn area_id_accepts_both_json_shapes() {
        let as_number = r#"{"code":0,"message":"0","data":[{"id":145,"parent_id":1}]}"#;
        let as_string = r#"{"code":0,"message":"0","data":[{"id":"145","parent_id":"1"}]}"#;
        let a = parse_room_area_list(as_number).expect("数字型 id 应解析成功");
        let b = parse_room_area_list(as_string).expect("字符串型 id 应解析成功");
        assert_eq!(a, b, "两种 JSON 类型必须归一，否则相等比较恒为假");
        assert_eq!(a[0].id, 145);
    }

    #[test]
    fn unparsable_area_id_drops_the_entry() {
        let body = r#"{"code":0,"message":"0","data":[{"id":"abc"},{"id":7}]}"#;
        let areas = parse_room_area_list(body).expect("应解析成功");
        assert_eq!(areas.len(), 1, "id 解析不出来的条目丢弃，不得塞 0");
        assert_eq!(areas[0].id, 7);
    }

    #[test]
    fn system_cover_falls_back_to_cover() {
        let body = r#"{"code":0,"message":"0","data":{"list":[
            {"id":1,"cover":"c1","system_cover":""},
            {"id":2,"cover":"c2","system_cover":"s2"},
            {"id":3,"cover":"c3"}
        ]}}"#;
        let page = parse_second_list(body).expect("应解析成功");
        assert_eq!(
            page.cards[0].system_cover.as_deref(),
            Some("c1"),
            "空串按缺失"
        );
        assert_eq!(page.cards[1].system_cover.as_deref(), Some("s2"));
        assert_eq!(page.cards[2].system_cover.as_deref(), Some("c3"));
    }

    #[test]
    fn second_list_card_id_is_read_from_either_key() {
        let body = r#"{"code":0,"message":"0","data":{"list":[{"id":11},{"roomid":22}]}}"#;
        let page = parse_second_list(body).expect("应解析成功");
        assert_eq!(page.cards[0].roomid, Some(11));
        assert_eq!(page.cards[1].roomid, Some(22));
    }

    #[test]
    fn follow_list_keeps_only_live_rooms() {
        let body = r#"{"code":0,"message":"0","data":{"list":[
            {"roomid":1,"live_status":1},
            {"roomid":2,"live_status":0},
            {"roomid":3}
        ]}}"#;
        let page = parse_follow(body).expect("应解析成功");
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].roomid, Some(1));
    }

    #[test]
    fn index_falls_back_to_the_builtin_area_entrance() {
        let body = r#"{"code":0,"message":"0","data":{"card_list":[],"has_more":1}}"#;
        let page = parse_index(body).expect("应解析成功");
        assert_eq!(
            page.area_entrance.len(),
            default_area_entrance().len(),
            "服务端不给分区入口时用内置表兜底"
        );
        assert!(page.has_more);
        assert!(page.follow.is_none());
    }

    #[test]
    fn index_ignores_unknown_card_types() {
        let body = r#"{"code":0,"message":"0","data":{"card_list":[
            {"card_type":"banner_v2","card_data":{"banner_v2":{"list":[]}}},
            {"card_type":"brand_new_card","card_data":{}},
            {"card_type":"small_card_v1","card_data":{"small_card_v1":{"roomid":9}}}
        ],"has_more":0}}"#;
        let page = parse_index(body).expect("首页多出新卡片不得让整页失败");
        assert_eq!(page.cards.len(), 1);
        assert_eq!(page.cards[0].roomid, Some(9));
        assert!(!page.has_more);
    }

    #[test]
    fn empty_area_entrance_section_also_falls_back() {
        // 服务端给了壳但列表是空的——基线的 `??=` 只判 null，会留下一个
        // 空列表，首页顶部于是只剩一个"推荐"。这里按缺失处理。
        let body = r#"{"code":0,"message":"0","data":{"card_list":[
            {"card_type":"area_entrance_v3","card_data":{"area_entrance_v3":{"list":[]}}}
        ]}}"#;
        let page = parse_index(body).expect("应解析成功");
        assert_eq!(page.area_entrance.len(), default_area_entrance().len());
    }

    #[test]
    fn room_h5_flattens_three_levels() {
        let body = r#"{"code":0,"message":"0","data":{
            "room_info":{"uid":7,"title":"标题"},
            "anchor_info":{"base_info":{"uname":"主播","face":"f"}},
            "watched_show":{"text_large":"1.2万人看过"}
        }}"#;
        let room = parse_room_h5(body).expect("应解析成功");
        assert_eq!(room.uid, Some(7));
        assert_eq!(room.anchor_name.as_deref(), Some("主播"));
        assert_eq!(room.watched_text.as_deref(), Some("1.2万人看过"));
        assert_eq!(room.app_background, None);
    }

    #[test]
    fn missing_anchor_is_not_an_error() {
        let body = r#"{"code":0,"message":"0","data":{"room_info":{"title":"标题"}}}"#;
        let room = parse_room_h5(body).expect("主播信息缺失不该让房间信息整体失败");
        assert_eq!(room.title.as_deref(), Some("标题"));
        assert_eq!(room.anchor_name, None);
    }

    #[test]
    fn remote_error_code_is_carried_through() {
        let body = r#"{"code":-101,"message":"账号未登录","data":null}"#;
        let err = parse_index(body).expect_err("业务错误码必须变成错误");
        assert_eq!(err.kind(), ErrorKind::Auth);
    }

    #[test]
    fn missing_list_is_an_empty_page_not_an_error() {
        let body = r#"{"code":0,"message":"0","data":{}}"#;
        assert!(
            parse_second_list(body)
                .expect("应解析成功")
                .cards
                .is_empty()
        );
        assert!(parse_area_list(body).expect("应解析成功").is_empty());
        assert!(parse_fav_tags(body).expect("应解析成功").is_empty());
    }

    #[test]
    fn invalid_room_id_is_rejected_before_sending() {
        assert!(check_room_id(0).is_err());
        assert!(check_room_id(-1).is_err());
        assert!(check_room_id(1).is_ok());
    }
}
