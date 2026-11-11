//! PGC golden 快照（S10 第一批 / issue #60）。
//!
//! 对照机制见 ADR-011 决策五与 S9 的补充：本测试把 Rust 的解析结果冻结成
//! `*.rust.json`，Dart 侧 `test/services/rust/pgc_parity_test.dart` 用迁移前
//! 的基线解析同一份 `*.raw.json` 与之逐字段比对。单看任何一侧都只是
//! 「自己跟自己一致」。
//!
//! 重新生成快照：
//! ```sh
//! UPDATE_GOLDEN=1 cargo test -p bili-api --test pgc_golden
//! ```
//! 生成后**必须**再跑一遍 Dart 侧的 parity 测试。
//!
//! fixture 取自真实响应的形状，mid、昵称、稿件 id、CDN 地址一律替换。
//! 保留的是**结构**：缺字段的条目、空列表的那一天、没有 vip 的作者、
//! 两种排行各自的载荷键。

// 测试里 expect 是对的：fixture 缺失或损坏时就该崩掉并指出原因。
#![allow(clippy::expect_used)]

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/pgc")
        .join(name)
}

fn raw(name: &str) -> String {
    fs::read_to_string(fixture(name)).unwrap_or_else(|e| panic!("{name} 可读: {e}"))
}

fn assert_snapshot(stem: &str, got: &Value) {
    let path = fixture(&format!("{stem}.rust.json"));

    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        let pretty = serde_json::to_string_pretty(got).expect("可序列化");
        fs::write(&path, format!("{pretty}\n")).expect("快照可写");
        return;
    }

    let expected: Value = serde_json::from_str(
        &fs::read_to_string(&path)
            .unwrap_or_else(|_| panic!("缺少 {stem}.rust.json，用 UPDATE_GOLDEN=1 重新生成")),
    )
    .expect("快照必须是合法 JSON");

    assert_eq!(got, &expected, "{stem} 的解析结果与冻结快照不一致");
}

#[test]
fn index_matches_the_frozen_snapshot() {
    let page = bili_api::pgc::parse_index(&raw("index.raw.json")).expect("应解析成功");
    assert_snapshot("index", &serde_json::to_value(&page).expect("可序列化"));
}

#[test]
fn index_condition_matches_the_frozen_snapshot() {
    let c =
        bili_api::pgc::parse_index_condition(&raw("index_condition.raw.json")).expect("应解析成功");
    assert_snapshot(
        "index_condition",
        &serde_json::to_value(&c).expect("可序列化"),
    );
}

#[test]
fn timeline_matches_the_frozen_snapshot() {
    let days = bili_api::pgc::parse_timeline(&raw("timeline.raw.json")).expect("应解析成功");
    assert_snapshot("timeline", &serde_json::to_value(&days).expect("可序列化"));
}

#[test]
fn review_matches_the_frozen_snapshot() {
    let page = bili_api::pgc::parse_reviews(&raw("review.raw.json")).expect("应解析成功");
    assert_snapshot("review", &serde_json::to_value(&page).expect("可序列化"));
}

#[test]
fn season_status_matches_the_frozen_snapshot() {
    let s = bili_api::pgc::parse_season_status(&raw("season_status.raw.json")).expect("应解析成功");
    assert_snapshot(
        "season_status",
        &serde_json::to_value(&s).expect("可序列化"),
    );
}

#[test]
fn rank_web_matches_the_frozen_snapshot() {
    let items = bili_api::pgc::parse_rank(&raw("rank_web.raw.json"), false).expect("应解析成功");
    assert_snapshot("rank_web", &serde_json::to_value(&items).expect("可序列化"));
}

#[test]
fn rank_season_matches_the_frozen_snapshot() {
    let items = bili_api::pgc::parse_rank(&raw("rank_season.raw.json"), true).expect("应解析成功");
    assert_snapshot(
        "rank_season",
        &serde_json::to_value(&items).expect("可序列化"),
    );
}

// --------------------------------------------------- 快照之外要钉住的行为

#[test]
fn the_two_rank_variants_are_not_interchangeable() {
    // web 排行的载荷在 result、season 排行在 data。读错键不会解出空列表
    // 而是报错——这正是要的：静默变空才是最难查的那种。
    assert!(
        bili_api::pgc::parse_rank(&raw("rank_web.raw.json"), true).is_err(),
        "web 样本按 season 口径读应当失败"
    );
    assert!(
        bili_api::pgc::parse_rank(&raw("rank_season.raw.json"), false).is_err(),
        "season 样本按 web 口径读应当失败"
    );
}

#[test]
fn score_ten_and_nine_differ_by_integer_division() {
    // 基线 `~/ 2`：10 → 5，9 → 4。改成四舍五入会让一整批九分短评多出
    // 半颗星，而这种偏差没人会报 bug，只会慢慢被当成「本来就这样」。
    let page = bili_api::pgc::parse_reviews(&raw("review.raw.json")).expect("应解析成功");
    assert_eq!(page.list[0].score, 5);
    assert_eq!(page.list[1].score, 4);
}

#[test]
fn review_without_author_or_stat_survives() {
    // fixture 最后一条只有 review_id 与 content。整条丢掉或整页失败都不
    // 合适——它仍然能显示。
    let page = bili_api::pgc::parse_reviews(&raw("review.raw.json")).expect("应解析成功");
    let last = page.list.last().expect("样本有四条");
    assert_eq!(last.review_id, Some(500_000_204));
    assert!(last.author.is_none());
    assert!(last.stat.is_none());
    assert_eq!(last.score, 0);
}

#[test]
fn rank_item_without_stat_counts_as_zero() {
    // 基线 `(json['follow'] as int?) ?? 0`。给 null 会让界面的数字格式化
    // 函数拿到空值。
    let items = bili_api::pgc::parse_rank(&raw("rank_web.raw.json"), false).expect("应解析成功");
    let second = &items[1];
    assert_eq!(second.follow, 0);
    assert_eq!(second.view, 0);
    assert!(second.new_ep_index_show.is_none());
}

#[test]
fn a_day_with_no_episodes_is_kept() {
    // 时间表里没有新番的那天也要留着——界面按天排版，少一天会让日期错位。
    let days = bili_api::pgc::parse_timeline(&raw("timeline.raw.json")).expect("应解析成功");
    assert_eq!(days.len(), 3);
    assert!(days[2].episodes.is_empty());
    assert_eq!(days[2].date.as_deref(), Some("11-12"));
}
