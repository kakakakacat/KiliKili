//! SponsorBlock golden 快照。
//!
//! # typed 直连的对照机制（样板，见 issue #64 / ADR-011 #48）
//!
//! 往返模式下期望值由 Dart 基线产出，保证「不是 Rust 的自我验证」。
//! typed 直连删掉了 Dart 模型，这个来源会随之消失，因此分两半：
//!
//! 1. **本测试**把 Rust 的解析结果快照成 `segments.rust.json`；
//! 2. **`test/services/rust/sponsor_block_parity_test.dart`** 用还活着的
//!    Dart 基线模型解析同一份 `segments.raw.json`，与那份快照逐字段比对。
//!
//! 两者合起来才构成对照：单独看本测试只是"Rust 跟自己一致"。基线模型哪天
//! 真的删干净了，快照仍然留在仓库里，作为迁移当时行为的冻结证据。
//!
//! 重新生成快照：
//! ```sh
//! UPDATE_GOLDEN=1 cargo test -p bili-api --test sponsor_block_golden
//! ```
//! 生成后必须跑一遍 Dart 侧的 parity 测试，否则等于把 Rust 的改动直接
//! 盖进"期望值"。

// 测试里 expect 是对的：fixture 缺失或损坏时就该崩掉并指出原因。
#![allow(clippy::expect_used)]

use std::fs;
use std::path::{Path, PathBuf};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sponsor_block")
        .join(name)
}

#[test]
fn segments_match_the_frozen_snapshot() {
    let raw = fs::read_to_string(fixture("segments.raw.json")).expect("raw fixture 可读");
    let parsed = bili_api::sponsor_block::parse_segments(&raw).expect("样本应解析成功");
    let got = serde_json::to_value(&parsed).expect("实体可序列化");

    let snapshot_path = fixture("segments.rust.json");
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

    assert_eq!(got, expected, "解析结果与冻结快照不一致");
}

#[test]
fn incomplete_entries_are_dropped_rather_than_failing_the_batch() {
    // fixture 的最后一条缺 cid / actionType / videoDuration，但字段齐全到
    // 足以跳转，必须留下；真正残缺的条目由单元测试覆盖。
    let raw = fs::read_to_string(fixture("segments.raw.json")).expect("raw fixture 可读");
    let parsed = bili_api::sponsor_block::parse_segments(&raw).expect("样本应解析成功");
    assert_eq!(parsed.len(), 5, "样本里五条都应当被保留");
    assert!(parsed[4].cid.is_none());
    assert!(parsed[4].action_type.is_none());
}
