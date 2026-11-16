// 测试前提失败应当直接指出位置，不把断言失败包装成业务错误。
#![allow(clippy::unwrap_used)]

use bili_api::grpc_metadata::{GrpcIdentity, headers};
use bili_domain::redact::Redacted;

#[test]
fn metadata_matches_dart_including_explicit_empty_access_key() {
    let cases: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/grpc/metadata.dart.json")).unwrap();
    for case in cases.as_array().unwrap() {
        let identity = GrpcIdentity {
            access_key: case["access_key"]
                .as_str()
                .map(|s| Redacted::new(s.to_owned())),
            buvid: "buvid-placeholder".to_owned(),
            session_id: "abcdefgh".to_owned(),
        };
        let actual: std::collections::BTreeMap<_, _> = headers(&identity).into_iter().collect();
        for (key, field) in [
            ("x-bili-metadata-bin", "metadata"),
            ("x-bili-device-bin", "device"),
            ("x-bili-fawkes-req-bin", "fawkes"),
            ("x-bili-locale-bin", "locale"),
            ("x-bili-network-bin", "network"),
        ] {
            assert_eq!(actual[key], case[field].as_str().unwrap());
        }
        assert_eq!(
            actual.contains_key("authorization"),
            identity.access_key.is_some()
        );
        assert_eq!(actual["gzip-accept-encoding"], "gzip,identity");
        assert!(!actual.contains_key("grpc-accept-encoding"));
        assert!(!format!("{identity:?}").contains("access-placeholder"));
    }
}
