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
    SpaceArchiveSearchPage, SpaceArchiveTag, SpaceArticleBlock, SpaceArticleItem, SpaceArticlePage,
    SpaceAudioBlock, SpaceAudioItem, SpaceAudioPage, SpaceCard, SpaceCardTag, SpaceCheeseItem,
    SpaceCheesePage, SpaceElec, SpaceEpisodicButton, SpaceFavBlock, SpaceFavItem, SpaceFollowedBy,
    SpaceGuard, SpaceImages, SpaceLive, SpaceLiveMedal, SpacePage, SpacePrInfo, SpaceRelation,
    SpaceReservation, SpaceReservationLottery, SpaceSeasonSeries, SpaceSeasonSeriesPage,
    SpaceSeasonWebPage, SpaceSetting, SpaceShopItem, SpaceShopPage, SpaceSimpleUser, SpaceTab2,
    SpaceTab2Item, SpaceTabFilter, SpaceTopImage, SpaceTopTitle, SpaceVideoBlock, SpaceWebBadge,
    SpaceWebVideo, UpowerLevel, UpowerRank, UpowerRankItem,
};
use bili_protocol::risk::RiskTokens;
use serde::Deserialize;

use crate::http::{HttpTransport, Idempotency, RequestContext};
use crate::request::{CsrfIn, PreparedRequest, PreparedWrite, WriteBody};

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

    /// 专栏列表。**挂在 `app.bilibili.com` 上**，base_url 由调用方给。
    pub const SPACE_ARTICLE: &str = "/x/v2/space/article";
    /// 音频列表。
    pub const SPACE_AUDIO: &str = "/audio/music-service/web/song/upper";
    /// 课堂列表。
    pub const SPACE_CHEESE: &str = "/pugv/app/web/season/page";
    /// 会员购小店。**挂在 `mall.bilibili.com` 上**，base_url 由调用方给。
    pub const SPACE_SHOP: &str = "/community-hub/small_shop/feed/tab/item";

    /// 空间内的投稿搜索（网页端）。WBI 签名。
    pub const SEARCH_ARCHIVE: &str = "/x/space/wbi/arc/search";
    /// 合集里的视频（网页端）。
    pub const SEASON_ARCHIVES: &str = "/x/polymer/web-space/seasons_archives_list";
    /// 列表里的视频（网页端）。
    pub const SERIES_ARCHIVES: &str = "/x/series/archives";
}

/// `web_location`：空间页的页面来源标记，服务端据此分流风控策略。
/// 与别处的 `333.1387`（空间内各 tab）不是一回事，不要合并。
const WEB_LOCATION_SPACE: &str = "1550101";

/// `web_location`：空间内各 tab 的页面来源标记。与 [`WEB_LOCATION_SPACE`]
/// 不是一回事。
const WEB_LOCATION_SPACE_TAB: &str = "333.1387";

/// 课堂分区号。基线只把分区表里的这一条当成标签，其余分区全丢。
const LESSON_TID: &str = "196";

/// 会员购自己的版本号。与 app 域那组常量不是一套，不要合并。
const MALL_M_VERSION: &str = "309";

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
/// APP 签名是当前网关的必需项，缺失时连匿名请求也返回 -400（#260）。
/// 时间由调用方注入，保证签名可复现。
#[must_use]
pub fn space_request(
    mid: i64,
    from_view_aid: Option<&str>,
    appkey: &str,
    appsec: &str,
    ts: i64,
) -> PreparedRequest {
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
        query: bili_protocol::app_sign::sign(&query, appkey, appsec, ts),
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

/// 专栏条目的映射。主页那一块与专栏列表页共用：同一个界面组件，
/// 服务端给的也是同一组键。
fn article_item(a: SpaceArticleItemDto) -> SpaceArticleItem {
    SpaceArticleItem {
        title: a.title,
        // 基线留了整个 `origin_image_urls`，但组件只读第一张。
        cover: a.origin_image_urls.and_then(|urls| urls.into_iter().next()),
        publish_time_text: a.publish_time_text,
        view: a.stats.as_ref().and_then(|s| s.view),
        reply: a.stats.and_then(|s| s.reply),
        uri: a.uri,
    }
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
            .map(article_item)
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
                // 主页这一块给的是秒，不除 1000；也不给播放与评论数。
                ctime: a.ctime,
                play: None,
                comment: None,
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
    appkey: &str,
    appsec: &str,
    ts: i64,
) -> Result<SpacePage, DomainError> {
    let req = space_request(mid, from_view_aid, appkey, appsec, ts);
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

// --------------------------------------------- 网页端的投稿搜索与合集

/// 投稿搜索的排序字段。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpaceArchiveSearchOrder {
    /// 最新发布。
    Pubdate,
    /// 最多播放。
    Click,
    /// 最多收藏。
    Stow,
}

impl SpaceArchiveSearchOrder {
    #[must_use]
    pub const fn as_param(self) -> &'static str {
        match self {
            Self::Pubdate => "pubdate",
            Self::Click => "click",
            Self::Stow => "stow",
        }
    }
}

/// 合集与列表是两条接口，参数名也不一样。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpaceSeasonWebKind {
    Season,
    Series,
}

/// 投稿搜索的请求。WBI 签名 + 浏览器 UA。
///
/// `order_avoided=true` 与那串 `dm_*` 是基线写死的风控参数，照抄。
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn space_archive_search_request(
    mid: i64,
    tid: i64,
    page: i64,
    page_size: i64,
    keyword: Option<&str>,
    special_type: Option<&str>,
    order: SpaceArchiveSearchOrder,
    tokens: &RiskTokens,
    mixin_key: &str,
    wts: i64,
) -> PreparedRequest {
    let mut query = vec![
        ("mid".to_owned(), mid.to_string()),
        ("ps".to_owned(), page_size.to_string()),
        ("tid".to_owned(), tid.to_string()),
        ("pn".to_owned(), page.to_string()),
    ];
    if let Some(keyword) = keyword {
        query.push(("keyword".to_owned(), keyword.to_owned()));
    }
    if let Some(special_type) = special_type {
        query.push(("special_type".to_owned(), special_type.to_owned()));
    }
    query.extend([
        ("order".to_owned(), order.as_param().to_owned()),
        ("platform".to_owned(), "web".to_owned()),
        ("web_location".to_owned(), WEB_LOCATION_SPACE_TAB.to_owned()),
        ("order_avoided".to_owned(), "true".to_owned()),
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
    ]);
    query.sort_by(|a, b| a.0.cmp(&b.0));
    let w_rid = bili_protocol::wbi::sign(&query, mixin_key);
    query.push(("w_rid".to_owned(), w_rid));
    PreparedRequest {
        path: path::SEARCH_ARCHIVE,
        query,
    }
}

/// 合集 / 列表的请求。
///
/// 两条接口的参数名不同（`page_num`/`page_size` 与 `pn`/`ps`），排序的
/// 表达方式也不同（一个是布尔 `sort_reverse`、一个是字符串 `sort`），
/// 所以这里按 [`SpaceSeasonWebKind`] 分两支，不强行合并。
#[must_use]
pub fn space_season_web_request(
    kind: SpaceSeasonWebKind,
    mid: i64,
    id: i64,
    page: i64,
    page_size: i64,
    ascending: bool,
) -> PreparedRequest {
    let query = match kind {
        SpaceSeasonWebKind::Season => vec![
            ("mid".to_owned(), mid.to_string()),
            ("season_id".to_owned(), id.to_string()),
            ("sort_reverse".to_owned(), ascending.to_string()),
            ("page_size".to_owned(), page_size.to_string()),
            ("page_num".to_owned(), page.to_string()),
            ("web_location".to_owned(), WEB_LOCATION_SPACE_TAB.to_owned()),
        ],
        SpaceSeasonWebKind::Series => vec![
            ("mid".to_owned(), mid.to_string()),
            ("series_id".to_owned(), id.to_string()),
            (
                "sort".to_owned(),
                if ascending { "asc" } else { "desc" }.to_owned(),
            ),
            ("ps".to_owned(), page_size.to_string()),
            ("pn".to_owned(), page.to_string()),
            ("web_location".to_owned(), WEB_LOCATION_SPACE_TAB.to_owned()),
        ],
    };
    PreparedRequest {
        path: match kind {
            SpaceSeasonWebKind::Season => path::SEASON_ARCHIVES,
            SpaceSeasonWebKind::Series => path::SERIES_ARCHIVES,
        },
        query,
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ArchiveSearchDto {
    list: Option<ArchiveSearchListDto>,
    page: Option<ArchiveSearchPageDto>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ArchiveSearchPageDto {
    count: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ArchiveSearchListDto {
    vlist: Option<Vec<VlistItemDto>>,
    slist: Option<Vec<SpaceArchiveTagDto>>,
    tlist: Option<std::collections::BTreeMap<String, SpaceArchiveTagDto>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SpaceArchiveTagDto {
    tid: Option<i64>,
    count: Option<i64>,
    name: Option<String>,
    special_type: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct VlistItemDto {
    aid: Option<i64>,
    bvid: Option<String>,
    pic: Option<String>,
    title: Option<String>,
    description: Option<String>,
    created: Option<i64>,
    /// `"12:34"` 这种字符串。
    length: Option<String>,
    play: Option<i64>,
    video_review: Option<i64>,
    mid: Option<i64>,
    author: Option<String>,
    season_id: Option<i64>,
    jump_url: Option<String>,
    is_lesson_video: Option<i64>,
    is_charging_arc: Option<bool>,
    is_union_video: Option<i64>,
    /// 百分比。
    playback_position: Option<f64>,
}

/// 看到哪儿了。
///
/// 服务端给的是百分比，基线把 100% 记成 `-1`（界面据此显示「已看完」而
/// 不是一条满格进度条），其余按时长折算并四舍五入。照抄。
fn web_progress(position: Option<f64>, duration: Option<i64>) -> Option<i64> {
    let position = position?;
    if (position - 100.0).abs() < f64::EPSILON {
        return Some(-1);
    }
    #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
    Some((position / 100.0 * duration.unwrap_or(0) as f64).round() as i64)
}

fn vlist_item(dto: VlistItemDto) -> SpaceWebVideo {
    let duration = dto
        .length
        .as_deref()
        .map(crate::search_text::parse_duration_secs);
    SpaceWebVideo {
        aid: dto.aid,
        bvid: dto.bvid,
        cover: dto.pic,
        title: dto.title,
        desc: dto.description,
        pubdate: dto.created,
        duration,
        view: dto.play,
        danmaku: dto.video_review,
        owner_mid: dto.mid,
        owner_name: dto.author,
        season_id: dto.season_id,
        redirect_url: dto.jump_url,
        // 三支互斥，顺序照抄基线：课堂 > 充电专属 > 合作。
        badge: if dto.is_lesson_video == Some(1) {
            Some(SpaceWebBadge::Lesson)
        } else if dto.is_charging_arc == Some(true) {
            Some(SpaceWebBadge::Charging)
        } else if dto.is_union_video == Some(1) {
            Some(SpaceWebBadge::Union)
        } else {
            None
        },
        progress: web_progress(dto.playback_position, duration),
    }
}

/// 解析一页投稿搜索。
///
/// # Errors
/// 信封失败沿用统一分类。
pub fn parse_space_archive_search(body: &str) -> Result<SpaceArchiveSearchPage, DomainError> {
    let dto: ArchiveSearchDto = crate::envelope::unwrap_envelope(body)?;
    let list = dto.list.unwrap_or_default();
    let mut tags: Vec<SpaceArchiveTag> = list
        .slist
        .unwrap_or_default()
        .into_iter()
        .map(|t| SpaceArchiveTag {
            tid: t.tid,
            count: t.count,
            name: t.name,
            special_type: t.special_type,
        })
        .collect();
    // 基线只从分区表里挑课堂那一条（`196`），其余分区不进标签条。
    if let Some(lesson) = list.tlist.unwrap_or_default().remove(LESSON_TID) {
        tags.push(SpaceArchiveTag {
            tid: lesson.tid,
            count: lesson.count,
            name: lesson.name,
            special_type: lesson.special_type,
        });
    }
    Ok(SpaceArchiveSearchPage {
        count: dto.page.and_then(|p| p.count),
        tags,
        items: list
            .vlist
            .unwrap_or_default()
            .into_iter()
            .map(vlist_item)
            .collect(),
    })
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SeasonWebDto {
    archives: Option<Vec<SeasonWebArchiveDto>>,
    page: Option<SeasonWebPageDto>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SeasonWebPageDto {
    total: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SeasonWebArchiveDto {
    aid: Option<i64>,
    bvid: Option<String>,
    pic: Option<String>,
    title: Option<String>,
    pubdate: Option<i64>,
    duration: Option<i64>,
    stat: Option<SeasonWebStatDto>,
    /// 基线读的就是这个键名（驼峰），而当前响应里**根本没有它**。
    /// 照抄：真有一天服务端补上，这里不用再改。
    #[serde(rename = "upMid")]
    up_mid: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SeasonWebStatDto {
    view: Option<i64>,
    danmaku: Option<i64>,
}

/// 解析一页合集或列表。
///
/// # Errors
/// 信封失败沿用统一分类。
pub fn parse_space_season_web(body: &str) -> Result<SpaceSeasonWebPage, DomainError> {
    let dto: SeasonWebDto = crate::envelope::unwrap_envelope(body)?;
    Ok(SpaceSeasonWebPage {
        total: dto.page.and_then(|p| p.total),
        items: dto
            .archives
            .unwrap_or_default()
            .into_iter()
            .map(|a| {
                let stat = a.stat.unwrap_or_default();
                SpaceWebVideo {
                    aid: a.aid,
                    bvid: a.bvid,
                    cover: a.pic,
                    title: a.title,
                    desc: None,
                    pubdate: a.pubdate,
                    duration: a.duration,
                    view: stat.view,
                    danmaku: stat.danmaku,
                    owner_mid: a.up_mid,
                    owner_name: None,
                    season_id: None,
                    redirect_url: None,
                    badge: None,
                    progress: None,
                }
            })
            .collect(),
    })
}

/// 取一页投稿搜索。
///
/// # Errors
/// 同 [`fetch_member_info`]。
#[allow(clippy::too_many_arguments)]
pub async fn fetch_space_archive_search<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    mid: i64,
    tid: i64,
    page: i64,
    page_size: i64,
    keyword: Option<&str>,
    special_type: Option<&str>,
    order: SpaceArchiveSearchOrder,
    tokens: &RiskTokens,
    mixin_key: &str,
    wts: i64,
) -> Result<SpaceArchiveSearchPage, DomainError> {
    let req = space_archive_search_request(
        mid,
        tid,
        page,
        page_size,
        keyword,
        special_type,
        order,
        tokens,
        mixin_key,
        wts,
    );
    let body = crate::client::send(transport, ctx, &req).await?;
    parse_space_archive_search(&body)
}

/// 取一页合集或列表。
///
/// # Errors
/// 同 [`fetch_member_info`]。
#[allow(clippy::too_many_arguments)]
pub async fn fetch_space_season_web<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    kind: SpaceSeasonWebKind,
    mid: i64,
    id: i64,
    page: i64,
    page_size: i64,
    ascending: bool,
) -> Result<SpaceSeasonWebPage, DomainError> {
    let req = space_season_web_request(kind, mid, id, page, page_size, ascending);
    let body = crate::client::send(transport, ctx, &req).await?;
    parse_space_season_web(&body)
}

// ------------------------------------------------- 专栏 / 音频 / 课堂 / 会员购

/// 专栏列表一页的请求。**挂在 app 域**，base_url 由调用方给。
///
/// 参数顺序照抄基线（`MemberHttp.spaceArticle`）。这条没有签名，顺序本不
/// 影响结果，但两边一致时抓包对照是逐行的。
///
/// 这条还要 `bili-http-engine: cronet` 与 app 端 UA 两个头，由 Dart 补——
/// 理由见本模块开头那段「请求头不在这里」。
#[must_use]
pub fn space_article_request(mid: i64, page: i64, page_size: i64) -> PreparedRequest {
    PreparedRequest {
        path: path::SPACE_ARTICLE,
        query: vec![
            ("build".to_owned(), app_fixed::BUILD.to_owned()),
            ("channel".to_owned(), app_fixed::CHANNEL.to_owned()),
            ("version".to_owned(), app_fixed::VERSION.to_owned()),
            ("c_locale".to_owned(), app_fixed::LOCALE.to_owned()),
            ("mobi_app".to_owned(), app_fixed::MOBI_APP.to_owned()),
            ("platform".to_owned(), app_fixed::PLATFORM.to_owned()),
            ("pn".to_owned(), page.to_string()),
            ("ps".to_owned(), page_size.to_string()),
            ("s_locale".to_owned(), app_fixed::LOCALE.to_owned()),
            ("statistics".to_owned(), app_fixed::STATISTICS.to_owned()),
            ("vmid".to_owned(), mid.to_string()),
        ],
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SpaceArticlePageDto {
    count: Option<i64>,
    item: Option<Vec<SpaceArticleItemDto>>,
    lists_count: Option<i64>,
}

/// 解析一页专栏列表。
///
/// # Errors
/// 信封失败沿用统一分类。`item` 缺失按空列表处理。
pub fn parse_space_article(body: &str) -> Result<SpaceArticlePage, DomainError> {
    let dto: SpaceArticlePageDto = crate::envelope::unwrap_envelope(body)?;
    Ok(SpaceArticlePage {
        count: dto.count,
        lists_count: dto.lists_count,
        items: dto
            .item
            .unwrap_or_default()
            .into_iter()
            .map(article_item)
            .collect(),
    })
}

/// 取一页专栏列表。
///
/// # Errors
/// 同 [`fetch_member_info`]。
pub async fn fetch_space_article<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    mid: i64,
    page: i64,
    page_size: i64,
) -> Result<SpaceArticlePage, DomainError> {
    let req = space_article_request(mid, page, page_size);
    let body = crate::client::send(transport, ctx, &req).await?;
    parse_space_article(&body)
}

/// 音频列表一页的请求。
///
/// `order=1` 是基线写死的「按发布时间倒序」。
#[must_use]
pub fn space_audio_request(mid: i64, page: i64, page_size: i64) -> PreparedRequest {
    PreparedRequest {
        path: path::SPACE_AUDIO,
        query: vec![
            ("pn".to_owned(), page.to_string()),
            ("ps".to_owned(), page_size.to_string()),
            ("order".to_owned(), "1".to_owned()),
            ("uid".to_owned(), mid.to_string()),
            ("web_location".to_owned(), WEB_LOCATION_SPACE_TAB.to_owned()),
        ],
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SpaceAudioPageDto {
    #[serde(rename = "totalSize")]
    total_size: Option<i64>,
    /// 条目藏在 `data.data` 里，不是 `data.item`。
    data: Option<Vec<SpaceAudioListItemDto>>,
}

/// 音频列表页的条目。与主页那一块的 [`SpaceAudioItemDto`] 是两条接口，
/// 键名相同但多一个 `statistic`，`ctime` 的单位也不同。
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SpaceAudioListItemDto {
    id: Option<i64>,
    uid: Option<i64>,
    title: Option<String>,
    cover: Option<String>,
    ctime: Option<i64>,
    statistic: Option<SpaceAudioStatisticDto>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SpaceAudioStatisticDto {
    play: Option<i64>,
    comment: Option<i64>,
}

/// 解析一页音频列表。
///
/// # Errors
/// 信封失败沿用统一分类。
///
/// `ctime` **按毫秒处理**：基线在界面里按「这一条有没有 `statistic` 块」
/// 决定除不除 1000（`lib/pages/member_audio/view.dart`）。那个判断照抄到
/// 这里——它是对服务端两条接口单位不一致的补救，不是排版；带 `statistic`
/// 的是列表页这条，给的是毫秒。
pub fn parse_space_audio(body: &str) -> Result<SpaceAudioPage, DomainError> {
    let dto: SpaceAudioPageDto = crate::envelope::unwrap_envelope(body)?;
    Ok(SpaceAudioPage {
        total_size: dto.total_size,
        items: dto
            .data
            .unwrap_or_default()
            .into_iter()
            .map(|a| {
                let stat = a.statistic;
                SpaceAudioItem {
                    id: a.id,
                    uid: a.uid,
                    title: a.title,
                    cover: a.cover,
                    ctime: match (a.ctime, stat.is_some()) {
                        (Some(ms), true) => Some(ms / 1000),
                        (ctime, _) => ctime,
                    },
                    play: stat.as_ref().and_then(|s| s.play),
                    comment: stat.and_then(|s| s.comment),
                }
            })
            .collect(),
    })
}

/// 取一页音频列表。
///
/// # Errors
/// 同 [`fetch_member_info`]。
pub async fn fetch_space_audio<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    mid: i64,
    page: i64,
    page_size: i64,
) -> Result<SpaceAudioPage, DomainError> {
    let req = space_audio_request(mid, page, page_size);
    let body = crate::client::send(transport, ctx, &req).await?;
    parse_space_audio(&body)
}

/// 课堂列表一页的请求。
#[must_use]
pub fn space_cheese_request(mid: i64, page: i64, page_size: i64) -> PreparedRequest {
    PreparedRequest {
        path: path::SPACE_CHEESE,
        query: vec![
            ("pn".to_owned(), page.to_string()),
            ("ps".to_owned(), page_size.to_string()),
            ("mid".to_owned(), mid.to_string()),
            ("web_location".to_owned(), WEB_LOCATION_SPACE_TAB.to_owned()),
        ],
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SpaceCheesePageDto {
    items: Option<Vec<SpaceCheeseItemDto>>,
    page: Option<SpaceCheesePageInfoDto>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SpaceCheesePageInfoDto {
    next: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SpaceCheeseItemDto {
    cover: Option<String>,
    marks: Option<Vec<String>>,
    season_id: Option<i64>,
    status: Option<String>,
    title: Option<String>,
    /// 服务端给的是字符串形态的 Unix 秒。
    ctime: Option<String>,
}

/// 解析一页课堂列表。
///
/// # Errors
/// 信封失败沿用统一分类。
pub fn parse_space_cheese(body: &str) -> Result<SpaceCheesePage, DomainError> {
    let dto: SpaceCheesePageDto = crate::envelope::unwrap_envelope(body)?;
    Ok(SpaceCheesePage {
        has_next: dto.page.and_then(|p| p.next),
        items: dto
            .items
            .unwrap_or_default()
            .into_iter()
            .map(|c| SpaceCheeseItem {
                cover: c.cover,
                marks: c.marks.unwrap_or_default(),
                season_id: c.season_id,
                status: c.status,
                title: c.title,
                // 解不出来按没有，同基线界面里的 `int.tryParse`。
                ctime: c.ctime.and_then(|t| t.trim().parse().ok()),
            })
            .collect(),
    })
}

/// 取一页课堂列表。
///
/// # Errors
/// 同 [`fetch_member_info`]。
pub async fn fetch_space_cheese<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    mid: i64,
    page: i64,
    page_size: i64,
) -> Result<SpaceCheesePage, DomainError> {
    let req = space_cheese_request(mid, page, page_size);
    let body = crate::client::send(transport, ctx, &req).await?;
    parse_space_cheese(&body)
}

/// 会员购小店一页的请求。**挂在 `mall.bilibili.com` 上**，而且是 POST。
///
/// 三件事与这一族的其它接口都不同，都是基线的事实：
///
/// 1. **POST**，但它是只读的——分页游标在体里（`searchAfter`），不是查询串；
/// 2. **APP 签名**，鉴权靠 `access_key` 而不是 cookie，所以 [`CsrfIn::None`]；
/// 3. `mVersion` / `mallVersion` 是会员购自己的版本号，与 app 域那组常量
///    不是一套，不要合并。
///
/// 未登录时 `access_key` 整条不发（基线的 `?Accounts.main.accessKey`）——
/// 发一个空值会参与签名，换来的是另一个 `sign`。
///
/// 定级 [`Idempotency::Idempotent`]：它只是取一页列表，重发没有副作用。
#[must_use]
pub fn space_shop_request(
    mid: i64,
    page_size: i64,
    access_key: Option<&str>,
    appkey: &str,
    appsec: &str,
    ts: i64,
) -> PreparedWrite {
    let mut params = Vec::new();
    if let Some(key) = access_key {
        params.push(("access_key".to_owned(), key.to_owned()));
    }
    params.push(("actionKey".to_owned(), "appkey".to_owned()));
    params.push(("build".to_owned(), app_fixed::BUILD.to_owned()));
    params.push(("mVersion".to_owned(), MALL_M_VERSION.to_owned()));
    params.push(("mallVersion".to_owned(), app_fixed::BUILD.to_owned()));
    params.push(("statistics".to_owned(), app_fixed::STATISTICS.to_owned()));

    // `from` 与 `msource` 是同一个值：服务端用它区分「从谁的小店 tab 进来」。
    let source = format!("cps_productTab_{mid}");
    PreparedWrite {
        path: path::SPACE_SHOP,
        query: bili_protocol::app_sign::sign(&params, appkey, appsec, ts),
        body: WriteBody::Json(serde_json::json!({
            "from": source,
            "searchAfter": 0,
            "msource": source,
            "pageSize": page_size,
            // 基线发的是字符串，照抄。
            "upMid": mid.to_string(),
        })),
        csrf: CsrfIn::None,
        idempotency: Idempotency::Idempotent,
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SpaceShopPageDto {
    data: Option<Vec<SpaceShopItemDto>>,
    #[serde(rename = "showMoreTab")]
    show_more_tab: Option<bool>,
    #[serde(rename = "clickUrl")]
    click_url: Option<String>,
    #[serde(rename = "showMoreDesc")]
    show_more_desc: Option<String>,
    #[serde(rename = "haveNextPage")]
    have_next_page: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SpaceShopItemDto {
    cover: Option<SpaceShopCoverDto>,
    title: Option<String>,
    #[serde(rename = "cardUrl")]
    card_url: Option<String>,
    #[serde(rename = "belowLabels")]
    below_labels: Option<Vec<SpaceShopLabelDto>>,
    #[serde(rename = "netPrice")]
    net_price: Option<SpaceShopPriceDto>,
    #[serde(rename = "benefitInfos")]
    benefit_infos: Option<Vec<SpaceShopBenefitDto>>,
    #[serde(rename = "itemSourceName")]
    item_source_name: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SpaceShopCoverDto {
    url: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SpaceShopLabelDto {
    title: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SpaceShopPriceDto {
    #[serde(rename = "netPrice")]
    net_price: Option<String>,
    #[serde(rename = "priceSymbol")]
    price_symbol: Option<String>,
    #[serde(rename = "pricePrefix")]
    price_prefix: Option<String>,
}

/// 优惠信息。`suffix` 在基线里是 `dynamic`——服务端给过字符串也给过
/// `null`，所以收成 [`serde_json::Value`] 再按标量渲染。
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SpaceShopBenefitDto {
    prefix: Option<String>,
    amount: Option<String>,
    suffix: Option<serde_json::Value>,
}

/// 把 `suffix` 这种「可能是任何标量」的值渲染成字符串。
///
/// 基线是 Dart 的字符串插值 `'${e.suffix ?? ''}'`：字符串原样，数字按
/// 它的字面量，`null` 按空串。这里复刻那三种，对象与数组按空串——基线
/// 插值会得到 `{a: 1}` 这种 Dart 内部写法，复刻它没有意义。
fn scalar_text(value: Option<&serde_json::Value>) -> String {
    match value {
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(serde_json::Value::Number(n)) => n.to_string(),
        Some(serde_json::Value::Bool(b)) => b.to_string(),
        _ => String::new(),
    }
}

/// 按表单编码解查询参数，与基线 Uri.queryParameters 的加号与片段语义一致。
fn query_param(url: &str, key: &str) -> Option<String> {
    let before_fragment = url.split('#').next()?;
    let query = before_fragment.split_once('?')?.1;
    fn decode(value: &str) -> String {
        percent_encoding::percent_decode_str(&value.replace('+', " "))
            .decode_utf8_lossy()
            .into_owned()
    }
    query.split('&').find_map(|pair| {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        (decode(k) == key).then(|| decode(v))
    })
}

/// 解析一页会员购小店。
///
/// # Errors
/// 信封失败沿用统一分类。
pub fn parse_space_shop(body: &str) -> Result<SpaceShopPage, DomainError> {
    let dto: SpaceShopPageDto = crate::envelope::unwrap_envelope(body)?;
    Ok(SpaceShopPage {
        show_more_tab: dto.show_more_tab,
        show_more_desc: dto.show_more_desc,
        // 基线在界面里 `Uri.parse(clickUrl).queryParameters['url']`。
        more_url: dto.click_url.as_deref().and_then(|u| query_param(u, "url")),
        has_next_page: dto.have_next_page,
        items: dto
            .data
            .unwrap_or_default()
            .into_iter()
            .map(|i| {
                let price = i.net_price.unwrap_or_default();
                SpaceShopItem {
                    cover: i.cover.and_then(|c| c.url),
                    title: i.title,
                    card_url: i.card_url,
                    // 缺标题的标签不显示：刻意不复刻旧 Widget 把 null 插值
                    // 成字面量 "null"，这不是服务端可展示的文案。
                    below_labels: i
                        .below_labels
                        .unwrap_or_default()
                        .into_iter()
                        .filter_map(|l| l.title)
                        .collect(),
                    price_prefix: price.price_prefix,
                    price_symbol: price.price_symbol,
                    net_price: price.net_price,
                    benefits: i
                        .benefit_infos
                        .unwrap_or_default()
                        .into_iter()
                        .map(|b| {
                            format!(
                                "{}{}{}",
                                b.prefix.unwrap_or_default(),
                                b.amount.unwrap_or_default(),
                                scalar_text(b.suffix.as_ref()),
                            )
                        })
                        .collect(),
                    item_source_name: i.item_source_name,
                }
            })
            .collect(),
    })
}

/// 取一页会员购小店。
///
/// # Errors
/// 传输、状态码、信封三类失败，见各自的文档。
#[allow(clippy::too_many_arguments)]
pub async fn fetch_space_shop<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    mid: i64,
    page_size: i64,
    access_key: Option<&str>,
    appkey: &str,
    appsec: &str,
    ts: i64,
) -> Result<SpaceShopPage, DomainError> {
    let req = space_shop_request(mid, page_size, access_key, appkey, appsec, ts);
    let body = crate::client::send_write(transport, ctx, &req).await?;
    parse_space_shop(&body)
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
