#![allow(clippy::expect_used)]

//! 清晰度 / 编码 / 音质选择的对照用例，与基线共用一份输入和期望值
//! （`tests/fixtures/media_select/cases.json`）。
//!
//! 基线那侧在 `test/utils/media_select_golden_test.dart`，跑的是真正的
//! `VideoUtils`。期望值是人写的（理由见 fixture 顶部的 comment），因此
//! **判据是那个 Dart 测试**：期望值与基线不符时，它在 CI 里会红。

use std::fs;
use std::path::Path;

use bili_core::media_select::{
    FormatCandidate, VideoCandidate, VideoCodec, find_target_audio, find_target_video,
};
use serde_json::Value;

fn cases() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/media_select/cases.json");
    serde_json::from_str(&fs::read_to_string(path).expect("fixture readable"))
        .expect("fixture JSON")
}

fn codec_from_name(name: &str) -> VideoCodec {
    match name {
        "DVH1" => VideoCodec::Dvh1,
        "AV1" => VideoCodec::Av1,
        "HEVC" => VideoCodec::Hevc,
        "AVC" => VideoCodec::Avc,
        other => panic!("fixture 里认不出的编码名: {other}"),
    }
}

fn i64_list(value: &Value) -> Vec<i64> {
    value
        .as_array()
        .map(|items| items.iter().filter_map(Value::as_i64).collect())
        .unwrap_or_default()
}

#[test]
fn video_selection_matches_the_baseline_cases() {
    let cases = cases();
    let videos_cases = cases["video"].as_array().expect("video 用例是数组");
    assert!(!videos_cases.is_empty(), "用例为空，本测试形同虚设");

    for case in videos_cases {
        let name = case["name"].as_str().unwrap_or("<未命名>");
        let videos: Vec<VideoCandidate> = case["videos"]
            .as_array()
            .expect("videos 是数组")
            .iter()
            .map(|v| VideoCandidate {
                id: v["id"].as_i64().expect("id"),
                codecs: v["codecs"].as_str().map(str::to_owned),
            })
            .collect();
        let formats: Vec<FormatCandidate> = case["support_formats"]
            .as_array()
            .expect("support_formats 是数组")
            .iter()
            .map(|f| FormatCandidate {
                quality: f["quality"].as_i64().expect("quality"),
                codecs: f["codecs"].as_array().map(|items| {
                    items
                        .iter()
                        .filter_map(|c| c.as_str().map(str::to_owned))
                        .collect()
                }),
            })
            .collect();
        let accept = i64_list(&case["accept_quality"]);
        let prefer_codecs: Vec<VideoCodec> = case["prefer_codecs"]
            .as_array()
            .expect("prefer_codecs 是数组")
            .iter()
            .filter_map(|c| c.as_str().map(codec_from_name))
            .collect();

        let selection = find_target_video(
            &videos,
            case["prefer_quality"].as_i64().expect("prefer_quality"),
            Some(&accept),
            &formats,
            &prefer_codecs,
        )
        .unwrap_or_else(|e| panic!("{name}: {e}"));

        let expected = &case["expected"];
        assert_eq!(
            selection.quality,
            expected["quality"].as_i64().expect("quality"),
            "{name}: 清晰度"
        );
        assert_eq!(
            selection.format_index as i64,
            expected["format_index"].as_i64().expect("format_index"),
            "{name}: support_formats 下标"
        );
        assert_eq!(
            selection.video_index as i64,
            expected["video_index"].as_i64().expect("video_index"),
            "{name}: 视频流下标"
        );
        assert_eq!(
            selection.codec,
            codec_from_name(expected["codec"].as_str().expect("codec")),
            "{name}: 编码"
        );
    }
}

#[test]
fn audio_selection_matches_the_baseline_cases() {
    let cases = cases();
    let audio_cases = cases["audio"].as_array().expect("audio 用例是数组");
    assert!(!audio_cases.is_empty(), "用例为空，本测试形同虚设");

    for case in audio_cases {
        let name = case["name"].as_str().unwrap_or("<未命名>");
        let audios = i64_list(&case["audios"]);
        let index = find_target_audio(
            &audios,
            case["prefer_quality"].as_i64().expect("prefer_quality"),
        )
        .unwrap_or_else(|e| panic!("{name}: {e}"));

        let expected = &case["expected"];
        assert_eq!(
            index as i64,
            expected["audio_index"].as_i64().expect("audio_index"),
            "{name}: 音频流下标"
        );
        assert_eq!(
            audios[index],
            expected["id"].as_i64().expect("id"),
            "{name}: 音质 code"
        );
    }
}
