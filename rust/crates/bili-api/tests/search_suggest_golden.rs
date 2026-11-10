#![allow(clippy::expect_used)]

use std::fs;
use std::path::Path;

#[test]
fn search_suggestions_match_dart_model() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/search_suggest");
    let mut raws: Vec<_> = fs::read_dir(dir)
        .expect("fixture directory exists")
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            path.to_string_lossy()
                .ends_with(".raw.json")
                .then_some(path)
        })
        .collect();
    raws.sort();
    assert!(!raws.is_empty());

    for raw in raws {
        let expected = raw
            .to_string_lossy()
            .replace(".raw.json", ".normalized.json");
        let body = fs::read_to_string(&raw).expect("raw fixture readable");
        let expected: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(expected).expect("Dart-generated golden exists"),
        )
        .expect("golden is JSON");
        let actual = bili_api::search_suggest::parse(&body).expect("valid fixture parses");
        assert_eq!(serde_json::to_value(actual).expect("serializes"), expected);
    }
}

#[test]
fn invalid_item_does_not_silently_disappear() {
    let body = r#"{"code":0,"data":{"result":{"tag":[{"term":"x"}]}}}"#;
    assert!(bili_api::search_suggest::parse(body).is_err());
}
