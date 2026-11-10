//! WBI golden 对照测试。
//!
//! fixture 由 Dart 基线实现产出（`tool/fixture-gen/wbi_fixture.dart`），
//! 不是本实现的自我验证——Rust 必须复现 Dart 的逐字节输出，否则服务端
//! 会拒绝请求，而拒绝的形式是业务错误码而非签名错误，极难反推。
//!
//! 重新生成 fixture：
//! ```sh
//! dart tool/fixture-gen/wbi_fixture.dart \
//!   > rust/crates/bili-protocol/tests/fixtures/wbi.json
//! ```
//! 若上游改了算法，重新生成后本测试会红，从而暴露差异。

// 测试中 expect 是正确做法：fixture 损坏或前提不成立时，测试就该崩掉并
// 指出原因，而非走进分支做更多无意义断言。豁免只作用于本文件。
#![allow(clippy::expect_used)]

use bili_protocol::wbi;
use serde::Deserialize;

#[derive(Deserialize)]
struct Fixture {
    protocol_version: u32,
    img_key: String,
    sub_key: String,
    mixin_key: String,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    params: serde_json::Map<String, serde_json::Value>,
    w_rid: String,
}

fn load() -> Fixture {
    let raw = include_str!("fixtures/wbi.json");
    serde_json::from_str(raw).expect("fixture 必须是合法 JSON")
}

/// JSON 值转为基线 `params[i].toString()` 的等价形式。
///
/// Dart 的 `Object.toString()` 对 int 不带小数点，对 String 即其本身。
/// fixture 仅含这两种类型；出现其他类型说明 fixture 与此处假设脱节，
/// 应当失败而非猜测。
fn to_dart_string(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) if n.is_i64() || n.is_u64() => n.to_string(),
        other => panic!("fixture 含未预期的值类型: {other:?}"),
    }
}

#[test]
fn fixture_matches_current_protocol_version() {
    // fixture 停留在旧协议版本时，对照仍可能通过（旧用例恰好没变），
    // 从而让人误以为新协议已被覆盖。版本号对不上即失败（ADR-008）。
    assert_eq!(
        load().protocol_version,
        bili_protocol::PROTOCOL_VERSION,
        "fixture 与 bili-protocol 的协议版本不一致，需重新生成 fixture"
    );
}

#[test]
fn mixin_key_matches_baseline() {
    let f = load();
    let got = wbi::mixin_key(&f.img_key, &f.sub_key).expect("长度足够时必须成功");
    assert_eq!(got, f.mixin_key, "mixin_key 重排结果与 Dart 基线不一致");
}

#[test]
fn signatures_match_baseline() {
    let f = load();
    assert!(!f.cases.is_empty(), "fixture 不得为空，否则本测试形同虚设");

    for case in &f.cases {
        let params: Vec<(String, String)> = case
            .params
            .iter()
            .map(|(k, v)| (k.clone(), to_dart_string(v)))
            .collect();
        let got = wbi::sign(&params, &f.mixin_key);
        assert_eq!(
            got, case.w_rid,
            "用例 `{}` 的 w_rid 与 Dart 基线不一致",
            case.name
        );
    }
}

#[test]
fn mixin_key_rejects_short_input() {
    // 重排表最大索引 58，输入不足时必须返回 None 而非静默产出错误签名：
    // 错误签名会让服务端返回业务错误码，无法反推到签名环节。
    assert!(wbi::mixin_key("short", "also_short").is_none());
}

#[test]
fn param_order_does_not_affect_signature() {
    let f = load();
    let mut a = vec![
        ("bvid".to_owned(), "BV1L94y1H7CV".to_owned()),
        ("wts".to_owned(), "1702204169".to_owned()),
    ];
    let signed = wbi::sign(&a, &f.mixin_key);
    a.reverse();
    assert_eq!(
        wbi::sign(&a, &f.mixin_key),
        signed,
        "签名须对输入顺序不敏感"
    );
}
