//! 基于 reqwest 的传输实现。
//!
//! TLS 用 rustls + webpki-roots（见 workspace 的依赖声明）：Android NDK
//! 上不必准备 OpenSSL，证书根也不随各平台系统信任库而变——五端行为一致
//! 比「跟随系统」更重要，否则同一个请求在某一端失败时无从复现。
//!
//! 代理、证书豁免、HTTP/1.1 回退等基线里存在的开关（`lib/http/init.dart`）
//! 暂未接：它们改变的是安全边界，应当随「Rust 接管传输」整体迁移，而不是
//! 由一个只读接口顺手引入。在用户开了这些开关的环境里，Rust 路径会取数
//! 失败并降级回 Dart——这正是功能开关的作用。

use std::sync::OnceLock;
use std::time::Duration;

use bili_domain::error::{DomainError, ErrorKind};

use crate::http::{HttpGet, HttpResponse, HttpTransport};

/// 共享的 reqwest 客户端。
///
/// 必须复用：每次新建 `Client` 会丢掉连接池与 TLS 会话，移动网络下每个
/// 请求都要重做握手。
fn client() -> Result<&'static reqwest::Client, DomainError> {
    static CLIENT: OnceLock<Result<reqwest::Client, String>> = OnceLock::new();
    CLIENT
        .get_or_init(|| {
            reqwest::Client::builder()
                // 连接池空闲超时与基线 (`lib/http/init.dart`) 保持一致。
                .pool_idle_timeout(Duration::from_secs(15))
                .build()
                .map_err(|e| e.to_string())
        })
        .as_ref()
        .map_err(|e| {
            // 构建失败通常意味着 TLS 后端没初始化起来，属环境问题，
            // 不是本次请求的问题——分类成 Network 让调用方可以重试/降级。
            DomainError::new(ErrorKind::Network, format!("HTTP 客户端不可用: {e}"))
        })
}

/// reqwest 传输。无状态，克隆廉价。
#[derive(Debug, Clone, Copy, Default)]
pub struct ReqwestTransport;

impl HttpTransport for ReqwestTransport {
    async fn get(&self, request: HttpGet) -> Result<HttpResponse, DomainError> {
        let mut builder = client()?.get(&request.url).timeout(request.timeout);
        for (k, v) in &request.headers {
            builder = builder.header(k, v);
        }

        let resp = builder.send().await.map_err(map_reqwest_error)?;
        let status = resp.status().as_u16();
        // 先取 body 再判状态：风控与业务错误都会在非 2xx 的响应体里给出
        // 原因，丢掉它等于把唯一的线索扔了。
        let body = resp.text().await.map_err(map_reqwest_error)?;

        Ok(HttpResponse { status, body })
    }
}

/// reqwest 错误到领域错误的映射。
///
/// 错误信息里**不含**请求头，只含 reqwest 自己的描述与 URL——URL 带查询
/// 参数，其中有 `w_rid`/`wts` 但没有凭据（凭据在头里）。
fn map_reqwest_error(e: reqwest::Error) -> DomainError {
    let kind = if e.is_timeout() || e.is_connect() || e.is_request() {
        ErrorKind::Network
    } else if e.is_decode() {
        ErrorKind::Protocol
    } else {
        ErrorKind::Network
    };
    DomainError::new(kind, e.to_string())
}
