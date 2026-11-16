//! BV 号 ↔ av 号对照 Dart 基线（ADR-008 决策三）。
//!
//! 期望值由 `tool/fixture-gen/bvid_fixture.dart` 产出，该脚本逐字复制
//! `lib/utils/id_utils.dart`。本测试只负责复现，不负责定义正确。
//!
//! 为什么这一面值得一份 golden：BV 号是服务端定义的一种 av 号编码，算错
//! 一位不会报错，只会让请求指向**另一个稿件**——写操作上那意味着点赞、
//! 投币落到别人头上。
//!
//! 重新生成：
//! ```sh
//! dart tool/fixture-gen/bvid_fixture.dart > rust/crates/bili-protocol/tests/fixtures/bvid.json
//! ```

// 测试里 expect 是正确做法：前提不成立时就该崩掉并指出原因。
#![allow(clippy::expect_used)]

use std::fs;
use std::path::Path;

use serde::Deserialize;

#[derive(Deserialize)]
struct Fixture {
    protocol_version: u32,
    /// 58 字符的字母表。顺序即进制位值。
    alphabet: String,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    aid: i64,
    bvid: String,
}

fn load() -> Fixture {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/bvid.json");
    let body = fs::read_to_string(path).expect("缺少 bvid fixture");
    serde_json::from_str(&body).expect("fixture 必须是合法 JSON")
}

#[test]
fn fixture_matches_the_protocol_version() {
    assert_eq!(
        load().protocol_version,
        bili_protocol::PROTOCOL_VERSION,
        "fixture 停在旧协议版本，重新生成一次"
    );
}

#[test]
fn bv_to_av_matches_the_dart_baseline() {
    for case in load().cases {
        assert_eq!(
            bili_protocol::bvid::bv_to_av(&case.bvid),
            Some(case.aid),
            "{} 应当转成 av{}",
            case.bvid,
            case.aid
        );
    }
}

#[test]
fn av_to_bv_matches_the_dart_baseline() {
    for case in load().cases {
        assert_eq!(
            bili_protocol::bvid::av_to_bv(case.aid).as_deref(),
            Some(case.bvid.as_str()),
            "av{} 应当转成 {}",
            case.aid,
            case.bvid
        );
    }
}

/// 字母表本身也逐字对照一次。
///
/// 上面两条测试覆盖不到它的每一位：fixture 里的样本未必用上全部 58 个
/// 字符。改一个没被命中的位置，那两条会继续绿，而线上会有一小部分 BV 号
/// 悄悄转错。
#[test]
fn the_alphabet_is_identical_to_the_baseline() {
    let fixture = load();
    assert_eq!(
        fixture.alphabet.as_bytes(),
        bili_protocol::bvid::ALPHABET.as_slice(),
        "字母表与基线不一致——顺序即进制位值，错一位就是另一个稿件"
    );
}
