//! 批次 5 的响应与请求约定。真实响应和手工边界样本的区别见 fixture README。
#![allow(clippy::expect_used)]
use bili_api::space;
use std::{fs, path::Path};

fn check(name: &str, parse: impl Fn(&str) -> serde_json::Value) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/space_tabs");
    let raw = fs::read_to_string(dir.join(format!("{name}.raw.json"))).expect("样本可读");
    let got = parse(&raw);
    let path = dir.join(format!("{name}.rust.json"));
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        fs::write(
            &path,
            format!(
                "{}\n",
                serde_json::to_string_pretty(&got).expect("可序列化")
            ),
        )
        .expect("快照可写");
    }
    let expected: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(path).expect("快照可读")).expect("快照合法");
    assert_eq!(got, expected, "{name}");
}
#[test]
fn tabs_match_frozen_snapshots() {
    check("article_real", |s| {
        serde_json::to_value(space::parse_space_article(s).expect("专栏成功")).expect("可序列化")
    });
    for name in ["audio_empty_real", "audio_cases"] {
        check(name, |s| {
            serde_json::to_value(space::parse_space_audio(s).expect("音频成功")).expect("可序列化")
        });
    }
    for name in ["cheese_empty_real", "cheese_cases"] {
        check(name, |s| {
            serde_json::to_value(space::parse_space_cheese(s).expect("课堂成功")).expect("可序列化")
        });
    }
    check("shop_empty_real", |s| {
        serde_json::to_value(space::parse_space_shop(s).expect("小店成功")).expect("可序列化")
    });
    check("shop_cases", |s| {
        serde_json::to_value(space::parse_space_shop(s).expect("小店成功")).expect("可序列化")
    });
}
#[test]
fn request_shapes_keep_baseline_parameters() {
    let article = space::space_article_request(
        123,
        2,
        10,
        None,
        "dfca71928277209b",
        "b5475a8825547a4fc26c7d518eaaa02e",
        1_700_000_000,
    );
    assert_eq!(article.path, "/x/v2/space/article");
    assert!(article.query.contains(&("vmid".into(), "123".into())));
    assert!(article.query.contains(&("pn".into(), "2".into())));
    assert!(article.query.contains(&("ps".into(), "10".into())));
    assert!(article.query.contains(&("build".into(), "8430300".into())));
    let audio = space::space_audio_request(123, 3, 20);
    assert_eq!(audio.path, "/audio/music-service/web/song/upper");
    assert_eq!(
        audio.query,
        vec![
            ("pn".into(), "3".into()),
            ("ps".into(), "20".into()),
            ("order".into(), "1".into()),
            ("uid".into(), "123".into()),
            ("web_location".into(), "333.1387".into())
        ]
    );
    let cheese = space::space_cheese_request(123, 4, 30);
    assert_eq!(
        cheese.query,
        vec![
            ("pn".into(), "4".into()),
            ("ps".into(), "30".into()),
            ("mid".into(), "123".into()),
            ("web_location".into(), "333.1387".into())
        ]
    );
    let shop = space::space_shop_request(123, 8, Some(""), "key", "secret", 1700000000);
    assert!(shop.query.contains(&("access_key".into(), "".into())));
    assert!(shop.query.contains(&("appkey".into(), "key".into())));
    assert!(shop.query.contains(&("ts".into(), "1700000000".into())));
    assert!(shop.query.iter().any(|(k, v)| k == "sign" && v.len() == 32));
    assert_eq!(shop.path, "/community-hub/small_shop/feed/tab/item");
    assert_eq!(shop.csrf, bili_api::request::CsrfIn::None);
    assert_eq!(shop.idempotency, bili_api::http::Idempotency::Idempotent);
    match shop.body {
        bili_api::request::WriteBody::Json(v) => assert_eq!(
            v,
            serde_json::json!({"from":"cps_productTab_123", "searchAfter":0, "msource":"cps_productTab_123", "pageSize":8, "upMid":"123"})
        ),
        _ => panic!("小店必须用JSON"),
    }
}
#[test]
fn shop_redirect_matches_uri_form_query_semantics() {
    for (url, expected) in [
        ("https://example.invalid/?%75rl=a+b#ignored", Some("a b")),
        ("https://example.invalid/#fragment?url=wrong", None),
        ("https://example.invalid/?url", Some("")),
        ("https://example.invalid/?url=a%2Bb%26c", Some("a+b&c")),
    ] {
        let raw = serde_json::json!({"code":0,"data":{"clickUrl":url}}).to_string();
        assert_eq!(
            space::parse_space_shop(&raw)
                .expect("成功")
                .more_url
                .as_deref(),
            expected
        );
    }
}
#[test]
fn failures_use_domain_results() {
    let body = r#"{"code":-101,"message":"账号未登录"}"#;
    assert_eq!(
        space::parse_space_article(body).expect_err("失败").code(),
        Some(-101)
    );
    assert_eq!(
        space::parse_space_audio(body).expect_err("失败").code(),
        Some(-101)
    );
    assert_eq!(
        space::parse_space_cheese(body).expect_err("失败").code(),
        Some(-101)
    );
    assert_eq!(
        space::parse_space_shop(body).expect_err("失败").code(),
        Some(-101)
    );
}

#[test]
fn opus_wire_matches_baseline_decoder_snapshot() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/space_tabs");
    let bytes = fs::read(dir.join("opus.raw.pb")).expect("样本可读");
    let got = serde_json::to_value(
        bili_api::grpc_space::parse_opus_space_flow(&bytes).expect("图文成功"),
    )
    .expect("可序列化");
    let path = dir.join("opus.rust.json");
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        fs::write(
            &path,
            format!(
                "{}\n",
                serde_json::to_string_pretty(&got).expect("可序列化")
            ),
        )
        .expect("快照可写");
    }
    let expected: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(path).expect("快照可读")).expect("快照合法");
    assert_eq!(got, expected);
    let baseline: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(dir.join("opus.dart.json")).expect("裁判可读"))
            .expect("裁判合法");
    assert_eq!(got, baseline["data"]);
    assert!(bili_api::grpc_space::parse_opus_space_flow(&[0x0a, 0xff]).is_err());
    let empty =
        bili_api::grpc_space::parse_opus_space_flow(&[0x12, 0x02, 0x0a, 0x00]).expect("空游标合法");
    assert!(empty.next.is_none());
}
#[test]
fn opus_request_keeps_grpc_path_and_pagination() {
    use bili_api::grpc_space::{OPUS_SPACE_FLOW, opus_space_flow_request};
    assert_eq!(
        OPUS_SPACE_FLOW,
        "/bilibili.app.dynamic.v2.Opus/OpusSpaceFlow"
    );
    // 来自基线 protobuf 编码器；分页大小 20，cursor 为 next，filter 为 all。
    assert_eq!(
        opus_space_flow_request(100, Some("next"), "all", 20),
        vec![
            8, 100, 26, 8, 8, 20, 18, 4, 110, 101, 120, 116, 34, 3, 97, 108, 108
        ]
    );
    assert_eq!(
        opus_space_flow_request(100, None, "all", 20),
        opus_space_flow_request(100, Some(""), "all", 20)
    );
}
