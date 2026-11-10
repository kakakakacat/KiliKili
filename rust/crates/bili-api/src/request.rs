//! 请求构造：路径 + 已签名的查询参数。
//!
//! 产出是一个**纯数据**的 [`PreparedRequest`]，由调用方（Stage 1 是
//! Dart 的 Dio）去发。这样请求的形状可以被 fixture 锁死，而不需要起
//! 网络或 mock 一整个 HTTP 客户端。

use bili_protocol::wbi;

/// 接口路径常量。与基线 `lib/http/api.dart` 保持一致。
pub mod path {
    /// 视频详情（WBI 签名版）。
    pub const VIDEO_DETAIL: &str = "/x/web-interface/wbi/view";
}

/// 待发送的请求。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedRequest {
    pub path: &'static str,
    /// 查询参数，**已按 key 升序**且含 `wts`/`w_rid`。
    ///
    /// 用有序列表而非 Map：签名覆盖的是序列化后的字符串，顺序是签名
    /// 的一部分，换成 Map 等于把这个约束交给调用方的实现细节。
    pub query: Vec<(String, String)>,
}

/// 构造视频详情请求。
///
/// `mixin_key` 由 [`bili_protocol::wbi::mixin_key`] 从 nav 接口的
/// key 推导；`wts` 由调用方注入（Unix 秒），使本函数保持纯函数，可被
/// golden fixture 固定。
///
/// 返回 `None` 表示 `bvid` 为空——没有 bvid 的详情请求必然失败，与其
/// 发出去换一个业务错误码，不如在此处就拒绝。
#[must_use]
pub fn video_detail(bvid: &str, mixin_key: &str, wts: i64) -> Option<PreparedRequest> {
    if bvid.is_empty() {
        return None;
    }

    let mut query = vec![
        ("bvid".to_owned(), bvid.to_owned()),
        ("wts".to_owned(), wts.to_string()),
    ];
    query.sort_by(|a, b| a.0.cmp(&b.0));

    let w_rid = wbi::sign(&query, mixin_key);
    query.push(("w_rid".to_owned(), w_rid));

    Some(PreparedRequest {
        path: path::VIDEO_DETAIL,
        query,
    })
}

impl PreparedRequest {
    /// 拼成 `k=v&k=v` 形式。仅用于调试与测试断言；真正发送时由传输层
    /// 自行编码，避免这里的编码口径与传输层不一致。
    #[must_use]
    pub fn query_string(&self) -> String {
        self.query
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("&")
    }
}

#[cfg(test)]
// 测试里 expect 是正确做法：前提不成立时就该崩掉并指出原因。
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    // 与 bili-protocol 的 fixture 同源的占位 key（非真实凭据）。
    const MIXIN_KEY: &str = "ea1db124af3c7062474693fa704f4ff8";

    #[test]
    fn signs_and_appends_w_rid() {
        let req = super::video_detail("BV1L94y1H7CV", MIXIN_KEY, 1_702_204_169)
            .expect("非空 bvid 必须能构造请求");
        assert_eq!(req.path, path::VIDEO_DETAIL);

        let keys: Vec<&str> = req.query.iter().map(|(k, _)| k.as_str()).collect();
        // w_rid 必须在最后：它是对前面所有参数的签名，混进排序里会让
        // 「签名覆盖了什么」变得不可一眼看清。
        assert_eq!(keys, ["bvid", "wts", "w_rid"]);
    }

    #[test]
    fn signature_is_deterministic_for_fixed_wts() {
        let a = super::video_detail("BV1L94y1H7CV", MIXIN_KEY, 1_702_204_169);
        let b = super::video_detail("BV1L94y1H7CV", MIXIN_KEY, 1_702_204_169);
        assert_eq!(a, b, "相同输入必须产出相同签名，否则无法被 fixture 锁死");
    }

    #[test]
    fn different_wts_changes_signature() {
        let a = super::video_detail("BV1L94y1H7CV", MIXIN_KEY, 1_702_204_169);
        let b = super::video_detail("BV1L94y1H7CV", MIXIN_KEY, 1_702_204_170);
        assert_ne!(a, b, "wts 参与签名，变化必须反映到 w_rid");
    }

    #[test]
    fn empty_bvid_is_rejected() {
        assert!(super::video_detail("", MIXIN_KEY, 1).is_none());
    }
}
