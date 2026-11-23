//! 宽松数字解析：服务端把「数字」发成字符串时的兜底。
//!
//! ## 为什么需要它
//!
//! serde 的失败是**整条响应级**的：它先把整个 DTO 解出来，才轮到业务
//! 分支。于是一个界面根本用不上的字段，类型猜错一次就能把整条接口判成
//! 协议错误。#161 的第三条 bug 就是这个形状——满级账号的
//! `level_info.next_exp` 服务端发的是字符串 `"--"`，而 DTO 写的是
//! `Option<i64>`，结果登录走不完、「我的」页也打不开。
//!
//! 这件事不是 `next_exp` 独有的。它能漏到真机，是三件事叠加：
//!
//! 1. **基线读不到的字段，迁移时没有参照物。** `LevelInfo.fromJson` 在
//!    `current_level == 6` 时改取 `current_exp`，压根不碰 `next_exp`，
//!    所以「它不是数字」在 Dart 侧永远不会暴露，类型只能靠猜。
//! 2. **手工构造的 fixture 会把错误假设一起冻进测试。** 当初那份
//!    `user_info_max_level.raw.json` 里 `next_exp` 填的是 `0`，正好把真机上
//!    会炸的那一种形状挡在测试之外，CI 全绿。
//! 3. Dart 基线的 `as int?` 同样会抛，但它抛在**那一个模型**上；Rust 这边
//!    是整条响应作废，代价不对等。
//!
//! 详见 issue #166 的普查结论。
//!
//! ## 什么字段该用，什么字段绝不能用
//!
//! 判据是**解不出来时，静默当作「没有这个值」是不是比报错更好**。
//!
//! **最该用**——**界面根本没有消费者的字段**。它跨 FFI 传过去就没人读
//! （典型：登录的 `expires_in`，续期靠 refresh_token，不靠它算时间）。
//! 这种字段类型猜错的代价与它的价值完全不成比例：零收益，换一整条接口
//! 不可用。#161 炸的那个 `next_exp` 正是这一类。
//!
//! **该用**——展示用的数字。播放量、弹幕数、收藏数、等级经验、粉丝数
//! 这类：解不出来最多少显示一个数字或一条进度条，远好过整个页面打不开。
//!
//! **绝不能用**——下面三类，解不出来就是真的出了问题，静默吞掉只会把一个
//! 协议错误换成一个空白页面，而且换得更难查：
//!
//! - **标识符**：`aid` / `mid` / `cid` / `oid` / `fid` / `room_id` / `season_id`
//!   …… 拿它们去发下一个请求，`None` 会变成一条打不通的链路。
//! - **金额**：硬币数、B 币余额。它们是 `f64` 且会参与计算，一个被吞掉的
//!   值和一个真的 0 在界面上长得一样。
//! - **判别用的枚举/状态码**：`type` / `state` / `code` / `attr` / `status`
//!   这类决定走哪条分支的字段。分支选错比报错更难查。
//!
//! 时间戳（`ctime` / `mtime` / `pubdate`）按用途判：只拿去格式化成一行字的
//! 可以放宽；参与排序或增量拉取游标的不行。本 crate 当前的时间戳都只用于
//! 显示，**但目前一律没有放宽**——基线的 `as int?` 也没放宽，没有实测依据
//! 就不单方面改口径。新增字段时请重新判一次，别照抄邻居。
//!
//! ## 一条容易搞反的推论
//!
//! 放宽**从不**比收紧丢信息：字段已经是 `Option`，「缺失」本来就要被下游
//! 处理；收紧只是把「服务端换了个写法」升级成整条响应作废。所以上面三类
//! 「绝不能用」靠的不是「怕丢信息」，而是**哪种失败更好查**——标识符变
//! `None` 之后，错误会在离现场很远的地方才冒出来。

use serde::Deserialize as _;
use serde_json::Value;

/// `serde` 的 `deserialize_with`：数字或数字字符串都收，再不行当没有这个值。
///
/// 用在 DTO 字段上：
///
/// ```ignore
/// #[serde(deserialize_with = "crate::lenient::opt_i64")]
/// play_count: Option<i64>,
/// ```
///
/// 浮点也收并截断——服务端偶尔把计数发成 `1.0`。截断而非四舍五入是因为
/// 这些都是计数，不存在 `0.5` 次播放，给到小数本身就说明那一侧出了偏差。
///
/// # Errors
/// 不产生自己的错误：任何解不出整数的形状都落到 `None`。上游 `Deserializer`
/// 的错误（例如 JSON 本身残缺）照常向外传。
pub fn opt_i64<'de, D>(deserializer: D) -> Result<Option<i64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(from_value(
        Option::<Value>::deserialize(deserializer)?.as_ref(),
    ))
}

/// 手工遍历 [`Value`] 时的同一套规则，取 `value[key]`。
///
/// 搜索那一族的响应形状在运行时才定（同一个 `result` 数组里混着多种
/// `result_type`），DTO 接不住，只能手工取值；这里保证它与
/// [`opt_i64`] 对「什么算数字」的判断完全一致。
#[must_use]
pub fn i64_at(value: &Value, key: &str) -> Option<i64> {
    from_value(value.get(key))
}

/// 两条路径共用的本体。
fn from_value(value: Option<&Value>) -> Option<i64> {
    match value? {
        Value::Number(n) => n.as_i64().or_else(|| {
            #[allow(clippy::cast_possible_truncation)]
            n.as_f64().map(|f| f as i64)
        }),
        Value::String(s) => {
            let s = s.trim();
            // 先按整数，再按浮点截断。两步是因为**同一个值写成串和写成数字
            // 必须得到同一个结果**：`204.8` 会截断成 204，而 `"204.8"` 若
            // 只试整数就会落到 None。这种不一致最难查——两台机器上服务端
            // 给的形态不同，界面就一个有数字一个没有。
            s.parse::<i64>().ok().or_else(|| {
                #[allow(clippy::cast_possible_truncation)]
                s.parse::<f64>()
                    .ok()
                    .filter(|f| f.is_finite())
                    .map(|f| f as i64)
            })
        }
        _ => None,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Default, Deserialize)]
    #[serde(default)]
    struct Probe {
        #[serde(deserialize_with = "opt_i64")]
        n: Option<i64>,
    }

    fn probe(body: &str) -> Option<i64> {
        serde_json::from_str::<Probe>(body).unwrap().n
    }

    #[test]
    fn numbers_and_numeric_strings_are_both_accepted() {
        assert_eq!(probe(r#"{"n": 31000}"#), Some(31_000));
        assert_eq!(probe(r#"{"n": "31000"}"#), Some(31_000));
        assert_eq!(probe(r#"{"n": " 31000 "}"#), Some(31_000));
        assert_eq!(probe(r#"{"n": -1}"#), Some(-1));
        assert_eq!(probe(r#"{"n": "-1"}"#), Some(-1));
    }

    #[test]
    fn floats_are_truncated_not_rounded() {
        assert_eq!(probe(r#"{"n": 1.0}"#), Some(1));
        assert_eq!(probe(r#"{"n": 1.9}"#), Some(1));
        // 同一个值写成串必须得到同一个结果，否则就成了「换台机器数字就没了」。
        assert_eq!(probe(r#"{"n": "1.0"}"#), Some(1));
        assert_eq!(probe(r#"{"n": "204.8"}"#), Some(204));
        assert_eq!(probe(r#"{"n": "-1.9"}"#), Some(-1));
        // NaN / inf 的串不是数字，落 None 而不是一个垃圾整数。
        assert_eq!(probe(r#"{"n": "NaN"}"#), None);
        assert_eq!(probe(r#"{"n": "inf"}"#), None);
    }

    /// 真机上把整条 nav 判成协议错误的那个形状（#161）。
    #[test]
    fn a_non_numeric_string_falls_to_none_instead_of_failing() {
        assert_eq!(probe(r#"{"n": "--"}"#), None);
        assert_eq!(probe(r#"{"n": ""}"#), None);
        assert_eq!(probe(r#"{"n": null}"#), None);
        assert_eq!(probe(r#"{"n": true}"#), None);
        assert_eq!(probe(r#"{"n": []}"#), None);
        assert_eq!(probe("{}"), None);
    }

    #[test]
    fn i64_at_follows_the_same_rule_as_opt_i64() {
        let v: Value = serde_json::from_str(r#"{"a": "7", "b": "--", "c": 7}"#).unwrap();
        assert_eq!(i64_at(&v, "a"), Some(7));
        assert_eq!(i64_at(&v, "b"), None);
        assert_eq!(i64_at(&v, "c"), Some(7));
        assert_eq!(i64_at(&v, "absent"), None);
    }
}
