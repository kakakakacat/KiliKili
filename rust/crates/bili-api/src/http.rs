//! 传输抽象与请求上下文。
//!
//! 传输是一个 Port（RFC-0001 §16.6）：业务代码依赖 [`HttpTransport`]，
//! 具体实现（当前是 reqwest）可替换，也可以在测试里换成桩件——接口迁移
//! 的测试不该需要网络。
//!
//! ## 凭据处理
//!
//! Rust 侧**不保存任何凭据**：Cookie、buvid、access_key 的真相源仍在
//! Dart（`lib/utils/accounts/`）。调用方每次把当次请求所需的头传进来，
//! 用完即弃。这样不会出现「同一账号状态两份」的问题——账号切换、Cookie
//! 刷新、登出全部只发生在一个地方。
//!
//! 凭据用 [`Redacted`] 包裹，它的 `Debug`/`Display` 只打印 `<redacted>`，
//! 因此误把 [`RequestContext`] 整个写进日志也不会泄漏（RFC §2.2）。

use std::time::Duration;

use bili_domain::error::{DomainError, ErrorKind};
use bili_domain::redact::{Redacted, is_credential_key};

/// 一次请求所需的上下文。由调用方（Stage 1 是 Dart）按请求提供。
#[derive(Debug, Clone)]
pub struct RequestContext {
    /// 接口基地址，如 `https://api.bilibili.com`。
    ///
    /// 由调用方给出而非写死：基线把它放在 `lib/http/constants.dart`，
    /// 两处各写一份迟早会漂移。
    pub base_url: String,
    /// `Cookie` 头的完整值。未登录时为 `None`。
    pub cookie: Option<Redacted<String>>,
    /// 其余请求头（User-Agent、Referer、x-bili-* 等），由 Dart 侧的账号
    /// 体系产出，这里原样透传。
    pub headers: Vec<(String, String)>,
    pub timeout: Duration,
}

impl RequestContext {
    /// 组装最终请求头。Cookie 单独放是为了让它在类型上就与普通头不同，
    /// 不会被顺手打印。
    #[must_use]
    pub fn all_headers(&self) -> Vec<(String, String)> {
        let mut out = self.headers.clone();
        if let Some(cookie) = &self.cookie {
            out.push(("cookie".to_owned(), cookie.expose().clone()));
        }
        out
    }

    /// 可安全写进日志的头部摘要：凭据类字段只留键名。
    #[must_use]
    pub fn redacted_headers(&self) -> Vec<String> {
        self.all_headers()
            .iter()
            .map(|(k, v)| {
                if is_credential_key(k) {
                    format!("{k}: <redacted>")
                } else {
                    format!("{k}: {v}")
                }
            })
            .collect()
    }
}

/// 待发送的 HTTP 请求。只支持 GET——本 crate 当前只迁移了只读接口，
/// 写操作要先有幂等与重试策略，不该顺手打开。
///
/// 那套策略现在有地方写了：`docs/architecture/adr/ADR-013-write-operations.md`
/// （issue #50）。**它还是 Proposed**，没批准之前不要加 `post`——尤其注意
/// ADR 决策一指出的那处：现在的 [`crate::transport_reqwest`] 对超时也重试，
/// 那条规则对写操作不成立。
#[derive(Debug, Clone)]
pub struct HttpGet {
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub timeout: Duration,
}

/// 传输层返回的原始响应。
#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status: u16,
    pub body: String,
}

/// 传输 Port。
pub trait HttpTransport {
    /// 发送 GET 请求。
    ///
    /// 实现方只负责把字节取回来：**不要**在这里解释业务 code，也不要
    /// 重试。错误一律映射为 [`ErrorKind::Network`]（连接、超时、TLS）
    /// 或 [`ErrorKind::Cancelled`]。
    fn get(
        &self,
        request: HttpGet,
    ) -> impl Future<Output = Result<HttpResponse, DomainError>> + Send;
}

/// HTTP 状态码到错误大类的映射。
///
/// 2xx 由调用方继续解析业务信封；这里只处理传输层面的失败。
///
/// # Errors
/// 非 2xx 一律为错误。
pub fn check_status(status: u16) -> Result<(), DomainError> {
    match status {
        200..=299 => Ok(()),
        // 412 是风控拦截的常见形态：重试只会继续被拦，必须走验证流程。
        401 | 403 | 412 => Err(DomainError::new(
            ErrorKind::Auth,
            format!("HTTP {status}：被拒绝或触发风控"),
        )),
        // 429 与 5xx 是暂时性的，归入可重试。
        429 | 500..=599 => Err(DomainError::new(
            ErrorKind::Remote,
            format!("HTTP {status}：服务端暂时不可用"),
        )),
        _ => Err(DomainError::new(
            ErrorKind::Protocol,
            format!("HTTP {status}：非预期状态码"),
        )),
    }
}

/// 传输层配置。对应基线 Dio 里那组影响「请求怎么发」的开关
/// （`lib/http/init.dart`、`lib/http/retry_interceptor.dart`）。
///
/// 由调用方按请求传入而非在 Rust 侧读配置：这些开关的真相源是 Dart 的
/// `Pref`（用户在设置页改的就是它），Rust 再存一份就会出现「设置改了但
/// Rust 路径还用旧值」的问题——与凭据不落 Rust 是同一个理由。
///
/// 默认值刻意与基线的默认值一致，见各字段注释。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransportConfig {
    /// 重试次数。基线 `Pref.retryCount` 默认 2；0 表示不重试。
    pub retry_count: u32,
    /// 重试基础间隔（毫秒）。第 n 次重试等待 `n * retry_delay_ms`，
    /// 与基线 `RetryInterceptor` 的递增间隔一致。
    pub retry_delay_ms: u64,
    /// 系统代理。`None` 表示直连。
    pub proxy: Option<ProxyConfig>,
    /// 接受无效证书。
    ///
    /// 基线在开了系统代理时**无条件**接受（抓包场景），另有一个独立开关
    /// `Pref.badCertificateCallback`。两者都由调用方折算成这一个布尔值。
    pub accept_invalid_certs: bool,
    /// 只用 HTTP/1.1。
    ///
    /// 基线默认 `Pref.enableHttp2 == false`，即默认就走 HTTP/1.1；而
    /// reqwest 默认经 ALPN 协商 h2。不显式对齐的话，Rust 路径会在用户
    /// 没开 HTTP/2 时偷偷用 h2——这正是「Rust 路径与基线行为不一致」
    /// 最隐蔽的一种。
    pub http1_only: bool,
}

/// 代理配置。只支持 HTTP 代理：基线的 `findProxy` 返回的也是
/// `PROXY host:port`，没有 SOCKS 分支。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProxyConfig {
    pub host: String,
    pub port: u16,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            retry_count: 2,
            retry_delay_ms: 500,
            proxy: None,
            accept_invalid_certs: false,
            // 与基线 `Pref.enableHttp2` 的默认值（false）对应。
            http1_only: true,
        }
    }
}
