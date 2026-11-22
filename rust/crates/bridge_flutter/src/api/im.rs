//! 私信（IM）的 Dart 导出（S12 续单 / issue #63）。
//!
//! 覆盖基线 `lib/grpc/im.dart` 的 17 个方法。REST 那一半在
//! [`crate::api::msg`]。
//!
//! 上下文复用 [`GrpcContext`]——gRPC 的凭据与 metadata 全在那里，
//! 这一片没有任何额外的账号参数（ADR-012 决策六）。
//!
//! ## 导出面上刻意不出现的两样东西
//!
//! 1. **protobuf 生成类型**（ADR-012 决策五）。会话树在 Rust 里投影完才
//!    过来：头像三件套已经按图层位置取好、折叠入口的会话类型已经映射成
//!    二级页类型、`vip_info` 那段 JSON 已经解开。
//! 2. **`Int64`**。基线满屏的 `Int64` 来自 protobuf 的 Dart 实现；
//!    FRB 这边 i64 就是 Dart 的 int，调用点不必再 `.toInt()`。
//!
//! ## 设置页那段不透明字节
//!
//! [`ImSettingsSnapshot::raw`] 是整条 `GetImSettingsReply` 的原始字节。
//! 写回设置时它原样传回来（[`set_im_setting`]），Rust 重新解码、只改一处、
//! 再整张发出去。理由见 `bili_api::grpc_im::ImSettingsSnapshot` 的文档：
//! 按投影重建会发出一个**缺字段的赝品**，把用户没碰过的设置悄悄抹掉。
//!
//! Dart 侧只需把它当黑盒存着，不要解释它。

use bili_api::grpc_im::{self, OutgoingMessage};
use bili_domain::error::DomainError;
use bili_domain::im as domain;

use crate::api::grpc::GrpcContext;
use crate::api::init;
use crate::api::write::WriteResult;

// ──────────────────────────── 导出实体 ────────────────────────────

/// 会话指向哪里。与协议 `SessionId` 的 oneof 分支一一对应。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImSessionTargetKind {
    /// 私聊。点进去是聊天页。
    Private,
    /// 折叠入口。点进去是二级会话列表。
    Fold,
    /// 系统消息入口。
    System,
    /// 群聊。界面没有分支。
    Group,
    /// 客服。界面没有分支。
    Customer,
    Unknown,
}

/// 会话的指向，已摊平。按 [`Self::kind`] 分支取对应的那个 `Option`。
#[derive(Debug, Clone)]
pub struct ImSessionTarget {
    pub kind: ImSessionTargetKind,
    /// `Private` 时是对方的 mid。
    pub talker_uid: Option<i64>,
    /// `Fold` 时是**已经映射好**的二级页类型，直接拿去开页面。
    ///
    /// `None` 表示这个折叠类型没有对应的二级页——此时界面 toast
    /// [`Self::type_name`]，与基线一致。
    pub fold_page_type: Option<i32>,
    pub session_type: Option<i32>,
    /// 原始会话类型的枚举名，映射不出来时拿它提示。
    pub type_name: Option<String>,
}

/// 未读数的显示方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImUnreadStyle {
    None,
    Number,
    Dot,
}

/// 私信列表里的一条会话。
#[derive(Debug, Clone)]
pub struct ImSession {
    pub target: ImSessionTarget,
    pub name: String,
    /// 主头像。三种资源形态都试过了。
    pub avatar_url: Option<String>,
    /// 头像挂件。
    pub pendant_url: Option<String>,
    /// 认证标：0 个人、1 机构。已从协议的 `LocalRes` 映射过。
    pub official_type: Option<i32>,
    pub vip_status: Option<i64>,
    /// 2 是年度及以上，昵称才染色。
    pub vip_type: Option<i64>,
    pub label_text: Option<String>,
    pub is_live: bool,
    pub is_pinned: bool,
    pub is_muted: bool,
    /// **微秒**时间戳。显示前要 `~/ 1000000`，协议如此。
    pub timestamp_us: Option<i64>,
    pub summary: Option<String>,
    pub unread_style: ImUnreadStyle,
    pub unread_number: i64,
    /// 点头像进个人页用。
    pub mid: Option<i64>,
}

/// 翻页游标。原样带回下一次请求。
#[derive(Debug, Clone, Copy)]
pub struct ImSessionOffset {
    pub key: i32,
    pub normal_offset: i64,
    pub top_offset: i64,
}

/// 「…」菜单里那一项是什么。
///
/// 只列**界面有分支的**那几种，其余一律 [`Self::Other`]——基线的 switch
/// 也有 `_` 兜底，画一个圆点。导成枚举而不是 i32，是为了让「加了一种新
/// 类型」在 Dart 侧变成一个编译期的 switch 缺分支，而不是悄悄走进兜底。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImThreeDotKind {
    /// 一键已读。
    ReadAll,
    /// 清空列表。
    ClearList,
    /// 消息设置。
    MsgSetting,
    /// UP 主助手。
    UpHelper,
    /// 通讯录。
    Contacts,
    /// 粉丝群助手。
    FansGroupHelper,
    Other,
}

/// 会话列表顶部「…」菜单里的一项。
#[derive(Debug, Clone)]
pub struct ImThreeDotItem {
    pub title: Option<String>,
    pub icon: Option<String>,
    pub url: Option<String>,
    pub kind: ImThreeDotKind,
    pub has_red_dot: bool,
}

/// 界面要主动打开的那几个设置页。
///
/// 协议的 `ImSettingType` 有二十多个取值，界面只会主动开这四个（其余的
/// 由 [`ImSettingRedirect::parent_setting_type`] 带过来）。导成枚举而不是
/// 让 Dart 写死 0 / 17 / 18 / 19——那四个数字在协议里相邻且无规律。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImSettingPage {
    /// 全部私信设置。
    All,
    /// 「回复我的」的通知设置。
    ReplyMe,
    /// 「@我的」的通知设置。
    AtMe,
    /// 「收到的赞」的通知设置。
    ReceiveLike,
}

/// 把 [`ImSettingPage`] 换成协议的 `ImSettingType` 取值。
///
/// 取值在 [`bili_domain::im::setting_page_type`]，由 `bili_api::grpc_im`
/// 的一条测试钉在生成枚举上——这一层**看不见 protobuf**（ADR-012 决策
/// 五的保证就是靠 `bridge_flutter` 不依赖 `bili-grpc`）。
#[flutter_rust_bridge::frb(sync)]
#[must_use]
pub fn im_setting_type(page: ImSettingPage) -> i32 {
    use domain::setting_page_type as t;
    match page {
        ImSettingPage::All => t::ALL,
        ImSettingPage::ReplyMe => t::OLD_REPLY_ME,
        ImSettingPage::AtMe => t::OLD_AT_ME,
        ImSettingPage::ReceiveLike => t::OLD_RECEIVE_LIKE,
    }
}

/// 一页会话列表。
#[derive(Debug, Clone)]
pub struct ImSessionPage {
    pub sessions: Vec<ImSession>,
    pub offsets: Vec<ImSessionOffset>,
    pub has_more: bool,
    pub three_dot_items: Vec<ImThreeDotItem>,
    /// 单独摆在菜单外的那几项。只有主列表有，二级列表恒为空。
    pub outside_items: Vec<ImThreeDotItem>,
}

/// 一条私信。
///
/// `content` **原样是一段 JSON 字符串**，形状随 `msg_type` 变。不在
/// Rust 里拆：每一种的画法都是 Widget 的事（AGENTS.md §2 明确留在宿主
/// 的那一侧）。
#[derive(Debug, Clone)]
pub struct ImMessage {
    pub sender_uid: i64,
    pub receiver_id: i64,
    pub msg_type: i32,
    /// 按界面画法分的类型。原始 [`Self::msg_type`] 也留着——
    /// `Unsupported` 那一档要靠它说清是哪一种。
    pub kind: ImMsgKind,
    /// 基线用 8..=11 这一段判「是不是分享来的」。
    pub msg_source: i32,
    /// 1 表示已撤回。
    pub msg_status: i32,
    pub msg_key: i64,
    pub msg_seqno: i64,
    /// **秒**级时间戳，与 [`ImSession::timestamp_us`] 不是一个单位。
    pub timestamp: i64,
    pub content: String,
}

/// 一条私信是哪一种，按界面的画法分。
///
/// 协议的 `MsgType` 有三十来个取值，这里只列界面有分支的那些，其余
/// 一律 [`Self::Unsupported`]。导成枚举而不是透传 i32：协议加一种新
/// 消息时，Dart 那边会变成一个编译期的 switch 缺分支。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImMsgKind {
    Text,
    /// 图片与自定义表情共用一种画法。
    Picture,
    /// 已撤回。
    DrawBack,
    ShareV2,
    NotifyMsg,
    VideoCard,
    ArticleCard,
    PictureCard,
    CommonShareCard,
    /// 提示条（灰色小字）。
    TipMessage,
    /// 协议里没有名字的 16，基线按数字单独画一种卡片。
    Unnamed16,
    Unsupported,
}

/// 把正文里的 `[文字]` 换成图。
#[derive(Debug, Clone)]
pub struct ImEmote {
    pub text: String,
    pub url: String,
    pub size: i32,
    pub gif_url: Option<String>,
}

/// 一页私信。
#[derive(Debug, Clone)]
pub struct ImMessagePage {
    pub messages: Vec<ImMessage>,
    pub emotes: Vec<ImEmote>,
    pub has_more: bool,
}

/// 分享面板里的「最近聊过的人」。
#[derive(Debug, Clone)]
pub struct ImShareTarget {
    pub talker_id: i64,
    pub name: Option<String>,
    pub avatar: Option<String>,
}

/// 消息中心四个 Tab 的未读数（gRPC 版）。
///
/// 与 [`crate::api::msg::MsgFeedUnread`] 同义不同源：那条是 REST 的
/// `/x/msgfeed/unread`（首页红点用），这条是 gRPC（私信列表页用，顺手
/// 从同一个响应里拿，省一次往返）。
#[derive(Debug, Clone, Copy)]
pub struct ImTotalUnread {
    pub reply: i64,
    pub at: i64,
    pub like: i64,
    pub sys_msg: i64,
}

/// 设置页的一项。
#[derive(Debug, Clone)]
pub struct ImSetting {
    /// 协议里 settings 是 `map<int32, Setting>`，这是 key。写回时要用它。
    pub key: i32,
    pub title: Option<String>,
    pub subtitle: Option<String>,
    /// 开关型：当前是否打开。非开关型为 `None`。
    pub switch_on: Option<bool>,
    /// 选择型：当前选中项在 [`Self::options`] 里的下标。
    pub selected_index: Option<i32>,
    pub options: Vec<ImSettingOption>,
    /// 跳转型的去向。其余形态为 `None`。
    pub redirect: Option<ImSettingRedirect>,
}

/// 跳转型设置的去向，已摊平。按哪个字段有值分支，顺序与基线一致：
/// 子设置页 → 弹窗选择 → 外部页 → 设置页 url。
///
/// **不带子设置树。** 基线读它只为算一行右侧文字，那段规则在 Rust 侧
/// 算完了，结果是 [`Self::trailing_text`]。
#[derive(Debug, Clone)]
pub struct ImSettingRedirect {
    /// 子设置页的类型，拿它开下一页。
    pub parent_setting_type: Option<i32>,
    pub page_url: Option<String>,
    pub other_url: Option<String>,
    /// 弹窗选择的可选项。非该形态时为空。
    pub window_options: Vec<ImSettingOption>,
    pub window_selected_index: Option<i32>,
    pub selected_summary: Option<String>,
    /// 这一行右侧显示什么。已按基线的三级回退算好。
    pub trailing_text: Option<String>,
}

/// 选择型设置的一个可选项。
#[derive(Debug, Clone)]
pub struct ImSettingOption {
    pub title: Option<String>,
    /// 写回时传这个值，不是下标。
    pub value: i32,
}

/// 一次设置读取的结果。
#[derive(Debug, Clone)]
pub struct ImSettingsSnapshot {
    pub page_title: String,
    pub settings: Vec<ImSetting>,
    /// **不透明字节**，原样传回 [`set_im_setting`]。见模块文档。
    pub raw: Vec<u8>,
}

/// 关键词屏蔽列表。
#[derive(Debug, Clone)]
pub struct ImKeywordBlocking {
    pub keywords: Vec<String>,
    /// 最多能加几条，界面拿它拦住「再加一条」。
    pub list_limit: Option<i32>,
    pub char_limit: Option<i32>,
}

// ─────────────────────────── 结果信封 ───────────────────────────

macro_rules! result_type {
    ($name:ident, $data:ty) => {
        #[derive(Debug, Clone)]
        pub struct $name {
            pub data: Option<$data>,
            pub error_kind: Option<String>,
            pub error_detail: Option<String>,
            pub error_code: Option<i64>,
        }

        impl $name {
            fn from_result(result: Result<$data, DomainError>) -> Self {
                match result {
                    Ok(data) => Self {
                        data: Some(data),
                        error_kind: None,
                        error_detail: None,
                        error_code: None,
                    },
                    Err(error) => Self {
                        data: None,
                        error_kind: Some(error.kind().to_string()),
                        error_detail: Some(error.detail().to_owned()),
                        error_code: error.code(),
                    },
                }
            }
        }
    };
}

result_type!(ImSessionPageResult, ImSessionPage);

/// 单个会话的结果。**不用 `result_type!`**：`data` 为空在这里不表示失败，
/// 而是「服务端查不到这个会话」。宏生成的那个 `from_result` 会把两者混为
/// 一谈。
#[derive(Debug, Clone)]
pub struct ImSessionResult {
    pub data: Option<ImSession>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

result_type!(ImMessagePageResult, ImMessagePage);
result_type!(ImShareListResult, Vec<ImShareTarget>);
result_type!(ImTotalUnreadResult, ImTotalUnread);
result_type!(ImAckSeqnoResult, i64);
result_type!(ImSettingsResult, ImSettingsSnapshot);
result_type!(ImKeywordBlockingResult, ImKeywordBlocking);

// ──────────────────────────── 领域 → 导出 ────────────────────────────

fn target_from(t: domain::SessionTarget) -> ImSessionTarget {
    ImSessionTarget {
        kind: match t.kind {
            domain::SessionTargetKind::Private => ImSessionTargetKind::Private,
            domain::SessionTargetKind::Fold => ImSessionTargetKind::Fold,
            domain::SessionTargetKind::System => ImSessionTargetKind::System,
            domain::SessionTargetKind::Group => ImSessionTargetKind::Group,
            domain::SessionTargetKind::Customer => ImSessionTargetKind::Customer,
            domain::SessionTargetKind::Unknown => ImSessionTargetKind::Unknown,
        },
        talker_uid: t.talker_uid,
        fold_page_type: t.fold_page_type,
        session_type: t.session_type,
        type_name: t.type_name,
    }
}

fn session_from(s: domain::ImSession) -> ImSession {
    ImSession {
        target: target_from(s.target),
        name: s.name,
        avatar_url: s.avatar_url,
        pendant_url: s.pendant_url,
        official_type: s.official_type,
        vip_status: s.vip_status,
        vip_type: s.vip_type,
        label_text: s.label_text,
        is_live: s.is_live,
        is_pinned: s.is_pinned,
        is_muted: s.is_muted,
        timestamp_us: s.timestamp_us,
        summary: s.summary,
        unread_style: match s.unread_style {
            domain::UnreadStyle::None => ImUnreadStyle::None,
            domain::UnreadStyle::Number => ImUnreadStyle::Number,
            domain::UnreadStyle::Dot => ImUnreadStyle::Dot,
        },
        unread_number: s.unread_number,
        mid: s.mid,
    }
}

fn three_dots_from(items: Vec<domain::ThreeDotItem>) -> Vec<ImThreeDotItem> {
    items
        .into_iter()
        .map(|i| ImThreeDotItem {
            title: i.title,
            icon: i.icon,
            url: i.url,
            kind: match i.kind {
                domain::ThreeDotKind::ReadAll => ImThreeDotKind::ReadAll,
                domain::ThreeDotKind::ClearList => ImThreeDotKind::ClearList,
                domain::ThreeDotKind::MsgSetting => ImThreeDotKind::MsgSetting,
                domain::ThreeDotKind::UpHelper => ImThreeDotKind::UpHelper,
                domain::ThreeDotKind::Contacts => ImThreeDotKind::Contacts,
                domain::ThreeDotKind::FansGroupHelper => ImThreeDotKind::FansGroupHelper,
                domain::ThreeDotKind::Other => ImThreeDotKind::Other,
            },
            has_red_dot: i.has_red_dot,
        })
        .collect()
}

fn page_from(p: domain::ImSessionPage) -> ImSessionPage {
    ImSessionPage {
        sessions: p.sessions.into_iter().map(session_from).collect(),
        offsets: p
            .offsets
            .into_iter()
            .map(|o| ImSessionOffset {
                key: o.key,
                normal_offset: o.normal_offset,
                top_offset: o.top_offset,
            })
            .collect(),
        has_more: p.has_more,
        three_dot_items: three_dots_from(p.three_dot_items),
        outside_items: three_dots_from(p.outside_items),
    }
}

fn offsets_into(offsets: Vec<ImSessionOffset>) -> Vec<domain::SessionOffset> {
    offsets
        .into_iter()
        .map(|o| domain::SessionOffset {
            key: o.key,
            normal_offset: o.normal_offset,
            top_offset: o.top_offset,
        })
        .collect()
}

fn message_page_from(p: domain::ImMessagePage) -> ImMessagePage {
    ImMessagePage {
        messages: p
            .messages
            .into_iter()
            .map(|m| ImMessage {
                sender_uid: m.sender_uid,
                receiver_id: m.receiver_id,
                msg_type: m.msg_type,
                kind: match m.kind {
                    domain::MsgKind::Text => ImMsgKind::Text,
                    domain::MsgKind::Picture => ImMsgKind::Picture,
                    domain::MsgKind::DrawBack => ImMsgKind::DrawBack,
                    domain::MsgKind::ShareV2 => ImMsgKind::ShareV2,
                    domain::MsgKind::NotifyMsg => ImMsgKind::NotifyMsg,
                    domain::MsgKind::VideoCard => ImMsgKind::VideoCard,
                    domain::MsgKind::ArticleCard => ImMsgKind::ArticleCard,
                    domain::MsgKind::PictureCard => ImMsgKind::PictureCard,
                    domain::MsgKind::CommonShareCard => ImMsgKind::CommonShareCard,
                    domain::MsgKind::TipMessage => ImMsgKind::TipMessage,
                    domain::MsgKind::Unnamed16 => ImMsgKind::Unnamed16,
                    domain::MsgKind::Unsupported => ImMsgKind::Unsupported,
                },
                msg_source: m.msg_source,
                msg_status: m.msg_status,
                msg_key: m.msg_key,
                msg_seqno: m.msg_seqno,
                timestamp: m.timestamp,
                content: m.content,
            })
            .collect(),
        emotes: p
            .emotes
            .into_iter()
            .map(|e| ImEmote {
                text: e.text,
                url: e.url,
                size: e.size,
                gif_url: e.gif_url,
            })
            .collect(),
        has_more: p.has_more,
    }
}

fn options_from(options: Vec<domain::ImSettingOption>) -> Vec<ImSettingOption> {
    options
        .into_iter()
        .map(|o| ImSettingOption {
            title: o.title,
            value: o.value,
        })
        .collect()
}

fn settings_from(s: grpc_im::ImSettingsSnapshot) -> ImSettingsSnapshot {
    ImSettingsSnapshot {
        page_title: s.page.page_title,
        settings: s
            .page
            .settings
            .into_iter()
            .map(|item| ImSetting {
                key: item.key,
                title: item.title,
                subtitle: item.subtitle,
                switch_on: item.switch_on,
                selected_index: item.selected_index,
                options: options_from(item.options),
                redirect: item.redirect.map(|r| ImSettingRedirect {
                    parent_setting_type: r.parent_setting_type,
                    page_url: r.page_url,
                    other_url: r.other_url,
                    window_options: options_from(r.window_options),
                    window_selected_index: r.window_selected_index,
                    selected_summary: r.selected_summary,
                    trailing_text: r.trailing_text,
                }),
            })
            .collect(),
        raw: s.raw,
    }
}

/// 把 [`ImMsgKind`] 换成协议的 `MsgType` 取值，发消息时要用。
///
/// 只覆盖**界面会主动发**的那几种；`Unsupported` 之类没有对应取值，
/// 返回 `None`。
#[flutter_rust_bridge::frb(sync)]
#[must_use]
pub fn im_msg_type(kind: ImMsgKind) -> Option<i32> {
    use domain::msg_type as t;
    Some(match kind {
        ImMsgKind::Text => t::TEXT,
        ImMsgKind::Picture => t::PIC,
        ImMsgKind::DrawBack => t::DRAW_BACK,
        ImMsgKind::ShareV2 => t::SHARE_V2,
        ImMsgKind::CommonShareCard => t::COMMON_SHARE_CARD,
        _ => return None,
    })
}

/// 主会话列表那一页的页面类型。
///
/// 导一个函数而不是让 Dart 写死 `1`：这个数字来自协议的
/// `SessionPageType`，而同名的 `SessionType` 枚举里 1 是「私聊」——两张表
/// 的数字几乎每行都不同（见 `bili_domain::im::fold_page_type`）。界面需要
/// 的其余页面类型都由 [`ImSessionTarget::fold_page_type`] 带过来，已经
/// 映射好，所以只有这一个要单独给。
#[flutter_rust_bridge::frb(sync)]
#[must_use]
pub fn im_home_page_type() -> i32 {
    domain::page_type::HOME
}

// ──────────────────────────── 会话列表 ────────────────────────────

/// 取主会话列表。首屏传空的 `offsets`。
pub async fn fetch_im_session_main(
    context: GrpcContext,
    offsets: Vec<ImSessionOffset>,
) -> ImSessionPageResult {
    let (ctx, transport) = context.split();
    let offsets = offsets_into(offsets);
    ImSessionPageResult::from_result(
        flatten(
            init::runtime()
                .spawn(async move { grpc_im::session_main(&transport, &ctx, &offsets).await })
                .await,
        )
        .map(page_from),
    )
}

/// 取二级会话列表。`page_type` 来自
/// [`ImSessionTarget::fold_page_type`]——那是 Rust 映射好的，别拿会话
/// 类型直接传。
pub async fn fetch_im_session_secondary(
    context: GrpcContext,
    page_type: i32,
    offsets: Vec<ImSessionOffset>,
) -> ImSessionPageResult {
    let (ctx, transport) = context.split();
    let offsets = offsets_into(offsets);
    ImSessionPageResult::from_result(
        flatten(
            init::runtime()
                .spawn(async move {
                    grpc_im::session_secondary(&transport, &ctx, page_type, &offsets).await
                })
                .await,
        )
        .map(page_from),
    )
}

/// 取单个会话的最新状态。界面用它查「现在是不是置顶」。
///
/// 服务端没给这个会话时 `data` 为空且没有错误——那是「查不到」，不是
/// 失败。
pub async fn fetch_im_session(context: GrpcContext, talker_uid: i64) -> ImSessionResult {
    let (ctx, transport) = context.split();
    match flatten(
        init::runtime()
            .spawn(async move { grpc_im::session_update(&transport, &ctx, talker_uid).await })
            .await,
    ) {
        Ok(session) => ImSessionResult {
            data: session.map(session_from),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => ImSessionResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

// ────────────────────────── 会话上的写操作 ──────────────────────────

/// 置顶会话。置为语义，重发安全。
pub async fn pin_im_session(context: GrpcContext, talker_uid: i64) -> WriteResult {
    let (ctx, transport) = context.split();
    WriteResult::from_result(flatten(
        init::runtime()
            .spawn(async move { grpc_im::pin_session(&transport, &ctx, talker_uid).await })
            .await,
    ))
}

/// 取消置顶。
pub async fn unpin_im_session(context: GrpcContext, talker_uid: i64) -> WriteResult {
    let (ctx, transport) = context.split();
    WriteResult::from_result(flatten(
        init::runtime()
            .spawn(async move { grpc_im::unpin_session(&transport, &ctx, talker_uid).await })
            .await,
    ))
}

/// 把某一页的未读全部清掉。
pub async fn clear_im_unread(context: GrpcContext, page_type: i32) -> WriteResult {
    let (ctx, transport) = context.split();
    WriteResult::from_result(flatten(
        init::runtime()
            .spawn(async move { grpc_im::clear_unread(&transport, &ctx, page_type).await })
            .await,
    ))
}

/// 删掉某一页的整个会话列表。
pub async fn delete_im_session_list(context: GrpcContext, page_type: i32) -> WriteResult {
    let (ctx, transport) = context.split();
    WriteResult::from_result(flatten(
        init::runtime()
            .spawn(async move { grpc_im::delete_session_list(&transport, &ctx, page_type).await })
            .await,
    ))
}

// ──────────────────────────── 消息 ────────────────────────────

/// 拉一页会话消息。`end_seqno` 是上一页最老那条的 seqno；首屏传 `None`。
pub async fn fetch_im_messages(
    context: GrpcContext,
    talker_id: i64,
    end_seqno: Option<i64>,
) -> ImMessagePageResult {
    let (ctx, transport) = context.split();
    ImMessagePageResult::from_result(
        flatten(
            init::runtime()
                .spawn(async move {
                    grpc_im::fetch_session_msgs(&transport, &ctx, talker_id, end_seqno).await
                })
                .await,
        )
        .map(message_page_from),
    )
}

/// 发一条私信。**传输层一次都不重发。**
///
/// `dev_id` 服务端拿来去重，**必须在一次逻辑发送内固定**：重试时沿用同
/// 一个，用户重新点发送时换一个。由宿主给而不是 Rust 现生成，就是为了
/// 让这个区别留在看得见的地方（基线每次调用现生成一个 UUID，等于默认
/// 「每次都是新的一条」）。
///
/// `content` 是已序列化的 JSON 串，形状随 `msg_type` 变，由调用方编码。
pub async fn send_im_message(
    context: GrpcContext,
    sender_uid: i64,
    receiver_id: i64,
    msg_type: i32,
    content: String,
    timestamp: i64,
    dev_id: String,
) -> WriteResult {
    let (ctx, transport) = context.split();
    WriteResult::from_result(flatten(
        init::runtime()
            .spawn(async move {
                grpc_im::send_msg(
                    &transport,
                    &ctx,
                    &OutgoingMessage {
                        sender_uid,
                        receiver_id,
                        msg_type,
                        content: &content,
                        timestamp,
                        dev_id: &dev_id,
                    },
                )
                .await
            })
            .await,
    ))
}

/// 取会话的已读位点（`ack_seqno`）。「标为已读」要拿它当参数。
pub async fn fetch_im_ack_seqno(context: GrpcContext, talker_id: i64) -> ImAckSeqnoResult {
    let (ctx, transport) = context.split();
    ImAckSeqnoResult::from_result(flatten(
        init::runtime()
            .spawn(async move { grpc_im::session_detail(&transport, &ctx, talker_id).await })
            .await,
    ))
}

/// 取「最近聊过的人」，分享面板用。
pub async fn fetch_im_share_list(context: GrpcContext, size: i32) -> ImShareListResult {
    let (ctx, transport) = context.split();
    ImShareListResult::from_result(
        flatten(
            init::runtime()
                .spawn(async move { grpc_im::share_list(&transport, &ctx, size).await })
                .await,
        )
        .map(|list| {
            list.into_iter()
                .map(|t| ImShareTarget {
                    talker_id: t.talker_id,
                    name: t.name,
                    avatar: t.avatar,
                })
                .collect()
        }),
    )
}

/// 取消息中心四个 Tab 的未读数（gRPC 版）。
pub async fn fetch_im_total_unread(context: GrpcContext, unread_type: i32) -> ImTotalUnreadResult {
    let (ctx, transport) = context.split();
    ImTotalUnreadResult::from_result(
        flatten(
            init::runtime()
                .spawn(async move { grpc_im::total_unread(&transport, &ctx, unread_type).await })
                .await,
        )
        .map(|u| ImTotalUnread {
            reply: u.reply,
            at: u.at,
            like: u.like,
            sys_msg: u.sys_msg,
        }),
    )
}

// ──────────────────────────── 设置与屏蔽词 ────────────────────────────

/// 取私信设置页。返回里带着一段不透明字节，写回时要原样传回来。
pub async fn fetch_im_settings(context: GrpcContext, setting_type: i32) -> ImSettingsResult {
    let (ctx, transport) = context.split();
    ImSettingsResult::from_result(
        flatten(
            init::runtime()
                .spawn(async move { grpc_im::get_settings(&transport, &ctx, setting_type).await })
                .await,
        )
        .map(settings_from),
    )
}

/// 改一项设置并写回整张表。
///
/// `raw` 是 [`fetch_im_settings`] 带回来的那段字节，原样传。
/// `switch_on` 与 `selected_value` 二选一：前者改开关，后者改选择项
/// （传的是 [`ImSettingOption::value`]，**不是下标**）。
///
/// 为什么要把字节传回来而不是让 Rust 存着：没有「当前打开的是哪一页
/// 设置」这个状态，存就要有生命周期、有失效、有并发。字节走一圈是**无
/// 状态**的，而且传错了会在 Rust 侧以协议错误明确失败（`key` 不在表里）。
pub async fn set_im_setting(
    context: GrpcContext,
    raw: Vec<u8>,
    key: i32,
    switch_on: Option<bool>,
    selected_value: Option<i32>,
) -> WriteResult {
    let (ctx, transport) = context.split();
    WriteResult::from_result(flatten(
        init::runtime()
            .spawn(async move {
                grpc_im::set_settings(&transport, &ctx, &raw, key, switch_on, selected_value).await
            })
            .await,
    ))
}

/// 取关键词屏蔽列表。
pub async fn fetch_im_keyword_blocking(context: GrpcContext) -> ImKeywordBlockingResult {
    let (ctx, transport) = context.split();
    ImKeywordBlockingResult::from_result(
        flatten(
            init::runtime()
                .spawn(async move { grpc_im::keyword_blocking_list(&transport, &ctx).await })
                .await,
        )
        .map(|l| ImKeywordBlocking {
            keywords: l.keywords,
            list_limit: l.list_limit,
            char_limit: l.char_limit,
        }),
    )
}

/// 加一个屏蔽词。
///
/// 定级「可判定」：重发会多一条，但列表查得到，所以传输层不自动重试
/// （ADR-013 决策一）。失败了由用户再按一次。
pub async fn add_im_keyword_blocking(context: GrpcContext, keyword: String) -> WriteResult {
    let (ctx, transport) = context.split();
    WriteResult::from_result(flatten(
        init::runtime()
            .spawn(async move { grpc_im::keyword_blocking_add(&transport, &ctx, &keyword).await })
            .await,
    ))
}

/// 删一个屏蔽词。
pub async fn delete_im_keyword_blocking(context: GrpcContext, keyword: String) -> WriteResult {
    let (ctx, transport) = context.split();
    WriteResult::from_result(flatten(
        init::runtime()
            .spawn(
                async move { grpc_im::keyword_blocking_delete(&transport, &ctx, &keyword).await },
            )
            .await,
    ))
}

/// 把 `spawn` 的双层 Result 压平。与其余桥接模块同一份实现。
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
