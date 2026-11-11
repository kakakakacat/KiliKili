//! `DmViewReply` 解码：目前只取字幕轨。
//!
//! ## 为什么只解一个子树
//!
//! `DmViewReply` 有 25 个字段：弹幕开关、蒙版、字幕、AI 过滤配置、
//! 播放器配置、发送框样式、表情包、活动面板、QoE……基线里**只有一个
//! 有消费方**——`lib/grpc/dm.dart` 的 `dmViewToSubtitle` 取 `subtitle`，
//! 用于视频详情页与下载时的字幕列表。其余子树在整个 app 里没有一处读取。
//!
//! 这与弹幕分段（`crate::danmaku`）解全部标量字段的取舍不矛盾，因为
//! 两者「宽度」的代价不同：那边是一条弹幕的二十来个标量，多解几个是
//! 几行映射，而且 shadow 比对的覆盖面确实因此变宽；这边每多一个子树就
//! 是一组嵌套消息、一套实体、一份 fixture，而它们没有读者，比对出差异
//! 也无从判断对错。需要时再加，并在那时写明谁要用。
//!
//! 未解的字段按 protobuf 的惯例跳过——这正是 protobuf 对未知字段的
//! 定义，不是偷工减料。
//!
//! 入参同 [`crate::danmaku`]：**解帧并解压之后**的 protobuf 字节。

use bili_domain::error::{DomainError, ErrorKind};
use bili_domain::subtitle::{Subtitle, sort_like_baseline};

use crate::protobuf::{Reader, WireType};

/// `DmViewReply` 里本模块关心的字段号。
mod reply_field {
    /// `VideoSubtitle subtitle = 3`
    pub(super) const SUBTITLE: u32 = 3;
}

/// `VideoSubtitle` 的字段号。
mod video_subtitle_field {
    /// `repeated SubtitleItem subtitles = 3`
    pub(super) const SUBTITLES: u32 = 3;
}

/// `SubtitleItem` 的字段号。
mod item_field {
    pub(super) const ID: u32 = 1;
    pub(super) const LAN: u32 = 3;
    pub(super) const LAN_DOC: u32 = 4;
    pub(super) const SUBTITLE_URL: u32 = 5;
    pub(super) const TYPE: u32 = 7;
}

/// `SubtitleType.AI` 的线上数值。
const SUBTITLE_TYPE_AI: i32 = 1;

/// 解码 `DmViewReply`，取出按基线顺序排好的字幕轨。
///
/// 返回空 `Vec` 表示这个视频没有字幕——基线在 `hasSubtitle()` 为假和
/// 列表为空两种情况下都返回 null，两者在调用方是同一种处理。
///
/// # Errors
/// 字节不是合法 protobuf、字符串不是合法 UTF-8、或已知字段的 wire type
/// 与协议不符时返回 `Protocol`。**不做**部分成功：少一条字幕轨的表现是
/// 「这个视频没有英文字幕」，没有任何提示，比整条降级回 Dart 难查得多。
pub fn parse_subtitles(bytes: &[u8]) -> Result<Vec<Subtitle>, DomainError> {
    let mut reader = Reader::new(bytes);
    let mut subtitles = Vec::new();

    while !reader.is_empty() {
        let tag = reader.read_tag()?;
        match (tag.field, tag.wire) {
            // 单数的消息字段出现多次时，proto3 的语义是**合并**而非覆盖
            // （protobuf.dart 的 mergeFromBuffer 同样如此），因此这里是
            // 继续往同一个 Vec 里追加，而不是清空重来。
            (reply_field::SUBTITLE, WireType::LengthDelimited) => {
                parse_video_subtitle(reader.read_bytes()?, &mut subtitles)?;
            }
            (reply_field::SUBTITLE, wire) => {
                return Err(DomainError::new(
                    ErrorKind::Protocol,
                    format!("DmViewReply.subtitle 的 wire type 变成了 {wire:?}"),
                ));
            }
            // 其余 24 个字段：没有消费方（见模块文档）。
            _ => reader.skip(tag.wire)?,
        }
    }

    sort_like_baseline(&mut subtitles);
    Ok(subtitles)
}

/// 解码 `VideoSubtitle`，把它的字幕轨追加到 `out`。
fn parse_video_subtitle(bytes: &[u8], out: &mut Vec<Subtitle>) -> Result<(), DomainError> {
    let mut reader = Reader::new(bytes);
    while !reader.is_empty() {
        let tag = reader.read_tag()?;
        match (tag.field, tag.wire) {
            (video_subtitle_field::SUBTITLES, WireType::LengthDelimited) => {
                out.push(parse_item(reader.read_bytes()?)?);
            }
            (video_subtitle_field::SUBTITLES, wire) => {
                return Err(DomainError::new(
                    ErrorKind::Protocol,
                    format!("VideoSubtitle.subtitles 的 wire type 变成了 {wire:?}"),
                ));
            }
            // lan / lan_doc 是整个字幕集合的语言标记，基线不读。
            _ => reader.skip(tag.wire)?,
        }
    }
    Ok(())
}

/// 解码一条 `SubtitleItem`。
fn parse_item(bytes: &[u8]) -> Result<Subtitle, DomainError> {
    let mut reader = Reader::new(bytes);
    let mut subtitle = Subtitle {
        id: 0,
        lan: String::new(),
        lan_doc: String::new(),
        subtitle_url: String::new(),
        subtitle_type: 0,
        is_ai: false,
    };

    while !reader.is_empty() {
        let tag = reader.read_tag()?;
        match (tag.field, tag.wire) {
            (item_field::ID, WireType::Varint) => subtitle.id = reader.read_int64()?,
            (item_field::LAN, WireType::LengthDelimited) => subtitle.lan = reader.read_string()?,
            (item_field::LAN_DOC, WireType::LengthDelimited) => {
                subtitle.lan_doc = reader.read_string()?;
            }
            (item_field::SUBTITLE_URL, WireType::LengthDelimited) => {
                subtitle.subtitle_url = reader.read_string()?;
            }
            (item_field::TYPE, WireType::Varint) => {
                subtitle.subtitle_type = reader.read_int32()?;
            }
            // **本模块映射的**字段换了 wire type：协议真的变了，不能当
            // 未知字段跳过。不在这张表里的已知字段（id_str、author……）
            // 我们本来就不读，它们变成什么形状都与此处无关——按未知字段
            // 跳过即可，否则上游给 id_str 换个类型就会让字幕整条解不出来。
            (field, wire) if is_mapped_item_field(field) => {
                return Err(DomainError::new(
                    ErrorKind::Protocol,
                    format!("SubtitleItem 字段 {field} 的 wire type 变成了 {wire:?}"),
                ));
            }
            // id_str / author / lan_doc_brief / ai_type / ai_status / role：
            // 基线的映射不读它们。
            _ => reader.skip(tag.wire)?,
        }
    }

    // 基线按 `type == SubtitleType.AI` 判定，而不是看 `lan` 是否以 ai 开头
    // （后者在 `_setSubtitle` 的偏好选择里另有用处，不是同一件事）。
    subtitle.is_ai = subtitle.subtitle_type == SUBTITLE_TYPE_AI;
    Ok(subtitle)
}

/// 该字段号是否是本模块会映射的 `SubtitleItem` 字段。
const fn is_mapped_item_field(field: u32) -> bool {
    matches!(
        field,
        item_field::ID
            | item_field::LAN
            | item_field::LAN_DOC
            | item_field::SUBTITLE_URL
            | item_field::TYPE
    )
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::{parse_subtitles, reply_field, video_subtitle_field};

    fn varint(mut value: u64) -> Vec<u8> {
        let mut out = Vec::new();
        loop {
            let byte = u8::try_from(value & 0x7f).expect("低 7 位必然落在 u8 内");
            value >>= 7;
            if value == 0 {
                out.push(byte);
                return out;
            }
            out.push(byte | 0x80);
        }
    }

    fn tag(field: u32, wire: u64) -> Vec<u8> {
        varint((u64::from(field) << 3) | wire)
    }

    fn varint_field(field: u32, value: u64) -> Vec<u8> {
        let mut out = tag(field, 0);
        out.extend(varint(value));
        out
    }

    fn string_field(field: u32, value: &str) -> Vec<u8> {
        let mut out = tag(field, 2);
        out.extend(varint(value.len() as u64));
        out.extend(value.as_bytes());
        out
    }

    fn message_field(field: u32, body: &[u8]) -> Vec<u8> {
        let mut out = tag(field, 2);
        out.extend(varint(body.len() as u64));
        out.extend(body);
        out
    }

    fn item(id: u64, lan: &str, lan_doc: &str, url: &str, kind: u64) -> Vec<u8> {
        let mut body = varint_field(1, id);
        body.extend(string_field(3, lan));
        body.extend(string_field(4, lan_doc));
        body.extend(string_field(5, url));
        if kind != 0 {
            body.extend(varint_field(7, kind));
        }
        body
    }

    fn reply(items: &[Vec<u8>]) -> Vec<u8> {
        let mut subtitle = Vec::new();
        for i in items {
            subtitle.extend(message_field(video_subtitle_field::SUBTITLES, i));
        }
        message_field(reply_field::SUBTITLE, &subtitle)
    }

    #[test]
    fn empty_input_has_no_subtitles() {
        assert!(parse_subtitles(&[]).expect("空响应合法").is_empty());
    }

    #[test]
    fn reply_without_subtitle_field_has_no_subtitles() {
        // closed = true，别的什么都没有
        let bytes = varint_field(1, 1);
        assert!(parse_subtitles(&bytes).expect("合法").is_empty());
    }

    #[test]
    fn decodes_the_fields_the_baseline_maps() {
        let bytes = reply(&[item(123, "zh-CN", "中文（中国）", "https://x/1.json", 0)]);
        let subs = parse_subtitles(&bytes).expect("合法");
        assert_eq!(subs.len(), 1);
        assert_eq!(subs[0].id, 123);
        assert_eq!(subs[0].lan, "zh-CN");
        assert_eq!(subs[0].lan_doc, "中文（中国）");
        assert_eq!(subs[0].subtitle_url, "https://x/1.json");
        assert_eq!(subs[0].subtitle_type, 0);
        assert!(!subs[0].is_ai);
    }

    #[test]
    fn ai_flag_comes_from_the_type_field() {
        let bytes = reply(&[item(1, "ai-zh", "中文（自动）", "https://x/1.json", 1)]);
        let subs = parse_subtitles(&bytes).expect("合法");
        assert_eq!(subs[0].subtitle_type, 1);
        assert!(subs[0].is_ai);
    }

    #[test]
    fn result_is_sorted_like_the_baseline() {
        let bytes = reply(&[
            item(1, "en-US", "English", "u", 0),
            item(2, "ai-zh", "中文（自动）", "u", 1),
            item(3, "zh-CN", "中文", "u", 0),
        ]);
        let ids: Vec<_> = parse_subtitles(&bytes)
            .expect("合法")
            .iter()
            .map(|s| s.id)
            .collect();
        assert_eq!(ids, [3, 2, 1]);
    }

    #[test]
    fn unknown_subtrees_are_skipped() {
        let mut bytes = message_field(2, &varint_field(1, 1)); // mask
        bytes.extend(reply(&[item(1, "zh-CN", "中文", "u", 0)]));
        bytes.extend(message_field(6, &varint_field(1, 1))); // player_config
        bytes.extend(string_field(23, "{\"a\":1}")); // kv
        assert_eq!(parse_subtitles(&bytes).expect("合法").len(), 1);
    }

    #[test]
    fn repeated_subtitle_messages_merge() {
        // proto3：单数消息字段出现两次是合并，不是覆盖。
        let mut bytes = reply(&[item(1, "zh-CN", "中文", "u", 0)]);
        bytes.extend(reply(&[item(2, "zh-Hant", "繁體", "u", 0)]));
        let subs = parse_subtitles(&bytes).expect("合法");
        assert_eq!(subs.len(), 2);
    }

    #[test]
    fn known_field_with_wrong_wire_type_is_an_error() {
        // subtitle 本该是长度前缀的子消息
        assert!(parse_subtitles(&varint_field(reply_field::SUBTITLE, 1)).is_err());
    }

    #[test]
    fn unmapped_known_fields_are_skipped_whatever_their_shape() {
        // id_str（字段 2）我们不读。它在真实响应里一定出现，而且上游若
        // 改了它的类型，不该让字幕整条解不出来。
        let mut body = varint_field(1, 7);
        body.extend(string_field(2, "7"));
        body.extend(string_field(3, "zh-CN"));
        body.extend(message_field(6, &varint_field(1, 1))); // author
        body.extend(varint_field(9, 1)); // ai_type
        let bytes = reply(&[body]);
        let subs = parse_subtitles(&bytes).expect("合法");
        assert_eq!(subs[0].id, 7);
        assert_eq!(subs[0].lan, "zh-CN");
    }

    #[test]
    fn truncated_item_is_an_error() {
        let mut body = tag(video_subtitle_field::SUBTITLES, 2);
        body.extend(varint(10)); // 声称 10 字节
        body.extend([0x1a, 0x02]); // 实际只有 2
        assert!(parse_subtitles(&message_field(reply_field::SUBTITLE, &body)).is_err());
    }
}
