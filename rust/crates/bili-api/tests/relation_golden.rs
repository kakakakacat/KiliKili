//! 关注分组与关注关系的 golden 快照（S6 #56 / S7 #57）。
//!
//! 对照机制同 ADR-011 决策五：本测试冻结 Rust 的解析结果。
//! 迁移时已用不 import lib/ 的一次性 Dart parity 逐字段比对同一份 raw；
//! 对照通过后按 AGENTS.md 与被替换模型一起删除，长期保留本测试与快照。
//!
//! ## fixture 全部是手工构造的
//!
//! 这一族**整条都要登录态**（`/x/relation/tags`、`/x/relation/tag`、
//! `/x/relation` 都是「我的」数据），匿名取不到；而把真实账号的分组名、
//! 关注的人与关注时间写进仓库，脱敏之后剩下的也就是这几条示例。与其抓一份
//! 再逐条改名，不如直接写出要验的那几种情形，每一条都对着基线的某个分支：
//!
//! - `follow_tags`：`tip` 有值 / 为空串 / 为 null，以及 `tagid` 的两个特殊
//!   值（`0` 默认分组、`-10` 特别关注）——界面要按 `tagid == 0` 把默认分组
//!   从面板里摘掉（`lib/pages/group_panel/view.dart`）；
//! - `follow_group`：**裸数组**形态（这条接口的 `data` 没有 `{list,total}`
//!   那层包装），外加缺 `mid` 与空签名这两个基线分支；
//! - `relation` / `relation_no_tag`：`tag` 有值与为 `null` 两种。这两种在
//!   这条接口上**含义不同**（见 `bili_domain::user_list::Relation::tag`），
//!   所以要两份 fixture，不能靠一份里的一个字段兼顾。
//!
//! 它们按基线模型的字段逐一写成，证明的是**映射一致**，不能证明我们对协议
//! 的理解正确。真机确认按 #67 的约定留到 Phase 2。
//!
//! 重新生成：
//! ```sh
//! UPDATE_GOLDEN=1 cargo test -p bili-api --test relation_golden
//! ```

#![allow(clippy::expect_used)]

use std::fs;
use std::path::{Path, PathBuf};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/relation")
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
fn follow_tags_matches_the_frozen_snapshot() {
    check("follow_tags.raw.json", "follow_tags.rust.json", |raw| {
        value(&bili_api::relation::parse_follow_up_tags(raw).expect("样本应解析成功"))
    });
}

/// 分组内的人走的是**裸数组**那条解析，不是 `parse_follow_list`。两条路径
/// 的元素映射共用同一个函数，这里固定的是「外层形状不同也能解析，且 total
/// 为 None」。
#[test]
fn follow_group_matches_the_frozen_snapshot() {
    check("follow_group.raw.json", "follow_group.rust.json", |raw| {
        value(&bili_api::follow::parse_follow_user_array(raw).expect("样本应解析成功"))
    });
}

#[test]
fn relation_matches_the_frozen_snapshot() {
    check("relation.raw.json", "relation.rust.json", |raw| {
        value(&bili_api::relation::parse_relation(raw).expect("样本应解析成功"))
    });
}

#[test]
fn relation_without_tag_matches_the_frozen_snapshot() {
    check(
        "relation_no_tag.raw.json",
        "relation_no_tag.rust.json",
        |raw| value(&bili_api::relation::parse_relation(raw).expect("样本应解析成功")),
    );
}

/// 分组列表不给总数。固定这件事，免得有人看到快照里的 `total: null` 以为
/// 是解析漏了，去给它编一个（编出来的总数会让「还有没有下一页」判错）。
#[test]
fn the_group_list_carries_no_total() {
    let raw = fs::read_to_string(fixture("follow_group.raw.json")).expect("可读");
    let list = bili_api::follow::parse_follow_user_array(&raw).expect("应解析成功");
    assert_eq!(list.total, None, "/x/relation/tag 的响应里本来就没有 total");
    assert_eq!(list.list.len(), 2);
}
