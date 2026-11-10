//! playurl 请求构造的 golden 对照测试。
//!
//! fixture 由 Dart 基线产出（`tool/fixture-gen/playurl_fixture.dart`），
//! 逐项对照**完整查询串**与签名，而不只是签名：风控参数的作用正是让请求
//! 看起来像网页播放器，少一个参数不会报错，只会换来降级的清晰度或一个
//! 普通业务错误码。
//!
//! 重新生成 fixture：
//! ```sh
//! dart tool/fixture-gen/playurl_fixture.dart \
//!   > rust/crates/bili-api/tests/fixtures/playurl.json
//! ```
//! 上游改了参数组装，重新生成后本测试会红，差异即暴露（ADR-008）。

// 测试中 expect 是正确做法：fixture 损坏时就该崩掉并指出原因。
#![allow(clippy::expect_used)]

use bili_api::request::{self, PlayUrlParams};
use bili_domain::video::VideoType;
use bili_protocol::risk::RiskTokens;
use serde::Deserialize;

#[derive(Deserialize)]
struct Fixture {
    protocol_version: u32,
    mixin_key: String,
    wts: i64,
    dm_img_str: String,
    dm_cover_img_str: String,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    input: Input,
    path: String,
    query: Vec<Vec<String>>,
}

#[derive(Deserialize)]
struct Input {
    #[serde(default)]
    avid: Option<i64>,
    #[serde(default)]
    bvid: Option<String>,
    #[serde(default)]
    ep_id: Option<i64>,
    #[serde(default)]
    season_id: Option<i64>,
    cid: i64,
    qn: i64,
    video_type: String,
    try_look: bool,
    voice_balance: bool,
    fallback: bool,
    #[serde(default)]
    language: Option<String>,
}

impl Input {
    fn to_params(&self) -> PlayUrlParams {
        PlayUrlParams {
            avid: self.avid,
            bvid: self.bvid.clone(),
            ep_id: self.ep_id,
            season_id: self.season_id,
            cid: self.cid,
            qn: self.qn,
            video_type: match self.video_type.as_str() {
                "ugc" => VideoType::Ugc,
                "pgc" => VideoType::Pgc,
                "pugv" => VideoType::Pugv,
                other => panic!("fixture 含未知 video_type: {other}"),
            },
            try_look: self.try_look,
            voice_balance: self.voice_balance,
            language: self.language.clone(),
            fallback: self.fallback,
        }
    }
}

fn load() -> Fixture {
    let raw = include_str!("fixtures/playurl.json");
    serde_json::from_str(raw).expect("fixture 必须是合法 JSON")
}

fn tokens(f: &Fixture) -> RiskTokens {
    RiskTokens {
        dm_img_str: f.dm_img_str.clone(),
        dm_cover_img_str: f.dm_cover_img_str.clone(),
    }
}

#[test]
fn fixture_matches_current_protocol_version() {
    // fixture 停留在旧协议版本时，下面的逐项比对可能仍然通过（旧参数
    // 恰好没变），从而让人误以为新协议已被覆盖。版本号对不上即失败。
    assert_eq!(
        load().protocol_version,
        bili_protocol::PROTOCOL_VERSION,
        "fixture 与 bili-protocol 的协议版本不一致，需重新生成 fixture"
    );
}

#[test]
fn requests_match_baseline() {
    let f = load();
    assert!(!f.cases.is_empty(), "fixture 不得为空，否则本测试形同虚设");
    let tokens = tokens(&f);

    for case in &f.cases {
        let prepared = request::play_url(&case.input.to_params(), &tokens, &f.mixin_key, f.wts)
            .unwrap_or_else(|| panic!("用例 `{}` 的参数应当足以构造请求", case.name));

        assert_eq!(
            prepared.path, case.path,
            "用例 `{}` 的接口路径不一致",
            case.name
        );

        let expected: Vec<(String, String)> = case
            .query
            .iter()
            .map(|pair| {
                let [k, v] = pair.as_slice() else {
                    panic!("fixture 的 query 项必须是 [key, value]");
                };
                (k.clone(), v.clone())
            })
            .collect();

        assert_eq!(
            prepared.query, expected,
            "用例 `{}` 的查询参数与 Dart 基线不一致",
            case.name
        );
    }
}

#[test]
fn fallback_drops_the_ogv_only_param_group() {
    // 降级接口不认 biz_type/scene/from_client/drm_tech_type/app_id；
    // 带上它们会被整体拒绝，而不是忽略。
    let f = load();
    let tokens = tokens(&f);
    let mut params = PlayUrlParams {
        ep_id: Some(374_717),
        ..PlayUrlParams::ugc("", 221_404_201, 80)
    };
    params.bvid = None;
    params.video_type = VideoType::Pgc;

    let primary = request::play_url(&params, &tokens, &f.mixin_key, f.wts).expect("主接口请求");
    assert!(primary.query.iter().any(|(k, _)| k == "biz_type"));

    params.fallback = true;
    let fallback = request::play_url(&params, &tokens, &f.mixin_key, f.wts).expect("降级请求");
    for key in [
        "biz_type",
        "scene",
        "from_client",
        "drm_tech_type",
        "app_id",
    ] {
        assert!(
            !fallback.query.iter().any(|(k, _)| k == key),
            "降级请求不应含 {key}"
        );
    }
    assert_ne!(
        primary.path, fallback.path,
        "PGC 的降级接口与主接口必须是不同路径"
    );
}

#[test]
fn missing_identifier_is_rejected() {
    let f = load();
    let tokens = tokens(&f);

    // UGC 缺 avid/bvid
    let mut ugc = PlayUrlParams::ugc("", 1, 80);
    ugc.bvid = None;
    assert!(request::play_url(&ugc, &tokens, &f.mixin_key, f.wts).is_none());

    // PGC 缺 ep_id/season_id
    let pgc = PlayUrlParams {
        video_type: VideoType::Pgc,
        bvid: Some("BV1L94y1H7CV".to_owned()),
        ..PlayUrlParams::ugc("BV1L94y1H7CV", 1, 80)
    };
    assert!(
        request::play_url(&pgc, &tokens, &f.mixin_key, f.wts).is_none(),
        "PGC 只有 bvid 时服务端取不到流，应在构造处就拒绝"
    );

    // cid 缺失
    let no_cid = PlayUrlParams::ugc("BV1L94y1H7CV", 0, 80);
    assert!(request::play_url(&no_cid, &tokens, &f.mixin_key, f.wts).is_none());
}
