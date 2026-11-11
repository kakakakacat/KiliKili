//! 字幕转换：`SubtitleCue` → WebVTT / SRT。
//!
//! 基线是 `lib/utils/subtitle_format.dart`（从 `SubtitleUtils` 里原样搬出
//! 来的纯 dart 版本）。基线把这段放在 `compute` 里跑——一个长视频的字幕
//! 有几千条，它重到必须躲开 UI 线程。迁到 Rust 后由 FRB 的工作线程承担，
//! 比 isolate 还少一次消息拷贝。
//!
//! ## 为什么要自己写定点格式化
//!
//! VTT 的秒数走基线的 `toStringAsFixed(3)`，那是 ECMAScript 的
//! `Number.prototype.toFixed`：按 f64 的**精确值**取最近的三位小数，
//! **平局进位**（取较大的 n）。Rust 的 `{:.3}` 同样按精确值，但平局
//! **取偶**。两者只在精确平局时分岔，而那不是空想：平局要求精确值形如
//! 奇数/16（0.0625、0.1875……），这种值在以帧率算出来的时间戳里会出现。
//! 差异是 1 毫秒——没人看得出来，但 golden 会红，而红的时候很难想到是
//! 舍入模式。所以这里按位实现，见 [`to_fixed_3`]。
//!
//! SRT 的毫秒走的是 `((seconds - s) * 1000).round()`，Dart 的
//! `num.round()` 与 Rust 的 `f64::round` 都是四舍五入远离零，逐位的浮点
//! 运算也相同，因此直接算即可。两种格式的舍入方式不一致是基线的既有
//! 行为，不要「顺手统一」。

use bili_domain::subtitle::SubtitleCue;

/// 转成 WebVTT。空列表得到的是只有文件头的 `"WEBVTT\n\n"`，与基线一致。
#[must_use]
pub fn to_vtt(cues: &[SubtitleCue]) -> String {
    let mut out = String::from("WEBVTT\n\n");
    for (index, cue) in cues.iter().enumerate() {
        if index > 0 {
            out.push_str("\n\n");
        }
        out.push_str(&vtt_timecode(cue.from));
        out.push_str(" --> ");
        out.push_str(&vtt_timecode(cue.to));
        out.push('\n');
        out.push_str(trim_like_dart(&cue.content));
    }
    out
}

/// 转成 SRT。空列表得到空串，与基线一致。
#[must_use]
pub fn to_srt(cues: &[SubtitleCue]) -> String {
    let mut out = String::new();
    for (index, cue) in cues.iter().enumerate() {
        if index > 0 {
            out.push_str("\n\n");
        }
        out.push_str(&(index + 1).to_string());
        out.push('\n');
        out.push_str(&srt_timecode(cue.from));
        out.push_str(" --> ");
        out.push_str(&srt_timecode(cue.to));
        out.push('\n');
        out.push_str(trim_like_dart(&cue.content));
    }
    out
}

/// `HH:MM:SS.mmm`。
#[must_use]
pub fn vtt_timecode(seconds: f64) -> String {
    let h = (seconds / 3600.0).trunc();
    let rest = seconds.rem_euclid(3600.0);
    let m = (rest / 60.0).trunc();
    let s = rest.rem_euclid(60.0);
    let sms = pad_left(&to_fixed_3(s), 6);
    format!(
        "{}:{}:{sms}",
        pad_left(&int_string(h), 2),
        pad_left(&int_string(m), 2)
    )
}

/// `HH:MM:SS,mmm`。
#[must_use]
pub fn srt_timecode(seconds: f64) -> String {
    let h = (seconds / 3600.0).trunc();
    let rest = seconds.rem_euclid(3600.0);
    let m = (rest / 60.0).trunc();
    let s = rest.rem_euclid(60.0);
    let whole = s.trunc();
    // 基线：((seconds - s) * 1000).round()。可能得到 1000——那时基线输出
    // 四位毫秒，这里照搬，不做夹取。
    let ms = ((s - whole) * 1000.0).round();
    format!(
        "{}:{}:{},{}",
        pad_left(&int_string(h), 2),
        pad_left(&int_string(m), 2),
        pad_left(&int_string(whole), 2),
        pad_left(&int_string(ms), 3)
    )
}

/// 按 Dart `String.trim` 的定义去掉首尾空白。
///
/// Dart 的空白是 Unicode `White_Space` **加上** U+FEFF（BOM）；Rust 的
/// `char::is_whitespace` 只有前者。字幕文件以 BOM 开头并不稀奇，漏掉它
/// 会让第一条字幕前面多一个不可见字符。
fn trim_like_dart(value: &str) -> &str {
    value.trim_matches(|c: char| c.is_whitespace() || c == '\u{FEFF}')
}

/// 把一个已经是整数的 f64 转成十进制字符串（不带 `.0`）。
fn int_string(value: f64) -> String {
    format!("{value:.0}")
}

/// 左侧补 `0` 到指定宽度；已够长则原样返回（同 Dart 的 `padLeft`）。
fn pad_left(value: &str, width: usize) -> String {
    if value.chars().count() >= width {
        return value.to_owned();
    }
    let mut out = "0".repeat(width - value.chars().count());
    out.push_str(value);
    out
}

/// Dart `num.toStringAsFixed(3)` 的等价实现：按**精确值**取三位小数，
/// 平局向上（绝对值变大）。
///
/// 做法是先拿到精确的十进制展开再按位进位。f64 的精确值一定是有限小数
/// （m·2⁻ᵏ），80 位足够覆盖字幕时间这个量级；Rust 的 `{:.N}` 本身是按
/// 精确值正确舍入的，因此第 4 位之后不会被「制造」出进位。
#[must_use]
pub fn to_fixed_3(value: f64) -> String {
    if !value.is_finite() {
        // 基线此时得到 "NaN"/"Infinity"，字幕里不会出现；保持可读即可。
        return format!("{value}");
    }
    let negative = value.is_sign_negative() && value != 0.0;
    let exact = format!("{:.80}", value.abs());
    let (int_part, frac_part) = exact.split_once('.').unwrap_or((exact.as_str(), ""));

    let mut digits: Vec<u8> = int_part.bytes().map(|b| b - b'0').collect();
    let keep: Vec<u8> = frac_part
        .bytes()
        .take(3)
        .map(|b| b - b'0')
        .chain(std::iter::repeat(0))
        .take(3)
        .collect();
    digits.extend_from_slice(&keep);

    // 平局向上：只看被丢掉的第一位即可（≥5 进位，<5 舍去）。
    let round_up = frac_part.as_bytes().get(3).is_some_and(|b| *b >= b'5');
    if round_up {
        let mut index = digits.len();
        loop {
            if index == 0 {
                digits.insert(0, 1);
                break;
            }
            index -= 1;
            if digits[index] == 9 {
                digits[index] = 0;
            } else {
                digits[index] += 1;
                break;
            }
        }
    }

    let split = digits.len() - 3;
    let mut out = String::new();
    if negative {
        out.push('-');
    }
    for d in &digits[..split] {
        out.push(char::from(b'0' + d));
    }
    out.push('.');
    for d in &digits[split..] {
        out.push(char::from(b'0' + d));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{SubtitleCue, srt_timecode, to_fixed_3, to_srt, to_vtt, vtt_timecode};

    fn cue(from: f64, to: f64, content: &str) -> SubtitleCue {
        SubtitleCue {
            from,
            to,
            content: content.to_owned(),
        }
    }

    #[test]
    fn fixed_three_rounds_half_up_like_dart() {
        // 0.0625 是精确平局：Dart 的 toStringAsFixed 进位成 0.063，
        // Rust 的 {:.3} 取偶会得到 0.062。
        assert_eq!(to_fixed_3(0.0625), "0.063");
        assert_eq!(
            format!("{:.3}", 0.0625),
            "0.062",
            "这正是不能直接用它的原因"
        );
        assert_eq!(to_fixed_3(0.1875), "0.188");
    }

    #[test]
    fn fixed_three_keeps_three_digits() {
        assert_eq!(to_fixed_3(0.0), "0.000");
        assert_eq!(to_fixed_3(1.0), "1.000");
        assert_eq!(to_fixed_3(12.5), "12.500");
        assert_eq!(to_fixed_3(59.9999), "60.000", "进位要跨到整数位");
        assert_eq!(to_fixed_3(0.9995), "1.000");
    }

    #[test]
    fn fixed_three_does_not_round_up_below_half() {
        // 看的是**精确值**，不是写出来的那串十进制。1.0005 的 f64 精确值
        // 是 1.00049999...，该舍去；0.0015 的是 0.00150000...003，该进位。
        // 下面每一行都用 Dart 的 toStringAsFixed(3) 核对过。
        assert_eq!(to_fixed_3(1.0005), "1.000");
        assert_eq!(to_fixed_3(9.9995), "9.999");
        assert_eq!(to_fixed_3(7.0005), "7.000");
        assert_eq!(to_fixed_3(0.0015), "0.002");
        assert_eq!(to_fixed_3(0.0005), "0.001");
        assert_eq!(to_fixed_3(1234.5675), "1234.568");
        assert_eq!(to_fixed_3(0.00025), "0.000");
    }

    #[test]
    fn vtt_timecode_pads_like_the_baseline() {
        assert_eq!(vtt_timecode(0.0), "00:00:00.000");
        assert_eq!(vtt_timecode(5.5), "00:00:05.500");
        assert_eq!(vtt_timecode(65.25), "00:01:05.250");
        assert_eq!(vtt_timecode(3661.125), "01:01:01.125");
        assert_eq!(vtt_timecode(36000.0), "10:00:00.000");
    }

    #[test]
    fn srt_timecode_uses_a_comma_and_its_own_rounding() {
        assert_eq!(srt_timecode(0.0), "00:00:00,000");
        assert_eq!(srt_timecode(5.5), "00:00:05,500");
        assert_eq!(srt_timecode(3661.125), "01:01:01,125");
    }

    #[test]
    fn srt_milliseconds_can_reach_one_thousand_like_the_baseline() {
        // ((1.9996 - 1) * 1000).round() == 1000，基线会输出四位毫秒。
        // 这是基线的既有行为，不是我们该在这里修的。
        assert_eq!(srt_timecode(1.9996), "00:00:01,1000");
    }

    #[test]
    fn vtt_has_a_header_and_blank_line_separated_entries() {
        let text = to_vtt(&[cue(0.0, 1.0, "第一句"), cue(1.0, 2.0, "第二句")]);
        assert_eq!(
            text,
            "WEBVTT\n\n00:00:00.000 --> 00:00:01.000\n第一句\n\n00:00:01.000 --> 00:00:02.000\n第二句"
        );
    }

    #[test]
    fn srt_entries_are_numbered_from_one() {
        let text = to_srt(&[cue(0.0, 1.0, "第一句"), cue(1.0, 2.0, "第二句")]);
        assert_eq!(
            text,
            "1\n00:00:00,000 --> 00:00:01,000\n第一句\n\n2\n00:00:01,000 --> 00:00:02,000\n第二句"
        );
    }

    #[test]
    fn empty_input_matches_the_baseline() {
        assert_eq!(to_vtt(&[]), "WEBVTT\n\n");
        assert_eq!(to_srt(&[]), "");
    }

    #[test]
    fn content_is_trimmed_including_the_bom() {
        let text = to_vtt(&[cue(0.0, 1.0, "\u{FEFF}  有空白  \n")]);
        assert!(text.ends_with("有空白"), "{text}");
    }
}
