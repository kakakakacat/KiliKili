// 测试前提失败应当直接指出位置，不把断言失败包装成业务错误。
#![allow(clippy::unwrap_used)]

use bili_api::grpc_danmaku::{segment_request, view_request};
#[test]
fn request_bytes_keep_explicit_zero_presence() {
    assert_eq!(segment_request(2, 1, 0), vec![0x10, 2, 0x18, 0, 0x20, 1]);
    assert_eq!(view_request(1, 2), vec![8, 1, 0x10, 2, 0x18, 1]);
}
#[test]
fn real_sanitized_frame_matches_dart_decoding() {
    assert_eq!(
        bili_api::grpc::decode_frame(include_bytes!("fixtures/grpc/dm_segment.frame.bin")).unwrap(),
        include_bytes!("fixtures/grpc/dm_segment.payload.pb")
    );
}
