//! 杂项只读接口的 golden 快照（S15 / issue #66）。
//!
//! 对照机制同 ADR-011 决策五：本测试冻结 Rust 的解析结果，
//! `test/services/rust/misc_parity_test.dart` 用迁移前那几个 `fromJson`
//! 的逐行转写解析同一份 raw，与快照逐字段比对。
//!
//! ## fixture 的来源，分两类，不要混
//!
//! **真实响应（已脱敏、已裁剪）**：`match_info`。匿名可取
//! （`/x/esports/match/info?cid=1&platform=2`）。脱敏掉队伍 uid、CDN 路径
//! 与赛事页 URL；裁掉了七十多个键里没有消费者的那些，但**保留**若干未被
//! 映射的键（`etime`、`game_state`、`status`、`live_status`、`sub_title`、
//! 队伍与赛季的 `id`），用来证明多余字段会被忽略。
//!
//! **手工构造**：`bgm_detail`、`bgm_recommend`、`black_list`、
//! `danmaku_filter`。原因各不相同，都不是「懒得抓」：
//! - 黑名单与弹幕屏蔽规则是**账号自己的数据**，匿名取不到，而把真实账号的
//!   拉黑名单与屏蔽词写进仓库，脱敏之后也没剩下什么可证明的；
//! - 音乐两条接口匿名可达，但要一个真实的 `music_id`，而站内没有公开入口
//!   能枚举（试过 recommend 的空 id：返回 `code 0` + 空列表，证明不了解析）。
//!
//! 它们按基线模型的字段逐一写成，证明的是**映射一致**，不能证明我们对协议
//! 的理解正确。真机确认按 #67 的约定留到 Phase 2。
//!
//! 重新生成：
//! ```sh
//! UPDATE_GOLDEN=1 cargo test -p bili-api --test misc_golden
//! ```

#![allow(clippy::expect_used)]

use std::fs;
use std::path::{Path, PathBuf};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/misc")
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
fn bgm_detail_matches_the_frozen_snapshot() {
    check("bgm_detail.raw.json", "bgm_detail.rust.json", |raw| {
        value(&bili_api::music::parse_bgm_detail(raw).expect("样本应解析成功"))
    });
}

#[test]
fn bgm_recommend_matches_the_frozen_snapshot() {
    check("bgm_recommend.raw.json", "bgm_recommend.rust.json", |raw| {
        value(&bili_api::music::parse_bgm_recommend(raw).expect("样本应解析成功"))
    });
}

#[test]
fn black_list_matches_the_frozen_snapshot() {
    check("black_list.raw.json", "black_list.rust.json", |raw| {
        value(&bili_api::user_list::parse_black_list(raw).expect("样本应解析成功"))
    });
}

#[test]
fn match_info_matches_the_frozen_snapshot() {
    check("match_info.raw.json", "match_info.rust.json", |raw| {
        value(&bili_api::match_info::parse_match_info(raw).expect("样本应解析成功"))
    });
}

#[test]
fn danmaku_filter_matches_the_frozen_snapshot() {
    check(
        "danmaku_filter.raw.json",
        "danmaku_filter.rust.json",
        |raw| value(&bili_api::danmaku_filter::parse_danmaku_filter(raw).expect("样本应解析成功")),
    );
}

/// 真实赛事响应里七十多个键，只有十一个被映射。固定这件事，免得有人看到
/// 快照「字段怎么这么少」而去补一堆没有消费者的字段。
#[test]
fn unmapped_keys_in_the_real_response_are_ignored() {
    let raw = fs::read_to_string(fixture("match_info.raw.json")).expect("可读");
    let parsed: serde_json::Value = serde_json::from_str(&raw).expect("合法 JSON");
    let contest = &parsed["data"]["contest"];
    for ignored in ["etime", "game_state", "status", "live_status"] {
        assert!(
            !contest[ignored].is_null(),
            "fixture 里要留着 {ignored} 这类未映射的键"
        );
    }

    let got = bili_api::match_info::parse_match_info(&raw)
        .expect("应解析成功")
        .expect("有比赛");
    assert_eq!(got.contest_status, Some(3));
    assert_eq!(got.game_stage.as_deref(), Some("夏季赛第一周"));
}
