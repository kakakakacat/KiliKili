//! 取流的 Rust 取数路径（HTTP 由 Rust 发起）。
//!
//! 与 `video_detail_fetch` 同构：开关打开时请求由 Rust 发出并解析，
//! 任何一步失败都返回一个**值**而不是异常，由 Dart 决定降级
//! （RFC-0001 §5.3）。
//!
//! ## 为什么取流比详情更需要「失败即降级」
//!
//! 详情取不到，用户看到的是一个空页面；流取不到，用户看到的是播放器
//! 转圈。两者都不该由一个尚未成为默认路径的组件引发，而取流还多出
//! 几条基线特有的重试分支（DRM 降级、UGC→PGC），任何一条没对齐都会
//! 表现为「某些番剧在 Rust 路径下播不了」——降级把这类差异挡在用户
//! 之外，shadow 与真机验证再把它们找出来。
//!
//! ## 凭据
//!
//! 同 `video_detail_fetch`：Cookie 与账号头按请求传入，Rust 不保存。

use std::time::Duration;

use bili_api::client;
use bili_api::http::RequestContext;
use bili_api::request::PlayUrlParams;
use bili_api::transport_reqwest::ReqwestTransport;
use bili_domain::redact::Redacted;
use bili_domain::video::VideoType;
use bili_protocol::risk::SystemEntropy;

use crate::api::init;
use crate::api::transport::TransportSettings;

/// 取流的业务入参。与基线 `VideoHttp.videoUrl` 的签名一一对应。
#[derive(Debug, Clone)]
pub struct PlayUrlRequest {
    pub avid: Option<i64>,
    pub bvid: Option<String>,
    pub ep_id: Option<i64>,
    pub season_id: Option<i64>,
    pub cid: i64,
    /// 清晰度代码。
    pub qn: i64,
    /// `ugc` / `pgc` / `pugv`，取基线 `VideoType` 的枚举名。
    pub video_type: String,
    /// 未登录试看 1080P。
    pub try_look: bool,
    pub voice_balance: bool,
    /// 多语言音轨（基线 `cur_language`）。
    pub language: Option<String>,
}

/// 取流结果。失败不抛异常，由 Dart 决定降级。
#[derive(Debug, Clone)]
pub struct PlayUrlFetch {
    /// 规范化后的 JSON。失败时为 `None`。
    pub json: Option<String>,
    /// 失败大类：`network|remote|protocol|auth|storage|player|cancelled`。
    pub error_kind: Option<String>,
    /// 失败详情。**不含凭据**，也不含完整 URL。
    pub error_detail: Option<String>,
    /// 服务端业务错误码。仅当失败来自接口信封时才有值。
    ///
    /// 带给 Dart 是为了让它能判断「这是内容本身的问题还是 Rust 路径的
    /// 问题」——前者降级回 Dart 只会再失败一次，但仍照做：错误文案由
    /// 基线统一产出，两条路径给出不同措辞才是更糟的事。
    pub error_code: Option<i64>,
}

/// 取一次流。
///
/// `mixin_key` 由 Dart 的 WBI key 缓存提供，`wts` 为当前 Unix 秒。
/// 风控 token 由 Rust 现生成（每次尝试都重新生成），不经过 Dart：它们
/// 没有任何需要与 Dart 共享的状态。
#[allow(clippy::too_many_arguments)]
pub async fn fetch_play_url(
    base_url: String,
    request: PlayUrlRequest,
    mixin_key: String,
    wts: i64,
    cookie: Option<String>,
    headers: Vec<(String, String)>,
    timeout_ms: u64,
    transport: TransportSettings,
) -> PlayUrlFetch {
    let Some(params) = request.into_params() else {
        return failure("protocol", "未知的 video_type".to_owned(), None);
    };

    let transport = ReqwestTransport::new(transport.to_config());
    let ctx = RequestContext {
        base_url,
        cookie: cookie.map(Redacted::new),
        headers,
        timeout: Duration::from_millis(timeout_ms),
    };

    // 跑在我们自己的 Tokio 运行时上：reqwest 需要 Tokio 的反应器，而 FRB
    // 的执行器不提供。
    let handle = init::runtime().spawn(async move {
        client::fetch_play_url(
            &transport,
            &ctx,
            &params,
            &mut SystemEntropy,
            &mixin_key,
            wts,
        )
        .await
        .and_then(|play| {
            serde_json::to_string(&play).map_err(|e| {
                bili_domain::error::DomainError::new(
                    bili_domain::error::ErrorKind::Protocol,
                    format!("实体序列化失败: {e}"),
                )
            })
        })
    });

    match handle.await {
        Ok(Ok(json)) => PlayUrlFetch {
            json: Some(json),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Ok(Err(e)) => failure(&e.kind().to_string(), e.detail().to_owned(), e.code()),
        // 任务 panic 或运行时正在关闭。不把 panic 传播到 Dart：宿主 App
        // 不该因为一条可降级的取数路径而崩溃。
        Err(e) => failure("network", format!("取数任务未完成: {e}"), None),
    }
}

fn failure(kind: &str, detail: String, code: Option<i64>) -> PlayUrlFetch {
    PlayUrlFetch {
        json: None,
        error_kind: Some(kind.to_owned()),
        error_detail: Some(detail),
        error_code: code,
    }
}

impl PlayUrlRequest {
    /// 转成数据层入参。`None` 表示 `video_type` 无法识别——那是桥接两端
    /// 不同源的信号，不该被当成一次普通的取数失败悄悄咽下。
    fn into_params(self) -> Option<PlayUrlParams> {
        let video_type = match self.video_type.as_str() {
            "ugc" => VideoType::Ugc,
            "pgc" => VideoType::Pgc,
            "pugv" => VideoType::Pugv,
            _ => return None,
        };

        Some(PlayUrlParams {
            avid: self.avid,
            bvid: self.bvid,
            ep_id: self.ep_id,
            season_id: self.season_id,
            cid: self.cid,
            qn: self.qn,
            video_type,
            try_look: self.try_look,
            voice_balance: self.voice_balance,
            language: self.language,
            // 首次一律走主接口；降级由 `client::fetch_play_url` 按响应
            // 自行决定，不由 Dart 指定——基线的降级同样是响应驱动的。
            fallback: false,
        })
    }
}
