//! 清晰度 / 编码 / 音质选择的 Dart 导出。
//!
//! 纯计算，没有 I/O：输入是几十条「id + codecs」，输出是几个下标。因此
//! 两个函数都是 `#[frb(sync)]`——基线 `VideoUtils.findTargetVideo` 是同步
//! 的，改成 Future 会迫使两个调用方（播放与下载）都改成异步，而它们正处
//! 在起播路径上。
//!
//! 返回**下标**而不是对象：Stage 1 的取流响应仍由 Dart 持有，把
//! `VideoItem` / `FormatItem` 搬过来再搬回去没有意义。
//!
//! 算法在 `bili_core::media_select`，本模块只做类型转换（RFC-0001 §3：
//! 桥接层不承载业务逻辑）。

use bili_core::media_select::{
    FormatCandidate, VideoCandidate, VideoCodec, find_target_audio, find_target_video,
};
use flutter_rust_bridge::frb;

/// 一条视频流里选择算法用得到的部分。
#[derive(Debug, Clone)]
pub struct VideoCandidateInput {
    pub id: i64,
    /// 基线此处是 `e.codecs!`，为空会抛 NPE；这里允许缺席，缺席的流只是
    /// 不会命中任何编码偏好。
    pub codecs: Option<String>,
}

/// `support_formats` 的一项。
#[derive(Debug, Clone)]
pub struct FormatCandidateInput {
    pub quality: i64,
    pub codecs: Option<Vec<String>>,
}

/// 选择结果。`selection` 为 `None` 即失败，调用方走 Dart 原实现。
#[derive(Debug, Clone)]
pub struct VideoSelectionResult {
    pub selection: Option<VideoSelectionOutput>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct VideoSelectionOutput {
    /// 选中流的 `id`，即最终清晰度。
    pub quality: i64,
    /// 传入的 `support_formats` 里的下标。
    pub format_index: u32,
    /// 传入的视频流列表里的下标。
    pub video_index: u32,
    /// Dart `VideoDecodeFormatType.values` 的下标。
    ///
    /// 用下标而不是字符串：两端的枚举本就是同一张表，字符串要多一次
    /// `byName` 且拼错只能在运行时发现。顺序一致由
    /// [`codec_order_matches_the_dart_enum`] 钉住。
    pub codec_index: i32,
}

/// 音质选择结果。`index` 为 `None` 即失败。
#[derive(Debug, Clone)]
pub struct AudioSelectionResult {
    pub index: Option<u32>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
}

/// 按用户偏好挑清晰度、编码与具体视频流。
///
/// `prefer_codec_indices` 是 Dart `VideoDecodeFormatType` 的下标，顺序即
/// 优先级。认不出的下标**直接忽略**，不报错：偏好少一项的后果是退回
/// 下一个偏好，而报错会让整条路径降级，代价更大。
#[frb(sync)]
#[must_use]
pub fn select_target_video(
    videos: Vec<VideoCandidateInput>,
    prefer_quality: i64,
    accept_quality: Option<Vec<i64>>,
    support_formats: Vec<FormatCandidateInput>,
    prefer_codec_indices: Vec<i32>,
) -> VideoSelectionResult {
    let videos: Vec<VideoCandidate> = videos
        .into_iter()
        .map(|v| VideoCandidate {
            id: v.id,
            codecs: v.codecs,
        })
        .collect();
    let formats: Vec<FormatCandidate> = support_formats
        .into_iter()
        .map(|f| FormatCandidate {
            quality: f.quality,
            codecs: f.codecs,
        })
        .collect();
    let prefer: Vec<VideoCodec> = prefer_codec_indices
        .into_iter()
        .filter_map(codec_from_index)
        .collect();

    match find_target_video(
        &videos,
        prefer_quality,
        accept_quality.as_deref(),
        &formats,
        &prefer,
    ) {
        Ok(selection) => VideoSelectionResult {
            selection: Some(VideoSelectionOutput {
                quality: selection.quality,
                format_index: u32::try_from(selection.format_index).unwrap_or(u32::MAX),
                video_index: u32::try_from(selection.video_index).unwrap_or(u32::MAX),
                codec_index: codec_index(selection.codec),
            }),
            error_kind: None,
            error_detail: None,
        },
        Err(e) => VideoSelectionResult {
            selection: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
        },
    }
}

/// 按用户偏好挑音频流，返回下标。
#[frb(sync)]
#[must_use]
pub fn select_target_audio(audio_ids: Vec<i64>, prefer_quality: i64) -> AudioSelectionResult {
    match find_target_audio(&audio_ids, prefer_quality) {
        Ok(index) => AudioSelectionResult {
            index: Some(u32::try_from(index).unwrap_or(u32::MAX)),
            error_kind: None,
            error_detail: None,
        },
        Err(e) => AudioSelectionResult {
            index: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
        },
    }
}

/// Dart 枚举下标 → Rust 枚举。
#[frb(ignore)]
fn codec_from_index(index: i32) -> Option<VideoCodec> {
    usize::try_from(index)
        .ok()
        .and_then(|i| VideoCodec::all().get(i).copied())
}

/// Rust 枚举 → Dart 枚举下标。
#[frb(ignore)]
fn codec_index(codec: VideoCodec) -> i32 {
    VideoCodec::all()
        .iter()
        .position(|c| *c == codec)
        .and_then(|i| i32::try_from(i).ok())
        .unwrap_or(-1)
}

#[cfg(test)]
mod tests {
    use super::{codec_from_index, codec_index};
    use bili_core::media_select::VideoCodec;

    /// Dart 的 `VideoDecodeFormatType` 声明顺序是 DVH1 / AV1 / HEVC / AVC，
    /// 下标跨 FFI 传递，两边必须一致。这条断言是那份一致性的唯一守卫——
    /// 改了任一侧的顺序，它会红。
    #[test]
    fn codec_order_matches_the_dart_enum() {
        assert_eq!(codec_index(VideoCodec::Dvh1), 0);
        assert_eq!(codec_index(VideoCodec::Av1), 1);
        assert_eq!(codec_index(VideoCodec::Hevc), 2);
        assert_eq!(codec_index(VideoCodec::Avc), 3);
        assert_eq!(codec_from_index(0), Some(VideoCodec::Dvh1));
        assert_eq!(codec_from_index(3), Some(VideoCodec::Avc));
    }

    #[test]
    fn out_of_range_indices_are_ignored() {
        assert_eq!(codec_from_index(-1), None);
        assert_eq!(codec_from_index(4), None);
    }
}
