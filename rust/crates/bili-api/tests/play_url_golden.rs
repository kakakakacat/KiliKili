//! 取流 golden 对照测试。
//!
//! 期望值由 Dart 基线实现产出（`tool/fixture-gen/play_url_fixture.dart`），
//! 不是本实现的自我验证：Rust 必须对同一份响应产出与 Dart **逐字段相同**
//! 的规范化结果，否则 shadow mode 的比对会被我们自己的「改进」淹没。
//!
//! 样本的文件名指出响应形状——响应本身不足以区分（PUGV 降级与 UGC 的
//! 外层形状相同）：
//!
//! ```text
//! <名字>.<ugc|pgc|pugv>[-fallback].raw.json
//! ```
//!
//! 重新生成期望值：
//! ```sh
//! dart tool/fixture-gen/play_url_fixture.dart
//! ```

// 测试里 expect 是对的：fixture 缺失或损坏时就该崩掉并指出原因。
#![allow(clippy::expect_used)]

use std::fs;
use std::path::{Path, PathBuf};

use bili_api::playurl::{self, Outcome};
use bili_domain::video::VideoType;

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/play_url")
}

struct Case {
    name: String,
    raw: PathBuf,
    expected: PathBuf,
    video_type: VideoType,
    fallback: bool,
}

/// 收集所有 `*.raw.json` 及其期望值路径与响应形状。
fn cases() -> Vec<Case> {
    let dir = fixture_dir();
    let mut out: Vec<Case> = fs::read_dir(&dir)
        .expect("fixture 目录必须存在")
        .filter_map(|e| {
            let path = e.expect("目录项可读").path();
            let name = path.file_name()?.to_str()?.to_owned();
            let stem = name.strip_suffix(".raw.json")?.to_owned();

            let (stem_without_shape, fallback) = match stem.strip_suffix("-fallback") {
                Some(rest) => (rest.to_owned(), true),
                None => (stem.clone(), false),
            };
            let video_type = match stem_without_shape.rsplit('.').next()? {
                "ugc" => VideoType::Ugc,
                "pgc" => VideoType::Pgc,
                "pugv" => VideoType::Pugv,
                other => panic!("样本 `{name}` 的文件名含未知形状 `{other}`"),
            };

            Some(Case {
                expected: dir.join(format!("{stem}.normalized.json")),
                raw: path,
                name: stem,
                video_type,
                fallback,
            })
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// 找出两份 JSON 第一处不同，并给出其路径。
///
/// 直接 `assert_eq!` 两个大 JSON 会打印上百行，真正的差异要靠肉眼找。
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
        // 数值比较走 f64：两端对 minBufferTime 之类的浮点各自序列化，
        // `1.5` 与 `1.50` 是同一个值而不是差异。
        (Value::Number(x), Value::Number(y)) if x.as_f64() == y.as_f64() => None,
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

    for case in cases {
        let raw = fs::read_to_string(&case.raw).expect("raw fixture 可读");
        let expected_src = fs::read_to_string(&case.expected).unwrap_or_else(|_| {
            panic!(
                "缺少 {} 的期望值，请运行 dart tool/fixture-gen/play_url_fixture.dart",
                case.name
            )
        });

        let outcome = playurl::parse(&raw, case.video_type, case.fallback)
            .unwrap_or_else(|e| panic!("样本 `{}` 应解析成功，实际失败: {e}", case.name));
        let Outcome::Ready(play) = outcome else {
            panic!("样本 `{}` 被判为需要降级，不该出现在对照样本里", case.name);
        };

        let got = serde_json::to_value(&play).expect("实体可序列化");
        let expected: serde_json::Value =
            serde_json::from_str(&expected_src).expect("期望值必须是合法 JSON");

        if let Some(diff) = first_difference(&got, &expected, &case.name) {
            panic!(
                "样本 `{}` 的规范化结果与 Dart 基线不一致：{diff}",
                case.name
            );
        }
    }
}

#[test]
fn every_raw_fixture_has_expectations() {
    // 只加 raw 而忘了重新生成期望值，会让新样本悄悄不参与对照。
    for case in cases() {
        assert!(
            case.expected.exists(),
            "样本 `{}` 缺少 .normalized.json，重新生成 fixture 后再提交",
            case.name
        );
    }
}
