#![allow(clippy::unwrap_used)]
use bili_api::live_danmaku::*;
#[test]
fn packet_roundtrip_and_truncation() {
    let bytes = encode_packet(5, br#"{"cmd":"ROOM_CHANGE","data":{"title":"x"}}"#).unwrap();
    assert_eq!(decode_packets(&bytes).unwrap()[0].operation, 5);
    assert!(decode_packets(&bytes[..15]).is_err());
    assert!(decode_packets(&bytes[..bytes.len() - 1]).is_err());
    let mut doubled = bytes.clone();
    doubled.extend(&bytes);
    assert_eq!(decode_packets(&doubled).unwrap().len(), 2);
}
#[test]
fn acknowledgements_and_unknown_events() {
    assert_eq!(auth_code(br#"{"code":0}"#).unwrap(), 0);
    assert_eq!(auth_code(br#"{"code":-101}"#).unwrap(), -101);
    assert!(auth_code(b"{}").is_err());
    assert_eq!(popularity(&42u32.to_be_bytes()).unwrap(), 42);
    assert!(
        parse_event(br#"{"cmd":"FUTURE_EVENT"}"#, 1)
            .unwrap()
            .is_none()
    );
}
#[test]
fn requests_preserve_baseline() {
    assert_eq!(
        build_prefetch_request(42).query,
        vec![("roomid".into(), "42".into())]
    );
    assert_eq!(
        build_super_chat_request(42).query,
        vec![("room_id".into(), "42".into())]
    );
    let req = build_token_request(42, "key", 1700000000);
    assert!(req.query.contains(&("web_location".into(), "444.8".into())));
    assert_eq!(req.query.last().unwrap().0, "w_rid");
}
#[test]
fn compression_and_limits() {
    use std::io::Write;
    let raw = encode_packet(5, br#"{"cmd":"ROOM_CHANGE","data":{"title":"x"}}"#).unwrap();
    let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    z.write_all(&raw).unwrap();
    let mut compressed = encode_packet(5, &z.finish().unwrap()).unwrap();
    compressed[7] = 2;
    assert_eq!(decode_packets(&compressed).unwrap()[0].body, raw[16..]);
    let mut encoded = Vec::new();
    {
        let mut b = brotli::CompressorWriter::new(&mut encoded, 4096, 5, 22);
        b.write_all(&raw).unwrap();
    }
    let mut compressed = encode_packet(5, &encoded).unwrap();
    compressed[7] = 3;
    assert_eq!(decode_packets(&compressed).unwrap()[0].body, raw[16..]);
    for _ in 0..6 {
        let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        z.write_all(&compressed).unwrap();
        compressed = encode_packet(5, &z.finish().unwrap()).unwrap();
        compressed[7] = 2;
    }
    assert!(decode_packets(&compressed).is_err());
    let mut bad = raw.clone();
    bad[3] = 0;
    assert!(decode_packets(&bad).is_err());
    let mut bad = raw.clone();
    bad[5] = 15;
    assert!(decode_packets(&bad).is_err());
    let mut bad = raw;
    bad[7] = 9;
    assert!(decode_packets(&bad).is_err());
}
#[test]
fn typed_events_and_filters() {
    use bili_domain::live_danmaku::*;
    let event = parse_event(br#"{"cmd":"ONLINE_RANK_COUNT","data":{"count":987}}"#, 42)
        .unwrap()
        .unwrap();
    assert_eq!(event.count, Some(987));
    let event = parse_event(
        br#"{"cmd":"WATCHED_CHANGE","data":{"text_large":"many"}}"#,
        42,
    )
    .unwrap()
    .unwrap();
    assert_eq!(event.text.as_deref(), Some("many"));
    assert!(parse_event(br#"{"cmd":"DANMU_MSG","info":[]}"#, 42).is_err());
    let rules = LiveBlockRules {
        keywords: vec!["bad".into()],
        uids: vec![7],
    };
    assert!(rules.is_blocked("bad text", 2));
    assert!(rules.is_blocked("ok", 7));
    assert!(!rules.is_blocked("ok", 2));
}
#[test]
fn real_documented_responses() {
    let p = parse_prefetch(include_str!("fixtures/live_danmaku/prefetch.json")).unwrap();
    assert!(!p.is_empty());
    let dm = parse_event(include_bytes!("fixtures/live_danmaku/danmaku.json"), 42)
        .unwrap()
        .unwrap()
        .danmaku
        .unwrap();
    assert_eq!(dm.emotes.len(), 1);
    assert_eq!(dm.segments.len(), 2);
    let sc = parse_event(include_bytes!("fixtures/live_danmaku/super_chat.json"), 42)
        .unwrap()
        .unwrap()
        .super_chat
        .unwrap();
    assert_eq!(sc.price, 30);
    let token = parse_token(include_str!("fixtures/live_danmaku/token.json")).unwrap();
    assert!(!token.servers.is_empty());
    let actual = serde_json::json!({"prefetch":p,"danmaku":dm,"super_chat":sc,"token":token});
    let expected: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/live_danmaku/normalized.json")).unwrap();
    assert_eq!(actual, expected);
}

#[test]
fn non_json_packets_do_not_hide_later_packets() {
    let mut bytes = encode_packet(5, b"not-json").unwrap();
    bytes.extend(encode_packet(5, br#"{"cmd":"ROOM_CHANGE","data":{"title":"next"}}"#).unwrap());
    let packets = decode_packets(&bytes).unwrap();
    assert!(parse_event(&packets[0].body, 42).is_err());
    assert_eq!(
        parse_event(&packets[1].body, 42)
            .unwrap()
            .unwrap()
            .text
            .as_deref(),
        Some("next")
    );
    // 畸形帧长无法可靠找下一帧，整条帧拒绝；JSON 错误只影响当前消息。
    bytes[3] = 0;
    assert!(decode_packets(&bytes).is_err());
}

#[test]
fn authentication_and_heartbeat_match_baseline() {
    let packet = decode_packets(&auth_packet(42, 7, "fixture-token").unwrap())
        .unwrap()
        .remove(0);
    assert_eq!(packet.operation, 7);
    let body: serde_json::Value = serde_json::from_slice(&packet.body).unwrap();
    assert_eq!(
        body,
        serde_json::json!({"roomid":42,"uid":7,"protover":3,"platform":"web","type":2,"key":"fixture-token"})
    );
    let heartbeat = heartbeat_packet(23);
    assert_eq!(heartbeat.len(), 16);
    assert_eq!(&heartbeat[12..16], &23u32.to_be_bytes());
    assert_eq!(decode_packets(&heartbeat).unwrap()[0].operation, 2);
}

#[test]
fn decompression_and_packet_count_budgets_are_enforced() {
    use std::io::Write;
    let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    z.write_all(&vec![0; 8 * 1024 * 1024 + 1]).unwrap();
    let mut bomb = encode_packet(5, &z.finish().unwrap()).unwrap();
    bomb[7] = 2;
    assert!(decode_packets(&bomb).is_err());
    let raw = encode_packet(5, b"").unwrap();
    assert!(decode_packets(&raw.repeat(4097)).is_err());
}
