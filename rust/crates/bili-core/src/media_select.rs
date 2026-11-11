//! 清晰度、编码与音质的选择。
//!
//! RFC-0001 §16.1 把**清晰度选择**明确划给 Rust Player Orchestrator：
//! 它是业务规则（用户偏好 + 服务端给了什么 + 降级策略），不是播放后端
//! 的事实。基线实现在 `lib/utils/video_utils.dart`
//! （`findTargetVideo` / `findTargetAudio` / `selectCodec`）与
//! `lib/utils/extension/iterable_ext.dart` 的 `findTargetMedia`。
//!
//! ## 输入为什么不是 `bili_domain::playurl::MediaStream`
//!
//! 算法只用到 `id` 与 `codecs` 两个字段。Stage 1 的取流响应仍由 Dart
//! 持有（`rust.fetch.playUrl` 默认关闭），跨 FFI 送整份 `MediaStream`
//! 等于为了两个字段拷贝十几个没人看的字段。因此这里定义**最小输入**
//! [`VideoCandidate`] / [`FormatCandidate`]，并提供从领域实体来的转换：
//! Phase 2 由 Rust 持有取流结果时，一行 `map` 即可，算法不用改。
//!
//! ## 与基线的等价性
//!
//! 逐行对着基线写，包括几处「基线会抛异常」的地方——那些位置本模块
//! 返回 [`ErrorKind::Unsupported`]，调用方降级回 Dart，由基线原样抛出。
//! 不在 Rust 侧替基线「修」掉它们：修了就不是同一个行为，而这条路径
//! 的开关是可以随时关掉的，两边必须给出同一个结果。

use bili_domain::error::{DomainError, ErrorKind};
use bili_domain::playurl::{MediaStream, PlayFormat};

/// 视频解码格式。顺序与基线 `VideoDecodeFormatType` 的声明顺序一致——
/// 用户偏好列表按它排，顺序即优先级。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VideoCodec {
    Dvh1,
    Av1,
    Hevc,
    Avc,
}

impl VideoCodec {
    /// 该格式在 `codecs` 字符串里的前缀。与基线的 `codes` 表一字不差。
    #[must_use]
    pub const fn prefixes(self) -> &'static [&'static str] {
        match self {
            Self::Dvh1 => &["dvh1"],
            Self::Av1 => &["av01"],
            Self::Hevc => &["hev1", "hvc1"],
            Self::Avc => &["avc1"],
        }
    }

    /// 声明顺序，用于与 Dart 侧的枚举下标互转。
    #[must_use]
    pub const fn all() -> [Self; 4] {
        [Self::Dvh1, Self::Av1, Self::Hevc, Self::Avc]
    }

    /// 这个 `codecs` 字符串是否属于该格式。
    #[must_use]
    pub fn matches(self, codecs: &str) -> bool {
        self.prefixes().iter().any(|p| codecs.starts_with(p))
    }

    /// 由 `codecs` 字符串判定格式。
    ///
    /// 基线的 `fromString` 用的是 `firstWhere` 且**没有** `orElse`：认不
    /// 出就抛 `StateError`。这里返回 `None`，由调用方转成错误——行为相同，
    /// 只是把「抛异常」换成了「让调用方降级」。
    #[must_use]
    pub fn from_codecs(codecs: &str) -> Option<Self> {
        Self::all().into_iter().find(|c| c.matches(codecs))
    }
}

/// 选择算法用到的视频流字段。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoCandidate {
    pub id: i64,
    /// 服务端给的 `codecs`。基线此处是 `e.codecs!`——为空会抛，见模块文档。
    pub codecs: Option<String>,
}

impl From<&MediaStream> for VideoCandidate {
    fn from(stream: &MediaStream) -> Self {
        Self {
            id: stream.id,
            codecs: stream.codecs.clone(),
        }
    }
}

/// 选择算法用到的 `support_formats` 字段。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatCandidate {
    pub quality: i64,
    pub codecs: Option<Vec<String>>,
}

impl From<&PlayFormat> for FormatCandidate {
    fn from(format: &PlayFormat) -> Self {
        Self {
            // 基线的 `FormatItem.quality` 由同名字段来，缺席时取 0；
            // 0 不会等于任何真实清晰度，于是落到 `supportFormats.first`。
            quality: format.quality.unwrap_or(0),
            codecs: format.codecs.clone(),
        }
    }
}

/// 一次视频选择的结果，全部以**下标**表示。
///
/// 返回下标而非对象：Stage 1 的 `VideoItem` / `FormatItem` 在 Dart 侧，
/// 把它们搬过来再搬回去没有意义，调用方用下标取回自己的对象即可。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VideoSelection {
    /// 最终选中的清晰度（等于选中视频流的 `id`）。
    pub quality: i64,
    /// `support_formats` 里的下标。
    pub format_index: usize,
    pub codec: VideoCodec,
    /// 视频流列表里的下标。
    pub video_index: usize,
}

/// 基线 `findTargetMedia` 的等价实现：返回**不超过** `prefer_id` 的最大项；
/// 没有这样的项时返回 id 最小的那项。
///
/// 相等时取先出现的（`id > selected_id` 是严格大于），与基线一致。
fn find_target_index(ids: impl IntoIterator<Item = i64>, prefer_id: i64) -> Option<usize> {
    let mut selected: Option<(usize, i64)> = None;
    let mut lowest: Option<(usize, i64)> = None;
    for (index, id) in ids.into_iter().enumerate() {
        if id <= prefer_id && selected.is_none_or(|(_, best)| id > best) {
            selected = Some((index, id));
        }
        if lowest.is_none_or(|(_, low)| id < low) {
            lowest = Some((index, id));
        }
    }
    selected.or(lowest).map(|(index, _)| index)
}

/// 基线 `VideoUtils.selectCodec`：按用户偏好的顺序挑第一个可用格式。
///
/// `prefer` 为空、或一个都没命中时，退回「列表里第一条流的格式」——
/// 注意基线取的是 `codecs.first`，即**第一条流**的格式，而不是某种默认值。
///
/// # Errors
/// 退路也认不出来时返回 `Unsupported`（基线此处抛 `StateError`）。
pub fn select_codec(codecs: &[&str], prefer: &[VideoCodec]) -> Result<VideoCodec, DomainError> {
    if !prefer.is_empty() {
        let mut best = prefer.len();
        for codec in codecs {
            for (i, candidate) in prefer.iter().enumerate().take(best) {
                if candidate.matches(codec) {
                    best = i;
                    break;
                }
            }
            if best == 0 {
                break;
            }
        }
        if best < prefer.len() {
            return Ok(prefer[best]);
        }
    }

    let first = codecs.first().ok_or_else(|| {
        DomainError::new(ErrorKind::Unsupported, "support_formats 的 codecs 为空")
    })?;
    VideoCodec::from_codecs(first).ok_or_else(|| {
        DomainError::new(ErrorKind::Unsupported, format!("认不出的编码格式: {first}"))
    })
}

/// 基线 `_findAvailableVideoQuality`。
fn find_available_quality(
    videos: &[VideoCandidate],
    accept_quality: Option<&[i64]>,
    prefer_quality: i64,
) -> Result<i64, DomainError> {
    let highest = videos
        .first()
        .ok_or_else(|| DomainError::new(ErrorKind::Unsupported, "视频流列表为空"))?
        .id;
    if prefer_quality >= highest {
        return Ok(highest);
    }

    if let Some(accept) = accept_quality
        && !accept.is_empty()
    {
        let index = find_target_index(accept.iter().copied(), prefer_quality)
            .ok_or_else(|| DomainError::new(ErrorKind::Unsupported, "accept_quality 为空"))?;
        return Ok(accept[index]);
    }

    let index = find_target_index(videos.iter().map(|v| v.id), prefer_quality)
        .ok_or_else(|| DomainError::new(ErrorKind::Unsupported, "视频流列表为空"))?;
    Ok(videos[index].id)
}

/// 基线 `VideoUtils.findTargetVideo`。
///
/// # Errors
/// 列表为空、`codecs` 缺席、编码认不出——基线在这些位置抛异常，这里
/// 返回 `Unsupported` 让调用方降级回 Dart（见模块文档）。
pub fn find_target_video(
    videos: &[VideoCandidate],
    prefer_quality: i64,
    accept_quality: Option<&[i64]>,
    support_formats: &[FormatCandidate],
    prefer_codecs: &[VideoCodec],
) -> Result<VideoSelection, DomainError> {
    let target_quality = find_available_quality(videos, accept_quality, prefer_quality)?;

    // 没有同清晰度的 format 时退回第一条——基线的 orElse。
    let format_index = support_formats
        .iter()
        .position(|f| f.quality == target_quality)
        .or(if support_formats.is_empty() {
            None
        } else {
            Some(0)
        })
        .ok_or_else(|| DomainError::new(ErrorKind::Unsupported, "support_formats 为空"))?;

    let format_codecs = support_formats[format_index]
        .codecs
        .as_ref()
        // 基线是 `targetFormat.codecs!`：为空会抛 NPE。
        .ok_or_else(|| {
            DomainError::new(ErrorKind::Unsupported, "support_formats 的 codecs 缺席")
        })?;
    let refs: Vec<&str> = format_codecs.iter().map(String::as_str).collect();
    let codec = select_codec(&refs, prefer_codecs)?;

    // 该清晰度下的所有流，保留原下标以便返回。
    let same_quality: Vec<(usize, &VideoCandidate)> = videos
        .iter()
        .enumerate()
        .filter(|(_, v)| v.id == target_quality)
        .collect();
    let (fallback_index, _) = *same_quality.first().ok_or_else(|| {
        // 基线此处是 `targetQaVideoList.first`，空列表会抛 StateError。
        // 真实数据里 accept_quality 可能给出 dash 里没有的清晰度。
        DomainError::new(
            ErrorKind::Unsupported,
            format!("清晰度 {target_quality} 在视频流里不存在"),
        )
    })?;

    let video_index = same_quality
        .iter()
        .find(|(_, v)| {
            v.codecs
                .as_deref()
                .is_some_and(|codecs| codec.matches(codecs))
        })
        .map_or(fallback_index, |(index, _)| *index);

    Ok(VideoSelection {
        // 基线返回的是 `targetVideo.id`，不是 `targetQuality`——两者在
        // 正常数据下相同，但选中的流才是事实。
        quality: videos[video_index].id,
        format_index,
        codec,
        video_index,
    })
}

/// 音质优先级。数值越大音质越高，未知音质为 0。与基线 `AudioQuality.rank`
/// 一致（`lib/models/common/video/audio_quality.dart`）。
#[must_use]
pub const fn audio_rank(code: i64) -> i32 {
    match code {
        30251 => 60, // Hi-Res 无损
        30255 | 30250 => 50,
        30280 => 40,
        30232 => 30,
        30216 => 20,
        _ => 0,
    }
}

/// 基线 `VideoUtils.findTargetAudio`：按 rank 而非 code 比较。
///
/// 返回音频流列表里的下标。
///
/// # Errors
/// 列表为空时返回 `Unsupported`（基线此处 `assert` + `first` 抛异常）。
pub fn find_target_audio(audios: &[i64], prefer_quality: i64) -> Result<usize, DomainError> {
    find_target_index(
        audios.iter().map(|code| i64::from(audio_rank(*code))),
        i64::from(audio_rank(prefer_quality)),
    )
    .ok_or_else(|| DomainError::new(ErrorKind::Unsupported, "音频流列表为空"))
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::{
        FormatCandidate, VideoCandidate, VideoCodec, audio_rank, find_target_audio,
        find_target_video, select_codec,
    };

    fn video(id: i64, codecs: &str) -> VideoCandidate {
        VideoCandidate {
            id,
            codecs: Some(codecs.to_owned()),
        }
    }

    fn format(quality: i64, codecs: &[&str]) -> FormatCandidate {
        FormatCandidate {
            quality,
            codecs: Some(codecs.iter().map(|c| (*c).to_owned()).collect()),
        }
    }

    #[test]
    fn prefers_the_highest_quality_not_above_the_preference() {
        let videos = [video(120, "av01"), video(80, "av01"), video(64, "av01")];
        let formats = [
            format(120, &["av01"]),
            format(80, &["av01"]),
            format(64, &["av01"]),
        ];
        let selection = find_target_video(&videos, 80, None, &formats, &[]).expect("可选");
        assert_eq!(selection.quality, 80);
    }

    #[test]
    fn a_preference_above_everything_takes_the_highest() {
        let videos = [video(80, "av01"), video(64, "av01")];
        let formats = [format(80, &["av01"]), format(64, &["av01"])];
        let selection = find_target_video(&videos, 127, None, &formats, &[]).expect("可选");
        assert_eq!(selection.quality, 80);
    }

    #[test]
    fn a_preference_below_everything_takes_the_lowest() {
        let videos = [video(120, "av01"), video(80, "av01")];
        let formats = [format(120, &["av01"]), format(80, &["av01"])];
        let selection = find_target_video(&videos, 6, None, &formats, &[]).expect("可选");
        assert_eq!(selection.quality, 80, "没有不超过偏好的，取最低的那个");
    }

    #[test]
    fn accept_quality_wins_over_the_stream_list() {
        // accept_quality 里有 100，dash 里也有：基线先用 accept_quality 定档
        let videos = [video(120, "av01"), video(100, "av01"), video(80, "av01")];
        let formats = [
            format(120, &["av01"]),
            format(100, &["av01"]),
            format(80, &["av01"]),
        ];
        let accept = [120, 100, 80];
        let selection =
            find_target_video(&videos, 100, Some(&accept), &formats, &[]).expect("可选");
        assert_eq!(selection.quality, 100);
    }

    #[test]
    fn a_quality_only_in_accept_quality_is_an_error_not_a_guess() {
        // accept_quality 给了 dash 里没有的清晰度：基线在 targetQaVideoList.first
        // 上抛 StateError。我们报错让调用方降级，而不是自作主张挑一个。
        //
        // 偏好必须低于最高清晰度，否则第一步就直接返回最高档，accept_quality
        // 根本不会被查——这正是基线的短路顺序。
        let videos = [video(120, "av01"), video(80, "av01")];
        let formats = [format(120, &["av01"]), format(80, &["av01"])];
        let accept = [120, 100, 80];
        assert!(find_target_video(&videos, 100, Some(&accept), &formats, &[]).is_err());
    }

    #[test]
    fn codec_preference_decides_which_stream_of_the_same_quality() {
        let videos = [
            video(80, "av01.0.08M"),
            video(80, "hev1.1.6"),
            video(80, "avc1.64"),
        ];
        let formats = [format(80, &["av01", "hev1", "avc1"])];
        let selection =
            find_target_video(&videos, 80, None, &formats, &[VideoCodec::Avc]).expect("可选");
        assert_eq!(selection.codec, VideoCodec::Avc);
        assert_eq!(selection.video_index, 2);
    }

    #[test]
    fn the_first_matching_preference_wins() {
        let codecs = ["av01", "hev1", "avc1"];
        let codec = select_codec(&codecs, &[VideoCodec::Hevc, VideoCodec::Av1]).expect("可选");
        assert_eq!(codec, VideoCodec::Hevc, "偏好列表的顺序即优先级");
    }

    #[test]
    fn no_preference_matches_falls_back_to_the_first_codec() {
        let codecs = ["hev1", "avc1"];
        let codec = select_codec(&codecs, &[VideoCodec::Dvh1]).expect("可选");
        assert_eq!(codec, VideoCodec::Hevc, "退回的是第一条流的格式");
    }

    #[test]
    fn unknown_codec_is_an_error_instead_of_a_guess() {
        // 基线的 fromString 没有 orElse，认不出就抛。
        assert!(select_codec(&["vp09"], &[]).is_err());
    }

    #[test]
    fn stream_without_codecs_does_not_match_any_preference() {
        let videos = [
            VideoCandidate {
                id: 80,
                codecs: None,
            },
            video(80, "avc1.64"),
        ];
        let formats = [format(80, &["avc1"])];
        let selection =
            find_target_video(&videos, 80, None, &formats, &[VideoCodec::Avc]).expect("可选");
        assert_eq!(selection.video_index, 1);
    }

    #[test]
    fn missing_format_codecs_is_an_error() {
        let videos = [video(80, "avc1")];
        let formats = [FormatCandidate {
            quality: 80,
            codecs: None,
        }];
        assert!(find_target_video(&videos, 80, None, &formats, &[]).is_err());
    }

    #[test]
    fn format_falls_back_to_the_first_entry() {
        let videos = [video(80, "avc1")];
        let formats = [format(120, &["avc1"]), format(64, &["avc1"])];
        let selection = find_target_video(&videos, 80, None, &formats, &[]).expect("可选");
        assert_eq!(selection.format_index, 0, "没有同清晰度的就取第一条");
    }

    #[test]
    fn audio_is_ranked_not_compared_by_code() {
        // 30251(Hi-Res, 60) > 30255(杜比, 50) > 30280(192K, 40)，
        // 而 code 的大小顺序是 30280 > 30255 > 30251——直接比 code 会反。
        let audios = [30251, 30255, 30280, 30232];
        assert_eq!(find_target_audio(&audios, 30255).expect("可选"), 1);
        assert_eq!(find_target_audio(&audios, 30280).expect("可选"), 2);
        assert_eq!(find_target_audio(&audios, 30251).expect("可选"), 0);
    }

    #[test]
    fn unknown_audio_quality_ranks_zero_and_takes_the_lowest() {
        assert_eq!(audio_rank(12345), 0);
        let audios = [30251, 30232];
        // 偏好 rank 0，没有 rank<=0 的流，于是取 rank 最低的 30232
        assert_eq!(find_target_audio(&audios, 12345).expect("可选"), 1);
    }

    #[test]
    fn empty_lists_are_errors() {
        assert!(find_target_audio(&[], 30251).is_err());
        assert!(find_target_video(&[], 80, None, &[format(80, &["avc1"])], &[]).is_err());
    }
}
