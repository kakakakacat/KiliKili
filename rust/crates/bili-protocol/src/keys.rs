//! WBI key 的来源与缓存。
//!
//! `img_key` / `sub_key` 不是常量：它们是 nav 接口（`/x/web-interface/nav`）
//! 返回的两个图片 URL 的**文件名**，服务端按天轮换。基线
//! （`lib/utils/wbi_sign.dart`）取到后把推导出的 `mixin_key` 存进 Hive，
//! 按日失效。
//!
//! 本模块只做纯逻辑：URL → key、key → 缓存是否还新鲜。取数（发 nav 请求）
//! 在 `bili-api`，持久化在调用方——缓存写到哪里（Hive / 文件 / 内存）是
//! 宿主的事，塞进协议层会把一个易变协议细节和存储实现绑死。

use crate::wbi;

/// 从 nav 返回的图片 URL 中取出 key（即去掉扩展名的文件名）。
///
/// 例：`https://i0.hdslb.com/bfs/wbi/7cd0...77c.png` → `7cd0...77c`。
///
/// 与基线 `Utils.getFileName(url, fileExt: false)` 等价：取最后一个 `/`
/// 之后、最后一个 `.` 之前的部分；查询串（`?` 之后）不计入，且 `?` 之后
/// 的 `.` 不算扩展名分隔符。
///
/// 返回 `None` 表示 URL 里没有 `/`——基线此时抛 `RangeError` 并被
/// `catch` 吞成空串签名；空串签名会产出一个"看着像样"却必然被拒的请求，
/// 故此处让失败显形。
#[must_use]
pub fn key_from_url(url: &str) -> Option<&str> {
    let bytes = url.as_bytes();
    let mut slash: Option<usize> = None;
    let mut dot: Option<usize> = None;
    let mut q_mark = bytes.len();

    for index in (0..bytes.len()).rev() {
        match bytes[index] {
            b'/' => {
                slash = Some(index);
                break;
            }
            b'.' => {
                if dot.is_none() {
                    dot = Some(index);
                }
            }
            b'?' => {
                q_mark = index;
                // 倒序扫描，故此前记下的 `.` 若在 `?` 之后，属于查询串，
                // 不是扩展名分隔符。
                if dot.is_some_and(|d| d > q_mark) {
                    dot = None;
                }
            }
            _ => {}
        }
    }

    let start = slash? + 1;
    let end = dot.unwrap_or(q_mark);
    url.get(start..end)
}

/// 由 nav 的两个 URL 直接推导 `mixin_key`。
///
/// 任一步失败都返回 `None`：URL 形状变了、key 长度不足，都该让调用方
/// 降级并上报，而不是继续用一个错的 key 去签名。
#[must_use]
pub fn mixin_key_from_nav_urls(img_url: &str, sub_url: &str) -> Option<String> {
    let img_key = key_from_url(img_url)?;
    let sub_key = key_from_url(sub_url)?;
    wbi::mixin_key(img_key, sub_key)
}

/// 本地日序号：自 Unix 纪元起的第几天，按给定时区偏移折算。
///
/// 时区偏移由调用方给出（宿主知道本机时区，协议层不该拉进一个时区库）。
#[must_use]
pub const fn local_day_index(unix_secs: i64, utc_offset_secs: i32) -> i64 {
    let local = unix_secs + utc_offset_secs as i64;
    // 向下取整除：负数（1970 年之前）也要落到更早的那一天。
    local.div_euclid(86_400)
}

/// 按日失效的 `mixin_key` 缓存。
///
/// ## 与基线的一处有意偏差
///
/// 基线比较的是「上次取 key 的时间戳的**日期中的日**」与今天的日：
/// `DateTime.fromMillisecondsSinceEpoch(ts).day == now.day`。10 月 10 日
/// 取到的 key 在 11 月 10 日会被判定为新鲜，一整个月不刷新。这里改用
/// 自纪元起的日序号，跨月跨年都正确。
///
/// 偏差的方向是安全的：只会**更频繁**地刷新 key（最坏每天多一次 nav
/// 请求），不会用上过期的 key。反过来——沿用基线的判断——失效时拿到的
/// 是一个过期 key，签名随之失效，而服务端只会回普通业务错误码。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MixinKeyCache {
    entry: Option<(String, i64)>,
}

impl MixinKeyCache {
    #[must_use]
    pub const fn new() -> Self {
        Self { entry: None }
    }

    /// 从持久化的值恢复。`day` 为写入当天的 [`local_day_index`]。
    #[must_use]
    pub fn restored(mixin_key: String, day: i64) -> Self {
        Self {
            entry: Some((mixin_key, day)),
        }
    }

    /// 取今天可用的 key。`today` 为 [`local_day_index`] 的结果。
    ///
    /// 返回 `None` 表示需要重新取 nav。
    #[must_use]
    pub fn get(&self, today: i64) -> Option<&str> {
        match &self.entry {
            Some((key, day)) if *day == today => Some(key.as_str()),
            _ => None,
        }
    }

    /// 写入新取到的 key。
    pub fn store(&mut self, mixin_key: String, today: i64) {
        self.entry = Some((mixin_key, today));
    }

    /// 供持久化用的快照。
    #[must_use]
    pub fn snapshot(&self) -> Option<(&str, i64)> {
        self.entry.as_ref().map(|(k, d)| (k.as_str(), *d))
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn extracts_key_from_real_shaped_url() {
        assert_eq!(
            key_from_url("https://i0.hdslb.com/bfs/wbi/7cd084941338484aae1ad9425b84077c.png"),
            Some("7cd084941338484aae1ad9425b84077c")
        );
    }

    #[test]
    fn ignores_query_string() {
        assert_eq!(
            key_from_url("https://i0.hdslb.com/bfs/wbi/abc.png?foo=1"),
            Some("abc")
        );
    }

    #[test]
    fn dot_inside_query_is_not_an_extension() {
        // `?` 之后的 `.` 不是扩展名分隔符，否则会把 key 截断。
        assert_eq!(
            key_from_url("https://i0.hdslb.com/bfs/wbi/abc?v=1.2"),
            Some("abc")
        );
    }

    #[test]
    fn name_without_extension_is_kept_whole() {
        assert_eq!(key_from_url("https://x/y/abc"), Some("abc"));
    }

    #[test]
    fn url_without_slash_is_rejected() {
        // 基线在这里抛 RangeError 并被 catch 成空串；空串会签出一个
        // 必然被拒且无法从现象反推的请求。
        assert_eq!(key_from_url("abc.png"), None);
    }

    #[test]
    fn derives_mixin_key_from_urls() {
        let img = "https://i0.hdslb.com/bfs/wbi/7cd084941338484aae1ad9425b84077c.png";
        let sub = "https://i0.hdslb.com/bfs/wbi/4932caff0ff746eab6f01bf08b70ac45.png";
        let got = mixin_key_from_nav_urls(img, sub).expect("合法 URL 必须能推导");
        // 与 bili-protocol fixture 同源的期望值。
        assert_eq!(got, "ea1db124af3c7062474693fa704f4ff8");
    }

    #[test]
    fn day_index_rolls_at_local_midnight() {
        // 2026-11-10 15:30 UTC，东八区本地时间是 23:30，仍是同一天；
        // 再过一小时跨日。
        let utc8 = 8 * 3600;
        let a = local_day_index(1_762_788_600, utc8);
        let b = local_day_index(1_762_788_600 + 3_600, utc8);
        assert_eq!(b, a + 1, "本地午夜必须跨日");
    }

    #[test]
    fn cache_expires_across_months() {
        // 基线按「日期中的日」比较，10-10 的 key 在 11-10 会被判新鲜。
        let oct10 = local_day_index(1_760_000_000, 8 * 3600);
        let nov10 = local_day_index(1_762_600_000, 8 * 3600);
        let cache = MixinKeyCache::restored("stale".to_owned(), oct10);
        assert_eq!(cache.get(oct10), Some("stale"));
        assert_eq!(cache.get(nov10), None, "跨月必须失效");
    }

    #[test]
    fn store_then_get_same_day() {
        let mut cache = MixinKeyCache::new();
        assert_eq!(cache.get(20_000), None);
        cache.store("k".to_owned(), 20_000);
        assert_eq!(cache.get(20_000), Some("k"));
        assert_eq!(cache.get(20_001), None);
        assert_eq!(cache.snapshot(), Some(("k", 20_000)));
    }
}
