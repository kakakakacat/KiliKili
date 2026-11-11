//! 评论 REST 只读接口的 Dart 导出（S1 / issue #51）。
//!
//! 形态照 `video_page.rs` 与 `sponsor_block.rs`：typed 结构直连，失败用值
//! 不用异常，凭据与设置按请求传入（ADR-011）。
//!
//! ## 未登录不是「没传 Cookie」，是请求的一部分
//!
//! 根评论列表有两个入口函数：[`fetch_reply_list_anonymous`] 与
//! [`fetch_reply_list_paged`]。前者对应基线那条**刻意匿名**的请求——
//! shadow ban 自检要的正是匿名视角，调用点必须保证 `context.cookie` 为空。
//! 合成一个带 `is_login` 开关的函数会让这件事看起来像个可选项。
//!
//! ## `error_code` 是业务信封的 `code`
//!
//! 楼中楼的 `12022`（评论区已关闭/评论不可见）是自检区分 shadow ban 与
//! 普通失败的依据，必须原样带回。

use std::time::Duration;

use bili_api::http::RequestContext;
use bili_api::reply;
use bili_api::transport_reqwest::ReqwestTransport;
use bili_domain::error::DomainError;
use bili_domain::redact::Redacted;
use bili_domain::reply as domain;

use crate::api::init;
use crate::api::transport::TransportSettings;

/// 一条评论。
///
/// 字段比基线 `lib/models_new/reply/` 少很多：那 20 个模型里的勋章、挂件、
/// 大会员标签、等级、认证等装饰子树在迁移前就没有消费者（评论区 UI 走
/// gRPC），照搬只是把要删的代码换种语言再写一遍。要显示 REST 评论时再按需
/// 加字段——解析层加字段是向后兼容的。
#[derive(Debug, Clone)]
pub struct ReplyItem {
    pub rpid: Option<i64>,
    pub oid: Option<i64>,
    /// 评论区类型（1 = 视频稿件，17 = 动态）。
    pub kind: Option<i64>,
    pub mid: Option<i64>,
    /// 根评论的 rpid；0 表示它自己就是根评论。
    pub root: Option<i64>,
    pub parent: Option<i64>,
    pub dialog: Option<i64>,
    pub count: Option<i64>,
    pub rcount: Option<i64>,
    pub state: Option<i64>,
    pub fansgrade: Option<i64>,
    pub attr: Option<i64>,
    /// Unix 秒。
    pub ctime: Option<i64>,
    pub like: Option<i64>,
    pub action: Option<i64>,
    /// 仅自己可见。
    pub invisible: Option<bool>,
    pub message: Option<String>,
    pub member: Option<ReplyMember>,
}

/// 评论的发布者。
#[derive(Debug, Clone)]
pub struct ReplyMember {
    pub mid: Option<i64>,
    pub uname: Option<String>,
    pub avatar: Option<String>,
}

/// 游标式分页（未登录形态）。
#[derive(Debug, Clone)]
pub struct ReplyCursor {
    pub is_begin: Option<bool>,
    pub is_end: Option<bool>,
    pub prev: Option<i64>,
    pub next: Option<i64>,
    /// 下一页的 offset，原样回填给下一次请求。
    pub next_offset: Option<String>,
    pub mode: Option<i64>,
    pub mode_text: Option<String>,
    pub all_count: Option<i64>,
    pub name: Option<String>,
}

/// 页码式分页（登录形态与楼中楼）。
#[derive(Debug, Clone)]
pub struct ReplyPage {
    pub num: Option<i64>,
    pub size: Option<i64>,
    pub count: Option<i64>,
}

/// 一页根评论。`cursor` 与 `page` 各自只在对应的请求形态下有值。
#[derive(Debug, Clone)]
pub struct ReplyList {
    pub cursor: Option<ReplyCursor>,
    pub page: Option<ReplyPage>,
    pub replies: Vec<ReplyItem>,
    pub top_replies: Vec<ReplyItem>,
    pub top_upper: Option<ReplyItem>,
    pub upper_mid: Option<i64>,
}

/// 一页楼中楼。
#[derive(Debug, Clone)]
pub struct ReplyReplyList {
    pub page: Option<ReplyPage>,
    pub root: Option<ReplyItem>,
    pub upper_mid: Option<i64>,
    pub replies: Vec<ReplyItem>,
}

/// UP 主对自己稿件评论区的两个开关。
#[derive(Debug, Clone)]
pub struct ReplyInteract {
    pub up_reply_selection: InteractStatus,
    pub up_reply: InteractStatus,
}

/// 一个开关的状态与可改性。
#[derive(Debug, Clone)]
pub struct InteractStatus {
    pub can_modify: bool,
    pub status: i64,
}

/// 表情包面板里的一组表情。
#[derive(Debug, Clone)]
pub struct EmotePackage {
    pub url: Option<String>,
    /// 4 表示这一组是文字表情。
    pub kind: Option<i64>,
    pub emote: Vec<Emote>,
}

/// 一个表情。
#[derive(Debug, Clone)]
pub struct Emote {
    /// 插入正文的占位文本，如 `[doge]`。
    pub text: Option<String>,
    pub url: Option<String>,
    pub meta: Option<EmoteMeta>,
}

/// 表情的尺寸与别名。
#[derive(Debug, Clone)]
pub struct EmoteMeta {
    pub size: Option<i64>,
    pub alias: Option<String>,
}

/// 根评论列表的结果。
#[derive(Debug, Clone)]
pub struct ReplyListResult {
    pub data: Option<ReplyList>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

/// 楼中楼的结果。`error_code == 12022` 表示评论不可见。
#[derive(Debug, Clone)]
pub struct ReplyReplyListResult {
    pub data: Option<ReplyReplyList>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

/// 评论区开关状态的结果。
#[derive(Debug, Clone)]
pub struct ReplyInteractResult {
    pub data: Option<ReplyInteract>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

/// 表情包面板的结果。`data` 为空列表是正常的（未登录时服务端就给 null）。
#[derive(Debug, Clone)]
pub struct EmotePanelResult {
    pub data: Option<Vec<EmotePackage>>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

/// 一次请求所需的公共参数。
///
/// 没有 `mixin_key`/`wts`：本模块的四条接口都不做 WBI 签名。
#[derive(Debug, Clone)]
pub struct ReplyContext {
    pub base_url: String,
    /// 完整的 Cookie 头值。**匿名请求必须为 `None`**。
    pub cookie: Option<String>,
    pub headers: Vec<(String, String)>,
    pub timeout_ms: u64,
    pub transport: TransportSettings,
}

impl ReplyContext {
    fn split(self) -> (RequestContext, ReqwestTransport) {
        let transport = ReqwestTransport::new(self.transport.to_config());
        let ctx = RequestContext {
            base_url: self.base_url,
            cookie: self.cookie.map(Redacted::new),
            headers: self.headers,
            timeout: Duration::from_millis(self.timeout_ms),
        };
        (ctx, transport)
    }
}

/// 取一页根评论（**匿名**形态，`/x/v2/reply/main`）。
///
/// `sort`：0 按时间，1 按热度（基线 `ReplySortType` 的序号）。
/// `offset`：上一页的 `cursor.next_offset`，首页传空串。
pub async fn fetch_reply_list_anonymous(
    context: ReplyContext,
    oid: i64,
    kind: i64,
    offset: String,
    sort: i64,
) -> ReplyListResult {
    let (ctx, transport) = context.split();
    let joined = init::runtime()
        .spawn(async move {
            reply::fetch_reply_list_anonymous(&transport, &ctx, oid, kind, &offset, sort).await
        })
        .await;
    into_list_result(flatten(joined))
}

/// 取一页根评论（登录形态，`/x/v2/reply`）。
pub async fn fetch_reply_list_paged(
    context: ReplyContext,
    oid: i64,
    kind: i64,
    sort: i64,
    page: i64,
) -> ReplyListResult {
    let (ctx, transport) = context.split();
    let joined = init::runtime()
        .spawn(async move {
            reply::fetch_reply_list_paged(&transport, &ctx, oid, kind, sort, page).await
        })
        .await;
    into_list_result(flatten(joined))
}

/// 取一页楼中楼。`csrf` 只在登录时给。
pub async fn fetch_reply_reply_list(
    context: ReplyContext,
    oid: i64,
    root: i64,
    kind: i64,
    page: i64,
    csrf: Option<String>,
) -> ReplyReplyListResult {
    let (ctx, transport) = context.split();
    let joined = init::runtime()
        .spawn(async move {
            reply::fetch_reply_reply_list(&transport, &ctx, oid, root, kind, page, csrf.as_deref())
                .await
        })
        .await;

    match flatten(joined) {
        Ok(list) => ReplyReplyListResult {
            data: Some(ReplyReplyList {
                page: list.page.map(page_from_domain),
                root: list.root.map(ReplyItem::from_domain),
                upper_mid: list.upper_mid,
                replies: list
                    .replies
                    .into_iter()
                    .map(ReplyItem::from_domain)
                    .collect(),
            }),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => ReplyReplyListResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 取评论区开关状态。要登录态。
pub async fn fetch_reply_interaction(
    context: ReplyContext,
    oid: i64,
    kind: i64,
) -> ReplyInteractResult {
    let (ctx, transport) = context.split();
    let joined = init::runtime()
        .spawn(async move { reply::fetch_reply_interaction(&transport, &ctx, oid, kind).await })
        .await;

    match flatten(joined) {
        Ok(i) => ReplyInteractResult {
            data: Some(ReplyInteract {
                up_reply_selection: InteractStatus {
                    can_modify: i.up_reply_selection.can_modify,
                    status: i.up_reply_selection.status,
                },
                up_reply: InteractStatus {
                    can_modify: i.up_reply.can_modify,
                    status: i.up_reply.status,
                },
            }),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => ReplyInteractResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 取表情包面板。`business` 基线固定传 `reply`。
pub async fn fetch_emote_panel(context: ReplyContext, business: String) -> EmotePanelResult {
    let (ctx, transport) = context.split();
    let joined = init::runtime()
        .spawn(async move { reply::fetch_emote_panel(&transport, &ctx, &business).await })
        .await;

    match flatten(joined) {
        Ok(packages) => EmotePanelResult {
            data: Some(
                packages
                    .into_iter()
                    .map(|p| EmotePackage {
                        url: p.url,
                        kind: p.kind,
                        emote: p
                            .emote
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
                    .collect(),
            ),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => EmotePanelResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

fn into_list_result(result: Result<domain::ReplyList, DomainError>) -> ReplyListResult {
    match result {
        Ok(list) => ReplyListResult {
            data: Some(ReplyList {
                cursor: list.cursor.map(|c| ReplyCursor {
                    is_begin: c.is_begin,
                    is_end: c.is_end,
                    prev: c.prev,
                    next: c.next,
                    next_offset: c.next_offset,
                    mode: c.mode,
                    mode_text: c.mode_text,
                    all_count: c.all_count,
                    name: c.name,
                }),
                page: list.page.map(page_from_domain),
                replies: list
                    .replies
                    .into_iter()
                    .map(ReplyItem::from_domain)
                    .collect(),
                top_replies: list
                    .top_replies
                    .into_iter()
                    .map(ReplyItem::from_domain)
                    .collect(),
                top_upper: list.top_upper.map(ReplyItem::from_domain),
                upper_mid: list.upper_mid,
            }),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => ReplyListResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

fn page_from_domain(p: domain::ReplyPage) -> ReplyPage {
    ReplyPage {
        num: p.num,
        size: p.size,
        count: p.count,
    }
}

impl ReplyItem {
    fn from_domain(i: domain::ReplyItem) -> Self {
        Self {
            rpid: i.rpid,
            oid: i.oid,
            kind: i.kind,
            mid: i.mid,
            root: i.root,
            parent: i.parent,
            dialog: i.dialog,
            count: i.count,
            rcount: i.rcount,
            state: i.state,
            fansgrade: i.fansgrade,
            attr: i.attr,
            ctime: i.ctime,
            like: i.like,
            action: i.action,
            invisible: i.invisible,
            message: i.message,
            member: i.member.map(|m| ReplyMember {
                mid: m.mid,
                uname: m.uname,
                avatar: m.avatar,
            }),
        }
    }
}

/// 把 `JoinHandle` 的两层结果压平。任务 panic 不传给 Dart。
fn flatten<T>(
    joined: Result<Result<T, DomainError>, tokio::task::JoinError>,
) -> Result<T, DomainError> {
    match joined {
        Ok(inner) => inner,
        Err(e) => Err(DomainError::new(
            bili_domain::error::ErrorKind::Network,
            format!("取数任务未完成: {e}"),
        )),
    }
}
