//! APP 签名对照 Dart 基线（ADR-008 决策三）。
//!
//! 期望值由 `tool/fixture-gen/app_sign_fixture.dart` 产出，该脚本逐字
//! 复制 `lib/utils/app_sign.dart`。本测试只负责复现，不负责定义正确。
//!
//! 重新生成：
//! ```sh
//! dart tool/fixture-gen/app_sign_fixture.dart > rust/crates/bili-protocol/tests/fixtures/app_sign.json
//! ```

// 测试里 expect 是正确做法：前提不成立时就该崩掉并指出原因。
#![allow(clippy::expect_used)]

use std::fs;
use std::path::Path;

use serde::Deserialize;

#[derive(Deserialize)]
struct Fixture {
    protocol_version: u32,
    appkey: String,
    appsec: String,
    ts: i64,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    params: serde_json::Map<String, serde_json::Value>,
    /// 整组已签名参数，`[key, value]` 的有序列表。
    signed: Vec<(String, String)>,
    sign: String,
}

fn load() -> Fixture {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/app_sign.json");
    let body = fs::read_to_string(path).expect("缺少 app_sign fixture");
    serde_json::from_str(&body).expect("fixture 必须是合法 JSON")
}

/// 把 fixture 里的 JSON 值按基线 `value?.toString()` 的口径转成字符串。
///
/// 基线把整个参数表当 `Map<String, dynamic>` 处理，数字与布尔在拼接时
/// 都走 `toString()`。这里必须同口径，否则 `build: 8430300` 会被序列化
/// 成 `8430300.0` 之类，签名立刻对不上。
fn as_query_value(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        other => other.to_string(),
    }
}

#[test]
fn fixture_matches_the_current_protocol_version() {
    assert_eq!(
        load().protocol_version,
        bili_protocol::PROTOCOL_VERSION,
        "fixture 停在旧协议版本，用生成器重新产出（ADR-008 决策二）"
    );
}

#[test]
fn signatures_match_the_dart_baseline() {
    let fixture = load();

    for case in &fixture.cases {
        let params: Vec<(String, String)> = case
            .params
            .iter()
            .map(|(k, v)| (k.clone(), as_query_value(v)))
            .collect();

        let signed =
            bili_protocol::app_sign::sign(&params, &fixture.appkey, &fixture.appsec, fixture.ts);

        assert_eq!(
            signed, case.signed,
            "{}：已签名参数（含顺序）与基线不一致",
            case.name
        );

        let got = signed
            .iter()
            .find(|(k, _)| k == "sign")
            .map(|(_, v)| v.as_str())
            .expect("sign 必须存在");
        assert_eq!(got, case.sign, "{}：sign 与基线不一致", case.name);
    }
}

/// `appkey` 与 `ts` 必须参与签名，且 `sign` 垫底。
///
/// 这条看似与上面重复，实际锁的是**布局**：若哪天把 `sign` 混进排序里，
/// 上面的逐项比对会失败但原因不明显，这里直接说出是布局变了。
#[test]
fn sign_is_the_last_parameter() {
    let fixture = load();
    for case in &fixture.cases {
        let keys: Vec<&str> = case.signed.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(
            keys.last(),
            Some(&"sign"),
            "{}：sign 必须是最后一个参数",
            case.name
        );
        assert!(keys.contains(&"appkey"), "{}：缺 appkey", case.name);
        assert!(keys.contains(&"ts"), "{}：缺 ts", case.name);
    }
}
