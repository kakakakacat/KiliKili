//! 消息中心与私信设置的请求构造与解析（S12 / issue #63）。
//!
//! 对应基线 `lib/http/msg.dart` 里属于**消息域**的那 19 条。
//!
//! 那个文件一共 24 条（另有一段早被注释掉的 `sendMsg`），账算得清：
//! **19 条迁到这里**（十条只读 + 九条写），**2 条直接删**（`createTextDynamic`
//! 与 `setTop`，全仓库无调用点；会话置顶早已改走 gRPC 的 `pinSession`），
//! **3 条留在 Dart**（见下面「刻意没迁的」）。私信的收发在 gRPC 那一侧，
//! 不在本模块。
//!
//! ## 幂等定级表
//!
//! ADR-013 要求「哪些接口允许重试、哪些一次都不能重」写成显式表格，而不是
//! 靠默认策略兜底。这一片有九条写操作：
//!
//! | 接口 | 定级 | 为什么 |
//! | --- | --- | --- |
//! | `update_cursor`（系统通知已读游标） | 幂等 | 置为语义：写同一个 cursor 两次结果一样 |
//! | `update_ack`（私信已读位点） | 幂等 | 同上，`ack_seqno` 是位点不是增量 |
//! | `del_msgfeed`（删一条消息中心的条目） | 幂等 | 删第二次服务端回「不存在」，最终状态一致 |
//! | `del_notify_list`（删系统通知） | 幂等 | 同上 |
//! | `remove_session`（删会话） | 幂等 | 同上 |
//! | `notice`（某条赞的通知开关） | 幂等 | `notice_state` 是目标状态 |
//! | `set_msg_dnd`（免打扰） | 幂等 | `setting` 是目标状态 |
//! | `set_push_ss`（内容推送开关） | 幂等 | 同上 |
//! | `report/add`（举报） | **不可重发** | 见下 |
//!
//! **举报是这一片唯一一条 `Idempotency::Unsafe`。** 它没有幂等键，服务端
//! 每收到一次就立一条工单；重发的代价落在被举报的人身上，而且界面查不到
//! 「上一次到底立没立」——没有「我的举报记录」这个接口。超时重发它，用户
//! 看到的是「举报成功」，被举报者收到的是两条。
//!
//! 私信发送（`sendMsg`）同样不可重发，但它走 gRPC，定级写在那一侧；基线
//! 的 REST 版本早已被注释掉，这次一并删除。
//!
//! ## 刻意没迁的四条
//!
//! `msg.dart` 里还有 `uploadImage`、`uploadBfs`、`createTextDynamic`、
//! `removeDynamic`。它们**不属于消息域**，是历史上顺手放进这个文件的：
//!
//! - `uploadImage` / `uploadBfs` 是 multipart 文件上传，还要带
//!   `CancelToken`。ADR-013 决策一只给传输层定了 `Form` 与 `Json` 两种体，
//!   multipart 与上传取消是一项**新的传输能力**，该有自己的决策而不是
//!   夹带在本片里。
//! - `removeDynamic` 属于动态域，归 S5 续单（#150）的 `lib/http/dynamics.dart`。
//! - `createTextDynamic` 全仓库无调用点，随本片直接删除。
//!
//! 前两条留在 Dart，去向见 issue #193（multipart 与取消要先有决策）；
//! `removeDynamic` 归 S5 续单 #150。

use bili_domain::error::DomainError;
use bili_domain::msg::{
    AtFeed, AtFeedContent, AtFeedItem, FeedCursor, FeedUser, ImUserInfo, LikeDetail,
    LikeDetailCard, LikeDetailItem, LikeDetailPage, LikeFeed, LikeFeedContent, LikeFeedItem,
    LikeLatest, LikeTotal, MsgDndSetting, MsgFeedUnread, ReplyFeed, ReplyFeedContent,
    ReplyFeedItem, SessionSs, SingleUnread, SysMsgItem,
};
use serde::Deserialize;

use crate::http::{HttpTransport, Idempotency, RequestContext};
use crate::request::{CsrfIn, PreparedRequest, PreparedWrite, WriteBody};

/// 接口路径。与基线 `lib/http/api.dart` 保持一致。
///
/// **这些是路径不是整条 URL**：这一片横跨三个域名（`api.bilibili.com`、
/// `api.vc.bilibili.com`、`message.bilibili.com`），基线把域名拼进了常量，
/// 这里按 `RequestContext::base_url` 由调用方给出。哪条走哪个域名见
/// 下面每个常量的注释——写错域名的表现是 404 或 -403，不是「参数不对」，
/// 排查时最容易往参数上想。
pub mod path {
    /// `api.bilibili.com`。回复我的。
    pub const MSG_FEED_REPLY: &str = "/x/msgfeed/reply";
    /// `api.bilibili.com`。@我的。
    pub const MSG_FEED_AT: &str = "/x/msgfeed/at";
    /// `api.bilibili.com`。收到的赞。
    pub const MSG_FEED_LIKE: &str = "/x/msgfeed/like";
    /// `api.bilibili.com`。某条赞的明细。
    pub const MSG_LIKE_DETAIL: &str = "/x/msgfeed/like_detail";
    /// `api.bilibili.com`。消息中心四个 Tab 的未读数。
    pub const MSG_FEED_UNREAD: &str = "/x/msgfeed/unread";
    /// `api.bilibili.com`。删一条消息中心的条目（写）。
    pub const DEL_MSG_FEED: &str = "/x/msgfeed/del";
    /// `api.bilibili.com`。某条赞的通知开关（写）。
    pub const MSG_SET_NOTICE: &str = "/x/msgfeed/notice";

    /// `message.bilibili.com`。系统通知列表。
    pub const SYS_NOTIFY_LIST: &str = "/x/sys-msg/query_notify_list";
    /// `message.bilibili.com`。系统通知已读游标（写，但基线发的是 GET）。
    pub const SYS_UPDATE_CURSOR: &str = "/x/sys-msg/update_cursor";
    /// `message.bilibili.com`。删系统通知（写）。
    pub const DEL_SYS_MSG: &str = "/x/sys-msg/del_notify_list";

    /// `api.vc.bilibili.com`。私信未读数。
    pub const SINGLE_UNREAD: &str = "/session_svr/v1/session_svr/single_unread";
    /// `api.vc.bilibili.com`。私信已读位点（写，但基线发的是 GET）。
    pub const UPDATE_ACK: &str = "/session_svr/v1/session_svr/update_ack";
    /// `api.vc.bilibili.com`。删会话（写）。
    pub const REMOVE_SESSION: &str = "/session_svr/v1/session_svr/remove_session";
    /// `api.vc.bilibili.com`。IM 用户资料。
    pub const IM_USER_INFOS: &str = "/x/im/user_infos";
    /// `api.vc.bilibili.com`。会话设置。
    pub const GET_SESSION_SS: &str = "/link_setting/v1/link_setting/get_session_ss";
    /// `api.vc.bilibili.com`。免打扰设置。
    pub const GET_MSG_DND: &str = "/link_setting/v1/link_setting/get_msg_dnd";
    /// `api.vc.bilibili.com`。设置免打扰（写）。
    pub const SET_MSG_DND: &str = "/link_setting/v1/link_setting/set_msg_dnd";
    /// `api.vc.bilibili.com`。设置内容推送（写）。
    pub const SET_PUSH_SS: &str = "/link_setting/v1/link_setting/set_push_ss";
    /// `api.vc.bilibili.com`。举报（写，不可重发）。
    pub const IM_REPORT: &str = "/x/bplus/im/report/add";
}

/// 三条 feed 共用的那组客户端标识。
///
/// 抽出来不是为了少打字，是因为**漏掉其中一个的表现是返回空列表而不是
/// 报错**——服务端把它当成另一种客户端，给了一份没有内容的响应。
///
/// 只覆盖三条 feed。赞的明细、系统通知、两处未读数各自的组合都略有不同
/// （明细页的字段顺序不一样、系统通知没有 `platform`、未读数还要
/// `unread_type`），硬套同一个函数等于给它加一串开关，而那几处本来就只用
/// 一次。按基线各写各的。
fn web_client_params() -> Vec<(String, String)> {
    vec![
        ("platform".to_owned(), "web".to_owned()),
        ("mobi_app".to_owned(), "web".to_owned()),
        ("build".to_owned(), "0".to_owned()),
    ]
}

/// 消息中心的 `web_location`。基线三条 feed 与明细页都用这个值。
const WEB_LOCATION_FEED: &str = "333.40164";
/// 未读数两条接口用的是另一个值。不是笔误——`web_location` 标识的是
/// 「这个请求从哪个页面发出」，首页的红点与消息中心页不是同一处。
const WEB_LOCATION_UNREAD: &str = "333.1365";

// ======================================================== 三条 feed

/// 构造「回复我的」请求。
///
/// `cursor`/`cursor_time` 同时为 `None` 时取第一页。两个都给才是翻页：
/// 服务端要 id 与时间一起定位，只给 id 会从头开始。
#[must_use]
pub fn reply_feed_request(cursor: Option<i64>, cursor_time: Option<i64>) -> PreparedRequest {
    let mut query = Vec::new();
    if let Some(id) = cursor {
        query.push(("id".to_owned(), id.to_string()));
    }
    if let Some(t) = cursor_time {
        query.push(("reply_time".to_owned(), t.to_string()));
    }
    query.extend(web_client_params());
    query.push(("web_location".to_owned(), WEB_LOCATION_FEED.to_owned()));
    PreparedRequest {
        path: path::MSG_FEED_REPLY,
        query,
    }
}

/// 构造「@我的」请求。游标字段名是 `at_time`，不是 `reply_time`。
#[must_use]
pub fn at_feed_request(cursor: Option<i64>, cursor_time: Option<i64>) -> PreparedRequest {
    let mut query = Vec::new();
    if let Some(id) = cursor {
        query.push(("id".to_owned(), id.to_string()));
    }
    if let Some(t) = cursor_time {
        query.push(("at_time".to_owned(), t.to_string()));
    }
    query.extend(web_client_params());
    query.push(("web_location".to_owned(), WEB_LOCATION_FEED.to_owned()));
    PreparedRequest {
        path: path::MSG_FEED_AT,
        query,
    }
}

/// 构造「收到的赞」请求。游标字段名是 `like_time`。
#[must_use]
pub fn like_feed_request(cursor: Option<i64>, cursor_time: Option<i64>) -> PreparedRequest {
    let mut query = Vec::new();
    if let Some(id) = cursor {
        query.push(("id".to_owned(), id.to_string()));
    }
    if let Some(t) = cursor_time {
        query.push(("like_time".to_owned(), t.to_string()));
    }
    query.extend(web_client_params());
    query.push(("web_location".to_owned(), WEB_LOCATION_FEED.to_owned()));
    PreparedRequest {
        path: path::MSG_FEED_LIKE,
        query,
    }
}

/// 构造「某条赞的明细」请求。
///
/// `card_id` 在基线是 `Object`——服务端对它既可能回整数也可能回字符串
/// （见 #166 那一类问题），所以这里按**字符串**传递，由调用方原样转交。
#[must_use]
pub fn like_detail_request(card_id: &str, pn: i64, last_mid: &str) -> PreparedRequest {
    let mut query = vec![
        ("card_id".to_owned(), card_id.to_owned()),
        ("pn".to_owned(), pn.to_string()),
        ("last_mid".to_owned(), last_mid.to_owned()),
        ("platform".to_owned(), "web".to_owned()),
        ("build".to_owned(), "0".to_owned()),
        ("mobi_app".to_owned(), "web".to_owned()),
    ];
    query.push(("web_location".to_owned(), WEB_LOCATION_FEED.to_owned()));
    PreparedRequest {
        path: path::MSG_LIKE_DETAIL,
        query,
    }
}

/// 取「回复我的」一页。
///
/// # Errors
/// 传输、状态码、信封三类失败。
pub async fn fetch_reply_feed<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    cursor: Option<i64>,
    cursor_time: Option<i64>,
) -> Result<ReplyFeed, DomainError> {
    let body =
        crate::client::send(transport, ctx, &reply_feed_request(cursor, cursor_time)).await?;
    parse_reply_feed(&body)
}

/// 取「@我的」一页。
///
/// # Errors
/// 同 [`fetch_reply_feed`]。
pub async fn fetch_at_feed<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    cursor: Option<i64>,
    cursor_time: Option<i64>,
) -> Result<AtFeed, DomainError> {
    let body = crate::client::send(transport, ctx, &at_feed_request(cursor, cursor_time)).await?;
    parse_at_feed(&body)
}

/// 取「收到的赞」一页。
///
/// # Errors
/// 同 [`fetch_reply_feed`]。
pub async fn fetch_like_feed<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    cursor: Option<i64>,
    cursor_time: Option<i64>,
) -> Result<LikeFeed, DomainError> {
    let body = crate::client::send(transport, ctx, &like_feed_request(cursor, cursor_time)).await?;
    parse_like_feed(&body)
}

/// 取某条赞的明细。
///
/// # Errors
/// 同 [`fetch_reply_feed`]。
pub async fn fetch_like_detail<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    card_id: &str,
    pn: i64,
    last_mid: &str,
) -> Result<LikeDetail, DomainError> {
    let body =
        crate::client::send(transport, ctx, &like_detail_request(card_id, pn, last_mid)).await?;
    parse_like_detail(&body)
}

// ===================================================== 系统通知与未读数

/// 构造系统通知列表请求。走 `message.bilibili.com`。
#[must_use]
pub fn sys_notify_request(cursor: Option<i64>, page_size: i64) -> PreparedRequest {
    let mut query = Vec::new();
    if let Some(c) = cursor {
        query.push(("cursor".to_owned(), c.to_string()));
    }
    query.push(("page_size".to_owned(), page_size.to_string()));
    query.push(("mobi_app".to_owned(), "web".to_owned()));
    query.push(("build".to_owned(), "0".to_owned()));
    query.push(("web_location".to_owned(), WEB_LOCATION_FEED.to_owned()));
    PreparedRequest {
        path: path::SYS_NOTIFY_LIST,
        query,
    }
}

/// 构造私信未读数请求。走 `api.vc.bilibili.com`。
#[must_use]
pub fn single_unread_request() -> PreparedRequest {
    PreparedRequest {
        path: path::SINGLE_UNREAD,
        query: vec![
            ("build".to_owned(), "0".to_owned()),
            ("mobi_app".to_owned(), "web".to_owned()),
            ("unread_type".to_owned(), "0".to_owned()),
            ("web_location".to_owned(), WEB_LOCATION_UNREAD.to_owned()),
        ],
    }
}

/// 构造消息中心未读数请求。
#[must_use]
pub fn feed_unread_request() -> PreparedRequest {
    PreparedRequest {
        path: path::MSG_FEED_UNREAD,
        query: vec![
            ("build".to_owned(), "0".to_owned()),
            ("mobi_app".to_owned(), "web".to_owned()),
            ("web_location".to_owned(), WEB_LOCATION_UNREAD.to_owned()),
        ],
    }
}

/// 取系统通知列表。
///
/// 返回空列表与服务端明确回 `null` 在这里不区分——基线对这条写的是
/// `(data as List?)?.map(...)`，两种都走到「没有更多」。
///
/// # Errors
/// 传输、状态码、信封三类失败。
pub async fn fetch_sys_notify<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    cursor: Option<i64>,
    page_size: i64,
) -> Result<Vec<SysMsgItem>, DomainError> {
    let body = crate::client::send(transport, ctx, &sys_notify_request(cursor, page_size)).await?;
    parse_sys_notify(&body)
}

/// 取私信未读数。
///
/// # Errors
/// 同上。
pub async fn fetch_single_unread<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
) -> Result<SingleUnread, DomainError> {
    let body = crate::client::send(transport, ctx, &single_unread_request()).await?;
    parse_single_unread(&body)
}

/// 取消息中心未读数。
///
/// # Errors
/// 同上。
pub async fn fetch_feed_unread<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
) -> Result<MsgFeedUnread, DomainError> {
    let body = crate::client::send(transport, ctx, &feed_unread_request()).await?;
    parse_feed_unread(&body)
}

// ================================================== 私信会话的只读设置

/// 这三条只读接口为什么要带 csrf。
///
/// `api.vc.bilibili.com` 的 `link_setting` 与 `im` 这两组接口即使是 GET
/// 也校验 csrf——不带就回 `-111`。这是服务端的既有行为，不是我们多发的；
/// 基线同样在这三条 GET 上带了 `csrf_token` 与 `csrf`。
///
/// 因此它们的 csrf 由**调用方显式传入**，而不是走 [`CsrfIn`]——那套只在
/// 写路径（`send_write`）上。与 `user_list::fetch_black_list` 同一形态。
fn vc_read_params(csrf: &str) -> Vec<(String, String)> {
    vec![
        ("build".to_owned(), "0".to_owned()),
        ("mobi_app".to_owned(), "web".to_owned()),
        ("csrf_token".to_owned(), csrf.to_owned()),
        ("csrf".to_owned(), csrf.to_owned()),
    ]
}

/// 构造 IM 用户资料请求。`uids` 是逗号分隔的 mid 串。
#[must_use]
pub fn im_user_infos_request(uids: &str, csrf: &str) -> PreparedRequest {
    let mut query = vec![("uids".to_owned(), uids.to_owned())];
    query.extend(vc_read_params(csrf));
    PreparedRequest {
        path: path::IM_USER_INFOS,
        query,
    }
}

/// 构造会话设置请求。
#[must_use]
pub fn session_ss_request(talker_uid: i64, csrf: &str) -> PreparedRequest {
    let mut query = vec![("talker_uid".to_owned(), talker_uid.to_string())];
    query.extend(vc_read_params(csrf));
    PreparedRequest {
        path: path::GET_SESSION_SS,
        query,
    }
}

/// 构造免打扰设置请求。
///
/// `own_uid` 是**自己的** mid，`uids_str` 是要查的那些人。两个都给错方向
/// 的话服务端照样回 200 和一份空设置，所以参数名在这里写死而不是由调用点拼。
#[must_use]
pub fn msg_dnd_request(own_uid: i64, uids_str: &str, csrf: &str) -> PreparedRequest {
    let mut query = vec![
        ("own_uid".to_owned(), own_uid.to_string()),
        ("uids_str".to_owned(), uids_str.to_owned()),
    ];
    query.extend(vc_read_params(csrf));
    PreparedRequest {
        path: path::GET_MSG_DND,
        query,
    }
}

/// 取 IM 用户资料。
///
/// # Errors
/// 传输、状态码、信封三类失败。
pub async fn fetch_im_user_infos<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    uids: &str,
    csrf: &str,
) -> Result<Vec<ImUserInfo>, DomainError> {
    let body = crate::client::send(transport, ctx, &im_user_infos_request(uids, csrf)).await?;
    parse_im_user_infos(&body)
}

/// 取会话设置。
///
/// # Errors
/// 同上。
pub async fn fetch_session_ss<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    talker_uid: i64,
    csrf: &str,
) -> Result<SessionSs, DomainError> {
    let body = crate::client::send(transport, ctx, &session_ss_request(talker_uid, csrf)).await?;
    parse_session_ss(&body)
}

/// 取免打扰设置。
///
/// # Errors
/// 同上。
pub async fn fetch_msg_dnd<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    own_uid: i64,
    uids_str: &str,
    csrf: &str,
) -> Result<Vec<MsgDndSetting>, DomainError> {
    let body =
        crate::client::send(transport, ctx, &msg_dnd_request(own_uid, uids_str, csrf)).await?;
    parse_msg_dnd(&body)
}

// ============================================================= 写操作

/// 构造「更新系统通知已读游标」请求。
///
/// **这条是写操作，但基线发的是 GET**，参数在查询串上。照搬——请求面要与
/// 基线一致，把它改成 POST 是一次没人验证过的协议改动。因此它走
/// [`PreparedRequest`] 而不是 [`PreparedWrite`]，csrf 由调用方传入。
///
/// 定级上它等价于 [`Idempotency::Idempotent`]：`cursor` 是位点不是增量。
/// 但 GET 路径的重试规则由 [`crate::http::should_retry`] 按只读处理，
/// 本来就会重试，这里不需要额外声明。
#[must_use]
pub fn sys_update_cursor_request(cursor: i64, csrf: &str) -> PreparedRequest {
    PreparedRequest {
        path: path::SYS_UPDATE_CURSOR,
        query: vec![
            ("csrf".to_owned(), csrf.to_owned()),
            ("cursor".to_owned(), cursor.to_string()),
        ],
    }
}

/// 构造「私信已读位点」请求。
///
/// 与上一条同理是 GET；区别是它**要 WBI 签名**，而且 `csrf_token`/`csrf`
/// 本身参与签名——所以 csrf 必须在签名前就进参数表，不能事后补。这是
/// ADR-013 决策四「csrf 只在 `send_write` 注入」的一处显式例外：协议把
/// 凭据放进了签名覆盖的范围，事后补的话 `w_rid` 对不上，服务端回的是
/// 业务错误码，极难反推。
#[must_use]
pub fn ack_session_request(
    talker_id: i64,
    ack_seqno: i64,
    csrf: &str,
    mixin_key: &str,
    wts: i64,
) -> PreparedRequest {
    let mut query = vec![
        ("talker_id".to_owned(), talker_id.to_string()),
        ("session_type".to_owned(), "1".to_owned()),
        ("ack_seqno".to_owned(), ack_seqno.to_string()),
        ("build".to_owned(), "0".to_owned()),
        ("mobi_app".to_owned(), "web".to_owned()),
        ("csrf_token".to_owned(), csrf.to_owned()),
        ("csrf".to_owned(), csrf.to_owned()),
        ("wts".to_owned(), wts.to_string()),
    ];
    let w_rid = bili_protocol::wbi::sign(&query, mixin_key);
    query.push(("w_rid".to_owned(), w_rid));
    PreparedRequest {
        path: path::UPDATE_ACK,
        query,
    }
}

/// 构造「删除会话」请求。WBI 签名的**表单体**。
///
/// 仓库里第一条签名写请求。签名范围与 [`ack_session_request`] 同样覆盖
/// csrf，理由见那里。
#[must_use]
pub fn remove_session_request(
    talker_id: i64,
    csrf: &str,
    mixin_key: &str,
    wts: i64,
) -> PreparedWrite {
    let mut form = vec![
        ("talker_id".to_owned(), talker_id.to_string()),
        ("session_type".to_owned(), "1".to_owned()),
        ("build".to_owned(), "0".to_owned()),
        ("mobi_app".to_owned(), "web".to_owned()),
        ("csrf_token".to_owned(), csrf.to_owned()),
        ("csrf".to_owned(), csrf.to_owned()),
        ("wts".to_owned(), wts.to_string()),
    ];
    let w_rid = bili_protocol::wbi::sign(&form, mixin_key);
    form.push(("w_rid".to_owned(), w_rid));

    PreparedWrite {
        path: path::REMOVE_SESSION,
        query: Vec::new(),
        body: WriteBody::Form(form),
        // csrf 已经在表单里并被签名覆盖，不能再补一份。
        csrf: CsrfIn::None,
        idempotency: Idempotency::Idempotent,
    }
}

/// 构造「删除消息中心条目」请求。
///
/// `tp` 是 Tab 类型（基线用 0/1/2 对应赞/@/回复），`id` 是条目 id。
#[must_use]
pub fn del_msg_feed_request(tp: i64, id: &str) -> PreparedWrite {
    PreparedWrite {
        path: path::DEL_MSG_FEED,
        query: Vec::new(),
        body: WriteBody::Form(vec![
            ("tp".to_owned(), tp.to_string()),
            ("id".to_owned(), id.to_owned()),
            ("build".to_owned(), "0".to_owned()),
            ("mobi_app".to_owned(), "web".to_owned()),
        ]),
        csrf: CsrfIn::BodyWithToken,
        idempotency: Idempotency::Idempotent,
    }
}

/// 构造「删除系统通知」请求。
///
/// 这条形状特别：**JSON 体**，而且查询串里还要再带一份 `csrf` 与
/// `mobi_app=android`。`mobi_app` 在体与查询串里都是 `android` 而不是
/// `web`——基线如此，这条接口只认 APP 端标识。
#[must_use]
pub fn del_sys_msg_request(id: &str) -> PreparedWrite {
    PreparedWrite {
        path: path::DEL_SYS_MSG,
        query: vec![("mobi_app".to_owned(), "android".to_owned())],
        body: WriteBody::Json(serde_json::json!({
            "ids": [id],
            "station_ids": [],
            "type": 4,
            "mobi_app": "android",
        })),
        csrf: CsrfIn::BodyAndQuery,
        idempotency: Idempotency::Idempotent,
    }
}

/// 构造「某条赞的通知开关」请求。`notice_state` 是目标状态，不是切换。
#[must_use]
pub fn set_notice_request(id: &str, notice_state: i64) -> PreparedWrite {
    PreparedWrite {
        path: path::MSG_SET_NOTICE,
        query: Vec::new(),
        body: WriteBody::Form(vec![
            ("mobi_app".to_owned(), "web".to_owned()),
            ("platform".to_owned(), "web".to_owned()),
            ("tp".to_owned(), "0".to_owned()),
            ("id".to_owned(), id.to_owned()),
            ("notice_state".to_owned(), notice_state.to_string()),
            ("build".to_owned(), "0".to_owned()),
        ]),
        csrf: CsrfIn::BodyWithToken,
        idempotency: Idempotency::Idempotent,
    }
}

/// 构造「免打扰」请求。
///
/// `uid` 是自己的 mid，`dnd_uid` 是对方的。与 [`msg_dnd_request`] 的
/// `own_uid`/`uids_str` 是同一对方向，但字段名不一样——这是服务端的命名，
/// 不是笔误。
#[must_use]
pub fn set_msg_dnd_request(uid: i64, setting: i64, dnd_uid: i64) -> PreparedWrite {
    PreparedWrite {
        path: path::SET_MSG_DND,
        query: Vec::new(),
        body: WriteBody::Form(vec![
            ("uid".to_owned(), uid.to_string()),
            ("setting".to_owned(), setting.to_string()),
            ("dnd_uid".to_owned(), dnd_uid.to_string()),
            ("build".to_owned(), "0".to_owned()),
            ("mobi_app".to_owned(), "web".to_owned()),
        ]),
        csrf: CsrfIn::BodyWithToken,
        idempotency: Idempotency::Idempotent,
    }
}

/// 构造「内容推送开关」请求。
#[must_use]
pub fn set_push_ss_request(setting: i64, talker_uid: i64) -> PreparedWrite {
    PreparedWrite {
        path: path::SET_PUSH_SS,
        query: Vec::new(),
        body: WriteBody::Form(vec![
            ("setting".to_owned(), setting.to_string()),
            ("talker_uid".to_owned(), talker_uid.to_string()),
            ("build".to_owned(), "0".to_owned()),
            ("mobi_app".to_owned(), "web".to_owned()),
        ]),
        csrf: CsrfIn::BodyWithToken,
        idempotency: Idempotency::Idempotent,
    }
}

/// 构造「举报」请求。
///
/// **定级 [`Idempotency::Unsafe`]：传输层一次都不会重发。** 没有幂等键，
/// 服务端每收一次立一条工单，而且查不到上一次立没立——重发的代价落在
/// 被举报的人身上。超时就是超时，要不要再举报一次由用户决定，不由传输层
/// 替他决定（ADR-013 决策一、二）。
///
/// `comment` 与 `extra` 是**已序列化的 JSON 字符串**，不是对象：基线对它们
/// 调了 `jsonEncode` 再放进表单。照搬，否则服务端收到的是 `[object Object]`
/// 那一类的东西。
#[must_use]
pub fn im_report_request(
    accused_uid: i64,
    reason_type: i64,
    reason_desc: &str,
    comment_json: &str,
    extra_json: &str,
) -> PreparedWrite {
    PreparedWrite {
        path: path::IM_REPORT,
        query: Vec::new(),
        body: WriteBody::Form(vec![
            ("biz_code".to_owned(), "4".to_owned()),
            ("accused_uid".to_owned(), accused_uid.to_string()),
            // 基线把 object_id 也填成被举报人的 mid。私信举报的「对象」就是
            // 那个人，不是某一条消息。
            ("object_id".to_owned(), accused_uid.to_string()),
            ("reason_type".to_owned(), reason_type.to_string()),
            ("reason_desc".to_owned(), reason_desc.to_owned()),
            ("module".to_owned(), "604".to_owned()),
            ("comment".to_owned(), comment_json.to_owned()),
            ("extra".to_owned(), extra_json.to_owned()),
        ]),
        csrf: CsrfIn::Body,
        idempotency: Idempotency::Unsafe,
    }
}

/// 更新系统通知已读游标。
///
/// # Errors
/// 传输、状态码、信封三类失败。
pub async fn update_sys_cursor<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    cursor: i64,
    csrf: &str,
) -> Result<(), DomainError> {
    let body =
        crate::client::send(transport, ctx, &sys_update_cursor_request(cursor, csrf)).await?;
    crate::envelope::check_envelope(&body)
}

/// 更新私信已读位点。
///
/// # Errors
/// 同上。
pub async fn ack_session<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    talker_id: i64,
    ack_seqno: i64,
    csrf: &str,
    mixin_key: &str,
    wts: i64,
) -> Result<(), DomainError> {
    let body = crate::client::send(
        transport,
        ctx,
        &ack_session_request(talker_id, ack_seqno, csrf, mixin_key, wts),
    )
    .await?;
    crate::envelope::check_envelope(&body)
}

/// 删除一个会话。
///
/// # Errors
/// 同上。
pub async fn remove_session<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    talker_id: i64,
    csrf: &str,
    mixin_key: &str,
    wts: i64,
) -> Result<(), DomainError> {
    let body = crate::client::send_write(
        transport,
        ctx,
        &remove_session_request(talker_id, csrf, mixin_key, wts),
    )
    .await?;
    crate::envelope::check_envelope(&body)
}

/// 删除一条消息中心的条目。
///
/// # Errors
/// 同上。
pub async fn del_msg_feed<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    tp: i64,
    id: &str,
) -> Result<(), DomainError> {
    let body = crate::client::send_write(transport, ctx, &del_msg_feed_request(tp, id)).await?;
    crate::envelope::check_envelope(&body)
}

/// 删除一条系统通知。
///
/// # Errors
/// 同上。
pub async fn del_sys_msg<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    id: &str,
) -> Result<(), DomainError> {
    let body = crate::client::send_write(transport, ctx, &del_sys_msg_request(id)).await?;
    crate::envelope::check_envelope(&body)
}

/// 设置某条赞的通知开关。
///
/// # Errors
/// 同上。
pub async fn set_notice<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    id: &str,
    notice_state: i64,
) -> Result<(), DomainError> {
    let body =
        crate::client::send_write(transport, ctx, &set_notice_request(id, notice_state)).await?;
    crate::envelope::check_envelope(&body)
}

/// 设置免打扰。
///
/// # Errors
/// 同上。
pub async fn set_msg_dnd<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    uid: i64,
    setting: i64,
    dnd_uid: i64,
) -> Result<(), DomainError> {
    let body =
        crate::client::send_write(transport, ctx, &set_msg_dnd_request(uid, setting, dnd_uid))
            .await?;
    crate::envelope::check_envelope(&body)
}

/// 设置内容推送开关。
///
/// # Errors
/// 同上。
pub async fn set_push_ss<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    setting: i64,
    talker_uid: i64,
) -> Result<(), DomainError> {
    let body = crate::client::send_write(transport, ctx, &set_push_ss_request(setting, talker_uid))
        .await?;
    crate::envelope::check_envelope(&body)
}

/// 举报一位私信对象。**不可重发**，见 [`im_report_request`]。
///
/// # Errors
/// 同上。
pub async fn im_report<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    accused_uid: i64,
    reason_type: i64,
    reason_desc: &str,
    comment_json: &str,
    extra_json: &str,
) -> Result<(), DomainError> {
    let body = crate::client::send_write(
        transport,
        ctx,
        &im_report_request(
            accused_uid,
            reason_type,
            reason_desc,
            comment_json,
            extra_json,
        ),
    )
    .await?;
    crate::envelope::check_envelope(&body)
}

// ================================================================ DTO

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct CursorDto {
    is_end: Option<bool>,
    id: Option<i64>,
    time: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct UserDto {
    mid: Option<i64>,
    nickname: Option<String>,
    avatar: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ReplyFeedDto {
    cursor: Option<CursorDto>,
    items: Option<Vec<ReplyItemDto>>,
    last_view_at: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ReplyItemDto {
    id: Option<i64>,
    user: Option<UserDto>,
    item: Option<ReplyContentDto>,
    counts: Option<i64>,
    is_multi: Option<i64>,
    reply_time: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ReplyContentDto {
    subject_id: Option<i64>,
    business_id: Option<i64>,
    business: Option<String>,
    native_uri: Option<String>,
    root_reply_content: Option<String>,
    source_content: Option<String>,
    target_reply_content: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct AtFeedDto {
    cursor: Option<CursorDto>,
    items: Option<Vec<AtItemDto>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct AtItemDto {
    id: Option<i64>,
    user: Option<UserDto>,
    item: Option<AtContentDto>,
    at_time: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct AtContentDto {
    business: Option<String>,
    image: Option<String>,
    source_content: Option<String>,
    native_uri: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct LikeFeedDto {
    latest: Option<LikeLatestDto>,
    total: Option<LikeTotalDto>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct LikeLatestDto {
    items: Option<Vec<LikeItemDto>>,
    last_view_at: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct LikeTotalDto {
    cursor: Option<CursorDto>,
    items: Option<Vec<LikeItemDto>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct LikeItemDto {
    id: Option<i64>,
    users: Option<Vec<UserDto>>,
    item: Option<LikeContentDto>,
    counts: Option<i64>,
    like_time: Option<i64>,
    notice_state: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct LikeContentDto {
    business: Option<String>,
    title: Option<String>,
    image: Option<String>,
    native_uri: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct LikeDetailDto {
    page: Option<LikeDetailPageDto>,
    card: Option<LikeDetailCardDto>,
    items: Option<Vec<LikeDetailItemDto>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct LikeDetailPageDto {
    is_end: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct LikeDetailCardDto {
    business: Option<String>,
    title: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct LikeDetailItemDto {
    user: Option<UserDto>,
    like_time: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SysMsgDto {
    id: Option<i64>,
    cursor: Option<i64>,
    title: Option<String>,
    content: Option<String>,
    time_at: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SingleUnreadDto {
    unfollow_unread: Option<i64>,
    follow_unread: Option<i64>,
    unfollow_push_msg: Option<i64>,
    dustbin_push_msg: Option<i64>,
    dustbin_unread: Option<i64>,
    biz_msg_unfollow_unread: Option<i64>,
    biz_msg_follow_unread: Option<i64>,
    custom_unread: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct FeedUnreadDto {
    at: Option<i64>,
    like: Option<i64>,
    reply: Option<i64>,
    sys_msg: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ImUserInfoDto {
    mid: Option<i64>,
    name: Option<String>,
    face: Option<String>,
    sign: Option<String>,
    vip: Option<VipDto>,
    pendant: Option<PendantDto>,
    official: Option<OfficialDto>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct VipDto {
    status: Option<i64>,
    #[serde(rename = "type")]
    vip_type: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct PendantDto {
    image: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct OfficialDto {
    /// 基线 `BaseOfficialVerify` 读的就是这个字段，不是 `role`。
    #[serde(rename = "type")]
    official_type: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SessionSsDto {
    follow_status: Option<i64>,
    push_setting: Option<i64>,
    show_push_setting: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct MsgDndDto {
    uid_settings: Option<Vec<UidSettingDto>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct UidSettingDto {
    setting: Option<i64>,
}

// ============================================================== 解析

fn cursor(d: CursorDto) -> FeedCursor {
    FeedCursor {
        is_end: d.is_end,
        id: d.id,
        time: d.time,
    }
}

fn user(d: UserDto) -> FeedUser {
    FeedUser {
        mid: d.mid,
        nickname: d.nickname,
        avatar: d.avatar,
    }
}

/// 解析「回复我的」。
///
/// # Errors
/// 信封失败沿用统一分类。
pub fn parse_reply_feed(body: &str) -> Result<ReplyFeed, DomainError> {
    let dto: ReplyFeedDto = crate::envelope::unwrap_envelope(body)?;
    Ok(ReplyFeed {
        cursor: dto.cursor.map(cursor),
        last_view_at: dto.last_view_at,
        items: dto
            .items
            .unwrap_or_default()
            .into_iter()
            .map(|i| ReplyFeedItem {
                id: i.id,
                user: i.user.map(user),
                item: i.item.map(|c| ReplyFeedContent {
                    subject_id: c.subject_id,
                    business_id: c.business_id,
                    business: c.business,
                    native_uri: c.native_uri,
                    root_reply_content: c.root_reply_content,
                    source_content: c.source_content,
                    target_reply_content: c.target_reply_content,
                }),
                counts: i.counts,
                is_multi: i.is_multi,
                reply_time: i.reply_time,
            })
            .collect(),
    })
}

/// 解析「@我的」。
///
/// # Errors
/// 同上。
pub fn parse_at_feed(body: &str) -> Result<AtFeed, DomainError> {
    let dto: AtFeedDto = crate::envelope::unwrap_envelope(body)?;
    Ok(AtFeed {
        cursor: dto.cursor.map(cursor),
        items: dto
            .items
            .unwrap_or_default()
            .into_iter()
            .map(|i| AtFeedItem {
                id: i.id,
                user: i.user.map(user),
                item: i.item.map(|c| AtFeedContent {
                    business: c.business,
                    image: c.image,
                    source_content: c.source_content,
                    native_uri: c.native_uri,
                }),
                at_time: i.at_time,
            })
            .collect(),
    })
}

fn like_items(items: Option<Vec<LikeItemDto>>) -> Vec<LikeFeedItem> {
    items
        .unwrap_or_default()
        .into_iter()
        .map(|i| LikeFeedItem {
            id: i.id,
            users: i.users.unwrap_or_default().into_iter().map(user).collect(),
            item: i.item.map(|c| LikeFeedContent {
                business: c.business,
                title: c.title,
                image: c.image,
                native_uri: c.native_uri,
            }),
            counts: i.counts,
            like_time: i.like_time,
            notice_state: i.notice_state,
        })
        .collect()
}

/// 解析「收到的赞」。
///
/// # Errors
/// 同上。
pub fn parse_like_feed(body: &str) -> Result<LikeFeed, DomainError> {
    let dto: LikeFeedDto = crate::envelope::unwrap_envelope(body)?;
    Ok(LikeFeed {
        latest: dto.latest.map(|l| LikeLatest {
            items: like_items(l.items),
            last_view_at: l.last_view_at,
        }),
        total: dto.total.map(|t| LikeTotal {
            cursor: t.cursor.map(cursor),
            items: like_items(t.items),
        }),
    })
}

/// 解析赞的明细页。
///
/// # Errors
/// 同上。
pub fn parse_like_detail(body: &str) -> Result<LikeDetail, DomainError> {
    let dto: LikeDetailDto = crate::envelope::unwrap_envelope(body)?;
    Ok(LikeDetail {
        page: dto.page.map(|p| LikeDetailPage { is_end: p.is_end }),
        card: dto.card.map(|c| LikeDetailCard {
            business: c.business,
            title: c.title,
        }),
        items: dto
            .items
            .unwrap_or_default()
            .into_iter()
            .map(|i| LikeDetailItem {
                user: i.user.map(user),
                like_time: i.like_time,
            })
            .collect(),
    })
}

/// 从一条系统通知的 `content` 里取出要显示的那一段。
///
/// 服务端对这个字段给了两种形态：纯文本，或一段 `{"web": "...",
/// "android": "..."}` 的 JSON。取哪一个是「协议没规定、但两端必须一致」
/// 的决定，按 AGENTS.md §2 留在 Rust。
///
/// 规则与基线 `MsgSysItem.fromJson` 一致：能解成 JSON **且**有非空的
/// `web` 字段时取 `web`，否则原样保留。基线把解析失败吞掉（`catch (_) {}`），
/// 这里同样不报错——那不是协议变更，是这条通知本来就是纯文本。
#[must_use]
pub fn sys_msg_content(raw: Option<String>) -> Option<String> {
    let raw = raw?;
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return Some(raw);
    };
    match value.get("web").and_then(serde_json::Value::as_str) {
        Some(web) => Some(web.to_owned()),
        None => Some(raw),
    }
}

/// 解析系统通知列表。
///
/// # Errors
/// 信封失败沿用统一分类。`data` 为 `null` 不是错误——基线把它当成空列表。
pub fn parse_sys_notify(body: &str) -> Result<Vec<SysMsgItem>, DomainError> {
    let dto: Option<Vec<SysMsgDto>> = crate::envelope::unwrap_envelope_nullable(body)?;
    Ok(dto
        .unwrap_or_default()
        .into_iter()
        .map(|i| SysMsgItem {
            id: i.id,
            cursor: i.cursor,
            title: i.title,
            content: sys_msg_content(i.content),
            time_at: i.time_at,
        })
        .collect())
}

/// 解析私信未读数。
///
/// # Errors
/// 信封失败沿用统一分类。
pub fn parse_single_unread(body: &str) -> Result<SingleUnread, DomainError> {
    let d: SingleUnreadDto = crate::envelope::unwrap_envelope(body)?;
    Ok(SingleUnread {
        unfollow_unread: d.unfollow_unread.unwrap_or(0),
        follow_unread: d.follow_unread.unwrap_or(0),
        unfollow_push_msg: d.unfollow_push_msg.unwrap_or(0),
        dustbin_push_msg: d.dustbin_push_msg.unwrap_or(0),
        dustbin_unread: d.dustbin_unread.unwrap_or(0),
        biz_msg_unfollow_unread: d.biz_msg_unfollow_unread.unwrap_or(0),
        biz_msg_follow_unread: d.biz_msg_follow_unread.unwrap_or(0),
        custom_unread: d.custom_unread.unwrap_or(0),
    })
}

/// 解析消息中心未读数。
///
/// # Errors
/// 同上。
pub fn parse_feed_unread(body: &str) -> Result<MsgFeedUnread, DomainError> {
    let d: FeedUnreadDto = crate::envelope::unwrap_envelope(body)?;
    Ok(MsgFeedUnread {
        at: d.at.unwrap_or(0),
        like: d.like.unwrap_or(0),
        reply: d.reply.unwrap_or(0),
        sys_msg: d.sys_msg.unwrap_or(0),
    })
}

/// 解析 IM 用户资料。
///
/// # Errors
/// 同上。`data` 为 `null` 当成空列表，与基线一致。
pub fn parse_im_user_infos(body: &str) -> Result<Vec<ImUserInfo>, DomainError> {
    let dto: Option<Vec<ImUserInfoDto>> = crate::envelope::unwrap_envelope_nullable(body)?;
    Ok(dto
        .unwrap_or_default()
        .into_iter()
        .map(|d| {
            let vip = d.vip.unwrap_or_default();
            let official = d.official.unwrap_or_default();
            ImUserInfo {
                mid: d.mid,
                name: d.name,
                face: d.face,
                sign: d.sign,
                vip_status: vip.status,
                vip_type: vip.vip_type,
                // 空串与没有挂件在界面上是同一件事，归一成 None，省得每个
                // 调用点各写一遍 `?.isNotEmpty == true`。
                pendant_image: d
                    .pendant
                    .unwrap_or_default()
                    .image
                    .filter(|s| !s.is_empty()),
                official_type: official.official_type,
            }
        })
        .collect())
}

/// 解析会话设置。
///
/// # Errors
/// 同上。
pub fn parse_session_ss(body: &str) -> Result<SessionSs, DomainError> {
    let d: SessionSsDto = crate::envelope::unwrap_envelope(body)?;
    Ok(SessionSs {
        follow_status: d.follow_status,
        push_setting: d.push_setting,
        show_push_setting: d.show_push_setting,
    })
}

/// 解析免打扰设置。
///
/// # Errors
/// 同上。
pub fn parse_msg_dnd(body: &str) -> Result<Vec<MsgDndSetting>, DomainError> {
    let dto: Option<MsgDndDto> = crate::envelope::unwrap_envelope_nullable(body)?;
    Ok(dto
        .unwrap_or_default()
        .uid_settings
        .unwrap_or_default()
        .into_iter()
        .map(|s| MsgDndSetting { setting: s.setting })
        .collect())
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    // 与 bili-protocol 的 fixture 同源的占位 key（非真实凭据）。
    const MIXIN_KEY: &str = "ea1db124af3c7062474693fa704f4ff8";
    // 占位 csrf。真实 token 从不进仓库。
    const CSRF: &str = "0000000000000000000000000000cafe";

    #[test]
    fn reply_feed_first_page_has_no_cursor_params() {
        let req = reply_feed_request(None, None);
        assert_eq!(req.path, path::MSG_FEED_REPLY);
        assert_eq!(
            req.query_string(),
            "platform=web&mobi_app=web&build=0&web_location=333.40164"
        );
    }

    #[test]
    fn each_feed_uses_its_own_cursor_time_field() {
        // 三条 feed 的 id 字段同名、时间字段不同名。把 at 的时间发成
        // reply_time，服务端不报错，只是永远给第一页——这个测试就是为了
        // 让那种「翻页翻不动」的改动在这里先红。
        assert!(
            reply_feed_request(Some(1), Some(2))
                .query_string()
                .contains("reply_time=2")
        );
        assert!(
            at_feed_request(Some(1), Some(2))
                .query_string()
                .contains("at_time=2")
        );
        assert!(
            like_feed_request(Some(1), Some(2))
                .query_string()
                .contains("like_time=2")
        );
    }

    #[test]
    fn unread_endpoints_use_the_other_web_location() {
        assert!(
            single_unread_request()
                .query_string()
                .contains("web_location=333.1365")
        );
        assert!(
            feed_unread_request()
                .query_string()
                .contains("web_location=333.1365")
        );
    }

    #[test]
    fn vc_reads_carry_both_csrf_fields() {
        // 这三条 GET 不带 csrf 会被服务端以 -111 拒绝，表现得像协议问题。
        for q in [
            im_user_infos_request("1", CSRF).query_string(),
            session_ss_request(1, CSRF).query_string(),
            msg_dnd_request(1, "2", CSRF).query_string(),
        ] {
            assert!(q.contains("csrf_token="), "缺 csrf_token: {q}");
            assert!(q.contains("&csrf="), "缺 csrf: {q}");
        }
    }

    // ---------------------------------------------------------- 写

    #[test]
    fn ack_signature_covers_csrf() {
        // 签名范围里必须有 csrf：事后补的话 w_rid 对不上，而服务端只会
        // 回一个业务错误码。
        let req = ack_session_request(42, 7, CSRF, MIXIN_KEY, 1_702_204_169);
        let keys: Vec<&str> = req.query.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(keys.last(), Some(&"w_rid"), "w_rid 必须在最后");
        assert!(keys.contains(&"csrf"));
        assert!(keys.contains(&"wts"));

        // 换一个 csrf，签名必须跟着变——否则就说明它没进签名范围。
        let other = ack_session_request(
            42,
            7,
            "ffffffffffffffffffffffffffffffff",
            MIXIN_KEY,
            1_702_204_169,
        );
        assert_ne!(req.query, other.query);
    }

    #[test]
    fn remove_session_signs_the_form_body() {
        let req = remove_session_request(42, CSRF, MIXIN_KEY, 1_702_204_169);
        assert_eq!(req.path, path::REMOVE_SESSION);
        assert_eq!(
            req.csrf,
            CsrfIn::None,
            "csrf 已被签名覆盖，不能让 send_write 再补一份"
        );
        let form = req.form_fields().expect("这条是表单体");
        assert_eq!(form.last().map(|(k, _)| k.as_str()), Some("w_rid"));
    }

    #[test]
    fn report_is_the_only_unsafe_write() {
        assert_eq!(
            im_report_request(1, 2, "", "{}", "{}").idempotency,
            Idempotency::Unsafe,
            "举报没有幂等键，重发会多立一条工单"
        );
        for req in [
            del_msg_feed_request(0, "1"),
            del_sys_msg_request("1"),
            set_notice_request("1", 0),
            set_msg_dnd_request(1, 0, 2),
            set_push_ss_request(1, 2),
            remove_session_request(1, CSRF, MIXIN_KEY, 0),
        ] {
            assert_eq!(
                req.idempotency,
                Idempotency::Idempotent,
                "{} 是置为/删除语义，应当可重发",
                req.path
            );
        }
    }

    #[test]
    fn writes_do_not_carry_credentials_in_their_constructed_form() {
        // csrf 由 send_write 统一补（ADR-013 决策四），所以这几条的构造
        // 产物可以原样写进请求快照而不含凭据。带签名的那两条是显式例外。
        for req in [
            del_msg_feed_request(0, "1"),
            set_notice_request("1", 0),
            set_msg_dnd_request(1, 0, 2),
            set_push_ss_request(1, 2),
            im_report_request(1, 2, "x", "{}", "{}"),
        ] {
            let form = req.form_fields().expect("这几条都是表单体");
            assert!(
                form.iter().all(|(k, _)| k != "csrf" && k != "csrf_token"),
                "{} 的构造产物里不该有凭据",
                req.path
            );
        }
    }

    #[test]
    fn del_sys_msg_is_json_with_csrf_in_two_places() {
        let req = del_sys_msg_request("12345");
        assert_eq!(req.csrf, CsrfIn::BodyAndQuery);
        assert_eq!(
            req.query,
            vec![("mobi_app".to_owned(), "android".to_owned())]
        );
        // ids 是数组，不是裸值——发成裸值服务端会静默不删。
        assert!(req.form_string().contains(r#""ids":["12345"]"#));
    }

    // -------------------------------------------------------- 解析

    const REPLY_BODY: &str = r#"{"code":0,"message":"0","data":{
        "cursor":{"is_end":false,"id":100,"time":1700000000},
        "last_view_at":1699999999,
        "items":[{"id":1,"user":{"mid":9,"nickname":"甲","avatar":"a.jpg"},
          "item":{"subject_id":5,"business":"评论","source_content":"原文",
                  "root_reply_content":"根","target_reply_content":"目标"},
          "counts":3,"is_multi":1,"reply_time":1700000001}]}}"#;

    #[test]
    fn parses_reply_feed() {
        let f = parse_reply_feed(REPLY_BODY).expect("应解析成功");
        assert_eq!(f.last_view_at, Some(1_699_999_999));
        assert_eq!(f.cursor.expect("有游标").id, Some(100));
        assert_eq!(f.items.len(), 1);
        let it = &f.items[0];
        assert_eq!(
            it.user.as_ref().expect("有用户").nickname.as_deref(),
            Some("甲")
        );
        assert_eq!(
            it.item.as_ref().expect("有内容").source_content.as_deref(),
            Some("原文")
        );
    }

    #[test]
    fn missing_items_is_an_empty_page_not_an_error() {
        // 翻到底时服务端只回 cursor。判成协议错误的话，最后一页会变成红字。
        let f = parse_reply_feed(r#"{"code":0,"message":"0","data":{"cursor":{"is_end":true}}}"#)
            .expect("空页不是错误");
        assert!(f.items.is_empty());
        assert_eq!(f.cursor.expect("有游标").is_end, Some(true));
    }

    #[test]
    fn parses_like_feed_two_sections() {
        let body = r#"{"code":0,"message":"0","data":{
            "latest":{"last_view_at":1,"items":[{"id":1,"counts":5,
                "users":[{"mid":1},{"mid":2}],"item":{"title":"标题"}}]},
            "total":{"cursor":{"is_end":false,"id":9},"items":[]}}}"#;
        let f = parse_like_feed(body).expect("应解析成功");
        let latest = f.latest.expect("有 latest");
        assert_eq!(latest.items.len(), 1);
        // counts 是总人数，users 只有前几个——两者不等是正常的。
        assert_eq!(latest.items[0].counts, Some(5));
        assert_eq!(latest.items[0].users.len(), 2);
        assert!(f.total.expect("有 total").items.is_empty());
    }

    #[test]
    fn sys_msg_content_prefers_the_web_variant() {
        assert_eq!(
            sys_msg_content(Some(
                r#"{"web":"网页文案","android":"安卓文案"}"#.to_owned()
            )),
            Some("网页文案".to_owned())
        );
    }

    #[test]
    fn sys_msg_content_keeps_plain_text_as_is() {
        // 纯文本解不成 JSON，照原样留着——基线把解析异常吞掉也是这个效果。
        assert_eq!(
            sys_msg_content(Some("您的稿件已通过审核".to_owned())),
            Some("您的稿件已通过审核".to_owned())
        );
        // 能解成 JSON 但没有 web 字段的，同样原样保留。
        assert_eq!(
            sys_msg_content(Some(r#"{"android":"只有安卓"}"#.to_owned())),
            Some(r#"{"android":"只有安卓"}"#.to_owned())
        );
        assert_eq!(sys_msg_content(None), None);
    }

    #[test]
    fn null_data_is_an_empty_list_for_sys_notify() {
        // 没有系统通知时服务端回 data: null。判成协议错误的话，空列表会
        // 变成一句红色报错。
        let items =
            parse_sys_notify(r#"{"code":0,"message":"0","data":null}"#).expect("null 不是错误");
        assert!(items.is_empty());
    }

    #[test]
    fn unread_counts_default_to_zero() {
        let u = parse_single_unread(r#"{"code":0,"message":"0","data":{"follow_unread":3}}"#)
            .expect("应解析成功");
        assert_eq!(u.follow_unread, 3);
        assert_eq!(u.custom_unread, 0, "缺字段当 0，与基线的 ?? 0 一致");
        assert_eq!(u.badge_count(), 3);
    }

    #[test]
    fn parses_im_user_info_and_normalises_empty_strings() {
        let body = r#"{"code":0,"message":"0","data":[{"mid":1,"name":"某人",
            "vip":{"status":1,"type":2},
            "pendant":{"image":""},"official":{"type":0,"desc":"某机构认证"}}]}"#;
        let list = parse_im_user_infos(body).expect("应解析成功");
        assert_eq!(list.len(), 1);
        let u = &list[0];
        assert_eq!(u.vip_status, Some(1));
        assert_eq!(u.vip_type, Some(2));
        // 空串归一成 None：界面不必每处再写一遍 isNotEmpty。
        assert_eq!(u.pendant_image, None);
        // 认证标读的是 `type`——基线 BaseOfficialVerify 就读这个字段。
        assert_eq!(u.official_type, Some(0));
    }

    #[test]
    fn parses_msg_dnd_settings() {
        let body = r#"{"code":0,"message":"0","data":{"uid_settings":[{"setting":0}]}}"#;
        let list = parse_msg_dnd(body).expect("应解析成功");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].setting, Some(0));
    }

    #[test]
    fn a_business_error_is_carried_through() {
        let err = parse_reply_feed(r#"{"code":-101,"message":"账号未登录"}"#)
            .expect_err("业务失败必须是错误");
        assert_eq!(err.code(), Some(-101));
    }
}
