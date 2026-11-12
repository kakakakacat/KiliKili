//! 基于 reqwest 的传输实现。
//!
//! TLS 用 rustls + webpki-roots（见 workspace 的依赖声明）：Android NDK
//! 上不必准备 OpenSSL，证书根也不随各平台系统信任库而变——五端行为一致
//! 比「跟随系统」更重要，否则同一个请求在某一端失败时无从复现。
//!
//! ## 与基线 Dio 的对齐（issue #25）
//!
//! 基线里影响「请求怎么发」的四项开关——重试、系统代理、证书豁免、
//! HTTP/1.1——在这里都由 [`TransportConfig`] 承载，值仍由 Dart 的 `Pref`
//! 给出。未对齐之前，开了这些开关的用户永远跑不到 Rust 路径（取数失败
//! → 降级回 Dart），等于这条路径对他们不存在。
//!
//! 仍未覆盖的差异，均属「Rust 接管传输」的后续工作，不在只读接口里顺手
//! 引入：
//!
//! - **HTTP/2 连接池的网络切换重建**。基线在 iOS 上监听
//!   `onConnectivityChanged` 重建连接池；reqwest 的连接池自己会淘汰失效
//!   连接，差别只是切网后的首个请求可能多一次超时——重试会接住它。
//! - **重定向**。基线的 `RetryInterceptor` 手工处理 3xx；reqwest 默认
//!   跟随最多 10 次重定向，只读接口上两者等价。
//! - **`autoUncompress = false` + 自行解压**。基线这么做是为了统一
//!   Http2Adapter 与 IOHttpClientAdapter 的行为；reqwest 的 gzip/br 解压
//!   是内建的，无须对齐。

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use bili_domain::error::{DomainError, ErrorKind};

use crate::http::{
    FailureShape, HttpGet, HttpPost, HttpResponse, HttpTransport, PostBody, ProxyConfig,
    RetryPolicy, TransportConfig, should_retry,
};

/// 连接超时。基线 `BaseOptions.connectTimeout` 为 10s；请求整体超时由
/// 调用方按请求给出（[`HttpGet::timeout`]），两者不是一回事。
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// 影响 `reqwest::Client` 构造的那部分配置。
///
/// 重试不在其中：它是每次请求的行为，不需要一个独立的客户端。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ClientKey {
    proxy: Option<(String, u16)>,
    accept_invalid_certs: bool,
    http1_only: bool,
}

impl ClientKey {
    fn of(config: &TransportConfig) -> Self {
        Self {
            proxy: config.proxy.as_ref().map(|p| (p.host.clone(), p.port)),
            accept_invalid_certs: config.accept_invalid_certs,
            http1_only: config.http1_only,
        }
    }
}

/// 按配置缓存的 reqwest 客户端。
///
/// 必须复用：每次新建 `Client` 会丢掉连接池与 TLS 会话，移动网络下每个
/// 请求都要重做握手。按配置分桶而非只留一个：代理与证书开关是构造期
/// 参数，改了就得换客户端；而用户在设置页改一次开关的频率极低，桶数
/// 实际上恒为个位数。
static CLIENT_CACHE: OnceLock<Mutex<HashMap<ClientKey, reqwest::Client>>> = OnceLock::new();

fn client(config: &TransportConfig) -> Result<reqwest::Client, DomainError> {
    let cache = CLIENT_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let key = ClientKey::of(config);

    // 锁被毒化说明此前有线程在持锁时 panic。缓存里只有客户端，没有需要
    // 维持一致性的状态，继续用里面的内容是安全的；为此让取数路径整个
    // 挂掉才是更糟的选择。
    let mut guard = cache.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(existing) = guard.get(&key) {
        return Ok(existing.clone());
    }

    let built = build_client(config).map_err(|e| {
        // 构建失败通常意味着 TLS 后端没初始化起来或代理地址非法，属环境
        // 问题——分类成 Network 让调用方可以重试/降级。
        DomainError::new(ErrorKind::Network, format!("HTTP 客户端不可用: {e}"))
    })?;
    guard.insert(key, built.clone());
    Ok(built)
}

fn build_client(config: &TransportConfig) -> Result<reqwest::Client, reqwest::Error> {
    let mut builder = reqwest::Client::builder()
        // 连接池空闲超时与基线 (`lib/http/init.dart`) 保持一致。
        .pool_idle_timeout(Duration::from_secs(15))
        .connect_timeout(CONNECT_TIMEOUT);

    if config.http1_only {
        builder = builder.http1_only();
    }
    if config.accept_invalid_certs {
        // 仅在调用方明确要求时开启（基线：开了系统代理，或用户自行打开
        // 了证书豁免开关）。它关掉的是中间人防护，不该有任何隐式触发。
        builder = builder.danger_accept_invalid_certs(true);
    }
    if let Some(ProxyConfig { host, port }) = &config.proxy {
        builder = builder.proxy(reqwest::Proxy::all(format!("http://{host}:{port}"))?);
    }

    builder.build()
}

/// reqwest 传输。
#[derive(Debug, Clone, Default)]
pub struct ReqwestTransport {
    config: TransportConfig,
}

impl ReqwestTransport {
    #[must_use]
    pub const fn new(config: TransportConfig) -> Self {
        Self { config }
    }
}

impl HttpTransport for ReqwestTransport {
    async fn get(&self, request: HttpGet) -> Result<HttpResponse, DomainError> {
        let client = client(&self.config)?;

        // GET 是只读的，走 [`RetryPolicy::Read`]：连接失败与超时都重试，
        // 与基线 `RetryInterceptor` 一致。写操作**不是**这条规则，见
        // [`crate::http::should_retry`] 的表。
        let mut attempt: u32 = 0;
        loop {
            match send_once(&client, &request).await {
                Ok(resp) => return Ok(resp),
                Err(e) => {
                    if attempt >= self.config.retry_count
                        || !should_retry(RetryPolicy::Read, failure_shape(&e))
                    {
                        return Err(map_reqwest_error(&e));
                    }
                    attempt += 1;
                    let delay = backoff_delay(attempt, self.config.retry_delay_ms);
                    tracing::debug!(attempt, delay_ms = delay.as_millis(), "重试请求");
                    tokio::time::sleep(delay).await;
                }
            }
        }
    }

    async fn post(&self, request: HttpPost) -> Result<HttpResponse, DomainError> {
        let client = client(&self.config)?;

        // 与 GET 唯一的区别就在这个 policy 上：写请求按它自己的幂等级别
        // 决定能不能重发，**不要**改成 RetryPolicy::Read（ADR-013 决策一）。
        let policy = request.idempotency.retry_policy();

        let mut attempt: u32 = 0;
        loop {
            match send_post_once(&client, &request).await {
                Ok(resp) => return Ok(resp),
                Err(e) => {
                    if attempt >= self.config.retry_count
                        || !should_retry(policy, failure_shape(&e))
                    {
                        return Err(map_reqwest_error(&e));
                    }
                    attempt += 1;
                    let delay = backoff_delay(attempt, self.config.retry_delay_ms);
                    tracing::debug!(attempt, delay_ms = delay.as_millis(), ?policy, "重试写请求");
                    tokio::time::sleep(delay).await;
                }
            }
        }
    }
}

async fn send_post_once(
    client: &reqwest::Client,
    request: &HttpPost,
) -> Result<HttpResponse, reqwest::Error> {
    let mut builder = client.post(&request.url).timeout(request.timeout);
    for (k, v) in &request.headers {
        builder = builder.header(k, v);
    }
    builder = match &request.body {
        // reqwest 的 .form() 自己带 application/x-www-form-urlencoded，
        // .json() 带 application/json——与基线调用点显式写的那两个
        // content-type 一致，不必再手工设头。
        PostBody::Form(fields) => builder.form(fields),
        PostBody::Json(text) => builder
            .header("content-type", "application/json")
            .body(text.clone()),
    };

    let resp = builder.send().await?;
    let status = resp.status().as_u16();
    let body = resp.text().await?;

    Ok(HttpResponse { status, body })
}

async fn send_once(
    client: &reqwest::Client,
    request: &HttpGet,
) -> Result<HttpResponse, reqwest::Error> {
    let mut builder = client.get(&request.url).timeout(request.timeout);
    for (k, v) in &request.headers {
        builder = builder.header(k, v);
    }

    let resp = builder.send().await?;
    let status = resp.status().as_u16();
    // 先取 body 再判状态：风控与业务错误都会在非 2xx 的响应体里给出
    // 原因，丢掉它等于把唯一的线索扔了。
    let body = resp.text().await?;

    Ok(HttpResponse { status, body })
}

/// 第 `attempt` 次重试前等多久。
///
/// 线性递增（1x、2x、3x……），与基线 `RetryInterceptor` 的
/// `++_rt * delay` 一致。不改成指数退避：基线的节奏是用户可感知的行为，
/// 两条路径在同一次网络抖动下的表现应当一样。
fn backoff_delay(attempt: u32, base_ms: u64) -> Duration {
    Duration::from_millis(base_ms.saturating_mul(u64::from(attempt)))
}

/// reqwest 的错误属于哪种失败形态。
///
/// 判定规则本身在 [`crate::http::should_retry`]，这里只负责把 reqwest 的
/// 分类翻译过去。
///
/// **`is_connect()` 要先判**：连接阶段超时在 reqwest 里 `is_connect()` 与
/// `is_timeout()` 同时为真，顺序反了就会把「没发出去」误判成「可能已送达」，
/// 白白放弃一次安全的重试。连接阶段之所以分得出来，是因为客户端显式设了
/// [`CONNECT_TIMEOUT`]（见 [`build_client`]）；去掉它，整体 `.timeout()`
/// 会把两者糊成一类，ADR-013 决策一那条分界就实现不了了。
fn failure_shape(e: &reqwest::Error) -> FailureShape {
    if e.is_connect() {
        FailureShape::NotSent
    } else if e.is_timeout() {
        FailureShape::NoResponse
    } else {
        FailureShape::Other
    }
}

/// reqwest 错误到领域错误的映射。
///
/// 错误信息里**不含**请求头，只含 reqwest 自己的描述与 URL——URL 带查询
/// 参数，其中有 `w_rid`/`wts` 但没有凭据（凭据在头里）。
fn map_reqwest_error(e: &reqwest::Error) -> DomainError {
    let kind = if e.is_decode() {
        ErrorKind::Protocol
    } else {
        ErrorKind::Network
    };
    DomainError::new(kind, e.to_string())
}

#[cfg(test)]
// 测试里 expect 是正确做法：前提不成立时就该崩掉并指出原因。
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    fn cfg() -> TransportConfig {
        TransportConfig::default()
    }

    /// 当前缓存里有几个客户端。只在测试中使用。
    fn cached_client_count() -> usize {
        CLIENT_CACHE
            .get()
            .map_or(0, |c| c.lock().unwrap_or_else(|e| e.into_inner()).len())
    }

    #[test]
    fn defaults_match_baseline() {
        let c = cfg();
        assert_eq!(c.retry_count, 2, "基线 Pref.retryCount 默认 2");
        assert_eq!(c.retry_delay_ms, 500, "基线 Pref.retryDelay 默认 500ms");
        assert!(
            c.http1_only,
            "基线 Pref.enableHttp2 默认 false，Rust 侧不对齐就会偷偷用 h2"
        );
        assert!(!c.accept_invalid_certs);
        assert!(c.proxy.is_none());
    }

    #[test]
    fn clients_are_reused_per_config() {
        // 同一份配置必须命中同一个客户端：每次新建会丢掉连接池与 TLS
        // 会话，移动网络下代价是每个请求重做一次握手。
        let _ = client(&cfg()).expect("默认配置可构造客户端");
        let before = cached_client_count();
        let _ = client(&cfg()).expect("默认配置可构造客户端");
        assert_eq!(before, cached_client_count(), "相同配置不应再建客户端");
        assert_eq!(ClientKey::of(&cfg()), ClientKey::of(&cfg()));
    }

    #[test]
    fn backoff_matches_baseline_cadence() {
        // 基线 RetryInterceptor：第 n 次重试等 n * delay。
        assert_eq!(backoff_delay(1, 500), Duration::from_millis(500));
        assert_eq!(backoff_delay(2, 500), Duration::from_millis(1000));
        assert_eq!(backoff_delay(3, 500), Duration::from_millis(1500));
        // 溢出只可能来自荒唐的配置值，不该因此 panic。
        assert_eq!(
            backoff_delay(u32::MAX, u64::MAX),
            Duration::from_millis(u64::MAX)
        );
    }

    #[test]
    fn differing_transport_switches_use_different_clients() {
        let proxied = TransportConfig {
            proxy: Some(ProxyConfig {
                host: "127.0.0.1".to_owned(),
                port: 8888,
            }),
            ..cfg()
        };
        assert_ne!(
            ClientKey::of(&cfg()),
            ClientKey::of(&proxied),
            "代理是构造期参数，不能共用客户端"
        );
        assert!(build_client(&proxied).is_ok(), "代理地址应当可被接受");
    }

    #[test]
    fn invalid_proxy_is_a_network_error_not_a_panic() {
        let bad = TransportConfig {
            proxy: Some(ProxyConfig {
                host: "not a host".to_owned(),
                port: 0,
            }),
            ..cfg()
        };
        // 代理地址非法时只能失败，但必须是可降级的错误，不能 panic 掉
        // 宿主 App。
        if let Err(e) = client(&bad) {
            assert_eq!(e.kind(), ErrorKind::Network);
        }
    }
}
