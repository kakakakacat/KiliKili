//! BV 号与 av 号的互转。
//!
//! 移植自基线 `lib/utils/id_utils.dart` 的 `bv2av` / `av2bv`。
//!
//! ## 为什么它属于协议层而不是工具函数
//!
//! BV 号是服务端定义的一种 av 号编码：一张 58 字符的字母表、两次固定
//! 位置的换位、一个异或掩码。算错一位不会报错，只会发出一个指向**另一个
//! 稿件**的请求——点赞点到别人头上。这正是 RFC-0001 §16.7 说的「易变
//! 协议」：正确性不取决于它合不合理，只取决于与服务端是否逐字节一致。
//!
//! ## 基线里还有一份
//!
//! `IdUtils.bv2av` 仍被几十处尚未迁移的 Dart 调用点使用，本切片
//! （S4 / issue #54）只把**写操作**这条路径换成了下面这一份。两份实现
//! 并存期间由 `test/bridge/video_write_bridge_test.dart` 对照同一组
//! 样本钉住；Dart 那份随最后一个调用点（预计 S3 之后的 Phase 2 清理）
//! 删除。

/// 异或掩码。与基线 `IdUtils.XOR_CODE` 同值。
const XOR_CODE: u64 = 23_442_827_791_579;
/// 低位掩码，截出 51 位。
const MASK_CODE: u64 = 2_251_799_813_685_247;
/// `1 << 51`，av2bv 时占住最高位保证定长。
const MAX_AID: u64 = 1 << 51;
const BASE: u64 = 58;

/// 字母表。顺序即进制位值，不可排序、不可去重。
///
/// 公开是为了让 `tests/bvid_golden.rs` 能逐字对照基线那一份：样本只能
/// 覆盖到它用上的那些字符，改一个没被命中的位置，往返测试照样全绿，而
/// 线上会有一小部分 BV 号悄悄转错。
pub const ALPHABET: &[u8; 58] = b"FcwAPNKTMug3GV5Lj7EJnHpWsx4tb8haYeviqBz6rkCy12mUSDQX9RdoZf";

/// 字符在字母表里的位置。`None` 表示该字符不属于字母表。
fn index_of(c: u8) -> Option<u64> {
    ALPHABET
        .iter()
        .position(|&x| x == c)
        .map(|i| u64::try_from(i).unwrap_or(0))
}

/// BV 号转 av 号。
///
/// 返回 `None` 而不是 panic 或 0：基线的 `invData[char]!` 遇到非法字符
/// 直接抛，而这里的入参有一部分来自分享链接解析，非法值是会真实出现的
/// 输入。给个 0 会发出一个指向 av0 的请求——比失败更难诊断。
///
/// 校验的是**形状**（长度 12、前三位 `BV1`、其余都在字母表里），不校验
/// 转换结果是否是一个存在的稿件——那只有服务端知道。
#[must_use]
pub fn bv_to_av(bvid: &str) -> Option<i64> {
    let bytes = bvid.as_bytes();
    if bytes.len() != 12 || !bvid.starts_with("BV1") {
        return None;
    }

    // 去掉 "BV1" 前缀后换位：基线是 `sublist(3)..swap(0,6)..swap(1,4)`。
    let mut arr = [0_u8; 9];
    arr.copy_from_slice(&bytes[3..]);
    arr.swap(0, 6);
    arr.swap(1, 4);

    let mut tmp: u64 = 0;
    for c in arr {
        // 58^9 < 2^53，不会溢出 u64；用 checked 只是免得将来改了字母表
        // 长度还默默回绕。
        tmp = tmp.checked_mul(BASE)?.checked_add(index_of(c)?)?;
    }

    i64::try_from((tmp & MASK_CODE) ^ XOR_CODE).ok()
}

/// av 号转 BV 号。
///
/// 目前只有测试用到（对照 [`bv_to_av`] 的往返），导出是因为它与
/// `bv_to_av` 是同一张表的两个方向，分开放会让下一个人以为只迁了一半。
#[must_use]
pub fn av_to_bv(aid: i64) -> Option<String> {
    let aid = u64::try_from(aid).ok()?;
    if aid >= MAX_AID {
        return None;
    }

    let mut bytes = *b"BV1000000000";
    let mut tmp = (MAX_AID | aid) ^ XOR_CODE;
    let mut i = bytes.len() - 1;
    while tmp > 0 {
        bytes[i] = ALPHABET[usize::try_from(tmp % BASE).unwrap_or(0)];
        tmp /= BASE;
        // 基线用的是 `bvIndex--`，位数固定，走不到越界；这里仍然兜一下，
        // 免得将来掩码改了之后静默写坏前缀。
        i = i.checked_sub(1)?;
    }

    bytes.swap(3, 9);
    bytes.swap(4, 7);

    String::from_utf8(bytes.to_vec()).ok()
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::{av_to_bv, bv_to_av};

    /// 几个样本，只当冒烟用。
    ///
    /// 真正的对照在 `tests/bvid_golden.rs`：期望值由
    /// `tool/fixture-gen/bvid_fixture.dart`（逐字复制基线 `IdUtils`）产出，
    /// 覆盖面与字母表的每一位都在那里钉死（ADR-008 决策三）。这里留三格，
    /// 是为了 `cargo test --lib` 改完算法就能立刻看出错没错。
    ///
    /// `BV1xx411c7mD` 是 av2，公开可查，用来锚住整张表。
    const PAIRS: &[(&str, i64)] = &[
        ("BV1xx411c7mD", 2),
        ("BV1L94y1H7CV", 366_088_782),
        ("BV1GJ411x7h7", 80_433_022),
    ];

    #[test]
    fn known_pairs_round_trip() {
        for (bvid, aid) in PAIRS {
            assert_eq!(bv_to_av(bvid), Some(*aid), "{bvid} 应当转成 av{aid}");
            assert_eq!(
                av_to_bv(*aid).as_deref(),
                Some(*bvid),
                "av{aid} 应当转回 {bvid}"
            );
        }
    }

    #[test]
    fn malformed_input_is_none_not_zero() {
        // 给 0 会发出一个指向 av0 的写请求——那比失败难诊断得多。
        for bad in ["", "BV1", "bv1xx411c7mD", "BV1xx411c7mDD", "BV1xx411c7m!"] {
            assert_eq!(bv_to_av(bad), None, "{bad} 不该转出一个号来");
        }
    }

    #[test]
    fn the_alphabet_has_no_duplicates() {
        // 有重复字符的话 bv_to_av 与 av_to_bv 就不再互逆，而且不会报错。
        let mut seen = super::ALPHABET.to_vec();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), super::ALPHABET.len());
    }
}
