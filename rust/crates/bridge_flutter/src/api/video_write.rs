//! 视频页写操作的 Dart 导出（S4 / issue #54）。
//!
//! 对应基线 `VideoHttp` 里除发评论以外的全部写操作（`lib/http/video.dart`）。
//! 幂等定级、csrf 位置、APP 签名都在 `bili_api::video_write` 一侧定死，
//! 这一层只负责把凭据与设置递过去、把失败折成返回值。
//!
//! ## 为什么是两个上下文
//!
//! [`VideoWriteContext`] 走 Cookie + csrf，[`AppWriteContext`] 走
//! `access_key` + APP 签名、且**不带 Cookie**。合成一个要么让一半字段
//! 常年为 `None`，要么让调用方自己记住「这条该填哪几格」——而填错的
//! 表现是服务端以 `-101` 拒绝，看起来像没登录。
//!
//! 分成两个之后，Dart 侧选哪一个由接口自己决定（见
//! `lib/services/rust/rust_video_write.dart`），选错编译不过。
//!
//! ## 刻意没迁的那一条
//!
//! `replyAdd`（发评论）留在 Dart。它返回的是一个 protobuf `ReplyInfo`，
//! 经 `RequestUtils.replyCast` 还原后存进 Hive，再由 **gRPC 驱动的**评论
//! 区消费。把它迁过来等于先把评论区的 gRPC 模型迁完——那是 S2（#52）的
//! 范围，ADR-013 也明说「没碰 gRPC 里的写操作」。它同时是本片唯一一条
//! `Unsafe` 定级的接口，单独做、单独审更稳妥。

use std::time::Duration;

use bili_api::http::RequestContext;
use bili_api::transport_reqwest::ReqwestTransport;
use bili_api::video_write::{self, AppAuth, DislikeReason, HeartBeatParams};
use bili_domain::error::DomainError;
use bili_domain::redact::Redacted;

use crate::api::init;
use crate::api::transport::TransportSettings;
use crate::api::write::WriteResult;

// ------------------------------------------------------------------ 入参

/// web 端写操作的公共参数（`api.bilibili.com` / `api.live.bilibili.com`）。
#[derive(Debug, Clone)]
pub struct VideoWriteContext {
    /// 主机。这一片横跨 `api.bilibili.com` 与 `api.live.bilibili.com`，
    /// 由调用方按接口给出。
    pub base_url: String,
    /// 完整的 Cookie 头值。未登录时为 `None`。**Rust 不保存**，用完即弃。
    pub cookie: Option<String>,
    /// CSRF token（cookie 里的 `bili_jct`）。补在哪里由请求构造决定
    /// （ADR-013 决策四）。
    pub csrf: Option<String>,
    pub headers: Vec<(String, String)>,
    pub timeout_ms: u64,
    pub transport: TransportSettings,
}

/// APP 端写操作的公共参数（`app.bilibili.com`）。
///
/// **没有 cookie 字段**，这不是遗漏：基线拦截器对 `app.bilibili.com`
/// 的请求根本不加载 Cookie，鉴权全靠 `access_key` + APP 签名
/// （`account_mgr.dart` 的 `isApp` 分支）。给它一个 cookie 字段，下一个
/// 人就会填上，而填上之后多出来的那条头只会让请求面与基线对不上。
#[derive(Debug, Clone)]
pub struct AppWriteContext {
    pub base_url: String,
    pub headers: Vec<(String, String)>,
    pub appkey: String,
    pub appsec: String,
    /// 未登录时为 `None`，此时整个 `access_key` 参数不出现。
    pub access_key: Option<String>,
    /// 签名用的 Unix 秒，由调用方注入以便快照固定。
    pub ts: i64,
    pub timeout_ms: u64,
    pub transport: TransportSettings,
}

impl VideoWriteContext {
    fn split(self) -> (RequestContext, ReqwestTransport) {
        let transport = ReqwestTransport::new(self.transport.to_config());
        let ctx = RequestContext {
            base_url: self.base_url,
            cookie: self.cookie.map(Redacted::new),
            csrf: self.csrf.map(Redacted::new),
            headers: self.headers,
            timeout: Duration::from_millis(self.timeout_ms),
        };
        (ctx, transport)
    }
}

impl AppWriteContext {
    fn split(self) -> (RequestContext, ReqwestTransport, AppAuth) {
        let transport = ReqwestTransport::new(self.transport.to_config());
        let auth = AppAuth {
            appkey: self.appkey,
            appsec: self.appsec,
            access_key: self.access_key,
            ts: self.ts,
        };
        let ctx = RequestContext {
            base_url: self.base_url,
            // APP 端接口不带 Cookie，见结构体文档。
            cookie: None,
            // 也没有 csrf：构造处已经标了 `CsrfIn::None`，这里再给一个
            // 值也不会被用上，但留着会让人以为它有用。
            csrf: None,
            headers: self.headers,
            timeout: Duration::from_millis(self.timeout_ms),
        };
        (ctx, transport, auth)
    }
}

// ------------------------------------------------------------------ 结果

/// 一键三连（UGC）的回执。
///
/// 三个布尔各自表示那一项**这次是否生效**——三连在协议上不是原子操作，
/// 部分成功是正常结果，不是错误。
#[derive(Debug, Clone)]
pub struct UgcTripleResult {
    pub like: Option<bool>,
    pub coin: Option<bool>,
    pub fav: Option<bool>,
    pub multiply: Option<i64>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

/// 一键三连（PGC）的回执。字段与 UGC 那套不同名也不同类型，见
/// [`bili_domain::video_write::PgcTriple`]。
#[derive(Debug, Clone)]
pub struct PgcTripleResult {
    pub coin: Option<i64>,
    pub coin_number: Option<i64>,
    pub favorite: Option<i64>,
    pub fmid: Option<i64>,
    pub follow: Option<i64>,
    pub like: Option<i64>,
    pub relation: Option<bool>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

/// 带一句 toast 文案的写操作结果（点赞、追番的加删改）。
///
/// `toast` 为 `None` 有两种情形：操作失败（看 `error_kind`），或者成功
/// 但服务端没给文案。调用方先判 `error_kind == null` 再取 `toast`，兜底
/// 文案由宿主决定——那是 i18n 的事。
#[derive(Debug, Clone)]
pub struct ToastResult {
    pub toast: Option<String>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

// ------------------------------------------------------- APP 端写操作导出

/// 点赞 / 取消赞。`like` 是**目标状态**。
///
/// 取值映射（目标为赞发 `0`）与它的反直觉之处见
/// [`bili_api::video_write::like_request`]。
pub async fn like_video(context: AppWriteContext, bvid: String, like: bool) -> ToastResult {
    let (ctx, transport, auth) = context.split();
    let joined = init::runtime()
        .spawn(async move {
            let aid = video_write::aid_from_bvid(&bvid)?;
            let prepared = video_write::like_request(aid, like, &auth);
            video_write::send_parsed(&transport, &ctx, &prepared, video_write::parse_like_toast)
                .await
        })
        .await;

    match flatten(joined) {
        Ok(toast) => ToastResult {
            toast,
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => ToastResult {
            toast: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 点踩 / 取消踩。`dislike` 是**目标状态**。
///
/// 这条接口的响应可能根本不是 JSON，判定因此走
/// [`bili_api::video_write::check_dislike_response`] 而不是通用信封。
pub async fn dislike_video(context: AppWriteContext, bvid: String, dislike: bool) -> WriteResult {
    let (ctx, transport, auth) = context.split();
    let joined = init::runtime()
        .spawn(async move {
            let aid = video_write::aid_from_bvid(&bvid)?;
            let prepared = video_write::dislike_request(aid, dislike, &auth);
            video_write::send_dislike(&transport, &ctx, &prepared).await
        })
        .await;

    WriteResult::from_result(flatten(joined))
}

/// 投币。`multiply` 是币数，`select_like` 为 1 时附带点赞。
///
/// **不自动重试**：定级 `Verifiable`，重发会多扣硬币（ADR-013 决策一）。
/// 失败了由用户自己再按一次。
pub async fn coin_video(
    context: AppWriteContext,
    bvid: String,
    multiply: i64,
    select_like: i64,
) -> WriteResult {
    let (ctx, transport, auth) = context.split();
    let joined = init::runtime()
        .spawn(async move {
            let aid = video_write::aid_from_bvid(&bvid)?;
            let prepared = video_write::coin_request(aid, multiply, select_like, &auth);
            video_write::send_write(&transport, &ctx, &prepared).await
        })
        .await;

    WriteResult::from_result(flatten(joined))
}

/// 首页「不感兴趣」的反馈与取消。
///
/// `reason_id` 与 `feedback_id` 二选一：两个都给或都不给时返回协议错误
/// 而不是挑一个发出去。基线在调用点用 `assert` 表达同一件事，而 assert
/// 在 release 构建里不执行。
///
/// 基线用的是 GET，这里照搬，理由见
/// [`bili_api::video_write::feed_dislike_request`]。
pub async fn feed_dislike(
    context: AppWriteContext,
    goto: String,
    id: i64,
    reason_id: Option<i64>,
    feedback_id: Option<i64>,
    cancel: bool,
) -> WriteResult {
    let reason = match (reason_id, feedback_id) {
        (Some(r), None) => DislikeReason::Reason(r),
        (None, Some(f)) => DislikeReason::Feedback(f),
        _ => {
            return WriteResult::failed(&DomainError::new(
                bili_domain::error::ErrorKind::Protocol,
                "reason_id 与 feedback_id 必须二选一",
            ));
        }
    };

    let (ctx, transport, auth) = context.split();
    let joined = init::runtime()
        .spawn(async move {
            let prepared = video_write::feed_dislike_request(&goto, id, reason, cancel, &auth);
            video_write::send_feed_dislike(&transport, &ctx, &prepared).await
        })
        .await;

    WriteResult::from_result(flatten(joined))
}

// ------------------------------------------------------- web 端写操作导出

/// UGC 一键三连。
///
/// **不自动重试**（含投币，定级 `Verifiable`）。
pub async fn ugc_triple(context: VideoWriteContext, bvid: String) -> UgcTripleResult {
    let (ctx, transport) = context.split();
    let joined = init::runtime()
        .spawn(async move {
            let aid = video_write::aid_from_bvid(&bvid)?;
            let prepared = video_write::ugc_triple_request(aid);
            video_write::send_parsed(&transport, &ctx, &prepared, video_write::parse_ugc_triple)
                .await
        })
        .await;

    match flatten(joined) {
        Ok(t) => UgcTripleResult {
            like: t.like,
            coin: t.coin,
            fav: t.fav,
            multiply: t.multiply,
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => UgcTripleResult {
            like: None,
            coin: None,
            fav: None,
            multiply: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// PGC 一键三连。
pub async fn pgc_triple(context: VideoWriteContext, ep_id: i64) -> PgcTripleResult {
    let (ctx, transport) = context.split();
    let joined = init::runtime()
        .spawn(async move {
            let prepared = video_write::pgc_triple_request(ep_id);
            video_write::send_parsed(&transport, &ctx, &prepared, video_write::parse_pgc_triple)
                .await
        })
        .await;

    match flatten(joined) {
        Ok(t) => PgcTripleResult {
            coin: t.coin,
            coin_number: t.coin_number,
            favorite: t.favorite,
            fmid: t.fmid,
            follow: t.follow,
            like: t.like,
            relation: t.relation,
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => PgcTripleResult {
            coin: None,
            coin_number: None,
            favorite: None,
            fmid: None,
            follow: None,
            like: None,
            relation: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 操作用户关系：关注 1 / 取关 2 / 拉黑 5 / 取消拉黑 6 / 移除粉丝 7。
///
/// `fp` 是基线 `extend_content` 里的浏览器 UA（`BrowserUa.pc`），真相源
/// 在宿主。
///
/// **未登录时的拉黑不走这里**：基线那条分支只写本地 `Pref`、不碰网络，
/// 按 ADR-013 决策三第 1 条属于平台存储，留在宿主。
pub async fn relation_mod(
    context: VideoWriteContext,
    mid: i64,
    act: i64,
    re_src: i64,
    fp: String,
) -> WriteResult {
    send_plain(context, move || {
        video_write::relation_mod_request(mid, act, re_src, &fp)
    })
    .await
}

/// 删除一条评论。
///
/// 成功后清本地缓存那一步留在宿主，且必须在**本函数返回成功之后**做
/// （ADR-013 决策八）。
pub async fn reply_del(
    context: VideoWriteContext,
    reply_type: i64,
    oid: i64,
    rpid: i64,
) -> WriteResult {
    send_plain(context, move || {
        video_write::reply_del_request(reply_type, oid, rpid)
    })
    .await
}

/// 追番 / 追剧：加入。
///
/// `season_id` 可空是因为基线的模型里它就是可空的。为空时**不发请求**：
/// 没有 season_id 的追番必然失败，与其换回一个业务错误码，不如在这里就
/// 说清楚（与 `video_detail` 对空 bvid 的处理同一条原则）。
pub async fn pgc_add(context: VideoWriteContext, season_id: Option<i64>) -> ToastResult {
    match season_id {
        Some(id) => send_season(context, video_write::pgc_add_request(id)).await,
        None => missing_season_id(),
    }
}

/// 追番 / 追剧：取消。
pub async fn pgc_del(context: VideoWriteContext, season_id: Option<i64>) -> ToastResult {
    match season_id {
        Some(id) => send_season(context, video_write::pgc_del_request(id)).await,
        None => missing_season_id(),
    }
}

/// 追番状态改为「想看 1 / 在看 2 / 看过 3」。
///
/// `season_ids` 是**逗号分隔的一串** id——追番列表页会批量改。理由见
/// [`bili_api::video_write::pgc_update_request`]。
pub async fn pgc_update(
    context: VideoWriteContext,
    season_ids: String,
    status: i64,
) -> ToastResult {
    if season_ids.is_empty() {
        return missing_season_id();
    }
    send_season(
        context,
        video_write::pgc_update_request(&season_ids, status),
    )
    .await
}

/// 没有 season_id 时的结果。不发请求，直接告诉调用方参数不成立。
fn missing_season_id() -> ToastResult {
    let e = DomainError::new(
        bili_domain::error::ErrorKind::Protocol,
        "缺少 season_id，追番请求不会有有效结果",
    );
    ToastResult {
        toast: None,
        error_kind: Some(e.kind().to_string()),
        error_detail: Some(e.detail().to_owned()),
        error_code: e.code(),
    }
}

// --------------------------------------------------------------- 上报类导出

/// 历史记录上报。
///
/// 高频自动触发：调用方应当把 `transport.retry_count` 设成 0
/// （ADR-013 决策三「上报类的例外」）。重试的收益接近零，代价是账号级的
/// 风控计数。
pub async fn history_report(context: VideoWriteContext, aid: i64, kind: i64) -> WriteResult {
    send_plain(context, move || {
        video_write::history_report_request(aid, kind)
    })
    .await
}

/// 播放进度心跳。
///
/// `video_type` 取 `ugc` / `pgc` / `pugv`，与
/// [`crate::api::play_url_fetch`] 的写法一致——枚举名跨 FFI 走字符串，
/// 不认识的值按 UGC 处理（基线的 `VideoType` 没有第四种）。
///
/// **未登录时不该调用**：基线拦截器会直接拒掉这条请求。那个判断要的是
/// 账号状态，留在宿主。
#[allow(clippy::too_many_arguments)]
pub async fn heart_beat(
    context: VideoWriteContext,
    aid: Option<i64>,
    bvid: Option<String>,
    cid: i64,
    epid: Option<i64>,
    season_id: Option<i64>,
    sub_type: Option<i64>,
    video_type: String,
    played_time: i64,
) -> WriteResult {
    let (code, is_pugv) = match video_type.as_str() {
        "pgc" => (4, false),
        "pugv" => (10, true),
        _ => (3, false),
    };
    let params = HeartBeatParams {
        aid,
        bvid,
        cid,
        epid,
        season_id,
        sub_type,
        video_type: code,
        played_time,
        is_pugv,
    };
    send_plain(context, move || video_write::heart_beat_request(&params)).await
}

/// 播放列表（medialist）的历史上报。
pub async fn media_list_history(
    context: VideoWriteContext,
    desc: i64,
    oid: i64,
    upper_mid: i64,
) -> WriteResult {
    send_plain(context, move || {
        video_write::media_list_history_request(desc, oid, upper_mid)
    })
    .await
}

/// 进入直播间的上报。体是 JSON、csrf 在查询串上，见
/// [`bili_api::video_write::room_entry_action_request`]。
pub async fn room_entry_action(context: VideoWriteContext, room_id: i64) -> WriteResult {
    send_plain(context, move || {
        video_write::room_entry_action_request(room_id)
    })
    .await
}

// ------------------------------------------------------------------- 内部

/// 没有载荷的 web 端写操作共用的发送路径。
///
/// 不导出给 Dart（没有 `pub`）。请求在 spawn **之内**构造：构造函数拿的
/// 是引用，放到外面就得把它跨 `await` 搬进闭包，白白多一次克隆。
async fn send_plain<F>(context: VideoWriteContext, build: F) -> WriteResult
where
    F: FnOnce() -> bili_api::request::PreparedWrite + Send + 'static,
{
    let (ctx, transport) = context.split();
    let joined = init::runtime()
        .spawn(async move {
            let prepared = build();
            video_write::send_write(&transport, &ctx, &prepared).await
        })
        .await;

    WriteResult::from_result(flatten(joined))
}

/// 追番三条共用的发送路径：载荷在 `result` 里，只取一句 toast。
async fn send_season(
    context: VideoWriteContext,
    prepared: bili_api::request::PreparedWrite,
) -> ToastResult {
    let (ctx, transport) = context.split();
    let joined = init::runtime()
        .spawn(async move {
            video_write::send_parsed(&transport, &ctx, &prepared, video_write::parse_season_toast)
                .await
        })
        .await;

    match flatten(joined) {
        Ok(toast) => ToastResult {
            toast,
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => ToastResult {
            toast: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 把 `spawn` 的 JoinError 折进领域错误。与其余桥接模块同一份写法。
fn flatten<T>(
    joined: Result<Result<T, DomainError>, tokio::task::JoinError>,
) -> Result<T, DomainError> {
    match joined {
        Ok(inner) => inner,
        Err(e) => Err(DomainError::new(
            bili_domain::error::ErrorKind::Network,
            format!("写操作任务未完成: {e}"),
        )),
    }
}
