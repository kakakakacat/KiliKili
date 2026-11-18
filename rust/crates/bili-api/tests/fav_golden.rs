//! 收藏夹那一族的 golden 快照（S8 / issue #58）。
//!
//! 对照机制同 ADR-011 决策五：本测试冻结 Rust 的解析结果。迁移时已用
//! 不 import `lib/` 的一次性 Dart parity 逐字段比对同一份 raw；对照通过后
//! 按 AGENTS.md 与被替换模型一起删除，长期保留本测试与快照。
//!
//! ## fixture 是手工构造的，但每一份都对着一条分支
//!
//! 收藏夹整条都是「我的」数据，匿名取不到；脱敏之后剩下的也就是这些示例。
//! 与其抓一份再逐条改名，不如直接写出要验的分支：
//!
//! - `fav_folder_list` 的第三项**几乎什么都没有**，证明缺字段落在基线的
//!   默认值上（`id = 0`、`title = ''`、`media_count = 0`）而不是判失败；
//! - `fav_detail` 的三项各走一条路：稿件（有 `ugc.first_cid`）、番剧
//!   （有 `ogv`，`duration` 为 0）、**已失效**（`attr = 9`，只有 `bv_id`
//!   没有 `bvid`）。最后那一项同时钉住「两个拼法都要认」；
//! - `fav_pgc` 的第二项 `progress` 与 `badge` 都是空串，钉住空串归一成
//!   `null`——不归一的话每张卡片底下都会多一条空白；
//! - `fav_notes` 的两条 id 一个是字符串一个是数字，且都超出 JS 安全整数
//!   范围，钉住「不整数化、不丢位」；
//! - `space_fav` 两组的项疏密不同，且**与 `fav_folder_list` 是同一棵子树
//!   的两种解法**（见 `fav_folder_as_sub_list_*`）。
//!
//! 它们按基线模型的字段逐一写成，证明的是**映射一致**，不能证明我们对协议
//! 的理解正确。真机确认按 #67 的约定留到 Phase 2。
//!
//! 写操作的对照物不在这里：按 ADR-013 决策六，写操作该钉的是**请求面**，
//! 见 `fav_write_requests.rs`。
//!
//! 重新生成：
//! ```sh
//! UPDATE_GOLDEN=1 cargo test -p bili-api --test fav_golden
//! ```

#![allow(clippy::expect_used)]

use std::fs;
use std::path::{Path, PathBuf};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/fav")
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
fn fav_folder_list_matches_the_frozen_snapshot() {
    check(
        "fav_folder_list.raw.json",
        "fav_folder_list.rust.json",
        |raw| value(&bili_api::fav::parse_fav_folder_list(raw).expect("样本应解析成功")),
    );
}

/// **同一份响应的第二种解法**：个人空间收藏页把它读成 `SubItem`。
///
/// 用同一个 raw 而不是再抓一份，正是为了钉住「它们确实是同一条接口」——
/// 两份 fixture 迟早会漂成两种形状，然后下一个人以为是两条接口。
#[test]
fn fav_folder_as_sub_list_matches_the_frozen_snapshot() {
    check(
        "fav_folder_list.raw.json",
        "fav_folder_as_sub_list.rust.json",
        |raw| value(&bili_api::fav::parse_fav_folder_sub_list(raw).expect("样本应解析成功")),
    );
}

#[test]
fn fav_folder_matches_the_frozen_snapshot() {
    check("fav_folder.raw.json", "fav_folder.rust.json", |raw| {
        value(&bili_api::fav::parse_fav_folder(raw).expect("样本应解析成功"))
    });
}

#[test]
fn fav_detail_matches_the_frozen_snapshot() {
    check("fav_detail.raw.json", "fav_detail.rust.json", |raw| {
        value(&bili_api::fav::parse_fav_detail(raw, false).expect("样本应解析成功"))
    });
}

/// 倒序翻页时整页要反过来，否则接不上上一页。单独冻一份，免得有人把
/// `page_desc` 当成只影响请求参数。
#[test]
fn fav_detail_page_desc_matches_the_frozen_snapshot() {
    check("fav_detail.raw.json", "fav_detail_desc.rust.json", |raw| {
        value(&bili_api::fav::parse_fav_detail(raw, true).expect("样本应解析成功"))
    });
}

#[test]
fn fav_season_detail_matches_the_frozen_snapshot() {
    check(
        "fav_season_detail.raw.json",
        "fav_season_detail.rust.json",
        |raw| value(&bili_api::fav::parse_fav_season_detail(raw, false).expect("样本应解析成功")),
    );
}

#[test]
fn fav_pgc_matches_the_frozen_snapshot() {
    check("fav_pgc.raw.json", "fav_pgc.rust.json", |raw| {
        value(&bili_api::fav::parse_fav_pgc(raw).expect("样本应解析成功"))
    });
}

#[test]
fn fav_pugv_matches_the_frozen_snapshot() {
    check("fav_pugv.raw.json", "fav_pugv.rust.json", |raw| {
        value(&bili_api::fav::parse_fav_pugv(raw).expect("样本应解析成功"))
    });
}

#[test]
fn fav_topic_matches_the_frozen_snapshot() {
    check("fav_topic.raw.json", "fav_topic.rust.json", |raw| {
        value(&bili_api::fav::parse_fav_topic(raw).expect("样本应解析成功"))
    });
}

#[test]
fn fav_article_matches_the_frozen_snapshot() {
    check("fav_article.raw.json", "fav_article.rust.json", |raw| {
        value(&bili_api::fav::parse_fav_article(raw).expect("样本应解析成功"))
    });
}

#[test]
fn fav_notes_matches_the_frozen_snapshot() {
    check("fav_notes.raw.json", "fav_notes.rust.json", |raw| {
        value(&bili_api::fav::parse_fav_notes(raw).expect("样本应解析成功"))
    });
}

#[test]
fn space_fav_matches_the_frozen_snapshot() {
    check("space_fav.raw.json", "space_fav.rust.json", |raw| {
        value(&bili_api::fav::parse_space_fav(raw).expect("样本应解析成功"))
    });
}
