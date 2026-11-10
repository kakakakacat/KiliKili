//! WBI 签名：生成请求的 `w_rid` 与 `wts` 字段。
//!
//! 移植自基线 `lib/utils/wbi_sign.dart`。算法：
//!
//! 1. `mixin_key` = 按固定重排表从 `img_key + sub_key` 取 32 个字符
//! 2. `wts` = 当前 Unix 秒
//! 3. 参数按 key 升序排列
//! 4. 拼成 `k=v&k=v`，k 与 v 均百分号编码，**v 先剔除 `!'()*`**
//! 5. `w_rid` = md5(查询串 + mixin_key)
//!
//! 正确性完全依赖那张重排表与编码细节，任何偏差都会导致服务端拒绝请求
//! 而非报错，很难从现象反推。故以 Dart 基线产出的 fixture 锁死
//! （`tests/wbi_golden.rs`）。

use md5::{Digest, Md5};
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};

/// 字符重排表。来自基线 `_mixinKeyEncTab`，**不得改动**。
const MIXIN_KEY_ENC_TAB: [usize; 32] = [
    46, 47, 18, 2, 53, 8, 23, 32, 15, 50, 10, 31, 58, 3, 45, 35, 27, 43, 5, 49, 33, 9, 42, 19, 29,
    28, 14, 39, 12, 38, 41, 13,
];

/// 参数值中需剔除的字符。来自基线 `_chrFilter`。
const STRIPPED: [char; 5] = ['!', '\'', '(', ')', '*'];

/// 百分号编码字符集。
///
/// 基线用 Dart 的 `Uri.encodeComponent`，其保留集为
/// `A-Za-z0-9 - _ . ! ~ * ' ( )`。由于 `!'()*` 已在上一步被剔除，
/// 实际等价于 RFC 3986 的 unreserved 集合 `A-Za-z0-9 - . _ ~`。
const ENCODE_SET: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');

/// 由 `img_key` 与 `sub_key` 推导 `mixin_key`。
///
/// 两个 key 来自 nav 接口的 `wbi_img` 字段，各 32 字符。
///
/// 返回 `None` 表示输入长度不足以覆盖重排表索引（最大 58），
/// 此时签名无法进行——静默返回空串会让请求以难以诊断的方式失败。
#[must_use]
pub fn mixin_key(img_key: &str, sub_key: &str) -> Option<String> {
    let joined: Vec<char> = img_key.chars().chain(sub_key.chars()).collect();
    MIXIN_KEY_ENC_TAB
        .iter()
        .map(|&i| joined.get(i).copied())
        .collect()
}

/// 为参数计算 `w_rid`。
///
/// `params` 须**已包含** `wts`，因为签名覆盖它；由调用方注入时间，
/// 使本函数保持纯函数、可被 fixture 测试。
#[must_use]
pub fn sign(params: &[(String, String)], mixin_key: &str) -> String {
    let mut sorted: Vec<&(String, String)> = params.iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));

    let query = sorted
        .iter()
        .map(|(k, v)| {
            let cleaned: String = v.chars().filter(|c| !STRIPPED.contains(c)).collect();
            format!(
                "{}={}",
                utf8_percent_encode(k, ENCODE_SET),
                utf8_percent_encode(&cleaned, ENCODE_SET)
            )
        })
        .collect::<Vec<_>>()
        .join("&");

    let mut hasher = Md5::new();
    hasher.update(query.as_bytes());
    hasher.update(mixin_key.as_bytes());
    format!("{:x}", hasher.finalize())
}
