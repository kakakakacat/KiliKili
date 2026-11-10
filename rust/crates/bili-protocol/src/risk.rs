//! playurl 的风控参数。
//!
//! 移植自基线 `lib/http/video.dart::videoUrl` 与 `lib/utils/utils.dart`
//! 的 `base64EncodeRandomString`。
//!
//! 这些参数对播放地址本身毫无影响：服务端用它们识别「这是不是一个真的
//! 网页播放器」。缺了或形状不对，返回的不是参数错误，而是清晰度被降级
//! 或一个普通业务错误码——与签名错误一样无法从现象反推。因此这里逐项
//! 复刻基线的形状，并以不变量（长度区间、字符集）而非 golden 值锁死：
//! 随机值无法对照固定期望，能锁的是**它的形状**。
//!
//! ## 为什么随机源要由调用方注入
//!
//! [`RiskTokens::generate`] 取一个 [`Entropy`]，而不是内部直接取系统
//! 随机数。这样请求构造全程是纯函数，可以被 fixture 固定（见
//! `bili-api/tests/playurl_golden.rs`）；否则每次构造出的查询串都不同，
//! 签名也就无从对照。

/// 随机源 Port。
///
/// 只要一个「取 `[0, bound)` 内整数」的能力：基线用的就是
/// `Random.nextInt`，再多的接口（填充字节、生成区间）都会让测试替身
/// 需要猜测调用顺序。
pub trait Entropy {
    /// 返回 `[0, bound)` 内的一个值。`bound` 恒大于 0。
    fn next_below(&mut self, bound: u32) -> u32;
}

/// 系统随机源。生产路径用它；测试用脚本化的替身。
///
/// 用 `rand` 的线程局部生成器（ChaCha12，由 OS 熵播种）：这些 token
/// 不是密钥，但可预测的序列等于一个稳定指纹，与参数存在的目的相悖。
#[cfg(feature = "system-entropy")]
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemEntropy;

#[cfg(feature = "system-entropy")]
impl Entropy for SystemEntropy {
    fn next_below(&mut self, bound: u32) -> u32 {
        use rand::Rng as _;
        rand::rng().random_range(0..bound)
    }
}

/// `dm_img_inter`。基线写死的常量，不是模板。
///
/// 真实网页播放器会在这里报告弹幕区域的尺寸与偏移；基线一律报 0，
/// 说明服务端当前只校验结构而非数值。保持与基线逐字节一致：改成
/// "看起来更真实" 的数值等于在未经验证的方向上赌一把。
pub const DM_IMG_INTER: &str = r#"{"ds":[],"wh":[0,0,0],"of":[0,0,0]}"#;

/// `dm_img_list`。基线恒为空数组字面量。
pub const DM_IMG_LIST: &str = "[]";

/// `gaia_source`。基线恒为 `pre-load`。
pub const GAIA_SOURCE: &str = "pre-load";

/// `web_location`。网页播放器页面的魔数，含义未公开，原样透传。
pub const WEB_LOCATION: i64 = 1_315_873;

/// 随机 token 的字节取值区间 `[0x26, 0x7E]`。
///
/// 来自基线 `0x26 + random.nextInt(0x59)`。下界 0x26 是为了避开 `%`
/// （0x25）——注释见基线 `generateRandomBytes`：`dm_img_str` 不能含
/// `%`，否则在拼接查询串时会被当成百分号编码的起始。
const BYTE_BASE: u8 = 0x26;
const BYTE_SPAN: u32 = 0x59;

/// `dm_img_str` 的字节数区间（编码前）。
pub const IMG_STR_BYTES: (usize, usize) = (16, 64);
/// `dm_cover_img_str` 的字节数区间（编码前）。
pub const COVER_IMG_STR_BYTES: (usize, usize) = (32, 128);

/// 一次 playurl 请求用的随机风控 token。
///
/// 两个字段每次请求都应重新生成：基线就是在 `videoUrl` 内部现生成的，
/// 复用同一对值等于给服务端一个稳定指纹，与参数存在的目的相悖。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RiskTokens {
    pub dm_img_str: String,
    pub dm_cover_img_str: String,
}

impl RiskTokens {
    /// 用给定随机源生成一对 token。
    #[must_use]
    pub fn generate<E: Entropy + ?Sized>(entropy: &mut E) -> Self {
        Self {
            dm_img_str: random_token(IMG_STR_BYTES.0, IMG_STR_BYTES.1, entropy),
            dm_cover_img_str: random_token(COVER_IMG_STR_BYTES.0, COVER_IMG_STR_BYTES.1, entropy),
        }
    }
}

/// 生成一个长度在 `[min_bytes, max_bytes]` 的随机字节串的 base64，
/// **并去掉末尾两个字符**。
///
/// 去掉两个字符是基线行为（`base64EncodeRandomString`）：
/// `randomBase64.substring(0, randomBase64.length - 2)`。
/// 当字节数 ≡ 1 (mod 3) 时去掉的恰是两个 `=`，等价于无填充编码；
/// 其余情况去掉的是真实数据字符，产出一个**不能解码**的串。
/// 这看起来像 bug，但服务端接受的正是这个形状，所以照搬——此处与基线
/// 的任何"修正"都只能靠线上验证，不能靠推理。
///
/// # Panics
/// `min_bytes > max_bytes` 时 panic：这是调用方写错了常量，不是运行时
/// 可恢复的状态。
#[must_use]
pub fn random_token<E: Entropy + ?Sized>(
    min_bytes: usize,
    max_bytes: usize,
    entropy: &mut E,
) -> String {
    assert!(min_bytes <= max_bytes, "字节数区间颠倒");
    let span = u32::try_from(max_bytes - min_bytes + 1).unwrap_or(u32::MAX);
    let len = min_bytes + entropy.next_below(span) as usize;

    let bytes: Vec<u8> = (0..len)
        .map(|_| BYTE_BASE.saturating_add(entropy.next_below(BYTE_SPAN) as u8))
        .collect();

    let mut encoded = base64_standard(&bytes);
    // 基线用 substring 截断，对 Dart 的 UTF-16 码元操作；base64 输出全是
    // ASCII，故按字节截断等价。
    encoded.truncate(encoded.len().saturating_sub(2));
    encoded
}

/// 标准 base64（RFC 4648，带 `=` 填充）。
///
/// 手写而非引入依赖：只需要编码一个方向、固定字母表，二十行即可，
/// 且避免为一个易变协议细节绑定一个外部 crate 的版本策略。
fn base64_standard(input: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = u32::from(chunk[0]);
        let b1 = chunk.get(1).copied().map_or(0, u32::from);
        let b2 = chunk.get(2).copied().map_or(0, u32::from);
        let n = (b0 << 16) | (b1 << 8) | b2;

        out.push(ALPHABET[(n >> 18) as usize & 0x3F] as char);
        out.push(ALPHABET[(n >> 12) as usize & 0x3F] as char);
        if chunk.len() > 1 {
            out.push(ALPHABET[(n >> 6) as usize & 0x3F] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(ALPHABET[n as usize & 0x3F] as char);
        } else {
            out.push('=');
        }
    }
    out
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    /// 可预测的随机源：按给定序列循环取值。
    struct Scripted {
        values: Vec<u32>,
        at: usize,
    }

    impl Entropy for Scripted {
        fn next_below(&mut self, bound: u32) -> u32 {
            let v = self.values[self.at % self.values.len()] % bound;
            self.at += 1;
            v
        }
    }

    /// 线性同余，仅用于测试里制造"看起来随机"的序列。
    struct Lcg(u64);

    impl Entropy for Lcg {
        fn next_below(&mut self, bound: u32) -> u32 {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            u32::try_from((self.0 >> 33) % u64::from(bound)).unwrap_or(0)
        }
    }

    #[test]
    fn base64_matches_rfc4648_vectors() {
        // RFC 4648 §10 的测试向量，覆盖三种填充情形。
        assert_eq!(base64_standard(b""), "");
        assert_eq!(base64_standard(b"f"), "Zg==");
        assert_eq!(base64_standard(b"fo"), "Zm8=");
        assert_eq!(base64_standard(b"foo"), "Zm9v");
        assert_eq!(base64_standard(b"foob"), "Zm9vYg==");
        assert_eq!(base64_standard(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64_standard(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn token_length_tracks_baseline_formula() {
        // 取最小长度：区间内取 0。16 字节 → base64 24 字符（含两个 `=`）
        // → 去掉 2 个 → 22。
        let mut min = Scripted {
            values: vec![0],
            at: 0,
        };
        assert_eq!(random_token(16, 64, &mut min).len(), 22);

        // 取最大长度：区间 [16,64] 的跨度是 49，取 48 即 64 字节 →
        // base64 88 字符（含一个 `=`）→ 去掉 2 → 86。
        let mut max = Scripted {
            values: vec![48],
            at: 0,
        };
        let token = random_token(16, 64, &mut max);
        assert_eq!(token.len(), 86);
    }

    #[test]
    fn token_never_contains_percent() {
        // 基线特意把字节下界设在 0x26 以避开 `%`；这条不变量若被破坏，
        // 查询串会被当成含百分号编码，服务端看到的值与我们签名的不同。
        let mut rng = Lcg(0x5DEE_CE66);
        for _ in 0..200 {
            let token = random_token(16, 64, &mut rng);
            assert!(!token.contains('%'), "token 不得含 % : {token}");
        }
    }

    #[test]
    fn token_is_base64_alphabet_only() {
        let mut rng = Lcg(42);
        for _ in 0..200 {
            let token = random_token(32, 128, &mut rng);
            assert!(
                token
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '='),
                "token 含 base64 字母表之外的字符: {token}"
            );
        }
    }

    #[test]
    fn token_length_stays_in_baseline_range() {
        let mut rng = Lcg(7);
        for _ in 0..500 {
            let n = random_token(16, 64, &mut rng).len();
            // 16..=64 字节 → 去掉 2 字符后 22..=86。
            assert!((22..=86).contains(&n), "长度越界: {n}");
        }
    }

    #[cfg(feature = "system-entropy")]
    #[test]
    fn system_entropy_stays_in_bounds_and_varies() {
        let mut e = SystemEntropy;
        for _ in 0..1000 {
            assert!(e.next_below(49) < 49);
        }
        // 两次生成相同的概率可忽略；若相等，说明随机源没在工作，
        // 而那正是这些参数失去意义的情形。
        let a = RiskTokens::generate(&mut e);
        let b = RiskTokens::generate(&mut e);
        assert_ne!(a, b);
        assert!(!a.dm_img_str.contains('%'));
    }

    #[test]
    fn two_tokens_differ() {
        // 同一次请求里两个 token 必须独立取值。若实现复用了同一串，
        // 指纹就固定了，参数也就失去意义。
        let mut rng = Lcg(99);
        let t = RiskTokens::generate(&mut rng);
        assert_ne!(t.dm_img_str, t.dm_cover_img_str);
    }
}
