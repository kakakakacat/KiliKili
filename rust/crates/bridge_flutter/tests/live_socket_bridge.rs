//! 不依赖真实服务端验证会话代际与关闭幂等。
#![allow(clippy::unwrap_used)]
use bridge_flutter::api::live_danmaku::*;
use bridge_flutter::api::transport::TransportSettings;

fn transport() -> TransportSettings {
    TransportSettings {
        retry_count: 0,
        retry_delay_ms: 0,
        proxy_host: None,
        proxy_port: None,
        accept_invalid_certs: false,
        http1_only: true,
    }
}
fn rules() -> LiveBlockRules {
    LiveBlockRules {
        keywords: vec![],
        uids: vec![],
    }
}

#[test]
fn old_generation_cannot_read_or_close_live_session() {
    let opened = live_socket_open(
        vec!["ws://127.0.0.1:0/sub".into()],
        1,
        0,
        "fixture".into(),
        7,
        transport(),
        rules(),
    );
    let handle = opened.data.unwrap();
    assert!(live_socket_poll(handle.session_id, 6).data.is_none());
    live_socket_close(handle.session_id, 6);
    assert!(live_socket_poll(handle.session_id, 7).data.is_some());
    live_socket_close(handle.session_id, 7);
    assert_eq!(
        live_socket_poll(handle.session_id, 7).error_kind.as_deref(),
        Some("cancelled")
    );
    assert!(live_socket_close(handle.session_id, 7).error_kind.is_none());
}

#[test]
fn missing_servers_returns_protocol_value() {
    let result = live_socket_open(vec![], 1, 0, String::new(), 1, transport(), rules());
    assert!(result.data.is_none());
    assert_eq!(result.error_kind.as_deref(), Some("protocol"));
}

#[test]
fn historical_filter_is_batch_based_and_uses_rust_rules() {
    let result = live_danmaku_filter_mask(
        vec![
            LiveFilterCandidate {
                text: "visible".into(),
                mid: 1,
            },
            LiveFilterCandidate {
                text: "blocked".into(),
                mid: 2,
            },
            LiveFilterCandidate {
                text: "visible".into(),
                mid: 3,
            },
        ],
        LiveBlockRules {
            keywords: vec!["blocked".into()],
            uids: vec![3],
        },
    );
    assert_eq!(result, vec![true, false, false]);
}
