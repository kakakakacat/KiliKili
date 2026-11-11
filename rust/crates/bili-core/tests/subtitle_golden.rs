#![allow(clippy::expect_used)]

//! Rust 转出来的 WebVTT / SRT 必须与基线**逐字节**相同。
//!
//! ```text
//! <name>.source.json  字幕文件的正文（tool/fixture-gen/subtitle_fixture.dart）
//!   ↓ 基线 json2Vtt / json2Srt
//! <name>.vtt / <name>.srt   期望值
//! ```
//!
//! 逐字节而不是「大致一样」：这份文本直接喂给播放器，少一个换行就是
//! 整条字幕轨不显示；时间戳差 1 毫秒看不出来，但它说明舍入规则分岔了，
//! 那在别的值上可能不止差 1 毫秒。

use std::fs;
use std::path::{Path, PathBuf};

use bili_core::subtitle_format::{to_srt, to_vtt};

fn sources() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/subtitle");
    let mut sources: Vec<_> = fs::read_dir(dir)
        .expect("fixture directory exists")
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            path.to_string_lossy()
                .ends_with(".source.json")
                .then_some(path)
        })
        .collect();
    sources.sort();
    assert!(!sources.is_empty(), "fixture 目录为空，本测试形同虚设");
    sources
}

#[test]
fn subtitle_conversion_matches_the_dart_baseline() {
    for source in sources() {
        let stem = source.to_string_lossy().replace(".source.json", "");
        let body = fs::read_to_string(&source).expect("source readable");
        let cues = bili_api::subtitle::parse_cues(&body)
            .unwrap_or_else(|e| panic!("{}: {e}", source.display()))
            .expect("fixture 里都有 body");

        assert_eq!(
            to_vtt(&cues),
            fs::read_to_string(format!("{stem}.vtt")).expect("golden vtt exists"),
            "{}: WebVTT",
            source.display()
        );
        assert_eq!(
            to_srt(&cues),
            fs::read_to_string(format!("{stem}.srt")).expect("golden srt exists"),
            "{}: SRT",
            source.display()
        );
    }
}

/// fixture 里必须有那几个刁钻的时间戳，否则这条链只证明了「整秒能转」。
#[test]
fn the_fixtures_cover_the_rounding_edges() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/subtitle/tricky_times.source.json");
    let body = fs::read_to_string(path).expect("fixture readable");
    let cues = bili_api::subtitle::parse_cues(&body)
        .expect("合法")
        .expect("有 body");
    let times: Vec<f64> = cues.iter().flat_map(|c| [c.from, c.to]).collect();

    // 精确平局（奇数/16），两端舍入模式不同就会在这里分岔
    assert!(times.contains(&0.0625));
    // SRT 的毫秒会进到 1000，基线输出四位毫秒
    assert!(times.contains(&1.9996));
    // 进位跨到整数位
    assert!(times.contains(&59.9999));
}
