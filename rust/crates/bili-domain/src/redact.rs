//! 日志脱敏。
//!
//! RFC-0001 §2.2：**日志中不得包含 Cookie/Token**。
//!
//! 这不是可选的卫生习惯——日志会进崩溃报告、CI 产物和用户贴出的
//! issue。一次泄漏即等同于账号泄漏。
//!
//! 本模块的策略是**按字段名拒绝**而非按内容猜测：凭据长得像普通
//! 字符串，没有可靠的内容特征可供识别。因此调用方必须用
//! [`Redacted`] 包裹敏感值，或用 [`redact_query`] 处理整串查询参数。

use std::fmt;

/// 基线实际使用的凭据字段名（见 BASELINE.md §4）。
///
/// 比对时忽略大小写。新增凭据字段必须同步加入此列表。
pub const CREDENTIAL_KEYS: &[&str] = &[
    "sessdata",
    "bili_jct",
    "access_key",
    "refresh_token",
    "csrf",
    "cookie",
    "set-cookie",
    "authorization",
    "dedeuserid__ckmd5",
    "sid",
];

/// 字段名是否属于凭据。
#[must_use]
pub fn is_credential_key(key: &str) -> bool {
    let k = key.trim().to_ascii_lowercase();
    CREDENTIAL_KEYS.contains(&k.as_str())
}

/// 包裹敏感值，使其**无法**被意外打印。
///
/// `Debug` 与 `Display` 均只输出 `<redacted>`；取真实值必须显式调用
/// [`Redacted::expose`]，使泄漏点在 code review 中可被搜索。
#[derive(Clone, PartialEq, Eq)]
pub struct Redacted<T>(T);

impl<T> Redacted<T> {
    pub const fn new(value: T) -> Self {
        Self(value)
    }

    /// 显式取出真实值。**调用点应被 review。**
    pub fn expose(&self) -> &T {
        &self.0
    }

    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> fmt::Debug for Redacted<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<redacted>")
    }
}

impl<T> fmt::Display for Redacted<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<redacted>")
    }
}

impl<T> From<T> for Redacted<T> {
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

/// 对 `a=1&sessdata=xxx` 形式的查询串脱敏，保留字段名与顺序以便诊断。
///
/// 不解码百分号转义——目的是产出可读日志，不是还原语义。
#[must_use]
pub fn redact_query(query: &str) -> String {
    query
        .split('&')
        .map(|pair| match pair.split_once('=') {
            Some((k, _)) if is_credential_key(k) => format!("{k}=<redacted>"),
            _ => pair.to_owned(),
        })
        .collect::<Vec<_>>()
        .join("&")
}

/// 对 header 键值对脱敏。
#[must_use]
pub fn redact_header(name: &str, value: &str) -> String {
    if is_credential_key(name) {
        format!("{name}: <redacted>")
    } else {
        format!("{name}: {value}")
    }
}
