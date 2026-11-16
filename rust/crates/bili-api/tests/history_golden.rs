//! 观看历史 / 稍后再看 / 播放列表的 golden 快照（S7 / issue #57）。
//!
//! 对照机制同 ADR-011 决策五：本测试冻结 Rust 的解析结果，
//! `test/services/rust/history_parity_test.dart` 用迁移前那几个 `fromJson`
//! 的逐行转写解析同一份 raw，与快照逐字段比对。
//!
//! ## fixture 全部是手工构造的
//!
//! 这三条都是**登录后的个人数据**，匿名取不到；脱敏之后剩下的也就是这几条
//! 示例。与其抓一份再逐条改名，不如直接写出要验的分支：
//!
//! - `history_page` 三项分别是稿件、直播、专栏，覆盖
//!   **`cid: 0` 要被抹成 `null`**（直播那条）、`progress: -1`（没看过/直播
//!   中）、`covers` 多图与 `cover` 单图两种封面、以及 `history` 整个缺字段；
//!   `cursor`、`long_title`、`author_face`、`part`、`dt`、`is_finish` 等
//!   **刻意留着不消费**，用来证明会被忽略；
//! - `later_page` 三项覆盖：普通稿件（`rotate: 0`）、番剧
//!   （`rotate: 1` 要宽高对调、`subtitle` 要切掉季名前缀、`pgc_label` 有值）、
//!   充电稿件（`charging_pay.level` 存在即算充电，`pgc_label` 是空串要当没有）；
//! - `media_list` 两项覆盖：`pages` 有多项时只取第一个 cid、`pages` 为空数组时
//!   `first_page_cid` 是 `null`，以及 `badge.text` 空串与有值两种。
//!
//! 它们按基线模型的字段逐一写成，证明的是**映射一致**，不能证明我们对协议
//! 的理解正确。真机确认按 #67 的约定留到 Phase 2。
//!
//! 重新生成：
//! ```sh
//! UPDATE_GOLDEN=1 cargo test -p bili-api --test history_golden
//! ```

#![allow(clippy::expect_used)]

use std::fs;
use std::path::{Path, PathBuf};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/history")
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

/// 搜索历史走的是同一个解析，只换路径与入参；再放一份 fixture 只会得到
/// 字节相同的第二个文件。
#[test]
fn history_page_matches_the_frozen_snapshot() {
    check("history_page.raw.json", "history_page.rust.json", |raw| {
        value(&bili_api::history::parse_history_page(raw).expect("样本应解析成功"))
    });
}

#[test]
fn history_status_matches_the_frozen_snapshot() {
    check(
        "history_status.raw.json",
        "history_status.rust.json",
        |raw| value(&bili_api::history::parse_history_status(raw).expect("样本应解析成功")),
    );
}

#[test]
fn later_page_matches_the_frozen_snapshot() {
    check("later_page.raw.json", "later_page.rust.json", |raw| {
        value(&bili_api::history::parse_later_page(raw).expect("样本应解析成功"))
    });
}

#[test]
fn media_list_matches_the_frozen_snapshot() {
    check("media_list.raw.json", "media_list.rust.json", |raw| {
        value(&bili_api::history::parse_media_list(raw).expect("样本应解析成功"))
    });
}

/// 协议里没有「播放进度毫秒」这个字段，它是基线 `HistoryItemModel` 里那句
/// `progress == -1 ? 0 : progress * 1000` 的规则。规则进 Rust，所以在这里验。
#[test]
fn a_progress_of_minus_one_means_the_very_beginning() {
    let page = bili_api::history::parse_history_page(
        &fs::read_to_string(fixture("history_page.raw.json")).expect("raw fixture 可读"),
    )
    .expect("样本应解析成功");

    assert_eq!(
        page.list[0].playback_progress_ms(),
        Some(125_000),
        "正常进度换算成毫秒"
    );
    assert_eq!(
        page.list[1].playback_progress_ms(),
        Some(0),
        "-1 是「没看过」，不是负进度"
    );
}
