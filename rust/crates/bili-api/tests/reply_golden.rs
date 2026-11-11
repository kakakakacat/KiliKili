//! 评论 REST 只读接口的 golden 快照（S1 / issue #51）。
//!
//! 对照机制同 ADR-011 决策五：本测试冻结 Rust 的解析结果，
//! `test/services/rust/reply_parity_test.dart` 用迁移前那几个 `fromJson`
//! 的逐行转写解析同一份 raw，与快照逐字段比对。
//!
//! ## fixture 的来源，分两类，不要混
//!
//! **真实响应（已脱敏、已裁剪）**：`reply_list_cursor`（未登录形态
//! `/x/v2/reply/main`）、`reply_list_paged`（`/x/v2/reply`）、`reply_reply`
//! （楼中楼）。三份都是匿名抓的真实响应，脱敏掉 rpid/oid/mid（含 `*_str`）、
//! 昵称、头像、个性签名与评论正文，并删去没有消费者的装饰子树（勋章、
//! 挂件、大会员标签、等级、认证）。**没有删光**：`handle`、`sex`、`rank`、
//! `session_id`、`support_mode`、`note` 等未被映射的键仍留在里面，用来
//! 证明多余字段会被忽略。
//!
//! **手工构造**：`interaction`、`emote_panel`。两者都要登录态——匿名请求
//! 评论区开关状态是 `code -101`，表情包面板返回 `packages: null`（已实测，
//! 后者作为「空包」的真实样本保留在单元测试里）。它们按基线模型的字段写成，
//! 证明的是映射一致，**不能**证明我们对协议的理解正确；真机确认留 Phase 2。
//!
//! 重新生成：
//! ```sh
//! UPDATE_GOLDEN=1 cargo test -p bili-api --test reply_golden
//! ```

#![allow(clippy::expect_used)]

use std::fs;
use std::path::{Path, PathBuf};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/reply")
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
fn anonymous_reply_list_matches_the_frozen_snapshot() {
    check(
        "reply_list_cursor.raw.json",
        "reply_list_cursor.rust.json",
        |raw| value(&bili_api::reply::parse_reply_list(raw).expect("样本应解析成功")),
    );
}

#[test]
fn paged_reply_list_matches_the_frozen_snapshot() {
    check(
        "reply_list_paged.raw.json",
        "reply_list_paged.rust.json",
        |raw| value(&bili_api::reply::parse_reply_list(raw).expect("样本应解析成功")),
    );
}

#[test]
fn reply_reply_list_matches_the_frozen_snapshot() {
    check("reply_reply.raw.json", "reply_reply.rust.json", |raw| {
        value(&bili_api::reply::parse_reply_reply_list(raw).expect("样本应解析成功"))
    });
}

#[test]
fn interaction_matches_the_frozen_snapshot() {
    check("interaction.raw.json", "interaction.rust.json", |raw| {
        value(&bili_api::reply::parse_reply_interaction(raw).expect("样本应解析成功"))
    });
}

#[test]
fn emote_panel_matches_the_frozen_snapshot() {
    check("emote_panel.raw.json", "emote_panel.rust.json", |raw| {
        value(&bili_api::reply::parse_emote_panel(raw).expect("样本应解析成功"))
    });
}

/// 两条请求形态的响应形状确实不同——这正是不能把 `cursor` 与 `page` 合并
/// 成一个字段的理由，固定下来免得将来有人「顺手统一」。
#[test]
fn the_two_request_shapes_paginate_differently() {
    let cursor_form = fs::read_to_string(fixture("reply_list_cursor.raw.json")).expect("可读");
    let paged_form = fs::read_to_string(fixture("reply_list_paged.raw.json")).expect("可读");

    let anonymous = bili_api::reply::parse_reply_list(&cursor_form).expect("应解析成功");
    let logged_in = bili_api::reply::parse_reply_list(&paged_form).expect("应解析成功");

    assert!(
        anonymous.cursor.is_some() && anonymous.page.is_none(),
        "未登录形态只有 cursor"
    );
    assert!(
        logged_in.page.is_some() && logged_in.cursor.is_none(),
        "登录形态只有 page"
    );
    assert!(
        anonymous
            .cursor
            .and_then(|c| c.next_offset)
            .is_some_and(|o| !o.is_empty()),
        "未登录形态要靠 next_offset 翻页"
    );
}
