//! 消息中心与私信设置的 golden 快照（S12 / issue #63）。
//!
//! 对照机制同 ADR-011 决策五：本测试冻结 Rust 的解析结果，
//! `test/services/rust/msg_parity_test.dart` 用迁移前那批 `fromJson` 的
//! 逐行转写解析同一份 raw，与快照逐字段比对。
//!
//! ## fixture 全部是手工构造的
//!
//! 这一片**一条匿名可取的接口都没有**：消息中心的三条 feed、系统通知、
//! 未读数、会话设置，全是账号自己的数据。而真实响应脱敏之后剩下的就是
//! 一串示例用户——把自己收到的赞和私信设置写进仓库，脱敏完也证明不了
//! 什么，还多一份要维护的隐私面。
//!
//! 所以这几份是**按基线模型的字段逐一写出来的**，并刻意带上三类东西：
//!
//! 1. **未映射的键**（`source_id`、`root_id`、`at_details`、`fans`、
//!    `due_date`、`chat`、`up`、`pn`/`ps` 等）：证明多余字段会被忽略，
//!    免得有人看到快照「字段怎么这么少」去补一堆没有消费者的字段；
//! 2. **缺字段的条目**（`at_feed` 第二条只有 mid、`like_detail` 第三条
//!    `user` 为 null）：证明缺失不会炸掉一整页；
//! 3. **`content` 的三种形态**（带 web 的 JSON、纯文本、只有 android 的
//!    JSON）：那是系统通知里唯一一处真正的解析分支。
//!
//! 它们证明的是**映射一致**，不能证明我们对协议的理解正确。真机确认按
//! #67 的约定留到 Phase 2。
//!
//! 重新生成：
//! ```sh
//! UPDATE_GOLDEN=1 cargo test -p bili-api --test msg_golden
//! ```

#![allow(clippy::expect_used)]

use std::fs;
use std::path::{Path, PathBuf};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/msg")
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
fn reply_feed_matches_the_frozen_snapshot() {
    check("reply_feed.raw.json", "reply_feed.rust.json", |raw| {
        value(&bili_api::msg::parse_reply_feed(raw).expect("样本应解析成功"))
    });
}

#[test]
fn at_feed_matches_the_frozen_snapshot() {
    check("at_feed.raw.json", "at_feed.rust.json", |raw| {
        value(&bili_api::msg::parse_at_feed(raw).expect("样本应解析成功"))
    });
}

#[test]
fn like_feed_matches_the_frozen_snapshot() {
    check("like_feed.raw.json", "like_feed.rust.json", |raw| {
        value(&bili_api::msg::parse_like_feed(raw).expect("样本应解析成功"))
    });
}

#[test]
fn like_detail_matches_the_frozen_snapshot() {
    check("like_detail.raw.json", "like_detail.rust.json", |raw| {
        value(&bili_api::msg::parse_like_detail(raw).expect("样本应解析成功"))
    });
}

#[test]
fn sys_notify_matches_the_frozen_snapshot() {
    check("sys_notify.raw.json", "sys_notify.rust.json", |raw| {
        value(&bili_api::msg::parse_sys_notify(raw).expect("样本应解析成功"))
    });
}

/// 快照里多一个 `badge`。
///
/// 它不是解析出来的字段，是红点的求和规则（基线写在
/// `lib/pages/main/controller.dart`，按 AGENTS.md §2 下沉到
/// [`bili_domain::msg::SingleUnread::badge_count`]）。放进快照是为了让
/// Dart 侧的 parity 能直接对着基线那段加法比——否则那条规则在跨语言这
/// 一层就没有任何东西守着。
#[test]
fn single_unread_matches_the_frozen_snapshot() {
    check("single_unread.raw.json", "single_unread.rust.json", |raw| {
        let u = bili_api::msg::parse_single_unread(raw).expect("样本应解析成功");
        let mut v = value(&u);
        v["badge"] = serde_json::json!(u.badge_count());
        v
    });
}

/// 同上多一个 `badge`，按**四项全勾**算。
///
/// 选择集是用户设置，不属于快照；固定成全勾是因为那一档才覆盖四个加项，
/// 少勾一项等于少验一个分支。勾选对结果的影响由
/// `bili_domain::msg` 的单测守着。
#[test]
fn feed_unread_matches_the_frozen_snapshot() {
    check("feed_unread.raw.json", "feed_unread.rust.json", |raw| {
        let u = bili_api::msg::parse_feed_unread(raw).expect("样本应解析成功");
        let mut v = value(&u);
        v["badge"] = serde_json::json!(u.badge_count(bili_domain::msg::FeedBadgeSelection {
            reply: true,
            at: true,
            like: true,
            sys_msg: true,
        }));
        v
    });
}

#[test]
fn im_user_infos_matches_the_frozen_snapshot() {
    check("im_user_infos.raw.json", "im_user_infos.rust.json", |raw| {
        value(&bili_api::msg::parse_im_user_infos(raw).expect("样本应解析成功"))
    });
}

#[test]
fn session_ss_matches_the_frozen_snapshot() {
    check("session_ss.raw.json", "session_ss.rust.json", |raw| {
        value(&bili_api::msg::parse_session_ss(raw).expect("样本应解析成功"))
    });
}

#[test]
fn msg_dnd_matches_the_frozen_snapshot() {
    check("msg_dnd.raw.json", "msg_dnd.rust.json", |raw| {
        value(&bili_api::msg::parse_msg_dnd(raw).expect("样本应解析成功"))
    });
}

/// fixture 里要一直留着这些没有消费者的键。
///
/// 它们证明的是「接口多给了字段，解析不受影响」。有人把它们删掉之后，
/// 这条性质就没有任何东西守着了——而协议加字段是常态。
#[test]
fn unmapped_keys_stay_in_the_fixtures() {
    let raw = fs::read_to_string(fixture("reply_feed.raw.json")).expect("可读");
    let parsed: serde_json::Value = serde_json::from_str(&raw).expect("合法 JSON");
    let item = &parsed["data"]["items"][0]["item"];
    for ignored in [
        "source_id",
        "target_id",
        "root_id",
        "type",
        "uri",
        "at_details",
    ] {
        assert!(
            parsed["data"]["items"][0]["item"].get(ignored).is_some(),
            "fixture 里要留着 {ignored} 这类未映射的键"
        );
    }
    assert!(item["title"].is_string(), "title 同样是未映射的键");

    let raw = fs::read_to_string(fixture("feed_unread.raw.json")).expect("可读");
    let parsed: serde_json::Value = serde_json::from_str(&raw).expect("合法 JSON");
    for ignored in ["chat", "up"] {
        assert!(
            parsed["data"].get(ignored).is_some(),
            "未读数里 {ignored} 这两个桶没有消费者，留着证明会被忽略"
        );
    }
}

/// 系统通知的 `content` 有三种形态，三条都要在 fixture 里。
///
/// 这是这一片唯一一处真正的解析分支（取 `web` / 原样保留），没有它
/// golden 只是在冻结一堆字段搬运。
#[test]
fn sys_notify_covers_all_three_content_shapes() {
    let raw = fs::read_to_string(fixture("sys_notify.raw.json")).expect("可读");
    let items = bili_api::msg::parse_sys_notify(&raw).expect("应解析成功");
    assert_eq!(items.len(), 3);
    assert_eq!(
        items[0].content.as_deref(),
        Some("您的稿件《示例标题》已通过审核"),
        "有 web 字段时取 web"
    );
    assert_eq!(
        items[1].content.as_deref(),
        Some("这是一条纯文本的系统通知"),
        "纯文本原样保留"
    );
    assert_eq!(
        items[2].content.as_deref(),
        Some(r#"{"android":"安卓专属文案"}"#),
        "能解成 JSON 但没有 web 时，同样原样保留——基线的 catch 也是这个效果"
    );
}

/// 缺字段的条目掉一条，不掉一整页（ADR-011 决策三）。
#[test]
fn incomplete_entries_do_not_fail_the_page() {
    let raw = fs::read_to_string(fixture("at_feed.raw.json")).expect("可读");
    let feed = bili_api::msg::parse_at_feed(&raw).expect("应解析成功");
    assert_eq!(feed.items.len(), 2, "只有 mid 的那条也要在");
    let sparse = &feed.items[1];
    assert_eq!(sparse.user.as_ref().expect("有 user").mid, Some(100_004));
    assert_eq!(sparse.user.as_ref().expect("有 user").nickname, None);

    let raw = fs::read_to_string(fixture("like_detail.raw.json")).expect("可读");
    let detail = bili_api::msg::parse_like_detail(&raw).expect("应解析成功");
    assert_eq!(detail.items.len(), 3);
    assert!(
        detail.items[2].user.is_none(),
        "user 为 null 的那条仍在列表里"
    );
}
