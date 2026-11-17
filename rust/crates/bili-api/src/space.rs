//! 用户空间那一族的请求构造与解析（S6 / issue #56）。
//!
//! 对应基线 `lib/http/member.dart`。本模块按「看别人」这条线组织：
//! 资料卡、计数、名片、举报。
//!
//! ## 请求头不在这里
//!
//! 这一族里好几条要显式带浏览器 UA 与 `space.bilibili.com` 的
//! `origin`/`referer`（服务端据此判断是不是网页空间页）。[`PreparedRequest`]
//! 只有 path 与 query，没有头——头由 `lib/services/rust/rust_space.dart`
//! 按条补上，同 `rust_search.dart` / `rust_video_feed.dart` 的既有做法。
//!
//! 按 AGENTS.md「例外要写下来」：这是**宿主在拼协议**，本该在 Rust。
//! 给 `PreparedRequest` 加 headers 会动到全部已迁切片的导出面，不在本
//! 切片范围内；**由统一改造请求表示的那个 PR 收口**（见 issue #67）。

use bili_domain::error::DomainError;
use bili_domain::space::{
    CoinLikeArcItem, CoinLikeArcPage, GuardItem, MemberCard, MemberGuardPage, MemberInfo,
    MemberRelationStat, MemberUpStat, SpaceArchiveHistory, SpaceArchiveItem, SpaceArchivePage,
    SpaceArticleBlock, SpaceArticleItem, SpaceAudioBlock, SpaceAudioItem, SpaceCard, SpaceCardTag,
    SpaceElec, SpaceEpisodicButton, SpaceFavBlock, SpaceFavItem, SpaceFollowedBy, SpaceGuard,
    SpaceImages, SpaceLive, SpaceLiveMedal, SpacePage, SpacePrInfo, SpaceRelation,
    SpaceReservation, SpaceReservationLottery, SpaceSeasonSeries, SpaceSeasonSeriesPage,
    SpaceSetting, SpaceSimpleUser, SpaceTab2, SpaceTab2Item, SpaceTabFilter, SpaceTopImage,
    SpaceTopTitle, SpaceVideoBlock, UpowerLevel, UpowerRank, UpowerRankItem,
};
use bili_protocol::risk::RiskTokens;
use serde::Deserialize;

use crate::http::{HttpTransport, Idempotency, RequestContext};
use crate::request::{PreparedRequest, PreparedWrite};

/// 接口路径。与基线 `lib/http/api.dart` 保持一致。
pub mod path {
    pub const MEMBER_INFO: &str = "/x/space/wbi/acc/info";
    pub const MEMBER_RELATION_STAT: &str = "/x/relation/stat";
    pub const MEMBER_UP_STAT: &str = "/x/space/upstat";
    pub const MEMBER_CARD: &str = "/x/web-interface/card";

    /// 举报。**挂在 `space.bilibili.com` 上**，不是 api 域，base_url 由
    /// 调用方给。
    pub const REPORT_MEMBER: &str = "/ajax/report/add";

    pub const SEASON_SERIES: &str = "/x/polymer/web-space/seasons_series_list";
    pub const UPOWER_RANK: &str = "/x/upower/up/member/rank/v2";

    /// 投币过的稿件。**挂在 `app.bilibili.com` 上**，base_url 由调用方给。
    pub const COIN_ARC: &str = "/x/v2/space/coinarc";
    /// 点赞过的稿件。同上，与 [`COIN_ARC`] 只差路径。
    pub const LIKE_ARC: &str = "/x/v2/space/likearc";

    /// 大航海名单。**挂在 `live.bilibili.com` 上**，base_url 由调用方给。
    pub const MEMBER_GUARD: &str = "/xlive/app-ucenter/v1/guard/MainGuardCardAll";

    /// 空间主页。下面这一族全**挂在 `app.bilibili.com` 上**。
    pub const SPACE: &str = "/x/v2/space";
    /// 投稿视频。唯一一条游标分页的。
    pub const SPACE_ARCHIVE: &str = "/x/v2/space/archive/cursor";
    /// 充电专属稿件。
    pub const SPACE_CHARGING_ARCHIVE: &str = "/x/v2/space/archive/charging";
    /// 合集里的视频。
    pub const SPACE_SEASON: &str = "/x/v2/space/season/videos";
    /// 列表里的视频。
    pub const SPACE_SERIES: &str = "/x/v2/space/series";
    /// 追番。
    pub const SPACE_BANGUMI: &str = "/x/v2/space/bangumi";
    /// 漫画。
    pub const SPACE_COMIC: &str = "/x/v2/space/comic";
}

/// `web_location`：空间页的页面来源标记，服务端据此分流风控策略。
/// 与别处的 `333.1387`（空间内各 tab）不是一回事，不要合并。
const WEB_LOCATION_SPACE: &str = "1550101";

// ------------------------------------------------------------------ 资料卡

/// 别人的资料的请求。WBI 签名。
///
/// `token` 在基线里恒为空串，但**照样发出去**——它参与签名，省掉会换一个
/// 不同的 `w_rid`。
#[must_use]
pub fn member_info_request(
    mid: i64,
    token: &str,
    tokens: &RiskTokens,
    mixin_key: &str,
    wts: i64,
) -> PreparedRequest {
    let mut query = vec![
        ("mid".to_owned(), mid.to_string()),
        ("token".to_owned(), token.to_owned()),
        ("platform".to_owned(), "web".to_owned()),
        ("web_location".to_owned(), WEB_LOCATION_SPACE.to_owned()),
        (
            "dm_img_list".to_owned(),
            bili_protocol::risk::DM_IMG_LIST.to_owned(),
        ),
        ("dm_img_str".to_owned(), tokens.dm_img_str.clone()),
        (
            "dm_cover_img_str".to_owned(),
            tokens.dm_cover_img_str.clone(),
        ),
        (
            "dm_img_inter".to_owned(),
            bili_protocol::risk::DM_IMG_INTER.to_owned(),
        ),
        ("wts".to_owned(), wts.to_string()),
    ];
    query.sort_by(|a, b| a.0.cmp(&b.0));
    let w_rid = bili_protocol::wbi::sign(&query, mixin_key);
    query.push(("w_rid".to_owned(), w_rid));
    PreparedRequest {
        path: path::MEMBER_INFO,
        query,
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct MemberInfoDto {
    mid: Option<i64>,
    name: Option<String>,
    face: Option<String>,
    level: Option<i64>,
    is_followed: Option<bool>,
    vip: Option<VipDto>,
    is_senior_member: Option<i64>,
}

/// 大会员信息。
///
/// 基线的 `Vip.fromJson` 对每个字段都试两个键
/// （`type ?? vipType`、`status ?? vipStatus`）：同一个结构在不同接口下
/// 有两种键名，`Vip` 被所有接口共用，于是两套都认。这里照搬——这条接口
/// 现在给的是哪一套没有人验证过，改成只认一套是在赌。
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct VipDto {
    #[serde(rename = "type")]
    kind: Option<i64>,
    #[serde(rename = "vipType")]
    kind_alt: Option<i64>,
    status: Option<i64>,
    #[serde(rename = "vipStatus")]
    status_alt: Option<i64>,
    /// 大会员标签（「十年大会员」之类）。只有空间主页那条读它。
    label: Option<VipLabelDto>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct VipLabelDto {
    text: Option<String>,
}

impl VipDto {
    fn kind(&self) -> Option<i64> {
        self.kind.or(self.kind_alt)
    }

    /// 基线的 `status` 是 `late int`，缺了就落到 0（不是 null）。两个显示
    /// 判据都是 `status > 0`，所以 0 与 null 在界面上同义；保留 `Option`
    /// 只会让每个调用点多写一次 `?? 0`。
    fn status(&self) -> i64 {
        self.status.or(self.status_alt).unwrap_or(0)
    }
}

/// 解析别人的资料。
///
/// # Errors
/// 信封失败沿用统一分类。
pub fn parse_member_info(body: &str) -> Result<MemberInfo, DomainError> {
    let dto: MemberInfoDto = crate::envelope::unwrap_envelope(body)?;
    Ok(MemberInfo {
        mid: dto.mid,
        name: dto.name,
        face: dto.face,
        level: dto.level,
        is_followed: dto.is_followed,
        vip_status: dto.vip.as_ref().map(VipDto::status),
        vip_type: dto.vip.as_ref().and_then(VipDto::kind),
        is_senior_member: dto.is_senior_member,
    })
}

/// 取别人的资料。
///
/// # Errors
/// 传输、状态码、信封三类失败，见各自的文档。
pub async fn fetch_member_info<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    mid: i64,
    token: &str,
    tokens: &RiskTokens,
    mixin_key: &str,
    wts: i64,
) -> Result<MemberInfo, DomainError> {
    let req = member_info_request(mid, token, tokens, mixin_key, wts);
    let body = crate::client::send(transport, ctx, &req).await?;
    parse_member_info(&body)
}

// -------------------------------------------------------------------- 计数

/// 关注关系计数的请求。
#[must_use]
pub fn member_relation_stat_request(mid: i64) -> PreparedRequest {
    PreparedRequest {
        path: path::MEMBER_RELATION_STAT,
        query: vec![("vmid".to_owned(), mid.to_string())],
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct MemberRelationStatDto {
    following: Option<i64>,
    follower: Option<i64>,
}

/// 解析关注关系计数。
///
/// # Errors
/// 信封失败沿用统一分类。
pub fn parse_member_relation_stat(body: &str) -> Result<MemberRelationStat, DomainError> {
    let dto: MemberRelationStatDto = crate::envelope::unwrap_envelope(body)?;
    Ok(MemberRelationStat {
        following: dto.following,
        follower: dto.follower,
    })
}

/// 取关注关系计数。
///
/// # Errors
/// 同 [`fetch_member_info`]。
pub async fn fetch_member_relation_stat<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    mid: i64,
) -> Result<MemberRelationStat, DomainError> {
    let req = member_relation_stat_request(mid);
    let body = crate::client::send(transport, ctx, &req).await?;
    parse_member_relation_stat(&body)
}

/// 播放与获赞总数的请求。
#[must_use]
pub fn member_up_stat_request(mid: i64) -> PreparedRequest {
    PreparedRequest {
        path: path::MEMBER_UP_STAT,
        query: vec![("mid".to_owned(), mid.to_string())],
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct MemberUpStatDto {
    archive: Option<ViewDto>,
    article: Option<ViewDto>,
    likes: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ViewDto {
    view: Option<i64>,
}

/// 解析播放与获赞总数。
///
/// # Errors
/// 信封失败沿用统一分类。
pub fn parse_member_up_stat(body: &str) -> Result<MemberUpStat, DomainError> {
    let dto: MemberUpStatDto = crate::envelope::unwrap_envelope(body)?;
    Ok(MemberUpStat {
        archive_view: dto.archive.and_then(|a| a.view),
        article_view: dto.article.and_then(|a| a.view),
        likes: dto.likes,
    })
}

/// 取播放与获赞总数。需要登录。
///
/// # Errors
/// 同 [`fetch_member_info`]。
pub async fn fetch_member_up_stat<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    mid: i64,
) -> Result<MemberUpStat, DomainError> {
    let req = member_up_stat_request(mid);
    let body = crate::client::send(transport, ctx, &req).await?;
    parse_member_up_stat(&body)
}

// -------------------------------------------------------------------- 名片

/// 用户名片的请求。
///
/// `photo=false` 是基线写死的：要的只是昵称与计数，不要空间头图。
#[must_use]
pub fn member_card_request(mid: i64) -> PreparedRequest {
    PreparedRequest {
        path: path::MEMBER_CARD,
        query: vec![
            ("mid".to_owned(), mid.to_string()),
            ("photo".to_owned(), "false".to_owned()),
        ],
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct MemberCardDataDto {
    card: Option<MemberCardDto>,
    archive_count: Option<i64>,
    follower: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct MemberCardDto {
    mid: Option<String>,
    name: Option<String>,
    face: Option<String>,
    /// 这条接口的认证信息键名是**大写开头**的 `Official`。服务端的事实。
    #[serde(rename = "Official")]
    official: Option<OfficialDto>,
    vip: Option<VipDto>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct OfficialDto {
    #[serde(rename = "type")]
    kind: Option<i64>,
}

/// 解析用户名片。
///
/// # Errors
/// 信封失败沿用统一分类。
pub fn parse_member_card(body: &str) -> Result<MemberCard, DomainError> {
    let dto: MemberCardDataDto = crate::envelope::unwrap_envelope(body)?;
    let card = dto.card.unwrap_or_default();
    Ok(MemberCard {
        mid: card.mid,
        name: card.name,
        face: card.face,
        official_type: card.official.and_then(|o| o.kind),
        vip_status: card.vip.as_ref().map(VipDto::status),
        vip_type: card.vip.as_ref().and_then(VipDto::kind),
        archive_count: dto.archive_count,
        follower: dto.follower,
    })
}

/// 取用户名片。
///
/// # Errors
/// 同 [`fetch_member_info`]。
pub async fn fetch_member_card<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    mid: i64,
) -> Result<MemberCard, DomainError> {
    let req = member_card_request(mid);
    let body = crate::client::send(transport, ctx, &req).await?;
    parse_member_card(&body)
}

// -------------------------------------------------------------------- 举报

/// 举报用户。
///
/// 定级 [`Idempotency::Unsafe`]：重发会重复提交举报工单。服务端没有去重
/// 承诺，而「误报一次」对被举报的人是实打实的代价，按 ADR-013 从严。
///
/// `reason_v2` 在基线里是可空的（选了预置理由才有），省掉时整条参数不发。
#[must_use]
pub fn report_member_request(mid: i64, reason: &str, reason_v2: Option<i64>) -> PreparedWrite {
    let mut body = vec![
        ("mid".to_owned(), mid.to_string()),
        ("reason".to_owned(), reason.to_owned()),
    ];
    if let Some(v2) = reason_v2 {
        body.push(("reason_v2".to_owned(), v2.to_string()));
    }
    PreparedWrite::form_only(path::REPORT_MEMBER, body, Idempotency::Unsafe)
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ReportResultDto {
    status: Option<bool>,
}

/// 解析举报结果。
///
/// **这条不是标准信封**：响应顶层就是 `{"status": true}`，没有
/// `code`/`message`/`data`。基线也是直接读 `res.data['status']`。所以不能
/// 走 [`crate::envelope`]——那会把一次成功的举报判成协议错误。
///
/// # Errors
/// 响应不是合法 JSON 时为协议错误。`status` 为 false 或缺失都算失败，
/// 返回 `Ok(false)`：服务端没有给出原因，调用方能做的只是提示失败。
pub fn parse_report_result(body: &str) -> Result<bool, DomainError> {
    let dto: ReportResultDto = serde_json::from_str(body).map_err(|e| {
        DomainError::new(
            bili_domain::error::ErrorKind::Protocol,
            format!("举报响应不是合法 JSON：{e}"),
        )
    })?;
    Ok(dto.status.unwrap_or(false))
}

/// 举报用户。返回服务端是否受理。
///
/// # Errors
/// 传输、状态码两类失败；响应不是 JSON 时为协议错误。
pub async fn report_member<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    mid: i64,
    reason: &str,
    reason_v2: Option<i64>,
) -> Result<bool, DomainError> {
    let req = report_member_request(mid, reason, reason_v2);
    let body = crate::client::send_write(transport, ctx, &req).await?;
    parse_report_result(&body)
}

/// 投稿列表的六个 tab。
///
/// 对应基线 `ContributeType`。放在这里而不是 `bili_domain`：它承载的是
/// 「这个 tab 发哪条路径、封面要哪档分辨率」，属于协议知识。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpaceArchiveKind {
    /// 投稿视频。唯一一条游标分页的。
    Video,
    /// 充电专属。
    Charging,
    /// 合集。
    Season,
    /// 列表。
    Series,
    /// 追番。
    Bangumi,
    /// 漫画。
    Comic,
}

impl SpaceArchiveKind {
    #[must_use]
    pub const fn path(self) -> &'static str {
        match self {
            Self::Video => path::SPACE_ARCHIVE,
            Self::Charging => path::SPACE_CHARGING_ARCHIVE,
            Self::Season => path::SPACE_SEASON,
            Self::Series => path::SPACE_SERIES,
            Self::Bangumi => path::SPACE_BANGUMI,
            Self::Comic => path::SPACE_COMIC,
        }
    }

    /// 封面分辨率档位。基线只给视频那条 80，其余 32。
    #[must_use]
    pub const fn cover_quality(self) -> i64 {
        match self {
            Self::Video => 80,
            _ => 32,
        }
    }
}

/// 投稿视频的排序字段。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpaceArchiveOrder {
    /// 最新发布。
    Pubdate,
    /// 最多播放。
    Click,
}

impl SpaceArchiveOrder {
    #[must_use]
    pub const fn as_param(self) -> &'static str {
        match self {
            Self::Pubdate => "pubdate",
            Self::Click => "click",
        }
    }
}

/// 合集 / 列表的排序方向。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpaceArchiveSort {
    Desc,
    Asc,
}

impl SpaceArchiveSort {
    #[must_use]
    pub const fn as_param(self) -> &'static str {
        match self {
            Self::Desc => "desc",
            Self::Asc => "asc",
        }
    }
}

// ---------------------------------------------------------------- 空间主页

/// app 域那几条接口的固定参数。值照抄基线 `Constants`——它们是客户端指纹
/// 的一部分，换一个值换来的是风控分支，不是参数错误。
mod app_fixed {
    pub const BUILD: &str = "8430300";
    pub const VERSION: &str = "8.43.0";
    pub const CHANNEL: &str = "master";
    pub const MOBI_APP: &str = "android";
    pub const PLATFORM: &str = "android";
    pub const LOCALE: &str = "zh_CN";
    pub const STATISTICS: &str = r#"{"appId":1,"platform":3,"version":"8.43.0","abtest":""}"#;
}

/// app 域那几条接口都带的那组参数。
fn app_common_query() -> Vec<(String, String)> {
    vec![
        ("build".to_owned(), app_fixed::BUILD.to_owned()),
        ("version".to_owned(), app_fixed::VERSION.to_owned()),
        ("c_locale".to_owned(), app_fixed::LOCALE.to_owned()),
        ("channel".to_owned(), app_fixed::CHANNEL.to_owned()),
        ("mobi_app".to_owned(), app_fixed::MOBI_APP.to_owned()),
        ("platform".to_owned(), app_fixed::PLATFORM.to_owned()),
        ("s_locale".to_owned(), app_fixed::LOCALE.to_owned()),
        ("statistics".to_owned(), app_fixed::STATISTICS.to_owned()),
    ]
}

/// 空间主页的请求。**挂在 app 域**，base_url 由调用方给。
///
/// `from_view_aid` 是从视频页点进来时带的稿件 id，服务端据此把那条稿件
/// 排到投稿列表前面。基线声明成 `dynamic` 并原样塞进查询串，这里收成
/// 字符串——它只会被拼进 URL，转成整数再转回来只多一个出错点。
///
/// ## 这条接口现在要 app 签名，基线没签
///
/// 2026-11-17 实测：不带 `appkey`/`sign` 的匿名请求一律 `-400 请求错误`，
/// 带了就正常。基线 `MemberHttp.space` 从来没签过，也就是说空间页在当前
/// 服务端上是坏的。**本切片不顺手修**：加签名是改请求面，属于协议变更，
/// 要单独一条 issue、单独记 `protocol-changelog.md`。这里照抄基线的参数，
/// 把发现写在这儿，免得下一个人以为是迁移迁坏的。
#[must_use]
pub fn space_request(mid: i64, from_view_aid: Option<&str>) -> PreparedRequest {
    // 参数顺序照抄基线。这条没有签名，顺序本不影响结果，但两边一致时
    // 抓包对照是逐行的，不用先排序再比。
    let mut query: Vec<(String, String)> = app_common_query()
        .into_iter()
        .filter(|(k, _)| k != "statistics")
        .collect();
    if let Some(aid) = from_view_aid {
        query.push(("from_view_aid".to_owned(), aid.to_owned()));
    }
    query.push(("statistics".to_owned(), app_fixed::STATISTICS.to_owned()));
    query.push(("vmid".to_owned(), mid.to_string()));
    PreparedRequest {
        path: path::SPACE,
        query,
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SpacePageDto {
    relation: Option<i64>,
    guest_relation: Option<i64>,
    rel_special: Option<i64>,
    medal: Option<i64>,
    default_tab: Option<String>,
    setting: Option<SpaceSettingDto>,
    card: Option<SpaceCardDto>,
    images: Option<SpaceImagesDto>,
    live: Option<SpaceLiveDto>,
    elec: Option<SpaceElecDto>,
    guard: Option<SpaceGuardDto>,
    archive: Option<SpaceVideoBlockDto>,
    season: Option<SpaceVideoBlockDto>,
    coin_archive: Option<SpaceVideoBlockDto>,
    like_archive: Option<SpaceVideoBlockDto>,
    comic: Option<SpaceVideoBlockDto>,
    article: Option<SpaceArticleBlockDto>,
    audios: Option<SpaceAudioBlockDto>,
    favourite2: Option<SpaceFavBlockDto>,
    series: Option<SpaceCountedListDto>,
    ugc_season: Option<SpaceCountedListDto>,
    tab2: Option<Vec<SpaceTab2Dto>>,
    reservation_card_list: Option<Vec<SpaceReservationDto>>,
}

/// `series` / `ugc_season` 只剩「有几个」「空不空」两个问题，条目整条丢掉。
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SpaceCountedListDto {
    count: Option<i64>,
    item: Option<Vec<serde::de::IgnoredAny>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SpaceSettingDto {
    fav_video: Option<i64>,
    coins_video: Option<i64>,
    likes_video: Option<i64>,
    bangumi: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SpaceCardDto {
    mid: Option<String>,
    name: Option<String>,
    face: Option<String>,
    sign: Option<String>,
    fans: Option<i64>,
    attention: Option<i64>,
    likes: Option<SpaceLikesDto>,
    level_info: Option<SpaceLevelInfoDto>,
    pendant: Option<SpacePendantDto>,
    official_verify: Option<SpaceOfficialVerifyDto>,
    vip: Option<VipDto>,
    silence: Option<i64>,
    relation: Option<SpaceRelationDto>,
    live_fans_wearing: Option<SpaceLiveFansWearingDto>,
    space_tag: Option<Vec<SpaceCardTagDto>>,
    pr_info: Option<SpacePrInfoDto>,
    followings_followed_upper: Option<SpaceFollowedByDto>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SpaceLikesDto {
    like_num: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SpaceLevelInfoDto {
    current_level: Option<i64>,
    identity: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SpacePendantDto {
    image: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SpaceOfficialVerifyDto {
    #[serde(rename = "type")]
    kind: Option<i64>,
    desc: Option<String>,
    splice_title: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SpaceRelationDto {
    status: Option<i64>,
    is_follow: Option<i64>,
    is_followed: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SpaceLiveFansWearingDto {
    detail_v2: Option<SpaceLiveMedalDto>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SpaceLiveMedalDto {
    medal_name: Option<String>,
    level: Option<i64>,
    medal_color: Option<String>,
    medal_color_name: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SpaceCardTagDto {
    #[serde(rename = "type")]
    kind: Option<String>,
    title: Option<String>,
    uri: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SpacePrInfoDto {
    content: Option<String>,
    url: Option<String>,
    icon: Option<String>,
    icon_night: Option<String>,
    text_color: Option<String>,
    bg_color: Option<String>,
    text_color_night: Option<String>,
    bg_color_night: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SpaceFollowedByDto {
    items: Option<Vec<SpaceSimpleUserDto>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SpaceSimpleUserDto {
    mid: Option<i64>,
    name: Option<String>,
    face: Option<String>,
    /// 充电榜那份用的是另一对键名。
    uname: Option<String>,
    avatar: Option<String>,
}

impl SpaceSimpleUserDto {
    fn into_domain(self) -> SpaceSimpleUser {
        SpaceSimpleUser {
            mid: self.mid,
            name: self.name.or(self.uname),
            face: self.face.or(self.avatar),
        }
    }

    /// 充电榜那份的 `mid` 是**被充电的那个人**（也就是空间主人），充电者
    /// 的 id 在 `pay_mid`。照搬 `mid` 会让「点头像进谁的空间」全部指向
    /// 主人自己。基线的 `ElecItem` 干脆不给 mid（它的构造函数只收昵称与
    /// 头像），这里保持同样的结果——没有人需要这个 id。
    fn into_domain_anonymous(self) -> SpaceSimpleUser {
        SpaceSimpleUser {
            mid: None,
            name: self.name.or(self.uname),
            face: self.face.or(self.avatar),
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SpaceImagesDto {
    #[serde(rename = "imgUrl")]
    img_url: Option<String>,
    night_imgurl: Option<String>,
    collection_top_simple: Option<SpaceCollectionTopSimpleDto>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SpaceCollectionTopSimpleDto {
    top: Option<SpaceTopDto>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SpaceTopDto {
    result: Option<Vec<SpaceTopImageDto>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SpaceTopImageDto {
    cover: Option<String>,
    title: Option<SpaceTopTitleDto>,
    item: Option<SpaceTopItemDto>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SpaceTopItemDto {
    image: Option<SpaceTopPictureDto>,
    animation: Option<SpaceTopPictureDto>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SpaceTopPictureDto {
    default_image: Option<String>,
    /// 形如 `0-29-1079-1079`。
    location: Option<String>,
    height: Option<f64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SpaceTopTitleDto {
    title: Option<String>,
    sub_title: Option<String>,
    sub_title_color_format: Option<SpaceTopColorsDto>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SpaceTopColorsDto {
    colors: Option<Vec<String>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SpaceLiveDto {
    #[serde(rename = "roomStatus")]
    room_status: Option<i64>,
    #[serde(rename = "liveStatus")]
    live_status: Option<i64>,
    roomid: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SpaceElecDto {
    total: Option<i64>,
    list: Option<Vec<SpaceSimpleUserDto>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SpaceGuardDto {
    desc: Option<String>,
    item: Option<Vec<SpaceSimpleUserDto>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SpaceVideoBlockDto {
    count: Option<i64>,
    item: Option<Vec<SpaceArchiveItemDto>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SpaceArticleBlockDto {
    count: Option<i64>,
    item: Option<Vec<SpaceArticleItemDto>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SpaceArticleItemDto {
    title: Option<String>,
    origin_image_urls: Option<Vec<String>>,
    publish_time_text: Option<String>,
    stats: Option<SpaceArticleStatsDto>,
    uri: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SpaceArticleStatsDto {
    view: Option<i64>,
    reply: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SpaceAudioBlockDto {
    count: Option<i64>,
    item: Option<Vec<SpaceAudioItemDto>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SpaceAudioItemDto {
    id: Option<i64>,
    uid: Option<i64>,
    title: Option<String>,
    cover: Option<String>,
    ctime: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SpaceFavBlockDto {
    count: Option<i64>,
    item: Option<Vec<SpaceFavItemDto>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SpaceFavItemDto {
    media_id: Option<i64>,
    title: Option<String>,
    cover: Option<String>,
    count: Option<i64>,
    is_public: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SpaceTab2Dto {
    title: Option<String>,
    param: Option<String>,
    items: Option<Vec<SpaceTab2ItemDto>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SpaceTab2ItemDto {
    title: Option<String>,
    param: Option<String>,
    filter: Option<Vec<SpaceTabFilterDto>>,
    season_id: Option<i64>,
    series_id: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SpaceTabFilterDto {
    text: Option<String>,
    meta: Option<String>,
    tab_name: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SpaceReservationDto {
    sid: Option<i64>,
    name: Option<String>,
    total: Option<i64>,
    is_follow: Option<i64>,
    desc_text_1: Option<SpaceReservationDescDto>,
    dynamic_id: Option<String>,
    lottery_prize_info: Option<SpaceReservationLotteryDto>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SpaceReservationDescDto {
    text: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SpaceReservationLotteryDto {
    text: Option<String>,
    jump_url: Option<String>,
}

/// 空串按没有。服务端在「这块没内容」时给的是 `""` 而不是省略键。
fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|s| !s.is_empty())
}

/// 置顶合集封面的纵向裁切位置。
///
/// `location` 形如 `0-29-1079-1079`，取第二、三段与 `height` 一起算：
/// `(start + end) / height - 1`。基线用 `num.parse` 并整段 try/catch，
/// 任何一步不成立就退回 0（居中）；这里用同样的宽容度。
fn top_image_dy(picture: Option<&SpaceTopPictureDto>) -> f64 {
    let Some(picture) = picture else {
        return 0.0;
    };
    let Some(location) = picture.location.as_deref().filter(|s| !s.is_empty()) else {
        return 0.0;
    };
    let mut parts = location.split('-').skip(1);
    let (Some(start), Some(end)) = (parts.next(), parts.next()) else {
        return 0.0;
    };
    let (Ok(start), Ok(end)) = (start.parse::<f64>(), end.parse::<f64>()) else {
        return 0.0;
    };
    match picture.height {
        Some(height) if height != 0.0 => (start + end) / height - 1.0,
        _ => 0.0,
    }
}

fn top_image(dto: SpaceTopImageDto) -> Option<SpaceTopImage> {
    // 没有原图这条就画不出来。基线在这里会抛，然后被 `Top.fromJson` 的
    // try/catch 吞掉**整组图**；我们只丢这一条。
    let full_cover = non_empty(dto.cover)?;
    let item = dto.item.unwrap_or_default();
    // `image` 与 `animation` 二选一：静态封面走前者，动态封面走后者。
    let picture = item.image.as_ref().or(item.animation.as_ref());
    let header = picture
        .and_then(|p| p.default_image.clone())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| full_cover.clone());
    Some(SpaceTopImage {
        header,
        full_cover,
        dy: top_image_dy(picture),
        title: dto.title.map(|t| SpaceTopTitle {
            title: t.title,
            sub_title: t.sub_title,
            sub_title_colors: t
                .sub_title_color_format
                .and_then(|c| c.colors)
                .unwrap_or_default(),
        }),
    })
}

/// 大航海人数。服务端只给一句「2人加入大航海」，数字在句首。
fn guard_count(desc: Option<&str>) -> Option<String> {
    let desc = desc?;
    let digits: String = desc.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() {
        None
    } else {
        Some(digits)
    }
}

fn space_card(dto: SpaceCardDto) -> SpaceCard {
    let official = dto.official_verify.unwrap_or_default();
    let level = dto.level_info.unwrap_or_default();
    SpaceCard {
        mid: dto.mid,
        name: dto.name,
        face: dto.face,
        sign: dto.sign,
        fans: dto.fans,
        attention: dto.attention,
        like_num: dto.likes.and_then(|l| l.like_num),
        level: level.current_level,
        level_identity: level.identity,
        pendant_image: non_empty(dto.pendant.and_then(|p| p.image)),
        official_type: official.kind,
        official_desc: official.desc,
        official_splice_title: official.splice_title,
        // `vip` 整块缺失与「status 为 0」在界面上同义，但快照要分得清，
        // 所以保留 `Option`，不在这里塌成 0。
        vip_status: dto.vip.as_ref().map(VipDto::status),
        vip_type: dto.vip.as_ref().and_then(VipDto::kind),
        vip_label_text: non_empty(dto.vip.and_then(|v| v.label).and_then(|l| l.text)),
        silence: dto.silence,
        relation: dto.relation.map(|r| SpaceRelation {
            status: r.status,
            is_follow: r.is_follow,
            is_followed: r.is_followed,
        }),
        live_medal: dto
            .live_fans_wearing
            .and_then(|w| w.detail_v2)
            .map(|m| SpaceLiveMedal {
                name: m.medal_name,
                level: m.level,
                color: m.medal_color,
                name_color: m.medal_color_name,
            }),
        tags: dto
            .space_tag
            .unwrap_or_default()
            .into_iter()
            // 基线只认这两类，其余丢掉。
            .filter(|t| matches!(t.kind.as_deref(), Some("location" | "real_name")))
            .map(|t| SpaceCardTag {
                title: t.title,
                uri: t.uri,
            })
            .collect(),
        pr_info: dto.pr_info.and_then(space_pr_info),
        followed_by: dto.followings_followed_upper.map(|u| SpaceFollowedBy {
            items: u
                .items
                .unwrap_or_default()
                .into_iter()
                .map(SpaceSimpleUserDto::into_domain)
                .collect(),
        }),
    }
}

/// `content` 为空就当整块不存在——基线也是只在它非空时才读其余字段。
fn space_pr_info(dto: SpacePrInfoDto) -> Option<SpacePrInfo> {
    let content = non_empty(dto.content)?;
    Some(SpacePrInfo {
        content: Some(content),
        url: dto.url,
        icon: dto.icon,
        icon_night: dto.icon_night,
        text_color: dto.text_color.unwrap_or_else(|| "#999999".to_owned()),
        bg_color: dto.bg_color.unwrap_or_else(|| "#e7e7e7".to_owned()),
        text_color_night: dto.text_color_night.unwrap_or_else(|| "#727272".to_owned()),
        bg_color_night: dto.bg_color_night.unwrap_or_else(|| "#2A2A2A".to_owned()),
    })
}

fn video_block(dto: SpaceVideoBlockDto) -> SpaceVideoBlock {
    SpaceVideoBlock {
        count: dto.count,
        items: dto
            .item
            .unwrap_or_default()
            .into_iter()
            .map(archive_item)
            .collect(),
    }
}

fn block_is_empty(block: Option<&SpaceVideoBlock>) -> bool {
    block.is_none_or(|b| b.items.is_empty())
}

/// 解析空间主页。
///
/// # Errors
/// 信封失败沿用统一分类。
#[allow(clippy::too_many_lines)]
pub fn parse_space_page(body: &str) -> Result<SpacePage, DomainError> {
    let dto: SpacePageDto = crate::envelope::unwrap_envelope(body)?;

    let archive = dto.archive.map(video_block);
    let season = dto.season.map(video_block);
    let coin_archive = dto.coin_archive.map(video_block);
    let like_archive = dto.like_archive.map(video_block);
    let comic = dto.comic.map(video_block);
    let article = dto.article.map(|b| SpaceArticleBlock {
        count: b.count,
        items: b
            .item
            .unwrap_or_default()
            .into_iter()
            .map(|a| SpaceArticleItem {
                title: a.title,
                cover: a.origin_image_urls.and_then(|urls| urls.into_iter().next()),
                publish_time_text: a.publish_time_text,
                view: a.stats.as_ref().and_then(|s| s.view),
                reply: a.stats.and_then(|s| s.reply),
                uri: a.uri,
            })
            .collect(),
    });
    let audio = dto.audios.map(|b| SpaceAudioBlock {
        count: b.count,
        items: b
            .item
            .unwrap_or_default()
            .into_iter()
            .map(|a| SpaceAudioItem {
                id: a.id,
                uid: a.uid,
                title: a.title,
                cover: a.cover,
                ctime: a.ctime,
            })
            .collect(),
    });
    let favourite = dto.favourite2.map(|b| SpaceFavBlock {
        count: b.count,
        items: b
            .item
            .unwrap_or_default()
            .into_iter()
            .map(|f| SpaceFavItem {
                media_id: f.media_id,
                title: f.title,
                cover: f.cover,
                count: f.count,
                is_public: f.is_public,
            })
            .collect(),
    });

    // 基线在 `SpaceData.fromJson` 末尾算这个值，八个块任一非空即为真。
    let has_item = ![
        block_is_empty(archive.as_ref()),
        block_is_empty(season.as_ref()),
        block_is_empty(coin_archive.as_ref()),
        block_is_empty(like_archive.as_ref()),
        block_is_empty(comic.as_ref()),
        article.as_ref().is_none_or(|b| b.items.is_empty()),
        audio.as_ref().is_none_or(|b| b.items.is_empty()),
        favourite.as_ref().is_none_or(|b| b.items.is_empty()),
    ]
    .iter()
    .all(|empty| *empty);

    let ugc_season = dto.ugc_season.unwrap_or_default();
    let series = dto.series.unwrap_or_default();
    let has_season_or_series = matches!(ugc_season.count, Some(c) if c != 0)
        || series.item.is_some_and(|items| !items.is_empty());

    Ok(SpacePage {
        relation: dto.relation,
        guest_relation: dto.guest_relation,
        rel_special: dto.rel_special,
        medal: dto.medal,
        default_tab: dto.default_tab,
        has_item,
        has_season_or_series,
        setting: dto.setting.map(|s| SpaceSetting {
            fav_video: s.fav_video,
            coins_video: s.coins_video,
            likes_video: s.likes_video,
            bangumi: s.bangumi,
        }),
        card: dto.card.map(space_card),
        images: dto.images.map(|i| SpaceImages {
            img_url: non_empty(i.img_url),
            night_img_url: non_empty(i.night_imgurl),
            top_images: i
                .collection_top_simple
                .and_then(|c| c.top)
                .and_then(|t| t.result)
                .unwrap_or_default()
                .into_iter()
                .filter_map(top_image)
                .collect(),
        }),
        live: dto.live.map(|l| SpaceLive {
            room_status: l.room_status,
            live_status: l.live_status,
            room_id: l.roomid,
        }),
        elec: dto.elec.map(|e| SpaceElec {
            total: e.total,
            list: e
                .list
                .unwrap_or_default()
                .into_iter()
                .map(SpaceSimpleUserDto::into_domain_anonymous)
                .collect(),
        }),
        guard: dto.guard.map(|g| SpaceGuard {
            count: guard_count(g.desc.as_deref()),
            items: g
                .item
                .unwrap_or_default()
                .into_iter()
                .map(SpaceSimpleUserDto::into_domain)
                .collect(),
        }),
        archive,
        season,
        coin_archive,
        like_archive,
        comic,
        article,
        audio,
        favourite,
        tab2: dto
            .tab2
            .unwrap_or_default()
            .into_iter()
            .map(|t| SpaceTab2 {
                title: t.title,
                param: t.param,
                items: t
                    .items
                    .unwrap_or_default()
                    .into_iter()
                    .map(|i| SpaceTab2Item {
                        title: i.title,
                        param: i.param,
                        filters: i
                            .filter
                            .unwrap_or_default()
                            .into_iter()
                            .map(|f| SpaceTabFilter {
                                text: f.text,
                                meta: f.meta.unwrap_or_else(|| "all".to_owned()),
                                tab_name: f.tab_name,
                            })
                            .collect(),
                        season_id: i.season_id,
                        series_id: i.series_id,
                    })
                    .collect(),
            })
            .collect(),
        reservations: dto
            .reservation_card_list
            .unwrap_or_default()
            .into_iter()
            .map(|r| SpaceReservation {
                sid: r.sid,
                name: r.name,
                total: r.total.unwrap_or(0),
                is_follow: r.is_follow == Some(1),
                desc_text: non_empty(r.desc_text_1.and_then(|d| d.text)),
                dynamic_id: r.dynamic_id,
                lottery: r.lottery_prize_info.map(|l| SpaceReservationLottery {
                    text: l.text,
                    jump_url: l.jump_url,
                }),
            })
            .collect(),
    })
}

/// 取空间主页。
///
/// # Errors
/// 同 [`fetch_member_info`]。
pub async fn fetch_space_page<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    mid: i64,
    from_view_aid: Option<&str>,
) -> Result<SpacePage, DomainError> {
    let req = space_request(mid, from_view_aid);
    let body = crate::client::send(transport, ctx, &req).await?;
    parse_space_page(&body)
}

// -------------------------------------------------------------- 投稿列表

/// 投稿列表的请求。
///
/// 六个 tab 共用这一个构造函数：参数集合是一样的，差别只在路径与几个
/// 按 tab 给不给的值，拆六份只会得到六段同样的代码。
///
/// `qn` 只有视频那条给 80，其余 32——这是封面分辨率，基线写死的。
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn space_archive_request(
    kind: SpaceArchiveKind,
    mid: Option<i64>,
    aid: Option<&str>,
    order: Option<SpaceArchiveOrder>,
    sort: Option<SpaceArchiveSort>,
    page: Option<i64>,
    next: Option<i64>,
    season_id: Option<i64>,
    series_id: Option<i64>,
    include_cursor: Option<bool>,
) -> PreparedRequest {
    let mut query = Vec::new();
    if let Some(aid) = aid {
        query.push(("aid".to_owned(), aid.to_owned()));
    }
    query.extend(app_common_query().into_iter().filter(|(k, _)| {
        // 基线这条的参数顺序是 aid、固定参数、ps…，`statistics` 排在最后，
        // 所以单独加。
        k != "statistics"
    }));
    query.push(("ps".to_owned(), "20".to_owned()));
    if let Some(page) = page {
        query.push(("pn".to_owned(), page.to_string()));
    }
    if let Some(next) = next {
        query.push(("next".to_owned(), next.to_string()));
    }
    if let Some(id) = season_id {
        query.push(("season_id".to_owned(), id.to_string()));
    }
    if let Some(id) = series_id {
        query.push(("series_id".to_owned(), id.to_string()));
    }
    query.push(("qn".to_owned(), kind.cover_quality().to_string()));
    if let Some(order) = order {
        query.push(("order".to_owned(), order.as_param().to_owned()));
    }
    if let Some(sort) = sort {
        query.push(("sort".to_owned(), sort.as_param().to_owned()));
    }
    if let Some(include_cursor) = include_cursor {
        query.push(("include_cursor".to_owned(), include_cursor.to_string()));
    }
    query.push(("statistics".to_owned(), app_fixed::STATISTICS.to_owned()));
    // 基线对 `vmid` 不判空，`null` 原样进查询串（会被编码成空值）。
    query.push((
        "vmid".to_owned(),
        mid.map_or_else(String::new, |m| m.to_string()),
    ));
    PreparedRequest {
        path: kind.path(),
        query,
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SpaceArchivePageDto {
    count: Option<i64>,
    item: Option<Vec<SpaceArchiveItemDto>>,
    episodic_button: Option<SpaceEpisodicButtonDto>,
    has_next: Option<bool>,
    has_prev: Option<bool>,
    next: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SpaceEpisodicButtonDto {
    text: Option<String>,
    uri: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SpaceArchiveItemDto {
    title: Option<String>,
    cover: Option<String>,
    bvid: Option<String>,
    first_cid: Option<i64>,
    param: Option<String>,
    goto: Option<String>,
    uri: Option<String>,
    duration: Option<i64>,
    play: Option<i64>,
    danmaku: Option<i64>,
    author: Option<String>,
    publish_time_text: Option<String>,
    badges: Option<Vec<SpaceBadgeDto>>,
    is_steins: Option<bool>,
    is_cooperation: Option<bool>,
    is_pgc: Option<bool>,
    is_pugv: Option<bool>,
    history: Option<SpaceArchiveHistoryDto>,
    season: Option<SpaceArchiveSeasonDto>,
    styles: Option<String>,
    label: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SpaceBadgeDto {
    text: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SpaceArchiveHistoryDto {
    progress: Option<i64>,
    duration: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SpaceArchiveSeasonDto {
    mtime: Option<i64>,
}

fn archive_item(dto: SpaceArchiveItemDto) -> SpaceArchiveItem {
    SpaceArchiveItem {
        title: dto.title,
        cover: dto.cover,
        bvid: dto.bvid,
        cid: dto.first_cid,
        param: dto.param,
        goto: dto.goto,
        uri: dto.uri,
        // 基线写的是 `json['duration'] ?? -1`，界面据此判断显不显示时长。
        duration: dto.duration.unwrap_or(-1),
        play: dto.play,
        danmaku: dto.danmaku,
        author: dto.author,
        publish_time_text: dto.publish_time_text,
        badges: dto
            .badges
            .unwrap_or_default()
            .into_iter()
            // 基线把 `text` 为空的角标渲染成空串再 join，留着只会多出
            // 一个 `|`。没有文案的角标画不出来，直接丢。
            .filter_map(|b| non_empty(b.text))
            .collect(),
        is_steins: dto.is_steins,
        is_cooperation: dto.is_cooperation,
        is_pgc: dto.is_pgc,
        is_pugv: dto.is_pugv,
        history: dto.history.map(|h| SpaceArchiveHistory {
            progress: h.progress,
            duration: h.duration,
        }),
        season_mtime: dto.season.and_then(|s| s.mtime),
        styles: dto.styles,
        label: dto.label,
    }
}

/// 解析一页投稿。
///
/// # Errors
/// 信封失败沿用统一分类。
pub fn parse_space_archive(body: &str) -> Result<SpaceArchivePage, DomainError> {
    let dto: SpaceArchivePageDto = crate::envelope::unwrap_envelope(body)?;
    Ok(SpaceArchivePage {
        count: dto.count,
        items: dto
            .item
            .unwrap_or_default()
            .into_iter()
            .map(archive_item)
            .collect(),
        episodic_button: dto.episodic_button.map(|b| SpaceEpisodicButton {
            text: b.text,
            uri: b.uri,
        }),
        has_next: dto.has_next,
        has_prev: dto.has_prev,
        next: dto.next,
    })
}

/// 取一页投稿。
///
/// # Errors
/// 同 [`fetch_member_info`]。
#[allow(clippy::too_many_arguments)]
pub async fn fetch_space_archive<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    kind: SpaceArchiveKind,
    mid: Option<i64>,
    aid: Option<&str>,
    order: Option<SpaceArchiveOrder>,
    sort: Option<SpaceArchiveSort>,
    page: Option<i64>,
    next: Option<i64>,
    season_id: Option<i64>,
    series_id: Option<i64>,
    include_cursor: Option<bool>,
) -> Result<SpaceArchivePage, DomainError> {
    let req = space_archive_request(
        kind,
        mid,
        aid,
        order,
        sort,
        page,
        next,
        season_id,
        series_id,
        include_cursor,
    );
    let body = crate::client::send(transport, ctx, &req).await?;
    parse_space_archive(&body)
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::{
        coin_like_arc_request, member_card_request, member_info_request, parse_coin_like_arc,
        parse_member_card, parse_member_guard, parse_member_info, parse_member_up_stat,
        parse_report_result, parse_season_series, parse_upower_rank, report_member_request,
        upower_rank_request,
    };

    use crate::http::Idempotency;
    use crate::request::{CsrfIn, WriteBody};
    use bili_protocol::risk::RiskTokens;

    fn tokens() -> RiskTokens {
        RiskTokens {
            dm_img_str: "AAAA".to_owned(),
            dm_cover_img_str: "BBBB".to_owned(),
        }
    }

    #[test]
    fn an_empty_token_is_still_sent_because_it_is_signed() {
        let q = member_info_request(1, "", &tokens(), "key", 1).query_string();
        assert!(
            q.contains("token=&"),
            "空 token 省掉会换一个不同的 w_rid — {q}"
        );
    }

    #[test]
    fn the_vip_status_falls_back_to_the_alternate_key() {
        let info =
            parse_member_info(r#"{"code":0,"data":{"mid":1,"vip":{"vipStatus":1,"vipType":2}}}"#)
                .expect("应解析成功");
        assert_eq!(info.vip_status, Some(1), "status 缺失时认 vipStatus");
        assert_eq!(info.vip_type, Some(2), "type 缺失时认 vipType");
    }

    #[test]
    fn a_missing_vip_block_is_not_a_zero_status() {
        let info = parse_member_info(r#"{"code":0,"data":{"mid":1}}"#).expect("应解析成功");
        assert_eq!(info.vip_status, None, "整块 vip 都没有时是 null，不是 0");
    }

    #[test]
    fn the_card_official_key_is_capitalised() {
        let card = parse_member_card(r#"{"code":0,"data":{"card":{"Official":{"type":1}}}}"#)
            .expect("应解析成功");
        assert_eq!(card.official_type, Some(1), "这条接口的键是 `Official`");
    }

    #[test]
    fn the_card_mid_stays_a_string() {
        let card =
            parse_member_card(r#"{"code":0,"data":{"card":{"mid":"123"}}}"#).expect("应解析成功");
        assert_eq!(card.mid.as_deref(), Some("123"), "服务端给的就是字符串");
    }

    #[test]
    fn the_card_request_asks_for_no_photo() {
        let q = member_card_request(7).query_string();
        assert_eq!(q, "mid=7&photo=false", "要的只是昵称与计数 — {q}");
    }

    #[test]
    fn up_stat_views_come_from_two_nested_blocks() {
        let stat = parse_member_up_stat(
            r#"{"code":0,"data":{"archive":{"view":10},"article":{"view":2},"likes":3}}"#,
        )
        .expect("应解析成功");
        assert_eq!(stat.archive_view, Some(10));
        assert_eq!(stat.article_view, Some(2));
        assert_eq!(stat.likes, Some(3));
    }

    #[test]
    fn reporting_is_unsafe_to_resend() {
        let w = report_member_request(1, "spam", None);
        assert_eq!(w.idempotency, Idempotency::Unsafe, "重发会重复提交举报工单");
        assert_eq!(w.csrf, CsrfIn::Body, "csrf 在表单里");
    }

    #[test]
    fn an_absent_reason_v2_is_omitted_entirely() {
        let WriteBody::Form(body) = report_member_request(1, "spam", None).body else {
            panic!("举报是表单体");
        };
        assert!(
            !body.iter().any(|(k, _)| k == "reason_v2"),
            "基线用 `?reasonV2`，null 时整条不发"
        );
    }

    #[test]
    fn a_report_without_a_status_field_counts_as_failed() {
        assert!(
            !parse_report_result("{}").expect("空对象是合法 JSON"),
            "没给 status 就不能当成功"
        );
        assert!(parse_report_result(r#"{"status":true}"#).expect("应解析成功"));
    }

    #[test]
    fn the_payload_hides_one_level_down_under_items_lists() {
        let page = parse_season_series(
            r#"{"code":0,"data":{"items_lists":{"page":{"total":3},
               "seasons_list":[{"meta":{"season_id":1,"name":"甲"}}],
               "series_list":[{"meta":{"series_id":2,"name":"乙"}}]}}}"#,
        )
        .expect("应解析成功");
        assert_eq!(page.total, Some(3));
        assert_eq!(page.seasons.len(), 1, "合集与列表分开放");
        assert_eq!(page.series.len(), 1);
        assert_eq!(page.seasons[0].season_id, Some(1));
        assert_eq!(page.series[0].series_id, Some(2));
    }

    #[test]
    fn a_missing_items_lists_is_an_empty_page_not_an_error() {
        let page = parse_season_series(r#"{"code":0,"data":{}}"#).expect("应解析成功");
        assert!(page.seasons.is_empty() && page.series.is_empty());
        assert_eq!(page.total, None);
    }

    #[test]
    fn coin_and_like_differ_only_in_the_path() {
        let coin = coin_like_arc_request(true, 1, 2, 20);
        let like = coin_like_arc_request(false, 1, 2, 20);
        assert_ne!(coin.path, like.path, "路径不同");
        assert_eq!(coin.query, like.query, "参数完全一样，所以共用一个构造函数");
    }

    #[test]
    fn the_arc_param_stays_a_string() {
        let page = parse_coin_like_arc(
            r#"{"code":0,"data":{"count":1,"item":[{"param":"123","title":"甲"}]}}"#,
        )
        .expect("应解析成功");
        assert_eq!(page.items[0].param.as_deref(), Some("123"));
    }

    #[test]
    fn a_guard_entry_missing_fields_does_not_fail_the_page() {
        let page = parse_member_guard(
            r#"{"code":0,"data":{"has_more":1,"guard_top_list":[{"uid":1},{}]}}"#,
        )
        .expect("基线这里会抛，我们不复刻");
        assert_eq!(page.items.len(), 2);
        assert_eq!(page.items[1].username, None);
        assert_eq!(page.has_more, Some(1), "服务端给的是整数不是布尔");
    }

    #[test]
    fn an_absent_privilege_type_is_omitted_entirely() {
        let q = upower_rank_request(7, 1, 100, None).query_string();
        assert!(
            !q.contains("privilege_type"),
            "基线用 `?privilegeType`，null 时整条不发 — {q}"
        );
        let q = upower_rank_request(7, 1, 100, Some(2)).query_string();
        assert!(q.contains("privilege_type=2"), "给了就要发 — {q}");
    }

    #[test]
    fn upower_tabs_and_levels_default_to_empty() {
        let rank =
            parse_upower_rank(r#"{"code":0,"data":{"privilege_type":1}}"#).expect("应解析成功");
        assert!(rank.items.is_empty() && rank.tabs.is_empty() && rank.levels.is_empty());
        assert_eq!(rank.privilege_type, Some(1));
    }
}

// ------------------------------------------------- 合集与列表、充电、大航海

/// 「合集和列表」一页的请求。
///
/// `page_size` 基线写死 10。
#[must_use]
pub fn season_series_request(mid: i64, page: i64, page_size: i64) -> PreparedRequest {
    PreparedRequest {
        path: path::SEASON_SERIES,
        query: vec![
            ("mid".to_owned(), mid.to_string()),
            ("page_num".to_owned(), page.to_string()),
            ("page_size".to_owned(), page_size.to_string()),
        ],
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SeasonSeriesEnvelopeDto {
    items_lists: Option<SeasonSeriesListsDto>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SeasonSeriesListsDto {
    page: Option<SeasonSeriesPageDto>,
    seasons_list: Option<Vec<SeasonSeriesItemDto>>,
    series_list: Option<Vec<SeasonSeriesItemDto>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SeasonSeriesPageDto {
    total: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SeasonSeriesItemDto {
    meta: Option<SeasonSeriesMetaDto>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SeasonSeriesMetaDto {
    cover: Option<String>,
    name: Option<String>,
    ptime: Option<i64>,
    total: Option<i64>,
    season_id: Option<i64>,
    series_id: Option<i64>,
}

fn season_series_item(dto: SeasonSeriesItemDto) -> SpaceSeasonSeries {
    let meta = dto.meta.unwrap_or_default();
    SpaceSeasonSeries {
        cover: meta.cover,
        name: meta.name,
        ptime: meta.ptime,
        total: meta.total,
        season_id: meta.season_id,
        series_id: meta.series_id,
    }
}

/// 解析一页「合集和列表」。
///
/// # Errors
/// 信封失败沿用统一分类。
///
/// 真正的载荷藏在 `data.items_lists` 里，基线用的是
/// `res.data['data']?['items_lists'] ?? {}`——整块缺失按空处理，不是错误。
pub fn parse_season_series(body: &str) -> Result<SpaceSeasonSeriesPage, DomainError> {
    let dto: SeasonSeriesEnvelopeDto = crate::envelope::unwrap_envelope(body)?;
    let lists = dto.items_lists.unwrap_or_default();
    Ok(SpaceSeasonSeriesPage {
        total: lists.page.and_then(|p| p.total),
        seasons: lists
            .seasons_list
            .unwrap_or_default()
            .into_iter()
            .map(season_series_item)
            .collect(),
        series: lists
            .series_list
            .unwrap_or_default()
            .into_iter()
            .map(season_series_item)
            .collect(),
    })
}

/// 取一页「合集和列表」。
///
/// # Errors
/// 同 [`fetch_member_info`]。
pub async fn fetch_season_series<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    mid: i64,
    page: i64,
    page_size: i64,
) -> Result<SpaceSeasonSeriesPage, DomainError> {
    let req = season_series_request(mid, page, page_size);
    let body = crate::client::send(transport, ctx, &req).await?;
    parse_season_series(&body)
}

/// 投币过/点赞过的稿件，一页。
///
/// `coin` 为 true 取投币列表，否则取点赞列表——两条接口的参数与响应形状
/// 完全一样，只换路径，拆成两个函数只会得到两份同样的代码。
#[must_use]
pub fn coin_like_arc_request(coin: bool, mid: i64, page: i64, page_size: i64) -> PreparedRequest {
    PreparedRequest {
        path: if coin { path::COIN_ARC } else { path::LIKE_ARC },
        query: vec![
            ("pn".to_owned(), page.to_string()),
            ("ps".to_owned(), page_size.to_string()),
            ("vmid".to_owned(), mid.to_string()),
        ],
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct CoinLikeArcPageDto {
    count: Option<i64>,
    item: Option<Vec<CoinLikeArcItemDto>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct CoinLikeArcItemDto {
    title: Option<String>,
    cover: Option<String>,
    uri: Option<String>,
    param: Option<String>,
    duration: Option<i64>,
    is_steins: Option<bool>,
    is_cooperation: Option<bool>,
    is_pgc: Option<bool>,
    play: Option<i64>,
    danmaku: Option<i64>,
    ctime: Option<i64>,
}

/// 解析一页投币/点赞列表。
///
/// # Errors
/// 信封失败沿用统一分类。`item` 缺失按空列表处理。
pub fn parse_coin_like_arc(body: &str) -> Result<CoinLikeArcPage, DomainError> {
    let dto: CoinLikeArcPageDto = crate::envelope::unwrap_envelope(body)?;
    Ok(CoinLikeArcPage {
        count: dto.count,
        items: dto
            .item
            .unwrap_or_default()
            .into_iter()
            .map(|i| CoinLikeArcItem {
                title: i.title,
                cover: i.cover,
                uri: i.uri,
                param: i.param,
                duration: i.duration,
                is_steins: i.is_steins,
                is_cooperation: i.is_cooperation,
                is_pgc: i.is_pgc,
                play: i.play,
                danmaku: i.danmaku,
                ctime: i.ctime,
            })
            .collect(),
    })
}

/// 取一页投币/点赞列表。
///
/// # Errors
/// 同 [`fetch_member_info`]。
pub async fn fetch_coin_like_arc<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    coin: bool,
    mid: i64,
    page: i64,
    page_size: i64,
) -> Result<CoinLikeArcPage, DomainError> {
    let req = coin_like_arc_request(coin, mid, page, page_size);
    let body = crate::client::send(transport, ctx, &req).await?;
    parse_coin_like_arc(&body)
}

/// 大航海名单一页的请求。
#[must_use]
pub fn member_guard_request(ruid: i64, page: i64, page_size: i64) -> PreparedRequest {
    PreparedRequest {
        path: path::MEMBER_GUARD,
        query: vec![
            ("page".to_owned(), page.to_string()),
            ("page_size".to_owned(), page_size.to_string()),
            ("ruid".to_owned(), ruid.to_string()),
        ],
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct MemberGuardPageDto {
    guard_top_list: Option<Vec<GuardItemDto>>,
    has_more: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct GuardItemDto {
    uid: Option<i64>,
    username: Option<String>,
    face: Option<String>,
    guard_level: Option<i64>,
}

/// 解析一页大航海名单。
///
/// # Errors
/// 信封失败沿用统一分类。
///
/// 基线把这四个字段都声明成非空（`int uid` 而不是 `int?`），服务端少给一个
/// 就直接抛。这里不复刻那个崩法：少一个字段显示不全，不该让整页打不开。
pub fn parse_member_guard(body: &str) -> Result<MemberGuardPage, DomainError> {
    let dto: MemberGuardPageDto = crate::envelope::unwrap_envelope(body)?;
    Ok(MemberGuardPage {
        has_more: dto.has_more,
        items: dto
            .guard_top_list
            .unwrap_or_default()
            .into_iter()
            .map(|g| GuardItem {
                uid: g.uid,
                username: g.username,
                face: g.face,
                guard_level: g.guard_level,
            })
            .collect(),
    })
}

/// 取一页大航海名单。
///
/// # Errors
/// 同 [`fetch_member_info`]。
pub async fn fetch_member_guard<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    ruid: i64,
    page: i64,
    page_size: i64,
) -> Result<MemberGuardPage, DomainError> {
    let req = member_guard_request(ruid, page, page_size);
    let body = crate::client::send(transport, ctx, &req).await?;
    parse_member_guard(&body)
}

/// 充电排行一页的请求。
///
/// `privilege_type` 省掉表示「全部档位」，此时响应里才带 `level_info`
/// ——界面靠它决定要不要显示档位 tab 条。
///
/// `csrf` 走 [`PreparedRequest`] 不合适（这是读接口），基线在**未登录时
/// 整条不发**；登录态由 [`RequestContext`] 的 cookie 表达，这里不再重复。
#[must_use]
pub fn upower_rank_request(
    up_mid: i64,
    page: i64,
    page_size: i64,
    privilege_type: Option<i64>,
) -> PreparedRequest {
    let mut query = vec![
        ("up_mid".to_owned(), up_mid.to_string()),
        ("pn".to_owned(), page.to_string()),
        ("ps".to_owned(), page_size.to_string()),
    ];
    if let Some(t) = privilege_type {
        query.push(("privilege_type".to_owned(), t.to_string()));
    }
    query.push(("mobi_app".to_owned(), "web".to_owned()));
    query.push(("web_location".to_owned(), "333.1196".to_owned()));
    PreparedRequest {
        path: path::UPOWER_RANK,
        query,
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct UpowerRankDto {
    rank_info: Option<Vec<UpowerRankItemDto>>,
    privilege_type: Option<i64>,
    tabs: Option<Vec<i64>>,
    level_info: Option<Vec<UpowerLevelDto>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct UpowerRankItemDto {
    mid: Option<i64>,
    nickname: Option<String>,
    avatar: Option<String>,
    day: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct UpowerLevelDto {
    privilege_type: Option<i64>,
    name: Option<String>,
    member_total: Option<i64>,
}

/// 解析充电排行。
///
/// # Errors
/// 信封失败沿用统一分类。
pub fn parse_upower_rank(body: &str) -> Result<UpowerRank, DomainError> {
    let dto: UpowerRankDto = crate::envelope::unwrap_envelope(body)?;
    Ok(UpowerRank {
        items: dto
            .rank_info
            .unwrap_or_default()
            .into_iter()
            .map(|r| UpowerRankItem {
                mid: r.mid,
                nickname: r.nickname,
                avatar: r.avatar,
                day: r.day,
            })
            .collect(),
        privilege_type: dto.privilege_type,
        tabs: dto.tabs.unwrap_or_default(),
        levels: dto
            .level_info
            .unwrap_or_default()
            .into_iter()
            .map(|l| UpowerLevel {
                privilege_type: l.privilege_type,
                name: l.name,
                member_total: l.member_total,
            })
            .collect(),
    })
}

/// 取充电排行。
///
/// # Errors
/// 同 [`fetch_member_info`]。
pub async fn fetch_upower_rank<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    up_mid: i64,
    page: i64,
    page_size: i64,
    privilege_type: Option<i64>,
) -> Result<UpowerRank, DomainError> {
    let req = upower_rank_request(up_mid, page, page_size, privilege_type);
    let body = crate::client::send(transport, ctx, &req).await?;
    parse_upower_rank(&body)
}
