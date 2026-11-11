#![allow(clippy::expect_used)]

//! Rust 解出的字幕轨必须与基线 `DmGrpc.dmViewToSubtitle` 逐字段相同，
//! **顺序也相同**（基线排过序，顺序决定播放器默认选哪条）。
//!
//! fixture 链同弹幕分段（`tool/fixture-gen/dm_view_fixture.dart`）：
//! 基线编码器写字节，基线解码器 + 规范化产出期望值，中间换成 Rust。

use std::fs;
use std::path::Path;

#[test]
fn dm_view_subtitles_match_the_dart_baseline() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dm_view");
    let mut raws: Vec<_> = fs::read_dir(dir)
        .expect("fixture directory exists")
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            path.to_string_lossy().ends_with(".raw.pb").then_some(path)
        })
        .collect();
    raws.sort();
    assert!(!raws.is_empty(), "fixture 目录为空，本测试形同虚设");

    for raw in raws {
        let expected_path = raw.to_string_lossy().replace(".raw.pb", ".normalized.json");
        let bytes = fs::read(&raw).expect("raw fixture readable");
        let expected: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&expected_path).expect("Dart-generated golden exists"),
        )
        .expect("golden is JSON");

        let actual = bili_api::dm_view::parse_subtitles(&bytes)
            .unwrap_or_else(|e| panic!("{}: {e}", raw.display()));
        assert_eq!(
            serde_json::to_value(actual).expect("serializes"),
            expected,
            "{}",
            raw.display()
        );
    }
}

/// 截断的响应不得解出「一半的字幕列表」。
///
/// 少一条字幕轨的表现是「这个视频没有英文字幕」——界面上没有任何提示，
/// 用户只会以为 up 主没传。
#[test]
fn a_truncated_reply_fails_instead_of_returning_half() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dm_view/typical.raw.pb");
    let bytes = fs::read(path).expect("fixture readable");
    let full = bili_api::dm_view::parse_subtitles(&bytes).expect("完整字节可解");
    assert!(full.len() > 1);

    let cut = &bytes[..bytes.len() - 3];
    assert!(
        bili_api::dm_view::parse_subtitles(cut).is_err(),
        "截断的响应必须报错"
    );
}
