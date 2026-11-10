//! protobuf wire format 的最小读取器。
//!
//! ## 为什么不用 prost
//!
//! `prost-build` 要求构建环境里有 `protoc`。本项目的 Rust 产物要交叉编译
//! 到五个平台（见 `.github/workflows/rust.yml` 的 cross-build 矩阵），给
//! 每个构建环境再加一个外部二进制依赖，换来的只是两条消息的生成代码。
//! 把生成结果签入仓库可以免掉构建期依赖，但那是一份没人读得懂、改协议
//! 时必须重新找 `protoc` 才能更新的大文件。
//!
//! 弹幕分段只用到 wire format 的三种 wire type、十几个标量字段。手写
//! 读取器约一百行，且能在这里写清楚**与基线 protobuf.dart 的语义对齐**
//! ——那才是 shadow mode 比对的关键，生成代码反而会把它藏起来。
//!
//! 等需要解码的消息多到手写不划算（如 viewunite、dynamic 那几棵树），
//! 再引入 prost 并为构建期依赖单独做决策。
//!
//! ## 与 protobuf.dart 的语义对齐
//!
//! - **proto3 标量**：未出现的字段取零值；同一字段重复出现时**后者覆盖
//!   前者**（`mergeFromBuffer` 的行为）。
//! - **未知字段**：跳过。基线会留在 `unknownFields` 里，但没有消费方，
//!   且 shadow 比对的是已知字段的取值。
//! - **字符串**：非法 UTF-8 视为解码失败。基线用严格的 `Utf8Decoder`，
//!   同样抛错，整个分段解析失败。两边一致地失败，好过一边静默替换成
//!   U+FFFD 而让 shadow 报出差异。
//! - **group**（wire type 3/4）：视为协议错误。proto2 的遗留特性，弹幕
//!   协议不使用；真出现了说明读错了位置，静默跳过只会把错误推到更远处。

use bili_domain::error::{DomainError, ErrorKind};

/// wire type。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WireType {
    Varint,
    Fixed64,
    LengthDelimited,
    Fixed32,
}

impl WireType {
    fn from_bits(bits: u64) -> Result<Self, DomainError> {
        match bits {
            0 => Ok(Self::Varint),
            1 => Ok(Self::Fixed64),
            2 => Ok(Self::LengthDelimited),
            5 => Ok(Self::Fixed32),
            3 | 4 => Err(protocol("group 已废弃，弹幕协议不应出现")),
            other => Err(protocol(&format!("未知 wire type {other}"))),
        }
    }
}

/// 字段头：字段号与 wire type。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Tag {
    pub(crate) field: u32,
    pub(crate) wire: WireType,
}

/// 在一段字节上顺序读取 protobuf 字段。
#[derive(Debug)]
pub(crate) struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

/// 构造协议错误。
///
/// 诊断信息只含**位置与字段号**，不含字段内容：解析失败的详情会经桥接
/// 回到 Dart 并写进日志，而弹幕内容是用户数据。
fn protocol(detail: &str) -> DomainError {
    DomainError::new(ErrorKind::Protocol, detail)
}

impl<'a> Reader<'a> {
    pub(crate) const fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    pub(crate) const fn is_empty(&self) -> bool {
        self.pos >= self.buf.len()
    }

    /// 读一个字段头。
    pub(crate) fn read_tag(&mut self) -> Result<Tag, DomainError> {
        let key = self.read_varint()?;
        let field = u32::try_from(key >> 3)
            .map_err(|_| protocol(&format!("字段号越界，偏移 {}", self.pos)))?;
        if field == 0 {
            return Err(protocol(&format!("字段号 0 非法，偏移 {}", self.pos)));
        }
        Ok(Tag {
            field,
            wire: WireType::from_bits(key & 0b111)?,
        })
    }

    /// 读一个 varint。
    ///
    /// 上限 10 字节：64 位 varint 的最大长度。超过说明数据损坏或读错了
    /// 位置，继续读下去只会把一段任意字节当成数字。
    pub(crate) fn read_varint(&mut self) -> Result<u64, DomainError> {
        let mut value: u64 = 0;
        for shift in 0..10u32 {
            let byte = *self
                .buf
                .get(self.pos)
                .ok_or_else(|| protocol(&format!("varint 被截断，偏移 {}", self.pos)))?;
            self.pos += 1;
            value |= u64::from(byte & 0x7f) << (shift * 7);
            if byte & 0x80 == 0 {
                return Ok(value);
            }
        }
        Err(protocol(&format!("varint 超过 10 字节，偏移 {}", self.pos)))
    }

    /// 读 `len` 字节。
    fn read_slice(&mut self, len: usize) -> Result<&'a [u8], DomainError> {
        let end = self
            .pos
            .checked_add(len)
            .ok_or_else(|| protocol("长度相加溢出"))?;
        let slice = self
            .buf
            .get(self.pos..end)
            .ok_or_else(|| protocol(&format!("长度 {len} 超出剩余字节，偏移 {}", self.pos)))?;
        self.pos = end;
        Ok(slice)
    }

    /// 读一段长度前缀的字节。
    pub(crate) fn read_bytes(&mut self) -> Result<&'a [u8], DomainError> {
        let len = usize::try_from(self.read_varint()?)
            .map_err(|_| protocol(&format!("长度越界，偏移 {}", self.pos)))?;
        self.read_slice(len)
    }

    /// 读一个字符串字段。
    pub(crate) fn read_string(&mut self) -> Result<String, DomainError> {
        let at = self.pos;
        let bytes = self.read_bytes()?;
        core::str::from_utf8(bytes)
            .map(str::to_owned)
            .map_err(|_| protocol(&format!("字符串不是合法 UTF-8，偏移 {at}")))
    }

    /// 读 `int32`。
    ///
    /// 负数在 wire 上是按 64 位符号扩展编码的（10 字节），截断到低 32 位
    /// 再按补码解释即可还原——与 protobuf.dart 的 `aI` 一致。
    pub(crate) fn read_int32(&mut self) -> Result<i32, DomainError> {
        let raw = self.read_varint()?;
        Ok(raw as u32 as i32)
    }

    /// 读 `uint32`。
    pub(crate) fn read_uint32(&mut self) -> Result<u32, DomainError> {
        let raw = self.read_varint()?;
        Ok(raw as u32)
    }

    /// 读 `int64`。
    pub(crate) fn read_int64(&mut self) -> Result<i64, DomainError> {
        let raw = self.read_varint()?;
        Ok(raw as i64)
    }

    /// 跳过一个不关心的字段。
    pub(crate) fn skip(&mut self, wire: WireType) -> Result<(), DomainError> {
        match wire {
            WireType::Varint => {
                self.read_varint()?;
            }
            WireType::Fixed64 => {
                self.read_slice(8)?;
            }
            WireType::LengthDelimited => {
                self.read_bytes()?;
            }
            WireType::Fixed32 => {
                self.read_slice(4)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
// 测试里 expect 是正确做法：前提不成立时就该崩掉并指出原因。
#[allow(clippy::expect_used)]
mod tests {
    use super::{Reader, WireType};

    /// 编码一个 varint，供测试构造字节串。
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

    #[test]
    fn reads_tag_and_wire_type() {
        // 字段 7，wire type 2
        let buf = varint((7 << 3) | 2);
        let mut r = Reader::new(&buf);
        let tag = r.read_tag().expect("合法输入应当解析成功");
        assert_eq!(tag.field, 7);
        assert_eq!(tag.wire, WireType::LengthDelimited);
        assert!(r.is_empty());
    }

    #[test]
    fn negative_int32_is_sign_extended_on_the_wire() {
        // protobuf 把 -1 的 int32 编成 64 位符号扩展的 10 字节 varint
        let buf = varint(u64::MAX);
        assert_eq!(
            Reader::new(&buf)
                .read_int32()
                .expect("合法输入应当解析成功"),
            -1
        );
    }

    #[test]
    fn int32_truncates_to_low_32_bits() {
        let buf = varint(0x1_0000_0007);
        assert_eq!(
            Reader::new(&buf)
                .read_int32()
                .expect("合法输入应当解析成功"),
            7
        );
    }

    #[test]
    fn truncated_varint_is_an_error() {
        assert!(Reader::new(&[0x80, 0x80]).read_varint().is_err());
    }

    #[test]
    fn overlong_varint_is_an_error() {
        assert!(Reader::new(&[0x80; 11]).read_varint().is_err());
    }

    #[test]
    fn length_beyond_buffer_is_an_error() {
        // 长度 9，实际只有 3 字节
        assert!(Reader::new(&[9, 1, 2, 3]).read_bytes().is_err());
    }

    #[test]
    fn invalid_utf8_string_is_an_error() {
        assert!(Reader::new(&[2, 0xff, 0xfe]).read_string().is_err());
    }

    #[test]
    fn group_wire_type_is_rejected() {
        let buf = varint((1 << 3) | 3);
        assert!(Reader::new(&buf).read_tag().is_err());
    }

    #[test]
    fn zero_field_number_is_rejected() {
        assert!(Reader::new(&[0x00]).read_tag().is_err());
    }

    #[test]
    fn skip_advances_past_each_wire_type() {
        let mut buf = Vec::new();
        buf.extend(varint(300)); // varint 字段值
        buf.extend([0; 8]); // fixed64
        buf.extend([3, b'a', b'b', b'c']); // 长度前缀
        buf.extend([0; 4]); // fixed32
        buf.push(0x2a); // 哨兵：跳完应正好停在这里

        let mut r = Reader::new(&buf);
        for wire in [
            WireType::Varint,
            WireType::Fixed64,
            WireType::LengthDelimited,
            WireType::Fixed32,
        ] {
            r.skip(wire).expect("合法输入应当解析成功");
        }
        assert_eq!(r.read_varint().expect("合法输入应当解析成功"), 0x2a);
        assert!(r.is_empty());
    }
}
