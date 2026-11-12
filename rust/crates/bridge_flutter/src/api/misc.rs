//! 杂项接口的 Dart 导出（S15 / issue #66）：音乐页、黑名单、电竞赛事、
//! 云端弹幕屏蔽规则。
//!
//! 本模块里有四条**写操作**：弹幕屏蔽规则的新增与删除、BGM 的「想听」、
//! 关注分组排序。写与只读在这一层的区别只有一处：[`MiscContext::csrf`]。
//! 其余（账号头、超时、传输配置）完全共用，所以不另起一个 `MiscWriteContext`
//! ——那只会多一个要同步维护的结构。
//!
//! ## 为什么四个领域挤在一个桥接模块里
//!
//! 它们在领域层是分开的（`bili_domain::{music,user_list,match_info,
//! danmaku_filter}`），这里合并只是因为**桥接层的上下文是同一个**：都不签名
//! （除音乐详情）、都走同一套账号头与超时。为每个领域再抄一遍
//! `XxxContext`，收益是零而维护点是四个。领域边界由 `bili-api` 那一侧守，
//! 不靠这里的文件数。
//!
//! [`MatchContest`] 被 [`crate::api::search`] 复用：综合搜索的赛事卡片与
//! 赛事详情是同一个实体的两套字段名（见 `bili_api::match_info`）。

use std::time::Duration;

use bili_api::http::RequestContext;
use bili_api::transport_reqwest::ReqwestTransport;
use bili_api::{danmaku_filter, follow, match_info, music, user_list};
use bili_domain::danmaku_filter as domain_filter;
use bili_domain::error::DomainError;
use bili_domain::match_info as domain_match;
use bili_domain::music as domain_music;
use bili_domain::redact::Redacted;

use crate::api::init;
use crate::api::transport::TransportSettings;
use crate::api::write::WriteResult;

// ------------------------------------------------------------------ 音乐

/// 一首 BGM 的详情。
#[derive(Debug, Clone)]
pub struct MusicDetail {
    pub music_title: Option<String>,
    pub origin_artist: Option<String>,
    pub origin_artist_list: Option<String>,
    pub mv_aid: Option<i64>,
    /// 服务端没给时为 0，与基线一致。
    pub mv_cid: i64,
    pub mv_bvid: Option<String>,
    pub mv_cover: Option<String>,
    pub wish_listen: Option<bool>,
    pub wish_count: Option<i64>,
    pub music_source: Option<String>,
    pub album: Option<String>,
    pub artists_list: Vec<Artist>,
    pub listen_pv: Option<i64>,
    /// 成就标签，顺序即界面排标签的顺序。
    pub achievement: Vec<String>,
    pub hot_song_heat: Option<HotSongHeat>,
    pub music_comment: Option<MusicComment>,
    pub music_relation: Option<i64>,
    pub music_publish: Option<String>,
}

/// 参与这首歌的一位艺人。
#[derive(Debug, Clone)]
pub struct Artist {
    pub mid: Option<i64>,
    pub name: Option<String>,
    pub face: Option<String>,
    /// 身份（作词、作曲、演唱……）。
    pub identity: Option<String>,
}

/// 热度曲线。`song_heat` 已按时间升序。
#[derive(Debug, Clone)]
pub struct HotSongHeat {
    pub last_heat: Option<i64>,
    pub song_heat: Vec<SongHeat>,
}

/// 曲线上的一个点。`date` 形如 20240131，不是时间戳。
#[derive(Debug, Clone)]
pub struct SongHeat {
    pub date: i64,
    pub heat: i64,
}

/// 这首歌的评论区入口。
#[derive(Debug, Clone)]
pub struct MusicComment {
    pub nums: Option<i64>,
    pub oid: Option<i64>,
    pub page_type: Option<i64>,
}

/// 相关推荐里的一条视频。
#[derive(Debug, Clone)]
pub struct BgmRecommend {
    pub bvid: Option<String>,
    pub cid: Option<i64>,
    pub cover: Option<String>,
    pub title: Option<String>,
    pub up_nick_name: Option<String>,
    pub play: Option<i64>,
    pub danmu: Option<i64>,
    pub duration: Option<i64>,
    pub labels: Vec<String>,
}

// ------------------------------------------------------------------ 黑名单

/// 一页黑名单。
#[derive(Debug, Clone)]
pub struct BlackList {
    pub list: Vec<BlackListItem>,
    pub total: Option<i64>,
}

/// 黑名单里的一个人。
#[derive(Debug, Clone)]
pub struct BlackListItem {
    pub mid: Option<i64>,
    /// 拉黑时间，Unix 秒。
    pub mtime: Option<i64>,
    pub uname: Option<String>,
    pub face: Option<String>,
}

// ------------------------------------------------------------------ 赛事

/// 一场比赛。搜索卡片与赛事详情共用。
#[derive(Debug, Clone)]
pub struct MatchContest {
    /// 只有搜索卡片给。
    pub id: Option<i64>,
    /// 赛段。空串已归一成 `None`。
    pub game_stage: Option<String>,
    /// 开赛时间，Unix 秒。
    pub stime: Option<i64>,
    pub home_score: Option<i64>,
    pub away_score: Option<i64>,
    /// 直播间号，0 表示没有直播。
    pub live_room: Option<i64>,
    pub season: Option<MatchSeason>,
    pub home_team: Option<MatchTeam>,
    pub away_team: Option<MatchTeam>,
    /// 1 未开始、2 进行中、3 已结束。
    pub contest_status: Option<i64>,
    pub playback: Option<String>,
}

/// 赛事（季）。
#[derive(Debug, Clone)]
pub struct MatchSeason {
    pub title: Option<String>,
    pub logo: Option<String>,
}

/// 参赛队伍。
#[derive(Debug, Clone)]
pub struct MatchTeam {
    pub title: Option<String>,
    pub logo: Option<String>,
    pub logo_full: Option<String>,
}

// -------------------------------------------------------------- 弹幕屏蔽

/// 云端弹幕屏蔽规则，已按类型分组。
#[derive(Debug, Clone)]
pub struct DanmakuFilter {
    pub keywords: Vec<FilterRule>,
    pub patterns: Vec<FilterRule>,
    /// `filter` 是 mid。
    pub users: Vec<FilterRule>,
    /// 空串已归一成 `None`。
    pub toast: Option<String>,
    pub valid: Option<i64>,
    pub ver: Option<i64>,
}

/// 一条规则。
#[derive(Debug, Clone)]
pub struct FilterRule {
    pub id: i64,
    /// 0 关键词、1 正则、2 用户。
    pub kind: i64,
    pub filter: String,
}

// ---------------------------------------------------------------- 结果类型

/// BGM 详情的结果。
#[derive(Debug, Clone)]
pub struct MusicDetailResult {
    pub data: Option<MusicDetail>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

/// 相关推荐的结果。
#[derive(Debug, Clone)]
pub struct BgmRecommendResult {
    pub data: Option<Vec<BgmRecommend>>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

/// 黑名单的结果。
#[derive(Debug, Clone)]
pub struct BlackListResult {
    pub data: Option<BlackList>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

/// 赛事详情的结果。
///
/// 请求成功但服务端没给比赛时，`data` 有值而 `contest` 为空——与「请求
/// 失败」是两回事，界面分别显示空态与错误。
#[derive(Debug, Clone)]
pub struct MatchInfoResult {
    pub data: Option<MatchInfo>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

/// 赛事详情的载荷。
#[derive(Debug, Clone)]
pub struct MatchInfo {
    pub contest: Option<MatchContest>,
}

/// 云端弹幕屏蔽规则的结果。
#[derive(Debug, Clone)]
pub struct DanmakuFilterResult {
    pub data: Option<DanmakuFilter>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

/// 新增一条规则的结果。`data` 是服务端建好的那条，带它分配的 id。
#[derive(Debug, Clone)]
pub struct FilterRuleResult {
    pub data: Option<FilterRule>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

// 写操作的结果类型是 [`crate::api::write::WriteResult`]——它不属于任何
// 一个业务领域，搬去基础设施模块了，见那里的说明。

/// 一次请求所需的公共参数。
#[derive(Debug, Clone)]
pub struct MiscContext {
    pub base_url: String,
    /// 完整的 Cookie 头值。未登录时为 `None`。
    pub cookie: Option<String>,
    pub headers: Vec<(String, String)>,
    /// CSRF token（cookie 里的 `bili_jct`）。**只有写操作要传**，只读的
    /// 那几条传 `None`。Rust 侧不保存它，用完即弃（ADR-013 决策四）。
    pub csrf: Option<String>,
    pub timeout_ms: u64,
    pub transport: TransportSettings,
    /// WBI mixin key。只有 BGM 详情签名，其余传空串即可。
    pub mixin_key: String,
    /// Unix 秒，由 Dart 注入。
    pub wts: i64,
}

impl MiscContext {
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

/// 取 BGM 详情。WBI 签名。
pub async fn fetch_bgm_detail(context: MiscContext, music_id: String) -> MusicDetailResult {
    let (ctx, transport, mixin_key, wts) = context.split();
    let joined = init::runtime()
        .spawn(async move {
            music::fetch_bgm_detail(&transport, &ctx, &music_id, &mixin_key, wts).await
        })
        .await;

    match flatten(joined) {
        Ok(d) => MusicDetailResult {
            data: Some(music_detail(d)),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => MusicDetailResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 取 BGM 的相关推荐。
pub async fn fetch_bgm_recommend(context: MiscContext, music_id: String) -> BgmRecommendResult {
    let (ctx, transport, _, _) = context.split();
    let joined = init::runtime()
        .spawn(async move { music::fetch_bgm_recommend(&transport, &ctx, &music_id).await })
        .await;

    match flatten(joined) {
        Ok(list) => BgmRecommendResult {
            data: Some(list.into_iter().map(bgm_recommend).collect()),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => BgmRecommendResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 取一页黑名单。只读却要 csrf，由 Dart 的账号体系传入。
pub async fn fetch_black_list(
    context: MiscContext,
    page: i64,
    page_size: i64,
    csrf: String,
) -> BlackListResult {
    let (ctx, transport, _, _) = context.split();
    let joined = init::runtime()
        .spawn(async move {
            user_list::fetch_black_list(&transport, &ctx, page, page_size, &csrf).await
        })
        .await;

    match flatten(joined) {
        Ok(list) => BlackListResult {
            data: Some(BlackList {
                list: list
                    .list
                    .into_iter()
                    .map(|i| BlackListItem {
                        mid: i.mid,
                        mtime: i.mtime,
                        uname: i.uname,
                        face: i.face,
                    })
                    .collect(),
                total: list.total,
            }),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => BlackListResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 取一场比赛的详情。
pub async fn fetch_match_info(context: MiscContext, contest_id: i64) -> MatchInfoResult {
    let (ctx, transport, _, _) = context.split();
    let joined = init::runtime()
        .spawn(async move { match_info::fetch_match_info(&transport, &ctx, contest_id).await })
        .await;

    match flatten(joined) {
        Ok(contest) => MatchInfoResult {
            data: Some(MatchInfo {
                contest: contest.map(match_contest),
            }),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => MatchInfoResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 取云端弹幕屏蔽规则。
pub async fn fetch_danmaku_filter(context: MiscContext) -> DanmakuFilterResult {
    let (ctx, transport, _, _) = context.split();
    let joined = init::runtime()
        .spawn(async move { danmaku_filter::fetch_danmaku_filter(&transport, &ctx).await })
        .await;

    match flatten(joined) {
        Ok(f) => DanmakuFilterResult {
            data: Some(DanmakuFilter {
                keywords: f.keywords.into_iter().map(filter_rule).collect(),
                patterns: f.patterns.into_iter().map(filter_rule).collect(),
                users: f.users.into_iter().map(filter_rule).collect(),
                toast: f.toast,
                valid: f.valid,
                ver: f.ver,
            }),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => DanmakuFilterResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 新增一条云端弹幕屏蔽规则（写操作）。
///
/// `kind`：0 关键词 / 1 正则 / 2 用户，与
/// [`bili_api::danmaku_filter::kind`] 一致。
///
/// 失败不重试：这条定级为「可判定」——重发会多一条，但只读的
/// [`fetch_danmaku_filter`] 查得到上一次生没生效，要不要重发由调用方在
/// 查过之后决定（ADR-013 决策一）。
pub async fn add_danmaku_filter(
    context: MiscContext,
    filter: String,
    kind: i64,
) -> FilterRuleResult {
    let (ctx, transport, _, _) = context.split();
    let joined =
        init::runtime()
            .spawn(async move {
                danmaku_filter::add_danmaku_filter(&transport, &ctx, &filter, kind).await
            })
            .await;

    match flatten(joined) {
        Ok(rule) => FilterRuleResult {
            data: Some(filter_rule(rule)),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => FilterRuleResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 删除一条云端弹幕屏蔽规则（写操作）。
///
/// 这条定级为「幂等」：删第二次服务端返回「不存在」，最终状态一致，
/// 所以传输层在「请求没发出去」时会重发（ADR-013 决策一）。
pub async fn del_danmaku_filter(context: MiscContext, ids: i64) -> WriteResult {
    let (ctx, transport, _, _) = context.split();
    let joined = init::runtime()
        .spawn(async move { danmaku_filter::del_danmaku_filter(&transport, &ctx, ids).await })
        .await;

    WriteResult::from_result(flatten(joined))
}

/// 把一首 BGM 标记为「想听」或取消（写操作，#66）。
///
/// `wish` 是**目标状态**不是「切换一下」，所以这条定级为幂等：传输层在
/// 「请求没发出去」时会重发（ADR-013 决策一）。
pub async fn update_wish(context: MiscContext, music_id: String, wish: bool) -> WriteResult {
    let (ctx, transport, _, _) = context.split();
    let joined = init::runtime()
        .spawn(async move { music::update_wish(&transport, &ctx, &music_id, wish).await })
        .await;

    WriteResult::from_result(flatten(joined))
}

/// 提交关注分组的新顺序（写操作，#66）。
///
/// `tagids` 是逗号分隔的分组 id，顺序即目标顺序；整串是置为语义，因此
/// 定级为幂等。
pub async fn sort_follow_tag(context: MiscContext, tagids: String) -> WriteResult {
    let (ctx, transport, _, _) = context.split();
    let joined = init::runtime()
        .spawn(async move { follow::sort_follow_tag(&transport, &ctx, &tagids).await })
        .await;

    WriteResult::from_result(flatten(joined))
}

fn music_detail(d: domain_music::MusicDetail) -> MusicDetail {
    MusicDetail {
        music_title: d.music_title,
        origin_artist: d.origin_artist,
        origin_artist_list: d.origin_artist_list,
        mv_aid: d.mv_aid,
        mv_cid: d.mv_cid,
        mv_bvid: d.mv_bvid,
        mv_cover: d.mv_cover,
        wish_listen: d.wish_listen,
        wish_count: d.wish_count,
        music_source: d.music_source,
        album: d.album,
        artists_list: d
            .artists_list
            .into_iter()
            .map(|a| Artist {
                mid: a.mid,
                name: a.name,
                face: a.face,
                identity: a.identity,
            })
            .collect(),
        listen_pv: d.listen_pv,
        achievement: d.achievement,
        hot_song_heat: d.hot_song_heat.map(|h| HotSongHeat {
            last_heat: h.last_heat,
            song_heat: h
                .song_heat
                .into_iter()
                .map(|p| SongHeat {
                    date: p.date,
                    heat: p.heat,
                })
                .collect(),
        }),
        music_comment: d.music_comment.map(|c| MusicComment {
            nums: c.nums,
            oid: c.oid,
            page_type: c.page_type,
        }),
        music_relation: d.music_relation,
        music_publish: d.music_publish,
    }
}

fn bgm_recommend(i: domain_music::BgmRecommend) -> BgmRecommend {
    BgmRecommend {
        bvid: i.bvid,
        cid: i.cid,
        cover: i.cover,
        title: i.title,
        up_nick_name: i.up_nick_name,
        play: i.play,
        danmu: i.danmu,
        duration: i.duration,
        labels: i.labels,
    }
}

/// 领域实体 → 导出结构。`pub(crate)`：综合搜索的赛事卡片也要用它。
pub(crate) fn match_contest(c: domain_match::MatchContest) -> MatchContest {
    MatchContest {
        id: c.id,
        game_stage: c.game_stage,
        stime: c.stime,
        home_score: c.home_score,
        away_score: c.away_score,
        live_room: c.live_room,
        season: c.season.map(|s| MatchSeason {
            title: s.title,
            logo: s.logo,
        }),
        home_team: c.home_team.map(team),
        away_team: c.away_team.map(team),
        contest_status: c.contest_status,
        playback: c.playback,
    }
}

fn team(t: domain_match::MatchTeam) -> MatchTeam {
    MatchTeam {
        title: t.title,
        logo: t.logo,
        logo_full: t.logo_full,
    }
}

fn filter_rule(r: domain_filter::FilterRule) -> FilterRule {
    FilterRule {
        id: r.id,
        kind: r.kind,
        filter: r.filter,
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
