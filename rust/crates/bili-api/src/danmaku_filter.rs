//! 云端弹幕屏蔽规则的请求构造与解析（S15 / issue #66）。
//!
//! 对应基线 `DanmakuFilterHttp.danmakuFilter`（`lib/http/danmaku_block.dart`）。
//! 新增与删除是写操作，等 ADR-013（issue #50）。
//!
//! 解析时按 `type` 把平列表分成三组（关键词/正则/用户）并**丢掉未知
//! type**——与基线的 `switch` 一致。那是协议知识，按 ADR-011 决策二留在
//! Rust 这一侧。

use bili_domain::danmaku_filter::{DanmakuFilter, FilterRule};
use bili_domain::error::DomainError;
use serde::Deserialize;

use crate::http::{HttpTransport, RequestContext};
use crate::request::PreparedRequest;

/// 接口路径。与基线 `lib/http/api.dart` 保持一致。
pub mod path {
    pub const DANMAKU_FILTER: &str = "/x/dm/filter/user";
}

/// 规则类型。服务端用数字表示，含义写在这里而不是调用点。
pub mod kind {
    /// 关键词。
    pub const KEYWORD: i64 = 0;
    /// 正则。
    pub const PATTERN: i64 = 1;
    /// 用户（`filter` 是 mid）。
    pub const USER: i64 = 2;
}

/// 取云端规则的请求。无参数。
#[must_use]
pub fn danmaku_filter_request() -> PreparedRequest {
    PreparedRequest {
        path: path::DANMAKU_FILTER,
        query: Vec::new(),
    }
}

/// 取云端弹幕屏蔽规则。
///
/// # Errors
/// 传输、状态码、信封三类失败，见各自的文档。
pub async fn fetch_danmaku_filter<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
) -> Result<DanmakuFilter, DomainError> {
    let body = crate::client::send(transport, ctx, &danmaku_filter_request()).await?;
    parse_danmaku_filter(&body)
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct DanmakuFilterDto {
    rule: Option<Vec<RuleDto>>,
    toast: Option<String>,
    valid: Option<i64>,
    ver: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RuleDto {
    id: Option<i64>,
    #[serde(rename = "type")]
    kind: Option<i64>,
    filter: Option<String>,
}

/// 解析云端规则。
///
/// # Errors
/// 信封失败沿用统一分类。缺 id/type/filter 的条目丢掉一条而不是炸一批
/// （ADR-011 决策三）；未知 type 同样丢掉，与基线的 `switch` 一致。
pub fn parse_danmaku_filter(body: &str) -> Result<DanmakuFilter, DomainError> {
    let dto: DanmakuFilterDto = crate::envelope::unwrap_envelope(body)?;

    let mut out = DanmakuFilter {
        // 基线把空串当成「没有提示」。
        toast: dto.toast.filter(|t| !t.is_empty()),
        valid: dto.valid,
        ver: dto.ver,
        ..DanmakuFilter::default()
    };

    for rule in dto.rule.unwrap_or_default() {
        let (Some(id), Some(k), Some(filter)) = (rule.id, rule.kind, rule.filter) else {
            continue;
        };
        let parsed = FilterRule {
            id,
            kind: k,
            filter,
        };
        match k {
            kind::KEYWORD => out.keywords.push(parsed),
            kind::PATTERN => out.patterns.push(parsed),
            kind::USER => out.users.push(parsed),
            // 未知 type：基线的 switch 没有 default 分支，同样丢掉。
            _ => {}
        }
    }

    Ok(out)
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    const BODY: &str = r#"{"code":0,"message":"0","data":{"rule":[
        {"id":1,"type":0,"filter":"剧透"},
        {"id":2,"type":1,"filter":"/^前方高能/"},
        {"id":3,"type":2,"filter":"12345"},
        {"id":4,"type":9,"filter":"未知类型"},
        {"id":5,"type":0}
    ],"toast":"","valid":1,"ver":7}}"#;

    #[test]
    fn rules_are_split_by_type() {
        let f = parse_danmaku_filter(BODY).expect("应解析成功");
        assert_eq!(f.keywords.len(), 1);
        assert_eq!(f.patterns.len(), 1);
        assert_eq!(f.users.len(), 1);
        assert_eq!(f.users[0].filter, "12345");
    }

    #[test]
    fn unknown_type_and_incomplete_rules_are_dropped() {
        let f = parse_danmaku_filter(BODY).expect("应解析成功");
        let total = f.keywords.len() + f.patterns.len() + f.users.len();
        assert_eq!(total, 3, "未知 type 与缺 filter 的各丢一条");
    }

    #[test]
    fn empty_toast_becomes_none() {
        let f = parse_danmaku_filter(BODY).expect("应解析成功");
        assert_eq!(f.toast, None);
        assert_eq!(f.ver, Some(7));
    }

    #[test]
    fn no_rules_is_not_an_error() {
        let f = parse_danmaku_filter(r#"{"code":0,"message":"0","data":{}}"#).expect("不是错误");
        assert!(f.keywords.is_empty() && f.patterns.is_empty() && f.users.is_empty());
    }
}
