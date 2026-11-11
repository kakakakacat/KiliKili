//! 统一错误模型。
//!
//! RFC-0001 §2.2 要求错误映射为**机器可识别的错误码**、用户提示与
//! 诊断字段。基线（Dart + media-kit）的错误恢复依赖匹配 mpv/ffmpeg
//! 的日志字符串（见 BASELINE.md §5），没有稳定错误码可供映射——
//! 因此错误分类必须在此处自行定义，而非从下层透传。
//!
//! 约束：错误的诊断字段**不得**包含 Cookie、Token 或其他凭据。

use std::fmt;

/// 错误大类。调用方据此决定重试、降级还是向用户报错。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ErrorKind {
    /// 网络不可达、超时、连接中断。通常可重试。
    Network,
    /// 服务端返回了可解析的失败响应。
    Remote,
    /// 响应结构与预期不符（协议变更的典型信号）。
    Protocol,
    /// 未登录、凭据失效、风控拦截。重试无意义。
    Auth,
    /// 本地存储、数据库、文件系统错误。
    Storage,
    /// 播放后端错误。
    Player,
    /// 请求被主动取消（路由切换、seek、输入变更）。
    Cancelled,
    /// 输入合法，但 Rust 这条实现**不保证**与基线等价，因此拒绝处理。
    ///
    /// 与 [`Self::Protocol`] 的区别是责任方：那边是对面变了，这边是
    /// 我们自己做不到。调用方唯一正确的反应是**整条路径降级回 Dart**，
    /// 而不是跳过这一项继续——典型例子是弹幕屏蔽正则，Dart `RegExp`
    /// 支持而 Rust `regex` 不支持（或语义不同）的那些，少过滤一条弹幕
    /// 在界面上没有任何提示（见 `bili-danmaku::rule`、ADR-010）。
    Unsupported,
}

impl ErrorKind {
    /// 原样重试是否有意义。
    #[must_use]
    pub const fn is_retryable(self) -> bool {
        matches!(self, Self::Network | Self::Remote)
    }

    /// 是否为调用方主动取消，不应向用户报错。
    #[must_use]
    pub const fn is_cancellation(self) -> bool {
        matches!(self, Self::Cancelled)
    }
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Network => "network",
            Self::Remote => "remote",
            Self::Protocol => "protocol",
            Self::Auth => "auth",
            Self::Storage => "storage",
            Self::Player => "player",
            Self::Cancelled => "cancelled",
            Self::Unsupported => "unsupported",
        };
        f.write_str(s)
    }
}

/// 领域层错误。
///
/// `detail` 用于诊断，不直接展示给用户，且不得包含凭据。
#[derive(Debug, Clone, thiserror::Error)]
#[error("{kind}: {detail}")]
pub struct DomainError {
    kind: ErrorKind,
    detail: String,
    code: Option<i64>,
}

impl DomainError {
    pub fn new(kind: ErrorKind, detail: impl Into<String>) -> Self {
        Self {
            kind,
            detail: detail.into(),
            code: None,
        }
    }

    /// 带上服务端业务错误码。
    ///
    /// [`ErrorKind`] 是给程序做决策用的粗分类，而有些**分支**要的恰是
    /// 原始码：基线在 playurl 上会因为「UGC 请求返回了非 0 码且带 epid」
    /// 改走 PGC 接口（`lib/http/video.dart`），这类分支无法从大类推出。
    /// 码同时也是用户可见文案的依据（`_parseVideoErr`）。
    pub fn with_code(kind: ErrorKind, detail: impl Into<String>, code: i64) -> Self {
        Self {
            kind,
            detail: detail.into(),
            code: Some(code),
        }
    }

    /// 服务端业务错误码。仅当失败来自接口信封时才有值。
    #[must_use]
    pub const fn code(&self) -> Option<i64> {
        self.code
    }

    #[must_use]
    pub const fn kind(&self) -> ErrorKind {
        self.kind
    }

    #[must_use]
    pub fn detail(&self) -> &str {
        &self.detail
    }
}

/// 领域层统一 Result。
pub type DomainResult<T> = Result<T, DomainError>;
