//! SponsorBlock 的 Dart 导出。
//!
//! # 本模块是 typed 直连的**样板**（issue #64 / EPIC #67）
//!
//! 与此前七条路径（videoDetail、playurl、danmaku…）的区别：那些返回
//! **JSON 字符串**，Dart 再还原成既有模型；本模块返回 **typed 结构**，
//! Dart 直接消费，对应的 Dart 模型随本次迁移删除。
//!
//! 照抄时请连同下面四条约定一起抄：
//!
//! ## 1. 结果用值表达失败，不用 `Result`
//!
//! FRB 把 `Err` 映射成 Dart 异常，会逼每个调用点写 try/catch，漏一处就把
//! 一条可降级的路径变成线上崩溃。统一形状：
//!
//! ```text
//! XxxResult { data: Option<T>, error_kind, error_detail, error_code }
//! ```
//!
//! - `error_kind`：`network|remote|protocol|auth|storage|player|cancelled`，
//!   给程序做决策；
//! - `error_detail`：**可直接展示**的文案，不含凭据、不含完整 URL；
//! - `error_code`：协议自带的码。bilibili 接口是业务 code，本模块是 HTTP
//!   状态码——调用点要靠它区分 404（没有片段，静默）与其余失败。
//!
//! ## 2. 设置与凭据按请求传入，Rust 不存
//!
//! 服务器地址来自用户设置（基线 `Pref.blockServer`），与 Cookie 同理：
//! 真相源留在 Dart，Rust 每次用完即弃。
//!
//! ## 3. 实体字段保持 `Option`
//!
//! 服务端确实会省字段。在跨 FFI 处收紧成非空，失败模式是整条记录拿不到。
//!
//! ## 4. 迁移 PR 内必须带一次性 parity 测试
//!
//! typed 直连删掉了 Dart 模型，golden fixture 的期望值就失去了外部来源。
//! 办法：在 PR 里用**尚未删除的** Dart 模型解析同一份 fixture，与 Rust
//! 输出逐字段比对，通过后连同旧模型一起删。见
//! `test/services/rust/sponsor_block_parity_test.dart`。

use std::time::Duration;

use bili_api::http::RequestContext;
use bili_api::sponsor_block;
use bili_api::transport_reqwest::ReqwestTransport;
use bili_domain::error::DomainError;

use crate::api::init;
use crate::api::transport::TransportSettings;

/// 一个可跳过片段。字段语义见 `bili_domain::sponsor_block::SkipSegment`。
#[derive(Debug, Clone)]
pub struct SkipSegment {
    pub uuid: String,
    /// 分类名（`sponsor` / `selfpromo` / `interaction` …）。
    ///
    /// 刻意**不是**枚举：服务端随时会加新分类，收成枚举会让新分类变成
    /// 解析错误，整条列表拿不到。
    pub category: String,
    pub action_type: Option<String>,
    /// 起止（毫秒）。
    pub start_ms: i64,
    pub end_ms: i64,
    pub video_duration_ms: Option<f64>,
    pub votes: Option<i64>,
    pub cid: Option<String>,
}

/// 提交者统计。
#[derive(Debug, Clone)]
pub struct SponsorUserInfo {
    pub view_count: i64,
    pub minutes_saved: f64,
    pub segment_count: i64,
}

/// 片段列表的结果。
#[derive(Debug, Clone)]
pub struct SkipSegmentsResult {
    pub data: Option<Vec<SkipSegment>>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    /// HTTP 状态码。**404 表示这个视频没有片段**，调用点应当静默处理。
    pub error_code: Option<i64>,
}

/// 统计信息的结果。
#[derive(Debug, Clone)]
pub struct SponsorUserInfoResult {
    pub data: Option<SponsorUserInfo>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

/// 只关心成败的结果（服务器在线探测）。
#[derive(Debug, Clone)]
pub struct SponsorStatusResult {
    pub ok: bool,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

/// 返回一个字符串的结果（绑定的 YouTube id）。
#[derive(Debug, Clone)]
pub struct SponsorTextResult {
    pub data: Option<String>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

/// 一次请求所需的公共参数。
///
/// 单独成结构而不是摊平成七八个参数：本模块有四个导出函数，每个都要这
/// 一组，摊平的话加一个字段要改四处签名，还得同步改 Dart 的四个调用点。
#[derive(Debug, Clone)]
pub struct SponsorBlockContext {
    /// 用户配置的服务器地址（基线 `Pref.blockServer`）。
    pub base_url: String,
    /// 请求头（`origin`、`x-ext-version`，debug 构建下基线不带）。
    pub headers: Vec<(String, String)>,
    pub timeout_ms: u64,
    pub transport: TransportSettings,
}

impl SponsorBlockContext {
    fn split(self) -> (String, RequestContext, ReqwestTransport) {
        let transport = ReqwestTransport::new(self.transport.to_config());
        let ctx = RequestContext {
            // 基地址在这个 API 里是整段可配的，不是路径前缀，因此单独传，
            // RequestContext 里留空。
            base_url: String::new(),
            // SponsorBlock 不认 bilibili 的账号体系，没有 Cookie 可带。
            cookie: None,
            headers: self.headers,
            timeout: Duration::from_millis(self.timeout_ms),
        };
        (self.base_url, ctx, transport)
    }
}

/// 取一个视频的可跳过片段。
pub async fn fetch_skip_segments(
    context: SponsorBlockContext,
    bvid: String,
    cid: i64,
) -> SkipSegmentsResult {
    let (base_url, ctx, transport) = context.split();
    let result = init::runtime()
        .spawn(async move {
            sponsor_block::fetch_skip_segments(&transport, &ctx, &base_url, &bvid, cid).await
        })
        .await;

    match flatten(result) {
        Ok(segments) => SkipSegmentsResult {
            data: Some(segments.into_iter().map(SkipSegment::from_domain).collect()),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => SkipSegmentsResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 取提交者统计。`values` 是要查询的字段名。
pub async fn fetch_sponsor_user_info(
    context: SponsorBlockContext,
    user_id: String,
    values: Vec<String>,
) -> SponsorUserInfoResult {
    let (base_url, ctx, transport) = context.split();
    let result = init::runtime()
        .spawn(async move {
            sponsor_block::fetch_user_info(&transport, &ctx, &base_url, &user_id, &values).await
        })
        .await;

    match flatten(result) {
        Ok(info) => SponsorUserInfoResult {
            data: Some(SponsorUserInfo {
                view_count: info.view_count,
                minutes_saved: info.minutes_saved,
                segment_count: info.segment_count,
            }),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => SponsorUserInfoResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 探测服务器是否在线。
pub async fn fetch_sponsor_uptime(context: SponsorBlockContext) -> SponsorStatusResult {
    let (base_url, ctx, transport) = context.split();
    let result = init::runtime()
        .spawn(async move { sponsor_block::fetch_uptime_status(&transport, &ctx, &base_url).await })
        .await;

    match flatten(result) {
        Ok(()) => SponsorStatusResult {
            ok: true,
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => SponsorStatusResult {
            ok: false,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 查稿件绑定的 YouTube 视频 id。
pub async fn fetch_port_video(
    context: SponsorBlockContext,
    bvid: String,
    cid: i64,
) -> SponsorTextResult {
    let (base_url, ctx, transport) = context.split();
    let result = init::runtime()
        .spawn(async move {
            sponsor_block::fetch_port_video(&transport, &ctx, &base_url, &bvid, cid).await
        })
        .await;

    match flatten(result) {
        Ok(id) => SponsorTextResult {
            data: Some(id),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => SponsorTextResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 把 `JoinHandle` 的两层结果压平。
///
/// 任务 panic 或运行时关闭时**不**把 panic 传给 Dart：宿主 App 不该因为
/// 一条可降级的取数路径而崩溃。
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

impl SkipSegment {
    pub(crate) fn from_domain(s: bili_domain::sponsor_block::SkipSegment) -> Self {
        Self {
            uuid: s.uuid,
            category: s.category,
            action_type: s.action_type,
            start_ms: s.start_ms,
            end_ms: s.end_ms,
            video_duration_ms: s.video_duration_ms,
            votes: s.votes,
            cid: s.cid,
        }
    }
}
