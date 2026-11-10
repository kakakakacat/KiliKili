//! 领域层：业务实体、规则、错误与 Port traits。
//!
//! 本 crate **不得**依赖 Flutter、Slint、具体平台 SDK 或任何
//! 协议/传输实现（RFC-0001 §2.1）。它必须能在不启动任何 UI
//! 的情况下独立测试。

pub mod error;

#[cfg(test)]
mod tests {
    use super::error::{DomainError, ErrorKind};

    #[test]
    fn error_kind_is_retryable() {
        assert!(ErrorKind::Network.is_retryable());
        assert!(!ErrorKind::Auth.is_retryable());
    }

    #[test]
    fn domain_error_carries_kind() {
        let e = DomainError::new(ErrorKind::Network, "timeout");
        assert_eq!(e.kind(), ErrorKind::Network);
    }
}
