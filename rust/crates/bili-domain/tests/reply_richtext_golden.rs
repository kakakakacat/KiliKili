#![allow(clippy::expect_used)]

//! 正文分词的对照用例（`tests/fixtures/reply_richtext/cases.json`）。
//!
//! ## 期望值的来历
//!
//! 与 `reply_filter_golden` 同一条链：期望值是**人写的**，判据在 Dart
//! 那一侧——`test/services/rust/reply_richtext_parity_test.dart` 逐行转写
//! 了迁移前 `reply_item_grpc.dart` 的 `_buildMessage`，用真正的 Dart
//! `RegExp` 与 `splitMapJoin` 跑同一份文件。期望值与基线不符时它会红。
//!
//! 单看本测试只是「Rust 跟一份 Rust 也在用的文件自洽」，两侧合起来才构成
//! 对照。Dart 那一侧是**一次性**的（ADR-011 决策五）：行为在 Phase 2 真机
//! 回归里确认过之后可以删，本文件与 fixture 长期留下。
//!
//! ## 比较哪些字段
//!
//! `kind` / `text` / `action` / `target` / `mid` / `topic` / `seek`——判定
//! 的产物。表情图的 url、链接的小图标这些是从实体透传过来的，#176 就已经
//! 对照过，不在这里重复。

use std::fs;
use std::path::Path;

use bili_domain::reply_grpc::{ReplyEmote, ReplyNote, ReplyUrl, ReplyVote};
use bili_domain::reply_richtext::{
    ReplyRichTextInput, ReplyRichTextSettings, ReplySegment, ReplySegmentKind, ReplyTapAction,
    segment,
};
use serde_json::{Value, json};

fn fixture() -> Value {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/reply_richtext/cases.json");
    serde_json::from_str(&fs::read_to_string(path).expect("fixture 可读")).expect("fixture 是 JSON")
}

fn settings_from(value: &Value) -> ReplyRichTextSettings {
    ReplyRichTextSettings {
        enable_word_re: value["enable_word_re"].as_bool().unwrap_or(false),
        // 缺字段 = 拿不到播放器 = 时间戳一律不可点。
        video_duration_ms: value["video_duration_ms"].as_i64(),
    }
}

fn input_from(value: &Value) -> ReplyRichTextInput {
    ReplyRichTextInput {
        message: value["message"].as_str().unwrap_or_default().to_owned(),
        emotes: map_of(&value["emotes"], |v| ReplyEmote {
            size: v["size"].as_i64().unwrap_or_default(),
            display_url: v["display_url"].as_str().unwrap_or_default().to_owned(),
            jump_url: v["jump_url"].as_str().map(ToOwned::to_owned),
        }),
        topics: value["topics"]
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .filter_map(|t| t.as_str().map(ToOwned::to_owned))
                    .collect()
            })
            .unwrap_or_default(),
        at_name_to_mid: value["at_name_to_mid"]
            .as_object()
            .map(|map| {
                map.iter()
                    .filter_map(|(k, v)| v.as_i64().map(|mid| (k.clone(), mid)))
                    .collect()
            })
            .unwrap_or_default(),
        urls: map_of(&value["urls"], |v| ReplyUrl {
            title: v["title"].as_str().unwrap_or_default().to_owned(),
            prefix_icon: v["prefix_icon"].as_str().map(ToOwned::to_owned),
            app_url_schema: v["app_url_schema"].as_str().unwrap_or_default().to_owned(),
            is_word_search: v["is_word_search"].as_bool().unwrap_or(false),
            is_note: v["is_note"].as_bool().unwrap_or(false),
        }),
        vote: value["vote"].as_object().map(|v| ReplyVote {
            id: v["id"].as_i64().unwrap_or_default(),
            title: v["title"].as_str().unwrap_or_default().to_owned(),
        }),
        note: value["note"].as_object().map(|v| ReplyNote {
            click_url: v
                .get("click_url")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            opus_id: v.get("opus_id").and_then(Value::as_i64),
        }),
        is_note: value["is_note"].as_bool().unwrap_or(false),
        is_note_v2: value["is_note_v2"].as_bool().unwrap_or(false),
    }
}

fn map_of<T>(
    value: &Value,
    mut build: impl FnMut(&Value) -> T,
) -> std::collections::HashMap<String, T> {
    value
        .as_object()
        .map(|map| map.iter().map(|(k, v)| (k.clone(), build(v))).collect())
        .unwrap_or_default()
}

/// 把一段压成 fixture 里那几个字段，缺省的一律不写出来——这样期望值只写
/// 真正有判定的那几项，读起来就是「这一段是什么」。
fn projection(segment: &ReplySegment) -> Value {
    let mut out = json!({
        "kind": match segment.kind {
            ReplySegmentKind::Text => "text",
            ReplySegmentKind::Emote => "emote",
            ReplySegmentKind::Mention => "mention",
            ReplySegmentKind::Topic => "topic",
            ReplySegmentKind::Vote => "vote",
            ReplySegmentKind::Timestamp => "timestamp",
            ReplySegmentKind::Link => "link",
            ReplySegmentKind::Note => "note",
        },
        "text": segment.text,
    });
    #[allow(clippy::unwrap_used)]
    let map = out.as_object_mut().unwrap();
    if segment.action != ReplyTapAction::None {
        map.insert(
            "action".to_owned(),
            Value::from(match segment.action {
                ReplyTapAction::None => unreachable!(),
                ReplyTapAction::Webview => "webview",
                ReplyTapAction::Archive => "archive",
                ReplyTapAction::Article => "article",
                ReplyTapAction::Opus => "opus",
                ReplyTapAction::Scheme => "scheme",
                ReplyTapAction::Search => "search",
            }),
        );
        map.insert("target".to_owned(), Value::from(segment.target.clone()));
    }
    if let Some(mid) = segment.mid {
        map.insert("mid".to_owned(), Value::from(mid));
    }
    if let Some(topic) = segment.topic.as_ref() {
        map.insert("topic".to_owned(), Value::from(topic.clone()));
    }
    if let Some(seek) = segment.seek_seconds {
        map.insert("seek".to_owned(), Value::from(seek));
    }
    out
}

#[test]
fn segments_match_the_fixture() {
    let fixture = fixture();
    let cases = fixture["cases"].as_array().expect("cases 是数组");
    assert!(!cases.is_empty(), "用例为空则本测试形同虚设");

    for case in cases {
        let name = case["name"].as_str().unwrap_or("<无名用例>");
        let input = input_from(&case["content"]);
        let settings = settings_from(&case["settings"]);

        let actual: Vec<Value> = segment(&input, &settings).iter().map(projection).collect();
        let expected = case["segments"].as_array().expect("segments 是数组");

        assert_eq!(&actual, expected, "用例「{name}」");
    }
}

/// fixture 里标了 `baseline_order_dependent` 的用例在 Dart 侧会被跳过。
/// 这条测试守住「别把它们标得到处都是」：它们是**已知的行为变更**，不是
/// 对不上就加一个标记的后门。
#[test]
fn only_the_known_divergence_is_marked() {
    let fixture = fixture();
    let marked: Vec<&str> = fixture["cases"]
        .as_array()
        .expect("cases 是数组")
        .iter()
        .filter(|c| c["baseline_order_dependent"].as_bool().unwrap_or(false))
        .filter_map(|c| c["name"].as_str())
        .collect();

    assert_eq!(
        marked,
        vec!["互为前缀的两个 @ 名：长的优先"],
        "新增这个标记前先想清楚：基线在那一处真的是不确定的吗"
    );
}
