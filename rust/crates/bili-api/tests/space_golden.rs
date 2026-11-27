//! 用户空间那一族的 golden 快照（S6 / issue #56）。
//!
//! 对照机制同 ADR-011 决策五：本测试冻结 Rust 的解析结果。
//! 迁移时已用不 import lib/ 的一次性 Dart parity 逐字段比对同一份 raw；
//! 对照通过后按 AGENTS.md 与被替换模型一起删除，长期保留本测试与快照。
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

/// 固定时间锁住首屏签名，游标、排序与时间变化都必须重新签名（#260）。
#[test]
fn the_archive_cursor_request_requires_app_signature() {
    use bili_api::space::{SpaceArchiveKind, SpaceArchiveOrder, space_archive_request};
    let request = |next, order, ts| {
        space_archive_request(
            SpaceArchiveKind::Video,
            Some(1),
            None,
            Some(order),
            None,
            None,
            Some(next),
            None,
            None,
            Some(true),
            "dfca71928277209b",
            "b5475a8825547a4fc26c7d518eaaa02e",
            ts,
        )
    };
    let req = request(0, SpaceArchiveOrder::Pubdate, 1_700_000_000);
    assert_eq!(req.path, "/x/v2/space/archive/cursor");
    let query: std::collections::BTreeMap<_, _> = req.query.iter().cloned().collect();
    assert_eq!(query["appkey"], "dfca71928277209b");
    assert_eq!(query["ts"], "1700000000");
    assert_eq!(query["next"], "0");
    assert_eq!(query["order"], "pubdate");
    assert_eq!(query["sign"], "07eacd484b4d2768c95d6259e8c9a6e6");
    for changed in [
        request(123, SpaceArchiveOrder::Pubdate, 1_700_000_000),
        request(0, SpaceArchiveOrder::Click, 1_700_000_000),
        request(0, SpaceArchiveOrder::Pubdate, 1_700_000_001),
    ] {
        assert_ne!(changed.query.last(), req.query.last());
    }
}

/// 本次只确认游标投稿接口必须签名，其余列表不扩展请求面。
#[test]
fn the_other_archive_requests_keep_their_unsigned_shape() {
    use bili_api::space::{SpaceArchiveKind, space_archive_request};
    for kind in [
        SpaceArchiveKind::Charging,
        SpaceArchiveKind::Season,
        SpaceArchiveKind::Series,
        SpaceArchiveKind::Bangumi,
        SpaceArchiveKind::Comic,
    ] {
        let req = space_archive_request(
            kind,
            Some(1),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            "dfca71928277209b",
            "b5475a8825547a4fc26c7d518eaaa02e",
            1_700_000_000,
        );
        assert!(
            !req.query
                .iter()
                .any(|(key, _)| matches!(key.as_str(), "appkey" | "ts" | "sign"))
        );
        assert!(req.query_string().contains("qn=32"));
    }
}

/// 六个 tab 共用请求构造，视频与漫画仍保留各自的路径和封面档位。
#[test]
fn the_contribute_tabs_keep_their_paths_and_quality() {
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
        "dfca71928277209b",
        "b5475a8825547a4fc26c7d518eaaa02e",
        1_700_000_000,
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
        "dfca71928277209b",
        "b5475a8825547a4fc26c7d518eaaa02e",
        1_700_000_000,
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
        "dfca71928277209b",
        "b5475a8825547a4fc26c7d518eaaa02e",
        1_700_000_000,
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
        "dfca71928277209b",
        "b5475a8825547a4fc26c7d518eaaa02e",
        1_700_000_000,
    )
    .query_string();
    assert!(full.contains("aid=12345"));
    assert!(full.contains("order=click"));
    assert!(full.contains("include_cursor=true"));
}

/// 固定时间的签名覆盖全部参数，来源稿件的变化也必须改变签名（#260）。
#[test]
fn the_space_request_is_app_signed() {
    use bili_api::space::space_request;
    const KEY: &str = "dfca71928277209b";
    const SEC: &str = "b5475a8825547a4fc26c7d518eaaa02e";
    let req = space_request(1, Some("123"), KEY, SEC, 1_700_000_000);
    assert_eq!(req.path, "/x/v2/space");
    let query: std::collections::BTreeMap<_, _> = req.query.iter().cloned().collect();
    assert_eq!(query["appkey"], KEY);
    assert_eq!(query["ts"], "1700000000");
    assert_eq!(query["vmid"], "1");
    assert_eq!(query["from_view_aid"], "123");
    assert_eq!(query["sign"], "020e132fa15b1c4ea87f50034a11216a");
    assert_eq!(query.len(), 13);

    let bare = space_request(1, None, KEY, SEC, 1_700_000_000);
    assert!(!bare.query_string().contains("from_view_aid"));
    assert_ne!(bare.query.last(), req.query.last());
    assert_ne!(
        space_request(1, Some("123"), KEY, SEC, 1_700_000_001)
            .query
            .last(),
        req.query.last(),
    );
}

// ------------------------------------------- 网页端的投稿搜索与合集
//
// `space_season_web` / `space_series_web` 取自真实响应再脱敏。
// `space_archive_search` 是**手工构造**的：那条接口的匿名请求一律
// `-352 风控校验失败`，要浏览器指纹才过得去，抓不到。它按基线
// `VListItemModel.fromJson` 读的每个键逐一写成，只证明映射一致，不证明
// 我们对协议的理解正确；真机确认按 #67 留到 Phase 2。

#[test]
fn the_archive_search_matches_the_frozen_snapshot() {
    check(
        "space_archive_search.raw.json",
        "space_archive_search.rust.json",
        |raw| value(&bili_api::space::parse_space_archive_search(raw).expect("样本应解析成功")),
    );
}

#[test]
fn the_season_web_page_matches_the_frozen_snapshot() {
    check(
        "space_season_web.raw.json",
        "space_season_web.rust.json",
        |raw| value(&bili_api::space::parse_space_season_web(raw).expect("样本应解析成功")),
    );
}

#[test]
fn the_series_web_page_matches_the_frozen_snapshot() {
    check(
        "space_series_web.raw.json",
        "space_series_web.rust.json",
        |raw| value(&bili_api::space::parse_space_season_web(raw).expect("样本应解析成功")),
    );
}

/// 标签来自两个地方：`slist`（专题）与 `tlist`（分区）。分区里只认课堂。
#[test]
fn only_the_lesson_category_joins_the_tag_row() {
    let raw =
        fs::read_to_string(fixture("space_archive_search.raw.json")).expect("raw fixture 可读");
    let page = bili_api::space::parse_space_archive_search(&raw).expect("样本应解析成功");
    assert_eq!(page.tags.len(), 2, "一条专题 + 一条课堂分区");
    assert_eq!(page.tags[0].special_type.as_deref(), Some("charging"));
    assert_eq!(page.tags[1].tid, Some(196));
    assert!(
        page.tags.iter().all(|t| t.tid != Some(188)),
        "科技分区不该进标签条"
    );
}

/// 看完了记成 -1，没看完按百分比折算；时长来自那个 `"24:52"` 字符串。
#[test]
fn the_playback_position_becomes_seconds_or_minus_one() {
    let raw =
        fs::read_to_string(fixture("space_archive_search.raw.json")).expect("raw fixture 可读");
    let page = bili_api::space::parse_space_archive_search(&raw).expect("样本应解析成功");
    // 24:52 = 1492 秒，50% → 746。
    assert_eq!(page.items[0].duration, Some(1492));
    assert_eq!(page.items[0].progress, Some(746));
    // 100% → -1，界面据此写「已看完」。
    assert_eq!(page.items[1].progress, Some(-1));
    // 服务端没给 playback_position 时整条不存在。
    assert_eq!(page.items[2].progress, None);
    // 1:02:03 = 3723 秒。
    assert_eq!(page.items[2].duration, Some(3723));
}

/// 三种角标互斥，顺序照抄基线：课堂 > 充电专属 > 合作。
#[test]
fn the_three_badges_are_mutually_exclusive_in_the_baseline_order() {
    use bili_domain::space::SpaceWebBadge;

    let raw =
        fs::read_to_string(fixture("space_archive_search.raw.json")).expect("raw fixture 可读");
    let page = bili_api::space::parse_space_archive_search(&raw).expect("样本应解析成功");
    // 第一条同时是 is_union_video，但没有前两个标志，所以是「合作」。
    assert_eq!(page.items[0].badge, Some(SpaceWebBadge::Union));
    // 第二条是课堂。
    assert_eq!(page.items[1].badge, Some(SpaceWebBadge::Lesson));
    // 第三条同时有充电专属与合作，充电专属优先。
    assert_eq!(page.items[2].badge, Some(SpaceWebBadge::Charging));
}

/// 合集与列表是两条接口，参数名都不一样，不合并成一份。
#[test]
fn seasons_and_series_send_different_param_names() {
    use bili_api::space::{SpaceSeasonWebKind, space_season_web_request};

    let season = space_season_web_request(SpaceSeasonWebKind::Season, 1, 2, 1, 30, false);
    let series = space_season_web_request(SpaceSeasonWebKind::Series, 1, 2, 1, 30, false);
    assert_ne!(season.path, series.path);

    let season = season.query_string();
    assert!(season.contains("season_id=2"), "{season}");
    assert!(season.contains("page_num=1"), "{season}");
    assert!(season.contains("sort_reverse=false"), "{season}");

    let series = series.query_string();
    assert!(series.contains("series_id=2"), "{series}");
    assert!(series.contains("pn=1"), "{series}");
    assert!(series.contains("sort=desc"), "{series}");
}

// ----------------------------------------------------- gRPC 投稿搜索

/// gRPC 投稿搜索的解析结果。
///
/// 字节由基线的编码器写出（`tool/fixture-gen/space_search_archive_fixture.dart`），
/// 所以这条 golden 证明的是「两端对同一串字节的理解一致」。
#[test]
fn the_grpc_search_archive_matches_the_frozen_snapshot() {
    let raw = fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/space_grpc/search_archive.raw.pb"),
    )
    .expect("raw fixture 可读");
    let got = value(&bili_api::grpc_space::parse_search_archive(&raw).expect("样本应解析成功"));

    let snapshot_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/space_grpc/search_archive.rust.json");
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        let pretty = serde_json::to_string_pretty(&got).expect("可序列化");
        fs::write(&snapshot_path, format!("{pretty}\n")).expect("快照可写");
        return;
    }
    let expected: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(&snapshot_path).expect("缺少快照，用 UPDATE_GOLDEN=1 重新生成"),
    )
    .expect("快照必须是合法 JSON");
    assert_eq!(got, expected, "gRPC 投稿搜索的解析结果与冻结快照不一致");
}

/// 竖屏判定：转过一次的稿件宽高要反过来看。
#[test]
fn a_rotated_archive_counts_as_vertical() {
    let raw = fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/space_grpc/search_archive.raw.pb"),
    )
    .expect("raw fixture 可读");
    let page = bili_api::grpc_space::parse_search_archive(&raw).expect("样本应解析成功");
    assert!(!page.items[0].is_vertical, "1920x1080 不转 → 横屏");
    assert!(page.items[1].is_vertical, "1080x1920 → 竖屏");
    assert!(page.items[2].is_vertical, "1920x1080 转过一次 → 竖屏");
}

/// 标题的 `<em>` 高亮在 Rust 拆好，实体也解掉，Dart 不再跑一遍正则。
#[test]
fn the_grpc_title_is_split_and_decoded_in_rust() {
    let raw = fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/space_grpc/search_archive.raw.pb"),
    )
    .expect("raw fixture 可读");
    let page = bili_api::grpc_space::parse_search_archive(&raw).expect("样本应解析成功");
    let spans = &page.items[0].title_spans;
    assert!(spans.iter().any(|s| s.is_em && s.text == "1 万次"));
    assert!(
        spans.iter().any(|s| !s.is_em && s.text.contains('&')),
        "`&amp;` 要解成 `&`：{spans:?}"
    );
}
