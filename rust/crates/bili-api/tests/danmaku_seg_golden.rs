#![allow(clippy::expect_used)]

use std::fs;
use std::path::Path;

/// Rust 解码的弹幕分段必须与基线 protobuf.dart 解出的结果逐字段相同。
///
/// 输入是基线**编码器**写出的字节（`tool/fixture-gen/danmaku_seg_fixture.dart`），
/// 期望值是基线**解码器**解出并规范化的结果。两端都是基线，中间换成
/// Rust：差异只可能来自解码。
#[test]
fn danmaku_segments_match_the_dart_baseline() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/danmaku_seg");
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

        let actual = bili_api::danmaku::parse_segment(&bytes)
            .unwrap_or_else(|e| panic!("{}: {e}", raw.display()));
        assert_eq!(
            serde_json::to_value(actual).expect("serializes"),
            expected,
            "{}",
            raw.display()
        );
    }
}

/// 截断的字节不得解出「一半的分段」。
///
/// 弹幕是逐条追加的，一个只解了前半段的分段在 UI 上和「这视频后半段
/// 没人发弹幕」一模一样——没有任何地方会报错，而这正是最难查的那类
/// 故障。
#[test]
fn a_truncated_segment_fails_instead_of_returning_half() {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/danmaku_seg/typical.raw.pb");
    let bytes = fs::read(path).expect("fixture readable");
    let full = bili_api::danmaku::parse_segment(&bytes).expect("完整字节可解");
    assert!(full.elems.len() > 1);

    // 砍掉最后一条弹幕的一部分
    let cut = &bytes[..bytes.len() - 3];
    assert!(
        bili_api::danmaku::parse_segment(cut).is_err(),
        "截断的分段必须报错"
    );
}
