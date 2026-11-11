//! 直播只读接口的 golden 快照（S11a 第一批 / issue #61）。
//!
//! 对照机制同 ADR-011 决策五：本测试冻结 Rust 的解析结果，
//! `test/services/rust/live_parity_test.dart` 用迁移前那些 `fromJson`
//! 的逐行转写解析同一份 raw，与快照逐字段比对，通过后随旧模型一并删除。
//!
//! ## fixture 的来源
//!
//! 七份 raw **全部是手工构造**的脱敏样本，不是真实抓取。字段名、嵌套
//! 层级与 JSON 类型按基线 `lib/models_new/live/**` 的 `fromJson` 写成，
//! 房间 id、mid、昵称与 CDN 地址一律是占位值。
//!
//! 这一点必须说清楚：这些快照证明的是「我们对协议的理解自洽」，**不能**
//! 证明理解本身正确。真机确认统一留到 Phase 2（issue #61 约束 6）。
//!
//! 样本刻意覆盖了几处解析分歧，它们都是迁移时最容易悄悄改掉行为的地方：
//!
//! - `room_area_list` 里 `id` 既有字符串也有整数——基线声明成 `dynamic`
//!   并原样透传，于是调用点的 `e.id == areaId` 比较恒为假；
//! - `feed_index` 混入 `banner_v2` 与一个虚构的新卡片类型，钉住「未知
//!   卡片被忽略而不是让整页失败」；
//! - `second_list` 的房间 id 键是 `id` 而非 `roomid`；
//! - `follow` 里有未开播与缺 `live_status` 两条，钉住过滤口径；
//! - 两处 `system_cover` 为空串，钉住回落到 `cover`。
//!
//! 重新生成：
//! ```sh
//! UPDATE_GOLDEN=1 cargo test -p bili-api --test live_golden
//! ```

#![allow(clippy::expect_used)]

use std::fs;
use std::path::{Path, PathBuf};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/live")
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
fn area_list_matches_the_frozen_snapshot() {
    check("area_list.raw.json", "area_list.rust.json", |raw| {
        value(&bili_api::live::parse_area_list(raw).expect("样本应解析成功"))
    });
}

#[test]
fn room_area_list_matches_the_frozen_snapshot() {
    check(
        "room_area_list.raw.json",
        "room_area_list.rust.json",
        |raw| value(&bili_api::live::parse_room_area_list(raw).expect("样本应解析成功")),
    );
}

#[test]
fn fav_tag_matches_the_frozen_snapshot() {
    check("fav_tag.raw.json", "fav_tag.rust.json", |raw| {
        value(&bili_api::live::parse_fav_tags(raw).expect("样本应解析成功"))
    });
}

#[test]
fn feed_index_matches_the_frozen_snapshot() {
    check("feed_index.raw.json", "feed_index.rust.json", |raw| {
        value(&bili_api::live::parse_index(raw).expect("样本应解析成功"))
    });
}

#[test]
fn second_list_matches_the_frozen_snapshot() {
    check("second_list.raw.json", "second_list.rust.json", |raw| {
        value(&bili_api::live::parse_second_list(raw).expect("样本应解析成功"))
    });
}

#[test]
fn follow_matches_the_frozen_snapshot() {
    check("follow.raw.json", "follow.rust.json", |raw| {
        value(&bili_api::live::parse_follow(raw).expect("样本应解析成功"))
    });
}

#[test]
fn room_h5_matches_the_frozen_snapshot() {
    check("room_h5.raw.json", "room_h5.rust.json", |raw| {
        value(&bili_api::live::parse_room_h5(raw).expect("样本应解析成功"))
    });
}

#[test]
fn room_play_info_matches_the_frozen_snapshot() {
    check(
        "room_play_info.raw.json",
        "room_play_info.rust.json",
        |raw| value(&bili_api::live::parse_room_play_info(raw).expect("样本应解析成功")),
    );
}

#[test]
fn room_play_info_offline_matches_the_frozen_snapshot() {
    check(
        "room_play_info_offline.raw.json",
        "room_play_info_offline.rust.json",
        |raw| value(&bili_api::live::parse_room_play_info(raw).expect("样本应解析成功")),
    );
}

/// 取流的**请求面**快照：参数集与 WBI 签名。
///
/// 解析快照只管响应，管不住请求。而取流那串参数里有好几个看不出含义、
/// 又必须逐字正确的常量——`web_location` 是浮点字面量 `444.8`，
/// `ptype=8` / `dolby=5` / `panorama=1` 决定响应里有哪些档位。它们一旦
/// 被「顺手规整」成 `444` 或少写一项，`w_rid` 跟着变，服务端回的是业务
/// 错误码而不是解析失败，从现象完全反推不出是签名参数错了。
///
/// 期望值由 `test/services/rust/live_playurl_parity_test.dart` 用基线
/// `WbiSign.encWbi` 的逐字拷贝独立算一遍核对（ADR-008 决策三）。
#[test]
fn room_play_info_request_matches_the_frozen_snapshot() {
    // 固定的测试用 mixin_key 与 wts，使签名可复现。不是真实密钥。
    const MIXIN_KEY: &str = "ea1db124af3c7062474693fa704f4ff8";
    const WTS: i64 = 1_762_800_000;

    let cases = [
        (
            "常规：指定画质、含视频",
            100_001_i64,
            Some(10_000_i64),
            false,
        ),
        ("仅音频：不指定画质", 100_002, None, true),
    ];

    let got: Vec<serde_json::Value> = cases
        .iter()
        .map(|&(name, room_id, qn, only_audio)| {
            let req = bili_api::live::build_room_play_info_request(
                room_id, qn, only_audio, MIXIN_KEY, WTS,
            );
            serde_json::json!({
                "case": name,
                "mixin_key": MIXIN_KEY,
                "wts": WTS,
                "path": req.path,
                "query": req
                    .query
                    .iter()
                    .map(|(k, v)| serde_json::json!([k, v]))
                    .collect::<Vec<_>>(),
            })
        })
        .collect();

    let snapshot_path = fixture("room_play_info_request.rust.json");
    let got = serde_json::Value::Array(got);

    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        let pretty = serde_json::to_string_pretty(&got).expect("可序列化");
        fs::write(&snapshot_path, format!("{pretty}\n")).expect("快照可写");
        return;
    }

    let expected: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(&snapshot_path).expect("缺少快照，用 UPDATE_GOLDEN=1 重新生成"),
    )
    .expect("快照必须是合法 JSON");

    assert_eq!(got, expected, "取流请求面与冻结快照不一致");
}

/// `base_url` 缺失必须让整次解析失败，而不是填空串继续。
///
/// 基线 `CodecItem.fromJson` 写的是 `json['base_url'] as String`，缺了
/// 就抛，被 `liveRoomInfo` 的 `catch` 收成 `Error`。如果这里退化成
/// `unwrap_or_default`，调用方会拿到一个拼得出来、但指向
/// `host + "" + extra` 的地址——播放器拿它去请求，失败在更远的地方。
#[test]
fn missing_base_url_is_an_error_not_an_empty_string() {
    let raw = r#"{
        "code": 0,
        "data": {
            "room_id": 1,
            "playurl_info": {
                "playurl": {
                    "g_qn_desc": [],
                    "stream": [{
                        "protocol_name": "http_stream",
                        "format": [{
                            "format_name": "flv",
                            "codec": [{
                                "codec_name": "avc",
                                "current_qn": 400,
                                "accept_qn": [400],
                                "url_info": []
                            }]
                        }]
                    }]
                }
            }
        }
    }"#;

    assert!(
        bili_api::live::parse_room_play_info(raw).is_err(),
        "缺 base_url 应解析失败"
    );
}
