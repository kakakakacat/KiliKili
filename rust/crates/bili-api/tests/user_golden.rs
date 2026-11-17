//! 「我的」那一族的 golden 快照（S7 / issue #57）。
//!
//! 对照机制同 ADR-011 决策五：本测试冻结 Rust 的解析结果。
//! 迁移时已用不 import lib/ 的一次性 Dart parity 逐字段比对同一份 raw；
//! 对照通过后按 AGENTS.md 与被替换模型一起删除，长期保留本测试与快照。
//!
//! ## fixture 全部是手工构造的
//!
//! 这一族**整条都是「我的」数据**（nav、硬币、流水、实名、隐私设置、
//! 订阅），匿名一条也取不到；脱敏之后剩下的也就是这几条示例。与其抓一份
//! 再逐条改名，不如直接写出要验的分支：
//!
//! - `user_info` 里**刻意留着十七个没有消费者的键**（`official`、
//!   `pendant`、`wallet`、`vip_label`…），用来证明它们会被忽略——
//!   同 `misc_golden.rs` 里 `match_info` 的做法；
//! - `user_info_max_level` 单独一份：满级时 `next_exp` 要取 `current_exp`，
//!   这是基线 `LevelInfo.fromJson` 里的规则，一份 fixture 兼顾不了两种；
//! - `coin_log` 的 `delta` 同时有整数、小数与负数：整数不能显示成 `2.0`；
//! - `space_setting` 的 `privacy` 里**故意少一个键**
//!   （`disable_show_school`），用来证明没下发的开关是 `null` 而不是 0；
//! - `sub_list` 的第二项几乎什么都没有，证明缺字段不是错误。
//!
//! 它们按基线模型的字段逐一写成，证明的是**映射一致**，不能证明我们对协议
//! 的理解正确。真机确认按 #67 的约定留到 Phase 2。
//!
//! 重新生成：
//! ```sh
//! UPDATE_GOLDEN=1 cargo test -p bili-api --test user_golden
//! ```

#![allow(clippy::expect_used)]

use std::fs;
use std::path::{Path, PathBuf};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/user")
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
fn user_info_matches_the_frozen_snapshot() {
    check("user_info.raw.json", "user_info.rust.json", |raw| {
        value(&bili_api::user::parse_user_info(raw).expect("样本应解析成功"))
    });
}

#[test]
fn max_level_user_info_matches_the_frozen_snapshot() {
    check(
        "user_info_max_level.raw.json",
        "user_info_max_level.rust.json",
        |raw| value(&bili_api::user::parse_user_info(raw).expect("样本应解析成功")),
    );
}

#[test]
fn user_stat_matches_the_frozen_snapshot() {
    check("user_stat.raw.json", "user_stat.rust.json", |raw| {
        value(&bili_api::user::parse_user_stat(raw).expect("样本应解析成功"))
    });
}

/// 硬币流水与经验流水是同一个解析，只换路径。再放一份 `exp_log.raw.json`
/// 只会得到字节相同的第二个文件。
#[test]
fn coin_log_matches_the_frozen_snapshot() {
    check("coin_log.raw.json", "coin_log.rust.json", |raw| {
        value(&bili_api::user::parse_coin_log(raw).expect("样本应解析成功"))
    });
}

#[test]
fn login_log_matches_the_frozen_snapshot() {
    check("login_log.raw.json", "login_log.rust.json", |raw| {
        value(&bili_api::user::parse_login_log(raw).expect("样本应解析成功"))
    });
}

#[test]
fn space_setting_matches_the_frozen_snapshot() {
    check("space_setting.raw.json", "space_setting.rust.json", |raw| {
        value(&bili_api::user::parse_space_setting(raw).expect("样本应解析成功"))
    });
}

#[test]
fn sub_list_matches_the_frozen_snapshot() {
    check("sub_list.raw.json", "sub_list.rust.json", |raw| {
        value(&bili_api::user::parse_sub_list(raw).expect("样本应解析成功"))
    });
}

#[test]
fn video_tags_matches_the_frozen_snapshot() {
    check("video_tags.raw.json", "video_tags.rust.json", |raw| {
        value(&bili_api::user::parse_video_tags(raw).expect("样本应解析成功"))
    });
}

#[test]
fn user_real_name_matches_the_frozen_snapshot() {
    check(
        "user_real_name.raw.json",
        "user_real_name.rust.json",
        |raw| value(&bili_api::user::parse_user_real_name(raw).expect("样本应解析成功")),
    );
}

/// 真实 nav 响应里 25 个键，只有九个被映射。固定这件事，免得有人看到快照
/// 「字段怎么这么少」而去补一堆没有消费者的字段。
#[test]
fn unmapped_keys_in_the_nav_response_are_ignored() {
    let raw = fs::read_to_string(fixture("user_info.raw.json")).expect("可读");
    let parsed: serde_json::Value = serde_json::from_str(&raw).expect("合法 JSON");
    let data = &parsed["data"];
    for ignored in [
        "official",
        "pendant",
        "wallet",
        "vip_label",
        "scores",
        "moral",
    ] {
        assert!(
            !data[ignored].is_null(),
            "fixture 里要留着 {ignored} 这类未映射的键"
        );
    }

    let info = bili_api::user::parse_user_info(&raw).expect("应解析成功");
    assert_eq!(info.is_login, Some(true));
    assert_eq!(info.money, Some(12.5));
}

/// 缓存往返：序列化之后再解回来，每个字段都在。
///
/// 这条不只是测 serde——缓存格式是**存盘格式**，`UserInfo` 加字段而忘了
/// 保留 `#[serde(default)]` 的话，老缓存会整份解不出来，表现是「升级之后
/// 每次启动头像都空一下」，很难归因。
#[test]
fn the_cache_round_trips() {
    let raw = fs::read_to_string(fixture("user_info.raw.json")).expect("可读");
    let info = bili_api::user::parse_user_info(&raw).expect("应解析成功");

    let json = bili_api::user::serialize_user_info_cache(&info).expect("可序列化");
    let back = bili_api::user::parse_user_info_cache(&json).expect("可解回");
    assert_eq!(back, info, "往返之后必须逐字段相等");

    assert!(
        bili_api::user::parse_user_info_cache("\u{1}\u{2}not json").is_none(),
        "老版本留下的 Hive 二进制帧要当作「没有缓存」，不是崩"
    );
}
