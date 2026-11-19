#![allow(clippy::expect_used)]

//! 评论过滤规则的对照用例（`tests/fixtures/reply_filter/cases.json`）。
//!
//! ## 期望值的来历
//!
//! 与 `bili-core` 的 `media_select_golden` 同一条链：期望值是**人写的**，
//! 判据在 Dart 那一侧——`test/services/rust/reply_filter_parity_test.dart`
//! 逐行转写了迁移前 `lib/grpc/reply.dart` 的 `ReplyGrpc.needRemoveGrpc`，
//! 用真正的 Dart `String` / `RegExp` 跑同一份文件。期望值与基线不符时
//! 它在 CI 里会红。
//!
//! 单看本测试只是「Rust 跟一份 Rust 也在用的文件自洽」，两侧合起来才构成
//! 对照。Dart 那一侧是**一次性**的（ADR-011 决策五）：行为在 Phase 2 真机
//! 回归里确认过之后可以删，本文件与 fixture 长期留下。
//!
//! 为什么不是「取一份 raw 响应、基线解析、冻结快照」那条链：这条路径的
//! 输入不是报文而是一组字段，而真实响应里恰恰难得见到边界形状（光一个
//! `@`、抬头缺冒号、emoji 昵称）。手写用例能把它们钉住。

use std::fs;
use std::path::Path;

use bili_domain::error::ErrorKind;
use bili_domain::reply_filter::{ReplyFilter, ReplyFilterItem, ReplyFilterSettings, ReplyUrlExtra};
use serde_json::Value;

fn fixture() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/reply_filter/cases.json");
    serde_json::from_str(&fs::read_to_string(path).expect("fixture 可读")).expect("fixture 是 JSON")
}

fn settings_from(value: &Value) -> ReplyFilterSettings {
    ReplyFilterSettings {
        blocked_mids: value["blocked_mids"]
            .as_array()
            .map(|items| items.iter().filter_map(Value::as_i64).collect())
            .unwrap_or_default(),
        anti_goods: value["anti_goods"].as_bool().unwrap_or(false),
        block_at_only: value["block_at_only"].as_bool().unwrap_or(false),
        // 缺字段 = 基线的 null = 不启用。
        block_at_count: value["block_at_count"].as_i64(),
        ban_word: value["ban_word"].as_str().unwrap_or_default().to_owned(),
    }
}

fn item_from(value: &Value) -> ReplyFilterItem {
    ReplyFilterItem {
        mid: value["mid"].as_i64().unwrap_or_default(),
        message: value["message"].as_str().unwrap_or_default().to_owned(),
        at_names: value["at_names"]
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .filter_map(|n| n.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default(),
        has_emotes: value["has_emotes"].as_bool().unwrap_or(false),
        has_pictures: value["has_pictures"].as_bool().unwrap_or(false),
        url_extras: value["url_extras"]
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .map(|e| ReplyUrlExtra {
                        goods_cm_control: e["goods_cm_control"].as_i64().unwrap_or_default(),
                        has_goods_item_id: e["has_goods_item_id"].as_bool().unwrap_or(false),
                        has_goods_prefetched_cache: e["has_goods_prefetched_cache"]
                            .as_bool()
                            .unwrap_or(false),
                    })
                    .collect()
            })
            .unwrap_or_default(),
        root: value["root"].as_i64().unwrap_or_default(),
        parent: value["parent"].as_i64().unwrap_or_default(),
    }
}

#[test]
fn filtering_matches_the_baseline_cases() {
    let fixture = fixture();
    let cases = fixture["cases"].as_array().expect("cases 是数组");
    assert!(!cases.is_empty(), "用例为空，本测试形同虚设");

    for case in cases {
        let name = case["name"].as_str().unwrap_or("<未命名>");
        let filter = ReplyFilter::compile(&settings_from(&case["settings"]))
            .unwrap_or_else(|e| panic!("用例「{name}」的设置应当可编译: {e}"));
        let expected = case["removed"].as_bool().expect("removed 是 bool");

        assert_eq!(
            filter.removes(&item_from(&case["item"])),
            expected,
            "用例「{name}」"
        );
    }
}

#[test]
fn divergent_patterns_are_rejected() {
    let fixture = fixture();
    let patterns = fixture["rejected_patterns"]["patterns"]
        .as_array()
        .expect("patterns 是数组");
    assert!(!patterns.is_empty(), "用例为空，本测试形同虚设");

    for pattern in patterns {
        let pattern = pattern.as_str().expect("pattern 是字符串");
        let err = ReplyFilter::compile(&ReplyFilterSettings {
            ban_word: pattern.to_owned(),
            ..ReplyFilterSettings::default()
        })
        .expect_err("两端语义不同的构造必须被拒");

        assert_eq!(err.kind(), ErrorKind::Unsupported, "正则 {pattern}");
    }
}
