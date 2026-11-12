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

/// 待发送的 HTTP 请求。只支持 GET——本 crate 当前只迁移了只读接口。
///
/// 加 `post` 时按 ADR-013（`docs/architecture/adr/ADR-013-write-operations.md`，
/// issue #50）决策一的形状来：`HttpPost` 带 `body: PostBody`（Form / Json
/// 两种，不能统一）与 [`Idempotency`]，重试走 [`should_retry`]——**不要**
/// 让它共用 `HttpGet` 的那条只读规则。
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

/// 写操作重发安全不安全（ADR-013 决策二）。
///
/// 它**进签名**而不是进文档里的一张表：表格会过期而代码照样编译，放进
/// 签名漏填就编译不过，改错了 code review 看得见。
///
/// 定级判据三问（ADR-013 决策三）：
/// 1. 它是不是真的发请求？有的写操作未登录时只改本地，那不归这里；
/// 2. 服务端语义是「置为」还是「加一次」？置为 → [`Self::Idempotent`]；
/// 3. 重发前查得到上一次生没生效吗？查得到 → [`Self::Verifiable`]，
///    查不到或重复的后果用户直接看得见 → [`Self::Unsafe`]。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Idempotency {
    /// 重发无副作用：服务端语义是「把状态置为 X」而不是「加一次 X」。
    /// 例：`likeVideo` 的参数是 `like: '0'/'1'`，不是 toggle。
    Idempotent,
    /// 重发会多算，但重发前**查得到**上一次是否已生效。
    /// 例：投币——会多扣硬币，但本稿已投几枚是查得到的。
    Verifiable,
    /// 重发会产生用户可见的重复内容或不可逆后果，且无从判定。
    /// 例：发评论、发弹幕、上传图片、清空收藏夹。
    Unsafe,
}

/// 一次请求失败后允许怎么重试。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetryPolicy {
    /// 只读请求。
    Read,
    /// 写操作，按幂等级别折算。
    Write(Idempotency),
}

impl Idempotency {
    /// 折算成重试策略。
    #[must_use]
    pub const fn retry_policy(self) -> RetryPolicy {
        RetryPolicy::Write(self)
    }
}

/// 一次失败「走到哪一步」了。
///
/// 传输实现负责从自己的错误类型判出形态，重试判定只看这个枚举——于是
/// [`should_retry`] 是纯函数，能被逐格钉死，而不必去构造一个
/// `reqwest::Error`（那玩意儿从外部几乎造不出来，规则也就没人验过）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureShape {
    /// 请求**没发出去**：连接没建立、DNS 失败、连接阶段超时。
    /// 重发一定安全。
    NotSent,
    /// 请求可能**已经送达**，但没拿回完整响应：整体超时。
    /// 服务端可能已经处理过了。
    NoResponse,
    /// 其余失败：解码失败、非法 URL、读响应体时断开。重发没有意义。
    ///
    /// 基线的 `RetryInterceptor` 会重试 `DioExceptionType.unknown`，这里
    /// 刻意不跟——reqwest 把「已经发出请求、读响应体时断开」也归在这一
    /// 类，对一个可能已被服务端处理的请求重发，哪怕只读也平白加一次
    /// 风控计数。
    Other,
}

/// 这次失败值不值得重试（ADR-013 决策一）。
///
/// | 策略 | `NotSent` | `NoResponse` | `Other` |
/// | --- | --- | --- | --- |
/// | `Read` | 重试 | 重试 | 否 |
/// | `Write(Idempotent)` | 重试 | **否** | 否 |
/// | `Write(Verifiable \| Unsafe)` | **否** | 否 | 否 |
///
/// 两处值得解释：
///
/// - **幂等写不重试超时。** 幂等保证的是「状态一样」，不保证「不被风控
///   计数」，而写操作的风控计数是**账号级**的，代价比只读高一个量级。
///   基线的 `RetryInterceptor` 重试超时，这里刻意不跟——这是两条路径行为
///   不一致的一处，写下来免得被当成 bug 修回去。
/// - **`Unsafe` 连 `NotSent` 都不重试**，尽管技术上它安全。因为那份「安全」
///   完全依赖于传输实现对错误分类边界的判断没出错，而猜错的代价是用户发出
///   两条一样的评论。把判断压缩成一个 `if`，实现里就没有第三条路可以走错。
///
/// 拿到了响应（哪怕是 412、503）一律不走这里：那是业务与状态码的事，由
/// [`check_status`] 与调用方决定，在传输层重试会把一次风控拦截变成三次。
#[must_use]
pub const fn should_retry(policy: RetryPolicy, shape: FailureShape) -> bool {
    matches!(
        (policy, shape),
        (
            RetryPolicy::Read,
            FailureShape::NotSent | FailureShape::NoResponse
        ) | (
            RetryPolicy::Write(Idempotency::Idempotent),
            FailureShape::NotSent
        )
    )
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

#[cfg(test)]
mod tests {
    use super::{FailureShape, Idempotency, RetryPolicy, should_retry};

    /// 逐格钉死 ADR-013 决策一那张表。写成全表而不是只测几条：
    /// 这里每放开一格，代价都是用户可见的重复提交。
    #[test]
    fn retry_table_matches_adr_013() {
        use FailureShape::{NoResponse, NotSent, Other};
        use Idempotency::{Idempotent, Unsafe, Verifiable};
        use RetryPolicy::{Read, Write};

        let table = [
            // 只读：与基线 RetryInterceptor 一致。
            (Read, NotSent, true),
            (Read, NoResponse, true),
            (Read, Other, false),
            // 幂等写：只重试「没发出去」。超时不重试——风控计数是账号级的。
            (Write(Idempotent), NotSent, true),
            (Write(Idempotent), NoResponse, false),
            (Write(Idempotent), Other, false),
            // 可判定与不可重发：一律不重试，连 NotSent 都不放。
            (Write(Verifiable), NotSent, false),
            (Write(Verifiable), NoResponse, false),
            (Write(Verifiable), Other, false),
            (Write(Unsafe), NotSent, false),
            (Write(Unsafe), NoResponse, false),
            (Write(Unsafe), Other, false),
        ];

        for (policy, shape, want) in table {
            assert_eq!(
                should_retry(policy, shape),
                want,
                "{policy:?} + {shape:?} 应当是 {want}"
            );
        }
    }

    /// 只读路径的行为必须与拆分前**完全一致**：拆分是为了给写操作让出
    /// 一条不同的规则，不是顺手改只读的行为。拆分前是
    /// `e.is_connect() || e.is_timeout()`，对应这两格。
    #[test]
    fn read_policy_is_unchanged_by_the_split() {
        assert!(should_retry(RetryPolicy::Read, FailureShape::NotSent));
        assert!(should_retry(RetryPolicy::Read, FailureShape::NoResponse));
        assert!(!should_retry(RetryPolicy::Read, FailureShape::Other));
    }

    #[test]
    fn idempotency_folds_into_a_write_policy() {
        assert_eq!(
            Idempotency::Idempotent.retry_policy(),
            RetryPolicy::Write(Idempotency::Idempotent)
        );
    }
}
