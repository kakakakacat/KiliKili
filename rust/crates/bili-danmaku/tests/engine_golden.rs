#![allow(clippy::expect_used)]

//! Rust 引擎的分桶结果必须与基线 `handleDanmaku` 逐字段相同。
//!
//! fixture 链沿用 PR #41 的做法（`tool/fixture-gen/danmaku_engine_fixture.dart`）：
//!
//! ```text
//! <name>.case.json      人写的样本：引擎参数 + 一个分段的弹幕
//!   ↓ 基线的 protobuf 编码器
//! <name>.raw.pb         原始字节，与线上响应同形
//!   ↓ 基线的解码器 + 基线的 bucketDanmaku + danmaku_normalize.dart
//! <name>.buckets.json   期望值
//! ```
//!
//! 两端都是基线，中间换成 Rust：差异只可能来自解码或引擎。`.case.json`
//! 里的 `options` 由两边各自解读（Dart 构造 `RuleFilter`，Rust 构造
//! `FilterRules`），这是**刻意**的——规则的解读本身就是要对齐的东西。

use std::fs;
use std::path::Path;

use bili_danmaku::engine::{Buckets, EngineOptions, ingest};
use bili_danmaku::rule::FilterRules;
use serde_json::Value;

fn fixture_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/engine")
}

/// 按 `.case.json` 的 `options` 构造引擎参数。
///
/// 字段缺席一律取基线的默认值（关闭 / 空），与 `.case.json` 里只写出
/// 关心的字段这一写法配套。
fn options_from(case: &Value) -> EngineOptions {
    let options = &case["options"];
    let strings = |key: &str| -> Vec<String> {
        options["rules"][key]
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .filter_map(|v| v.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default()
    };
    let patterns = strings("regexes");

    EngineOptions {
        is_login: options["is_login"].as_bool().unwrap_or(false),
        self_mid_hash: options["self_mid_hash"]
            .as_str()
            .unwrap_or_default()
            .to_owned(),
        merge: options["merge"].as_bool().unwrap_or(false),
        rules: FilterRules::compile(strings("keywords"), &patterns, strings("uids"))
            .expect("fixture 里的规则必须是 Rust 能接受的子集"),
    }
}

#[test]
fn engine_output_matches_the_dart_baseline() {
    let dir = fixture_dir();
    let mut cases: Vec<_> = fs::read_dir(&dir)
        .expect("fixture directory exists")
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            path.to_string_lossy()
                .ends_with(".case.json")
                .then_some(path)
        })
        .collect();
    cases.sort();
    assert!(!cases.is_empty(), "fixture 目录为空，本测试形同虚设");

    for case_path in cases {
        let stem = case_path.to_string_lossy().replace(".case.json", "");
        let case: Value = serde_json::from_str(&fs::read_to_string(&case_path).expect("case 可读"))
            .expect("case 是 JSON");
        let bytes = fs::read(format!("{stem}.raw.pb")).expect("基线编码器写出的字节存在");
        let expected: Value = serde_json::from_str(
            &fs::read_to_string(format!("{stem}.buckets.json")).expect("golden 存在"),
        )
        .expect("golden 是 JSON");

        let segment = bili_api::danmaku::parse_segment(&bytes)
            .unwrap_or_else(|e| panic!("{}: {e}", case_path.display()));

        let mut buckets = Buckets::default();
        ingest(&options_from(&case), &mut buckets, segment.elems);

        assert_eq!(
            serde_json::to_value(&buckets).expect("serializes"),
            expected,
            "{}",
            case_path.display()
        );
    }
}

/// 同一个分段并入两次，结果必须与并入一次相同。
///
/// 这不是重复测试会话去重（那在 `session` 的单测里）：这里钉的是**桶内
/// 数据没有可变的全局状态**——引擎只读 `options`，合并表每次新建。
#[test]
fn ingesting_the_same_segment_twice_doubles_nothing_but_the_elements() {
    let path = fixture_dir().join("merge_and_filter.case.json");
    let case: Value =
        serde_json::from_str(&fs::read_to_string(&path).expect("case 可读")).expect("case 是 JSON");
    let bytes = fs::read(
        path.to_string_lossy()
            .replace(".case.json", ".raw.pb")
            .as_str(),
    )
    .expect("字节存在");
    let options = options_from(&case);
    let segment = bili_api::danmaku::parse_segment(&bytes).expect("可解");

    let mut once = Buckets::default();
    ingest(&options, &mut once, segment.elems.clone());
    let mut twice = Buckets::default();
    ingest(&options, &mut twice, segment.elems.clone());
    ingest(&options, &mut twice, segment.elems);

    // 合并表不跨调用，所以第二次并入会再产生一批「首条」，总数翻倍；
    // 若某次改动让合并表跨了调用，这里会变成 1:1，测试即报警。
    assert_eq!(twice.elem_count(), once.elem_count() * 2);
}
