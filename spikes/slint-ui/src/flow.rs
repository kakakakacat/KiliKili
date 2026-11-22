//! 富文本的两条路：Slint 1.19 自带的 `StyledText`，以及自己排版。
//!
//! ## 为什么两条都要做
//!
//! 评论正文被 [`bili_domain::reply_richtext::segment`] 切成段之后，宿主要
//! 把它们拼成一行会换行的混排内容。Flutter 那边是 `Text.rich` + `InlineSpan`
//! + `WidgetSpan`（表情图直接塞进文本流）。Slint 这边：
//!
//! - **`StyledText`（1.19 新增）** 能做行内颜色、下划线、链接、文本选择，
//!   换行由渲染器自己做。但它基于 CommonMark 的一个子集，**明确不支持
//!   图片**（`parse_interpolated` 把 `Image` 归进 unsupported 一档）。
//!   表情只能退化成 `[doge]` 这样的占位文字。
//! - **自己排版**能把表情画成图，代价是断行要自己算，而 **Slint 没有给
//!   应用层的文本测量 API**——所以下面这份字宽是**估算**的。
//!
//! 两条并排画出来，截图里能直接看出各自丢了什么。这比任何一句
//! 「Slint 的富文本够不够用」都说明问题。
//!
//! ## 估算字宽有多离谱，是这个 spike 的结论之一
//!
//! 估算偏窄会把下一段挤出列宽，偏宽会提前换行。截图里看得见。真要做，
//! Phase 2 得让渲染器把测量结果交出来（Slint 的 `Text` 自己知道
//! `preferred-width`，但那个值拿不到 Rust 这一侧的循环里来用）。

use bili_domain::reply_richtext::{ReplySegment, ReplySegmentKind};

/// 排好位置的一段。坐标相对于所在的那一列。
#[derive(Debug, Clone)]
pub struct Run {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub font_size: f32,
    /// 0xRRGGBB。
    pub color: u32,
    pub emote: bool,
    pub underline: bool,
}

#[derive(Debug, Default)]
pub struct Flow {
    pub runs: Vec<Run>,
    pub height: f32,
    pub lines: usize,
}

const TEXT: u32 = 0x00e6_e9ee;
const LINKISH: u32 = 0x008b_b3ff;
const SECONDARY: u32 = 0x006b_7480;

#[must_use]
pub const fn color_of(kind: ReplySegmentKind) -> u32 {
    match kind {
        ReplySegmentKind::Text => TEXT,
        ReplySegmentKind::Emote => SECONDARY,
        ReplySegmentKind::Mention
        | ReplySegmentKind::Topic
        | ReplySegmentKind::Link
        | ReplySegmentKind::Timestamp
        | ReplySegmentKind::Vote
        | ReplySegmentKind::Note => LINKISH,
    }
}

/// 把一条评论的分词结果排进宽 `max_width` 的一列。
///
/// `y0` 是这条评论在列里的起始纵坐标，调用方按返回的 `height` 往下累。
#[must_use]
pub fn layout(
    segments: &[ReplySegment],
    max_width: f32,
    font_size: f32,
    line_height: f32,
    y0: f32,
) -> Flow {
    let mut flow = Flow {
        height: line_height,
        lines: 1,
        ..Flow::default()
    };
    let mut x = 0.0_f32;
    let mut line = 0_usize;

    let newline = |x: &mut f32, line: &mut usize| {
        *x = 0.0;
        *line += 1;
    };

    for segment in segments {
        if segment.kind == ReplySegmentKind::Emote {
            // 表情按一个正方形占位，边长比行高略小——基线也是这么画的
            // （`size == 1` 乘一个基准边长）。
            let side = line_height * 0.95;
            if x + side > max_width {
                newline(&mut x, &mut line);
            }
            flow.runs.push(Run {
                text: segment.text.clone(),
                x,
                y: y0 + line as f32 * line_height,
                w: side,
                h: side,
                font_size,
                color: color_of(segment.kind),
                emote: true,
                underline: false,
            });
            x += side;
            continue;
        }

        for token in tokenize(&segment.text) {
            let width = token.chars().map(|c| advance(c, font_size)).sum::<f32>();
            // 行首的空白不往下带，否则换行处会出现一格缩进。
            if x <= 0.0 && token.trim().is_empty() {
                continue;
            }
            if x + width > max_width && x > 0.0 {
                newline(&mut x, &mut line);
                if token.trim().is_empty() {
                    continue;
                }
            }
            flow.runs.push(Run {
                text: token.clone(),
                x,
                y: y0 + line as f32 * line_height,
                w: width,
                h: line_height,
                font_size,
                color: color_of(segment.kind),
                emote: false,
                underline: matches!(
                    segment.kind,
                    ReplySegmentKind::Link | ReplySegmentKind::Note
                ),
            });
            x += width;
        }
    }

    flow.lines = line + 1;
    flow.height = flow.lines as f32 * line_height;
    flow
}

/// 断行单位：CJK 一字一断，拉丁按词断，空白自成一个 token。
fn tokenize(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut word = String::new();
    for ch in text.chars() {
        if is_cjk(ch) || ch.is_whitespace() {
            if !word.is_empty() {
                out.push(std::mem::take(&mut word));
            }
            out.push(ch.to_string());
        } else {
            word.push(ch);
        }
    }
    if !word.is_empty() {
        out.push(word);
    }
    out
}

const fn is_cjk(ch: char) -> bool {
    matches!(ch as u32,
        0x1100..=0x11FF | 0x2E80..=0x303F | 0x3040..=0x30FF | 0x3400..=0x4DBF
        | 0x4E00..=0x9FFF | 0xF900..=0xFAFF | 0xFF00..=0xFF60 | 0xFFE0..=0xFFE6)
}

/// 估算一个字符的推进宽度。
///
/// 没有调用真正的字体度量——Slint 不把文本测量暴露给应用层（见模块文档）。
/// 这些系数是按 15px 的无衬线中文字体目测出来的，**偏差本身就是本 spike
/// 要记下来的代价**。
fn advance(ch: char, font_size: f32) -> f32 {
    let em = font_size;
    if is_cjk(ch) {
        return em;
    }
    if (ch as u32) >= 0x1F000 {
        // emoji 在大多数桌面字体里是全宽，而且常常根本没有彩色字形。
        return em * 1.2;
    }
    match ch {
        ' ' => em * 0.28,
        'i' | 'j' | 'l' | 't' | 'f' | 'I' | '.' | ',' | ':' | ';' | '\'' | '!' | '|' => em * 0.3,
        'm' | 'w' | 'M' | 'W' => em * 0.82,
        'A'..='Z' | '0'..='9' => em * 0.58,
        _ => em * 0.5,
    }
}

/// 分词结果 → `StyledText` 吃的 markdown。
///
/// 表情**只剩占位文字**：CommonMark 的图片语法被 Slint 的解析器归为不支持
/// （`UnsupportedMarkdown("images")`）。这不是本函数偷懒，是那条路走不通，
/// 实验会把这条错误原样记下来。
#[must_use]
pub fn markdown(segments: &[ReplySegment]) -> String {
    let mut out = String::new();
    for segment in segments {
        let text = escape(&segment.text);
        match segment.kind {
            ReplySegmentKind::Text => out.push_str(&text),
            ReplySegmentKind::Emote => {
                out.push_str(&format!("<font color=\"#6b7480\">{text}</font>"));
            }
            ReplySegmentKind::Link | ReplySegmentKind::Note => {
                // 链接用 CommonMark 的行内链接：点击会走 `link-clicked`，
                // 跳哪由领域层给的 action/target 决定，这里只放一个标识。
                let target = if segment.target.is_empty() {
                    "#".to_owned()
                } else {
                    escape_url(&segment.target)
                };
                out.push_str(&format!("[{text}]({target})"));
            }
            _ => out.push_str(&format!("<font color=\"#8bb3ff\">{text}</font>")),
        }
    }
    out
}

/// 评论正文里什么字符都可能有。不转义的话，一条含 `*` 的评论会被当成
/// 强调标记，含 `<` 的会被当成 HTML 标签——后者在 Slint 这边直接是
/// 解析错误，整条评论画不出来。
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '&' => out.push_str("&amp;"),
            '*' | '_' | '[' | ']' | '`' | '\\' | '#' | '!' => {
                out.push('\\');
                out.push(ch);
            }
            '\n' => out.push(' '),
            _ => out.push(ch),
        }
    }
    out
}

fn escape_url(url: &str) -> String {
    url.replace(' ', "%20").replace(')', "%29")
}

#[cfg(test)]
mod tests {
    use bili_domain::reply_richtext::{ReplyRichTextInput, ReplyRichTextSettings, segment};

    use super::*;

    fn segments(message: &str) -> Vec<ReplySegment> {
        segment(
            &ReplyRichTextInput {
                message: message.to_owned(),
                ..ReplyRichTextInput::default()
            },
            &ReplyRichTextSettings::default(),
        )
    }

    #[test]
    fn long_text_wraps_within_the_column() {
        let segments = segments(&"中文".repeat(200));
        let flow = layout(&segments, 300.0, 15.0, 22.0, 0.0);
        assert!(flow.lines > 10, "两百字挤不进一行，必须换行");
        assert!(
            flow.runs.iter().all(|r| r.x + r.w <= 300.0 + 0.01),
            "有 run 超出了列宽"
        );
    }

    #[test]
    fn a_line_never_starts_with_a_space() {
        let segments = segments(&"word ".repeat(80));
        let flow = layout(&segments, 200.0, 15.0, 22.0, 0.0);
        let first_of_each_line: Vec<&Run> = flow.runs.iter().filter(|r| r.x.abs() < 0.01).collect();
        assert!(
            first_of_each_line.iter().all(|r| !r.text.trim().is_empty()),
            "行首出现了空白 token"
        );
    }

    #[test]
    fn markdown_escapes_what_would_otherwise_be_markup() {
        let out = markdown(&segments("a*b* <b>c</b> [d] #e"));
        assert!(!out.contains("<b>"), "HTML 标签没转义：{out}");
        assert!(out.contains(r"\*"), "强调标记没转义：{out}");
    }
}
