//! 云端弹幕屏蔽规则的请求构造与解析（S15 / issue #66）。
//!
//! 对应基线整个 `DanmakuFilterHttp`（`lib/http/danmaku_block.dart`，本片迁完
//! 已删）：拉取、新增、删除三条。
//!
//! ## 它是 ADR-013 的第一次落地
//!
//! 新增与删除是仓库里**头两条走 Rust 的写操作**，所以这里的形状会被后面
//! 的切片照抄，有两处是刻意的：
//!
//! - **表单里没有 csrf。** 它由 [`crate::client::send_write`] 从上下文统一
//!   补，是全仓库唯一的注入点（ADR-013 决策四）。因此下面两个
//!   `*_request` 的产出是纯业务参数，可以原样写进请求快照而不含凭据——
//!   这正是 ADR-013 决策六要的那份对照物。
//! - **幂等级别写在构造处**，不在调用点：判据属于「对这条接口的理解」，
//!   跟协议知识同级，不该由界面那一侧来猜（ADR-013 决策二）。
//!
//! 解析时按 `type` 把平列表分成三组（关键词/正则/用户）并**丢掉未知
//! type**——与基线的 `switch` 一致。那是协议知识，按 ADR-011 决策二留在
//! Rust 这一侧。

use bili_domain::danmaku_filter::{DanmakuFilter, FilterRule};
use bili_domain::error::{DomainError, ErrorKind};
use serde::Deserialize;

use crate::http::{HttpTransport, Idempotency, RequestContext};
use crate::request::{PreparedRequest, PreparedWrite};

/// 接口路径。与基线 `lib/http/api.dart` 保持一致。
pub mod path {
    pub const DANMAKU_FILTER: &str = "/x/dm/filter/user";
    pub const DANMAKU_FILTER_ADD: &str = "/x/dm/filter/user/add";
    pub const DANMAKU_FILTER_DEL: &str = "/x/dm/filter/user/del";
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

/// 新增一条规则的请求。
///
/// 定级 [`Idempotency::Verifiable`]：服务端语义是「加一条」而不是「置为」，
/// 重发可能多出一条；但重发前**查得到**——只读的
/// [`danmaku_filter_request`] 列得出全部规则。按 ADR-013 决策一，这一档
/// 在传输层一律不重试，要不要重发由调用点在查过之后决定。
#[must_use]
pub fn danmaku_filter_add_request(filter: &str, kind: i64) -> PreparedWrite {
    PreparedWrite::form_only(
        path::DANMAKU_FILTER_ADD,
        // 字段顺序与基线一致（`type` 在前）。顺序对服务端无所谓，但它被
        // 请求快照钉住，换序会让 diff 变成噪音。
        vec![
            ("type".to_owned(), kind.to_string()),
            ("filter".to_owned(), filter.to_owned()),
        ],
        Idempotency::Verifiable,
    )
}

/// 删除一条规则的请求。
///
/// 定级 [`Idempotency::Idempotent`]：删第二次服务端返回「不存在」，最终
/// 状态一致。
#[must_use]
pub fn danmaku_filter_del_request(ids: i64) -> PreparedWrite {
    PreparedWrite::form_only(
        path::DANMAKU_FILTER_DEL,
        vec![("ids".to_owned(), ids.to_string())],
        Idempotency::Idempotent,
    )
}

/// 新增一条云端规则，返回服务端建好的那条（带 id）。
///
/// # Errors
/// 传输、状态码、信封三类失败，外加 `data.id` 缺失（见
/// [`parse_added_rule`]）。
pub async fn add_danmaku_filter<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    filter: &str,
    kind: i64,
) -> Result<FilterRule, DomainError> {
    let body = crate::client::send_write(transport, ctx, &danmaku_filter_add_request(filter, kind))
        .await?;
    parse_added_rule(&body, filter, kind)
}

/// 删除一条云端规则。
///
/// 成功时没有载荷——信封的 `code == 0` 就是全部结论。
///
/// # Errors
/// 传输、状态码、信封三类失败。
pub async fn del_danmaku_filter<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    ids: i64,
) -> Result<(), DomainError> {
    let body = crate::client::send_write(transport, ctx, &danmaku_filter_del_request(ids)).await?;
    // 只验 code：这条接口成功时**没有 data**，用 unwrap_envelope 去解会把
    // 「成功」判成协议错误。见 `envelope::check_envelope` 的说明。
    crate::envelope::check_envelope(&body)
}

/// 解析新增的结果。
///
/// `id` 是**服务端分配**的，界面之后要拿它来删，缺了这条规则就是个删不掉
/// 的孤儿——所以缺 `id` 判协议错误，而不是给个 0 糊过去。
///
/// `type` 与 `filter` 缺失时**回落到请求里的那两个值**：它们本来就是我们
/// 发过去的，服务端回显只是确认。基线在这里直接取 `data['filter']`，服务端
/// 哪天不回显就会在 Dart 侧拿到 null 再炸；这是一处刻意的分叉。
///
/// # Errors
/// 信封失败沿用统一分类；缺 `id` 为 [`ErrorKind::Protocol`]。
pub fn parse_added_rule(
    body: &str,
    requested_filter: &str,
    requested_kind: i64,
) -> Result<FilterRule, DomainError> {
    let dto: RuleDto = crate::envelope::unwrap_envelope(body)?;

    let Some(id) = dto.id else {
        return Err(DomainError::new(
            ErrorKind::Protocol,
            "新增屏蔽规则成功但没有返回 id，无法再删除它",
        ));
    };

    Ok(FilterRule {
        id,
        kind: dto.kind.unwrap_or(requested_kind),
        filter: dto.filter.unwrap_or_else(|| requested_filter.to_owned()),
    })
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

#[cfg(test)]
#[allow(clippy::expect_used)]
mod write_tests {
    use super::{
        FilterRule, danmaku_filter_add_request, danmaku_filter_del_request, kind, parse_added_rule,
        path,
    };
    use crate::http::Idempotency;
    use bili_domain::error::ErrorKind;

    /// 请求快照（ADR-013 决策六）。写操作的响应通常只有 `{"code":0}`，
    /// 冻结它证明不了什么；真正会错、而且错了要到真机上才暴露的是请求面。
    #[test]
    fn add_request_shape_is_pinned() {
        let req = danmaku_filter_add_request("剧透", kind::KEYWORD);
        assert_eq!(req.path, path::DANMAKU_FILTER_ADD);
        assert_eq!(req.form_string(), "type=0&filter=剧透");
        assert!(
            req.form.iter().all(|(k, _)| k != "csrf"),
            "csrf 不该出现在构造产物里——它由 send_write 统一补，\
             这样请求快照才不含凭据"
        );
    }

    #[test]
    fn del_request_shape_is_pinned() {
        let req = danmaku_filter_del_request(4242);
        assert_eq!(req.path, path::DANMAKU_FILTER_DEL);
        assert_eq!(req.form_string(), "ids=4242");
    }

    /// 定级是这两条接口的协议知识的一部分，钉住它：改级别要有人显式改测试。
    #[test]
    fn idempotency_levels_match_the_adr() {
        assert_eq!(
            danmaku_filter_del_request(1).idempotency,
            Idempotency::Idempotent,
            "删除是置为语义：删第二次状态一致"
        );
        assert_eq!(
            danmaku_filter_add_request("x", kind::KEYWORD).idempotency,
            Idempotency::Verifiable,
            "新增会多一条，但只读接口查得到，属于可判定档"
        );
    }

    #[test]
    fn parses_the_rule_the_server_created() {
        let body = r#"{"code":0,"message":"0","data":{"id":77,"type":1,"filter":"/^高能/"}}"#;
        let rule = parse_added_rule(body, "/^高能/", kind::PATTERN).expect("正常响应应当解析成功");
        assert_eq!(
            rule,
            FilterRule {
                id: 77,
                kind: kind::PATTERN,
                filter: "/^高能/".to_owned(),
            }
        );
    }

    #[test]
    fn falls_back_to_what_we_sent_when_the_server_does_not_echo() {
        // 基线在这里直接取 data['filter']，服务端哪天不回显就会拿到 null
        // 再在 Dart 侧炸。回显只是确认，值我们本来就有。
        let body = r#"{"code":0,"message":"0","data":{"id":88}}"#;
        let rule = parse_added_rule(body, "剧透", kind::KEYWORD).expect("缺回显不该失败");
        assert_eq!(rule.id, 88);
        assert_eq!(rule.kind, kind::KEYWORD);
        assert_eq!(rule.filter, "剧透");
    }

    #[test]
    fn a_rule_without_an_id_is_a_protocol_error() {
        // 没有 id 的规则是个删不掉的孤儿，给个 0 糊过去只会把问题推到界面。
        let body = r#"{"code":0,"message":"0","data":{"type":0,"filter":"剧透"}}"#;
        let err = parse_added_rule(body, "剧透", kind::KEYWORD).expect_err("缺 id 必须失败");
        assert_eq!(err.kind(), ErrorKind::Protocol);
    }

    #[test]
    fn a_business_error_is_carried_through() {
        let body = r#"{"code":-101,"message":"账号未登录"}"#;
        let err = parse_added_rule(body, "剧透", kind::KEYWORD).expect_err("业务失败必须是错误");
        assert_eq!(err.kind(), ErrorKind::Auth);
        assert_eq!(err.code(), Some(-101));
    }
}
