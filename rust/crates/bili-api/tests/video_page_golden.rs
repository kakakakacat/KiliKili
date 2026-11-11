//! 播放页附属接口的 golden 快照（S3 第二批 / issue #53）。
//!
//! 对照机制同 ADR-011 决策五：本测试冻结 Rust 的解析结果，
//! `test/services/rust/video_page_parity_test.dart` 用迁移前那几个
//! `fromJson` 的逐行转写解析同一份 raw，与快照逐字段比对。
//!
//! ## fixture 的来源，分两类，不要混
//!
//! **真实响应（已脱敏）**：`online_total`、`video_shot`、`play_info`、
//! `note_list`。脱敏掉的是 cvid、mid、昵称、头像、笔记标题与摘要、CDN
//! 地址；`play_info` 的真实响应里含 `ip_info`、`login_mid` 等，所以只留下
//! 解析用到的键，外加两个无害的未知键（`has_next`、`page_no`）用来证明
//! 多余字段会被忽略。
//!
//! **手工构造**：`ai_conclusion`、`ai_conclusion_processing`、
//! `play_info_rich`。前两个需要登录态才拿得到（匿名是 `code -101`），后者
//! 的字幕与看点服务端只对登录用户下发。它们按上游接口文档写成，**不是**
//! 真实抓取——这几份快照证明的是「我们对协议的理解」自洽，不能证明理解
//! 本身正确。这三条路径的真机确认留到 Phase 2。
//!
//! 重新生成：
//! ```sh
//! UPDATE_GOLDEN=1 cargo test -p bili-api --test video_page_golden
//! ```

#![allow(clippy::expect_used)]

use std::fs;
use std::path::{Path, PathBuf};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/video_page")
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
        &fs::read_to_string(&snapshot_path).expect("缺少快照，用 UPDATE_GOLDEN=1 重新生成"),
    )
    .expect("快照必须是合法 JSON");

    assert_eq!(got, expected, "{raw_name} 的解析结果与冻结快照不一致");
}

fn value<T: serde::Serialize>(v: &T) -> serde_json::Value {
    serde_json::to_value(v).expect("实体可序列化")
}

#[test]
fn online_total_matches_the_frozen_snapshot() {
    check("online_total.raw.json", "online_total.rust.json", |raw| {
        value(&bili_api::video_page::parse_online_total(raw).expect("样本应解析成功"))
    });
}

#[test]
fn ai_conclusion_matches_the_frozen_snapshot() {
    check("ai_conclusion.raw.json", "ai_conclusion.rust.json", |raw| {
        value(&bili_api::video_page::parse_ai_conclusion(raw).expect("样本应解析成功"))
    });
}

#[test]
fn play_info_matches_the_frozen_snapshot() {
    check("play_info.raw.json", "play_info.rust.json", |raw| {
        value(&bili_api::video_page::parse_play_info(raw).expect("样本应解析成功"))
    });
}

#[test]
fn play_info_with_subtitles_matches_the_frozen_snapshot() {
    check(
        "play_info_rich.raw.json",
        "play_info_rich.rust.json",
        |raw| value(&bili_api::video_page::parse_play_info(raw).expect("样本应解析成功")),
    );
}

#[test]
fn video_shot_matches_the_frozen_snapshot() {
    check("video_shot.raw.json", "video_shot.rust.json", |raw| {
        value(&bili_api::video_page::parse_video_shot(raw).expect("样本应解析成功"))
    });
}

#[test]
fn note_list_matches_the_frozen_snapshot() {
    check("note_list.raw.json", "note_list.rust.json", |raw| {
        value(&bili_api::video_page::parse_note_list(raw).expect("样本应解析成功"))
    });
}

#[test]
fn an_unfinished_ai_conclusion_is_an_error_carrying_the_inner_code() {
    // 基线靠内层 code == 1 提示「AI 处理中，请稍后再试」；外层 code 是 0，
    // 从信封看这次请求是成功的。
    let raw = fs::read_to_string(fixture("ai_conclusion_processing.raw.json")).expect("可读");
    let err = bili_api::video_page::parse_ai_conclusion(&raw).expect_err("应判为失败");
    assert_eq!(err.code(), Some(1), "必须带回内层 code");
}
