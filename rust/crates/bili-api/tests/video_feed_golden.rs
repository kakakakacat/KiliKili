//! 视频流五条接口的 golden 快照（S3 / issue #53）。
//!
//! 对照机制同 SponsorBlock 样板（ADR-011 决策五）：
//!
//! 1. **本测试**把 Rust 的解析结果冻结成 `*.rust.json`；
//! 2. **`test/services/rust/video_feed_parity_test.dart`** 用还活着的
//!    Dart 基线模型（`HotVideoItemModel` 等）解析同一份 `*.raw.json`，
//!    与快照逐字段比对。
//!
//! 单看任一侧都只是「自己跟自己一致」，合起来才构成对照。parity 测试随
//! 基线模型一起删；快照长期留下，是迁移当时行为的冻结证据。
//!
//! 样本取自真实响应并已脱敏：稿件 id、cid、mid、昵称、头像与封面地址、
//! 标题、简介、发布地区全部替换过（脱敏脚本的规则见 PR 说明）。
//!
//! 重新生成：
//! ```sh
//! UPDATE_GOLDEN=1 cargo test -p bili-api --test video_feed_golden
//! ```
//! 生成后**必须**再跑一遍 Dart 侧的 parity 测试。

// fixture 缺失或损坏时就该崩掉并指出原因。
#![allow(clippy::expect_used)]

use std::fs;
use std::path::{Path, PathBuf};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/video_feed")
        .join(name)
}

fn check(raw_name: &str, snapshot_name: &str, parse: impl Fn(&str) -> serde_json::Value) {
    let raw = fs::read_to_string(fixture(raw_name)).expect("raw fixture 可读");
    let got = parse(&raw);

    let snapshot_path = fixture(snapshot_name);
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        let pretty = serde_json::to_string_pretty(&got).expect("可序列化");
        fs::write(&snapshot_path, format!("{pretty}\n")).expect("快照可写");
        return;
    }

    let expected: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(&snapshot_path)
            .expect("缺少快照，用 UPDATE_GOLDEN=1 重新生成后再跑 Dart parity 测试"),
    )
    .expect("快照必须是合法 JSON");

    assert_eq!(got, expected, "{raw_name} 的解析结果与冻结快照不一致");
}

fn value<T: serde::Serialize>(v: &T) -> serde_json::Value {
    serde_json::to_value(v).expect("实体可序列化")
}

#[test]
fn hot_matches_the_frozen_snapshot() {
    check("hot.raw.json", "hot.rust.json", |raw| {
        value(&bili_api::video_feed::parse_list_in_data(raw).expect("样本应解析成功"))
    });
}

#[test]
fn rank_matches_the_frozen_snapshot() {
    check("rank.raw.json", "rank.rust.json", |raw| {
        value(&bili_api::video_feed::parse_list_in_data(raw).expect("样本应解析成功"))
    });
}

#[test]
fn related_matches_the_frozen_snapshot() {
    check("related.raw.json", "related.rust.json", |raw| {
        value(&bili_api::video_feed::parse_related(raw).expect("样本应解析成功"))
    });
}

#[test]
fn series_list_matches_the_frozen_snapshot() {
    check("series_list.raw.json", "series_list.rust.json", |raw| {
        value(&bili_api::video_feed::parse_series_list(raw).expect("样本应解析成功"))
    });
}

#[test]
fn series_one_matches_the_frozen_snapshot() {
    check("series_one.raw.json", "series_one.rust.json", |raw| {
        value(&bili_api::video_feed::parse_series_one(raw).expect("样本应解析成功"))
    });
}

#[test]
fn precious_matches_the_frozen_snapshot() {
    check("precious.raw.json", "precious.rust.json", |raw| {
        value(&bili_api::video_feed::parse_precious(raw).expect("样本应解析成功"))
    });
}
