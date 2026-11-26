//! 私信（IM）的 gRPC 取数与写操作（S12 续单 / issue #63）。
//!
//! 对应基线 `lib/grpc/im.dart` 的 **17 个方法**，分属两个服务：
//!
//! | 服务 | 方法 |
//! | --- | --- |
//! | `bilibili.im.interface.v1.ImInterface` | 发私信、分享列表、拉会话消息、总未读、会话详情 |
//! | `bilibili.app.im.v1.im` | 主/二级会话列表、清未读、会话更新、置顶/取消、删列表、设置读写、屏蔽词三条 |
//!
//! 生成的消息止步在本模块：对外只给 [`bili_domain::im`] 的实体
//! （ADR-012 决策五）。
//!
//! ## 幂等定级
//!
//! gRPC 的写操作沿用 ADR-013，但**没有 csrf**（决策七）。这一片：
//!
//! | 方法 | 定级 | 为什么 |
//! | --- | --- | --- |
//! | `SendMsg` | **不可重发** | 见下 |
//! | `PinSession` / `UnpinSession` | 幂等 | 置为语义 |
//! | `ClearUnread` | 幂等 | 置为语义（清到 0） |
//! | `DeleteSessionList` | 幂等 | 删第二次最终状态一致 |
//! | `SetImSettings` | 幂等 | 整张设置表是置为语义 |
//! | `KeywordBlockingAdd` | 可判定 | 重发会多一条，但只读接口查得到 |
//! | `KeywordBlockingDelete` | 幂等 | 删第二次状态一致 |
//!
//! ### 发私信为什么一次都不能重，以及 `dev_id` 的硬约束
//!
//! 重发的表现是**对方收到两条一样的消息**，而发送方的会话里也会多一条。
//! 它和举报（`im_report`，ADR-013 补充十九）同级：代价落在别人身上，
//! 操作者未必察觉。
//!
//! 但它比举报多一样东西：`dev_id`。基线每次调用**现生成一个 UUID**
//! （`const UuidV4().generate()`），服务端拿它去重。于是有一条硬约束：
//!
//! > **`dev_id` 由调用方给，而且必须在一次逻辑发送内固定。**
//!
//! [`send_msg`] 因此收一个 `dev_id` 参数而不是自己生成——自己生成的话，
//! 「重试」这件事在本模块内部就无法与「用户又发了一次」区分开，而服务端
//! 的去重正是靠它。参数化之后，重发安全与否由调用点决定，并且看得见。
//!
//! 定级仍然是 [`Idempotency::Unsafe`]：传输层一次都不重发。`dev_id` 是
//! 第二道闸，不是放开重试的理由——它只保证「万一发生重发，服务端有机会
//! 去重」，而服务端去不去重我们验不了。

use bili_domain::error::{DomainError, ErrorKind};
use bili_domain::im::{
    ImEmote, ImMessage, ImMessagePage, ImSession, ImSessionPage, ImSetting, ImSettingOption,
    ImSettingRedirect, ImSettingsPage, ImTotalUnread, KeywordBlockingList, SessionOffset,
    SessionTarget, SessionTargetKind, ShareTarget, ThreeDotItem, UnreadStyle, fold_page_type,
    msg_kind, official_type_from_local_res, three_dot_kind,
};
use bili_grpc::bilibili::app::im::v1 as pb;
use bili_grpc::bilibili::dagw::component::avatar::common as avatar_common;
use bili_grpc::bilibili::dagw::component::avatar::v1 as avatar;
use bili_grpc::bilibili::im::interfaces::v1 as ipb;
use bili_grpc::bilibili::im::r#type as tpb;
use bili_protocol::grpc as protocol;
use prost::Message;

use crate::http::{HttpTransport, Idempotency, RequestContext, RetryPolicy};

/// 基线 `syncFetchSessionMsgs` 固定一页 20 条。
const SESSION_MSG_PAGE_SIZE: i32 = 20;

/// 私聊的会话类型。基线在五处写死 `sessionType: 1`。
const SESSION_TYPE_PRIVATE: i32 = 1;

/// 私信的接收方类型。基线写死 1（发给人，不是群）。
const RECEIVER_TYPE_USER: i32 = 1;

/// 基线 `sendMsg` 写死的表情版本。
const NEW_FACE_VERSION: i32 = 1;

async fn unary<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    path: &str,
    payload: &impl Message,
    retry: RetryPolicy,
) -> Result<Vec<u8>, DomainError> {
    crate::grpc::unary(transport, ctx, path, &payload.encode_to_vec(), retry).await
}

fn decode<M: Message + Default>(bytes: &[u8], what: &str) -> Result<M, DomainError> {
    M::decode(bytes).map_err(|_| {
        DomainError::new(
            ErrorKind::Protocol,
            format!("{what}响应不是合法的 protobuf"),
        )
    })
}

// ====================================================== 会话列表

/// 从 `vip_info` 那段 JSON 里取会员状态与类型。
///
/// 协议把它塞成一个**字符串**而不是子消息。基线在界面里 `jsonDecode`
/// 之后读 `status` 与 `type`；解析失败就当没有会员信息（基线的
/// `hasVipInfo()` 为假时同样是 null）。
///
/// 放在数据层而不是领域层：`bili-domain` 不依赖 `serde_json`（它是最内
/// 那一层，解析不是它的事），而这就是一次解析。
///
/// 返回 `(status, type)`。
fn vip_from_json(raw: Option<&str>) -> (Option<i64>, Option<i64>) {
    let Some(raw) = raw.filter(|s| !s.is_empty()) else {
        return (None, None);
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) else {
        return (None, None);
    };
    (
        value.get("status").and_then(serde_json::Value::as_i64),
        value.get("type").and_then(serde_json::Value::as_i64),
    )
}

/// 把 `map<int32, Offset>` 摊成有序列表。
///
/// 用 `BTreeMap` 的自然顺序（prost 的 map 就是 BTreeMap），所以同一份响应
/// 每次摊出来的顺序都一样——否则请求快照没法钉。
fn offsets_from(params: Option<&pb::PaginationParams>) -> Vec<SessionOffset> {
    params
        .map(|p| {
            p.offsets
                .iter()
                .map(|(key, offset)| SessionOffset {
                    key: *key,
                    normal_offset: offset.normal_offset.unwrap_or_default(),
                    top_offset: offset.top_offset.unwrap_or_default(),
                })
                .collect()
        })
        .unwrap_or_default()
}

fn pagination_from(offsets: &[SessionOffset]) -> Option<pb::PaginationParams> {
    if offsets.is_empty() {
        return None;
    }
    Some(pb::PaginationParams {
        offsets: offsets
            .iter()
            .map(|o| {
                (
                    o.key,
                    pb::Offset {
                        normal_offset: Some(o.normal_offset),
                        top_offset: Some(o.top_offset),
                    },
                )
            })
            .collect(),
        has_more: None,
    })
}

/// 从头像图层里取一张图的地址。
///
/// 一层的 `resource` 是个 oneof，三种形态都可能装着同一张图：静态图、
/// webp 动图、原生绘制。基线按 `hasResImage()` → `hasResAnimation()` →
/// 否则走 `resNativeDraw` 的顺序试，这里照搬。
///
/// 三种都没有 url 时返回 `None`（例如那一层装的是 `local_value`——认证标
/// 就是这种，它没有 url，见 [`layer_local_value`]）。
fn layer_url(layer: Option<&avatar::Layer>) -> Option<String> {
    use avatar::basic_layer_resource::Payload;
    let payload = layer.and_then(|l| l.resource.as_ref())?.payload.as_ref()?;
    let source = match payload {
        Payload::ResImage(i) => i.image_src.as_ref(),
        Payload::ResAnimation(a) => a.webp_src.as_ref(),
        Payload::ResNativeDraw(d) => d.draw_src.as_ref(),
    }?;
    match source.res.as_ref()? {
        avatar_common::resource_source::Res::Remote(r) => r.url.clone().filter(|u| !u.is_empty()),
        avatar_common::resource_source::Res::LocalValue(_) => None,
    }
}

/// 取一层的 `local_value`。认证标就装在这里，没有 url。
fn layer_local_value(layer: Option<&avatar::Layer>) -> Option<i32> {
    use avatar::basic_layer_resource::Payload;
    let payload = layer.and_then(|l| l.resource.as_ref())?.payload.as_ref()?;
    let source = match payload {
        Payload::ResImage(i) => i.image_src.as_ref(),
        Payload::ResAnimation(a) => a.webp_src.as_ref(),
        Payload::ResNativeDraw(d) => d.draw_src.as_ref(),
    }?;
    match source.res.as_ref()? {
        avatar_common::resource_source::Res::LocalValue(v) => Some(*v),
        avatar_common::resource_source::Res::Remote(_) => None,
    }
}

/// 头像三件套：主图、挂件、认证标。
///
/// **位置就是语义**，这是整段投影里最脆的一处：服务端给的是一组可渲染的
/// 图层，哪一层是什么没有字段标注，基线按下标认（
/// `lib/pages/whisper/widgets/item.dart`）：
///
/// - `layers[0]` —— 主头像；
/// - `layers[1]` —— 挂件（可能没有）；
/// - `layers.last` —— 认证标（可能就是第 0 层，那时它没有 `local_value`，
///   映射出来是 `None`，于是不画）。
///
/// 返回 `(主图, 挂件, 认证类型)`。
fn avatar_from_layers(
    item: Option<&avatar::AvatarItem>,
) -> (Option<String>, Option<String>, Option<i32>) {
    let Some(layers) = item
        .and_then(|a| a.fallback_layers.as_ref())
        .map(|g| g.layers.as_slice())
    else {
        return (None, None, None);
    };

    let face = layer_url(layers.first());
    let pendant = layer_url(layers.get(1));
    // 认证标只认 LocalRes 的那两个值；别的（会员图标、默认头像……）
    // 映射成 None，画成认证标会让一堆人凭空多出个认证。
    let official = layer_local_value(layers.last()).and_then(official_type_from_local_res);
    (face, pendant, official)
}

/// 会话指向哪里。
///
/// `SessionId` 是个 oneof，这里摊成 [`SessionTarget`]。折叠入口顺手把
/// 会话类型映射成二级页类型（[`fold_page_type`]）——那张表两个枚举的数字
/// 几乎每行都不同，留在界面迟早有人拿会话类型直接当页面类型用。
fn target_from(id: Option<&pb::SessionId>) -> SessionTarget {
    use pb::session_id::Id;
    let Some(id) = id.and_then(|i| i.id.as_ref()) else {
        return SessionTarget::default();
    };
    match id {
        Id::PrivateId(p) => SessionTarget {
            kind: SessionTargetKind::Private,
            talker_uid: p.talker_uid,
            ..SessionTarget::default()
        },
        Id::FoldId(f) => {
            let session_type = f.r#type;
            SessionTarget {
                kind: SessionTargetKind::Fold,
                fold_page_type: session_type.and_then(fold_page_type),
                session_type,
                type_name: session_type.map(session_type_name),
                ..SessionTarget::default()
            }
        }
        Id::SystemId(s) => {
            let session_type = s.r#type;
            SessionTarget {
                kind: SessionTargetKind::System,
                session_type,
                type_name: session_type.map(session_type_name),
                ..SessionTarget::default()
            }
        }
        Id::GroupId(_) => SessionTarget {
            kind: SessionTargetKind::Group,
            ..SessionTarget::default()
        },
        Id::CustomerId(_) => SessionTarget {
            kind: SessionTargetKind::Customer,
            ..SessionTarget::default()
        },
    }
}

/// 会话类型的枚举名。界面在「没有对应页面」时拿它提示。
///
/// 名字在这边取：界面没有枚举表，自己拼只会拼出一个数字。
fn session_type_name(value: i32) -> String {
    pb::SessionType::try_from(value).map_or_else(
        |_| format!("SESSION_TYPE_{value}"),
        |t| t.as_str_name().to_owned(),
    )
}

fn unread_from(unread: Option<&pb::Unread>) -> (UnreadStyle, i64) {
    let Some(unread) = unread else {
        return (UnreadStyle::None, 0);
    };
    let style = match unread.style.and_then(|s| pb::UnreadStyle::try_from(s).ok()) {
        Some(pb::UnreadStyle::Number) => UnreadStyle::Number,
        Some(pb::UnreadStyle::Dot) => UnreadStyle::Dot,
        _ => UnreadStyle::None,
    };
    (style, unread.number.unwrap_or_default())
}

fn project_session(session: &pb::Session) -> ImSession {
    let info = session.session_info.as_ref();
    let (avatar_url, pendant_url, official_type) =
        avatar_from_layers(info.and_then(|i| i.avatar.as_ref()));
    let (vip_status, vip_type) = vip_from_json(info.and_then(|i| i.vip_info.as_deref()));
    let (unread_style, unread_number) = unread_from(session.unread.as_ref());

    ImSession {
        target: target_from(session.id.as_ref()),
        name: info
            .and_then(|i| i.session_name.clone())
            .unwrap_or_default(),
        avatar_url,
        pendant_url,
        official_type,
        vip_status,
        vip_type,
        label_text: info
            .and_then(|i| i.user_label.as_ref())
            .and_then(|l| l.style.as_ref())
            .and_then(|s| s.style.as_ref())
            // 裁剪后 `UserLabelStyle` 只剩描边这一支（见 proto 生成器的
            // KEEP），所以这里没有通配分支——上游再加一种标签样式时，
            // 这个 match 会直接编译失败，而不是悄悄不画。
            .and_then(|s| {
                let pb::user_label_style::Style::BorderedLabel(b) = s;
                b.text.clone().filter(|t| !t.is_empty())
            }),
        is_live: info.and_then(|i| i.is_live).unwrap_or_default(),
        is_pinned: session.is_pinned.unwrap_or_default(),
        is_muted: session.is_muted.unwrap_or_default(),
        timestamp_us: session.timestamp,
        summary: session
            .msg_summary
            .as_ref()
            .and_then(|s| s.raw_msg.clone())
            .filter(|m| !m.is_empty()),
        unread_style,
        unread_number,
        mid: info.and_then(|i| i.avatar.as_ref()).and_then(|a| a.mid),
    }
}

fn project_three_dots(items: &[pb::ThreeDotItem]) -> Vec<ThreeDotItem> {
    items
        .iter()
        .map(|i| ThreeDotItem {
            title: i.title.clone(),
            icon: i.icon.clone(),
            url: i.url.clone(),
            kind: three_dot_kind(i.r#type),
            has_red_dot: i.has_red_dot.unwrap_or_default(),
        })
        .collect()
}

/// 取主会话列表。
///
/// # Errors
/// 传输、gRPC 状态、protobuf 解码三类失败。
pub async fn session_main<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    offsets: &[SessionOffset],
) -> Result<ImSessionPage, DomainError> {
    let req = pb::SessionMainReq {
        restricted_mode: None,
        pagination_params: pagination_from(offsets),
        filter_type: None,
    };
    let bytes = unary(
        transport,
        ctx,
        protocol::IM_SESSION_MAIN,
        &req,
        RetryPolicy::Read,
    )
    .await?;
    project_session_main(&bytes)
}

/// 把 `SessionMain` 的响应字节投影成一页会话。
///
/// 与发请求分开，是为了让投影规则能脱离网络被钉住
/// （`tests/grpc_im_golden.rs`）——那几段按位置取头像、按表映射页面类型
/// 的规则才是这一片真正会出错的地方。
///
/// # Errors
/// 字节不是合法的 protobuf 时为 [`ErrorKind::Protocol`]。
pub fn project_session_main(bytes: &[u8]) -> Result<ImSessionPage, DomainError> {
    let reply: pb::SessionMainReply = decode(bytes, "私信会话列表")?;
    Ok(ImSessionPage {
        sessions: reply.sessions.iter().map(project_session).collect(),
        offsets: offsets_from(reply.pagination_params.as_ref()),
        has_more: reply
            .pagination_params
            .as_ref()
            .and_then(|p| p.has_more)
            .unwrap_or_default(),
        three_dot_items: project_three_dots(&reply.three_dot_items),
        outside_items: project_three_dots(&reply.outside_item),
    })
}

/// 取二级会话列表（陌生人、未关注、垃圾桶……）。
///
/// # Errors
/// 同上。
pub async fn session_secondary<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    page_type: i32,
    offsets: &[SessionOffset],
) -> Result<ImSessionPage, DomainError> {
    let req = pb::SessionSecondaryReq {
        restricted_mode: None,
        pagination_params: pagination_from(offsets),
        page_type: Some(page_type),
    };
    let bytes = unary(
        transport,
        ctx,
        protocol::IM_SESSION_SECONDARY,
        &req,
        RetryPolicy::Read,
    )
    .await?;
    project_session_secondary(&bytes)
}

/// 把 `SessionSecondary` 的响应字节投影成一页会话。见
/// [`project_session_main`]。
///
/// # Errors
/// 同上。
pub fn project_session_secondary(bytes: &[u8]) -> Result<ImSessionPage, DomainError> {
    let reply: pb::SessionSecondaryReply = decode(bytes, "二级私信会话列表")?;
    Ok(ImSessionPage {
        sessions: reply.sessions.iter().map(project_session).collect(),
        offsets: offsets_from(reply.pagination_params.as_ref()),
        has_more: reply
            .pagination_params
            .as_ref()
            .and_then(|p| p.has_more)
            .unwrap_or_default(),
        three_dot_items: project_three_dots(&reply.three_dot_items),
        // 二级列表的响应里没有「菜单外」那一组。
        outside_items: Vec::new(),
    })
}

/// 取单个会话的最新状态。界面用它查「现在是不是置顶」。
///
/// # Errors
/// 同上。
pub async fn session_update<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    talker_uid: i64,
) -> Result<Option<ImSession>, DomainError> {
    let req = pb::SessionUpdateReq {
        session_id: Some(private_session_id(talker_uid)),
        page_type: None,
    };
    let bytes = unary(
        transport,
        ctx,
        protocol::IM_SESSION_UPDATE,
        &req,
        RetryPolicy::Read,
    )
    .await?;
    let reply: pb::SessionUpdateReply = decode(&bytes, "会话状态")?;
    Ok(reply.session.as_ref().map(project_session))
}

fn private_session_id(talker_uid: i64) -> pb::SessionId {
    pb::SessionId {
        id: Some(pb::session_id::Id::PrivateId(pb::PrivateId {
            talker_uid: Some(talker_uid),
        })),
    }
}

// ====================================================== 会话上的写操作

/// 置顶一个会话。置为语义，重发安全。
///
/// # Errors
/// 同上。
pub async fn pin_session<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    talker_uid: i64,
) -> Result<(), DomainError> {
    let req = pb::PinSessionReq {
        session_id: Some(private_session_id(talker_uid)),
        top_time_micros: None,
    };
    unary(
        transport,
        ctx,
        protocol::IM_PIN_SESSION,
        &req,
        Idempotency::Idempotent.retry_policy(),
    )
    .await?;
    Ok(())
}

/// 取消置顶。
///
/// # Errors
/// 同上。
pub async fn unpin_session<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    talker_uid: i64,
) -> Result<(), DomainError> {
    let req = pb::UnPinSessionReq {
        session_id: Some(private_session_id(talker_uid)),
    };
    unary(
        transport,
        ctx,
        protocol::IM_UNPIN_SESSION,
        &req,
        Idempotency::Idempotent.retry_policy(),
    )
    .await?;
    Ok(())
}

/// 把某一页的未读全部清掉。
///
/// # Errors
/// 同上。
pub async fn clear_unread<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    page_type: i32,
) -> Result<(), DomainError> {
    let req = pb::ClearUnreadReq {
        page_type: Some(page_type),
        session_id: None,
    };
    unary(
        transport,
        ctx,
        protocol::IM_CLEAR_UNREAD,
        &req,
        Idempotency::Idempotent.retry_policy(),
    )
    .await?;
    Ok(())
}

/// 删掉某一页的整个会话列表。
///
/// # Errors
/// 同上。
pub async fn delete_session_list<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    page_type: i32,
) -> Result<(), DomainError> {
    let req = pb::DeleteSessionListReq {
        page_type: Some(page_type),
    };
    unary(
        transport,
        ctx,
        protocol::IM_DELETE_SESSION_LIST,
        &req,
        Idempotency::Idempotent.retry_policy(),
    )
    .await?;
    Ok(())
}

// ====================================================== 消息

fn project_message(msg: &tpb::Msg) -> ImMessage {
    ImMessage {
        sender_uid: msg.sender_uid.unwrap_or_default(),
        receiver_id: msg.receiver_id.unwrap_or_default(),
        msg_type: msg.msg_type.unwrap_or_default(),
        kind: msg_kind(msg.msg_type.unwrap_or_default()),
        msg_source: msg.msg_source.unwrap_or_default(),
        msg_status: msg.msg_status.unwrap_or_default(),
        msg_key: msg.msg_key.unwrap_or_default(),
        msg_seqno: msg.msg_seqno.unwrap_or_default(),
        timestamp: msg.timestamp.unwrap_or_default(),
        content: msg.content.clone().unwrap_or_default(),
    }
}

/// 拉一页会话消息。
///
/// `end_seqno` 是上一页最老那条的 seqno；首屏传 `None`。`begin_seqno`
/// 跟着基线走：翻页时固定传 0，首屏不传——两者都给才是「往回翻」。
///
/// # Errors
/// 同上。
pub async fn fetch_session_msgs<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    talker_id: i64,
    end_seqno: Option<i64>,
) -> Result<ImMessagePage, DomainError> {
    let req = ipb::ReqSessionMsg {
        talker_id: Some(talker_id),
        session_type: Some(SESSION_TYPE_PRIVATE),
        end_seqno,
        begin_seqno: end_seqno.map(|_| 0),
        size: Some(SESSION_MSG_PAGE_SIZE),
        order: None,
        // 基线这里写死 '1'，不是 UUID——拉取不需要去重。
        dev_id: Some("1".to_owned()),
        ..ipb::ReqSessionMsg::default()
    };
    let bytes = unary(
        transport,
        ctx,
        protocol::IM_SYNC_FETCH_SESSION_MSGS,
        &req,
        RetryPolicy::Read,
    )
    .await?;
    project_session_msgs(&bytes)
}

/// 把 `SyncFetchSessionMsgs` 的响应字节投影成一页消息。见
/// [`project_session_main`]。
///
/// # Errors
/// 同上。
pub fn project_session_msgs(bytes: &[u8]) -> Result<ImMessagePage, DomainError> {
    let reply: ipb::RspSessionMsg = decode(bytes, "会话消息")?;
    Ok(ImMessagePage {
        messages: reply.messages.iter().map(project_message).collect(),
        emotes: reply
            .e_infos
            .iter()
            .filter_map(|e| {
                Some(ImEmote {
                    text: e.text.clone().filter(|t| !t.is_empty())?,
                    url: e.url.clone().unwrap_or_default(),
                    size: e.size.unwrap_or_default(),
                    gif_url: e.gif_url.clone().filter(|u| !u.is_empty()),
                })
            })
            .collect(),
        // 协议是 int32，0 表示没有更多。
        has_more: reply.has_more.unwrap_or_default() != 0,
    })
}

/// 要发出去的那条私信。
///
/// 做成结构体而不是六个位置参数：`sender_uid` 与 `receiver_id` 同类型
/// 且相邻，传反了编译器不会说话，而表现是「消息发给了自己」。
#[derive(Debug, Clone)]
pub struct OutgoingMessage<'a> {
    pub sender_uid: i64,
    pub receiver_id: i64,
    pub msg_type: i32,
    /// 已序列化的内容。形状随 `msg_type` 变，由调用方编码。
    pub content: &'a str,
    /// Unix 秒。由调用方注入，使请求可被 fixture 固定。
    pub timestamp: i64,
    /// 服务端拿它去重。**必须在一次逻辑发送内固定**，见模块文档。
    pub dev_id: &'a str,
}

/// 发一条私信。**不可重发**，见模块文档。
///
/// # Errors
/// 同上。
pub async fn send_msg<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    msg: &OutgoingMessage<'_>,
) -> Result<(), DomainError> {
    let req = ipb::ReqSendMsg {
        msg: Some(tpb::Msg {
            sender_uid: Some(msg.sender_uid),
            receiver_type: Some(RECEIVER_TYPE_USER),
            receiver_id: Some(msg.receiver_id),
            msg_type: Some(msg.msg_type),
            content: Some(msg.content.to_owned()),
            timestamp: Some(msg.timestamp),
            msg_status: Some(0),
            new_face_version: Some(NEW_FACE_VERSION),
            ..tpb::Msg::default()
        }),
        dev_id: Some(msg.dev_id.to_owned()),
        ..ipb::ReqSendMsg::default()
    };
    unary(
        transport,
        ctx,
        protocol::IM_SEND_MSG,
        &req,
        Idempotency::Unsafe.retry_policy(),
    )
    .await?;
    Ok(())
}

/// 取会话详情。界面只要 `ack_seqno`（「标为已读」的位点）。
///
/// # Errors
/// 同上。
pub async fn session_detail<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    talker_id: i64,
) -> Result<i64, DomainError> {
    let req = ipb::ReqSessionDetail {
        talker_id: Some(talker_id),
        session_type: Some(SESSION_TYPE_PRIVATE),
        uid: None,
    };
    let bytes = unary(
        transport,
        ctx,
        protocol::IM_SESSION_DETAIL,
        &req,
        RetryPolicy::Read,
    )
    .await?;
    let reply: tpb::SessionInfo = decode(&bytes, "会话详情")?;
    Ok(reply.ack_seqno.unwrap_or_default())
}

/// 取「最近聊过的人」，分享面板用。
///
/// # Errors
/// 同上。
pub async fn share_list<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    size: i32,
) -> Result<Vec<ShareTarget>, DomainError> {
    let req = ipb::ReqShareList {
        size: Some(size),
        source: None,
    };
    let bytes = unary(
        transport,
        ctx,
        protocol::IM_SHARE_LIST,
        &req,
        RetryPolicy::Read,
    )
    .await?;
    let reply: ipb::RspShareList = decode(&bytes, "分享列表")?;
    Ok(reply
        .session_list
        .iter()
        .map(|s| ShareTarget {
            talker_id: s.talker_id.unwrap_or_default(),
            name: s.talker_uname.clone().filter(|n| !n.is_empty()),
            avatar: s.talker_icon.clone().filter(|a| !a.is_empty()),
        })
        .collect())
}

/// 取消息中心四个 Tab 的未读数（gRPC 版）。
///
/// 响应里那张表的 key 是字符串（`reply`/`at`/`like`/`sys_msg`）。缺键
/// 当 0，与基线一致。
///
/// # Errors
/// 同上。
pub async fn total_unread<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    unread_type: i32,
) -> Result<ImTotalUnread, DomainError> {
    let req = ipb::ReqTotalUnread {
        unread_type: Some(unread_type),
        show_unfollow_list: Some(1),
        ..ipb::ReqTotalUnread::default()
    };
    let bytes = unary(
        transport,
        ctx,
        protocol::IM_GET_TOTAL_UNREAD,
        &req,
        RetryPolicy::Read,
    )
    .await?;
    let reply: ipb::RspTotalUnread = decode(&bytes, "未读总数")?;
    let map = reply
        .msg_feed_unread
        .as_ref()
        .map(|m| &m.unread)
        .ok_or_else(|| {
            DomainError::new(ErrorKind::Protocol, "未读总数响应里没有 msg_feed_unread")
        })?;
    let get = |key: &str| map.get(key).copied().unwrap_or_default();
    Ok(ImTotalUnread {
        reply: get("reply"),
        at: get("at"),
        like: get("like"),
        sys_msg: get("sys_msg"),
    })
}

// ====================================================== 设置与屏蔽词

/// 把一项设置摊平给界面。
///
/// 协议的 `Setting` 是个 oneof：开关 / 选择 / 文本 / 跳转。界面画的是
/// 前两种加跳转那一项的标题与「当前选的是什么」。
fn project_setting(key: i32, setting: &pb::Setting) -> ImSetting {
    use pb::setting::Content;
    let mut out = ImSetting {
        key,
        ..ImSetting::default()
    };
    match setting.content.as_ref() {
        Some(Content::Switch(s)) => {
            out.title = s.title.clone();
            out.subtitle = s.subtitle.clone();
            out.switch_on = Some(s.switch_on.unwrap_or_default());
        }
        Some(Content::Select(s)) => {
            out.options = s
                .item
                .iter()
                .map(|i| ImSettingOption {
                    title: i.text.clone(),
                    value: i.item_type.unwrap_or_default(),
                })
                .collect();
            out.selected_index = s
                .item
                .iter()
                .position(|i| i.selected.unwrap_or_default())
                .and_then(|i| i32::try_from(i).ok());
        }
        Some(Content::Redirect(r)) => {
            out.title = r.title.clone();
            out.subtitle = r.subtitle.clone();
            out.redirect = Some(project_redirect(r));
        }
        Some(Content::Text(t)) => {
            out.title = t.text.clone();
        }
        None => {}
    }
    out
}

/// 跳转型设置的去向，以及那一行右侧显示什么。
fn project_redirect(r: &pb::SettingRedirect) -> ImSettingRedirect {
    use pb::setting_redirect::Content;
    let mut out = ImSettingRedirect {
        selected_summary: r.selected_summary.clone().filter(|s| !s.is_empty()),
        ..ImSettingRedirect::default()
    };
    match r.content.as_ref() {
        Some(Content::SettingPage(p)) => {
            out.parent_setting_type = p.parent_setting_type;
            out.page_url = p.url.clone().filter(|u| !u.is_empty());
            out.trailing_text = sub_setting_trailing(&p.sub_settings);
        }
        Some(Content::OtherPage(p)) => {
            out.other_url = p.url.clone().filter(|u| !u.is_empty());
        }
        Some(Content::WindowSelect(w)) => {
            out.window_options = w
                .item
                .iter()
                .map(|i| ImSettingOption {
                    title: i.text.clone(),
                    value: i.item_type.unwrap_or_default(),
                })
                .collect();
            out.window_selected_index = w
                .item
                .iter()
                .position(|i| i.selected.unwrap_or_default())
                .and_then(|i| i32::try_from(i).ok());
        }
        // 弹窗型界面没有分支，基线点了什么也不做。
        Some(Content::Popup(_)) | None => {}
    }
    if out.trailing_text.is_none() {
        out.trailing_text = out.selected_summary.clone();
    }
    out
}

/// 子设置里拿哪一条当这一行的 trailing 文字。
///
/// 照抄基线 `lib/pages/whisper_settings/widgets/item.dart` 的那段循环，
/// **包括它的两处不对称**：
///
/// - 选择型取**第一个选中的选项**，找到之后 `break` 的是内层循环，外层
///   还会继续看后面的子设置；
/// - 开关型取**第一个打开着的**开关的标题，找到就 `break` 掉整个循环。
///
/// 于是「先遇到一个选中的选项、后面又有一个打开着的开关」时，结果是那个
/// 选项（开关那一支的 `break` 轮不到）。看着像手滑，但它是既有行为，
/// 照搬。
fn sub_setting_trailing(subs: &std::collections::BTreeMap<i32, pb::Setting>) -> Option<String> {
    let mut selected: Option<String> = None;
    for setting in subs.values() {
        match setting.content.as_ref() {
            Some(pb::setting::Content::Select(s)) => {
                if selected.is_none() {
                    selected = s
                        .item
                        .iter()
                        .find(|i| i.selected.unwrap_or_default())
                        .and_then(|i| i.text.clone());
                }
            }
            // 基线这里 break 掉整个循环，而且**不覆盖**已经找到的选项
            // ——它用的是两个不同的变量。
            Some(pb::setting::Content::Switch(sw)) if sw.switch_on.unwrap_or_default() => {
                return selected.or_else(|| sw.title.clone());
            }
            _ => {}
        }
    }
    selected
}

/// 一次设置读取的结果：给界面看的投影，外加**原样的响应字节**。
///
/// `raw` 是整条 `GetImSettingsReply` 的 protobuf 字节。写回时拿它重新
/// 解码、改一处、再编码（见 [`set_settings`]）。
///
/// ## 为什么要带着字节走一圈
///
/// `SetImSettings` 收的是**整张设置表**，基线的做法是把读回来的那张表
/// 原地改一个字段再整个发回去。如果按投影出来的窄实体重建一个
/// `Setting`，发出去的就是一个**缺字段的赝品**——它编译得过、长得像、
/// 服务端多半也收，但凡是我们没投影的字段（跳转里的子设置树、文案、
/// url……）都会在这一次写回里被悄悄抹掉。
///
/// 代价是 Dart 侧要拿着一段不透明的字节。这比「可能悄悄删掉用户的别的
/// 设置」便宜得多，而且它逼着改动集中在一处（[`set_settings`]）。
#[derive(Debug, Clone)]
pub struct ImSettingsSnapshot {
    pub page: ImSettingsPage,
    pub raw: Vec<u8>,
}

/// 取私信设置页。
///
/// # Errors
/// 同上。
pub async fn get_settings<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    setting_type: i32,
) -> Result<ImSettingsSnapshot, DomainError> {
    let req = pb::GetImSettingsReq {
        r#type: Some(setting_type),
    };
    let bytes = unary(
        transport,
        ctx,
        protocol::IM_GET_SETTINGS,
        &req,
        RetryPolicy::Read,
    )
    .await?;
    Ok(ImSettingsSnapshot {
        page: project_settings(&bytes)?,
        raw: bytes,
    })
}

/// 把 `GetImSettings` 的响应字节投影成设置页。见
/// [`project_session_main`]。
///
/// # Errors
/// 同上。
pub fn project_settings(bytes: &[u8]) -> Result<ImSettingsPage, DomainError> {
    let reply: pb::GetImSettingsReply = decode(bytes, "私信设置")?;
    Ok(ImSettingsPage {
        page_title: reply.page_title.clone().unwrap_or_default(),
        settings: reply
            .settings
            .iter()
            .map(|(key, setting)| project_setting(*key, setting))
            .collect(),
    })
}

/// 改一项设置并写回。
///
/// `raw` 是 [`get_settings`] 带回来的那段字节。这里重新解码它、取出
/// `key` 那一项改掉，**只把这一项**发回去（#276）。基线就是这样：
/// `SetImSettingsReq(settings: {key: item})`。整张表发回去服务端回 -400，
/// 所以「带着原始字节走一圈」只用在这一项内部——它里面我们没投影的
/// 字段原样留着，不按投影重建。
///
/// `switch_on` 与 `selected_value` 二选一：前者改开关，后者把选择项里
/// `item_type == selected_value` 的那条设成选中、其余取消。
///
/// # Errors
/// 除传输三类外，`raw` 解不开或 `key` 不在表里时为
/// [`ErrorKind::Protocol`]——那说明界面拿着的是另一次读取的快照。
pub async fn set_settings<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    raw: &[u8],
    key: i32,
    switch_on: Option<bool>,
    selected_value: Option<i32>,
) -> Result<(), DomainError> {
    let req = set_settings_request(raw, key, switch_on, selected_value)?;
    unary(
        transport,
        ctx,
        protocol::IM_SET_SETTINGS,
        &req,
        Idempotency::Idempotent.retry_policy(),
    )
    .await?;
    Ok(())
}

/// [`set_settings`] 发出去的那个请求。单独拆出来，是为了让单测能断言
/// 「请求里只有被改的那一项」——#276 正是错在这里。
///
/// # Errors
/// 同 [`set_settings`] 的协议错误。
pub fn set_settings_request(
    raw: &[u8],
    key: i32,
    switch_on: Option<bool>,
    selected_value: Option<i32>,
) -> Result<pb::SetImSettingsReq, DomainError> {
    let mut reply: pb::GetImSettingsReply = decode(raw, "私信设置快照")?;
    let mut setting = reply.settings.remove(&key).ok_or_else(|| {
        DomainError::new(
            ErrorKind::Protocol,
            format!("设置快照里没有 key {key}，界面拿的可能是另一次读取的结果"),
        )
    })?;

    match (setting.content.as_mut(), switch_on, selected_value) {
        (Some(pb::setting::Content::Switch(s)), Some(on), _) => s.switch_on = Some(on),
        (Some(pb::setting::Content::Select(s)), _, Some(value)) => {
            for item in &mut s.item {
                item.selected = Some(item.item_type == Some(value));
            }
        }
        // 跳转型里的弹窗选择：选项在 redirect 里，不在顶层。基线改完
        // 还会把 `selected_summary` 同步成选中项的文字，一并照搬——
        // 不然写回之后这一行右侧还显示着旧值。
        (Some(pb::setting::Content::Redirect(r)), _, Some(value)) => {
            let Some(pb::setting_redirect::Content::WindowSelect(w)) = r.content.as_mut() else {
                return Err(DomainError::new(
                    ErrorKind::Protocol,
                    "这一项是跳转型，但不是弹窗选择",
                ));
            };
            let mut picked = None;
            for item in &mut w.item {
                let hit = item.item_type == Some(value);
                item.selected = Some(hit);
                if hit {
                    picked = item.text.clone();
                }
            }
            if let Some(text) = picked {
                r.selected_summary = Some(text);
            }
        }
        _ => {
            return Err(DomainError::new(
                ErrorKind::Protocol,
                "设置项的形态与要改的内容对不上",
            ));
        }
    }

    Ok(pb::SetImSettingsReq {
        settings: [(key, setting)].into_iter().collect(),
    })
}

/// 取关键词屏蔽列表。
///
/// # Errors
/// 同上。
pub async fn keyword_blocking_list<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
) -> Result<KeywordBlockingList, DomainError> {
    let bytes = unary(
        transport,
        ctx,
        protocol::IM_KEYWORD_BLOCKING_LIST,
        &pb::KeywordBlockingListReq {},
        RetryPolicy::Read,
    )
    .await?;
    let reply: pb::KeywordBlockingListReply = decode(&bytes, "屏蔽词列表")?;
    Ok(KeywordBlockingList {
        keywords: reply
            .items
            .iter()
            .filter_map(|i| i.keyword.clone().filter(|k| !k.is_empty()))
            .collect(),
        list_limit: reply.list_limit,
        char_limit: reply.char_limit,
    })
}

/// 加一个屏蔽词。
///
/// 定级 [`Idempotency::Verifiable`]：服务端语义是「加一条」，重发可能多
/// 一条；但只读的 [`keyword_blocking_list`] 查得到。这一档传输层不重试
/// （ADR-013 决策一）。
///
/// # Errors
/// 同上。
pub async fn keyword_blocking_add<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    keyword: &str,
) -> Result<(), DomainError> {
    let req = pb::KeywordBlockingAddReq {
        keyword: Some(keyword.to_owned()),
    };
    unary(
        transport,
        ctx,
        protocol::IM_KEYWORD_BLOCKING_ADD,
        &req,
        Idempotency::Verifiable.retry_policy(),
    )
    .await?;
    Ok(())
}

/// 删一个屏蔽词。删第二次状态一致，所以是幂等。
///
/// # Errors
/// 同上。
pub async fn keyword_blocking_delete<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    keyword: &str,
) -> Result<(), DomainError> {
    let req = pb::KeywordBlockingDeleteReq {
        keyword: Some(keyword.to_owned()),
    };
    unary(
        transport,
        ctx,
        protocol::IM_KEYWORD_BLOCKING_DELETE,
        &req,
        Idempotency::Idempotent.retry_policy(),
    )
    .await?;
    Ok(())
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use bili_domain::im::{page_type, session_type};

    /// 领域层那两组常量是**手写**的（`bili-domain` 不依赖 `bili-grpc`，
    /// 分层不允许）。这条测试就是它们唯一的锚：在能看见生成枚举的这一层
    /// 逐个核对。
    ///
    /// 写这张表时真的把数字猜错了——两个枚举的顺序不一样
    /// （`SessionPageType` 里 `Home` 占了 1，把后面整体顶了一位），
    /// 而猜错之后单测仍然全绿（它对着同一组错值自洽），表现只是「点折叠
    /// 入口打开了另一个列表」。
    #[test]
    fn domain_enum_constants_match_the_generated_ones() {
        assert_eq!(session_type::UNKNOWN, pb::SessionType::Unknown as i32);
        assert_eq!(session_type::PRIVATE, pb::SessionType::Private as i32);
        assert_eq!(session_type::GROUP, pb::SessionType::Group as i32);
        assert_eq!(session_type::GROUP_FOLD, pb::SessionType::GroupFold as i32);
        assert_eq!(session_type::UNFOLLOWED, pb::SessionType::Unfollowed as i32);
        assert_eq!(session_type::STRANGER, pb::SessionType::Stranger as i32);
        assert_eq!(session_type::DUSTBIN, pb::SessionType::Dustbin as i32);
        assert_eq!(
            session_type::CUSTOMER_FOLD,
            pb::SessionType::CustomerFold as i32
        );
        assert_eq!(session_type::SYSTEM, pb::SessionType::System as i32);
        assert_eq!(session_type::AI_FOLD, pb::SessionType::AiFold as i32);
        assert_eq!(
            session_type::CUSTOMER_ACCOUNT,
            pb::SessionType::CustomerAccount as i32
        );

        assert_eq!(page_type::UNKNOWN, pb::SessionPageType::Unknown as i32);
        assert_eq!(page_type::HOME, pb::SessionPageType::Home as i32);
        assert_eq!(
            page_type::UNFOLLOWED,
            pb::SessionPageType::Unfollowed as i32
        );
        assert_eq!(page_type::STRANGER, pb::SessionPageType::Stranger as i32);
        assert_eq!(page_type::DUSTBIN, pb::SessionPageType::Dustbin as i32);
        assert_eq!(page_type::GROUP, pb::SessionPageType::Group as i32);
        assert_eq!(page_type::HUA_HUO, pb::SessionPageType::HuaHuo as i32);
        assert_eq!(page_type::AI, pb::SessionPageType::Ai as i32);
        assert_eq!(page_type::CUSTOMER, pb::SessionPageType::Customer as i32);
    }

    /// 「…」菜单与设置页那两组常量同样是手写的，锚在这里。
    #[test]
    fn three_dot_and_setting_constants_match_the_generated_ones() {
        use bili_domain::im::{setting_page_type, three_dot_type};
        assert_eq!(
            three_dot_type::READ_ALL,
            pb::ThreeDotItemType::ReadAll as i32
        );
        assert_eq!(
            three_dot_type::MSG_SETTING,
            pb::ThreeDotItemType::MsgSetting as i32
        );
        assert_eq!(
            three_dot_type::UP_HELPER,
            pb::ThreeDotItemType::UpHelper as i32
        );
        assert_eq!(
            three_dot_type::FANS_GROUP_HELPER,
            pb::ThreeDotItemType::FansGroupHelper as i32
        );
        assert_eq!(
            three_dot_type::CONTACTS,
            pb::ThreeDotItemType::Contacts as i32
        );
        assert_eq!(
            three_dot_type::CLEAR_LIST,
            pb::ThreeDotItemType::ClearList as i32
        );

        assert_eq!(
            setting_page_type::ALL,
            pb::ImSettingType::SettingTypeNeedAll as i32
        );
        assert_eq!(
            setting_page_type::OLD_REPLY_ME,
            pb::ImSettingType::SettingTypeOldReplyMe as i32
        );
        assert_eq!(
            setting_page_type::OLD_AT_ME,
            pb::ImSettingType::SettingTypeOldAtMe as i32
        );
        assert_eq!(
            setting_page_type::OLD_RECEIVE_LIKE,
            pb::ImSettingType::SettingTypeOldReceiveLike as i32
        );
    }

    /// 消息类型那十二个取值同样是手写的，锚在这里。
    ///
    /// 基线把数字编进了函数名（`msgTypeText_1`、`msgTypeShareV2_7`、
    /// `msgTypeTipMessage_18`……），算是另一重交叉验证。16 是个例外：
    /// 协议里**没有**这个名字，基线按数字单独画一种卡片。
    #[test]
    fn msg_type_constants_match_the_generated_enum() {
        use bili_domain::im::msg_type as m;
        use bili_grpc::bilibili::im::r#type::MsgType as T;
        assert_eq!(m::TEXT, T::EnMsgTypeText as i32);
        assert_eq!(m::PIC, T::EnMsgTypePic as i32);
        assert_eq!(m::DRAW_BACK, T::EnMsgTypeDrawBack as i32);
        assert_eq!(m::CUSTOM_FACE, T::EnMsgTypeCustomFace as i32);
        assert_eq!(m::SHARE_V2, T::EnMsgTypeShareV2 as i32);
        assert_eq!(m::NOTIFY_MSG, T::EnMsgTypeNotifyMsg as i32);
        assert_eq!(m::VIDEO_CARD, T::EnMsgTypeVideoCard as i32);
        assert_eq!(m::ARTICLE_CARD, T::EnMsgTypeArticleCard as i32);
        assert_eq!(m::PICTURE_CARD, T::EnMsgTypePictureCard as i32);
        assert_eq!(m::COMMON_SHARE_CARD, T::EnMsgTypeCommonShareCard as i32);
        assert_eq!(m::TIP_MESSAGE, T::EnMsgTypeTipMessage as i32);
        // 16 刻意不在枚举里：协议的 15 是 TextShare、18 是 TipMessage，
        // 中间那个没有名字。确认它确实还没被占用——哪天上游补了名字，
        // 这条分支要重新对一遍。
        assert!(
            T::try_from(m::UNNAMED_16).is_err(),
            "16 一旦在协议里有了名字，按数字画的那条分支要重新对"
        );
    }

    /// 认证标那两个值同样是手写的，锚在这里。
    #[test]
    fn local_res_verify_values_match_the_generated_enum() {
        use avatar_common::resource_source::LocalRes;
        assert_eq!(
            official_type_from_local_res(LocalRes::IconPersonalVerify as i32),
            Some(0)
        );
        assert_eq!(
            official_type_from_local_res(LocalRes::IconEnterpriseVerify as i32),
            Some(1)
        );
        // 同一个枚举里的会员图标不是认证标。
        assert_eq!(official_type_from_local_res(LocalRes::IconVip as i32), None);
    }

    fn layer_with_remote(url: &str) -> avatar::Layer {
        avatar::Layer {
            resource: Some(avatar::BasicLayerResource {
                payload: Some(avatar::basic_layer_resource::Payload::ResImage(
                    avatar::ResImage {
                        image_src: Some(avatar_common::ResourceSource {
                            res: Some(avatar_common::resource_source::Res::Remote(
                                avatar_common::RemoteRes {
                                    url: Some(url.to_owned()),
                                },
                            )),
                            ..avatar_common::ResourceSource::default()
                        }),
                    },
                )),
            }),
        }
    }

    fn layer_with_local(value: i32) -> avatar::Layer {
        avatar::Layer {
            resource: Some(avatar::BasicLayerResource {
                payload: Some(avatar::basic_layer_resource::Payload::ResImage(
                    avatar::ResImage {
                        image_src: Some(avatar_common::ResourceSource {
                            res: Some(avatar_common::resource_source::Res::LocalValue(value)),
                            ..avatar_common::ResourceSource::default()
                        }),
                    },
                )),
            }),
        }
    }

    fn avatar_of(layers: Vec<avatar::Layer>) -> avatar::AvatarItem {
        avatar::AvatarItem {
            fallback_layers: Some(avatar::LayerGroup { layers }),
            mid: Some(100_001),
        }
    }

    /// 位置就是语义：第 0 层主图、第 1 层挂件、最后一层认证标。
    #[test]
    fn avatar_layers_are_read_by_position() {
        let item = avatar_of(vec![
            layer_with_remote("https://example.invalid/face.jpg"),
            layer_with_remote("https://example.invalid/pendant.png"),
            layer_with_local(4),
        ]);
        let (face, pendant, official) = avatar_from_layers(Some(&item));
        assert_eq!(face.as_deref(), Some("https://example.invalid/face.jpg"));
        assert_eq!(
            pendant.as_deref(),
            Some("https://example.invalid/pendant.png")
        );
        assert_eq!(official, Some(1), "机构认证");
    }

    /// 只有一层时，「最后一层」就是第 0 层——它是主图，没有 local_value，
    /// 于是认证标是 None。不这样的话每个没有挂件的人都会多出个认证标。
    #[test]
    fn a_single_layer_avatar_has_no_pendant_and_no_badge() {
        let item = avatar_of(vec![layer_with_remote("https://example.invalid/face.jpg")]);
        let (face, pendant, official) = avatar_from_layers(Some(&item));
        assert!(face.is_some());
        assert_eq!(pendant, None);
        assert_eq!(official, None);
    }

    #[test]
    fn an_avatar_without_layers_projects_to_nothing() {
        assert_eq!(avatar_from_layers(None), (None, None, None));
        assert_eq!(
            avatar_from_layers(Some(&avatar_of(vec![]))),
            (None, None, None)
        );
    }

    #[test]
    fn a_fold_target_carries_the_mapped_page_type_and_the_enum_name() {
        let id = pb::SessionId {
            id: Some(pb::session_id::Id::FoldId(pb::FoldId {
                r#type: Some(session_type::AI_FOLD),
            })),
        };
        let target = target_from(Some(&id));
        assert_eq!(target.kind, SessionTargetKind::Fold);
        assert_eq!(target.fold_page_type, Some(page_type::AI));
        assert_eq!(target.type_name.as_deref(), Some("SESSION_TYPE_AI_FOLD"));
    }

    #[test]
    fn an_unmapped_fold_target_keeps_the_name_for_the_toast() {
        let id = pb::SessionId {
            id: Some(pb::session_id::Id::FoldId(pb::FoldId {
                r#type: Some(session_type::PRIVATE),
            })),
        };
        let target = target_from(Some(&id));
        assert_eq!(target.fold_page_type, None, "没有对应的二级页");
        assert_eq!(target.type_name.as_deref(), Some("SESSION_TYPE_PRIVATE"));
    }

    #[test]
    fn a_private_target_carries_the_talker_uid() {
        let target = target_from(Some(&private_session_id(100_002)));
        assert_eq!(target.kind, SessionTargetKind::Private);
        assert_eq!(target.talker_uid, Some(100_002));
    }

    #[test]
    fn pagination_offsets_round_trip() {
        let params = pb::PaginationParams {
            offsets: [(
                1,
                pb::Offset {
                    normal_offset: Some(42),
                    top_offset: Some(7),
                },
            )]
            .into_iter()
            .collect(),
            has_more: Some(true),
        };
        let offsets = offsets_from(Some(&params));
        assert_eq!(offsets.len(), 1);
        assert_eq!(offsets[0].key, 1);
        assert_eq!(offsets[0].normal_offset, 42);
        // 原样带回下一次请求。
        assert_eq!(
            pagination_from(&offsets).expect("有游标").offsets,
            params.offsets
        );
    }

    #[test]
    fn no_offsets_means_no_pagination_params_at_all() {
        // 首屏不能发一个空的 pagination_params——那与「从头开始」不是一回事。
        assert!(pagination_from(&[]).is_none());
    }

    #[test]
    fn send_msg_is_the_only_unsafe_grpc_write() {
        assert_eq!(
            Idempotency::Unsafe.retry_policy(),
            RetryPolicy::Write(Idempotency::Unsafe)
        );
        // 屏蔽词新增是可判定档：会多一条，但列表查得到。
        assert_eq!(
            Idempotency::Verifiable.retry_policy(),
            RetryPolicy::Write(Idempotency::Verifiable)
        );
    }

    /// 只发被改的那一项（#276：整张表发回去服务端回 -400），而这一项
    /// 本身仍是从原始字节解出来的，不是按投影重建的赝品。
    #[test]
    fn set_settings_sends_only_the_changed_entry() {
        let reply = pb::GetImSettingsReply {
            page_title: Some("私信设置".to_owned()),
            settings: [
                (
                    1,
                    pb::Setting {
                        content: Some(pb::setting::Content::Switch(pb::SettingSwitch {
                            switch_on: Some(false),
                            title: Some("接收陌生人私信".to_owned()),
                            subtitle: Some("这段说明我们没有投影".to_owned()),
                        })),
                    },
                ),
                (
                    2,
                    pb::Setting {
                        content: Some(pb::setting::Content::Text(pb::SettingText {
                            text: Some("这一项我们没有投影".to_owned()),
                        })),
                    },
                ),
            ]
            .into_iter()
            .collect(),
        };
        let raw = reply.encode_to_vec();

        let req = set_settings_request(&raw, 1, Some(true), None).expect("能构造");

        // 第二项不在请求里：基线只发 {key: item}。
        assert_eq!(req.settings.keys().copied().collect::<Vec<_>>(), [1]);
        let Some(pb::setting::Content::Switch(s)) =
            req.settings.get(&1).and_then(|s| s.content.as_ref())
        else {
            panic!("第一项应是开关");
        };
        assert_eq!(s.switch_on, Some(true));
        // 这一项里没投影的字段原样带回。
        assert_eq!(s.subtitle.as_deref(), Some("这段说明我们没有投影"));
        assert_eq!(s.title.as_deref(), Some("接收陌生人私信"));
    }

    #[test]
    fn set_settings_request_rejects_a_key_not_in_the_snapshot() {
        let raw = pb::GetImSettingsReply::default().encode_to_vec();
        let err = set_settings_request(&raw, 9, Some(true), None).expect_err("应报错");
        assert_eq!(err.kind(), ErrorKind::Protocol);
    }

    #[test]
    fn vip_info_is_a_json_string_not_a_submessage() {
        assert_eq!(
            vip_from_json(Some(r#"{"status":1,"type":2,"due_date":0}"#)),
            (Some(1), Some(2))
        );
    }

    #[test]
    fn a_broken_vip_info_is_not_an_error() {
        // 基线的 hasVipInfo() 为假与解析失败走的是同一条路：当作没有。
        assert_eq!(vip_from_json(None), (None, None));
        assert_eq!(vip_from_json(Some("")), (None, None));
        assert_eq!(vip_from_json(Some("not json")), (None, None));
        assert_eq!(vip_from_json(Some("[]")), (None, None), "数组没有 status");
    }

    #[test]
    fn settings_are_projected_by_their_oneof_branch() {
        let switch = pb::Setting {
            content: Some(pb::setting::Content::Switch(pb::SettingSwitch {
                switch_on: Some(true),
                title: Some("标题".to_owned()),
                subtitle: Some("副标题".to_owned()),
            })),
        };
        let projected = project_setting(7, &switch);
        assert_eq!(projected.key, 7);
        assert_eq!(projected.switch_on, Some(true));
        assert_eq!(projected.title.as_deref(), Some("标题"));

        let select = pb::Setting {
            content: Some(pb::setting::Content::Select(pb::SettingSelect {
                item: vec![
                    pb::SelectItem {
                        item_type: Some(10),
                        text: Some("全部".to_owned()),
                        selected: Some(false),
                    },
                    pb::SelectItem {
                        item_type: Some(20),
                        text: Some("仅关注".to_owned()),
                        selected: Some(true),
                    },
                ],
            })),
        };
        let projected = project_setting(8, &select);
        assert_eq!(projected.selected_index, Some(1));
        assert_eq!(projected.options.len(), 2);
        assert_eq!(projected.options[1].value, 20);
        assert_eq!(projected.switch_on, None, "选择型没有开关");
    }
}
