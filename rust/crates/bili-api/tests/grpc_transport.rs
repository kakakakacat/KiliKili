// 测试前提失败应当直接指出位置，不把断言失败包装成业务错误。
#![allow(clippy::unwrap_used)]

use bili_api::{
    grpc::{decode_response, frame_request},
    http::HttpBytesResponse,
};
use bili_domain::error::ErrorKind;

fn response(body: Vec<u8>) -> HttpBytesResponse {
    HttpBytesResponse {
        status: 200,
        headers: vec![("Grpc-Status".into(), "0".into())],
        body,
    }
}

#[test]
fn frame_boundaries_and_compression_roundtrip() {
    for len in [0, 1, 64, 65, 1024] {
        let payload = vec![42; len];
        let frame = frame_request(&payload).unwrap();
        assert_eq!(frame[0], u8::from(len > 64));
        assert_eq!(
            u32::from_be_bytes(frame[1..5].try_into().unwrap()) as usize,
            frame.len() - 5
        );
        assert_eq!(decode_response(response(frame)).unwrap(), payload);
    }
}

#[test]
fn rejects_truncation_flags_surplus_and_bad_gzip() {
    for bytes in [
        vec![],
        vec![0; 4],
        vec![0, 0, 0, 0, 1],
        vec![2, 0, 0, 0, 0],
        vec![0, 0, 0, 0, 0, 1],
        vec![1, 0, 0, 0, 1, 0],
    ] {
        assert_eq!(
            decode_response(response(bytes)).unwrap_err().kind(),
            ErrorKind::Protocol
        );
    }
}

#[test]
fn missing_status_explains_gateway_assumption() {
    let mut resp = response(vec![]);
    resp.headers.clear();
    let err = decode_response(resp).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::Protocol);
    assert_eq!(
        err.detail(),
        "没有 Grpc-Status 响应头，网关可能改用 trailer，见 ADR-012 决策一"
    );
}

#[test]
fn nested_status_keeps_business_code_and_messages() {
    use base64::{Engine, engine::general_purpose::STANDARD_NO_PAD};
    // Status(code=13, details=[Any(value=Status(code=-400,message="bad"))])。
    let inner = [
        vec![8],
        vec![0xf0, 0xfc, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 1],
        vec![18, 3, b'b', b'a', b'd'],
    ]
    .concat();
    let any = [vec![18, inner.len() as u8], inner].concat();
    let status = [vec![8, 13, 26, any.len() as u8], any].concat();
    let mut resp = response(vec![]);
    resp.headers = vec![
        ("grpc-status".into(), "13".into()),
        (
            "grpc-status-details-bin".into(),
            STANDARD_NO_PAD.encode(status),
        ),
    ];
    let err = decode_response(resp).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::Remote);
    assert_eq!(err.code(), Some(-400));
    assert_eq!(err.detail(), "bad");
}

#[test]
fn malformed_status_falls_back_to_text_without_panicking() {
    use base64::{Engine, engine::general_purpose::STANDARD_NO_PAD};
    let mut resp = response(vec![]);
    resp.headers = vec![
        ("grpc-status".into(), "13".into()),
        (
            "grpc-status-details-bin".into(),
            STANDARD_NO_PAD.encode(b"failure"),
        ),
    ];
    assert_eq!(decode_response(resp).unwrap_err().detail(), "failure");
    let mut resp = response(vec![]);
    resp.headers = vec![
        ("grpc-status".into(), "13".into()),
        ("grpc-status-details-bin".into(), "%%%".into()),
    ];
    assert_eq!(decode_response(resp).unwrap_err().kind(), ErrorKind::Remote);
}

#[tokio::test]
async fn reqwest_bytes_preserves_payload_headers_and_read_retry_policy() {
    use bili_api::{
        http::{HttpPost, HttpTransport, PostBody, RetryPolicy, TransportConfig},
        transport_reqwest::ReqwestTransport,
    };
    use std::{
        io::{Read, Write},
        net::TcpListener,
        time::Duration,
    };
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (mut first, _) = listener.accept().unwrap();
        first
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut buffer = [0; 4096];
        let n = first.read(&mut buffer).unwrap();
        assert!(String::from_utf8_lossy(&buffer[..n]).contains("application/grpc"));
        // 第一条连接保持到客户端超时，第二次读请求应当重发。
        let (mut second, _) = listener.accept().unwrap();
        let mut incoming = Vec::new();
        second
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        loop {
            let n = second.read(&mut buffer).unwrap();
            incoming.extend_from_slice(&buffer[..n]);
            if incoming.windows(4).any(|w| w == b"\r\n\r\n") && incoming.ends_with(&[0, 255, 128]) {
                break;
            }
        }
        second.write_all(b"HTTP/1.1 200 OK\r\nGrpc-Status: 0\r\nX-Probe: retained\r\nContent-Length: 3\r\nConnection: close\r\n\r\n\x00\xff\x80").unwrap();
        incoming
    });
    let transport = ReqwestTransport::new(TransportConfig {
        retry_count: 1,
        retry_delay_ms: 0,
        ..Default::default()
    });
    let result = transport
        .post_bytes(HttpPost {
            url: format!("http://{address}/test"),
            headers: vec![],
            body: PostBody::Bytes {
                content_type: "application/grpc".into(),
                data: vec![0, 255, 128],
            },
            timeout: Duration::from_millis(200),
            retry_policy: RetryPolicy::Read,
        })
        .await
        .unwrap();
    assert_eq!(result.body, [0, 255, 128]);
    assert!(
        result
            .headers
            .iter()
            .any(|(key, value)| key == "x-probe" && value == "retained")
    );
    assert!(server.join().unwrap().starts_with(b"POST /test HTTP/1.1"));
}

#[tokio::test]
async fn reqwest_bytes_does_not_retry_timed_out_write() {
    use bili_api::{
        http::{HttpPost, HttpTransport, Idempotency, PostBody, TransportConfig},
        transport_reqwest::ReqwestTransport,
    };
    use std::{
        io::Read,
        net::TcpListener,
        time::{Duration, Instant},
    };
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (mut first, _) = listener.accept().unwrap();
        let mut buffer = [0; 4096];
        assert!(first.read(&mut buffer).unwrap() > 0);
        listener.set_nonblocking(true).unwrap();
        let until = Instant::now() + Duration::from_millis(400);
        while Instant::now() < until {
            assert!(listener.accept().is_err(), "写请求整体超时后被重发");
            std::thread::sleep(Duration::from_millis(5));
        }
    });
    let transport = ReqwestTransport::new(TransportConfig {
        retry_count: 2,
        retry_delay_ms: 0,
        ..Default::default()
    });
    let result = transport
        .post_bytes(HttpPost {
            url: format!("http://{address}/write"),
            headers: vec![],
            body: PostBody::Bytes {
                content_type: "application/grpc".into(),
                data: vec![0],
            },
            timeout: Duration::from_millis(100),
            retry_policy: Idempotency::Idempotent.retry_policy(),
        })
        .await;
    assert!(result.is_err());
    server.join().unwrap();
}

#[test]
fn nested_empty_status_keeps_proto3_default_code() {
    use base64::{Engine, engine::general_purpose::STANDARD};
    // 空内层 Status 的 code 是 proto3 默认 0；无 details 才没有业务码。
    let mut response = response(vec![]);
    response.headers = vec![
        ("grpc-status".into(), "13".into()),
        (
            "grpc-status-details-bin".into(),
            STANDARD.encode([26, 2, 18, 0]),
        ),
    ];
    assert_eq!(decode_response(response).unwrap_err().code(), Some(0));
}
