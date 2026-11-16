//! 登录流程里的设备标识与会话标识。
//!
//! 移植自基线 `lib/utils/login_utils.dart::genDeviceId` 与
//! `lib/http/login.dart` 里就地算出的 `login_session_id`。
//!
//! 这些值服务端不解码，只做指纹：形状不对不会报参数错误，而是换来一个
//! 风控分支（要求验证码、直接拒登），与签名错误一样无法从现象反推。
//! 因此逐项复刻基线的构造方式，并以形状（长度、字符集）锁死——随机值
//! 没有固定期望可对照，与 [`crate::risk`] 同理。
//!
//! ## 为什么时间要由调用方给
//!
//! 设备号里嵌了本地日期时间的 BCD 编码。就地取系统时间会让构造函数不可
//! 测；时区则是宿主才知道的事（协议层不该拉进一个时区库），因此 Unix 秒
//! 与时区偏移都由调用方注入，与 [`crate::keys::local_day_index`] 一致。

use md5::{Digest, Md5};

use crate::risk::Entropy;

/// `Utils.generateRandomString` 的字符集：小写字母 + 数字。
const RANDOM_ALPHABET: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";

/// 生成 `device_id` / `bili_local_id`。
///
/// 基线的构造（来自 bilive_client）：16 个随机字节 + 7 个 BCD 编码的
/// 本地时间字节（世纪、年、月、日、时、分、秒）+ 8 个随机字节，整体
/// 十六进制化，末尾再接一个字节的和校验（低 8 位，两位十六进制）。
///
/// 注意基线那句 `Digest(bytes).toString()` **不是** md5：`crypto` 包的
/// `Digest` 只是字节序列的包装，`toString` 即十六进制化。照着名字当成
/// 哈希会算出一个长度只有一半的值。
#[must_use]
pub fn device_id<E: Entropy + ?Sized>(
    entropy: &mut E,
    unix_secs: i64,
    utc_offset_secs: i32,
) -> String {
    let (year, month, day, hour, minute, second) = local_datetime(unix_secs, utc_offset_secs);

    let mut bytes = Vec::with_capacity(31);
    bytes.extend(random_bytes(entropy, 16));
    bytes.push(dec_to_bcd(year / 100));
    bytes.push(dec_to_bcd(year % 100));
    bytes.push(dec_to_bcd(month));
    bytes.push(dec_to_bcd(day));
    bytes.push(dec_to_bcd(hour));
    bytes.push(dec_to_bcd(minute));
    bytes.push(dec_to_bcd(second));
    bytes.extend(random_bytes(entropy, 8));

    // 基线是 `bytes.sum & 0xFF`：求和在 Dart 里不会溢出，这里先按 u32
    // 累加再取低 8 位，等价且不依赖回绕语义。
    let check = bytes.iter().map(|b| u32::from(*b)).sum::<u32>() & 0xFF;

    let mut out = String::with_capacity(bytes.len() * 2 + 2);
    for b in &bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out.push_str(&format!("{check:02x}"));
    out
}

/// 生成 `login_session_id`：`md5(buvid + 毫秒时间戳)`。
///
/// 来源见基线注释里引的 <https://chinggg.github.io/post/appre/>。基线用
/// `ascii.encode` —— buvid 与时间戳都是 ASCII，与 UTF-8 编码结果一致。
#[must_use]
pub fn login_session_id(buvid: &str, unix_millis: i64) -> String {
    let digest = Md5::digest(format!("{buvid}{unix_millis}").as_bytes());
    format!("{digest:x}")
}

/// `Utils.generateRandomString`：小写字母与数字组成的随机串。
///
/// 登录请求里的 `dt` 就是它加密后的产物。
#[must_use]
pub fn random_string<E: Entropy + ?Sized>(entropy: &mut E, length: usize) -> String {
    (0..length)
        .map(|_| {
            let index = entropy.next_below(RANDOM_ALPHABET.len() as u32) as usize;
            // 索引由 next_below 限定在集合长度内，取不到越界值；
            // 真取到了也只能是 Entropy 实现违约，此时退回首字符而不是崩。
            char::from(RANDOM_ALPHABET.get(index).copied().unwrap_or(b'0'))
        })
        .collect()
}

fn random_bytes<E: Entropy + ?Sized>(entropy: &mut E, count: usize) -> Vec<u8> {
    (0..count)
        .map(|_| {
            // 基线是 `random.nextInt(256)`。
            u8::try_from(entropy.next_below(256)).unwrap_or(0)
        })
        .collect()
}

/// 十进制转 BCD：`12` → `0x12`。基线有 `assert(0 <= dec && dec < 100)`，
/// 这里对越界值取模而不是断言——release 构建里基线那条 assert 不执行，
/// 崩在协议层比发出一个畸形设备号更糟。
const fn dec_to_bcd(dec: i64) -> u8 {
    let dec = dec.rem_euclid(100);
    (((dec / 10) << 4) | (dec % 10)) as u8
}

/// Unix 秒 → 本地日历字段（年、月、日、时、分、秒）。
///
/// 用 Howard Hinnant 的 civil_from_days，避免为七个字段引入一个时区/日历
/// 依赖。公历规则对 1970 年之后恒成立，而设备号里只会出现"现在"。
const fn local_datetime(unix_secs: i64, utc_offset_secs: i32) -> (i64, i64, i64, i64, i64, i64) {
    let local = unix_secs + utc_offset_secs as i64;
    let days = local.div_euclid(86_400);
    let secs_of_day = local.rem_euclid(86_400);

    // 纪元移到 0000-03-01，使闰日落在四年周期末尾。
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };

    (
        year,
        m,
        d,
        secs_of_day / 3_600,
        (secs_of_day / 60) % 60,
        secs_of_day % 60,
    )
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    /// 脚本化随机源：按给定序列回放，耗尽后从头再来。测试要的是可对照的
    /// 输出，不是真随机。
    struct Scripted {
        values: Vec<u32>,
        index: usize,
    }

    impl Entropy for Scripted {
        fn next_below(&mut self, bound: u32) -> u32 {
            let v = self.values[self.index % self.values.len()];
            self.index += 1;
            v % bound
        }
    }

    #[test]
    fn device_id_has_baseline_shape() {
        let mut entropy = Scripted {
            values: vec![0, 1, 255, 128],
            index: 0,
        };
        // 2026-11-16 10:30:45 UTC+8。
        let id = device_id(&mut entropy, 1_794_796_245, 8 * 3_600);
        // 31 字节十六进制 + 两位校验。长度错了说明把 `Digest` 当成了 md5。
        assert_eq!(id.len(), 64);
        assert!(
            id.chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_uppercase())
        );
        // 第 17~23 字节是 BCD 时间：20 26 11 16 10 30 45。
        assert_eq!(&id[32..46], "20261116103045");
    }

    #[test]
    fn device_id_checksum_is_the_low_byte_of_the_sum() {
        let mut entropy = Scripted {
            values: vec![1],
            index: 0,
        };
        let id = device_id(&mut entropy, 0, 0);
        let bytes: Vec<u8> = (0..31)
            .map(|i| u8::from_str_radix(&id[i * 2..i * 2 + 2], 16).expect("十六进制"))
            .collect();
        let sum = bytes.iter().map(|b| u32::from(*b)).sum::<u32>() & 0xFF;
        assert_eq!(&id[62..], &format!("{sum:02x}"));
    }

    #[test]
    fn local_datetime_matches_known_instants() {
        // 1970-01-01 00:00:00 UTC
        assert_eq!(local_datetime(0, 0), (1970, 1, 1, 0, 0, 0));
        // 2024-02-29 23:59:59 UTC —— 闰日必须落在二月。
        assert_eq!(local_datetime(1_709_251_199, 0), (2024, 2, 29, 23, 59, 59));
        // 东八区跨日：2026-11-16 00:30 本地 = 2026-11-15 16:30 UTC。
        assert_eq!(
            local_datetime(1_794_760_200, 8 * 3_600),
            (2026, 11, 16, 0, 30, 0)
        );
    }

    #[test]
    fn session_id_is_md5_of_buvid_and_millis() {
        // 与基线 `md5.convert(ascii.encode(buvid + ts))` 同源的期望值。
        assert_eq!(
            login_session_id("XY1234567890", 1_700_000_000_000),
            format!("{:x}", Md5::digest(b"XY12345678901700000000000"))
        );
        assert_eq!(
            login_session_id("XY1234567890", 1_700_000_000_000).len(),
            32
        );
    }

    #[test]
    fn random_string_uses_the_baseline_alphabet() {
        let mut entropy = Scripted {
            values: (0..36).collect(),
            index: 0,
        };
        let s = random_string(&mut entropy, 16);
        assert_eq!(s.len(), 16);
        assert!(s.bytes().all(|b| RANDOM_ALPHABET.contains(&b)));
    }
}
