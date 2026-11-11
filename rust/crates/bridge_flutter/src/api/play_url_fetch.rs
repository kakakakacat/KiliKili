//! 取流的 Rust 取数路径（HTTP 由 Rust 发起，issue #79）。
//!
//! **这是取流的唯一实现**：ADR-011 决策四不保留旧实现、不设降级开关，
//! 基线的 `VideoHttp.videoUrl` 连同 `rust.fetch.playUrl` 开关在本切片
//! 一并删除。失败仍然返回一个**值**而不是异常，但 Dart 收到失败后不再
//! 有第二条路可走——它拿 `error_kind` / `error_code` 产出文案。
//!
//! ## 重试分支只剩一份
//!
//! 基线在 Dart 侧有三条重试分支（PGC 内容为 DRM 时改走降级接口、
//! UGC 带 `ep_id` 失败后改按 PGC 重取），Rust 侧另有一份同名分支。
//! 迁移后只保留 Rust 这一份，由 `client::fetch_play_url` 按响应自行
//! 决定——基线的降级同样是响应驱动的，不由调用方指定。
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
use crate::api::play_url::{PlayUrl, play_url};
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

/// 取流结果。失败不抛异常，由 Dart 按 `error_kind` 产出文案。
///
/// 迁移前这里是一个 JSON 字符串 + Dart 侧的 `denormalizePlayUrl` 还原，
/// 现在直接给 typed 结构（ADR-011）。
#[derive(Debug, Clone)]
pub struct PlayUrlResult {
    /// 取流结果。失败时为 `None`。
    pub data: Option<PlayUrl>,
    /// 失败大类：`network|remote|protocol|auth|storage|player|cancelled`。
    pub error_kind: Option<String>,
    /// 失败详情。**不含凭据**，也不含完整 URL。
    pub error_detail: Option<String>,
    /// 服务端业务错误码。仅当失败来自接口信封时才有值。
    ///
    /// 基线 `VideoHttp._parseVideoErr` 的文案就是按这个码分支的
    /// （-404 稿件不存在、87008 需要充电…），迁移后由 Dart 侧的
    /// `playUrlErrorMessage` 接手，码必须原样带过去。
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
) -> PlayUrlResult {
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
    });

    match handle.await {
        Ok(Ok(play)) => PlayUrlResult {
            data: Some(play_url(play)),
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

fn failure(kind: &str, detail: String, code: Option<i64>) -> PlayUrlResult {
    PlayUrlResult {
        data: None,
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
