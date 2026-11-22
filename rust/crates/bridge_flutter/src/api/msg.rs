//! 消息中心与私信设置的 Dart 导出（S12 / issue #63）。
//!
//! 覆盖基线 `lib/http/msg.dart` 的消息域：三条 feed、赞的明细、系统通知、
//! 两处未读数、私信会话的只读设置，以及九条写操作。私信的收发走 gRPC，
//! 在 [`crate::api::grpc`] 那条路径上，不在这里。
//!
//! ## 未读数的红点算法在这一侧算完才回去
//!
//! [`fetch_single_unread`] 与 [`fetch_feed_unread`] 返回的结构里带一个
//! `badge` 字段，是**已经算好的**红点数字。基线把这两段加法写在
//! `lib/pages/main/controller.dart` 里；哪几个桶算进红点、用户关掉「赞」
//! 之后要不要减，都是换掉 Flutter 这层时必须原样存在的规则，按
//! AGENTS.md §2 下沉（算法在 [`bili_domain::msg`]）。
//!
//! 做成「取数时顺带算」而不是另开一个导出函数，是为了不多一次跨 FFI
//! 往返：选择集（[`FeedBadgeSelection`]）是用户设置，调用点本来就有。
//!
//! ## 带签名的两条写操作
//!
//! [`ack_session`] 与 [`remove_session`] 要 WBI 签名，而且**签名覆盖
//! csrf**，所以它们的 csrf 走参数而不是 [`MsgContext::csrf`]——事后补的话
//! `w_rid` 对不上。理由写在 `bili_api::msg::ack_session_request` 上，这是
//! ADR-013 决策四的一处显式例外。

use std::time::Duration;

use bili_api::http::RequestContext;
use bili_api::msg;
use bili_api::transport_reqwest::ReqwestTransport;
use bili_domain::error::DomainError;
use bili_domain::msg as domain;
use bili_domain::redact::Redacted;

use crate::api::init;
use crate::api::transport::TransportSettings;
use crate::api::write::WriteResult;

// ------------------------------------------------------------ 公共结构

/// 三条 feed 共用的翻页游标。
#[derive(Debug, Clone)]
pub struct FeedCursor {
    pub is_end: Option<bool>,
    pub id: Option<i64>,
    pub time: Option<i64>,
}

/// feed 里的一位用户。
#[derive(Debug, Clone)]
pub struct FeedUser {
    pub mid: Option<i64>,
    pub nickname: Option<String>,
    pub avatar: Option<String>,
}

// -------------------------------------------------------------- 回复我的

/// 「回复我的」一页。
#[derive(Debug, Clone)]
pub struct ReplyFeed {
    pub cursor: Option<FeedCursor>,
    pub items: Vec<ReplyFeedItem>,
    /// 上次查看时间，界面用它画「以下为已读」的分隔线。
    pub last_view_at: Option<i64>,
}

/// 「回复我的」里的一条。
#[derive(Debug, Clone)]
pub struct ReplyFeedItem {
    pub id: Option<i64>,
    pub user: Option<FeedUser>,
    pub item: Option<ReplyFeedContent>,
    pub counts: Option<i64>,
    pub is_multi: Option<i64>,
    pub reply_time: Option<i64>,
}

/// 被回复的那条内容。
#[derive(Debug, Clone)]
pub struct ReplyFeedContent {
    pub subject_id: Option<i64>,
    pub business_id: Option<i64>,
    pub business: Option<String>,
    pub native_uri: Option<String>,
    pub root_reply_content: Option<String>,
    pub source_content: Option<String>,
    pub target_reply_content: Option<String>,
}

// ---------------------------------------------------------------- @我的

/// 「@我的」一页。
#[derive(Debug, Clone)]
pub struct AtFeed {
    pub cursor: Option<FeedCursor>,
    pub items: Vec<AtFeedItem>,
}

/// 「@我的」里的一条。
#[derive(Debug, Clone)]
pub struct AtFeedItem {
    pub id: Option<i64>,
    pub user: Option<FeedUser>,
    pub item: Option<AtFeedContent>,
    pub at_time: Option<i64>,
}

/// @ 发生在哪条内容里。
#[derive(Debug, Clone)]
pub struct AtFeedContent {
    pub business: Option<String>,
    pub image: Option<String>,
    pub source_content: Option<String>,
    pub native_uri: Option<String>,
}

// -------------------------------------------------------------- 收到的赞

/// 「收到的赞」一页。两段结构，界面把它们拼起来显示。
#[derive(Debug, Clone)]
pub struct LikeFeed {
    pub latest: Option<LikeLatest>,
    pub total: Option<LikeTotal>,
}

/// 最近未读的一批赞。没有游标。
#[derive(Debug, Clone)]
pub struct LikeLatest {
    pub items: Vec<LikeFeedItem>,
    pub last_view_at: Option<i64>,
}

/// 全部赞，可翻页。
#[derive(Debug, Clone)]
pub struct LikeTotal {
    pub cursor: Option<FeedCursor>,
    pub items: Vec<LikeFeedItem>,
}

/// 「收到的赞」里的一条：一群人赞了同一个东西。
#[derive(Debug, Clone)]
pub struct LikeFeedItem {
    pub id: Option<i64>,
    pub users: Vec<FeedUser>,
    pub item: Option<LikeFeedContent>,
    /// 总人数，可能大于 `users.len()`。
    pub counts: Option<i64>,
    pub like_time: Option<i64>,
    pub notice_state: Option<i64>,
}

/// 被赞的那条内容。
#[derive(Debug, Clone)]
pub struct LikeFeedContent {
    pub business: Option<String>,
    pub title: Option<String>,
    pub image: Option<String>,
    pub native_uri: Option<String>,
}

/// 某条赞展开后的人名单。
#[derive(Debug, Clone)]
pub struct LikeDetail {
    pub page: Option<LikeDetailPage>,
    pub card: Option<LikeDetailCard>,
    pub items: Vec<LikeDetailItem>,
}

/// 明细页的翻页状态。按页码翻，不用游标。
#[derive(Debug, Clone)]
pub struct LikeDetailPage {
    pub is_end: Option<bool>,
}

/// 明细页顶部那张卡片。
#[derive(Debug, Clone)]
pub struct LikeDetailCard {
    pub business: Option<String>,
    pub title: Option<String>,
}

/// 明细页里的一个人。
#[derive(Debug, Clone)]
pub struct LikeDetailItem {
    pub user: Option<FeedUser>,
    pub like_time: Option<i64>,
}

// -------------------------------------------------------------- 系统通知

/// 一条系统通知。
///
/// `content` 已经从服务端那段「可能是 JSON 的字符串」里取好了要显示的
/// 那一段（见 `bili_api::msg::sys_msg_content`），界面直接显示即可。
#[derive(Debug, Clone)]
pub struct SysMsgItem {
    pub id: Option<i64>,
    /// 翻页游标，同时是「已读到哪」的标记。
    pub cursor: Option<i64>,
    pub title: Option<String>,
    pub content: Option<String>,
    /// 服务端已格式化好的时间串，不是时间戳。
    pub time_at: Option<String>,
}

// ---------------------------------------------------------------- 未读数

/// 私信未读数。`badge` 是算好的红点数字。
#[derive(Debug, Clone)]
pub struct SingleUnread {
    pub unfollow_unread: i64,
    pub follow_unread: i64,
    pub unfollow_push_msg: i64,
    pub dustbin_push_msg: i64,
    pub dustbin_unread: i64,
    pub biz_msg_unfollow_unread: i64,
    pub biz_msg_follow_unread: i64,
    pub custom_unread: i64,
    /// 底部导航私信那一项要显示的数字。算哪几个桶见
    /// [`bili_domain::msg::SingleUnread::badge_count`]。
    pub badge: i64,
}

/// 消息中心四个 Tab 的未读数。`badge` 是按用户勾选算好的红点数字。
#[derive(Debug, Clone)]
pub struct MsgFeedUnread {
    pub at: i64,
    pub like: i64,
    pub reply: i64,
    pub sys_msg: i64,
    pub badge: i64,
}

/// 红点要算哪几个 Tab。由用户在设置页勾选，所以是输入不是常量。
#[derive(Debug, Clone)]
pub struct FeedBadgeSelection {
    pub reply: bool,
    pub at: bool,
    /// 基线的两级开关（`msgUnReadTypes` 里有 like **且**没开
    /// `disableLikeMsg`）在调用点合并成这一个布尔。
    pub like: bool,
    pub sys_msg: bool,
}

// ------------------------------------------------------------ 会话设置

/// 一位 IM 用户的资料。只取私信设置页那张卡片实际显示的字段。
#[derive(Debug, Clone)]
pub struct ImUserInfo {
    pub mid: Option<i64>,
    pub name: Option<String>,
    pub face: Option<String>,
    pub sign: Option<String>,
    /// 0 无、>0 是会员。
    pub vip_status: Option<i64>,
    /// 2 是年度及以上，昵称才染色。
    pub vip_type: Option<i64>,
    /// 没有挂件时已归一成 `None`，不是空串。
    pub pendant_image: Option<String>,
    /// 认证类型：-1 无认证、0 个人、1 机构。
    pub official_type: Option<i64>,
}

/// 与某人的会话设置。
#[derive(Debug, Clone)]
pub struct SessionSs {
    /// 128 是「已拉黑」，`None` 表示没有关系。
    pub follow_status: Option<i64>,
    /// 内容推送开关：1 开、0 关。
    pub push_setting: Option<i64>,
    pub show_push_setting: Option<i64>,
}

/// 对某个 uid 的免打扰设置。1 接收提醒、0 免打扰。
#[derive(Debug, Clone)]
pub struct MsgDndSetting {
    pub setting: Option<i64>,
}

// -------------------------------------------------------------- 结果类型

/// 「回复我的」的结果。
#[derive(Debug, Clone)]
pub struct ReplyFeedResult {
    pub data: Option<ReplyFeed>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

/// 「@我的」的结果。
#[derive(Debug, Clone)]
pub struct AtFeedResult {
    pub data: Option<AtFeed>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

/// 「收到的赞」的结果。
#[derive(Debug, Clone)]
pub struct LikeFeedResult {
    pub data: Option<LikeFeed>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

/// 赞的明细的结果。
#[derive(Debug, Clone)]
pub struct LikeDetailResult {
    pub data: Option<LikeDetail>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

/// 系统通知列表的结果。
#[derive(Debug, Clone)]
pub struct SysNotifyResult {
    pub data: Option<Vec<SysMsgItem>>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

/// 私信未读数的结果。
#[derive(Debug, Clone)]
pub struct SingleUnreadResult {
    pub data: Option<SingleUnread>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

/// 消息中心未读数的结果。
#[derive(Debug, Clone)]
pub struct MsgFeedUnreadResult {
    pub data: Option<MsgFeedUnread>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

/// IM 用户资料的结果。
#[derive(Debug, Clone)]
pub struct ImUserInfosResult {
    pub data: Option<Vec<ImUserInfo>>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

/// 会话设置的结果。
#[derive(Debug, Clone)]
pub struct SessionSsResult {
    pub data: Option<SessionSs>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

/// 免打扰设置的结果。
#[derive(Debug, Clone)]
pub struct MsgDndResult {
    pub data: Option<Vec<MsgDndSetting>>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

// ------------------------------------------------------------ 上下文

/// 一次请求所需的公共参数。
#[derive(Debug, Clone)]
pub struct MsgContext {
    /// 这一片横跨三个域名，由调用方按接口给出，见
    /// [`bili_api::msg::path`] 每个常量上的注释。
    pub base_url: String,
    /// 完整的 Cookie 头值。未登录时为 `None`。
    pub cookie: Option<String>,
    pub headers: Vec<(String, String)>,
    /// CSRF token。只有**走 `send_write` 的那几条**写操作要传；
    /// `api.vc` 的三条只读 GET 与两条带签名的写操作走参数，不走这里
    /// （理由见模块文档）。
    pub csrf: Option<String>,
    pub timeout_ms: u64,
    pub transport: TransportSettings,
    /// WBI mixin key。只有 [`ack_session`] 与 [`remove_session`] 要。
    pub mixin_key: String,
    /// Unix 秒，由 Dart 注入。
    pub wts: i64,
}

impl MsgContext {
    fn split(self) -> (RequestContext, ReqwestTransport, String, i64) {
        let transport = ReqwestTransport::new(self.transport.to_config());
        let ctx = RequestContext {
            base_url: self.base_url,
            cookie: self.cookie.map(Redacted::new),
            csrf: self.csrf.map(Redacted::new),
            headers: self.headers,
            timeout: Duration::from_millis(self.timeout_ms),
        };
        (ctx, transport, self.mixin_key, self.wts)
    }
}

// ------------------------------------------------------------ 只读导出

/// 取「回复我的」一页。
pub async fn fetch_reply_feed(
    context: MsgContext,
    cursor: Option<i64>,
    cursor_time: Option<i64>,
) -> ReplyFeedResult {
    let (ctx, transport, _, _) = context.split();
    let joined = init::runtime()
        .spawn(async move { msg::fetch_reply_feed(&transport, &ctx, cursor, cursor_time).await })
        .await;

    match flatten(joined) {
        Ok(d) => ReplyFeedResult {
            data: Some(reply_feed(d)),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => ReplyFeedResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 取「@我的」一页。
pub async fn fetch_at_feed(
    context: MsgContext,
    cursor: Option<i64>,
    cursor_time: Option<i64>,
) -> AtFeedResult {
    let (ctx, transport, _, _) = context.split();
    let joined = init::runtime()
        .spawn(async move { msg::fetch_at_feed(&transport, &ctx, cursor, cursor_time).await })
        .await;

    match flatten(joined) {
        Ok(d) => AtFeedResult {
            data: Some(at_feed(d)),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => AtFeedResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 取「收到的赞」一页。
pub async fn fetch_like_feed(
    context: MsgContext,
    cursor: Option<i64>,
    cursor_time: Option<i64>,
) -> LikeFeedResult {
    let (ctx, transport, _, _) = context.split();
    let joined = init::runtime()
        .spawn(async move { msg::fetch_like_feed(&transport, &ctx, cursor, cursor_time).await })
        .await;

    match flatten(joined) {
        Ok(d) => LikeFeedResult {
            data: Some(like_feed(d)),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => LikeFeedResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 取某条赞的明细。
///
/// `card_id` 与 `last_mid` 按字符串传：服务端对它们既可能回整数也可能回
/// 字符串，基线在 Dart 侧声明成 `Object` 正是这个原因（同类问题见 #166）。
pub async fn fetch_like_detail(
    context: MsgContext,
    card_id: String,
    pn: i64,
    last_mid: String,
) -> LikeDetailResult {
    let (ctx, transport, _, _) = context.split();
    let joined = init::runtime()
        .spawn(
            async move { msg::fetch_like_detail(&transport, &ctx, &card_id, pn, &last_mid).await },
        )
        .await;

    match flatten(joined) {
        Ok(d) => LikeDetailResult {
            data: Some(like_detail(d)),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => LikeDetailResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 取系统通知列表。
pub async fn fetch_sys_notify(
    context: MsgContext,
    cursor: Option<i64>,
    page_size: i64,
) -> SysNotifyResult {
    let (ctx, transport, _, _) = context.split();
    let joined = init::runtime()
        .spawn(async move { msg::fetch_sys_notify(&transport, &ctx, cursor, page_size).await })
        .await;

    match flatten(joined) {
        Ok(list) => SysNotifyResult {
            data: Some(list.into_iter().map(sys_msg_item).collect()),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => SysNotifyResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 取私信未读数。返回的 `badge` 已算好，见模块文档。
pub async fn fetch_single_unread(context: MsgContext) -> SingleUnreadResult {
    let (ctx, transport, _, _) = context.split();
    let joined = init::runtime()
        .spawn(async move { msg::fetch_single_unread(&transport, &ctx).await })
        .await;

    match flatten(joined) {
        Ok(u) => SingleUnreadResult {
            data: Some(SingleUnread {
                unfollow_unread: u.unfollow_unread,
                follow_unread: u.follow_unread,
                unfollow_push_msg: u.unfollow_push_msg,
                dustbin_push_msg: u.dustbin_push_msg,
                dustbin_unread: u.dustbin_unread,
                biz_msg_unfollow_unread: u.biz_msg_unfollow_unread,
                biz_msg_follow_unread: u.biz_msg_follow_unread,
                custom_unread: u.custom_unread,
                badge: u.badge_count(),
            }),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => SingleUnreadResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 取消息中心未读数。`pick` 是用户在设置页勾选的那几项。
pub async fn fetch_feed_unread(
    context: MsgContext,
    pick: FeedBadgeSelection,
) -> MsgFeedUnreadResult {
    let (ctx, transport, _, _) = context.split();
    let joined = init::runtime()
        .spawn(async move { msg::fetch_feed_unread(&transport, &ctx).await })
        .await;

    match flatten(joined) {
        Ok(u) => {
            let selection = domain::FeedBadgeSelection {
                reply: pick.reply,
                at: pick.at,
                like: pick.like,
                sys_msg: pick.sys_msg,
            };
            MsgFeedUnreadResult {
                data: Some(MsgFeedUnread {
                    at: u.at,
                    like: u.like,
                    reply: u.reply,
                    sys_msg: u.sys_msg,
                    badge: u.badge_count(selection),
                }),
                error_kind: None,
                error_detail: None,
                error_code: None,
            }
        }
        Err(e) => MsgFeedUnreadResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 取 IM 用户资料。`uids` 是逗号分隔的 mid 串。
///
/// csrf 走参数：这条是 GET，但 `api.vc` 的这组接口即使只读也校验 csrf。
pub async fn fetch_im_user_infos(
    context: MsgContext,
    uids: String,
    csrf: String,
) -> ImUserInfosResult {
    let (ctx, transport, _, _) = context.split();
    let joined = init::runtime()
        .spawn(async move { msg::fetch_im_user_infos(&transport, &ctx, &uids, &csrf).await })
        .await;

    match flatten(joined) {
        Ok(list) => ImUserInfosResult {
            data: Some(list.into_iter().map(im_user_info).collect()),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => ImUserInfosResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 取与某人的会话设置。csrf 走参数，理由同 [`fetch_im_user_infos`]。
pub async fn fetch_session_ss(
    context: MsgContext,
    talker_uid: i64,
    csrf: String,
) -> SessionSsResult {
    let (ctx, transport, _, _) = context.split();
    let joined = init::runtime()
        .spawn(async move { msg::fetch_session_ss(&transport, &ctx, talker_uid, &csrf).await })
        .await;

    match flatten(joined) {
        Ok(s) => SessionSsResult {
            data: Some(SessionSs {
                follow_status: s.follow_status,
                push_setting: s.push_setting,
                show_push_setting: s.show_push_setting,
            }),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => SessionSsResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 取免打扰设置。`own_uid` 是自己的 mid，`uids_str` 是要查的那些人。
pub async fn fetch_msg_dnd(
    context: MsgContext,
    own_uid: i64,
    uids_str: String,
    csrf: String,
) -> MsgDndResult {
    let (ctx, transport, _, _) = context.split();
    let joined = init::runtime()
        .spawn(async move { msg::fetch_msg_dnd(&transport, &ctx, own_uid, &uids_str, &csrf).await })
        .await;

    match flatten(joined) {
        Ok(list) => MsgDndResult {
            data: Some(
                list.into_iter()
                    .map(|s| MsgDndSetting { setting: s.setting })
                    .collect(),
            ),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => MsgDndResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

// ------------------------------------------------------------ 写操作

/// 更新系统通知的已读游标。
///
/// 基线发的是 **GET**，参数在查询串上；照搬，改成 POST 是一次没人验证过
/// 的协议改动。`cursor` 是位点不是增量，重发安全。
pub async fn update_sys_cursor(context: MsgContext, cursor: i64, csrf: String) -> WriteResult {
    let (ctx, transport, _, _) = context.split();
    let joined = init::runtime()
        .spawn(async move { msg::update_sys_cursor(&transport, &ctx, cursor, &csrf).await })
        .await;

    WriteResult::from_result(flatten(joined))
}

/// 更新私信的已读位点。WBI 签名，签名覆盖 csrf，所以 csrf 走参数。
pub async fn ack_session(
    context: MsgContext,
    talker_id: i64,
    ack_seqno: i64,
    csrf: String,
) -> WriteResult {
    let (ctx, transport, mixin_key, wts) = context.split();
    let joined = init::runtime()
        .spawn(async move {
            msg::ack_session(
                &transport, &ctx, talker_id, ack_seqno, &csrf, &mixin_key, wts,
            )
            .await
        })
        .await;

    WriteResult::from_result(flatten(joined))
}

/// 删除一个会话。WBI 签名，csrf 走参数，理由同 [`ack_session`]。
pub async fn remove_session(context: MsgContext, talker_id: i64, csrf: String) -> WriteResult {
    let (ctx, transport, mixin_key, wts) = context.split();
    let joined = init::runtime()
        .spawn(async move {
            msg::remove_session(&transport, &ctx, talker_id, &csrf, &mixin_key, wts).await
        })
        .await;

    WriteResult::from_result(flatten(joined))
}

/// 删除消息中心里的一条。`tp` 是 Tab 类型，`id` 按字符串传（同 #166）。
pub async fn del_msg_feed(context: MsgContext, tp: i64, id: String) -> WriteResult {
    let (ctx, transport, _, _) = context.split();
    let joined = init::runtime()
        .spawn(async move { msg::del_msg_feed(&transport, &ctx, tp, &id).await })
        .await;

    WriteResult::from_result(flatten(joined))
}

/// 删除一条系统通知。
pub async fn del_sys_msg(context: MsgContext, id: String) -> WriteResult {
    let (ctx, transport, _, _) = context.split();
    let joined = init::runtime()
        .spawn(async move { msg::del_sys_msg(&transport, &ctx, &id).await })
        .await;

    WriteResult::from_result(flatten(joined))
}

/// 开关某条赞的通知。`notice_state` 是目标状态，不是切换。
pub async fn set_notice(context: MsgContext, id: String, notice_state: i64) -> WriteResult {
    let (ctx, transport, _, _) = context.split();
    let joined = init::runtime()
        .spawn(async move { msg::set_notice(&transport, &ctx, &id, notice_state).await })
        .await;

    WriteResult::from_result(flatten(joined))
}

/// 设置免打扰。`uid` 是自己的 mid，`dnd_uid` 是对方的。
pub async fn set_msg_dnd(context: MsgContext, uid: i64, setting: i64, dnd_uid: i64) -> WriteResult {
    let (ctx, transport, _, _) = context.split();
    let joined = init::runtime()
        .spawn(async move { msg::set_msg_dnd(&transport, &ctx, uid, setting, dnd_uid).await })
        .await;

    WriteResult::from_result(flatten(joined))
}

/// 设置内容推送开关。
pub async fn set_push_ss(context: MsgContext, setting: i64, talker_uid: i64) -> WriteResult {
    let (ctx, transport, _, _) = context.split();
    let joined = init::runtime()
        .spawn(async move { msg::set_push_ss(&transport, &ctx, setting, talker_uid).await })
        .await;

    WriteResult::from_result(flatten(joined))
}

/// 举报一位私信对象。
///
/// **这条一次都不会重发**（`Idempotency::Unsafe`）：没有幂等键，服务端每
/// 收一次立一条工单，而且查不到上一次立没立。超时就是超时，要不要再举报
/// 一次由用户决定（ADR-013 决策一）。
///
/// `comment_json` 与 `extra_json` 是**已序列化的 JSON 字符串**，由调用方
/// 编码——它们的结构由举报弹窗决定，Rust 这一侧不该替它定义形状。
pub async fn im_report(
    context: MsgContext,
    accused_uid: i64,
    reason_type: i64,
    reason_desc: String,
    comment_json: String,
    extra_json: String,
) -> WriteResult {
    let (ctx, transport, _, _) = context.split();
    let joined = init::runtime()
        .spawn(async move {
            msg::im_report(
                &transport,
                &ctx,
                accused_uid,
                reason_type,
                &reason_desc,
                &comment_json,
                &extra_json,
            )
            .await
        })
        .await;

    WriteResult::from_result(flatten(joined))
}

// ------------------------------------------------------------ 领域 → 导出

fn cursor(c: domain::FeedCursor) -> FeedCursor {
    FeedCursor {
        is_end: c.is_end,
        id: c.id,
        time: c.time,
    }
}

fn user(u: domain::FeedUser) -> FeedUser {
    FeedUser {
        mid: u.mid,
        nickname: u.nickname,
        avatar: u.avatar,
    }
}

fn reply_feed(f: domain::ReplyFeed) -> ReplyFeed {
    ReplyFeed {
        cursor: f.cursor.map(cursor),
        last_view_at: f.last_view_at,
        items: f
            .items
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
    }
}

fn at_feed(f: domain::AtFeed) -> AtFeed {
    AtFeed {
        cursor: f.cursor.map(cursor),
        items: f
            .items
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
    }
}

fn like_item(i: domain::LikeFeedItem) -> LikeFeedItem {
    LikeFeedItem {
        id: i.id,
        users: i.users.into_iter().map(user).collect(),
        item: i.item.map(|c| LikeFeedContent {
            business: c.business,
            title: c.title,
            image: c.image,
            native_uri: c.native_uri,
        }),
        counts: i.counts,
        like_time: i.like_time,
        notice_state: i.notice_state,
    }
}

fn like_feed(f: domain::LikeFeed) -> LikeFeed {
    LikeFeed {
        latest: f.latest.map(|l| LikeLatest {
            items: l.items.into_iter().map(like_item).collect(),
            last_view_at: l.last_view_at,
        }),
        total: f.total.map(|t| LikeTotal {
            cursor: t.cursor.map(cursor),
            items: t.items.into_iter().map(like_item).collect(),
        }),
    }
}

fn like_detail(d: domain::LikeDetail) -> LikeDetail {
    LikeDetail {
        page: d.page.map(|p| LikeDetailPage { is_end: p.is_end }),
        card: d.card.map(|c| LikeDetailCard {
            business: c.business,
            title: c.title,
        }),
        items: d
            .items
            .into_iter()
            .map(|i| LikeDetailItem {
                user: i.user.map(user),
                like_time: i.like_time,
            })
            .collect(),
    }
}

fn sys_msg_item(i: domain::SysMsgItem) -> SysMsgItem {
    SysMsgItem {
        id: i.id,
        cursor: i.cursor,
        title: i.title,
        content: i.content,
        time_at: i.time_at,
    }
}

fn im_user_info(u: domain::ImUserInfo) -> ImUserInfo {
    ImUserInfo {
        mid: u.mid,
        name: u.name,
        face: u.face,
        sign: u.sign,
        vip_status: u.vip_status,
        vip_type: u.vip_type,
        pendant_image: u.pendant_image,
        official_type: u.official_type,
    }
}

/// 把 `spawn` 的双层 Result 压平。与其余桥接模块同一份实现——它们各自
/// 持有自己的，是因为 FRB 的导出扫描按模块走，提到公共模块反而要多一个
/// `#[frb(ignore)]`。
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
