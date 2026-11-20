#![allow(clippy::expect_used)]

//! 线程化视图合成的对照用例（`tests/fixtures/reply_thread/cases.json`）。
//!
//! ## 期望值的来历
//!
//! 与 `reply_filter_golden` / `reply_richtext_golden` 同一条链：期望值是
//! **人写的**，判据在 Dart 那一侧——
//! `test/services/rust/reply_thread_parity_test.dart` 逐行转写了迁移前
//! `tree.dart` 的 `ReplyTree.build`，跑同一份文件。期望值与基线不符时它
//! 会红。
//!
//! 这条路径的输入是一组字段而不是报文，而真实响应里恰恰难得见到边界形状
//! （交叉 id、被屏蔽的祖先链、deferred 的 dialog）。手写用例能把它们钉住。

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use bili_domain::reply_grpc::{ReplyMember, ReplyParentMember};
use bili_domain::reply_thread::{
    ReplyRowReason, ReplyRowSource, ReplyThreadItem, ReplyThreadRow, build,
};
use serde_json::{Value, json};

fn fixture() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/reply_thread/cases.json");
    serde_json::from_str(&fs::read_to_string(path).expect("fixture 可读")).expect("fixture 是 JSON")
}

fn member_from(value: &Value) -> ReplyMember {
    ReplyMember {
        mid: value["mid"].as_i64().unwrap_or_default(),
        name: value["name"].as_str().unwrap_or_default().to_owned(),
        ..Default::default()
    }
}

fn item_from(value: &Value) -> ReplyThreadItem {
    ReplyThreadItem {
        id: value["id"].as_i64().unwrap_or_default(),
        parent: value["parent"].as_i64().unwrap_or_default(),
        dialog: value["dialog"].as_i64().unwrap_or_default(),
        ctime: value["ctime"].as_i64().unwrap_or_default(),
        member: value.get("member").map(member_from).unwrap_or_default(),
        parent_member: value.get("parent_member").map(|m| ReplyParentMember {
            // 缺 mid = 基线的「没有 mid 属性」那一支，不是 0。
            mid: m.get("mid").and_then(Value::as_i64),
            name: m["name"].as_str().unwrap_or_default().to_owned(),
        }),
        members: value
            .get("members")
            .and_then(Value::as_object)
            .map(|map| {
                map.iter()
                    .map(|(name, m)| (name.clone(), member_from(m)))
                    .collect()
            })
            .unwrap_or_default(),
    }
}

fn items_from(value: Option<&Value>) -> Vec<ReplyThreadItem> {
    value
        .and_then(Value::as_array)
        .map(|items| items.iter().map(item_from).collect())
        .unwrap_or_default()
}

/// 等于默认值的字段一律不写出来，这样期望值里只剩真正有判定的那几项。
fn projection(row: &ReplyThreadRow) -> Value {
    let mut out = json!({
        "source": match row.source {
            ReplyRowSource::Data => "data",
            ReplyRowSource::Removed => "removed",
            ReplyRowSource::Synthetic => "synthetic",
        },
        "id": row.id,
        "depth": row.depth,
    });
    #[allow(clippy::unwrap_used)]
    let map = out.as_object_mut().unwrap();
    if let Some(index) = row.index {
        map.insert("index".to_owned(), Value::from(index));
    }
    if let Some(parent) = row.parent {
        map.insert("parent".to_owned(), Value::from(parent));
    }
    if row.reason != ReplyRowReason::None {
        map.insert(
            "reason".to_owned(),
            Value::from(match row.reason {
                ReplyRowReason::None => unreachable!(),
                ReplyRowReason::Deleted => "deleted",
                ReplyRowReason::Blocked => "blocked",
            }),
        );
    }
    if row.collapsed != 0 {
        map.insert("collapsed".to_owned(), Value::from(row.collapsed));
    }
    if !row.visible {
        map.insert("visible".to_owned(), Value::from(false));
    }
    if let Some(placeholder) = row.placeholder.as_ref() {
        map.insert(
            "placeholder".to_owned(),
            json!({
                "parent": placeholder.parent,
                "dialog": placeholder.dialog,
                "ctime": placeholder.ctime,
                "mid": placeholder.member.mid,
                "name": placeholder.member.name,
            }),
        );
    }
    out
}

/// fixture 的键顺序由人写时决定，比较前统一成「字段名排序」，不然
/// `json!` 与手写文件的顺序差异会变成假红。
fn normalize(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort_unstable();
            Value::Object(
                keys.into_iter()
                    .map(|k| (k.clone(), normalize(&map[k])))
                    .collect(),
            )
        }
        Value::Array(items) => Value::Array(items.iter().map(normalize).collect()),
        other => other.clone(),
    }
}

#[test]
fn rows_match_the_fixture() {
    let fixture = fixture();
    let cases = fixture["cases"].as_array().expect("cases 是数组");
    assert!(!cases.is_empty(), "用例为空则本测试形同虚设");

    for case in cases {
        let name = case["name"].as_str().unwrap_or("<无名用例>");
        let data = items_from(case.get("data"));
        let removed: HashMap<i64, ReplyThreadItem> = items_from(case.get("removed"))
            .into_iter()
            .map(|item| (item.id, item))
            .collect();
        let root_id = case["root_id"].as_i64().unwrap_or_default();
        let fold_depth = case["fold_depth"].as_i64().unwrap_or_default() as i32;

        let actual: Vec<Value> = build(&data, &removed, root_id, fold_depth)
            .iter()
            .map(|row| normalize(&projection(row)))
            .collect();
        let expected: Vec<Value> = case["rows"]
            .as_array()
            .expect("rows 是数组")
            .iter()
            .map(normalize)
            .collect();

        assert_eq!(actual, expected, "用例「{name}」");
    }
}
