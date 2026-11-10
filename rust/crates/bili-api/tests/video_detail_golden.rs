//! 视频详情 golden 对照测试。
//!
//! 期望值由 Dart 基线实现产出（`tool/fixture-gen/video_detail_fixture.dart`），
//! 不是本实现的自我验证：Rust 必须对同一份响应产出与 Dart **逐字段相同**
//! 的规范化结果，否则 shadow mode 的比对会被我们自己的「改进」淹没，
//! 真正的解析差异反而淹没在噪声里。
//!
//! 重新生成期望值：
//! ```sh
//! dart tool/fixture-gen/video_detail_fixture.dart
//! ```
//! 基线模型改了而本实现没跟上（或反之），本测试会红。

// 测试里 expect 是对的：fixture 缺失或损坏时就该崩掉并指出原因。
#![allow(clippy::expect_used)]

use std::fs;
use std::path::{Path, PathBuf};

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/video_detail")
}

/// 收集所有 `*.raw.json` 及其期望值路径。
fn cases() -> Vec<(String, PathBuf, PathBuf)> {
    let dir = fixture_dir();
    let mut out: Vec<_> = fs::read_dir(&dir)
        .expect("fixture 目录必须存在")
        .filter_map(|e| {
            let path = e.expect("目录项可读").path();
            let name = path.file_name()?.to_str()?.to_owned();
            let stem = name.strip_suffix(".raw.json")?.to_owned();
            let expected = dir.join(format!("{stem}.normalized.json"));
            Some((stem, path, expected))
        })
        .collect();
    out.sort();
    out
}

/// 找出两份 JSON 第一处不同，并给出其路径。
///
/// 直接 `assert_eq!` 两个大 JSON 会打印上百行，真正的差异要靠肉眼找。
/// 解析差异往往只有一个字段，指出路径才让失败可诊断。
fn first_difference(a: &serde_json::Value, b: &serde_json::Value, path: &str) -> Option<String> {
    use serde_json::Value;
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            let mut keys: Vec<&String> = x.keys().chain(y.keys()).collect();
            keys.sort_unstable();
            keys.dedup();
            keys.into_iter().find_map(|k| {
                let next = format!("{path}.{k}");
                match (x.get(k), y.get(k)) {
                    (Some(xv), Some(yv)) => first_difference(xv, yv, &next),
                    (Some(xv), None) => Some(format!("{next}: Rust 有 {xv}，Dart 无此字段")),
                    (None, Some(yv)) => Some(format!("{next}: Dart 有 {yv}，Rust 无此字段")),
                    (None, None) => None,
                }
            })
        }
        (Value::Array(x), Value::Array(y)) => {
            if x.len() != y.len() {
                return Some(format!(
                    "{path}: 长度不同，Rust {} 项 / Dart {} 项",
                    x.len(),
                    y.len()
                ));
            }
            x.iter()
                .zip(y)
                .enumerate()
                .find_map(|(i, (xv, yv))| first_difference(xv, yv, &format!("{path}[{i}]")))
        }
        _ if a == b => None,
        _ => Some(format!("{path}: Rust {a} / Dart {b}")),
    }
}

#[test]
fn normalized_output_matches_dart_baseline() {
    let cases = cases();
    assert!(
        !cases.is_empty(),
        "没有 fixture 时本测试形同虚设，至少需要一个 *.raw.json"
    );

    for (name, raw_path, expected_path) in cases {
        let raw = fs::read_to_string(&raw_path).expect("raw fixture 可读");
        let expected_src = fs::read_to_string(&expected_path).unwrap_or_else(|_| {
            panic!(
                "缺少 {} 的期望值，请运行 dart tool/fixture-gen/video_detail_fixture.dart",
                name
            )
        });

        let parsed = bili_api::video_detail::parse(&raw)
            .unwrap_or_else(|e| panic!("样本 `{name}` 应解析成功，实际失败: {e}"));
        let got = serde_json::to_value(&parsed).expect("实体可序列化");
        let expected: serde_json::Value =
            serde_json::from_str(&expected_src).expect("期望值必须是合法 JSON");

        if let Some(diff) = first_difference(&got, &expected, name.as_str()) {
            panic!("样本 `{name}` 的规范化结果与 Dart 基线不一致：{diff}");
        }
    }
}

#[test]
fn every_raw_fixture_has_expectations() {
    // 只加 raw 而忘了重新生成期望值，会让新样本悄悄不参与对照。
    for (name, _, expected) in cases() {
        assert!(
            expected.exists(),
            "样本 `{name}` 缺少 .normalized.json，重新生成 fixture 后再提交"
        );
    }
}
