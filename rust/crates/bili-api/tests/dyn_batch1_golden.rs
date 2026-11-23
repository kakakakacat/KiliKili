//! S5 续单第一批的 golden 快照（issue #150）。
//!
//! 对照机制同 ADR-011 决策五：本测试冻结 Rust 的解析结果，
//! `test/services/rust/dyn_batch1_parity_test.dart` 用迁移前那几个
//! `fromJson` 的逐行转写解析同一份 raw，与快照逐字段比对。
//!
//! ## fixture 来源：**四份全是手工构造的**
//!
//! 按仓库 `tests/fixtures/README.md` 的默认假设对待。各自的理由：
//!
//! - `article_info`：这条接口要 WBI 签名，匿名也能取，但要一个真实 cv 号
//!   加一次签名才拿得到；而它的响应里除了计数就是作者信息，脱敏之后剩下
//!   的就是一组数字——与其抓一份再逐个改，不如直接写出要验的形状。
//! - `article_list`：同上，另外它的三条 `articles` 是**刻意写成三种形态**
//!   的（stats 齐全 / stats 缺字段 / 整个 stats 缺失），抓一份真实文集大
//!   概率三条都一样。
//! - `topic_top` / `topic_rcmd`：要验的是「id 或 name 缺失则整条丢掉」这条
//!   基线规则，真实响应里不会有这种条目。
//! - `bubble`：部落是灰度功能，当前账号拿不到非空响应。
//!
//! 四份都**刻意保留了未被映射的键**（`article_info` 的 `view`/`coin`、
//! `article_list` 的 `state`/`summary`、`topic_top` 的 `topic_label`、
//! `bubble` 分类的 `type`），用来证明多余字段会被忽略。
//!
//! 它们证明的是**映射一致**，不能证明我们对协议的理解正确。真机确认按
//! #67 的约定留到 Phase 2。
//!
//! 重新生成：
//! ```sh
//! UPDATE_GOLDEN=1 cargo test -p bili-api --test dyn_batch1_golden
//! ```

#![allow(clippy::expect_used)]

use std::fs;
use std::path::{Path, PathBuf};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/dyn_batch1")
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
fn article_info_matches_the_frozen_snapshot() {
    check("article_info.raw.json", "article_info.rust.json", |raw| {
        value(&bili_api::dynamic::parse_article_info(raw).expect("样本应解析成功"))
    });
}

#[test]
fn article_list_matches_the_frozen_snapshot() {
    check("article_list.raw.json", "article_list.rust.json", |raw| {
        value(&bili_api::dynamic::parse_article_list(raw).expect("样本应解析成功"))
    });
}

#[test]
fn topic_top_matches_the_frozen_snapshot() {
    check("topic_top.raw.json", "topic_top.rust.json", |raw| {
        value(&bili_api::dynamic::parse_topic_top(raw).expect("样本应解析成功"))
    });
}

#[test]
fn topic_rcmd_matches_the_frozen_snapshot() {
    check("topic_rcmd.raw.json", "topic_rcmd.rust.json", |raw| {
        value(&bili_api::dynamic::parse_topic_rcmd(raw).expect("样本应解析成功"))
    });
}

#[test]
fn bubble_matches_the_frozen_snapshot() {
    check("bubble.raw.json", "bubble.rust.json", |raw| {
        value(&bili_api::dynamic::parse_bubble(raw).expect("样本应解析成功"))
    });
}
