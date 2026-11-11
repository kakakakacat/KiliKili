//! 搜索 golden 快照（S9 / issue #59）。
//!
//! # 对照机制
//!
//! 按 ADR-011 决策五，typed 直连的对照分两半：
//!
//! 1. **本测试**把 Rust 的解析结果快照成 `*.rust.json`；
//! 2. **`test/services/rust/search_parity_test.dart`** 用**还活着的** Dart
//!    基线模型解析同一份 `*.raw.json`，与那份快照逐字段比对。
//!
//! 单看任何一侧都只是「自己跟自己一致」，合起来才构成对照。基线模型随本
//! PR 删除后，parity 测试也一并删掉，留在仓库里的是**快照**——它是迁移
//! 当时行为的冻结证据，日后 Rust 侧改动导致输出变化时快照会先红。
//!
//! 重新生成快照：
//! ```sh
//! UPDATE_GOLDEN=1 cargo test -p bili-api --test search_golden
//! ```
//! 生成后**必须**再跑一遍 Dart 侧的 parity 测试，否则等于把 Rust 的改动
//! 直接盖进「期望值」。
//!
//! # fixture 的脱敏
//!
//! 样本取自真实响应的形状，但 mid、昵称、稿件 id、CDN 地址一律替换成
//! `example-cdn.test` 与示例编号（issue #59 的交付物要求）。保留的是
//! **结构**：富文本标记、字符串形态的统计字段、缺字段的条目、未知分区块。

// 测试里 expect 是对的：fixture 缺失或损坏时就该崩掉并指出原因。
#![allow(clippy::expect_used)]

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/search")
        .join(name)
}

fn raw(name: &str) -> String {
    fs::read_to_string(fixture(name)).unwrap_or_else(|e| panic!("{name} 可读: {e}"))
}

/// 与冻结快照比对；`UPDATE_GOLDEN=1` 时改为写入。
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
fn video_search_matches_the_frozen_snapshot() {
    let parsed = bili_api::search::parse_video(&raw("search_video.raw.json")).expect("应解析成功");
    let got = serde_json::json!({
        "num_results": parsed.num_results,
        "items": parsed.items,
    });
    assert_snapshot("search_video", &got);
}

#[test]
fn combined_search_matches_the_frozen_snapshot() {
    let parsed = bili_api::search::parse_all(&raw("search_all.raw.json")).expect("应解析成功");
    assert_snapshot("search_all", &serde_json::to_value(&parsed).expect("可序列化"));
}

#[test]
fn user_search_matches_the_frozen_snapshot() {
    let parsed = bili_api::search::parse_user(&raw("search_user.raw.json")).expect("应解析成功");
    let got = serde_json::json!({
        "num_results": parsed.num_results,
        "items": parsed.items,
    });
    assert_snapshot("search_user", &got);
}

#[test]
fn live_search_matches_the_frozen_snapshot() {
    let parsed = bili_api::search::parse_live(&raw("search_live.raw.json")).expect("应解析成功");
    let got = serde_json::json!({
        "num_results": parsed.num_results,
        "items": parsed.items,
    });
    assert_snapshot("search_live", &got);
}

#[test]
fn article_search_matches_the_frozen_snapshot() {
    let parsed =
        bili_api::search::parse_article(&raw("search_article.raw.json")).expect("应解析成功");
    let got = serde_json::json!({
        "num_results": parsed.num_results,
        "items": parsed.items,
    });
    assert_snapshot("search_article", &got);
}

#[test]
fn trending_matches_the_frozen_snapshot() {
    // needs_top=true：热搜榜页用的是这一支，置顶项要拼进列表。
    let parsed =
        bili_api::search::parse_trending(&raw("search_trending.raw.json"), true).expect("应解析成功");
    assert_snapshot(
        "search_trending",
        &serde_json::to_value(&parsed).expect("可序列化"),
    );
}

// --------------------------------------------------- 快照之外要钉住的行为

#[test]
fn string_shaped_stats_are_read_as_numbers() {
    // fixture 第二条的 play/review/danmaku/favorite/like 全是字符串——接口
    // 确实会这样给，基线为此有个 safeToInt。收紧成只认数字的话，这一条的
    // 播放量会变成空。
    let parsed = bili_api::search::parse_video(&raw("search_video.raw.json")).expect("应解析成功");
    let item = &parsed.items[1];
    assert_eq!(item.stat.view, Some(54321));
    assert_eq!(item.stat.danmaku, Some(12));
    assert_eq!(item.stat.like, Some(56));
}

#[test]
fn unparsable_duration_does_not_drop_the_item() {
    // fixture 最后一条的 duration 是「直播中」。基线在这里 int.parse 抛异常，
    // 整次搜索被判失败；这里退化成 0 秒，其余字段照常可用。
    let parsed = bili_api::search::parse_video(&raw("search_video.raw.json")).expect("应解析成功");
    let item = parsed.items.last().expect("样本有四条");
    assert_eq!(item.duration_secs, 0);
    assert_eq!(item.bvid.as_deref(), Some("BV1cc4y1c0c3"));
}

#[test]
fn unknown_result_block_does_not_fail_the_page() {
    // fixture 里有一块 `brand_new_block_from_the_future`。服务端加新卡片
    // 类型时，老客户端应当少显示一块，而不是整页搜不出来。
    let all = bili_api::search::parse_all(&raw("search_all.raw.json")).expect("应解析成功");
    assert_eq!(all.videos.len(), 1);
    assert_eq!(all.users.len(), 1);
    // media_bangumi 与 media_ft 合并进同一个列表，与基线一致。
    assert_eq!(all.medias.len(), 2);
}

#[test]
fn trending_entry_without_keyword_is_dropped() {
    // fixture 第三条只有 show_name。基线此处 `as String` 会抛异常，整个
    // 热搜榜拿不到。
    let t = bili_api::search::parse_trending(&raw("search_trending.raw.json"), true)
        .expect("应解析成功");
    assert_eq!(t.top_count, 1);
    // 置顶 1 条 + list 里合法的 2 条，缺 keyword 的那条被丢掉。
    assert_eq!(t.list.len(), 3);
    assert!(t.list.iter().all(|i| !i.keyword.is_empty()));
}
