//! 评论 REST 只读接口的请求构造与解析（S1 / issue #51）。
//!
//! 对应基线 `ReplyHttp` 的 `replyList`、`replyReplyList`、`replyInteraction`、
//! `getEmoteList`（`lib/http/reply.dart`）。写操作（点赞/点踩/置顶/举报/
//! 开关评论区）不在本模块，等 ADR-013（issue #50）。
//!
//! ## 三处与别的接口不同的地方
//!
//! 1. **根评论列表有两条请求形态，响应形状也不同**。未登录走
//!    `/x/v2/reply/main`，分页参数是一个 JSON 串 `pagination_str`，响应里
//!    有 `cursor`；登录走 `/x/v2/reply`，分页是 `pn/ps`，响应里是 `page`，
//!    **没有 `cursor`**。基线用同一个模型装两者，这里沿用（见
//!    [`bili_domain::reply::ReplyList`]），但请求构造分成两个函数——把
//!    `is_login` 作为参数塞进一个函数，调用点就得记住「哪几个参数在哪条
//!    分支下才有用」。
//! 2. **未登录形态是「刻意不带账号」**，不是「碰巧没登录」。shadow ban
//!    自检要的正是匿名视角：同一条评论，登录能看到而匿名看不到，才说明
//!    被限制了。所以 Dart 侧传进来的 `RequestContext` 必须不含 Cookie，
//!    本模块不碰凭据，也无从替它补。
//! 3. **楼中楼的失败码要带回调用点**。`12022` 表示「评论区已关闭/评论
//!    不可见」，自检据此区分 shadow ban 与普通失败。信封解包已经把业务码
//!    放进 `DomainError::code`，这里只要不吞掉它。

use bili_domain::error::{DomainError, ErrorKind};
use bili_domain::reply::{
    Emote, EmoteMeta, EmotePackage, InteractStatus, ReplyCursor, ReplyInteract, ReplyItem,
    ReplyList, ReplyMember, ReplyPage, ReplyReplyList,
};
use serde::Deserialize;

use crate::http::{HttpTransport, RequestContext};
use crate::request::PreparedRequest;

/// 接口路径。与基线 `lib/http/api.dart` 保持一致。
pub mod path {
    /// 根评论列表（登录形态）。
    pub const REPLY_LIST: &str = "/x/v2/reply";
    /// 根评论列表（未登录形态）。
    pub const REPLY_LIST_MAIN: &str = "/x/v2/reply/main";
    /// 楼中楼。
    pub const REPLY_REPLY_LIST: &str = "/x/v2/reply/reply";
    /// UP 主的评论区开关状态。
    pub const REPLY_INTERACTION: &str = "/x/v2/reply/subject/interaction-status";
    /// 表情包面板。
    pub const EMOTE_PANEL: &str = "/x/emote/user/panel/web";
}

/// 基线固定的页长（`ps`）。
const PAGE_SIZE: i64 = 20;

/// 根评论列表（未登录形态）的请求。
///
/// `sort` 是基线的 `ReplySortType` 序号（0 时间、1 热度），服务端的
/// `mode` 比它大 2（2 按时间、3 按热度）——这个 `+2` 是协议知识，留在
/// 这一侧，调用点不该知道。
///
/// `offset` 是上一页 `cursor.pagination_reply.next_offset` 的原值；首页传
/// 空串。它要包成 `{"offset":"..."}` 再作为**一个查询参数**发出，其中的
/// 双引号按 JSON 规则转义——基线 `nextOffset.replaceAll('"', '\\"')` 做的
/// 就是这件事，这里用 `serde_json` 来做，顺带把反斜杠等其它需要转义的
/// 字符也处理对。
#[must_use]
pub fn reply_list_anonymous_request(
    oid: i64,
    kind: i64,
    offset: &str,
    sort: i64,
) -> PreparedRequest {
    PreparedRequest {
        path: path::REPLY_LIST_MAIN,
        query: vec![
            ("oid".to_owned(), oid.to_string()),
            ("type".to_owned(), kind.to_string()),
            ("pagination_str".to_owned(), pagination_str(offset)),
            ("mode".to_owned(), (sort + 2).to_string()),
        ],
    }
}

/// 把 offset 包成服务端要的 JSON 串。
fn pagination_str(offset: &str) -> String {
    // 只有一个字段，手工拼 + 转义比引入一个结构体更直白；转义本身交给
    // serde_json，自己写 `replace('"', "\\\"")` 会漏掉控制字符。
    let escaped = serde_json::Value::String(offset.to_owned()).to_string();
    format!("{{\"offset\":{escaped}}}")
}

/// 根评论列表（登录形态）的请求。
#[must_use]
pub fn reply_list_paged_request(oid: i64, kind: i64, sort: i64, page: i64) -> PreparedRequest {
    PreparedRequest {
        path: path::REPLY_LIST,
        query: vec![
            ("oid".to_owned(), oid.to_string()),
            ("type".to_owned(), kind.to_string()),
            ("sort".to_owned(), sort.to_string()),
            ("pn".to_owned(), page.to_string()),
            ("ps".to_owned(), PAGE_SIZE.to_string()),
        ],
    }
}

/// 楼中楼的请求。
///
/// `csrf` 只在登录时带（基线如此）。它是只读接口却要 csrf，属于服务端的
/// 要求；真相源在 Dart 的账号体系，Rust 不存凭据。
#[must_use]
pub fn reply_reply_list_request(
    oid: i64,
    root: i64,
    kind: i64,
    page: i64,
    csrf: Option<&str>,
) -> PreparedRequest {
    let mut query = vec![
        ("oid".to_owned(), oid.to_string()),
        ("root".to_owned(), root.to_string()),
        ("pn".to_owned(), page.to_string()),
        ("type".to_owned(), kind.to_string()),
        // 基线写死 1（按时间正序），楼中楼没有排序入口。
        ("sort".to_owned(), "1".to_owned()),
    ];
    if let Some(csrf) = csrf {
        query.push(("csrf".to_owned(), csrf.to_owned()));
    }
    PreparedRequest {
        path: path::REPLY_REPLY_LIST,
        query,
    }
}

/// 评论区开关状态的请求。
#[must_use]
pub fn reply_interaction_request(oid: i64, kind: i64) -> PreparedRequest {
    PreparedRequest {
        path: path::REPLY_INTERACTION,
        query: vec![
            ("oid".to_owned(), oid.to_string()),
            ("type".to_owned(), kind.to_string()),
            // 基线传的是 double 字面量 333.1369，序列化后就是这个串。
            ("web_location".to_owned(), "333.1369".to_owned()),
        ],
    }
}

/// 表情包面板的请求。
#[must_use]
pub fn emote_panel_request(business: &str) -> PreparedRequest {
    PreparedRequest {
        path: path::EMOTE_PANEL,
        query: vec![
            ("business".to_owned(), business.to_owned()),
            ("web_location".to_owned(), "333.1245".to_owned()),
        ],
    }
}

/// 取一页根评论（未登录形态）。
///
/// # Errors
/// 传输、状态码、信封三类失败，见各自的文档。
pub async fn fetch_reply_list_anonymous<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    oid: i64,
    kind: i64,
    offset: &str,
    sort: i64,
) -> Result<ReplyList, DomainError> {
    let req = reply_list_anonymous_request(oid, kind, offset, sort);
    let body = crate::client::send(transport, ctx, &req).await?;
    parse_reply_list(&body)
}

/// 取一页根评论（登录形态）。
///
/// # Errors
/// 见 [`fetch_reply_list_anonymous`]。
pub async fn fetch_reply_list_paged<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    oid: i64,
    kind: i64,
    sort: i64,
    page: i64,
) -> Result<ReplyList, DomainError> {
    let req = reply_list_paged_request(oid, kind, sort, page);
    let body = crate::client::send(transport, ctx, &req).await?;
    parse_reply_list(&body)
}

/// 取一页楼中楼。
///
/// # Errors
/// 见 [`fetch_reply_list_anonymous`]。失败时 `DomainError::code` 带着业务
/// 码，`12022` 表示评论不可见。
pub async fn fetch_reply_reply_list<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    oid: i64,
    root: i64,
    kind: i64,
    page: i64,
    csrf: Option<&str>,
) -> Result<ReplyReplyList, DomainError> {
    let req = reply_reply_list_request(oid, root, kind, page, csrf);
    let body = crate::client::send(transport, ctx, &req).await?;
    parse_reply_reply_list(&body)
}

/// 取评论区开关状态。
///
/// # Errors
/// 见 [`fetch_reply_list_anonymous`]；少任一开关时为 `Protocol`。
pub async fn fetch_reply_interaction<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    oid: i64,
    kind: i64,
) -> Result<ReplyInteract, DomainError> {
    let req = reply_interaction_request(oid, kind);
    let body = crate::client::send(transport, ctx, &req).await?;
    parse_reply_interaction(&body)
}

/// 取表情包面板。
///
/// # Errors
/// 见 [`fetch_reply_list_anonymous`]。
pub async fn fetch_emote_panel<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    business: &str,
) -> Result<Vec<EmotePackage>, DomainError> {
    let req = emote_panel_request(business);
    let body = crate::client::send(transport, ctx, &req).await?;
    parse_emote_panel(&body)
}

// ---------------------------------------------------------------- DTO

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ReplyItemDto {
    rpid: Option<i64>,
    oid: Option<i64>,
    #[serde(rename = "type")]
    kind: Option<i64>,
    mid: Option<i64>,
    root: Option<i64>,
    parent: Option<i64>,
    dialog: Option<i64>,
    count: Option<i64>,
    rcount: Option<i64>,
    state: Option<i64>,
    fansgrade: Option<i64>,
    attr: Option<i64>,
    ctime: Option<i64>,
    like: Option<i64>,
    action: Option<i64>,
    invisible: Option<bool>,
    content: Option<ContentDto>,
    member: Option<MemberDto>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ContentDto {
    message: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct MemberDto {
    /// 服务端给的是字符串。
    mid: Option<String>,
    uname: Option<String>,
    avatar: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct CursorDto {
    is_begin: Option<bool>,
    is_end: Option<bool>,
    prev: Option<i64>,
    next: Option<i64>,
    pagination_reply: Option<PaginationReplyDto>,
    mode: Option<i64>,
    mode_text: Option<String>,
    all_count: Option<i64>,
    name: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct PaginationReplyDto {
    next_offset: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct PageDto {
    num: Option<i64>,
    size: Option<i64>,
    count: Option<i64>,
}

/// 列表接口里的 `upper`。服务端在这里只给 mid——它与评论对象长得像，
/// 但字段几乎都是空的，当成评论解析只会得到一条全 null 的记录。
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct UpperDto {
    mid: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct TopDto {
    upper: Option<ReplyItemDto>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ReplyListDto {
    cursor: Option<CursorDto>,
    page: Option<PageDto>,
    replies: Option<Vec<ReplyItemDto>>,
    top_replies: Option<Vec<ReplyItemDto>>,
    top: Option<TopDto>,
    upper: Option<UpperDto>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ReplyReplyListDto {
    page: Option<PageDto>,
    root: Option<ReplyItemDto>,
    upper: Option<UpperDto>,
    replies: Option<Vec<ReplyItemDto>>,
}

#[derive(Debug, Deserialize)]
struct InteractStatusDto {
    can_modify: bool,
    status: i64,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ReplyInteractDto {
    up_reply_selection: Option<InteractStatusDto>,
    up_reply: Option<InteractStatusDto>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct EmotePanelDto {
    packages: Option<Vec<EmotePackageDto>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct EmotePackageDto {
    url: Option<String>,
    #[serde(rename = "type")]
    kind: Option<i64>,
    emote: Option<Vec<EmoteDto>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct EmoteDto {
    text: Option<String>,
    url: Option<String>,
    meta: Option<EmoteMetaDto>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct EmoteMetaDto {
    size: Option<i64>,
    alias: Option<String>,
}

// -------------------------------------------------------------- 解析

impl ReplyItemDto {
    fn into_domain(self) -> ReplyItem {
        ReplyItem {
            rpid: self.rpid,
            oid: self.oid,
            kind: self.kind,
            mid: self.mid,
            root: self.root,
            parent: self.parent,
            dialog: self.dialog,
            count: self.count,
            rcount: self.rcount,
            state: self.state,
            fansgrade: self.fansgrade,
            attr: self.attr,
            ctime: self.ctime,
            like: self.like,
            action: self.action,
            invisible: self.invisible,
            message: self.content.and_then(|c| c.message),
            member: self.member.map(|m| ReplyMember {
                // 解析不出来就当没有：一个畸形的 mid 不值得作废整条评论。
                mid: m.mid.and_then(|s| s.parse().ok()),
                uname: m.uname,
                avatar: m.avatar,
            }),
        }
    }
}

fn items(list: Option<Vec<ReplyItemDto>>) -> Vec<ReplyItem> {
    list.unwrap_or_default()
        .into_iter()
        .map(ReplyItemDto::into_domain)
        .collect()
}

/// 解析根评论列表。两条请求形态共用——它们的差别在 `cursor` 与 `page`
/// 哪个有值，而不是别的。
///
/// # Errors
/// 信封失败沿用统一分类。`replies` 缺失按空列表处理：评论区为空与
/// 「这一页没有评论」在基线里都是 `replies == null`，不是错误。
pub fn parse_reply_list(body: &str) -> Result<ReplyList, DomainError> {
    let dto: ReplyListDto = crate::envelope::unwrap_envelope(body)?;
    Ok(ReplyList {
        cursor: dto.cursor.map(|c| ReplyCursor {
            is_begin: c.is_begin,
            is_end: c.is_end,
            prev: c.prev,
            next: c.next,
            next_offset: c.pagination_reply.and_then(|p| p.next_offset),
            mode: c.mode,
            mode_text: c.mode_text,
            all_count: c.all_count,
            name: c.name,
        }),
        page: dto.page.map(|p| ReplyPage {
            num: p.num,
            size: p.size,
            count: p.count,
        }),
        replies: items(dto.replies),
        top_replies: items(dto.top_replies),
        top_upper: dto.top.and_then(|t| t.upper).map(ReplyItemDto::into_domain),
        upper_mid: dto.upper.and_then(|u| u.mid),
    })
}

/// 解析楼中楼。
///
/// # Errors
/// 见 [`parse_reply_list`]。
pub fn parse_reply_reply_list(body: &str) -> Result<ReplyReplyList, DomainError> {
    let dto: ReplyReplyListDto = crate::envelope::unwrap_envelope(body)?;
    Ok(ReplyReplyList {
        page: dto.page.map(|p| ReplyPage {
            num: p.num,
            size: p.size,
            count: p.count,
        }),
        root: dto.root.map(ReplyItemDto::into_domain),
        upper_mid: dto.upper.and_then(|u| u.mid),
        replies: items(dto.replies),
    })
}

/// 解析评论区开关状态。
///
/// # Errors
/// 少任一开关时为 `Protocol`：基线在这里是 `InteractStatus.fromJson(null)`
/// 抛异常再被 `catch` 转成错误文案，结果一样，但这里能说清缺了什么。
pub fn parse_reply_interaction(body: &str) -> Result<ReplyInteract, DomainError> {
    let dto: ReplyInteractDto = crate::envelope::unwrap_envelope(body)?;
    let missing = |what: &str| {
        DomainError::new(
            ErrorKind::Protocol,
            format!("响应里没有 {what}，无法判断评论区开关状态"),
        )
    };
    let selection = dto
        .up_reply_selection
        .ok_or_else(|| missing("up_reply_selection"))?;
    let reply = dto.up_reply.ok_or_else(|| missing("up_reply"))?;
    Ok(ReplyInteract {
        up_reply_selection: InteractStatus {
            can_modify: selection.can_modify,
            status: selection.status,
        },
        up_reply: InteractStatus {
            can_modify: reply.can_modify,
            status: reply.status,
        },
    })
}

/// 解析表情包面板。
///
/// # Errors
/// 见 [`parse_reply_list`]。`packages` 为 null（未登录时服务端就这么给）
/// 按空列表处理，与基线一致——面板会显示空，而不是报错。
pub fn parse_emote_panel(body: &str) -> Result<Vec<EmotePackage>, DomainError> {
    let dto: EmotePanelDto = crate::envelope::unwrap_envelope(body)?;
    Ok(dto
        .packages
        .unwrap_or_default()
        .into_iter()
        .map(|p| EmotePackage {
            url: p.url,
            kind: p.kind,
            emote: p
                .emote
                .unwrap_or_default()
                .into_iter()
                .map(|e| Emote {
                    text: e.text,
                    url: e.url,
                    meta: e.meta.map(|m| EmoteMeta {
                        size: m.size,
                        alias: m.alias,
                    }),
                })
                .collect(),
        })
        .collect())
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn anonymous_request_wraps_offset_into_json() {
        let req = reply_list_anonymous_request(2, 1, "CAEiAggC", 1);
        assert_eq!(req.path, path::REPLY_LIST_MAIN);
        let query = req.query_string();
        assert!(
            query.contains(r#"pagination_str={"offset":"CAEiAggC"}"#),
            "{query}"
        );
        // sort 1（热度）对应服务端 mode 3。
        assert!(query.contains("mode=3"), "{query}");
    }

    #[test]
    fn offset_with_quotes_is_escaped() {
        // 基线用 replaceAll('"', '\\"') 处理这种 offset；这里必须等价。
        let req = reply_list_anonymous_request(2, 1, r#"a"b"#, 0);
        let (_, value) = req
            .query
            .iter()
            .find(|(k, _)| k == "pagination_str")
            .expect("必须有 pagination_str");
        assert_eq!(value, r#"{"offset":"a\"b"}"#);
        // 整串仍是合法 JSON——服务端要把它解回来。
        let parsed: serde_json::Value = serde_json::from_str(value).expect("必须是合法 JSON");
        assert_eq!(parsed["offset"], r#"a"b"#);
    }

    #[test]
    fn empty_offset_is_an_empty_string_not_a_missing_key() {
        let req = reply_list_anonymous_request(2, 1, "", 0);
        let (_, value) = req
            .query
            .iter()
            .find(|(k, _)| k == "pagination_str")
            .expect("必须有 pagination_str");
        assert_eq!(value, r#"{"offset":""}"#);
    }

    #[test]
    fn paged_request_uses_the_baseline_page_size() {
        let req = reply_list_paged_request(2, 1, 1, 3);
        assert_eq!(req.path, path::REPLY_LIST);
        assert!(req.query_string().contains("ps=20"));
        assert!(req.query_string().contains("pn=3"));
    }

    #[test]
    fn csrf_is_only_sent_when_given() {
        let anonymous = reply_reply_list_request(2, 9, 1, 1, None);
        assert!(!anonymous.query_string().contains("csrf"));
        let logged_in = reply_reply_list_request(2, 9, 1, 1, Some("token"));
        assert!(logged_in.query_string().contains("csrf=token"));
    }

    #[test]
    fn missing_replies_is_an_empty_list_not_an_error() {
        let list =
            parse_reply_list(r#"{"code":0,"message":"0","data":{}}"#).expect("空评论区不是错误");
        assert!(list.replies.is_empty());
        assert!(list.cursor.is_none());
    }

    #[test]
    fn business_code_is_carried_out_of_the_envelope() {
        // 12022 是「评论不可见」，自检靠它区分 shadow ban 与普通失败。
        let err = parse_reply_reply_list(r#"{"code":12022,"message":"评论区已关闭","data":null}"#)
            .expect_err("非 0 码必须是错误");
        assert_eq!(err.code(), Some(12022));
    }

    #[test]
    fn member_mid_is_parsed_from_its_string_form() {
        let body = r#"{"code":0,"message":"0","data":{"replies":[
            {"rpid":1,"member":{"mid":"42","uname":"u"},"content":{"message":"m"}},
            {"rpid":2,"member":{"mid":"not-a-number"}}
        ]}}"#;
        let list = parse_reply_list(body).expect("应解析成功");
        assert_eq!(
            list.replies[0].member.as_ref().and_then(|m| m.mid),
            Some(42)
        );
        assert_eq!(
            list.replies[1].member.as_ref().and_then(|m| m.mid),
            None,
            "畸形的 mid 只丢这一个字段，不丢整条评论"
        );
        assert_eq!(list.replies[1].rpid, Some(2));
    }

    #[test]
    fn null_emote_packages_parse_as_empty() {
        // 未登录时服务端就是这么给的（已实测）。
        let packages = parse_emote_panel(r#"{"code":0,"message":"0","data":{"packages":null}}"#)
            .expect("不是错误");
        assert!(packages.is_empty());
    }

    #[test]
    fn interaction_without_a_switch_is_a_protocol_error() {
        let err = parse_reply_interaction(
            r#"{"code":0,"message":"0","data":{"up_reply":{"can_modify":true,"status":1}}}"#,
        )
        .expect_err("缺开关必须是错误");
        assert_eq!(err.kind(), ErrorKind::Protocol);
        assert!(err.detail().contains("up_reply_selection"));
    }
}
