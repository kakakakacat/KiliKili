//! APP 签名：给移动端接口补 `appkey` / `ts` / `sign`。
//!
//! 移植自基线 `lib/utils/app_sign.dart`。算法：
//!
//! 1. 参数里放入 `appkey` 与 `ts`（Unix 秒）
//! 2. 按 key 升序排列
//! 3. 拼成 `k=v&k=v`，k 与 v 均按 Dart `Uri.encodeComponent` 编码
//! 4. `sign` = md5(查询串 + appsec)
//!
//! ## 与 WBI 不是同一套，别合并
//!
//! 两者都是「排序 + 拼接 + md5 + 盐」，但盐的来源、编码口径与适用接口
//! 都不同：
//!
//! | | WBI（[`crate::wbi`]） | APP（本模块） |
//! |---|---|---|
//! | 盐 | `mixin_key`，由 nav 接口按天轮换 | `appsec`，随 appkey 固定 |
//! | 时间字段 | `wts` | `ts` |
//! | 签名字段 | `w_rid` | `sign` |
//! | 值的编码 | 先剔除 `!'()*` 再百分号编码 | **不剔除**，直接编码 |
//!
//! 第四行是最容易踩的：`Uri.encodeComponent` 本来就不转义 `!'()*`，而
//! WBI 那边额外把它们从值里**删掉**。直播接口的 `statistics` 参数是一段
//! JSON（`{"appId":1,...}`），里面没有这五个字符，所以两套编码在当前参数
//! 集上碰巧同结果——正因为碰巧，更要分开写：哪天某个参数带上 `(`，合并
//! 过的实现会签出一个必被拒、且无从诊断的请求。
//!
//! ## 为什么 `ts` 由调用方注入
//!
//! 与 [`crate::wbi::sign`] 同理：签名覆盖时间，就地取系统时间会让整个
//! 查询串无法被 fixture 锁死，只能靠肉眼比对。

use md5::{Digest, Md5};
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};

/// 百分号编码字符集：Dart `Uri.encodeComponent` 的保留集。
///
/// 不转义 `A-Za-z0-9` 与 `-_.!~*'()`。注意它比 RFC 3986 的 unreserved
/// 多出 `!*'()`——这正是与 [`crate::wbi`] 的差异所在。
const ENCODE_SET: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'!')
    .remove(b'~')
    .remove(b'*')
    .remove(b'\'')
    .remove(b'(')
    .remove(b')');

fn encode(value: &str) -> String {
    utf8_percent_encode(value, ENCODE_SET).to_string()
}

/// 给参数补上 `appkey` / `ts` / `sign`。
///
/// 返回**已按 key 升序**的查询参数，`sign` 追加在最后——它是对前面所有
/// 参数的签名，混进排序里会让「签名覆盖了什么」不再一眼可见（与
/// [`crate::wbi`] 里 `w_rid` 的摆法一致）。
///
/// 传入的 `params` 不必有序，也不应已经含有 `appkey` / `ts` / `sign`：
/// 基线那条被注释掉的 `assert` 说明重试路径上出现过重复补签，这里直接
/// 覆盖同名键，使重复调用是幂等的。
#[must_use]
pub fn sign(
    params: &[(String, String)],
    appkey: &str,
    appsec: &str,
    ts: i64,
) -> Vec<(String, String)> {
    let mut sorted: Vec<(String, String)> = params
        .iter()
        .filter(|(k, _)| k != "appkey" && k != "ts" && k != "sign")
        .cloned()
        .collect();
    sorted.push(("appkey".to_owned(), appkey.to_owned()));
    sorted.push(("ts".to_owned(), ts.to_string()));
    sorted.sort_by(|a, b| a.0.cmp(&b.0));

    let payload = sorted
        .iter()
        .map(|(k, v)| format!("{}={}", encode(k), encode(v)))
        .collect::<Vec<_>>()
        .join("&");

    let digest = Md5::digest(format!("{payload}{appsec}").as_bytes());
    sorted.push(("sign".to_owned(), format!("{digest:x}")));
    sorted
}

#[cfg(test)]
// 测试里 expect 是正确做法：前提不成立时就该崩掉并指出原因。
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    // 基线 `Constants.appKey` / `appSec`（移动端 Android 的公开 appkey，
    // 不是用户凭据）。写死在测试里是为了让签名结果可被逐字节比对。
    const APP_KEY: &str = "dfca71928277209b";
    const APP_SEC: &str = "b5475a8825547a4fc26c7d518eaaa02e";

    fn pairs(kv: &[(&str, &str)]) -> Vec<(String, String)> {
        kv.iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    fn get<'a>(params: &'a [(String, String)], key: &str) -> &'a str {
        params
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
            .expect("参数应存在")
    }

    #[test]
    fn appends_appkey_ts_and_sign() {
        let out = sign(&pairs(&[("page", "1")]), APP_KEY, APP_SEC, 1_700_000_000);

        let keys: Vec<&str> = out.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(keys, ["appkey", "page", "ts", "sign"]);
        assert_eq!(get(&out, "appkey"), APP_KEY);
        assert_eq!(get(&out, "ts"), "1700000000");
    }

    #[test]
    fn signature_is_deterministic_for_fixed_ts() {
        let a = sign(&pairs(&[("page", "1")]), APP_KEY, APP_SEC, 1_700_000_000);
        let b = sign(&pairs(&[("page", "1")]), APP_KEY, APP_SEC, 1_700_000_000);
        assert_eq!(a, b, "相同输入必须产出相同签名，否则无法被 fixture 锁死");
    }

    #[test]
    fn different_ts_changes_signature() {
        let a = sign(&pairs(&[("page", "1")]), APP_KEY, APP_SEC, 1_700_000_000);
        let b = sign(&pairs(&[("page", "1")]), APP_KEY, APP_SEC, 1_700_000_001);
        assert_ne!(get(&a, "sign"), get(&b, "sign"), "ts 参与签名");
    }

    #[test]
    fn re_signing_is_idempotent() {
        let once = sign(&pairs(&[("page", "1")]), APP_KEY, APP_SEC, 1_700_000_000);
        let twice = sign(&once, APP_KEY, APP_SEC, 1_700_000_000);
        assert_eq!(once, twice, "重试路径上重复补签不得叠加出第二个 appkey");
    }

    // 基线 `Uri.encodeComponent` 不转义 `!*'()`，WBI 那边则是先把它们从
    // 值里删掉。两套口径一旦被合并，这条会先红。
    #[test]
    fn values_keep_the_characters_wbi_would_strip() {
        let out = sign(&pairs(&[("q", "a!b'c(d)e*f")]), APP_KEY, APP_SEC, 1);
        assert_eq!(
            get(&out, "q"),
            "a!b'c(d)e*f",
            "这几个字符不参与转义，也不该被剔除"
        );
    }

    #[test]
    fn json_valued_parameters_are_percent_encoded() {
        let statistics = r#"{"appId":1,"platform":3,"version":"8.43.0","abtest":""}"#;
        let out = sign(
            &pairs(&[("statistics", statistics)]),
            APP_KEY,
            APP_SEC,
            1_700_000_000,
        );
        // 返回值保持原样，编码只发生在签名用的那串里——查询串的编码由
        // 传输层负责，这里再编一次就会双重转义。
        assert_eq!(get(&out, "statistics"), statistics);
    }

    #[test]
    fn encode_matches_dart_uri_encode_component() {
        assert_eq!(encode("a b"), "a%20b");
        assert_eq!(encode("{\"k\":1}"), "%7B%22k%22%3A1%7D");
        assert_eq!(encode("中文"), "%E4%B8%AD%E6%96%87");
        assert_eq!(encode("-_.!~*'()"), "-_.!~*'()");
    }
}
