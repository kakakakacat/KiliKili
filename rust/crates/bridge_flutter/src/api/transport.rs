//! 传输设置的跨 FFI 形状。
//!
//! 基线里「请求怎么发」由一组 `Pref` 开关决定（重试、系统代理、证书
//! 豁免、HTTP/2）。它们的真相源留在 Dart：用户在设置页改的就是那些键，
//! Rust 再存一份就会出现「设置改了而 Rust 路径仍用旧值」——与凭据不落
//! Rust 是同一个理由（issue #25）。
//!
//! 因此每次取数都带上本结构。构造它的代价只是几次 Hive 读取，发生在
//! 一次网络请求之前，可以忽略。

use bili_api::http::{ProxyConfig, TransportConfig};

/// 传输设置。字段与 Dart 的 `RustTransport.current()` 一一对应。
#[derive(Debug, Clone)]
pub struct TransportSettings {
    /// 重试次数，0 表示不重试（基线 `Pref.retryCount`）。
    pub retry_count: u32,
    /// 重试基础间隔（毫秒，基线 `Pref.retryDelay`）。第 n 次重试等待
    /// `n * retry_delay_ms`。
    pub retry_delay_ms: u64,
    /// 系统代理地址。为空表示直连。
    pub proxy_host: Option<String>,
    pub proxy_port: Option<u16>,
    /// 接受无效证书。由 Dart 侧把「开了系统代理」与
    /// `Pref.badCertificateCallback` 两种情形折算成这一个布尔值。
    pub accept_invalid_certs: bool,
    /// 只用 HTTP/1.1。对应 `!Pref.enableHttp2`。
    pub http1_only: bool,
}

impl TransportSettings {
    /// 转成数据层的配置。
    ///
    /// 代理只在 host 非空且 port 有效时生效：半截配置（有 host 没 port）
    /// 在基线里同样被当作「没开代理」处理，而不是用一个默认端口去连。
    pub(crate) fn to_config(&self) -> TransportConfig {
        let proxy = match (self.proxy_host.as_deref(), self.proxy_port) {
            (Some(host), Some(port)) if !host.is_empty() && port != 0 => Some(ProxyConfig {
                host: host.to_owned(),
                port,
            }),
            _ => None,
        };

        TransportConfig {
            retry_count: self.retry_count,
            retry_delay_ms: self.retry_delay_ms,
            proxy,
            accept_invalid_certs: self.accept_invalid_certs,
            http1_only: self.http1_only,
        }
    }
}
