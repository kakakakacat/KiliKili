//! 搜索结果里的富文本与时长解析。
//!
//! 这三个函数是**协议的一部分**（ADR-011 决策二）：接口用
//! `<em class="keyword">…</em>` 标命中词、用 `"12:34"` 给时长、用
//! `<span>…</span>` 包分区名。怎么拆是对协议的理解，怎么画才是界面。
//!
//! ## 为什么手写扫描而不是用 `regex`
//!
//! 基线的三个正则（`Em._exp`、`Em._htmlRegExp`、`DurationUtils._splitRegex`）
//! 都极简单，手写扫描可以**逐字符对齐 Dart 的语义**，包括它那些不显眼的
//! 边界行为（见下）。引入 `regex` 反而要先说服自己两边的正则方言一致。
//!
//! ## 刻意保留的基线行为
//!
//! 1. `<[^>]*>([^<]*)</[^>]*>` 要求成对标签，且内层不含 `<`。单个
//!    `<em>` 没有闭合标签时**整体不匹配**，那段文本原样作为非命中段
//!    留下——基线如此，不要「顺手修好」。
//! 2. 空串守卫只在非命中段上。基线的 `if (str != '')` 写在 `onNonMatch`
//!    里，`onMatch` 没有，于是空的**命中**段会产出而空的非命中段不会。
//! 3. 匹配失败时正则从**下一个字符**继续试，而不是跳过整个标签。
//! 4. `&#x` 实体只认 2–4 位十六进制。位数不符时原样保留。

use bili_domain::search::{TitleSpan, spans_to_plain};

/// 把带 `<em>` 标记的标题拆成段落。对应基线 `Em.regTitle`。
///
/// 返回的段落已做 HTML 实体解码。空输入返回空列表（基线同样不产生
/// 空段落：`onNonMatch` 里有 `if (str != '')`）。
#[must_use]
pub fn parse_title_spans(origin: &str) -> Vec<TitleSpan> {
    let mut out = Vec::new();
    let mut cursor = 0usize;

    while cursor < origin.len() {
        match find_tag_pair(origin, cursor) {
            Some((start, end, inner)) => {
                if start > cursor {
                    push_non_empty(&mut out, &origin[cursor..start]);
                }
                // 命中段**即使为空也要产出**。基线的空串守卫
                // （`if (str != '')`）只写在 `onNonMatch` 里，`onMatch`
                // 没有——所以 `a<em></em>b` 在基线下是三段，中间那段是
                // 空的命中段。看着像疏漏，但它是现行行为，Dart 侧的
                // parity 测试会钉住这一条。
                out.push(TitleSpan {
                    is_em: true,
                    text: decode_entities(inner),
                });
                cursor = end;
            }
            None => break,
        }
    }

    if cursor < origin.len() {
        push_non_empty(&mut out, &origin[cursor..]);
    }
    out
}

/// 富文本标题的纯文本形式。
#[must_use]
pub fn plain_title(origin: &str) -> String {
    spans_to_plain(&parse_title_spans(origin))
}

/// 剥掉分区名外层的标签。对应基线 `Em.regCate`。
///
/// 只取**第一个**成对标签的内容；没有匹配时原样返回。注意基线这里
/// **不**做实体解码，保持一致。
#[must_use]
pub fn strip_tags_first(origin: &str) -> String {
    find_tag_pair(origin, 0).map_or_else(|| origin.to_owned(), |(_, _, inner)| inner.to_owned())
}

/// 解码 HTML 实体。对应基线 `Em.parseHtml`。
///
/// 只认基线那张表：`lt` `gt` `quot` `apos` `nbsp` `amp` 与
/// `&#x` + 2–4 位十六进制。表外的实体原样保留。
///
/// `&nbsp;` 解成**普通空格**（U+0020）而不是 U+00A0——基线如此。
#[must_use]
pub fn decode_entities(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(input.len());
    let mut i = 0usize;

    while i < bytes.len() {
        if bytes[i] != b'&' {
            // 非 ASCII 字节原样搬运：只在 `&` 处才需要理解结构。
            let ch_len = utf8_len(bytes[i]);
            let end = (i + ch_len).min(bytes.len());
            out.push_str(&input[i..end]);
            i = end;
            continue;
        }

        match decode_one(input, i) {
            Some((decoded, next)) => {
                out.push_str(&decoded);
                i = next;
            }
            None => {
                out.push('&');
                i += 1;
            }
        }
    }
    out
}

/// 把 `"12:34"` / `"1:02:03"` 折算成秒。对应基线
/// `DurationUtils.parseDuration`。
///
/// 分隔符同时认半角 `:` 与全角 `：`（基线正则 `[:：]`）。
///
/// 基线在这里用的是 `int.parse`，**任何一段不是整数就抛异常**，
/// 调用点会把整次搜索判为失败。这里改成返回 0：一条时长读不出来
/// 不该让整页结果消失，与 ADR-011 决策三「单条残缺不拖垮整批」同向。
#[must_use]
pub fn parse_duration_secs(data: &str) -> i64 {
    if data.is_empty() {
        return 0;
    }
    let mut total: i64 = 0;
    let mut unit: i64 = 1;

    // 基线：split 后 reversed，第 i 段乘 60^i。即最右边是秒。
    for part in data.rsplit([':', '：']) {
        let Ok(value) = part.trim().parse::<i64>() else {
            return 0;
        };
        total += value.saturating_mul(unit);
        unit = unit.saturating_mul(60);
    }
    total
}

// ------------------------------------------------------------------ 内部

/// 产出一个**非命中**段。空串不入列表，对应基线 `onNonMatch` 里的
/// `if (str != '')`。命中段没有这个守卫，见 [`parse_title_spans`]。
fn push_non_empty(out: &mut Vec<TitleSpan>, raw: &str) {
    if raw.is_empty() {
        return;
    }
    out.push(TitleSpan {
        is_em: false,
        text: decode_entities(raw),
    });
}

/// 在 `from` 之后找第一个 `<a>b</c>` 形态的成对标签。
///
/// 返回 `(匹配起点, 匹配终点, 内层文本)`。语义对齐基线正则
/// `<[^>]*>([^<]*)</[^>]*>`：
/// - 开标签：`<` 到**第一个** `>`；
/// - 内层：到**第一个** `<` 为止，不可含 `<`；
/// - 闭标签：必须以 `</` 开头，再到第一个 `>`。
///
/// 任一步不成立时，从**下一个字节**重试（正则引擎的行为），而不是跳过
/// 整个开标签。
fn find_tag_pair(s: &str, from: usize) -> Option<(usize, usize, &str)> {
    let bytes = s.as_bytes();
    let mut start = from;

    while start < bytes.len() {
        let open = start + s[start..].find('<')?;

        if let Some(found) = try_match_at(s, open) {
            return Some(found);
        }
        // 正则在此处失败后只前进一个字符。`<` 是 ASCII，+1 不会切断
        // UTF-8 序列。
        start = open + 1;
    }
    None
}

fn try_match_at(s: &str, open: usize) -> Option<(usize, usize, &str)> {
    // `<[^>]*>`
    let open_tag_end = open + s[open..].find('>')? + 1;

    // `([^<]*)`。没有后续 `<` 就没有闭标签，匹配失败。
    let inner_end = open_tag_end + s[open_tag_end..].find('<')?;
    let inner = &s[open_tag_end..inner_end];

    // `</[^>]*>`
    if !s[inner_end..].starts_with("</") {
        return None;
    }
    let close_tag_end = inner_end + s[inner_end..].find('>')? + 1;

    Some((open, close_tag_end, inner))
}

/// 解码 `at` 处的一个实体。返回 `(解码结果, 下一个位置)`。
fn decode_one(s: &str, at: usize) -> Option<(String, usize)> {
    let bytes = s.as_bytes();
    // 基线正则里最长的是 `&#xFFFF;`，8 个字节。实体名全是 ASCII，所以
    // 按字节找 `;` 不会切断多字节字符——但 `&` 后面紧跟非 ASCII 时
    // `s.get(..n)` 会落在非字符边界，因此这里不切片，只扫字节。
    let limit = (at + 9).min(bytes.len());
    let semi = (at + 1..limit).find(|&i| bytes[i] == b';')?;
    let name = &s[at + 1..semi];
    let next = semi + 1;

    let decoded = match name {
        "lt" => "<".to_owned(),
        "gt" => ">".to_owned(),
        "quot" => "\"".to_owned(),
        "apos" => "'".to_owned(),
        // 基线解成普通空格，不是 U+00A0。
        "nbsp" => " ".to_owned(),
        "amp" => "&".to_owned(),
        // 基线正则只写了小写 `#x`，大写 `#X` 不匹配。保持一致。
        hex if hex.starts_with("#x") => {
            let digits = &hex[2..];
            // 基线正则限定 2–4 位；位数不符时整个实体原样保留。
            if !(2..=4).contains(&digits.len()) || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
                return None;
            }
            let code = u32::from_str_radix(digits, 16).ok()?;
            // Dart 的 String 是 UTF-16，`fromCharCode` 接受孤立代理项；
            // Rust 的 char 不能表示它们。这种输入原样保留，不是静默换成
            // 替换字符——那会让 parity 测试看起来通过而实际已经不同。
            char::from_u32(code)?.to_string()
        }
        _ => return None,
    };
    Some((decoded, next))
}

/// UTF-8 首字节对应的序列长度。
const fn utf8_len(first: u8) -> usize {
    match first {
        0x00..=0x7F => 1,
        0xC0..=0xDF => 2,
        0xE0..=0xEF => 3,
        _ => 4,
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn title_splits_around_em_markers() {
        let spans = parse_title_spans(r#"前<em class="keyword">命中</em>后"#);
        assert_eq!(spans.len(), 3);
        assert_eq!((spans[0].is_em, spans[0].text.as_str()), (false, "前"));
        assert_eq!((spans[1].is_em, spans[1].text.as_str()), (true, "命中"));
        assert_eq!((spans[2].is_em, spans[2].text.as_str()), (false, "后"));
    }

    #[test]
    fn title_without_markup_is_a_single_span() {
        let spans = parse_title_spans("普通标题");
        assert_eq!(spans.len(), 1);
        assert!(!spans[0].is_em);
        assert_eq!(spans[0].text, "普通标题");
    }

    #[test]
    fn empty_title_yields_no_spans() {
        // 基线 onNonMatch 里有 `if (str != '')`，不产生空段落。
        assert!(parse_title_spans("").is_empty());
    }

    #[test]
    fn unclosed_tag_is_left_as_plain_text() {
        // 基线正则要求成对标签。`<em>` 没有闭合时整体不匹配，这段文本
        // 原样留下——刻意不「修好」。
        let spans = parse_title_spans("前<em>没有闭合");
        assert_eq!(spans.len(), 1);
        assert!(!spans[0].is_em);
        assert_eq!(spans[0].text, "前<em>没有闭合");
    }

    #[test]
    fn consecutive_markers_do_not_emit_empty_spans() {
        let spans = parse_title_spans("<em>a</em><em>b</em>");
        assert_eq!(spans.len(), 2);
        assert!(spans.iter().all(|s| s.is_em));
        assert_eq!(spans[0].text, "a");
        assert_eq!(spans[1].text, "b");
    }

    #[test]
    fn empty_em_span_is_emitted_but_empty_plain_span_is_not() {
        // 基线的空串守卫只在 `onNonMatch` 里，`onMatch` 没有——所以空的
        // **命中**段会产出，空的非命中段不会。看着像疏漏，但这是现行
        // 行为，Dart 侧 parity 测试钉住了它。
        let spans = parse_title_spans("a<em></em>b");
        let got: Vec<(bool, &str)> = spans.iter().map(|s| (s.is_em, s.text.as_str())).collect();
        assert_eq!(got, [(false, "a"), (true, ""), (false, "b")]);
    }

    #[test]
    fn entities_inside_spans_are_decoded() {
        let spans = parse_title_spans("<em>a&amp;b</em>&lt;c&gt;");
        assert_eq!(spans[0].text, "a&b");
        assert_eq!(spans[1].text, "<c>");
    }

    #[test]
    fn plain_title_joins_every_span() {
        assert_eq!(plain_title(r#"前<em class="x">中</em>后"#), "前中后");
    }

    #[test]
    fn hex_entities_decode_within_two_to_four_digits() {
        assert_eq!(decode_entities("&#x41;"), "A");
        assert_eq!(decode_entities("&#x4E2D;"), "中");
        // 1 位与 5 位都不符合基线正则，原样保留。
        assert_eq!(decode_entities("&#x4;"), "&#x4;");
        assert_eq!(decode_entities("&#x41414;"), "&#x41414;");
    }

    #[test]
    fn unknown_entities_are_left_alone() {
        assert_eq!(decode_entities("&copy;"), "&copy;");
        assert_eq!(decode_entities("a & b"), "a & b");
    }

    #[test]
    fn nbsp_decodes_to_an_ascii_space() {
        // 基线是 U+0020，不是 U+00A0。换成后者会让排版悄悄变化。
        assert_eq!(decode_entities("a&nbsp;b"), "a b");
        assert_eq!(decode_entities("a&nbsp;b").as_bytes()[1], b' ');
    }

    #[test]
    fn lone_surrogate_is_kept_verbatim() {
        // Dart 能产出孤立代理项，Rust 的 char 不能。原样保留比静默换成
        // U+FFFD 诚实：parity 测试会看见差异而不是被瞒过去。
        assert_eq!(decode_entities("&#xD800;"), "&#xD800;");
    }

    #[test]
    fn cate_name_keeps_only_the_first_tag_content() {
        assert_eq!(strip_tags_first("<span>网游</span>"), "网游");
        assert_eq!(strip_tags_first("没有标签"), "没有标签");
    }

    #[test]
    fn duration_parses_minutes_and_hours() {
        // 基线：split 后 reversed，第 i 段乘 60^i。
        assert_eq!(parse_duration_secs("12:34"), 754);
        assert_eq!(parse_duration_secs("1:02:03"), 3723);
        assert_eq!(parse_duration_secs("45"), 45);
    }

    #[test]
    fn duration_accepts_the_fullwidth_colon() {
        // 基线正则是 `[:：]`，两种冒号都认。
        assert_eq!(parse_duration_secs("12：34"), 754);
    }

    #[test]
    fn duration_of_unparsable_input_is_zero_not_a_panic() {
        // 基线此处 int.parse 会抛异常，整次搜索被判失败。一条时长读不出
        // 来不该让整页结果消失。
        assert_eq!(parse_duration_secs(""), 0);
        assert_eq!(parse_duration_secs("直播中"), 0);
        assert_eq!(parse_duration_secs("12:ab"), 0);
    }
}
