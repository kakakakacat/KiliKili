//! 接口响应信封：`{"code":0,"message":"0","ttl":1,"data":{...}}`。
//!
//! 基线对失败的处理是 `Error(res.data['message'])`——把一个字符串丢给
//! UI（见 `lib/http/video.dart`）。调用方因此无从判断该重试、该去登录
//! 还是该提示用户，只能拿字符串去匹配，匹配又会随文案变化而失效。
//!
//! 这里把 `code` 映射成 [`ErrorKind`]，同时**保留原始 code 与 message**：
//! 前者供程序决策，后者供诊断与兜底展示。

use bili_domain::error::{DomainError, ErrorKind};
use serde::Deserialize;

/// 信封解包后的失败信息。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteFailure {
    pub code: i64,
    pub message: String,
}

/// 业务错误码到错误大类的映射。
///
/// 只收录**已知且分类明确**的码；未知码一律归入 [`ErrorKind::Remote`]，
/// 因为对未知失败重试一次是安全的，而猜成 `Auth` 会把用户踢去登录页。
///
/// 码表来源：bilibili-API-collect 的通用错误码，以及基线
/// `VideoHttp._parseVideoErr` 实际处理过的几个。
#[must_use]
pub const fn kind_for_code(code: i64) -> ErrorKind {
    match code {
        // 未登录(-101)、权限不足(-403)、风控校验失败(-352)：原样重试
        // 只会再被拒一次，必须先让用户登录或走验证流程。
        -101 | -403 | -352 => ErrorKind::Auth,
        _ => ErrorKind::Remote,
    }
}

impl RemoteFailure {
    #[must_use]
    pub fn kind(&self) -> ErrorKind {
        kind_for_code(self.code)
    }

    /// 转成领域错误。`detail` 只含 code 与服务端文案，不含请求参数——
    /// 参数里可能有凭据，而错误对象会进日志。
    ///
    /// 原始 code 一并带上：调用方的某些分支（playurl 的 UGC→PGC 重试）
    /// 与用户可见文案都依赖它，从 [`ErrorKind`] 反推不出来。
    #[must_use]
    pub fn into_domain_error(self) -> DomainError {
        DomainError::with_code(
            self.kind(),
            format!("code {}: {}", self.code, self.message),
            self.code,
        )
    }
}

#[derive(Deserialize)]
struct RawEnvelope<T> {
    code: i64,
    #[serde(default)]
    message: String,
    data: Option<T>,
}

/// 载荷放在 `result` 而不是 `data` 的信封。
///
/// PGC 那一组接口两种都有：索引与 season 排行用 `data`，放送时间表、
/// 追番状态与 web 排行用 `result`。不是历史包袱，是现状——按接口各取
/// 各的，别猜。
#[derive(Deserialize)]
struct RawResultEnvelope<T> {
    code: i64,
    #[serde(default)]
    message: String,
    result: Option<T>,
}

/// 解开信封。
///
/// 三种结果各自对应一类错误，不可合并：
/// - JSON 不合法或结构不符 → [`ErrorKind::Protocol`]，是协议变更的信号；
/// - `code != 0` → 服务端明确拒绝，按码分类；
/// - `code == 0` 但 `data` 缺失 → 同样是 [`ErrorKind::Protocol`]：
///   成功却没有数据，只可能是我们对接口的理解过时了。
///
/// # Errors
/// 见上。
pub fn unwrap_envelope<T: serde::de::DeserializeOwned>(body: &str) -> Result<T, DomainError> {
    let env: RawEnvelope<T> = serde_json::from_str(body).map_err(|e| {
        DomainError::new(ErrorKind::Protocol, format!("响应不是预期的信封结构: {e}"))
    })?;

    if env.code != 0 {
        return Err(RemoteFailure {
            code: env.code,
            message: env.message,
        }
        .into_domain_error());
    }

    env.data
        .ok_or_else(|| DomainError::new(ErrorKind::Protocol, "code 为 0 但 data 缺失"))
}

/// 解开载荷在 `result` 里的信封。语义与 [`unwrap_envelope`] 完全一致，
/// 只是键名不同。
///
/// # Errors
/// 同 [`unwrap_envelope`]。
pub fn unwrap_result_envelope<T: serde::de::DeserializeOwned>(
    body: &str,
) -> Result<T, DomainError> {
    let env: RawResultEnvelope<T> = serde_json::from_str(body).map_err(|e| {
        DomainError::new(ErrorKind::Protocol, format!("响应不是预期的信封结构: {e}"))
    })?;

    if env.code != 0 {
        return Err(RemoteFailure {
            code: env.code,
            message: env.message,
        }
        .into_domain_error());
    }

    env.result
        .ok_or_else(|| DomainError::new(ErrorKind::Protocol, "code 为 0 但 result 缺失"))
}
