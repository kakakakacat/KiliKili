//! 用户空间那一族的 Dart 导出（S6 / issue #56）。
//!
//! 基线对应的 `MemberHttp` 函数随本次迁移逐族删除，按 ADR-011 决策四
//! 不留降级开关。
//!
//! ## 请求头由 Dart 补
//!
//! 这一族里好几条要显式带浏览器 UA 与 `space.bilibili.com` 的
//! `origin`/`referer`。[`SpaceContext::headers`] 原样透传，由
//! `lib/services/rust/rust_space.dart` 按条拼好。这是宿主在拼协议，
//! 已在 `bili_api::space` 的模块文档里记为待收口的例外。
//!
//! ## 账号
//!
//! 这一族在 `ApiType.apiTypeSet` 里都没有登记，与基线拦截器一样走 `main`。
//! 举报那条挂在 `space.bilibili.com`，由 Dart 传不同的 `base_url`。

use std::time::Duration;

use bili_api::grpc_space;
use bili_api::http::RequestContext;
use bili_api::space;
use bili_api::transport_reqwest::ReqwestTransport;
use bili_domain::error::DomainError;
use bili_domain::redact::Redacted;
use bili_domain::space as domain;
use bili_protocol::risk::{RiskTokens, SystemEntropy};

use crate::api::grpc::GrpcContext;
use crate::api::init;
// 标题高亮的分段与搜索页是同一个类型：同一个概念在导出面上只应有一个
// Dart 类，拆成两份只会让调用点多一次转换。
use crate::api::search::TitleSpan;
use crate::api::transport::TransportSettings;

/// 别人的资料。
///
/// 只有八个字段：基线 `MemberInfoModel` 那 12 个里，`sex` / `sign` /
/// `top_photo` / `official` / `live_room` 没有任何消费者
/// （理由见 `bili_domain::space::MemberInfo`）。
#[derive(Debug, Clone)]
pub struct MemberInfo {
    pub mid: Option<i64>,
    pub name: Option<String>,
    pub face: Option<String>,
    /// 用户等级（0–6）。
    pub level: Option<i64>,
    /// 当前账号有没有关注他。
    pub is_followed: Option<bool>,
    /// `> 0` 为大会员。
    pub vip_status: Option<i64>,
    /// `2` 是年度大会员，昵称要染色。
    pub vip_type: Option<i64>,
    /// `1` 为硬核会员。
    pub is_senior_member: Option<i64>,
}

/// 关注关系计数。
#[derive(Debug, Clone)]
pub struct MemberRelationStat {
    /// 对方设了隐私时是 `-1`，不是 0。界面据此显示「已隐藏」。
    pub following: Option<i64>,
    pub follower: Option<i64>,
}

/// 投稿播放与获赞总数。需要登录才有数据。
#[derive(Debug, Clone)]
pub struct MemberUpStat {
    pub archive_view: Option<i64>,
    pub article_view: Option<i64>,
    pub likes: Option<i64>,
}

/// 用户名片。
#[derive(Debug, Clone)]
pub struct MemberCard {
    /// 这条接口的 `mid` 是**字符串**，服务端的事实，不转整数。
    pub mid: Option<String>,
    pub name: Option<String>,
    pub face: Option<String>,
    /// 认证类型，`>= 0` 时头像有角标。
    pub official_type: Option<i64>,
    pub vip_status: Option<i64>,
    pub vip_type: Option<i64>,
    pub archive_count: Option<i64>,
    pub follower: Option<i64>,
}

/// 一个合集或列表。
///
/// 基线的 `SpaceSsModel` 只有一个 `meta`，这里摊平——多一层只为复刻服务端
/// 的嵌套，没有第二个字段跟它并列。
#[derive(Debug, Clone)]
pub struct SpaceSeasonSeries {
    pub cover: Option<String>,
    pub name: Option<String>,
    /// 最近更新时间，Unix 秒。
    pub ptime: Option<i64>,
    /// 里面有多少个稿件。
    pub total: Option<i64>,
    /// 合集 id。与 [`Self::series_id`] 必有其一，界面据此决定跳哪个页面。
    pub season_id: Option<i64>,
    /// 列表 id。
    pub series_id: Option<i64>,
}

/// 一页「合集和列表」。合集与列表分开，拼接是界面的事。
#[derive(Debug, Clone)]
pub struct SpaceSeasonSeriesPage {
    pub total: Option<i64>,
    pub seasons: Vec<SpaceSeasonSeries>,
    pub series: Vec<SpaceSeasonSeries>,
}

/// 投币/点赞列表里的一个稿件。
#[derive(Debug, Clone)]
pub struct CoinLikeArcItem {
    pub title: Option<String>,
    pub cover: Option<String>,
    /// 形如 `bilibili://video/123`，交给路由解析。
    pub uri: Option<String>,
    /// 稿件 id，**字符串**（这条接口如此）。
    pub param: Option<String>,
    pub duration: Option<i64>,
    /// 互动视频。
    pub is_steins: Option<bool>,
    /// 联合投稿。
    pub is_cooperation: Option<bool>,
    pub is_pgc: Option<bool>,
    pub play: Option<i64>,
    pub danmaku: Option<i64>,
    /// 投币/点赞的时间，Unix 秒。不是稿件发布时间。
    pub ctime: Option<i64>,
}

/// 一页投币/点赞列表。
#[derive(Debug, Clone)]
pub struct CoinLikeArcPage {
    pub count: Option<i64>,
    pub items: Vec<CoinLikeArcItem>,
}

/// 一位舰长/提督/总督。
#[derive(Debug, Clone)]
pub struct GuardItem {
    pub uid: Option<i64>,
    pub username: Option<String>,
    pub face: Option<String>,
    /// 1 总督、2 提督、3 舰长。
    pub guard_level: Option<i64>,
}

/// 一页大航海名单。
#[derive(Debug, Clone)]
pub struct MemberGuardPage {
    /// `1` 表示还有下一页。服务端给的就是整数，不是布尔。
    pub has_more: Option<i64>,
    pub items: Vec<GuardItem>,
}

/// 充电排行里的一个人。
#[derive(Debug, Clone)]
pub struct UpowerRankItem {
    pub mid: Option<i64>,
    pub nickname: Option<String>,
    pub avatar: Option<String>,
    /// 已充电天数。
    pub day: Option<i64>,
}

/// 一个充电档位。
#[derive(Debug, Clone)]
pub struct UpowerLevel {
    pub privilege_type: Option<i64>,
    pub name: Option<String>,
    pub member_total: Option<i64>,
}

/// 充电排行。
#[derive(Debug, Clone)]
pub struct UpowerRank {
    pub items: Vec<UpowerRankItem>,
    pub privilege_type: Option<i64>,
    /// `Vec<i64>` 到 Dart 会变成装着 `BigInt` 的 `Int64List`，而档位编号是
    /// 个位数——窄化成 `i32`，省掉每个调用点一次 `.toInt()`。
    pub tabs: Vec<i32>,
    /// 各档位的名字与人数。只有一个档位时界面不显示 tab 条。
    pub levels: Vec<UpowerLevel>,
}

macro_rules! result_type {
    ($(#[$meta:meta])* $name:ident, $data:ty) => {
        $(#[$meta])*
        #[derive(Debug, Clone)]
        pub struct $name {
            pub data: Option<$data>,
            pub error_kind: Option<String>,
            pub error_detail: Option<String>,
            pub error_code: Option<i64>,
        }
    };
}

result_type!(
    /// 别人的资料的结果。
    MemberInfoResult,
    MemberInfo
);
result_type!(
    /// 关注关系计数的结果。
    MemberRelationStatResult,
    MemberRelationStat
);
result_type!(
    /// 播放与获赞总数的结果。
    MemberUpStatResult,
    MemberUpStat
);
result_type!(
    /// 用户名片的结果。
    MemberCardResult,
    MemberCard
);
result_type!(
    /// 「合集和列表」一页的结果。
    SpaceSeasonSeriesResult,
    SpaceSeasonSeriesPage
);
result_type!(
    /// 投币/点赞列表一页的结果。两条接口共用。
    CoinLikeArcResult,
    CoinLikeArcPage
);
result_type!(
    /// 大航海名单一页的结果。
    MemberGuardResult,
    MemberGuardPage
);
result_type!(
    /// 充电排行的结果。
    UpowerRankResult,
    UpowerRank
);
result_type!(
    /// 举报的结果。
    ///
    /// `data` 是**服务端是否受理**，不是「请求是否成功」：这条接口不走
    /// 标准信封，受理失败时 HTTP 仍是 200、响应里只有一个
    /// `"status": false`，没有原因。两者因此必须分开表达——
    /// `error_kind` 为空而 `data` 为 `false`，意思是「发到了，没受理」。
    ReportResult,
    bool
);

/// 这一族共用的请求上下文。
#[derive(Debug, Clone)]
pub struct SpaceContext {
    /// 多数接口是 `api.bilibili.com`；举报那条传 `space.bilibili.com`。
    pub base_url: String,
    /// 完整的 Cookie 头值。未登录时为 `None`。
    pub cookie: Option<String>,
    /// 账号头，外加这条接口要的 UA / origin / referer。
    pub headers: Vec<(String, String)>,
    /// CSRF token。**只有写操作要传**（ADR-013 决策四）。
    pub csrf: Option<String>,
    pub timeout_ms: u64,
    pub transport: TransportSettings,
    /// WBI 的 `mixin_key`。只有资料卡那条用得上。
    pub mixin_key: String,
    /// Unix 秒，由 Dart 注入。
    pub ts: i64,
}

impl SpaceContext {
    fn split(self) -> (RequestContext, ReqwestTransport, String, i64) {
        let transport = ReqwestTransport::new(self.transport.to_config());
        let ctx = RequestContext {
            base_url: self.base_url,
            cookie: self.cookie.map(Redacted::new),
            csrf: self.csrf.map(Redacted::new),
            headers: self.headers,
            timeout: Duration::from_millis(self.timeout_ms),
        };
        (ctx, transport, self.mixin_key, self.ts)
    }
}

// ------------------------------------------------------------------ 只读

/// 取别人的资料。
///
/// 风控 token 在这里现生成（同 playurl）：复用同一对值等于给服务端一个
/// 稳定指纹，与这组参数存在的目的相悖。
pub async fn fetch_member_info(context: SpaceContext, mid: i64) -> MemberInfoResult {
    let (ctx, transport, mixin_key, ts) = context.split();
    let tokens = RiskTokens::generate(&mut SystemEntropy);
    let joined = init::runtime()
        .spawn(async move {
            // `token` 在基线里恒为空串，但参与签名，所以照样发。
            space::fetch_member_info(&transport, &ctx, mid, "", &tokens, &mixin_key, ts).await
        })
        .await;

    match flatten(joined) {
        Ok(i) => MemberInfoResult {
            data: Some(MemberInfo {
                mid: i.mid,
                name: i.name,
                face: i.face,
                level: i.level,
                is_followed: i.is_followed,
                vip_status: i.vip_status,
                vip_type: i.vip_type,
                is_senior_member: i.is_senior_member,
            }),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => MemberInfoResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 取关注关系计数。
pub async fn fetch_member_relation_stat(
    context: SpaceContext,
    mid: i64,
) -> MemberRelationStatResult {
    let (ctx, transport, _, _) = context.split();
    let joined = init::runtime()
        .spawn(async move { space::fetch_member_relation_stat(&transport, &ctx, mid).await })
        .await;

    match flatten(joined) {
        Ok(s) => MemberRelationStatResult {
            data: Some(MemberRelationStat {
                following: s.following,
                follower: s.follower,
            }),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => MemberRelationStatResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 取投稿播放与获赞总数。
pub async fn fetch_member_up_stat(context: SpaceContext, mid: i64) -> MemberUpStatResult {
    let (ctx, transport, _, _) = context.split();
    let joined = init::runtime()
        .spawn(async move { space::fetch_member_up_stat(&transport, &ctx, mid).await })
        .await;

    match flatten(joined) {
        Ok(s) => MemberUpStatResult {
            data: Some(MemberUpStat {
                archive_view: s.archive_view,
                article_view: s.article_view,
                likes: s.likes,
            }),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => MemberUpStatResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 取用户名片。
pub async fn fetch_member_card(context: SpaceContext, mid: i64) -> MemberCardResult {
    let (ctx, transport, _, _) = context.split();
    let joined = init::runtime()
        .spawn(async move { space::fetch_member_card(&transport, &ctx, mid).await })
        .await;

    match flatten(joined) {
        Ok(c) => MemberCardResult {
            data: Some(MemberCard {
                mid: c.mid,
                name: c.name,
                face: c.face,
                official_type: c.official_type,
                vip_status: c.vip_status,
                vip_type: c.vip_type,
                archive_count: c.archive_count,
                follower: c.follower,
            }),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => MemberCardResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 取一页「合集和列表」。
pub async fn fetch_season_series(
    context: SpaceContext,
    mid: i64,
    page: i64,
    page_size: i64,
) -> SpaceSeasonSeriesResult {
    let (ctx, transport, _, _) = context.split();
    let joined = init::runtime()
        .spawn(
            async move { space::fetch_season_series(&transport, &ctx, mid, page, page_size).await },
        )
        .await;

    match flatten(joined) {
        Ok(p) => SpaceSeasonSeriesResult {
            data: Some(SpaceSeasonSeriesPage {
                total: p.total,
                seasons: p.seasons.into_iter().map(season_series).collect(),
                series: p.series.into_iter().map(season_series).collect(),
            }),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => SpaceSeasonSeriesResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

fn season_series(s: bili_domain::space::SpaceSeasonSeries) -> SpaceSeasonSeries {
    SpaceSeasonSeries {
        cover: s.cover,
        name: s.name,
        ptime: s.ptime,
        total: s.total,
        season_id: s.season_id,
        series_id: s.series_id,
    }
}

/// 取一页投币过/点赞过的稿件。`coin` 为真取投币列表。
pub async fn fetch_coin_like_arc(
    context: SpaceContext,
    coin: bool,
    mid: i64,
    page: i64,
    page_size: i64,
) -> CoinLikeArcResult {
    let (ctx, transport, _, _) = context.split();
    let joined = init::runtime()
        .spawn(async move {
            space::fetch_coin_like_arc(&transport, &ctx, coin, mid, page, page_size).await
        })
        .await;

    match flatten(joined) {
        Ok(p) => CoinLikeArcResult {
            data: Some(CoinLikeArcPage {
                count: p.count,
                items: p
                    .items
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
            }),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => CoinLikeArcResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 取一页大航海名单。
pub async fn fetch_member_guard(
    context: SpaceContext,
    ruid: i64,
    page: i64,
    page_size: i64,
) -> MemberGuardResult {
    let (ctx, transport, _, _) = context.split();
    let joined = init::runtime()
        .spawn(
            async move { space::fetch_member_guard(&transport, &ctx, ruid, page, page_size).await },
        )
        .await;

    match flatten(joined) {
        Ok(p) => MemberGuardResult {
            data: Some(MemberGuardPage {
                has_more: p.has_more,
                items: p
                    .items
                    .into_iter()
                    .map(|g| GuardItem {
                        uid: g.uid,
                        username: g.username,
                        face: g.face,
                        guard_level: g.guard_level,
                    })
                    .collect(),
            }),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => MemberGuardResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 取充电排行。`privilege_type` 省掉表示「全部档位」。
pub async fn fetch_upower_rank(
    context: SpaceContext,
    up_mid: i64,
    page: i64,
    page_size: i64,
    privilege_type: Option<i64>,
) -> UpowerRankResult {
    let (ctx, transport, _, _) = context.split();
    let joined = init::runtime()
        .spawn(async move {
            space::fetch_upower_rank(&transport, &ctx, up_mid, page, page_size, privilege_type)
                .await
        })
        .await;

    match flatten(joined) {
        Ok(r) => UpowerRankResult {
            data: Some(UpowerRank {
                items: r
                    .items
                    .into_iter()
                    .map(|i| UpowerRankItem {
                        mid: i.mid,
                        nickname: i.nickname,
                        avatar: i.avatar,
                        day: i.day,
                    })
                    .collect(),
                privilege_type: r.privilege_type,
                // 档位编号是个位数，窄化见字段文档。
                tabs: r
                    .tabs
                    .into_iter()
                    .map(|t| i32::try_from(t).unwrap_or(i32::MAX))
                    .collect(),
                levels: r
                    .levels
                    .into_iter()
                    .map(|l| UpowerLevel {
                        privilege_type: l.privilege_type,
                        name: l.name,
                        member_total: l.member_total,
                    })
                    .collect(),
            }),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => UpowerRankResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

// -------------------------------------------------------------------- 写

/// 举报用户。
pub async fn report_member(
    context: SpaceContext,
    mid: i64,
    reason: String,
    reason_v2: Option<i64>,
) -> ReportResult {
    let (ctx, transport, _, _) = context.split();
    let joined = init::runtime()
        .spawn(async move { space::report_member(&transport, &ctx, mid, &reason, reason_v2).await })
        .await;

    match flatten(joined) {
        Ok(accepted) => ReportResult {
            data: Some(accepted),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => ReportResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
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

// ---------------------------------------------------------------- 空间主页

/// 空间隐私设置。只有看自己的空间时服务端才下发。
#[derive(Debug, Clone)]
pub struct SpaceSetting {
    pub fav_video: Option<i64>,
    pub coins_video: Option<i64>,
    pub likes_video: Option<i64>,
    pub bangumi: Option<i64>,
}

/// 关注关系三元组。控制器据此拼出一个 `relation` 值。
#[derive(Debug, Clone)]
pub struct SpaceRelation {
    pub status: Option<i64>,
    pub is_follow: Option<i64>,
    pub is_followed: Option<i64>,
}

/// 他戴着的直播粉丝勋章。颜色是 `#RRGGBB`，解析留在宿主。
#[derive(Debug, Clone)]
pub struct SpaceLiveMedal {
    pub name: Option<String>,
    pub level: Option<i64>,
    pub color: Option<String>,
    pub name_color: Option<String>,
}

/// 空间页标签（常住地、实名认证）。只有这两类会被下发到这里。
#[derive(Debug, Clone)]
pub struct SpaceCardTag {
    pub title: Option<String>,
    pub uri: Option<String>,
}

/// 推广信息。`content` 为空时整块是 `None`，不是一个空结构。
#[derive(Debug, Clone)]
pub struct SpacePrInfo {
    pub content: Option<String>,
    pub url: Option<String>,
    pub icon: Option<String>,
    pub icon_night: Option<String>,
    pub text_color: String,
    pub bg_color: String,
    pub text_color_night: String,
    pub bg_color_night: String,
}

/// 一个只有昵称与头像的人。充电榜、大航海、共同关注共用。
#[derive(Debug, Clone)]
pub struct SpaceSimpleUser {
    pub mid: Option<i64>,
    pub name: Option<String>,
    pub face: Option<String>,
}

/// 「你关注的人也关注了 TA」。
#[derive(Debug, Clone)]
pub struct SpaceFollowedBy {
    pub items: Vec<SpaceSimpleUser>,
}

/// 空间主页的资料卡。
#[derive(Debug, Clone)]
pub struct SpaceCard {
    /// 这条接口的 `mid` 是**字符串**，同 [`MemberCard::mid`]。
    pub mid: Option<String>,
    pub name: Option<String>,
    pub face: Option<String>,
    pub sign: Option<String>,
    pub fans: Option<i64>,
    pub attention: Option<i64>,
    /// 累计获赞数。
    pub like_num: Option<i64>,
    pub level: Option<i64>,
    /// `2` 为硬核会员。
    pub level_identity: Option<i64>,
    /// 头像挂件图。空串已经过滤掉了。
    pub pendant_image: Option<String>,
    pub official_type: Option<i64>,
    pub official_desc: Option<String>,
    pub official_splice_title: Option<String>,
    /// `> 0` 为大会员。
    pub vip_status: Option<i64>,
    /// `2` 是年度大会员，昵称要染色。
    pub vip_type: Option<i64>,
    pub vip_label_text: Option<String>,
    /// `1` 为封禁中。
    pub silence: Option<i64>,
    pub relation: Option<SpaceRelation>,
    pub live_medal: Option<SpaceLiveMedal>,
    pub tags: Vec<SpaceCardTag>,
    pub pr_info: Option<SpacePrInfo>,
    pub followed_by: Option<SpaceFollowedBy>,
}

/// 置顶合集封面上的标题。
#[derive(Debug, Clone)]
pub struct SpaceTopTitle {
    pub title: Option<String>,
    pub sub_title: Option<String>,
    pub sub_title_colors: Vec<String>,
}

/// 置顶合集的一张封面。
#[derive(Debug, Clone)]
pub struct SpaceTopImage {
    /// 列表里显示的那张。
    pub header: String,
    /// 点开大图时用的原图。
    pub full_cover: String,
    /// 纵向裁切位置，`-1.0`～`1.0`，直接喂给对齐参数。
    pub dy: f64,
    pub title: Option<SpaceTopTitle>,
}

/// 空间头图与置顶合集的封面轮播。
#[derive(Debug, Clone)]
pub struct SpaceImages {
    pub img_url: Option<String>,
    /// 为空时界面回落到 [`Self::img_url`]。
    pub night_img_url: Option<String>,
    /// 非空时整个头图位置换成这组图。
    pub top_images: Vec<SpaceTopImage>,
}

/// 直播间状态。
#[derive(Debug, Clone)]
pub struct SpaceLive {
    /// `1` 表示他有直播间。
    pub room_status: Option<i64>,
    /// `1` 表示正在直播。
    pub live_status: Option<i64>,
    pub room_id: Option<i64>,
}

/// 充电榜。
#[derive(Debug, Clone)]
pub struct SpaceElec {
    pub total: Option<i64>,
    /// 这几条**没有 mid**：服务端在这里给的是空间主人的 id，不是充电者的。
    pub list: Vec<SpaceSimpleUser>,
}

/// 大航海。
#[derive(Debug, Clone)]
pub struct SpaceGuard {
    /// 人数。服务端只给一句话，数字是从里面抠出来的，所以是字符串。
    pub count: Option<String>,
    pub items: Vec<SpaceSimpleUser>,
}

/// 一条投稿的观看进度。
#[derive(Debug, Clone)]
pub struct SpaceArchiveHistory {
    pub progress: Option<i64>,
    pub duration: Option<i64>,
}

/// 投稿列表里的一条。空间主页的五个视频块与投稿页六个 tab 共用。
///
/// **没有作者 mid**：服务端只给 `author` 昵称。基线在这里造过一个
/// `Owner(mid: 0)` 的假 owner，连带让长按菜单多一处类型特判，两处一起
/// 删掉了（见 `bili_domain::space::SpaceArchiveItem`）。
#[derive(Debug, Clone)]
pub struct SpaceArchiveItem {
    pub title: Option<String>,
    pub cover: Option<String>,
    pub bvid: Option<String>,
    /// 首个分 P 的 cid。
    pub cid: Option<i64>,
    /// 稿件 id 或番剧 ep/season id，**字符串**：含义随 `goto` 变。
    pub param: Option<String>,
    pub goto: Option<String>,
    pub uri: Option<String>,
    /// 时长，秒。服务端不给时是 `-1`。
    pub duration: i64,
    pub play: Option<i64>,
    pub danmaku: Option<i64>,
    pub author: Option<String>,
    pub publish_time_text: Option<String>,
    /// 角标文案。空文案的角标已经丢掉了。
    pub badges: Vec<String>,
    pub is_steins: Option<bool>,
    pub is_cooperation: Option<bool>,
    pub is_pgc: Option<bool>,
    pub is_pugv: Option<bool>,
    pub history: Option<SpaceArchiveHistory>,
    /// 合集里这一条的更新时间，Unix 秒。有它时界面用它代替发布时间文案。
    pub season_mtime: Option<i64>,
    /// 漫画的题材。
    pub styles: Option<String>,
    /// 漫画的更新状态。
    pub label: Option<String>,
}

/// 主页上的一块视频（投稿 / 追番 / 投币 / 点赞 / 漫画共用）。
#[derive(Debug, Clone)]
pub struct SpaceVideoBlock {
    pub count: Option<i64>,
    pub items: Vec<SpaceArchiveItem>,
}

/// 主页上的一条专栏。
#[derive(Debug, Clone)]
pub struct SpaceArticleItem {
    pub title: Option<String>,
    pub cover: Option<String>,
    pub publish_time_text: Option<String>,
    pub view: Option<i64>,
    pub reply: Option<i64>,
    pub uri: Option<String>,
}

/// 主页上的一块专栏。
#[derive(Debug, Clone)]
pub struct SpaceArticleBlock {
    pub count: Option<i64>,
    pub items: Vec<SpaceArticleItem>,
}

/// 主页上的一条音频。
#[derive(Debug, Clone)]
pub struct SpaceAudioItem {
    pub play: Option<i64>,
    pub comment: Option<i64>,
    pub id: Option<i64>,
    pub uid: Option<i64>,
    pub title: Option<String>,
    pub cover: Option<String>,
    /// **Unix 秒**。音频页那条接口给的是毫秒，两边共用同一个组件。
    pub ctime: Option<i64>,
}

/// 主页上的一块音频。
#[derive(Debug, Clone)]
pub struct SpaceAudioBlock {
    pub count: Option<i64>,
    pub items: Vec<SpaceAudioItem>,
}

/// 主页上的一个收藏夹。
#[derive(Debug, Clone)]
pub struct SpaceFavItem {
    /// 路由用的是 `media_id`。
    pub media_id: Option<i64>,
    pub title: Option<String>,
    pub cover: Option<String>,
    /// 夹子里有多少条内容。
    pub count: Option<i64>,
    /// `1` 为私密。键名与含义是反的，这是服务端的事实。
    pub is_public: Option<i64>,
}

/// 主页上的一块收藏夹。
#[derive(Debug, Clone)]
pub struct SpaceFavBlock {
    pub count: Option<i64>,
    pub items: Vec<SpaceFavItem>,
}

/// 图文页的一个筛选项。
#[derive(Debug, Clone)]
pub struct SpaceTabFilter {
    pub text: Option<String>,
    /// 发给服务端的那个值，缺省为 `all`。
    pub meta: String,
    pub tab_name: Option<String>,
}

/// 二级 tab。
#[derive(Debug, Clone)]
pub struct SpaceTab2Item {
    pub title: Option<String>,
    pub param: Option<String>,
    pub filters: Vec<SpaceTabFilter>,
    pub season_id: Option<i64>,
    pub series_id: Option<i64>,
}

/// 空间的一级 tab。
#[derive(Debug, Clone)]
pub struct SpaceTab2 {
    pub title: Option<String>,
    pub param: Option<String>,
    pub items: Vec<SpaceTab2Item>,
}

/// 预约附带的抽奖。
#[derive(Debug, Clone)]
pub struct SpaceReservationLottery {
    pub text: Option<String>,
    pub jump_url: Option<String>,
}

/// 一条预约。
#[derive(Debug, Clone)]
pub struct SpaceReservation {
    pub sid: Option<i64>,
    pub name: Option<String>,
    /// 已预约人数。预约/取消后界面要在这个数上加减，所以不是 `Option`。
    pub total: i64,
    pub is_follow: bool,
    pub desc_text: Option<String>,
    /// 有值时多一个「看动态」按钮。
    pub dynamic_id: Option<String>,
    pub lottery: Option<SpaceReservationLottery>,
}

/// 空间主页。
#[derive(Debug, Clone)]
pub struct SpacePage {
    pub relation: Option<i64>,
    pub guest_relation: Option<i64>,
    /// `1` 表示特别关注。
    pub rel_special: Option<i64>,
    pub medal: Option<i64>,
    /// 取值与 [`SpaceTab2::param`] 对齐。
    pub default_tab: Option<String>,
    /// 八个内容块里还有没有东西。为假时「主页」tab 整个去掉。
    pub has_item: bool,
    /// 有没有合集或列表。投稿页据此多一个「全部合集/列表」tab。
    pub has_season_or_series: bool,
    /// **只有看自己的空间时才有**，界面据此判断是不是本人。
    pub setting: Option<SpaceSetting>,
    pub card: Option<SpaceCard>,
    pub images: Option<SpaceImages>,
    pub live: Option<SpaceLive>,
    pub elec: Option<SpaceElec>,
    pub guard: Option<SpaceGuard>,
    pub archive: Option<SpaceVideoBlock>,
    pub season: Option<SpaceVideoBlock>,
    pub coin_archive: Option<SpaceVideoBlock>,
    pub like_archive: Option<SpaceVideoBlock>,
    pub comic: Option<SpaceVideoBlock>,
    pub article: Option<SpaceArticleBlock>,
    pub audio: Option<SpaceAudioBlock>,
    pub favourite: Option<SpaceFavBlock>,
    pub tab2: Vec<SpaceTab2>,
    pub reservations: Vec<SpaceReservation>,
}

/// 「播放全部 / 继续播放」按钮。
#[derive(Debug, Clone)]
pub struct SpaceEpisodicButton {
    pub text: Option<String>,
    /// `oid` 等参数在查询串里。
    pub uri: Option<String>,
}

/// 一页投稿。六个 tab 共用这一个形状。
#[derive(Debug, Clone)]
pub struct SpaceArchivePage {
    /// 总条数。追番那条不给，界面改用本页长度。
    pub count: Option<i64>,
    pub items: Vec<SpaceArchiveItem>,
    pub episodic_button: Option<SpaceEpisodicButton>,
    pub has_next: Option<bool>,
    /// 还能不能往**前**翻。从视频页跳进来定位到某一条时才有意义。
    pub has_prev: Option<bool>,
    /// 下一页的游标。`0` 表示到底了。
    pub next: Option<i64>,
}

/// 投稿列表的六个 tab。与基线 `ContributeType` 一一对应。
#[derive(Debug, Clone, Copy)]
pub enum SpaceArchiveKind {
    Video,
    Charging,
    Season,
    Series,
    Bangumi,
    Comic,
}

/// 投稿视频的排序字段。
#[derive(Debug, Clone, Copy)]
pub enum SpaceArchiveOrder {
    /// 最新发布。
    Pubdate,
    /// 最多播放。
    Click,
}

/// 合集 / 列表的排序方向。
#[derive(Debug, Clone, Copy)]
pub enum SpaceArchiveSort {
    Desc,
    Asc,
}

impl From<SpaceArchiveKind> for space::SpaceArchiveKind {
    fn from(value: SpaceArchiveKind) -> Self {
        match value {
            SpaceArchiveKind::Video => Self::Video,
            SpaceArchiveKind::Charging => Self::Charging,
            SpaceArchiveKind::Season => Self::Season,
            SpaceArchiveKind::Series => Self::Series,
            SpaceArchiveKind::Bangumi => Self::Bangumi,
            SpaceArchiveKind::Comic => Self::Comic,
        }
    }
}

impl From<SpaceArchiveOrder> for space::SpaceArchiveOrder {
    fn from(value: SpaceArchiveOrder) -> Self {
        match value {
            SpaceArchiveOrder::Pubdate => Self::Pubdate,
            SpaceArchiveOrder::Click => Self::Click,
        }
    }
}

impl From<SpaceArchiveSort> for space::SpaceArchiveSort {
    fn from(value: SpaceArchiveSort) -> Self {
        match value {
            SpaceArchiveSort::Desc => Self::Desc,
            SpaceArchiveSort::Asc => Self::Asc,
        }
    }
}

// 下面这组 `From` 只是把领域实体逐字段搬到导出面上。导出面与领域层分开
// 的理由见 ADR-011：导出面要保持语言中立，领域层可以随时改形状。
impl From<domain::SpaceSimpleUser> for SpaceSimpleUser {
    fn from(v: domain::SpaceSimpleUser) -> Self {
        Self {
            mid: v.mid,
            name: v.name,
            face: v.face,
        }
    }
}

impl From<domain::SpaceArchiveItem> for SpaceArchiveItem {
    fn from(v: domain::SpaceArchiveItem) -> Self {
        Self {
            title: v.title,
            cover: v.cover,
            bvid: v.bvid,
            cid: v.cid,
            param: v.param,
            goto: v.goto,
            uri: v.uri,
            duration: v.duration,
            play: v.play,
            danmaku: v.danmaku,
            author: v.author,
            publish_time_text: v.publish_time_text,
            badges: v.badges,
            is_steins: v.is_steins,
            is_cooperation: v.is_cooperation,
            is_pgc: v.is_pgc,
            is_pugv: v.is_pugv,
            history: v.history.map(|h| SpaceArchiveHistory {
                progress: h.progress,
                duration: h.duration,
            }),
            season_mtime: v.season_mtime,
            styles: v.styles,
            label: v.label,
        }
    }
}

impl From<domain::SpaceVideoBlock> for SpaceVideoBlock {
    fn from(v: domain::SpaceVideoBlock) -> Self {
        Self {
            count: v.count,
            items: v.items.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<domain::SpaceCard> for SpaceCard {
    fn from(v: domain::SpaceCard) -> Self {
        Self {
            mid: v.mid,
            name: v.name,
            face: v.face,
            sign: v.sign,
            fans: v.fans,
            attention: v.attention,
            like_num: v.like_num,
            level: v.level,
            level_identity: v.level_identity,
            pendant_image: v.pendant_image,
            official_type: v.official_type,
            official_desc: v.official_desc,
            official_splice_title: v.official_splice_title,
            vip_status: v.vip_status,
            vip_type: v.vip_type,
            vip_label_text: v.vip_label_text,
            silence: v.silence,
            relation: v.relation.map(|r| SpaceRelation {
                status: r.status,
                is_follow: r.is_follow,
                is_followed: r.is_followed,
            }),
            live_medal: v.live_medal.map(|m| SpaceLiveMedal {
                name: m.name,
                level: m.level,
                color: m.color,
                name_color: m.name_color,
            }),
            tags: v
                .tags
                .into_iter()
                .map(|t| SpaceCardTag {
                    title: t.title,
                    uri: t.uri,
                })
                .collect(),
            pr_info: v.pr_info.map(|p| SpacePrInfo {
                content: p.content,
                url: p.url,
                icon: p.icon,
                icon_night: p.icon_night,
                text_color: p.text_color,
                bg_color: p.bg_color,
                text_color_night: p.text_color_night,
                bg_color_night: p.bg_color_night,
            }),
            followed_by: v.followed_by.map(|f| SpaceFollowedBy {
                items: f.items.into_iter().map(Into::into).collect(),
            }),
        }
    }
}

impl From<domain::SpacePage> for SpacePage {
    fn from(v: domain::SpacePage) -> Self {
        Self {
            relation: v.relation,
            guest_relation: v.guest_relation,
            rel_special: v.rel_special,
            medal: v.medal,
            default_tab: v.default_tab,
            has_item: v.has_item,
            has_season_or_series: v.has_season_or_series,
            setting: v.setting.map(|s| SpaceSetting {
                fav_video: s.fav_video,
                coins_video: s.coins_video,
                likes_video: s.likes_video,
                bangumi: s.bangumi,
            }),
            card: v.card.map(Into::into),
            images: v.images.map(|i| SpaceImages {
                img_url: i.img_url,
                night_img_url: i.night_img_url,
                top_images: i
                    .top_images
                    .into_iter()
                    .map(|t| SpaceTopImage {
                        header: t.header,
                        full_cover: t.full_cover,
                        dy: t.dy,
                        title: t.title.map(|x| SpaceTopTitle {
                            title: x.title,
                            sub_title: x.sub_title,
                            sub_title_colors: x.sub_title_colors,
                        }),
                    })
                    .collect(),
            }),
            live: v.live.map(|l| SpaceLive {
                room_status: l.room_status,
                live_status: l.live_status,
                room_id: l.room_id,
            }),
            elec: v.elec.map(|e| SpaceElec {
                total: e.total,
                list: e.list.into_iter().map(Into::into).collect(),
            }),
            guard: v.guard.map(|g| SpaceGuard {
                count: g.count,
                items: g.items.into_iter().map(Into::into).collect(),
            }),
            archive: v.archive.map(Into::into),
            season: v.season.map(Into::into),
            coin_archive: v.coin_archive.map(Into::into),
            like_archive: v.like_archive.map(Into::into),
            comic: v.comic.map(Into::into),
            article: v.article.map(|a| SpaceArticleBlock {
                count: a.count,
                items: a
                    .items
                    .into_iter()
                    .map(|i| SpaceArticleItem {
                        title: i.title,
                        cover: i.cover,
                        publish_time_text: i.publish_time_text,
                        view: i.view,
                        reply: i.reply,
                        uri: i.uri,
                    })
                    .collect(),
            }),
            audio: v.audio.map(|a| SpaceAudioBlock {
                count: a.count,
                items: a
                    .items
                    .into_iter()
                    .map(|i| SpaceAudioItem {
                        play: i.play,
                        comment: i.comment,
                        id: i.id,
                        uid: i.uid,
                        title: i.title,
                        cover: i.cover,
                        ctime: i.ctime,
                    })
                    .collect(),
            }),
            favourite: v.favourite.map(|f| SpaceFavBlock {
                count: f.count,
                items: f
                    .items
                    .into_iter()
                    .map(|i| SpaceFavItem {
                        media_id: i.media_id,
                        title: i.title,
                        cover: i.cover,
                        count: i.count,
                        is_public: i.is_public,
                    })
                    .collect(),
            }),
            tab2: v
                .tab2
                .into_iter()
                .map(|t| SpaceTab2 {
                    title: t.title,
                    param: t.param,
                    items: t
                        .items
                        .into_iter()
                        .map(|i| SpaceTab2Item {
                            title: i.title,
                            param: i.param,
                            filters: i
                                .filters
                                .into_iter()
                                .map(|f| SpaceTabFilter {
                                    text: f.text,
                                    meta: f.meta,
                                    tab_name: f.tab_name,
                                })
                                .collect(),
                            season_id: i.season_id,
                            series_id: i.series_id,
                        })
                        .collect(),
                })
                .collect(),
            reservations: v
                .reservations
                .into_iter()
                .map(|r| SpaceReservation {
                    sid: r.sid,
                    name: r.name,
                    total: r.total,
                    is_follow: r.is_follow,
                    desc_text: r.desc_text,
                    dynamic_id: r.dynamic_id,
                    lottery: r.lottery.map(|l| SpaceReservationLottery {
                        text: l.text,
                        jump_url: l.jump_url,
                    }),
                })
                .collect(),
        }
    }
}

impl From<domain::SpaceArchivePage> for SpaceArchivePage {
    fn from(v: domain::SpaceArchivePage) -> Self {
        Self {
            count: v.count,
            items: v.items.into_iter().map(Into::into).collect(),
            episodic_button: v.episodic_button.map(|b| SpaceEpisodicButton {
                text: b.text,
                uri: b.uri,
            }),
            has_next: v.has_next,
            has_prev: v.has_prev,
            next: v.next,
        }
    }
}

result_type!(
    /// 空间主页的结果。
    SpacePageResult,
    SpacePage
);
result_type!(
    /// 一页投稿的结果。六个 tab 共用。
    SpaceArchiveResult,
    SpaceArchivePage
);

/// 取空间主页。
///
/// `from_view_aid` 是从视频页点进来时带的稿件 id，服务端据此把那条稿件
/// 排到投稿列表前面。
pub async fn fetch_space_page(
    context: SpaceContext,
    mid: i64,
    from_view_aid: Option<String>,
) -> SpacePageResult {
    let (ctx, transport, _, _) = context.split();
    let joined = init::runtime()
        .spawn(async move {
            space::fetch_space_page(&transport, &ctx, mid, from_view_aid.as_deref()).await
        })
        .await;

    match flatten(joined) {
        Ok(page) => SpacePageResult {
            data: Some(page.into()),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => SpacePageResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 取一页投稿。
///
/// 六个 tab 共用这一个函数：参数集合一样，差别只在路径与封面档位，由
/// [`SpaceArchiveKind`] 决定。可省的参数省掉就不发，不是发空值。
#[allow(clippy::too_many_arguments)]
pub async fn fetch_space_archive(
    context: SpaceContext,
    kind: SpaceArchiveKind,
    mid: Option<i64>,
    aid: Option<String>,
    order: Option<SpaceArchiveOrder>,
    sort: Option<SpaceArchiveSort>,
    page: Option<i64>,
    next: Option<i64>,
    season_id: Option<i64>,
    series_id: Option<i64>,
    include_cursor: Option<bool>,
) -> SpaceArchiveResult {
    let (ctx, transport, _, _) = context.split();
    let joined = init::runtime()
        .spawn(async move {
            space::fetch_space_archive(
                &transport,
                &ctx,
                kind.into(),
                mid,
                aid.as_deref(),
                order.map(Into::into),
                sort.map(Into::into),
                page,
                next,
                season_id,
                series_id,
                include_cursor,
            )
            .await
        })
        .await;

    match flatten(joined) {
        Ok(page) => SpaceArchiveResult {
            data: Some(page.into()),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => SpaceArchiveResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

// --------------------------------------- 网页端的投稿搜索与合集 / 列表

/// 投稿搜索里那三种互斥的角标。
///
/// **只给类别，不给文案**：「课堂」「充电专属」「合作」这三个词是界面
/// 措辞，留在宿主（ADR-011 决策二）。
#[derive(Debug, Clone, Copy)]
pub enum SpaceWebBadge {
    /// 课堂（付费课程）。界面另外还要把它当 pugv 处理。
    Lesson,
    Charging,
    Union,
}

/// 投稿搜索与合集/列表里的一条视频。三条接口共用。
#[derive(Debug, Clone)]
pub struct SpaceWebVideo {
    pub aid: Option<i64>,
    pub bvid: Option<String>,
    pub cover: Option<String>,
    pub title: Option<String>,
    pub desc: Option<String>,
    /// Unix 秒。
    pub pubdate: Option<i64>,
    /// 秒。投稿搜索给的是 `"12:34"` 这种字符串，换算在 Rust 做。
    pub duration: Option<i64>,
    pub view: Option<i64>,
    pub danmaku: Option<i64>,
    /// 合集与列表**不给作者**，两个字段都是空的。
    pub owner_mid: Option<i64>,
    pub owner_name: Option<String>,
    pub season_id: Option<i64>,
    pub redirect_url: Option<String>,
    pub badge: Option<SpaceWebBadge>,
    /// 看到哪儿了，秒；`-1` 表示看完了。服务端给的是百分比。
    pub progress: Option<i64>,
}

/// 投稿搜索的一个分类标签。
#[derive(Debug, Clone)]
pub struct SpaceArchiveTag {
    pub tid: Option<i64>,
    pub count: Option<i64>,
    pub name: Option<String>,
    /// 形如 `charging`。有值时筛选发的是它而不是 `tid`。
    pub special_type: Option<String>,
}

/// 一页投稿搜索。
#[derive(Debug, Clone)]
pub struct SpaceArchiveSearchPage {
    pub count: Option<i64>,
    /// 专题（`slist`）加上课堂分区那一条，其余分区已经丢掉。
    pub tags: Vec<SpaceArchiveTag>,
    pub items: Vec<SpaceWebVideo>,
}

/// 一页合集或列表。
#[derive(Debug, Clone)]
pub struct SpaceSeasonWebPage {
    pub total: Option<i64>,
    pub items: Vec<SpaceWebVideo>,
}

/// 投稿搜索的排序字段。
#[derive(Debug, Clone, Copy)]
pub enum SpaceArchiveSearchOrder {
    /// 最新发布。
    Pubdate,
    /// 最多播放。
    Click,
    /// 最多收藏。
    Stow,
}

/// 合集与列表是两条接口。
#[derive(Debug, Clone, Copy)]
pub enum SpaceSeasonWebKind {
    Season,
    Series,
}

impl From<SpaceArchiveSearchOrder> for space::SpaceArchiveSearchOrder {
    fn from(value: SpaceArchiveSearchOrder) -> Self {
        match value {
            SpaceArchiveSearchOrder::Pubdate => Self::Pubdate,
            SpaceArchiveSearchOrder::Click => Self::Click,
            SpaceArchiveSearchOrder::Stow => Self::Stow,
        }
    }
}

impl From<SpaceSeasonWebKind> for space::SpaceSeasonWebKind {
    fn from(value: SpaceSeasonWebKind) -> Self {
        match value {
            SpaceSeasonWebKind::Season => Self::Season,
            SpaceSeasonWebKind::Series => Self::Series,
        }
    }
}

impl From<domain::SpaceWebBadge> for SpaceWebBadge {
    fn from(value: domain::SpaceWebBadge) -> Self {
        match value {
            domain::SpaceWebBadge::Lesson => Self::Lesson,
            domain::SpaceWebBadge::Charging => Self::Charging,
            domain::SpaceWebBadge::Union => Self::Union,
        }
    }
}

impl From<domain::SpaceWebVideo> for SpaceWebVideo {
    fn from(v: domain::SpaceWebVideo) -> Self {
        Self {
            aid: v.aid,
            bvid: v.bvid,
            cover: v.cover,
            title: v.title,
            desc: v.desc,
            pubdate: v.pubdate,
            duration: v.duration,
            view: v.view,
            danmaku: v.danmaku,
            owner_mid: v.owner_mid,
            owner_name: v.owner_name,
            season_id: v.season_id,
            redirect_url: v.redirect_url,
            badge: v.badge.map(Into::into),
            progress: v.progress,
        }
    }
}

result_type!(
    /// 一页投稿搜索的结果。
    SpaceArchiveSearchResult,
    SpaceArchiveSearchPage
);
result_type!(
    /// 一页合集或列表的结果。
    SpaceSeasonWebResult,
    SpaceSeasonWebPage
);

/// 取一页投稿搜索（网页端）。
///
/// 风控 token 现生成，同 [`fetch_member_info`]。
#[allow(clippy::too_many_arguments)]
pub async fn fetch_space_archive_search(
    context: SpaceContext,
    mid: i64,
    tid: i64,
    page: i64,
    page_size: i64,
    keyword: Option<String>,
    special_type: Option<String>,
    order: SpaceArchiveSearchOrder,
) -> SpaceArchiveSearchResult {
    let (ctx, transport, mixin_key, ts) = context.split();
    let tokens = RiskTokens::generate(&mut SystemEntropy);
    let joined = init::runtime()
        .spawn(async move {
            space::fetch_space_archive_search(
                &transport,
                &ctx,
                mid,
                tid,
                page,
                page_size,
                keyword.as_deref(),
                special_type.as_deref(),
                order.into(),
                &tokens,
                &mixin_key,
                ts,
            )
            .await
        })
        .await;

    match flatten(joined) {
        Ok(p) => SpaceArchiveSearchResult {
            data: Some(SpaceArchiveSearchPage {
                count: p.count,
                tags: p
                    .tags
                    .into_iter()
                    .map(|t| SpaceArchiveTag {
                        tid: t.tid,
                        count: t.count,
                        name: t.name,
                        special_type: t.special_type,
                    })
                    .collect(),
                items: p.items.into_iter().map(Into::into).collect(),
            }),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => SpaceArchiveSearchResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 取一页合集或列表（网页端）。
///
/// [`ascending`] 对两条接口的含义一致（正序），但发出去的参数名与形态
/// 不同，由 Rust 按 [`kind`] 分开拼。
pub async fn fetch_space_season_web(
    context: SpaceContext,
    kind: SpaceSeasonWebKind,
    mid: i64,
    id: i64,
    page: i64,
    page_size: i64,
    ascending: bool,
) -> SpaceSeasonWebResult {
    let (ctx, transport, _, _) = context.split();
    let joined = init::runtime()
        .spawn(async move {
            space::fetch_space_season_web(
                &transport,
                &ctx,
                kind.into(),
                mid,
                id,
                page,
                page_size,
                ascending,
            )
            .await
        })
        .await;

    match flatten(joined) {
        Ok(p) => SpaceSeasonWebResult {
            data: Some(SpaceSeasonWebPage {
                total: p.total,
                items: p.items.into_iter().map(Into::into).collect(),
            }),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => SpaceSeasonWebResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

// ------------------------------------------------------- gRPC 投稿搜索

/// gRPC 投稿搜索里的一条。
///
/// 与 [`SpaceWebVideo`] 是**两条不同的接口**：这条是 app 端的 gRPC 搜索，
/// 字段来自 `bilibili.app.archive.v1.Arc`，还带 `<em>` 高亮标记。
#[derive(Debug, Clone)]
pub struct SpaceSearchArchiveItem {
    pub aid: Option<i64>,
    /// 标题按 `<em>` 拆好的段，实体也解过了。Dart 不再跑一遍正则。
    pub title_spans: Vec<TitleSpan>,
    pub cover: Option<String>,
    /// Unix 秒。
    pub pubdate: Option<i64>,
    /// 秒。
    pub duration: Option<i64>,
    pub first_cid: Option<i64>,
    pub view: Option<i64>,
    pub danmaku: Option<i64>,
    pub author_name: Option<String>,
    /// 竖屏稿件。由 `dimension` 的宽高与旋转算出来。
    pub is_vertical: bool,
    /// 有值时点进去是番剧页，不是普通视频页。
    pub redirect_url: Option<String>,
    /// 课堂稿件的跳转地址。
    pub uri: Option<String>,
    pub is_pugv: bool,
}

/// 一页 gRPC 投稿搜索。
#[derive(Debug, Clone)]
pub struct SpaceSearchArchivePage {
    pub total: Option<i64>,
    pub items: Vec<SpaceSearchArchiveItem>,
}

result_type!(
    /// 一页 gRPC 投稿搜索的结果。
    SpaceSearchArchiveResult,
    SpaceSearchArchivePage
);

/// 取一页 gRPC 投稿搜索。
///
/// 走 gRPC 网关，所以上下文是 [`GrpcContext`] 而不是 [`SpaceContext`]：
/// 这条不带 cookie，身份走 metadata 头（同弹幕那几条）。
pub async fn fetch_space_search_archive(
    context: GrpcContext,
    mid: i64,
    keyword: String,
    page: i64,
    page_size: i64,
) -> SpaceSearchArchiveResult {
    let (ctx, transport) = context.split();
    let joined = init::runtime()
        .spawn(async move {
            grpc_space::fetch_search_archive(&transport, &ctx, mid, &keyword, page, page_size).await
        })
        .await;

    match flatten(joined) {
        Ok(p) => SpaceSearchArchiveResult {
            data: Some(SpaceSearchArchivePage {
                total: p.total,
                items: p
                    .items
                    .into_iter()
                    .map(|i| SpaceSearchArchiveItem {
                        aid: i.aid,
                        title_spans: i
                            .title_spans
                            .into_iter()
                            .map(|s| TitleSpan {
                                is_em: s.is_em,
                                text: s.text,
                            })
                            .collect(),
                        cover: i.cover,
                        pubdate: i.pubdate,
                        duration: i.duration,
                        first_cid: i.first_cid,
                        view: i.view,
                        danmaku: i.danmaku,
                        author_name: i.author_name,
                        is_vertical: i.is_vertical,
                        redirect_url: i.redirect_url,
                        uri: i.uri,
                        is_pugv: i.is_pugv,
                    })
                    .collect(),
            }),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => SpaceSearchArchiveResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

// 批次 5：列表实体与宿主无关，导出只保留数据。
#[derive(Debug, Clone)]
pub struct SpaceArticlePage {
    /// 这个人一共有多少篇。界面用它判断到底没到底。
    pub count: Option<i64>,
    /// 文集数量。服务端给了，界面暂时没用上，留着是因为它与 `count` 并列
    /// 出现、含义不同，丢掉之后下一个人会以为没有这个数。
    pub lists_count: Option<i64>,
    pub items: Vec<SpaceArticleItem>,
}
#[derive(Debug, Clone)]
pub struct SpaceAudioPage {
    /// 这个人一共有多少首。界面用它判断到底没到底。
    pub total_size: Option<i64>,
    pub items: Vec<SpaceAudioItem>,
}
#[derive(Debug, Clone)]
pub struct SpaceCheeseItem {
    pub cover: Option<String>,
    /// 角标文案，服务端下发，界面用 `|` 连起来。
    pub marks: Vec<String>,
    /// 课堂 id，点进去要它。
    pub season_id: Option<i64>,
    /// 形如「已更新至第 3 讲」。
    pub status: Option<String>,
    pub title: Option<String>,
    /// 收藏时间，Unix 秒。
    ///
    /// 服务端给的是**字符串**，基线在界面里 `int.tryParse`。解析挪到这儿：
    /// 「这串字符其实是个时间戳」是对服务端约定的解读，不是排版。解不出来
    /// 按没有处理，与基线的 `tryParse` 一致。
    pub ctime: Option<i64>,
}
#[derive(Debug, Clone)]
pub struct SpaceCheesePage {
    /// 还有没有下一页。服务端给的是 `page.next`。
    pub has_next: Option<bool>,
    pub items: Vec<SpaceCheeseItem>,
}
#[derive(Debug, Clone)]
pub struct SpaceShopItem {
    pub cover: Option<String>,
    pub title: Option<String>,
    /// 点开后跳的网页地址。
    pub card_url: Option<String>,
    /// 商品下面那排小标签的文案。界面用 `|` 连起来显示。
    pub below_labels: Vec<String>,
    /// 价格前缀（如「会员价」）。
    pub price_prefix: Option<String>,
    /// 货币符号。服务端下发，不在客户端写死。
    pub price_symbol: Option<String>,
    /// 价格本身。服务端给的就是字符串，不转数字——它可能是「99.9 起」
    /// 这种形态，转一道只会多一个出错点。
    pub net_price: Option<String>,
    /// 优惠信息。每条由服务端的 `prefix` + `amount` + `suffix` 拼成，
    /// **拼接在这里做**：三段之间没有分隔符是服务端的约定，不是排版选择。
    pub benefits: Vec<String>,
    /// 来源店铺名。界面会加「来自」前缀。
    pub item_source_name: Option<String>,
}
#[derive(Debug, Clone)]
pub struct SpaceShopPage {
    pub items: Vec<SpaceShopItem>,
    /// 要不要显示「查看更多」。
    pub show_more_tab: Option<bool>,
    /// 「查看更多」按钮的文案。服务端下发。
    pub show_more_desc: Option<String>,
    /// 「查看更多」真正要打开的网页地址。
    ///
    /// 服务端给的 `clickUrl` 是一个把目标地址塞在 `url` 查询参数里的跳转
    /// 链接，基线在界面里 `Uri.parse(...).queryParameters['url']`。拆解挪到
    /// 这儿：那是对服务端约定的解读。拆不出来时为空，界面据此不跳转。
    pub more_url: Option<String>,
    /// 还有没有下一页。
    pub has_next_page: Option<bool>,
}
#[derive(Debug, Clone)]
pub struct SpaceOpusCover {
    /// 长图顶对齐阈值，沿用基线 2.6。
    pub is_long_pic: bool,
    pub src: Option<String>,
    /// 高宽比，已按 `0.68..=2.7` 夹过。
    ///
    /// 夹取范围照抄基线（`lib/models_new/space/space_opus/cover.dart` 与
    /// 图文卡片组件里是同一组常数）：瀑布流里一张过长或过扁的图会把整列
    /// 撑坏。这是「协议没规定、但两端必须一致」的决定，所以在 Rust。
    /// 服务端没给宽高时为 `1.0`，同基线。
    pub ratio: f64,
}
#[derive(Debug, Clone)]
pub struct SpaceOpusFlowItem {
    /// 图文 id，点进去要它。
    pub oid: Option<i64>,
    /// 正文摘要。服务端按段下发，拼接在这里做。
    pub content: Option<String>,
    pub cover: Option<SpaceOpusCover>,
    /// 点赞数的**文案**（如 `1.2万`）。服务端直接给字符串，不是数字。
    pub like_text: Option<String>,
}
#[derive(Debug, Clone)]
pub struct SpaceOpusCollectionItem {
    /// 文集 id，点进去要它。
    pub collection_id: Option<i64>,
    /// 标题。服务端按段下发，拼接在这里做。
    pub title: Option<String>,
    pub cover: Option<String>,
    /// 封面左下角那行字（如「共 12 篇」）。
    pub cover_bottom_text: Option<String>,
    /// 标题下面那行小字。
    pub sub_title: Option<String>,
}
#[derive(Debug, Clone)]
pub struct SpaceOpusCollectionSection {
    pub title: Option<String>,
    pub items: Vec<SpaceOpusCollectionItem>,
    /// 服务端建议的初始展开状态。
    ///
    /// 之后的展开/收起是界面状态，**不回写到这里**：FRB 结构不可变，
    /// 基线那种 `collection.expand = !collection.expand` 的写法要挪到
    /// 控制器上。
    pub expand: bool,
}
#[derive(Debug, Clone)]
pub struct SpaceOpusFlowPage {
    pub items: Vec<SpaceOpusFlowItem>,
    /// 下一页的游标。空串按没有——proto3 的空串与未设置在 wire 上就是
    /// 一回事，基线判的也是 `isNotEmpty`。
    pub next: Option<String>,
    /// 文集区块。
    pub collection: Option<SpaceOpusCollectionSection>,
}
impl From<domain::SpaceArticlePage> for SpaceArticlePage {
    fn from(i: domain::SpaceArticlePage) -> Self {
        Self {
            count: i.count,
            lists_count: i.lists_count,
            items: i.items.into_iter().map(Into::into).collect(),
        }
    }
}
impl From<domain::SpaceAudioPage> for SpaceAudioPage {
    fn from(i: domain::SpaceAudioPage) -> Self {
        Self {
            total_size: i.total_size,
            items: i.items.into_iter().map(Into::into).collect(),
        }
    }
}
impl From<domain::SpaceCheeseItem> for SpaceCheeseItem {
    fn from(i: domain::SpaceCheeseItem) -> Self {
        Self {
            cover: i.cover,
            marks: i.marks,
            season_id: i.season_id,
            status: i.status,
            title: i.title,
            ctime: i.ctime,
        }
    }
}
impl From<domain::SpaceCheesePage> for SpaceCheesePage {
    fn from(i: domain::SpaceCheesePage) -> Self {
        Self {
            has_next: i.has_next,
            items: i.items.into_iter().map(Into::into).collect(),
        }
    }
}
impl From<domain::SpaceShopItem> for SpaceShopItem {
    fn from(i: domain::SpaceShopItem) -> Self {
        Self {
            cover: i.cover,
            title: i.title,
            card_url: i.card_url,
            below_labels: i.below_labels,
            price_prefix: i.price_prefix,
            price_symbol: i.price_symbol,
            net_price: i.net_price,
            benefits: i.benefits,
            item_source_name: i.item_source_name,
        }
    }
}
impl From<domain::SpaceShopPage> for SpaceShopPage {
    fn from(i: domain::SpaceShopPage) -> Self {
        Self {
            items: i.items.into_iter().map(Into::into).collect(),
            show_more_tab: i.show_more_tab,
            show_more_desc: i.show_more_desc,
            more_url: i.more_url,
            has_next_page: i.has_next_page,
        }
    }
}
impl From<domain::SpaceOpusCover> for SpaceOpusCover {
    fn from(i: domain::SpaceOpusCover) -> Self {
        Self {
            is_long_pic: i.is_long_pic,
            src: i.src,
            ratio: i.ratio,
        }
    }
}
impl From<domain::SpaceOpusFlowItem> for SpaceOpusFlowItem {
    fn from(i: domain::SpaceOpusFlowItem) -> Self {
        Self {
            oid: i.oid,
            content: i.content,
            cover: i.cover.map(Into::into),
            like_text: i.like_text,
        }
    }
}
impl From<domain::SpaceOpusCollectionItem> for SpaceOpusCollectionItem {
    fn from(i: domain::SpaceOpusCollectionItem) -> Self {
        Self {
            collection_id: i.collection_id,
            title: i.title,
            cover: i.cover,
            cover_bottom_text: i.cover_bottom_text,
            sub_title: i.sub_title,
        }
    }
}
impl From<domain::SpaceOpusCollectionSection> for SpaceOpusCollectionSection {
    fn from(i: domain::SpaceOpusCollectionSection) -> Self {
        Self {
            title: i.title,
            items: i.items.into_iter().map(Into::into).collect(),
            expand: i.expand,
        }
    }
}
impl From<domain::SpaceOpusFlowPage> for SpaceOpusFlowPage {
    fn from(i: domain::SpaceOpusFlowPage) -> Self {
        Self {
            items: i.items.into_iter().map(Into::into).collect(),
            next: i.next,
            collection: i.collection.map(Into::into),
        }
    }
}
impl From<domain::SpaceArticleItem> for SpaceArticleItem {
    fn from(i: domain::SpaceArticleItem) -> Self {
        Self {
            title: i.title,
            cover: i.cover,
            publish_time_text: i.publish_time_text,
            view: i.view,
            reply: i.reply,
            uri: i.uri,
        }
    }
}
impl From<domain::SpaceAudioItem> for SpaceAudioItem {
    fn from(i: domain::SpaceAudioItem) -> Self {
        Self {
            id: i.id,
            uid: i.uid,
            title: i.title,
            cover: i.cover,
            ctime: i.ctime,
            play: i.play,
            comment: i.comment,
        }
    }
}
result_type!(SpaceArticleResult, SpaceArticlePage);
result_type!(SpaceAudioResult, SpaceAudioPage);
result_type!(SpaceCheeseResult, SpaceCheesePage);
result_type!(SpaceShopResult, SpaceShopPage);
result_type!(SpaceOpusFlowResult, SpaceOpusFlowPage);
pub async fn fetch_space_article(
    context: SpaceContext,
    mid: i64,
    page: i64,
    page_size: i64,
) -> SpaceArticleResult {
    let (ctx, transport, _, _) = context.split();
    let joined = init::runtime()
        .spawn(
            async move { space::fetch_space_article(&transport, &ctx, mid, page, page_size).await },
        )
        .await;
    match flatten(joined) {
        Ok(p) => SpaceArticleResult {
            data: Some(p.into()),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => SpaceArticleResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}
pub async fn fetch_space_audio(
    context: SpaceContext,
    mid: i64,
    page: i64,
    page_size: i64,
) -> SpaceAudioResult {
    let (ctx, transport, _, _) = context.split();
    let joined = init::runtime()
        .spawn(
            async move { space::fetch_space_audio(&transport, &ctx, mid, page, page_size).await },
        )
        .await;
    match flatten(joined) {
        Ok(p) => SpaceAudioResult {
            data: Some(p.into()),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => SpaceAudioResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}
pub async fn fetch_space_cheese(
    context: SpaceContext,
    mid: i64,
    page: i64,
    page_size: i64,
) -> SpaceCheeseResult {
    let (ctx, transport, _, _) = context.split();
    let joined = init::runtime()
        .spawn(
            async move { space::fetch_space_cheese(&transport, &ctx, mid, page, page_size).await },
        )
        .await;
    match flatten(joined) {
        Ok(p) => SpaceCheeseResult {
            data: Some(p.into()),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => SpaceCheeseResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}
pub async fn fetch_space_shop(
    context: SpaceContext,
    mid: i64,
    page_size: i64,
    access_key: Option<String>,
    appkey: String,
    appsec: String,
) -> SpaceShopResult {
    let (ctx, transport, _, ts) = context.split();
    let joined = init::runtime()
        .spawn(async move {
            space::fetch_space_shop(
                &transport,
                &ctx,
                mid,
                page_size,
                access_key.as_deref(),
                &appkey,
                &appsec,
                ts,
            )
            .await
        })
        .await;
    match flatten(joined) {
        Ok(p) => SpaceShopResult {
            data: Some(p.into()),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => SpaceShopResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}
pub async fn fetch_space_opus_flow(
    context: GrpcContext,
    host_mid: i64,
    next: Option<String>,
    filter_type: String,
    page_size: i64,
) -> SpaceOpusFlowResult {
    let (ctx, transport) = context.split();
    let joined = init::runtime()
        .spawn(async move {
            grpc_space::fetch_opus_space_flow(
                &transport,
                &ctx,
                host_mid,
                next.as_deref(),
                &filter_type,
                page_size,
            )
            .await
        })
        .await;
    match flatten(joined) {
        Ok(p) => SpaceOpusFlowResult {
            data: Some(p.into()),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => SpaceOpusFlowResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 分页边界由 Rust 决定，宿主替换后保留相同语义。
#[flutter_rust_bridge::frb(sync)]
pub fn space_article_is_end(loaded_count: i64, total_count: Option<i64>) -> bool {
    domain::space_article_is_end(loaded_count, total_count)
}

/// 音频未知总数不结束，不能复用专栏的未知值哨兵。
#[flutter_rust_bridge::frb(sync)]
pub fn space_audio_is_end(loaded_count: i64, total_count: Option<i64>) -> bool {
    domain::space_audio_is_end(loaded_count, total_count)
}
