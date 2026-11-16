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

use bili_api::http::RequestContext;
use bili_api::space;
use bili_api::transport_reqwest::ReqwestTransport;
use bili_domain::error::DomainError;
use bili_domain::redact::Redacted;
use bili_protocol::risk::{RiskTokens, SystemEntropy};

use crate::api::init;
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
