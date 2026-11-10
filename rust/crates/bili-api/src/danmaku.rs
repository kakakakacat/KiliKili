//! `DmSegMobileReply` 解码：一个 6 分钟弹幕分段的全部弹幕。
//!
//! ## 输入是什么
//!
//! 入参是**解帧并解压之后**的 protobuf 字节，不是 HTTP 响应体。gRPC-over-
//! HTTP 的 5 字节前缀与 gzip 仍由 Dart 的 `GrpcReq.decompressProtobuf`
//! 处理，理由与「Dart 发请求、Rust 解析」一致（见本 crate 文档）：传输
//! 外壳属于那条还没迁移的链路，把它拆到两边会让同一段字节在两个实现里
//! 各走一半。等 Rust 接管传输时，解帧与解压一并过来，本函数的入参不变。
//!
//! ## 为什么值得迁移
//!
//! 一个热门视频的单个分段有数千条弹幕。基线为此专门在响应超过 256 KiB
//! 时把解析切到 isolate（`GrpcReq` 的 `_isolateSize` 与 `dmSegMobile` 的
//! `isolate: true`）——也就是说这段解析已经重到必须躲开 UI 线程，而其余
//! 已迁移的只读接口（详情、取流、搜索建议）都是一次一条记录。这里的收益
//! 是实打实的，而不是「因为能迁所以迁」。

use bili_domain::danmaku::{DanmakuColorful, DanmakuElem, DanmakuFrom, DanmakuSegment};
use bili_domain::error::{DomainError, ErrorKind};

use crate::protobuf::{Reader, WireType};

/// `DmSegMobileReply` 的字段号。
mod reply_field {
    pub(super) const ELEMS: u32 = 1;
    pub(super) const STATE: u32 = 2;
}

/// `DanmakuElem` 的字段号。
mod elem_field {
    pub(super) const ID: u32 = 1;
    pub(super) const PROGRESS: u32 = 2;
    pub(super) const MODE: u32 = 3;
    pub(super) const FONTSIZE: u32 = 4;
    pub(super) const COLOR: u32 = 5;
    pub(super) const MID_HASH: u32 = 6;
    pub(super) const CONTENT: u32 = 7;
    pub(super) const CTIME: u32 = 8;
    pub(super) const WEIGHT: u32 = 9;
    pub(super) const ACTION: u32 = 10;
    pub(super) const POOL: u32 = 11;
    pub(super) const ID_STR: u32 = 12;
    pub(super) const ATTR: u32 = 13;
    pub(super) const LIKE_COUNT: u32 = 15;
    pub(super) const ANIMATION: u32 = 22;
    pub(super) const EXTRA: u32 = 23;
    pub(super) const COLORFUL: u32 = 24;
    pub(super) const TYPE: u32 = 25;
    pub(super) const OID: u32 = 26;
    pub(super) const DM_FROM: u32 = 27;
}

/// 解码一个弹幕分段。
///
/// # Errors
/// 字节不是合法 protobuf、字符串不是合法 UTF-8、或字段的 wire type 与
/// 协议不符时返回 `Protocol`。**不做**部分成功：半个分段的弹幕比没有
/// 弹幕更难排查，调用方降级回 Dart 解析即可拿到完整结果。
pub fn parse_segment(bytes: &[u8]) -> Result<DanmakuSegment, DomainError> {
    let mut reader = Reader::new(bytes);
    let mut elems = Vec::new();
    let mut state = 0;

    while !reader.is_empty() {
        let tag = reader.read_tag()?;
        match (tag.field, tag.wire) {
            (reply_field::ELEMS, WireType::LengthDelimited) => {
                elems.push(parse_elem(reader.read_bytes()?)?);
            }
            (reply_field::STATE, WireType::Varint) => state = reader.read_int32()?,
            // 已知字段却换了 wire type：协议真的改了，不能当未知字段跳过
            // （同 `parse_elem`）。
            (field @ (reply_field::ELEMS | reply_field::STATE), wire) => {
                return Err(DomainError::new(
                    ErrorKind::Protocol,
                    &format!("分段字段 {field} 的 wire type 变成了 {wire:?}"),
                ));
            }
            // ai_flag / segment_rules / colorful_src / context_src 与将来
            // 新增的字段：当前没有消费方，解了也只是摆着。
            _ => reader.skip(tag.wire)?,
        }
    }

    Ok(DanmakuSegment { elems, state })
}

/// 解码一条弹幕。
fn parse_elem(bytes: &[u8]) -> Result<DanmakuElem, DomainError> {
    let mut reader = Reader::new(bytes);
    let mut elem = DanmakuElem::default();

    while !reader.is_empty() {
        let tag = reader.read_tag()?;
        // wire type 对不上就是读错了位置或协议变了。按字段名报错，不带值。
        match (tag.field, tag.wire) {
            (elem_field::ID, WireType::Varint) => elem.id = reader.read_int64()?,
            (elem_field::PROGRESS, WireType::Varint) => elem.progress = reader.read_int32()?,
            (elem_field::MODE, WireType::Varint) => elem.mode = reader.read_int32()?,
            (elem_field::FONTSIZE, WireType::Varint) => elem.fontsize = reader.read_int32()?,
            (elem_field::COLOR, WireType::Varint) => elem.color = reader.read_uint32()?,
            (elem_field::MID_HASH, WireType::LengthDelimited) => {
                elem.mid_hash = reader.read_string()?;
            }
            (elem_field::CONTENT, WireType::LengthDelimited) => {
                elem.content = reader.read_string()?;
            }
            (elem_field::CTIME, WireType::Varint) => elem.ctime = reader.read_int64()?,
            (elem_field::WEIGHT, WireType::Varint) => elem.weight = reader.read_int32()?,
            (elem_field::ACTION, WireType::LengthDelimited) => {
                elem.action = reader.read_string()?;
            }
            (elem_field::POOL, WireType::Varint) => elem.pool = reader.read_int32()?,
            (elem_field::ID_STR, WireType::LengthDelimited) => {
                elem.id_str = reader.read_string()?;
            }
            (elem_field::ATTR, WireType::Varint) => elem.attr = reader.read_int32()?,
            (elem_field::LIKE_COUNT, WireType::Varint) => elem.like_count = reader.read_int64()?,
            (elem_field::ANIMATION, WireType::LengthDelimited) => {
                elem.animation = reader.read_string()?;
            }
            (elem_field::EXTRA, WireType::LengthDelimited) => {
                elem.extra = reader.read_string()?;
            }
            (elem_field::COLORFUL, WireType::Varint) => {
                elem.colorful = DanmakuColorful::from_wire(reader.read_int32()?);
            }
            (elem_field::TYPE, WireType::Varint) => elem.dm_type = reader.read_int32()?,
            (elem_field::OID, WireType::Varint) => elem.oid = reader.read_int64()?,
            (elem_field::DM_FROM, WireType::Varint) => {
                elem.dm_from = DanmakuFrom::from_wire(reader.read_int32()?);
            }
            // 已知字段但 wire type 不符：协议真的变了（字段改了类型），
            // 这比未知字段严重，不能当作未知字段跳过。
            (field, wire) if is_known_elem_field(field) => {
                return Err(DomainError::new(
                    ErrorKind::Protocol,
                    &format!("弹幕字段 {field} 的 wire type 变成了 {wire:?}"),
                ));
            }
            // 真正的未知字段（含客户端本地的 100/101，理应不会从服务端来）
            _ => reader.skip(tag.wire)?,
        }
    }

    Ok(elem)
}

/// 该字段号是否属于已知的服务端字段。
const fn is_known_elem_field(field: u32) -> bool {
    matches!(field, 1..=13 | 15 | 22..=27)
}

#[cfg(test)]
// 测试里 expect 是正确做法：前提不成立时就该崩掉并指出原因。
#[allow(clippy::expect_used)]
mod tests {
    use super::{parse_segment, reply_field};
    use bili_domain::danmaku::{DanmakuColorful, DanmakuFrom};

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

    fn elem_field_bytes(body: &[u8]) -> Vec<u8> {
        let mut out = tag(reply_field::ELEMS, 2);
        out.extend(varint(body.len() as u64));
        out.extend(body);
        out
    }

    #[test]
    fn empty_input_is_an_empty_segment() {
        let seg = parse_segment(&[]).expect("空分段合法");
        assert!(seg.elems.is_empty());
        assert_eq!(seg.state, 0);
    }

    #[test]
    fn absent_fields_take_proto3_zero_values() {
        let seg = parse_segment(&elem_field_bytes(&[])).expect("空弹幕合法");
        let elem = &seg.elems[0];
        assert_eq!(elem.id, 0);
        assert_eq!(elem.content, "");
        assert_eq!(elem.colorful, DanmakuColorful::None);
        assert_eq!(elem.dm_from, DanmakuFrom::Unknown);
    }

    #[test]
    fn decodes_all_scalar_fields() {
        let mut body = Vec::new();
        body.extend(varint_field(1, 1_234_567_890_123));
        body.extend(varint_field(2, 61_000));
        body.extend(varint_field(3, 1));
        body.extend(varint_field(4, 25));
        body.extend(varint_field(5, 16_777_215));
        body.extend(string_field(6, "a1b2c3d4"));
        body.extend(string_field(7, "前方高能"));
        body.extend(varint_field(8, 1_700_000_000));
        body.extend(varint_field(9, 10));
        body.extend(string_field(10, ""));
        body.extend(varint_field(11, 0));
        body.extend(string_field(12, "1234567890123"));
        body.extend(varint_field(13, 1));
        body.extend(varint_field(15, 42));
        body.extend(string_field(22, "anim"));
        body.extend(string_field(23, r#"{"flag":1}"#));
        body.extend(varint_field(24, 60001));
        body.extend(varint_field(25, 1));
        body.extend(varint_field(26, 987_654));
        body.extend(varint_field(27, 1));

        let mut buf = elem_field_bytes(&body);
        buf.extend(varint_field(reply_field::STATE, 1));

        let seg = parse_segment(&buf).expect("合法分段");
        assert_eq!(seg.state, 1);
        let elem = &seg.elems[0];
        assert_eq!(elem.id, 1_234_567_890_123);
        assert_eq!(elem.progress, 61_000);
        assert_eq!(elem.color, 16_777_215);
        assert_eq!(elem.mid_hash, "a1b2c3d4");
        assert_eq!(elem.content, "前方高能");
        assert_eq!(elem.like_count, 42);
        assert_eq!(elem.extra, r#"{"flag":1}"#);
        assert_eq!(elem.colorful, DanmakuColorful::VipGradualColor);
        assert_eq!(elem.dm_from, DanmakuFrom::Normal);
        assert_eq!(elem.oid, 987_654);
    }

    #[test]
    fn unknown_enum_value_falls_back_to_default_like_the_baseline() {
        // protobuf.dart 认不出的枚举值会进 unknownFields，getter 返回默认值。
        let body = {
            let mut b = varint_field(24, 12_345);
            b.extend(varint_field(27, 99));
            b
        };
        let seg = parse_segment(&elem_field_bytes(&body)).expect("合法分段");
        assert_eq!(seg.elems[0].colorful, DanmakuColorful::None);
        assert_eq!(seg.elems[0].dm_from, DanmakuFrom::Unknown);
    }

    #[test]
    fn unknown_field_is_skipped() {
        let mut body = string_field(7, "hi");
        body.extend(varint_field(777, 1));
        body.extend(string_field(778, "未来新增的字段"));
        let seg = parse_segment(&elem_field_bytes(&body)).expect("未知字段不应使解析失败");
        assert_eq!(seg.elems[0].content, "hi");
    }

    #[test]
    fn repeated_singular_field_keeps_the_last_value() {
        // proto3 的 mergeFromBuffer 行为：后者覆盖前者。
        let mut body = string_field(7, "first");
        body.extend(string_field(7, "second"));
        let seg = parse_segment(&elem_field_bytes(&body)).expect("合法分段");
        assert_eq!(seg.elems[0].content, "second");
    }

    #[test]
    fn known_field_with_wrong_wire_type_is_an_error() {
        // content 本该是长度前缀，这里编成 varint
        let body = varint_field(7, 1);
        assert!(parse_segment(&elem_field_bytes(&body)).is_err());
    }

    #[test]
    fn reply_field_with_wrong_wire_type_is_an_error() {
        // elems 本该是长度前缀的子消息
        assert!(parse_segment(&varint_field(reply_field::ELEMS, 1)).is_err());
    }

    #[test]
    fn truncated_elem_is_an_error() {
        let mut buf = tag(reply_field::ELEMS, 2);
        buf.extend(varint(10)); // 声称 10 字节
        buf.extend([0x3a, 0x05]); // 实际只有 2
        assert!(parse_segment(&buf).is_err());
    }

    #[test]
    fn elems_keep_their_order() {
        let mut buf = elem_field_bytes(&string_field(7, "一"));
        buf.extend(elem_field_bytes(&string_field(7, "二")));
        buf.extend(elem_field_bytes(&string_field(7, "三")));
        let seg = parse_segment(&buf).expect("合法分段");
        let contents: Vec<_> = seg.elems.iter().map(|e| e.content.as_str()).collect();
        assert_eq!(contents, ["一", "二", "三"]);
    }
}
