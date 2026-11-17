//! 长连接以本地服务器验证，不向线上账号重复请求。
#![allow(clippy::unwrap_used)]
use std::time::Duration;

use bili_api::http::TransportConfig;
use bili_api::live_socket::{SocketSession, connect};
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::{accept_async, tungstenite::Message};

#[tokio::test]
async fn upgrade_rejects_non_websocket_response() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = [0; 4096];
        assert!(socket.read(&mut request).await.unwrap() > 0);
        socket
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
            .await
            .unwrap();
    });
    assert!(
        connect(&format!("ws://{address}/sub"), &TransportConfig::default())
            .await
            .is_err()
    );
    server.await.unwrap();
}

#[tokio::test]
async fn upgrade_supports_binary_frames_without_changing_transport() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        let mut ws = accept_async(socket).await.unwrap();
        assert_eq!(ws.next().await.unwrap().unwrap().into_data(), vec![1, 2, 3]);
        ws.send(Message::Binary(vec![4, 5].into())).await.unwrap();
    });
    let mut socket = connect(&format!("ws://{address}/sub"), &TransportConfig::default())
        .await
        .unwrap();
    socket
        .send(Message::Binary(vec![1, 2, 3].into()))
        .await
        .unwrap();
    assert_eq!(
        socket.next().await.unwrap().unwrap().into_data(),
        vec![4, 5]
    );
    server.await.unwrap();
}

#[tokio::test]
async fn close_cancels_pending_handshake() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let mut session = SocketSession::start(
        vec![format!("ws://{address}/sub")],
        100,
        0,
        "redacted".into(),
        TransportConfig::default(),
    );
    let (socket, _) = tokio::time::timeout(Duration::from_secs(2), listener.accept())
        .await
        .unwrap()
        .unwrap();
    session.close();
    assert!(session.is_finished().await);
    assert!(session.poll().events.is_empty());
    drop(socket);
}

fn packet(op: u32, body: &[u8]) -> Message {
    Message::Binary(
        bili_api::live_danmaku::encode_packet(op, body)
            .unwrap()
            .into(),
    )
}

#[tokio::test]
async fn authenticates_then_delivers_typed_batch_and_terminal_error() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        let mut ws = accept_async(socket).await.unwrap();
        let auth = ws.next().await.unwrap().unwrap().into_data();
        let packets = bili_api::live_danmaku::decode_packets(&auth).unwrap();
        assert_eq!(packets[0].operation, 7);
        let body: serde_json::Value = serde_json::from_slice(&packets[0].body).unwrap();
        assert_eq!(body["roomid"], 100);
        assert_eq!(body["key"], "fixture-token");
        ws.send(packet(8, br#"{"code":0}"#)).await.unwrap();
        ws.send(packet(
            5,
            br#"{"cmd":"ROOM_CHANGE","data":{"title":"title"}}"#,
        ))
        .await
        .unwrap();
        // 不发 close，模拟网络断开。错误不能替代已经收到的消息。
    });
    let session = SocketSession::start(
        vec![format!("ws://{address}/sub")],
        100,
        1,
        "fixture-token".into(),
        TransportConfig::default(),
    );
    server.await.unwrap();
    let mut messages = Vec::new();
    let final_batch = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let batch = session.poll();
            messages.extend(batch.events.clone());
            if batch.ended {
                break batch;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].text.as_deref(), Some("title"));
    assert!(final_batch.error.is_some());
}

#[tokio::test]
async fn rejects_authentication_failure_without_delivering_messages() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        let mut ws = accept_async(socket).await.unwrap();
        ws.next().await.unwrap().unwrap();
        ws.send(packet(8, br#"{"code":-101}"#)).await.unwrap();
    });
    let session = SocketSession::start(
        vec![format!("ws://{address}/sub")],
        100,
        0,
        "fixture-token".into(),
        TransportConfig::default(),
    );
    server.await.unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let batch = session.poll();
            if batch.ended {
                assert!(batch.events.is_empty());
                assert_eq!(batch.error.unwrap().code(), Some(-101));
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn heartbeat_begins_after_auth_and_sequence_increases() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
    let (beat_tx, mut beat_rx) = tokio::sync::mpsc::channel(2);
    let server = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        let mut ws = accept_async(socket).await.unwrap();
        ws.next().await.unwrap().unwrap();
        ws.send(packet(8, br#"{"code":0}"#)).await.unwrap();
        ws.send(packet(
            5,
            br#"{"cmd":"ROOM_CHANGE","data":{"title":"ready"}}"#,
        ))
        .await
        .unwrap();
        ready_tx.send(()).unwrap();
        for _ in 0..2 {
            let data = ws.next().await.unwrap().unwrap().into_data();
            beat_tx.send(data).await.unwrap();
        }
    });
    let mut session = SocketSession::start(
        vec!["ws://127.0.0.1:0/sub".into(), format!("ws://{address}/sub")],
        100,
        0,
        "redacted".into(),
        TransportConfig::default(),
    );
    ready_rx.await.unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        while session.poll().events.is_empty() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    tokio::time::pause();
    for seq in 1..=2u32 {
        tokio::time::advance(Duration::from_secs(30)).await;
        let bytes = beat_rx.recv().await.unwrap();
        assert_eq!(bytes.len(), 16);
        assert_eq!(&bytes[8..12], &2u32.to_be_bytes());
        assert_eq!(&bytes[12..16], &seq.to_be_bytes());
    }
    tokio::time::resume();
    session.close();
    assert!(session.poll().events.is_empty());
    server.await.unwrap();
}

#[tokio::test]
async fn close_releases_authentication_and_receiving_tasks() {
    for authenticated in [false, true] {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
        let server = tokio::spawn(async move {
            let (socket, _) = listener.accept().await.unwrap();
            let mut ws = accept_async(socket).await.unwrap();
            ws.next().await.unwrap().unwrap();
            if authenticated {
                ws.send(packet(8, br#"{"code":0}"#)).await.unwrap();
                ws.send(packet(
                    5,
                    br#"{"cmd":"ROOM_CHANGE","data":{"title":"ready"}}"#,
                ))
                .await
                .unwrap();
            }
            ready_tx.send(()).unwrap();
            // abort 直接释放底层连接，不依赖远端 close 握手。
            ws.next().await
        });
        let mut session = SocketSession::start(
            vec![format!("ws://{address}/sub")],
            100,
            0,
            "fixture".into(),
            TransportConfig::default(),
        );
        ready_rx.await.unwrap();
        if authenticated {
            tokio::time::timeout(Duration::from_secs(2), async {
                while session.poll().events.is_empty() {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();
        }
        session.close();
        assert!(
            tokio::time::timeout(Duration::from_secs(2), session.is_finished())
                .await
                .unwrap()
        );
        assert!(session.poll().events.is_empty());
        assert!(session.poll().error.is_none());
        tokio::time::timeout(Duration::from_secs(2), server)
            .await
            .unwrap()
            .unwrap();
    }
}
