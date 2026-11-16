//! 登录请求里的 RSA 加密。
//!
//! 移植自基线 `lib/http/login.dart`：密码登录把 `salt + password` 用
//! 接口下发的公钥加密，短信与密码登录都另带一个加密过的随机串 `dt`。
//!
//! ## 为什么这一段必须在 Rust
//!
//! 它是"协议没规定、但两端必须一致"的决定（AGENTS §2）：填充方式、
//! 编码顺序、以及**加密结果要先百分号编码再进表单**这一层，错一处的
//! 表现都是"密码错误"，从现象反推不出来。换宿主时这段不该重写一遍。
//!
//! ## 凭据边界
//!
//! 明文密码只以函数参数的形式出现，不落任何结构体、不进日志、调用结束
//! 即随栈帧消失。本模块不持有状态，也不接受 `Debug` 派生——见 RFC-0001
//! §2.2 与 `bili_domain::redact`。

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};
use rsa::pkcs1::DecodeRsaPublicKey as _;
use rsa::pkcs8::DecodePublicKey as _;
use rsa::traits::PublicKeyParts as _;
use rsa::{Pkcs1v15Encrypt, RsaPublicKey};

/// Dart `Uri.encodeComponent` 的保留集，与 [`crate::app_sign`] 同一套。
const ENCODE_SET: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'!')
    .remove(b'~')
    .remove(b'*')
    .remove(b'\'')
    .remove(b')')
    .remove(b'(');

/// 加密失败的原因。不含被加密的内容，也不含密钥。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CryptoError {
    /// 公钥不是可识别的 PEM（SPKI 或 PKCS#1）。
    BadKey,
    /// 明文超出该密钥一次能加密的长度，或随机源不可用。
    EncryptFailed,
}

impl std::fmt::Display for CryptoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BadKey => f.write_str("登录公钥无法解析"),
            Self::EncryptFailed => f.write_str("登录参数加密失败"),
        }
    }
}

impl std::error::Error for CryptoError {}

/// 解析接口下发的公钥。
///
/// 两种 PEM 都试：基线用的 `RSAKeyParser` 同时认 `PUBLIC KEY`（SPKI）与
/// `RSA PUBLIC KEY`（PKCS#1），线上两种都出现过。只认一种的话，另一种
/// 会在"密码错误"的伪装下失败。
///
/// 解析成功后还要求模数不短于 2048 bit（issue #156）。这是防御性的一层：
/// 要下发弱公钥，攻击者得先是 TLS MITM，而到了那一步密码请求的明文他本来
/// 就看得见，弱公钥并不额外给他什么。基线的 `RSAKeyParser` 同样不校验。
/// 加这一条只因为代价是一次比较，而「服务端某天换了短密钥」这件事没有
/// 别的地方会发现。
///
/// # Errors
/// 两种格式都解析不出，或模数短于 2048 bit → [`CryptoError::BadKey`]。
pub fn parse_public_key(pem: &str) -> Result<RsaPublicKey, CryptoError> {
    /// 可接受的最小模数字节数（2048 bit）。
    const MIN_MODULUS_BYTES: usize = 256;

    let key = RsaPublicKey::from_public_key_pem(pem.trim())
        .or_else(|_| RsaPublicKey::from_pkcs1_pem(pem.trim()))
        .map_err(|_| CryptoError::BadKey)?;
    if key.size() < MIN_MODULUS_BYTES {
        return Err(CryptoError::BadKey);
    }
    Ok(key)
}

/// 用公钥加密并 base64。PKCS#1 v1.5 填充，与基线 `encrypt` 包的默认一致。
///
/// 填充里的随机数取自系统随机源（`OsRng`）：同一明文每次加密结果不同，
/// 这是 PKCS#1 v1.5 的要求，不是可省的细节。
///
/// # Errors
/// 明文过长或随机源不可用 → [`CryptoError::EncryptFailed`]。
pub fn encrypt_base64(key: &RsaPublicKey, plaintext: &[u8]) -> Result<String, CryptoError> {
    let mut rng = rsa::rand_core::OsRng;
    key.encrypt(&mut rng, Pkcs1v15Encrypt, plaintext)
        .map(|cipher| BASE64.encode(cipher))
        .map_err(|_| CryptoError::EncryptFailed)
}

/// 加密密码：明文是 `salt + password`，salt 来自 web key 接口。
///
/// 返回值**不再百分号编码**——基线对 `password` 字段就是直接放 base64
/// （只有 `dt` 额外编码了一次），两者的差异必须原样保留。
///
/// # Errors
/// 见 [`encrypt_base64`]。
pub fn encrypt_password(
    key: &RsaPublicKey,
    salt: &str,
    password: &str,
) -> Result<String, CryptoError> {
    encrypt_base64(key, format!("{salt}{password}").as_bytes())
}

/// 加密 `dt` 字段：一个随机串，加密后**先百分号编码再进表单**。
///
/// 这层编码在基线里写作 `Uri.encodeComponent(...)`，于是表单序列化时
/// `%` 会被再编码一次（`%2F` → `%252F`），签名覆盖的也是编码后的值。
/// 看起来像 bug，但它是服务端实际收到的形状：去掉这一层等于改请求面。
///
/// # Errors
/// 见 [`encrypt_base64`]。
pub fn encrypt_dt(key: &RsaPublicKey, random: &str) -> Result<String, CryptoError> {
    let encrypted = encrypt_base64(key, random.as_bytes())?;
    Ok(utf8_percent_encode(&encrypted, ENCODE_SET).to_string())
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use rsa::pkcs8::EncodePublicKey as _;
    use rsa::{Pkcs1v15Encrypt as Padding, RsaPrivateKey};

    /// 2048 位：`parse_public_key` 的下限就是它（issue #156），再短的
    /// 密钥连解析都过不去。生成一次复用——2048 位在 debug 下要几秒，
    /// 每个用例各生成一次会让这个 crate 的测试明显变慢。
    fn key_pair() -> (RsaPrivateKey, RsaPublicKey) {
        static PAIR: std::sync::OnceLock<(RsaPrivateKey, RsaPublicKey)> =
            std::sync::OnceLock::new();
        PAIR.get_or_init(|| {
            let mut rng = rsa::rand_core::OsRng;
            let private = RsaPrivateKey::new(&mut rng, 2048).expect("生成测试密钥");
            let public = RsaPublicKey::from(&private);
            (private, public)
        })
        .clone()
    }

    #[test]
    fn password_round_trips_through_the_matching_private_key() {
        let (private, public) = key_pair();
        let encoded = encrypt_password(&public, "salt-", "hunter2").expect("加密");
        let cipher = BASE64.decode(encoded).expect("base64");
        let plain = private.decrypt(Padding, &cipher).expect("解密");
        // 盐在前、密码在后——顺序反了服务端只会回"密码错误"。
        assert_eq!(plain, b"salt-hunter2");
    }

    #[test]
    fn dt_is_percent_encoded_after_base64() {
        let (_, public) = key_pair();
        let dt = encrypt_dt(&public, "abcdefghijklmnop").expect("加密");
        // base64 的 `+` `/` `=` 必须已经被编码；否则表单里会被当成字段分隔。
        assert!(!dt.contains('+') && !dt.contains('/') && !dt.contains('='));
        assert!(
            dt.chars()
                .all(|c| c.is_ascii_alphanumeric() || "%-_.!~*'()".contains(c))
        );
    }

    #[test]
    fn spki_pem_is_accepted() {
        let (_, public) = key_pair();
        let pem = public
            .to_public_key_pem(rsa::pkcs8::LineEnding::LF)
            .expect("导出 SPKI");
        parse_public_key(&pem).expect("SPKI 必须能解析");
    }

    #[test]
    fn pkcs1_pem_is_accepted() {
        use rsa::pkcs1::EncodeRsaPublicKey as _;
        let (_, public) = key_pair();
        let pem = public
            .to_pkcs1_pem(rsa::pkcs8::LineEnding::LF)
            .expect("导出 PKCS#1");
        parse_public_key(&pem).expect("PKCS#1 必须能解析");
    }

    #[test]
    fn garbage_key_is_rejected_rather_than_guessed() {
        assert_eq!(parse_public_key("not a pem"), Err(CryptoError::BadKey));
    }

    /// issue #156：格式合法但模数过短的公钥也要拒。
    #[test]
    fn short_modulus_is_rejected_even_though_the_pem_parses() {
        let mut rng = rsa::rand_core::OsRng;
        let weak = RsaPublicKey::from(&RsaPrivateKey::new(&mut rng, 1024).expect("生成弱密钥"));
        let pem = weak
            .to_public_key_pem(rsa::pkcs8::LineEnding::LF)
            .expect("导出 SPKI");
        // PEM 本身没问题——拒绝的理由只能是长度。
        assert!(RsaPublicKey::from_public_key_pem(&pem).is_ok());
        assert_eq!(parse_public_key(&pem), Err(CryptoError::BadKey));
    }
}
