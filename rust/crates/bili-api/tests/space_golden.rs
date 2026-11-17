//! 用户空间那一族的 golden 快照（S6 / issue #56）。
//!
//! 对照机制同 ADR-011 决策五：本测试冻结 Rust 的解析结果，
//! `test/services/rust/space_parity_test.dart` 用迁移前那几个 `fromJson`
//! 的逐行转写解析同一份 raw，与快照逐字段比对。
//!
//! ## fixture 全部是手工构造的
//!
//! 「他有没有被我关注」「他的播放总数」这些都随登录账号而变，抓一份下来
//! 既不能复现也不能脱敏。与其如此，不如直接写出要验的分支：
//!
//! - `member_info` 里**刻意留着七个没有消费者的键**（`sex`、`sign`、
//!   `top_photo`、`official`、`live_room`、`rank`、`fans_badge`），用来证明
//!   它们会被忽略——同 `user_golden.rs` 里 nav 的做法；
//! - `member_info_alt_vip_keys` 单独一份：大会员信息有 `type`/`status` 与
//!   `vipType`/`vipStatus` 两套键名，基线的 `Vip.fromJson` 两套都认，一份
//!   fixture 兼顾不了；
//! - `member_relation_stat` 的 `following` 是 **-1**：对方设了隐私时服务端
//!   就给这个值，界面据此显示「已隐藏」，归一化成 0 会把这个区别抹掉；
//! - `member_card` 的认证信息键名是**大写开头**的 `Official`，`mid` 是
//!   **字符串**，两处都是服务端的事实；
//! - `member_card_no_card` 整个 `card` 都没有，证明缺它不是错误；
//! - `season_series` 的合集与列表**各一条**，证明两个数组分开解析，以及
//!   `data.items_lists` 这层嵌套；`season_series_empty` 整块缺失；
//! - `coin_like_arc` 的 `param` 是**字符串**，第二条是番剧（`is_pgc`）且
//!   少若干字段；
//! - `member_guard` 的第三项**只有 uid**：基线把这四个字段声明成非空，
//!   服务端少给一个就整页抛，我们不复刻那个崩法；
//! - `upower_rank` 带着 `level_info` 与 `tabs`（指定档位时服务端不下发）。
//!
//! 它们按基线模型的字段逐一写成，证明的是**映射一致**，不能证明我们对协议
//! 的理解正确。真机确认按 #67 的约定留到 Phase 2。
//!
//! 重新生成：
//! ```sh
//! UPDATE_GOLDEN=1 cargo test -p bili-api --test space_golden
//! ```

#![allow(clippy::expect_used)]

use std::fs;
use std::path::{Path, PathBuf};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/space")
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
fn member_info_matches_the_frozen_snapshot() {
    check("member_info.raw.json", "member_info.rust.json", |raw| {
        value(&bili_api::space::parse_member_info(raw).expect("样本应解析成功"))
    });
}

#[test]
fn alternate_vip_keys_match_the_frozen_snapshot() {
    check(
        "member_info_alt_vip_keys.raw.json",
        "member_info_alt_vip_keys.rust.json",
        |raw| value(&bili_api::space::parse_member_info(raw).expect("样本应解析成功")),
    );
}

#[test]
fn member_relation_stat_matches_the_frozen_snapshot() {
    check(
        "member_relation_stat.raw.json",
        "member_relation_stat.rust.json",
        |raw| value(&bili_api::space::parse_member_relation_stat(raw).expect("样本应解析成功")),
    );
}

#[test]
fn member_up_stat_matches_the_frozen_snapshot() {
    check(
        "member_up_stat.raw.json",
        "member_up_stat.rust.json",
        |raw| value(&bili_api::space::parse_member_up_stat(raw).expect("样本应解析成功")),
    );
}

#[test]
fn member_card_matches_the_frozen_snapshot() {
    check("member_card.raw.json", "member_card.rust.json", |raw| {
        value(&bili_api::space::parse_member_card(raw).expect("样本应解析成功"))
    });
}

#[test]
fn a_missing_card_block_matches_the_frozen_snapshot() {
    check(
        "member_card_no_card.raw.json",
        "member_card_no_card.rust.json",
        |raw| value(&bili_api::space::parse_member_card(raw).expect("样本应解析成功")),
    );
}

/// 没有消费者的那几个键必须被忽略，而不是让整条解析失败。
#[test]
fn unmapped_keys_in_the_member_info_response_are_ignored() {
    let raw = fs::read_to_string(fixture("member_info.raw.json")).expect("raw fixture 可读");
    assert!(
        raw.contains("\"live_room\"") && raw.contains("\"top_photo\"") && raw.contains("\"sign\""),
        "fixture 必须留着这些无人消费的键，否则这条测试什么也没证明"
    );
    let info = bili_api::space::parse_member_info(&raw).expect("样本应解析成功");
    assert_eq!(info.mid, Some(300_000_101), "无关键名不该影响解析");
}

/// 隐私设置下的 -1 要原样留着。
#[test]
fn a_hidden_following_count_stays_minus_one() {
    let raw =
        fs::read_to_string(fixture("member_relation_stat.raw.json")).expect("raw fixture 可读");
    let stat = bili_api::space::parse_member_relation_stat(&raw).expect("样本应解析成功");
    assert_eq!(
        stat.following,
        Some(-1),
        "归一化成 0 会把「已隐藏」显示成「关注了 0 个人」"
    );
}

#[test]
fn season_series_matches_the_frozen_snapshot() {
    check("season_series.raw.json", "season_series.rust.json", |raw| {
        value(&bili_api::space::parse_season_series(raw).expect("样本应解析成功"))
    });
}

#[test]
fn an_empty_season_series_page_matches_the_frozen_snapshot() {
    check(
        "season_series_empty.raw.json",
        "season_series_empty.rust.json",
        |raw| value(&bili_api::space::parse_season_series(raw).expect("样本应解析成功")),
    );
}

/// 投币与点赞是同一个解析，只换路径。再放一份 `like_arc.raw.json`
/// 只会得到字节相同的第二个文件。
#[test]
fn coin_like_arc_matches_the_frozen_snapshot() {
    check("coin_like_arc.raw.json", "coin_like_arc.rust.json", |raw| {
        value(&bili_api::space::parse_coin_like_arc(raw).expect("样本应解析成功"))
    });
}

#[test]
fn member_guard_matches_the_frozen_snapshot() {
    check("member_guard.raw.json", "member_guard.rust.json", |raw| {
        value(&bili_api::space::parse_member_guard(raw).expect("样本应解析成功"))
    });
}

#[test]
fn upower_rank_matches_the_frozen_snapshot() {
    check("upower_rank.raw.json", "upower_rank.rust.json", |raw| {
        value(&bili_api::space::parse_upower_rank(raw).expect("样本应解析成功"))
    });
}

/// 合集与列表必须分开：它们跳转到的是不同的页面，拼在一起就分不出来了。
#[test]
fn seasons_and_series_stay_in_separate_lists() {
    let raw = fs::read_to_string(fixture("season_series.raw.json")).expect("raw fixture 可读");
    let page = bili_api::space::parse_season_series(&raw).expect("样本应解析成功");
    assert_eq!(page.seasons.len(), 1);
    assert_eq!(page.series.len(), 1);
    assert_eq!(page.seasons[0].season_id, Some(70_011));
    assert_eq!(page.seasons[0].series_id, None, "合集没有 series_id");
    assert_eq!(page.series[0].series_id, Some(80_011));
    assert_eq!(page.series[0].season_id, None, "列表没有 season_id");
}

/// 名单里少字段的那一项不该让整页失败。
#[test]
fn a_guard_entry_with_only_a_uid_still_parses() {
    let raw = fs::read_to_string(fixture("member_guard.raw.json")).expect("raw fixture 可读");
    let page = bili_api::space::parse_member_guard(&raw).expect("样本应解析成功");
    assert_eq!(page.items.len(), 3);
    assert_eq!(page.items[2].uid, Some(300_000_203));
    assert_eq!(page.items[2].username, None, "基线这里会抛");
}
