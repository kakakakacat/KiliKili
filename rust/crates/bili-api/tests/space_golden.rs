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

// ---------------------------------------------------------------- 空间主页
//
// 下面这几份与上面不同：**取自真实响应再脱敏**（mid、昵称、CDN 文件名、
// 查询串都换过）。空间主页是全仓库最宽的一个响应，手工构造写不出那些
// 真实的怪形状——`images.collection_top_simple` 里静态封面走 `item.image`、
// 动态封面走 `item.animation`，两者只有一个有值，就是抓下来才发现的。
//
// fixture 里**刻意留着大量没有消费者的键**（`tab` 那 23 个布尔开关、
// `card` 的 `nameplate` / `honours`、`images` 的 `card_extra` / `extra`…），
// 由快照证明它们被忽略。
//
// `space_archive_extras` 与 `space_archive_empty` 是**手工构造**的：角标、
// 观看进度、合集更新时间、漫画题材这几支要特定的 UP 才出得来，抓不到。
// 它们只证明映射一致，不证明我们对协议的理解正确。

#[test]
fn the_space_page_matches_the_frozen_snapshot() {
    check("space_page.raw.json", "space_page.rust.json", |raw| {
        value(&bili_api::space::parse_space_page(raw).expect("样本应解析成功"))
    });
}

#[test]
fn the_alternate_space_page_matches_the_frozen_snapshot() {
    check(
        "space_page_alt.raw.json",
        "space_page_alt.rust.json",
        |raw| value(&bili_api::space::parse_space_page(raw).expect("样本应解析成功")),
    );
}

#[test]
fn the_reservations_match_the_frozen_snapshot() {
    check(
        "space_reservation.raw.json",
        "space_reservation.rust.json",
        |raw| value(&bili_api::space::parse_space_page(raw).expect("样本应解析成功")),
    );
}

#[test]
fn the_space_archive_matches_the_frozen_snapshot() {
    check("space_archive.raw.json", "space_archive.rust.json", |raw| {
        value(&bili_api::space::parse_space_archive(raw).expect("样本应解析成功"))
    });
}

#[test]
fn the_space_archive_extras_match_the_frozen_snapshot() {
    check(
        "space_archive_extras.raw.json",
        "space_archive_extras.rust.json",
        |raw| value(&bili_api::space::parse_space_archive(raw).expect("样本应解析成功")),
    );
}

#[test]
fn an_empty_space_archive_matches_the_frozen_snapshot() {
    check(
        "space_archive_empty.raw.json",
        "space_archive_empty.rust.json",
        |raw| value(&bili_api::space::parse_space_archive(raw).expect("样本应解析成功")),
    );
}

/// 置顶合集的封面有两种形态：静态的在 `item.image`、动态的在
/// `item.animation`，服务端只给其中一个。两种都要算出裁切位置。
#[test]
fn both_static_and_animated_collection_covers_yield_a_crop_offset() {
    let raw = fs::read_to_string(fixture("space_page.raw.json")).expect("raw fixture 可读");
    let page = bili_api::space::parse_space_page(&raw).expect("样本应解析成功");
    let images = page.images.expect("样本带 images");
    assert_eq!(images.top_images.len(), 2);
    // 动态封面：location = 0-29-1079-1079，height = 1863。
    let animated = &images.top_images[0];
    assert!(
        (animated.dy - ((29.0 + 1079.0) / 1863.0 - 1.0)).abs() < 1e-9,
        "动态封面也要算 dy，实际 {}",
        animated.dy
    );
    assert_eq!(
        animated.header, animated.full_cover,
        "没有 default_image 时回落到 cover"
    );
    // 静态封面：有 default_image，列表里显示它，点开看的还是 cover。
    let still = &images.top_images[1];
    assert_ne!(still.header, still.full_cover);
}

/// 空间页标签只认 `location` 与 `real_name` 两类。
#[test]
fn only_the_two_known_space_tags_survive() {
    let raw = r#"{"code":0,"data":{"card":{"space_tag":[
        {"type":"location","title":"IP属地：上海"},
        {"type":"school","title":"某某大学"},
        {"type":"real_name","title":"已实名"}
    ]}}}"#;
    let page = bili_api::space::parse_space_page(raw).expect("应解析成功");
    let tags = page.card.expect("有 card").tags;
    assert_eq!(tags.len(), 2);
    assert_eq!(tags[0].title.as_deref(), Some("IP属地：上海"));
    assert_eq!(tags[1].title.as_deref(), Some("已实名"));
}

/// `pr_info` 在没有推广时是个空对象，不该变成一个字段全空的结构。
#[test]
fn an_empty_pr_info_block_is_treated_as_absent() {
    let raw = r#"{"code":0,"data":{"card":{"pr_info":{}}}}"#;
    let page = bili_api::space::parse_space_page(raw).expect("应解析成功");
    assert!(page.card.expect("有 card").pr_info.is_none());
}

/// 推广信息的四个颜色有默认值，服务端不给时用基线那几个字面量。
#[test]
fn the_pr_info_colors_fall_back_to_the_baseline_literals() {
    let raw = r#"{"code":0,"data":{"card":{"pr_info":{"content":"广告"}}}}"#;
    let page = bili_api::space::parse_space_page(raw).expect("应解析成功");
    let pr = page.card.expect("有 card").pr_info.expect("有 pr_info");
    assert_eq!(pr.text_color, "#999999");
    assert_eq!(pr.bg_color, "#e7e7e7");
    assert_eq!(pr.text_color_night, "#727272");
    assert_eq!(pr.bg_color_night, "#2A2A2A");
}

/// 「主页」tab 要不要显示取决于八个块里还有没有东西，这个判断在 Rust 算。
#[test]
fn has_item_counts_all_eight_blocks() {
    let empty = r#"{"code":0,"data":{"archive":{"count":0,"item":[]},"article":{"count":0}}}"#;
    assert!(
        !bili_api::space::parse_space_page(empty)
            .expect("应解析成功")
            .has_item
    );

    // 只有收藏夹有东西，照样算「有」。
    let only_fav =
        r#"{"code":0,"data":{"archive":{"item":[]},"favourite2":{"item":[{"title":"甲"}]}}}"#;
    assert!(
        bili_api::space::parse_space_page(only_fav)
            .expect("应解析成功")
            .has_item
    );
}

/// 合集与列表任一非空，投稿页就要多一个「全部合集/列表」tab。
#[test]
fn has_season_or_series_follows_the_baseline_rule() {
    let none = r#"{"code":0,"data":{"ugc_season":{"count":0},"series":{"item":[]}}}"#;
    assert!(
        !bili_api::space::parse_space_page(none)
            .expect("应解析成功")
            .has_season_or_series
    );

    let by_series = r#"{"code":0,"data":{"ugc_season":{"count":0},"series":{"item":[{}]}}}"#;
    assert!(
        bili_api::space::parse_space_page(by_series)
            .expect("应解析成功")
            .has_season_or_series
    );

    let by_season = r#"{"code":0,"data":{"ugc_season":{"count":3}}}"#;
    assert!(
        bili_api::space::parse_space_page(by_season)
            .expect("应解析成功")
            .has_season_or_series
    );
}

/// 大航海人数要从「2人加入大航海」里抠出来。
#[test]
fn the_guard_count_is_dug_out_of_the_sentence() {
    let raw = r#"{"code":0,"data":{"guard":{"desc":"12人加入大航海","item":[]}}}"#;
    let guard = bili_api::space::parse_space_page(raw)
        .expect("应解析成功")
        .guard
        .expect("有 guard");
    assert_eq!(guard.count.as_deref(), Some("12"));

    let no_digits = r#"{"code":0,"data":{"guard":{"desc":"还没有人加入大航海"}}}"#;
    assert!(
        bili_api::space::parse_space_page(no_digits)
            .expect("应解析成功")
            .guard
            .expect("有 guard")
            .count
            .is_none()
    );
}

/// 六个 tab 共用一个请求构造函数，差别只在路径与封面档位。
#[test]
fn the_six_contribute_tabs_only_differ_in_path_and_quality() {
    use bili_api::space::{SpaceArchiveKind, space_archive_request};

    let video = space_archive_request(
        SpaceArchiveKind::Video,
        Some(1),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );
    let comic = space_archive_request(
        SpaceArchiveKind::Comic,
        Some(1),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );
    assert_ne!(video.path, comic.path);
    assert!(video.query_string().contains("qn=80"));
    assert!(comic.query_string().contains("qn=32"));
}

/// 可省的参数**省掉就不出现**，不是发一个空值。
#[test]
fn the_optional_archive_params_are_dropped_rather_than_sent_empty() {
    use bili_api::space::{SpaceArchiveKind, SpaceArchiveOrder, space_archive_request};

    let bare = space_archive_request(
        SpaceArchiveKind::Video,
        Some(1),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .query_string();
    for key in [
        "aid=",
        "pn=",
        "next=",
        "season_id=",
        "series_id=",
        "order=",
        "sort=",
        "include_cursor=",
    ] {
        assert!(!bare.contains(key), "{key} 不该出现 — {bare}");
    }

    let full = space_archive_request(
        SpaceArchiveKind::Video,
        Some(1),
        Some("12345"),
        Some(SpaceArchiveOrder::Click),
        None,
        Some(2),
        None,
        None,
        None,
        Some(true),
    )
    .query_string();
    assert!(full.contains("aid=12345"));
    assert!(full.contains("order=click"));
    assert!(full.contains("include_cursor=true"));
}

/// 空间主页的请求面照抄基线：同样的固定参数、同样的顺序。
#[test]
fn the_space_request_keeps_the_baseline_param_order() {
    use bili_api::space::space_request;

    let req = space_request(1, Some("123"));
    let keys: Vec<&str> = req.query.iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(
        keys,
        [
            "build",
            "version",
            "c_locale",
            "channel",
            "mobi_app",
            "platform",
            "s_locale",
            "from_view_aid",
            "statistics",
            "vmid",
        ]
    );

    // 没有 from_view_aid 时整条参数不出现，不是发一个空值。
    let bare = space_request(1, None).query_string();
    assert!(!bare.contains("from_view_aid"), "{bare}");
}
