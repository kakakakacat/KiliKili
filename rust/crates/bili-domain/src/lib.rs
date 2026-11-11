//! 领域层：业务实体、规则、错误与 Port traits。
//!
//! 本 crate **不得**依赖 Flutter、Slint、具体平台 SDK 或任何
//! 协议/传输实现（RFC-0001 §2.1）。它必须能在不启动任何 UI
//! 的情况下独立测试。

pub mod danmaku;
pub mod error;
pub mod player;
pub mod playurl;
pub mod redact;
pub mod search;
pub mod session;
pub mod sponsor_block;
pub mod subtitle;
pub mod video;

#[cfg(test)]
mod tests {
    use super::error::{DomainError, ErrorKind};
    use super::redact::{Redacted, is_credential_key, redact_header, redact_query};
    use super::session::{GenerationGate, SessionId, Stamped};

    #[test]
    fn danmaku_segment_boundaries_match_the_baseline() {
        use super::danmaku::{SEGMENT_LENGTH_MS, segment_index};
        // 基线 DmUtils.calcSegment：progress ~/ (60 * 6 * 1000)
        assert_eq!(SEGMENT_LENGTH_MS, 360_000);
        assert_eq!(segment_index(0), 0);
        assert_eq!(segment_index(359_999), 0);
        assert_eq!(segment_index(360_000), 1, "分段边界属于下一段");
        assert_eq!(segment_index(7_200_000), 20);
    }

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

    #[test]
    fn credential_keys_match_case_insensitively() {
        assert!(is_credential_key("SESSDATA"));
        assert!(is_credential_key("  bili_jct "));
        assert!(is_credential_key("Access_Key"));
        assert!(!is_credential_key("bvid"));
    }

    #[test]
    fn redacted_never_prints_its_value() {
        let secret = Redacted::new("super-secret-token");
        assert_eq!(format!("{secret}"), "<redacted>");
        assert_eq!(format!("{secret:?}"), "<redacted>");
        assert_eq!(*secret.expose(), "super-secret-token");
    }

    #[test]
    fn query_redaction_keeps_keys_and_drops_values() {
        let q = "bvid=BV1xx&SESSDATA=abc123&qn=80&bili_jct=deadbeef";
        assert_eq!(
            redact_query(q),
            "bvid=BV1xx&SESSDATA=<redacted>&qn=80&bili_jct=<redacted>"
        );
    }

    #[test]
    fn query_redaction_tolerates_malformed_pairs() {
        assert_eq!(redact_query("a&b=1&=2"), "a&b=1&=2");
        assert_eq!(redact_query(""), "");
    }

    #[test]
    fn header_redaction() {
        assert_eq!(redact_header("Cookie", "SESSDATA=x"), "Cookie: <redacted>");
        assert_eq!(
            redact_header("User-Agent", "Mozilla"),
            "User-Agent: Mozilla"
        );
    }

    #[test]
    fn stale_generation_is_rejected() {
        let gate = GenerationGate::new(SessionId(1));
        let g0 = gate.current();
        let old = Stamped::new(SessionId(1), g0, "first");

        gate.bump(); // 用户切了视频

        assert_eq!(old.accept(&gate), None, "过期结果必须被丢弃");
    }

    #[test]
    fn current_generation_is_accepted() {
        let gate = GenerationGate::new(SessionId(7));
        let g = gate.bump();
        let fresh = Stamped::new(SessionId(7), g, 42);
        assert_eq!(fresh.accept(&gate), Some(42));
    }

    #[test]
    fn cross_session_result_is_rejected() {
        let gate = GenerationGate::new(SessionId(1));
        let g = gate.current();
        let other = Stamped::new(SessionId(2), g, "wrong session");
        assert_eq!(other.accept(&gate), None, "跨会话结果绝不可采纳");
    }

    #[test]
    fn gate_is_shared_across_clones() {
        let gate = GenerationGate::new(SessionId(1));
        let clone = gate.clone();
        clone.bump();
        assert_eq!(gate.current(), clone.current(), "克隆须共享同一代际");
    }
}
