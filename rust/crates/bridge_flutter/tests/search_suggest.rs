#![allow(clippy::expect_used)]

#[test]
fn bridge_returns_normalized_search_suggestions() {
    let body = r#"{"code":0,"data":{"result":{"tag":[{"term":"test","name":"<em>test</em>"}]}}}"#;
    let parsed = bridge_flutter::api::search_suggest::parse_search_suggest(body.to_owned());
    let json = parsed.json.expect("valid response has JSON");
    let value: serde_json::Value = serde_json::from_str(&json).expect("normalized JSON");
    assert_eq!(value["tag"][0]["name"], "<em>test</em>");
}

#[test]
fn bridge_returns_error_without_throwing_on_bad_response() {
    let parsed = bridge_flutter::api::search_suggest::parse_search_suggest("bad".to_owned());
    assert!(parsed.json.is_none());
    assert_eq!(parsed.error_kind.as_deref(), Some("protocol"));
}
