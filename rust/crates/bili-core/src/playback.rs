//! 取流结果之上的播放决策：补档与音量均衡参数。
//!
//! 这些规则原本写在基线 Dart 的模型与控制器里（`PlayUrlModel
//! .missingVideoQualityBelowHighest`、`BaseItemExt.merge`、`Volume.format`）。
//! 它们既不是协议解析，也不是某个 UI 框架的事：换掉 Flutter 这层之后，
//! 「补哪一档清晰度」「两次取流怎么并」「loudnorm 怎么填」一个字都不会变。
//! 所以放在应用层，宿主只负责调用。

use bili_domain::playurl::{ClipInfo, MediaStream, Volume};
use bili_domain::sponsor_block::SkipSegment;

/// 低于最高档、但 DASH 里没给出的最高一档清晰度；没有可补的则返回 `None`。
///
/// 取流一次只会给出当前档位及以下的若干档，想把清晰度菜单补齐就得按这个
/// 档位再取一次。
///
/// 缺 `dash`、缺 `support_formats`、空列表都返回 `None`——基线用一个吞掉
/// 一切的 try/catch 表达同一件事：补档失败不该打断起播。
#[must_use]
pub fn missing_video_quality_below_highest(
    videos: &[MediaStream],
    support_qualities: &[i64],
) -> Option<i64> {
    let highest = videos.first()?.id;
    let available: Vec<i64> = videos.iter().map(|v| v.id).collect();

    support_qualities
        .iter()
        .copied()
        .filter(|q| *q < highest && !available.contains(q))
        .max()
}

/// 把补档那次取流的视频流并进原列表，按清晰度从高到低排好。
///
/// 去重键是 `(id, codecid)`：同一档清晰度会有多种编码，只按档位去重会把
/// AVC/HEVC/AV1 里的两种丢掉。
///
/// 基线是**就地改**那个列表的；这里返回新列表——跨 FFI 的结构不可变，而
/// 原地可变性本来也不是这条规则的一部分。
#[must_use]
pub fn merge_video_streams(base: Vec<MediaStream>, extra: &[MediaStream]) -> Vec<MediaStream> {
    let mut merged = base;
    if extra.is_empty() {
        return merged;
    }
    let mut keys: Vec<(i64, Option<i64>)> = merged.iter().map(|v| (v.id, v.codecid)).collect();
    for item in extra {
        let key = (item.id, item.codecid);
        if !keys.contains(&key) {
            keys.push(key);
            merged.push(item.clone());
        }
    }
    merged.sort_by_key(|s| std::cmp::Reverse(s.id));
    merged
}

/// FFmpeg `loudnorm` 滤镜的标准有效范围。
/// <https://ffmpeg.org/ffmpeg-filters.html#loudnorm>
const MIN_TP: f64 = -9.0;
const MAX_TP: f64 = 0.0;

/// 用户/预设给的 loudnorm 配置。缺省项由服务端给的测量值补。
#[derive(Debug, Clone, Copy, Default)]
pub struct LoudnormConfig {
    pub lra: Option<f64>,
    pub i: Option<f64>,
    pub tp: Option<f64>,
    pub offset: Option<f64>,
}

/// 拼出 `loudnorm` 的参数串。逐行对着基线 `Volume.format` 写。
///
/// 这是 FFmpeg 的语法，不是 Flutter 的：换 UI 框架不会换掉它，换播放
/// 后端才会。真换了后端，要改的也是这一个函数，而不是散在宿主各处。
#[must_use]
pub fn loudnorm_params(volume: &Volume, config: &LoudnormConfig) -> String {
    let lra = config.lra.unwrap_or(11.0).max(volume.measured_lra);
    let mut i = config.i.unwrap_or(volume.target_i);
    let tp = config
        .tp
        .unwrap_or(volume.target_tp)
        .min(volume.measured_tp)
        .clamp(MIN_TP, MAX_TP);
    let offset = config.offset.unwrap_or(volume.target_offset);

    // 正的测量值会让 loudnorm 反向增益。基线把它折进目标值后归零，
    // 照搬——这是两端必须一致的听感行为。
    let mut measured_i = volume.measured_i;
    if measured_i > 0.0 {
        i -= measured_i;
        measured_i = 0.0;
    }
    let measured_threshold = volume.measured_threshold.min(0.0);

    format!(
        "LRA={}:I={}:TP={}:offset={}:linear=true:measured_I={}:measured_LRA={}:measured_TP={}:measured_thresh={}",
        fmt(lra),
        fmt(i),
        fmt(tp),
        fmt(offset),
        fmt(measured_i),
        fmt(volume.measured_lra),
        fmt(volume.measured_tp),
        fmt(measured_threshold),
    )
}

/// 与 Dart 的数字字面量一致地格式化：整数不带小数点。
///
/// Dart 的 `num` 在这些位置上会保留 int/double 的区别（`11` 而不是
/// `11.0`），而 Rust 的 `f64` 一律带小数。FFmpeg 两种都认，但参数串会
/// 进日志、也被对照测试逐字比对，保持一致省掉一类假差异。
fn fmt(value: f64) -> String {
    if value.fract() == 0.0 && value.abs() < 1e15 {
        format!("{}", value as i64)
    } else {
        format!("{value}")
    }
}

/// 服务端是否给了有效的均衡参数。全为 0 即视为没给。
#[must_use]
pub fn has_volume_measurement(volume: &Volume) -> bool {
    volume.measured_i != 0.0
        || volume.measured_lra != 0.0
        || volume.measured_tp != 0.0
        || volume.measured_threshold != 0.0
}

/// PGC 的片头片尾 → 可跳过片段。
///
/// 番剧的 `clip_info_list` 与 SponsorBlock 的片段在播放器里走同一条跳过
/// 逻辑，基线也是转成同一个模型消费的。分类映射（`CLIP_TYPE_OP` → 片头、
/// `CLIP_TYPE_ED` → 片尾、其余归赞助）是业务规则，放在这一层。
///
/// 缺 `start`/`end` 的条目**整条丢弃**而不是让整次转换失败：基线用一个
/// 吞异常的 try/catch 表达「片段没拿到不该打断起播」，但它是整体放弃，
/// 这里按条放弃，更不容易因为一条脏数据丢掉全部片段。
#[must_use]
pub fn pgc_clip_segments(clips: &[ClipInfo], time_length_ms: Option<i64>) -> Vec<SkipSegment> {
    clips
        .iter()
        .filter_map(|clip| {
            let (start, end) = (clip.start?, clip.end?);
            Some(SkipSegment {
                uuid: String::new(),
                category: match clip.clip_type.as_deref() {
                    Some("CLIP_TYPE_OP") => "intro".to_owned(),
                    Some("CLIP_TYPE_ED") => "outro".to_owned(),
                    _ => "sponsor".to_owned(),
                },
                action_type: None,
                // 接口给的是秒，与 SponsorBlock 那条路径归一到同一个单位。
                start_ms: (start * 1000.0).round() as i64,
                end_ms: (end * 1000.0).round() as i64,
                #[allow(clippy::cast_precision_loss)]
                video_duration_ms: time_length_ms.map(|v| v as f64),
                votes: None,
                cid: None,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stream(id: i64, codecid: Option<i64>) -> MediaStream {
        MediaStream {
            id,
            base_url: None,
            backup_url: None,
            band_width: None,
            mime_type: None,
            codecs: None,
            width: None,
            height: None,
            frame_rate: None,
            sar: None,
            start_with_sap: None,
            segment_base: None,
            codecid,
        }
    }

    #[test]
    fn picks_highest_missing_quality_below_the_top() {
        let videos = vec![stream(80, Some(7)), stream(64, Some(7))];
        // 120 高于最高档、80/64 已有，只剩 32 可补。
        assert_eq!(
            missing_video_quality_below_highest(&videos, &[120, 80, 64, 32, 16]),
            Some(32)
        );
        // 16 也缺，但要最高的那一档。
        assert_eq!(
            missing_video_quality_below_highest(&videos, &[80, 16, 32]),
            Some(32)
        );
    }

    #[test]
    fn no_missing_quality_is_none() {
        let videos = vec![stream(80, None)];
        assert_eq!(missing_video_quality_below_highest(&videos, &[80]), None);
        assert_eq!(missing_video_quality_below_highest(&[], &[80]), None);
    }

    #[test]
    fn merge_dedupes_by_quality_and_codec() {
        let base = vec![stream(80, Some(7))];
        let extra = vec![
            // 同 (id, codecid)，丢弃
            stream(80, Some(7)),
            // 同档不同编码，保留
            stream(80, Some(12)),
            stream(32, Some(7)),
        ];
        let merged = merge_video_streams(base, &extra);
        assert_eq!(merged.len(), 3);
        // 从高到低
        assert_eq!(merged[0].id, 80);
        assert_eq!(merged[2].id, 32);
    }

    fn volume(measured_i: f64, measured_lra: f64, measured_tp: f64, thresh: f64) -> Volume {
        Volume {
            measured_i,
            measured_lra,
            measured_tp,
            measured_threshold: thresh,
            target_offset: 0.0,
            target_i: -16.0,
            target_tp: -1.5,
        }
    }

    #[test]
    fn loudnorm_uses_config_then_measured() {
        let v = volume(-14.5, 7.0, -1.0, -25.0);
        let params = loudnorm_params(
            &v,
            &LoudnormConfig {
                lra: Some(11.0),
                i: Some(-16.0),
                tp: Some(-1.5),
                offset: Some(0.0),
            },
        );
        // lra 取 max(11, 7) = 11；tp 取 min(-1.5, -1.0) = -1.5
        assert!(
            params.starts_with("LRA=11:I=-16:TP=-1.5:offset=0:"),
            "{params}"
        );
        assert!(params.contains("measured_I=-14.5"), "{params}");
        assert!(params.contains("measured_thresh=-25"), "{params}");
    }

    #[test]
    fn positive_measurements_are_folded_into_target() {
        let v = volume(2.0, 7.0, -1.0, 3.0);
        let params = loudnorm_params(&v, &LoudnormConfig::default());
        // target_i(-16) - measured_i(2) = -18，且 measured_I 归零
        assert!(params.contains("I=-18:"), "{params}");
        assert!(params.contains("measured_I=0"), "{params}");
        // 正的 threshold 同样归零
        assert!(params.contains("measured_thresh=0"), "{params}");
    }

    #[test]
    fn pgc_clips_map_to_skip_segments() {
        let clips = vec![
            ClipInfo {
                start: Some(0.0),
                end: Some(90.5),
                clip_type: Some("CLIP_TYPE_OP".to_owned()),
            },
            ClipInfo {
                start: Some(1300.0),
                end: Some(1400.0),
                clip_type: Some("CLIP_TYPE_ED".to_owned()),
            },
            ClipInfo {
                start: Some(10.0),
                end: Some(20.0),
                clip_type: None,
            },
            // 缺 end：整条丢弃，不影响其余三条
            ClipInfo {
                start: Some(5.0),
                end: None,
                clip_type: None,
            },
        ];
        let segments = pgc_clip_segments(&clips, Some(1_440_000));
        assert_eq!(segments.len(), 3);
        assert_eq!(segments[0].category, "intro");
        assert_eq!(segments[0].end_ms, 90_500);
        assert_eq!(segments[1].category, "outro");
        assert_eq!(segments[2].category, "sponsor");
        assert_eq!(segments[0].video_duration_ms, Some(1_440_000.0));
    }

    #[test]
    fn empty_measurement_is_detected() {
        assert!(!has_volume_measurement(&volume(0.0, 0.0, 0.0, 0.0)));
        assert!(has_volume_measurement(&volume(0.0, 0.0, -1.0, 0.0)));
    }
}
