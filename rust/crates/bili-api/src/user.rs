//! 「我的」那一族的请求构造与解析（S7 / issue #57）。
//!
//! 对应基线 `lib/http/user.dart` 里围绕账号本身的十四条接口。
//!
//! ## 三处与别的模块不同的地方
//!
//! 1. **硬币数挂在另一个域名上**（`account.bilibili.com`）。这里不记死
//!    主机：`base_url` 本来就是按请求传进 [`RequestContext`] 的，调用方
//!    换一个值即可。把它写进路径常量会让「哪些接口不在 api 域」这件事散
//!    落在两处。
//! 2. **实名走 APP 签名**（`access_key` + `appkey`/`ts`/`sign`），不是
//!    WBI，也不带 csrf——APP 端接口用 access_key 鉴权，带 csrf 既无用又
//!    让请求面与基线对不上（见 [`crate::request::CsrfIn::None`]）。
//! 3. **硬币流水与经验流水是同一个形状**，只换路径。不拆两个解析函数，
//!    拆了只会得到两个字节相同的副本。

use bili_domain::error::DomainError;
use bili_domain::user::{
    CoinLogItem, LevelInfo, LoginLogItem, PrivacySetting, RejectPage, SubItem, SubList, UserInfo,
    UserRealName, UserStat, VideoTag,
};
use serde::Deserialize;

use crate::http::{HttpTransport, Idempotency, RequestContext};
use crate::request::{CsrfIn, PreparedRequest, PreparedWrite, WriteBody};

/// 接口路径。与基线 `lib/http/api.dart` 保持一致。
pub mod path {
    pub const USER_INFO: &str = "/x/web-interface/nav";
    pub const USER_STAT_OWNER: &str = "/x/web-interface/nav/stat";
    /// **挂在 `account.bilibili.com`**，不是 api 域。见模块文档。
    pub const GET_COIN: &str = "/site/getCoin";
    pub const COIN_LOG: &str = "/x/member/web/coin/log";
    pub const EXP_LOG: &str = "/x/member/web/exp/log";
    pub const LOGIN_LOG: &str = "/x/member/web/login/log";
    pub const USER_REAL_NAME: &str = "/x/member/app/up/realname";
    pub const SPACE_SETTING: &str = "/x/space/setting/app";
    pub const USER_SUB_FOLDER: &str = "/x/v3/fav/folder/collected/list";
    pub const VIDEO_TAGS: &str = "/x/web-interface/view/detail/tag";

    pub const VIP_EXP_ADD: &str = "/x/vip/experience/add";
    pub const SPACE_SETTING_MOD: &str = "/x/space/privacy/batch/modify";
    pub const DYNAMIC_REPORT: &str = "/x/dynamic/feed/dynamic_report/add";
    pub const SPACE_RESERVE: &str = "/x/space/reserve";
    pub const SPACE_RESERVE_CANCEL: &str = "/x/space/reserve/cancel";
}

/// 三份流水共用的固定查询参数。`jsonp=jsonp` 是基线写死的老参数，照搬。
fn log_query(web_location: &str) -> Vec<(String, String)> {
    vec![
        ("jsonp".to_owned(), "jsonp".to_owned()),
        ("web_location".to_owned(), web_location.to_owned()),
    ]
}

// ------------------------------------------------------------------ 登录用户

/// 登录用户信息的请求。无参数，凭据全在 Cookie 里。
#[must_use]
pub fn user_info_request() -> PreparedRequest {
    PreparedRequest {
        path: path::USER_INFO,
        query: Vec::new(),
    }
}

/// 三个计数的请求。
#[must_use]
pub fn user_stat_owner_request() -> PreparedRequest {
    PreparedRequest {
        path: path::USER_STAT_OWNER,
        query: Vec::new(),
    }
}

/// #166 的逐字段核对结论（nav / `/x/web-interface/nav`）：
///
/// - `mid` 是**标识符**，不放宽：它要拿去发后续请求，解不出来静默当没有
///   只会把一条打不通的链路伪装成一个空页面。基线也是 `int? mid`。
/// - `money` 是**金额**，不放宽：基线 `json['money'] is int ? .toDouble()`，
///   只在 int/double 之间换，没见过字符串形态。
/// - `vipStatus` / `vipType` / `is_senior_member` 是**判别用的状态值**，
///   界面拿它们选分支，不放宽。
/// - 等级那四个在 [`LevelInfoDto`] 里，全部放宽，理由见那里。
///
/// 基线 `UserInfoData.fromJson` 读 25 个字段，这里只映射九个；其余 17 个
/// 全 `lib/` 没有消费者（见 #57 的迁移说明）。「基线读过、Rust 没映射」
/// 不构成类型风险——没映射的字段 serde 根本不碰。
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct UserInfoDto {
    #[serde(rename = "isLogin")]
    is_login: Option<bool>,
    mid: Option<i64>,
    uname: Option<String>,
    face: Option<String>,
    money: Option<f64>,
    level_info: Option<LevelInfoDto>,
    #[serde(rename = "vipStatus")]
    vip_status: Option<i64>,
    #[serde(rename = "vipType")]
    vip_type: Option<i64>,
    is_senior_member: Option<i64>,
}

/// 等级四件套**整组放宽**，见 [`crate::lenient`]。
///
/// 它们全是展示用的数字（界面拿 `current_exp`/`next_exp` 当进度条的分子
/// 分母），解不出来最多少显示一条进度条。满级账号的 `next_exp` 服务端实际
/// 发的是字符串 `"--"`（#161 真机实测），而基线从来读不到这个形状——
/// `LevelInfo.fromJson` 在 `current_level == 6` 时改取 `current_exp`。
///
/// `current_level` 也放宽：它虽然参与「是不是满级」的判断，但那条分支选错
/// 的后果只是进度条分母取错，不会让页面打不开；而它和另外三个来自同一个
/// 对象、同一套服务端序列化，单独收紧没有依据。
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct LevelInfoDto {
    #[serde(deserialize_with = "crate::lenient::opt_i64")]
    current_level: Option<i64>,
    #[serde(deserialize_with = "crate::lenient::opt_i64")]
    current_min: Option<i64>,
    #[serde(deserialize_with = "crate::lenient::opt_i64")]
    current_exp: Option<i64>,
    #[serde(deserialize_with = "crate::lenient::opt_i64")]
    next_exp: Option<i64>,
}

/// 解析登录用户信息。
///
/// # Errors
/// 信封失败沿用统一分类。
///
/// 注意 `vipStatus` / `vipType` 在协议里是**小驼峰**，与同一个响应里的
/// `is_senior_member` 风格不一致——这是服务端的事实，不是笔误。
pub fn parse_user_info(body: &str) -> Result<UserInfo, DomainError> {
    let dto: UserInfoDto = crate::envelope::unwrap_envelope(body)?;
    Ok(UserInfo {
        is_login: dto.is_login,
        mid: dto.mid,
        uname: dto.uname,
        face: dto.face,
        money: dto.money,
        level_info: dto.level_info.map(|l| LevelInfo {
            current_level: l.current_level,
            current_min: l.current_min,
            current_exp: l.current_exp,
            // 满级时 next_exp 没有意义，界面拿它当进度条分母。见实体文档。
            next_exp: if l.current_level == Some(6) {
                l.current_exp
            } else {
                l.next_exp
            },
        }),
        vip_status: dto.vip_status,
        vip_type: dto.vip_type,
        is_senior_member: dto.is_senior_member,
    })
}

/// 三个计数全是展示用的，整组放宽（#166）。关注数在大 V 账号上曾见过
/// 字符串形态；三个字段同源，单独放宽一个没有依据。
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct UserStatDto {
    #[serde(deserialize_with = "crate::lenient::opt_i64")]
    following: Option<i64>,
    #[serde(deserialize_with = "crate::lenient::opt_i64")]
    follower: Option<i64>,
    #[serde(deserialize_with = "crate::lenient::opt_i64")]
    dynamic_count: Option<i64>,
}

/// 解析三个计数。
///
/// # Errors
/// 信封失败沿用统一分类。
pub fn parse_user_stat(body: &str) -> Result<UserStat, DomainError> {
    let dto: UserStatDto = crate::envelope::unwrap_envelope(body)?;
    Ok(UserStat {
        following: dto.following,
        follower: dto.follower,
        dynamic_count: dto.dynamic_count,
    })
}

/// 取登录用户信息。
///
/// # Errors
/// 传输、状态码、信封三类失败，见各自的文档。
pub async fn fetch_user_info<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
) -> Result<UserInfo, DomainError> {
    let body = crate::client::send(transport, ctx, &user_info_request()).await?;
    parse_user_info(&body)
}

/// 取三个计数。
///
/// # Errors
/// 同 [`fetch_user_info`]。
pub async fn fetch_user_stat<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
) -> Result<UserStat, DomainError> {
    let body = crate::client::send(transport, ctx, &user_stat_owner_request()).await?;
    parse_user_stat(&body)
}

// ------------------------------------------------------- 登录信息的本地缓存

/// 把登录用户信息序列化成一段 JSON，供宿主存盘。
///
/// 基线用 Hive 适配器把 `UserInfoData` 整个对象存进去；迁移之后导出结构是
/// 不可变的 typed 值，适配器无从生成，缓存改存 JSON（S7 #57）。序列化在
/// 这一侧，是为了让**存盘格式只有 Rust 知道**——两边各自拼键名的话，
/// 加一个字段就会出现一方写、另一方读不到的静默不一致。
///
/// 失败返回 `None`：缓存写不成不是错误，下次启动多发一次 nav 请求而已。
#[must_use]
pub fn serialize_user_info_cache(info: &UserInfo) -> Option<String> {
    serde_json::to_string(info).ok()
}

/// 把存盘的 JSON 解回登录用户信息。
///
/// 解不出来返回 `None`，调用方当作「没有缓存」。旧版本留下的是 Hive 的
/// 二进制帧而不是 JSON，走到这里同样是 `None`——那正是要的降级行为：
/// 首屏空一下，等 nav 回来。
#[must_use]
pub fn parse_user_info_cache(json: &str) -> Option<UserInfo> {
    serde_json::from_str(json).ok()
}

// -------------------------------------------------------------- 硬币与流水

/// 硬币数的请求。无参数。注意它挂在 `account.bilibili.com`。
#[must_use]
pub fn get_coin_request() -> PreparedRequest {
    PreparedRequest {
        path: path::GET_COIN,
        query: Vec::new(),
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct CoinDto {
    money: Option<f64>,
}

/// 解析硬币数。
///
/// # Errors
/// 信封失败沿用统一分类。`data` 在也可以没有 `money`，此时返回 `None`
/// ——基线取的就是 `data?['money']`，拿不到当作「没有这个数」而非失败。
pub fn parse_coin(body: &str) -> Result<Option<f64>, DomainError> {
    let dto: Option<CoinDto> = crate::envelope::unwrap_envelope_nullable(body)?;
    Ok(dto.and_then(|d| d.money))
}

/// 取硬币数。
///
/// # Errors
/// 同 [`fetch_user_info`]。
pub async fn fetch_coin<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
) -> Result<Option<f64>, DomainError> {
    let body = crate::client::send(transport, ctx, &get_coin_request()).await?;
    parse_coin(&body)
}

/// 硬币流水的请求。`web_location=333.33` 是基线写死的。
#[must_use]
pub fn coin_log_request() -> PreparedRequest {
    PreparedRequest {
        path: path::COIN_LOG,
        query: log_query("333.33"),
    }
}

/// 经验流水的请求。与 [`coin_log_request`] 只差路径。
#[must_use]
pub fn exp_log_request() -> PreparedRequest {
    PreparedRequest {
        path: path::EXP_LOG,
        query: log_query("333.33"),
    }
}

/// 登录日志的请求。
#[must_use]
pub fn login_log_request() -> PreparedRequest {
    PreparedRequest {
        path: path::LOGIN_LOG,
        query: log_query("333.33"),
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct CoinLogDto {
    list: Option<Vec<CoinLogItemDto>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct CoinLogItemDto {
    time: Option<String>,
    /// 服务端给的是数字，基线 `toString()` 之后直接显示。
    delta: Option<serde_json::Number>,
    reason: Option<String>,
}

/// 解析硬币/经验流水。两条路径共用。
///
/// # Errors
/// 信封失败沿用统一分类。`list` 缺失按空列表处理：没有流水不是错误。
///
/// `delta` 在协议里是数字，这里按基线的做法格式化成串后原样显示。用
/// [`serde_json::Number`] 而不是 `f64` 接：`f64` 会把 `2` 显示成 `2.0`，
/// 而基线显示的是 `2`。
pub fn parse_coin_log(body: &str) -> Result<Vec<CoinLogItem>, DomainError> {
    let dto: CoinLogDto = crate::envelope::unwrap_envelope(body)?;
    Ok(dto
        .list
        .unwrap_or_default()
        .into_iter()
        .map(|i| CoinLogItem {
            time: i.time.unwrap_or_default(),
            delta: i.delta.map(|d| d.to_string()).unwrap_or_default(),
            reason: i.reason.unwrap_or_default(),
        })
        .collect())
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct LoginLogDto {
    list: Option<Vec<LoginLogItemDto>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct LoginLogItemDto {
    ip: Option<String>,
    time_at: Option<String>,
    geo: Option<String>,
}

/// 解析登录日志。
///
/// # Errors
/// 信封失败沿用统一分类。三个字段缺失时落到空串，与基线的 `?? ''` 一致。
pub fn parse_login_log(body: &str) -> Result<Vec<LoginLogItem>, DomainError> {
    let dto: LoginLogDto = crate::envelope::unwrap_envelope(body)?;
    Ok(dto
        .list
        .unwrap_or_default()
        .into_iter()
        .map(|i| LoginLogItem {
            ip: i.ip.unwrap_or_default(),
            time_at: i.time_at.unwrap_or_default(),
            geo: i.geo.unwrap_or_default(),
        })
        .collect())
}

/// 取硬币流水。
///
/// # Errors
/// 同 [`fetch_user_info`]。
pub async fn fetch_coin_log<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
) -> Result<Vec<CoinLogItem>, DomainError> {
    let body = crate::client::send(transport, ctx, &coin_log_request()).await?;
    parse_coin_log(&body)
}

/// 取经验流水。
///
/// # Errors
/// 同 [`fetch_user_info`]。
pub async fn fetch_exp_log<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
) -> Result<Vec<CoinLogItem>, DomainError> {
    let body = crate::client::send(transport, ctx, &exp_log_request()).await?;
    parse_coin_log(&body)
}

/// 取登录日志。
///
/// # Errors
/// 同 [`fetch_user_info`]。
pub async fn fetch_login_log<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
) -> Result<Vec<LoginLogItem>, DomainError> {
    let body = crate::client::send(transport, ctx, &login_log_request()).await?;
    parse_login_log(&body)
}

// -------------------------------------------------------------------- 实名

/// 实名信息的请求。**APP 签名**，不带 csrf。
///
/// `access_key` 由 Dart 的账号体系传入；`appkey`/`ts`/`sign` 由
/// [`bili_protocol::app_sign`] 按协议补齐。appkey/appsec 同样由调用方给
/// ——它们是「这次冒充哪个客户端」的一部分，不该在这里写死一份副本
/// （与 `crate::search` / `crate::live` 的做法一致）。
#[must_use]
pub fn user_real_name_request(
    access_key: &str,
    up_mid: i64,
    appkey: &str,
    appsec: &str,
    ts: i64,
) -> PreparedRequest {
    let params = vec![
        ("access_key".to_owned(), access_key.to_owned()),
        ("up_mid".to_owned(), up_mid.to_string()),
    ];
    PreparedRequest {
        path: path::USER_REAL_NAME,
        query: bili_protocol::app_sign::sign(&params, appkey, appsec, ts),
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct UserRealNameDto {
    name: Option<String>,
    reject_page: Option<RejectPageDto>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RejectPageDto {
    title: Option<String>,
    text: Option<String>,
}

/// 解析实名信息。
///
/// # Errors
/// 信封失败沿用统一分类。
pub fn parse_user_real_name(body: &str) -> Result<UserRealName, DomainError> {
    let dto: UserRealNameDto = crate::envelope::unwrap_envelope(body)?;
    Ok(UserRealName {
        name: dto.name,
        reject_page: dto.reject_page.map(|p| RejectPage {
            title: p.title,
            text: p.text,
        }),
    })
}

/// 取实名信息。
///
/// # Errors
/// 同 [`fetch_user_info`]。
#[allow(clippy::too_many_arguments)]
pub async fn fetch_user_real_name<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    access_key: &str,
    up_mid: i64,
    appkey: &str,
    appsec: &str,
    ts: i64,
) -> Result<UserRealName, DomainError> {
    let req = user_real_name_request(access_key, up_mid, appkey, appsec, ts);
    let body = crate::client::send(transport, ctx, &req).await?;
    parse_user_real_name(&body)
}

// ---------------------------------------------------------------- 隐私设置

/// 空间隐私设置的请求。
#[must_use]
pub fn space_setting_request(mid: i64) -> PreparedRequest {
    PreparedRequest {
        path: path::SPACE_SETTING,
        query: vec![("mid".to_owned(), mid.to_string())],
    }
}

/// 十五个开关的清单：key、是否反相、属于界面上的第几组。
///
/// 顺序与分组照搬基线 `Privacy.fromJson` 的 `list1`/`list2`/`list3`。
/// 这张表是**协议语义**（反相的那几个读错就是把隐私设置显示反），所以它在
/// Rust；对应的中文标题是文案，留在界面里查表。
const PRIVACY_KEYS: &[(&str, bool, i64)] = &[
    ("fav_video", false, 1),
    ("bangumi", false, 1),
    ("comic", false, 1),
    ("coins_video", false, 1),
    ("likes_video", false, 1),
    ("played_game", false, 1),
    ("dress_up", false, 1),
    ("disable_following", true, 1),
    ("disable_show_fans", true, 1),
    ("close_space_medal", true, 2),
    ("only_show_wearing", true, 2),
    ("disable_show_school", true, 2),
    ("live_playback", false, 3),
    ("charge_video", false, 3),
    ("lesson_video", false, 3),
];

/// 解析空间隐私设置。
///
/// # Errors
/// 信封失败沿用统一分类。
///
/// 服务端没下发的开关 `value` 为 `None`，**不替它填 0 或 1**：填哪个都等于
/// 替用户做了一次隐私决定，界面应当显示成「未知/不可改」。
pub fn parse_space_setting(body: &str) -> Result<Vec<PrivacySetting>, DomainError> {
    let dto: serde_json::Value = crate::envelope::unwrap_envelope(body)?;
    let privacy = &dto["privacy"];
    Ok(PRIVACY_KEYS
        .iter()
        .map(|&(key, reversed, group)| PrivacySetting {
            key: key.to_owned(),
            value: privacy[key].as_i64(),
            reversed,
            group,
        })
        .collect())
}

/// 取空间隐私设置。
///
/// # Errors
/// 同 [`fetch_user_info`]。
pub async fn fetch_space_setting<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    mid: i64,
) -> Result<Vec<PrivacySetting>, DomainError> {
    let body = crate::client::send(transport, ctx, &space_setting_request(mid)).await?;
    parse_space_setting(&body)
}

// ---------------------------------------------------------------- 订阅/标签

/// 订阅的收藏夹/合集一页的请求。
#[must_use]
pub fn user_sub_folder_request(up_mid: i64, page: i64, page_size: i64) -> PreparedRequest {
    PreparedRequest {
        path: path::USER_SUB_FOLDER,
        query: vec![
            ("up_mid".to_owned(), up_mid.to_string()),
            ("ps".to_owned(), page_size.to_string()),
            ("pn".to_owned(), page.to_string()),
            ("platform".to_owned(), "web".to_owned()),
        ],
    }
}

/// `count` 是列表总数，展示用，放宽（#166）。
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SubListDto {
    #[serde(deserialize_with = "crate::lenient::opt_i64")]
    count: Option<i64>,
    has_more: Option<bool>,
    list: Option<Vec<SubItemDto>>,
}

/// #166 的逐字段核对结论（订阅的收藏夹/合集）：
///
/// - **不放宽**：`id` / `fid` / `mid` 是标识符（点进去要拿它们请求详情）；
///   `attr` / `cover_type` / `state` / `fav_state` / `type` 是判别用的
///   状态与枚举（`type` 决定这一项显示成「收藏夹」还是「合集」，也决定点
///   进去走哪条路由）。
/// - **放宽**：`media_count` / `view_count` 两个计数，以及 `cnt_info` 里的
///   两个，见 [`CntInfoDto`]。
/// - `ctime` / `mtime` 两个时间戳**不放宽**：虽然当前只拿去格式化显示，
///   但基线 `as int?` 也没放宽，没有实测依据就不单方面改口径。
///
/// 基线 `SubItemModel.fromJson` 读的字段与这里一一对应（全是 `as int?`），
/// 没有「基线没读过」的字段。
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SubItemDto {
    id: Option<i64>,
    fid: Option<i64>,
    mid: Option<i64>,
    attr: Option<i64>,
    title: Option<String>,
    cover: Option<String>,
    upper: Option<UpperDto>,
    cover_type: Option<i64>,
    intro: Option<String>,
    ctime: Option<i64>,
    mtime: Option<i64>,
    state: Option<i64>,
    fav_state: Option<i64>,
    #[serde(deserialize_with = "crate::lenient::opt_i64")]
    media_count: Option<i64>,
    #[serde(deserialize_with = "crate::lenient::opt_i64")]
    view_count: Option<i64>,
    #[serde(rename = "type")]
    kind: Option<i64>,
    cnt_info: Option<CntInfoDto>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct UpperDto {
    /// 标识符，**不放宽**（#166）：头像点进去要拿它跳空间页。
    mid: Option<i64>,
    name: Option<String>,
    face: Option<String>,
}

/// 两个纯展示计数，放宽（#166）。收藏夹那一族的同名字段在搜索接口上
/// 已经实测见过字符串形态（见 [`crate::search`] 对 `safeToInt` 的注释）。
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct CntInfoDto {
    #[serde(deserialize_with = "crate::lenient::opt_i64")]
    play: Option<i64>,
    #[serde(deserialize_with = "crate::lenient::opt_i64")]
    danmaku: Option<i64>,
}

/// 解析订阅列表。
///
/// # Errors
/// 信封失败沿用统一分类。`list` 缺失按空列表处理。
///
/// `upper` 与 `cnt_info` 两层在这里**摊平**：基线分别套了 `Owner` 与
/// `CntInfo`，而那两个类各自还有别的生产者（收藏夹详情等），把它们一起
/// 拖进本切片会让迁移面无限扩散。订阅项只读三个作者字段与两个计数，摊平
/// 之后不欠别的切片任何东西。
pub fn parse_sub_list(body: &str) -> Result<SubList, DomainError> {
    let dto: SubListDto = crate::envelope::unwrap_envelope(body)?;
    Ok(SubList {
        count: dto.count,
        has_more: dto.has_more,
        list: dto
            .list
            .unwrap_or_default()
            .into_iter()
            .map(|i| SubItem {
                id: i.id,
                fid: i.fid,
                mid: i.mid,
                attr: i.attr,
                title: i.title,
                cover: i.cover,
                upper_mid: i.upper.as_ref().and_then(|u| u.mid),
                upper_name: i.upper.as_ref().and_then(|u| u.name.clone()),
                upper_face: i.upper.and_then(|u| u.face),
                cover_type: i.cover_type,
                intro: i.intro,
                ctime: i.ctime,
                mtime: i.mtime,
                state: i.state,
                fav_state: i.fav_state,
                media_count: i.media_count,
                view_count: i.view_count,
                kind: i.kind,
                play_count: i.cnt_info.as_ref().and_then(|c| c.play),
                collect_count: i.cnt_info.and_then(|c| c.danmaku),
            })
            .collect(),
    })
}

/// 取订阅的收藏夹/合集一页。
///
/// # Errors
/// 同 [`fetch_user_info`]。
pub async fn fetch_sub_list<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    up_mid: i64,
    page: i64,
    page_size: i64,
) -> Result<SubList, DomainError> {
    let req = user_sub_folder_request(up_mid, page, page_size);
    let body = crate::client::send(transport, ctx, &req).await?;
    parse_sub_list(&body)
}

/// 视频标签的请求。`cid` 可省。
#[must_use]
pub fn video_tags_request(bvid: &str, cid: Option<i64>) -> PreparedRequest {
    let mut query = vec![("bvid".to_owned(), bvid.to_owned())];
    if let Some(cid) = cid {
        query.push(("cid".to_owned(), cid.to_string()));
    }
    PreparedRequest {
        path: path::VIDEO_TAGS,
        query,
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct VideoTagDto {
    /// 标识符，**不放宽**（#166）：点标签要拿它请求标签页。
    tag_id: Option<i64>,
    tag_name: Option<String>,
    tag_type: Option<String>,
    music_id: Option<String>,
}

/// 解析视频标签。
///
/// # Errors
/// 信封失败沿用统一分类。`data` 为 `null` 按空列表处理——基线取的是
/// `data as List?`，没有标签的稿件确实会给 null。
pub fn parse_video_tags(body: &str) -> Result<Vec<VideoTag>, DomainError> {
    let dto: Option<Vec<VideoTagDto>> = crate::envelope::unwrap_envelope_nullable(body)?;
    Ok(dto
        .unwrap_or_default()
        .into_iter()
        .map(|t| VideoTag {
            tag_id: t.tag_id,
            tag_name: t.tag_name,
            tag_type: t.tag_type,
            music_id: t.music_id,
        })
        .collect())
}

/// 取视频标签。
///
/// # Errors
/// 同 [`fetch_user_info`]。
pub async fn fetch_video_tags<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    bvid: &str,
    cid: Option<i64>,
) -> Result<Vec<VideoTag>, DomainError> {
    let body = crate::client::send(transport, ctx, &video_tags_request(bvid, cid)).await?;
    parse_video_tags(&body)
}

// -------------------------------------------------------------------- 写操作

/// 大会员经验领取。
///
/// 定级 [`Idempotency::Idempotent`]：服务端按天只记一次，重发落到同一个
/// 状态。基线把 `mid` 与 `csrf` 都放在**查询串**上，体是空的——照搬。
#[must_use]
pub fn vip_exp_add_request(mid: i64) -> PreparedWrite {
    PreparedWrite {
        path: path::VIP_EXP_ADD,
        query: vec![("mid".to_owned(), mid.to_string())],
        body: WriteBody::Form(Vec::new()),
        csrf: CsrfIn::Query,
        idempotency: Idempotency::Idempotent,
    }
}

/// 批量修改空间隐私设置。
///
/// `values` 是「key → 值」，整批是**置为**语义，所以定级为幂等。
#[must_use]
pub fn space_setting_mod_request(values: Vec<(String, String)>) -> PreparedWrite {
    PreparedWrite {
        path: path::SPACE_SETTING_MOD,
        query: Vec::new(),
        body: WriteBody::Form(values),
        csrf: CsrfIn::Query,
        idempotency: Idempotency::Idempotent,
    }
}

/// 举报一条动态。
///
/// 定级 [`Idempotency::Unsafe`]：重发会多提交一次举报，事后也查不到上一次
/// 是否已受理。
#[must_use]
pub fn dynamic_report_request(
    mid: i64,
    dyn_id: &str,
    reason_type: i64,
    reason_desc: Option<&str>,
) -> PreparedWrite {
    let mut form = vec![
        ("accused_uid".to_owned(), mid.to_string()),
        ("dynamic_id".to_owned(), dyn_id.to_owned()),
        ("reason_type".to_owned(), reason_type.to_string()),
    ];
    // 基线无条件带上这个键，值为 null 时 dio 会把它整条丢掉。
    if let Some(desc) = reason_desc {
        form.push(("reason_desc".to_owned(), desc.to_owned()));
    }
    PreparedWrite {
        path: path::DYNAMIC_REPORT,
        query: Vec::new(),
        body: WriteBody::Form(form),
        csrf: CsrfIn::Query,
        idempotency: Idempotency::Unsafe,
    }
}

/// 预约/取消预约一个空间活动。`reserve` 为 false 时打的是取消那条路径。
///
/// 两条都是置为语义，重发安全。
#[must_use]
pub fn space_reserve_request(sid: i64, reserve: bool) -> PreparedWrite {
    PreparedWrite::form_only(
        if reserve {
            path::SPACE_RESERVE
        } else {
            path::SPACE_RESERVE_CANCEL
        },
        vec![("sid".to_owned(), sid.to_string())],
        Idempotency::Idempotent,
    )
}

/// 领大会员经验。
///
/// # Errors
/// 传输、状态码、信封三类失败。
pub async fn vip_exp_add<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    mid: i64,
) -> Result<(), DomainError> {
    let body = crate::client::send_write(transport, ctx, &vip_exp_add_request(mid)).await?;
    crate::envelope::check_envelope(&body)
}

/// 批量改隐私设置。
///
/// # Errors
/// 传输、状态码、信封三类失败。
pub async fn space_setting_mod<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    values: Vec<(String, String)>,
) -> Result<(), DomainError> {
    let body =
        crate::client::send_write(transport, ctx, &space_setting_mod_request(values)).await?;
    crate::envelope::check_envelope(&body)
}

/// 举报动态。
///
/// # Errors
/// 传输、状态码、信封三类失败。
pub async fn dynamic_report<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    mid: i64,
    dyn_id: &str,
    reason_type: i64,
    reason_desc: Option<&str>,
) -> Result<(), DomainError> {
    let req = dynamic_report_request(mid, dyn_id, reason_type, reason_desc);
    let body = crate::client::send_write(transport, ctx, &req).await?;
    crate::envelope::check_envelope(&body)
}

/// 预约/取消预约。
///
/// # Errors
/// 传输、状态码、信封三类失败。
pub async fn space_reserve<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    sid: i64,
    reserve: bool,
) -> Result<(), DomainError> {
    let body =
        crate::client::send_write(transport, ctx, &space_reserve_request(sid, reserve)).await?;
    crate::envelope::check_envelope(&body)
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::{
        coin_log_request, dynamic_report_request, exp_log_request, parse_coin, parse_coin_log,
        parse_space_setting, parse_sub_list, parse_user_info, parse_user_stat, parse_video_tags,
        path, space_reserve_request, user_real_name_request, video_tags_request,
        vip_exp_add_request,
    };
    use crate::http::Idempotency;
    use crate::request::CsrfIn;
    use bili_domain::error::ErrorKind;

    /// issue #161 的回归：满级账号的 `next_exp` 服务端发的是字符串
    /// `"--"`，整条 nav 响应不能因此作废。
    ///
    /// 基线读不到这个形状（满级时它改取 `current_exp`），所以迁移时没人
    /// 发现它不是数字；而 serde 先解完整个 DTO 才轮到那条分支，于是一个
    /// 用不上的字段把登录与「我的」页一起打断。
    #[test]
    fn a_max_level_next_exp_is_a_string_and_must_not_fail_the_whole_response() {
        let info = parse_user_info(
            r#"{"code":0,"message":"0","data":{"isLogin":true,"uname":"满级用户","level_info":{"current_level":6,"current_min":28800,"current_exp":31000,"next_exp":"--"}}}"#,
        )
        .expect("满级账号的 nav 响应必须解析得了");
        assert_eq!(info.is_login, Some(true), "响应整体不能因为这个字段作废");
        let level = info.level_info.expect("有等级");
        assert_eq!(level.next_exp, Some(31000), "满级时分母取 current_exp");
        assert_eq!(level.current_min, Some(28800));
    }

    /// 数字以字符串形态出现时照样收下——服务端在这几个字段上并不一致。
    #[test]
    fn a_numeric_string_level_field_is_still_a_number() {
        let info = parse_user_info(
            r#"{"code":0,"message":"0","data":{"level_info":{"current_level":"5","current_exp":10800,"next_exp":"28800"}}}"#,
        )
        .expect("应解析成功");
        let level = info.level_info.expect("有等级");
        assert_eq!(level.current_level, Some(5));
        assert_eq!(level.next_exp, Some(28800), "未满级时用服务端给的那个");
    }

    #[test]
    fn max_level_takes_current_exp_as_the_denominator() {
        let six = parse_user_info(
            r#"{"code":0,"message":"0","data":{"level_info":{"current_level":6,"current_exp":28800,"next_exp":0}}}"#,
        )
        .expect("应解析成功");
        let level = six.level_info.expect("有等级");
        assert_eq!(
            level.next_exp,
            Some(28800),
            "满级时 next_exp 取 current_exp，否则进度条分母是 0"
        );

        let five = parse_user_info(
            r#"{"code":0,"message":"0","data":{"level_info":{"current_level":5,"current_exp":10800,"next_exp":28800}}}"#,
        )
        .expect("应解析成功");
        assert_eq!(five.level_info.expect("有等级").next_exp, Some(28800));
    }

    #[test]
    fn vip_fields_are_lower_camel_in_the_protocol() {
        let info = parse_user_info(
            r#"{"code":0,"message":"0","data":{"isLogin":true,"vipStatus":1,"vipType":2,"is_senior_member":1}}"#,
        )
        .expect("应解析成功");
        assert_eq!(info.is_login, Some(true), "未登录时这条接口也回 code 0");
        assert_eq!(info.vip_status, Some(1));
        assert_eq!(info.vip_type, Some(2));
        assert_eq!(info.is_senior_member, Some(1), "同一个响应里风格并不统一");
    }

    #[test]
    fn missing_coin_is_none_not_an_error() {
        assert_eq!(
            parse_coin(r#"{"code":0,"message":"0","data":null}"#).expect("不是错误"),
            None
        );
        assert_eq!(
            parse_coin(r#"{"code":0,"message":"0","data":{"money":12.5}}"#).expect("应解析成功"),
            Some(12.5)
        );
    }

    #[test]
    fn integral_delta_is_not_rendered_with_a_trailing_zero() {
        let log = parse_coin_log(
            r#"{"code":0,"message":"0","data":{"list":[{"time":"2026-11-16","delta":2,"reason":"投币"},{"time":"2026-11-15","delta":0.4,"reason":"登录"}]}}"#,
        )
        .expect("应解析成功");
        assert_eq!(log[0].delta, "2", "整数不该显示成 2.0");
        assert_eq!(log[1].delta, "0.4");
    }

    #[test]
    fn the_three_logs_share_the_same_fixed_params() {
        assert_eq!(
            coin_log_request().query_string(),
            "jsonp=jsonp&web_location=333.33"
        );
        assert_eq!(exp_log_request().path, path::EXP_LOG);
    }

    #[test]
    fn privacy_keeps_the_reversed_flags_and_the_grouping() {
        let settings = parse_space_setting(
            r#"{"code":0,"message":"0","data":{"privacy":{"fav_video":1,"disable_following":0}}}"#,
        )
        .expect("应解析成功");
        assert_eq!(settings.len(), 15, "十五个开关一个都不能少");

        let fav = settings
            .iter()
            .find(|s| s.key == "fav_video")
            .expect("有这项");
        assert!(!fav.reversed);
        assert_eq!(fav.value, Some(1));
        assert_eq!(fav.group, 1);

        let following = settings
            .iter()
            .find(|s| s.key == "disable_following")
            .expect("有这项");
        assert!(following.reversed, "读错这个就是把隐私设置显示反");

        let medal = settings
            .iter()
            .find(|s| s.key == "close_space_medal")
            .expect("有这项");
        assert_eq!(medal.group, 2);
        assert_eq!(medal.value, None, "服务端没下发的开关不替它填默认值");
    }

    #[test]
    fn a_video_without_tags_is_not_an_error() {
        assert!(
            parse_video_tags(r#"{"code":0,"message":"0","data":null}"#)
                .expect("不是错误")
                .is_empty()
        );
        let q = video_tags_request("BV1xx411c7mD", None).query_string();
        assert_eq!(q, "bvid=BV1xx411c7mD", "cid 缺席时整条丢掉");
    }

    #[test]
    fn real_name_is_app_signed_and_carries_no_csrf() {
        let q = user_real_name_request(
            "ak",
            42,
            "dfca71928277209b",
            "b5475a8825547a4fc26c7d518eaaa02e",
            1_700_000_000,
        )
        .query_string();
        assert!(
            q.contains("access_key=ak") && q.contains("up_mid=42"),
            "{q}"
        );
        assert!(
            q.contains("appkey=") && q.contains("ts=") && q.contains("sign="),
            "{q}"
        );
        assert!(!q.contains("csrf"), "APP 端接口用 access_key 鉴权 — {q}");
    }

    #[test]
    fn write_shapes_are_pinned() {
        let vip = vip_exp_add_request(42);
        assert_eq!(vip.csrf, CsrfIn::Query, "基线把 mid 和 csrf 都放在查询串上");
        assert_eq!(vip.query, vec![("mid".to_owned(), "42".to_owned())]);

        let report = dynamic_report_request(1, "900", 3, None);
        assert_eq!(
            report.form_string(),
            "accused_uid=1&dynamic_id=900&reason_type=3",
            "reason_desc 为 null 时整条丢掉"
        );
        assert_eq!(report.idempotency, Idempotency::Unsafe, "重发会多一次举报");
        assert!(
            dynamic_report_request(1, "900", 3, Some("理由"))
                .form_string()
                .contains("reason_desc=理由")
        );

        assert_eq!(space_reserve_request(7, true).path, path::SPACE_RESERVE);
        assert_eq!(
            space_reserve_request(7, false).path,
            path::SPACE_RESERVE_CANCEL
        );
    }

    /// #166：三个计数是展示用的，字符串形态照收，不能因此判整条失败。
    #[test]
    fn user_stat_counts_accept_the_string_form() {
        let stat = parse_user_stat(
            r#"{"code":0,"message":"0","data":{"following":"120","follower":1048576,"dynamic_count":"--"}}"#,
        )
        .expect("计数的写法不该让整条响应作废");
        assert_eq!(stat.following, Some(120));
        assert_eq!(stat.follower, Some(1_048_576));
        assert_eq!(stat.dynamic_count, None, "解不出来当没有，而不是报错");
    }

    /// #166：订阅列表的计数同样放宽，而标识符与判别字段**仍然收紧**。
    #[test]
    fn sub_list_loosens_counts_but_not_identifiers() {
        let list = parse_sub_list(
            r#"{"code":0,"message":"0","data":{"count":"2","list":[
                 {"id":11,"fid":12,"mid":13,"type":21,"title":"合集",
                  "media_count":"37","view_count":"1200",
                  "upper":{"mid":14,"name":"up"},
                  "cnt_info":{"play":"9000","danmaku":"8"}}]}}"#,
        )
        .expect("应解析成功");
        assert_eq!(list.count, Some(2));
        let item = &list.list[0];
        assert_eq!(item.media_count, Some(37));
        assert_eq!(item.view_count, Some(1200));
        assert_eq!(item.play_count, Some(9000));
        assert_eq!(item.collect_count, Some(8));
        assert_eq!(item.id, Some(11), "标识符仍按数字收");
        assert_eq!(item.kind, Some(21), "判别字段仍按数字收");

        // 标识符给成字符串时**整条失败**，这是刻意的：静默当没有会把一条
        // 打不通的链路伪装成一个空页面，比报错难查得多（#166）。
        let err =
            parse_sub_list(r#"{"code":0,"message":"0","data":{"count":1,"list":[{"id":"11"}]}}"#)
                .expect_err("标识符不放宽");
        assert_eq!(err.kind(), ErrorKind::Protocol);
    }
}
