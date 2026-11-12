//! 视频详情的 Rust 取数路径（HTTP 由 Rust 发起）。
//!
//! 与同目录的 `video_detail.rs`（只解析）是**两条独立路径**：
//!
//! | | 发请求 | 解析 | 用途 |
//! |---|---|---|---|
//! | `parse_video_detail` | Dart | Rust | shadow mode 比对，显示仍用 Dart |
//! | `fetch_video_detail` | Rust | Rust | 功能开关打开时的取数路径 |
//!
//! 两者不会同时作用于一次请求：shadow 比对的是 Dart 取回的响应，取数路径
//! 走通时根本不经过 Dart 的 Dio。
//!
//! ## 凭据
//!
//! Rust **不保存**任何凭据。Cookie 与账号头由 Dart 按请求传入，用完即弃；
//! 账号切换、Cookie 刷新、登出仍只发生在 Dart 一侧，不存在两份账号状态。
//! 传入的 Cookie 立即用 `Redacted` 包裹，日志与 Debug 输出都只会看到
//! `<redacted>`。
//!
//! ## 传输设置
//!
//! 重试、系统代理、证书豁免、HTTP/1.1 由调用方按请求传入
//! （[`TransportSettings`]），值取自 Dart 的 `Pref`——即基线 Dio 用的
//! 同一批开关（issue #25）。仍未对齐的差异见
//! `bili_api::transport_reqwest` 的模块文档。

use std::time::Duration;

use bili_api::client;
use bili_api::http::RequestContext;
use bili_api::transport_reqwest::ReqwestTransport;
use bili_domain::redact::Redacted;

use crate::api::init;
use crate::api::transport::TransportSettings;

/// 取数结果。与 `VideoDetailParse` 同形：失败不抛异常，由 Dart 决定降级。
#[derive(Debug, Clone)]
pub struct VideoDetailFetch {
    /// 规范化后的 JSON。失败时为 `None`。
    pub json: Option<String>,
    /// 失败大类：`network|remote|protocol|auth|storage|player|cancelled`。
    pub error_kind: Option<String>,
    /// 失败详情。**不含凭据**，也不含完整 URL。
    pub error_detail: Option<String>,
}

/// 取一条视频详情。
///
/// `mixin_key` 由 Dart 侧的 WBI key 缓存提供（按日缓存，见 `WbiSign`），
/// `wts` 为当前 Unix 秒——由调用方注入使请求构造保持可被 fixture 固定。
///
/// `headers` 为账号体系产出的请求头（User-Agent、Referer、x-bili-* 等），
/// `cookie` 为完整的 Cookie 头值，未登录时传 `None`。
///
/// `transport` 为本次请求的传输设置，取自 Dart 的 `Pref`。
#[allow(clippy::too_many_arguments)]
pub async fn fetch_video_detail(
    base_url: String,
    bvid: String,
    mixin_key: String,
    wts: i64,
    cookie: Option<String>,
    headers: Vec<(String, String)>,
    timeout_ms: u64,
    transport: TransportSettings,
) -> VideoDetailFetch {
    let transport = ReqwestTransport::new(transport.to_config());
    let ctx = RequestContext {
        base_url,
        cookie: cookie.map(Redacted::new),
        // 只读路径：csrf 只有写操作用得上（ADR-013 决策四）。
        csrf: None,
        headers,
        timeout: Duration::from_millis(timeout_ms),
    };

    // 跑在我们自己的 Tokio 运行时上：reqwest 需要 Tokio 的反应器，而 FRB
    // 的执行器不提供。JoinHandle 本身是普通 Future，可以在 FRB 线程上
    // await。
    let handle = init::runtime().spawn(async move {
        client::fetch_video_detail(&transport, &ctx, &bvid, &mixin_key, wts)
            .await
            .and_then(|detail| {
                serde_json::to_string(&detail).map_err(|e| {
                    bili_domain::error::DomainError::new(
                        bili_domain::error::ErrorKind::Protocol,
                        format!("实体序列化失败: {e}"),
                    )
                })
            })
    });

    match handle.await {
        Ok(Ok(json)) => VideoDetailFetch {
            json: Some(json),
            error_kind: None,
            error_detail: None,
        },
        Ok(Err(e)) => VideoDetailFetch {
            json: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
        },
        // 任务 panic 或运行时正在关闭。不把 panic 传播到 Dart：宿主 App
        // 不该因为一条可降级的取数路径而崩溃。
        Err(e) => VideoDetailFetch {
            json: None,
            error_kind: Some("network".to_owned()),
            error_detail: Some(format!("取数任务未完成: {e}")),
        },
    }
}
