//! 私信（IM）的领域实体与投影规则（S12 续单 / issue #63）。
//!
//! 对应基线 `lib/grpc/im.dart` 的 17 个方法。REST 那一半（消息中心、
//! 未读数、会话设置）在 [`crate::msg`]。
//!
//! ## 这个模块里最值钱的东西不是实体，是三段「投影规则」
//!
//! protobuf 那棵树与界面要画的东西差得很远，中间隔着三段**协议没规定、
//! 但两端必须一致**的决定。基线把它们写在
//! `lib/pages/whisper/widgets/item.dart` 里，换掉 Flutter 这层就要原样
//! 重写一遍，所以按 AGENTS.md §2 下沉到这里：
//!
//! 1. **头像三件套从哪一层取**（[`avatar_from_layers`]）。服务端给的是
//!    一组可渲染的图层，界面只认位置：第 0 层是主头像、第 1 层是挂件、
//!    最后一层是认证标。
//! 2. **认证标的枚举值怎么映射**（`LOCAL_RES_ICON_PERSONAL_VERIFY` = 3
//!    → 个人认证 0，`..._ENTERPRISE_VERIFY` = 4 → 机构认证 1）。
//! 3. **折叠入口的会话类型怎么映射成二级页类型**
//!    （[`fold_page_type`]）。这张表基线写成一个 `switch`，映射不出来的
//!    那几种会 toast 出枚举名。
//!
//! ## 为什么会话的指向做成「平铺 + 枚举」而不是带载荷的枚举
//!
//! 协议里 `SessionId` 是个 oneof（私聊 / 群 / 折叠 / 系统 / 客服）。
//! 导出面保持语言中立（AGENTS.md §2 的推论），所以这里摊成
//! [`SessionTarget`]：一个简单枚举加几个 `Option`。调用方按 `kind` 分支，
//! 取对应那个 `Option`——与基线的 `hasPrivateId()` / `hasFoldId()` 一一对应。

use serde::Serialize;

/// 一个会话指向哪里。
///
/// 与协议 `SessionId` 的 oneof 分支一一对应。`Unknown` 是「服务端给了
/// 一个我们不认识的分支」，不是「没有 id」——后者在解析时就被丢掉了。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub enum SessionTargetKind {
    /// 私聊。点进去是聊天页。
    Private,
    /// 折叠入口（陌生人、未关注、垃圾桶……）。点进去是二级会话列表。
    Fold,
    /// 系统消息入口。点进去是系统通知页。
    System,
    /// 群聊。界面**没有分支**——基线点它什么也不做。
    Group,
    /// 客服。同样没有分支。
    Customer,
    #[default]
    Unknown,
}

/// 会话的指向，已经摊平。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct SessionTarget {
    pub kind: SessionTargetKind,
    /// [`SessionTargetKind::Private`] 时是对方的 mid。
    pub talker_uid: Option<i64>,
    /// [`SessionTargetKind::Fold`] 时是**已经映射好**的二级页类型。
    ///
    /// `None` 表示这个折叠类型我们没有对应的二级页——基线此时 toast 出
    /// [`Self::type_name`]。
    pub fold_page_type: Option<i32>,
    /// `Fold`/`System` 的原始会话类型。
    pub session_type: Option<i32>,
    /// 原始会话类型的枚举名，映射不出来时界面拿它提示。
    ///
    /// 名字在 Rust 这边取，不在界面——界面没有枚举表，自己拼只会拼出
    /// 一个数字。
    pub type_name: Option<String>,
}

/// 未读数的显示方式。与协议 `UnreadStyle` 一致。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub enum UnreadStyle {
    /// 不显示。
    #[default]
    None,
    /// 显示具体数字。
    Number,
    /// 只显示一个小红点。
    Dot,
}

/// 私信列表里的一条会话。
///
/// 字段全是**界面真的画的那些**。协议里的快捷入口、筛选器、自动回复
/// 弹窗、每条会话的操作菜单配置都没有消费者，在 proto 生成那一步就裁掉了
/// （见 `tool/proto-generator` 的 `KEEP`）。
#[derive(Debug, Clone, Default, Serialize)]
pub struct ImSession {
    pub target: SessionTarget,
    pub name: String,
    /// 主头像。取自图层组的第 0 层，三种资源形态都试过了。
    pub avatar_url: Option<String>,
    /// 头像挂件。取自第 1 层。
    pub pendant_url: Option<String>,
    /// 认证标：0 个人、1 机构。取自最后一层，已从 `LocalRes` 映射过。
    pub official_type: Option<i32>,
    /// 会员状态。协议里 `vip_info` 是**一段 JSON 字符串**而不是子消息，
    /// 解析在 `bili_api::grpc_im`——那是数据层的事，领域层不碰 JSON。
    pub vip_status: Option<i64>,
    /// 会员类型。2 是年度及以上，昵称才染色。
    pub vip_type: Option<i64>,
    /// 昵称后面那个描边小标签的文字。
    pub label_text: Option<String>,
    pub is_live: bool,
    pub is_pinned: bool,
    pub is_muted: bool,
    /// **微秒**时间戳，不是毫秒也不是秒。基线显示前要 `~/ 1000000`。
    pub timestamp_us: Option<i64>,
    /// 最后一条消息的摘要，界面直接显示。
    pub summary: Option<String>,
    pub unread_style: UnreadStyle,
    pub unread_number: i64,
    /// 头像对应的 mid，点头像进个人页。与 [`SessionTarget::talker_uid`]
    /// 多数时候相同，但折叠入口没有 talker_uid 却可能有它。
    pub mid: Option<i64>,
}

/// 翻页游标。原样带回下一次请求。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct SessionOffset {
    /// 协议里这是一张 `map<int32, Offset>`，key 是会话分组。
    pub key: i32,
    pub normal_offset: i64,
    pub top_offset: i64,
}

/// 「…」菜单里那一项是什么。
///
/// 只列**界面有分支的**那几种，其余一律 [`Self::Other`]——基线的 switch
/// 也有 `_` 兜底，画一个圆点。做成枚举而不是透传 i32，是为了让「协议加
/// 了一种新类型」在界面那边变成一个编译期的 switch 缺分支。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub enum ThreeDotKind {
    ReadAll,
    ClearList,
    MsgSetting,
    UpHelper,
    Contacts,
    FansGroupHelper,
    #[default]
    Other,
}

/// 会话列表顶部那个「…」菜单里的一项。
#[derive(Debug, Clone, Default, Serialize)]
pub struct ThreeDotItem {
    pub title: Option<String>,
    pub icon: Option<String>,
    pub url: Option<String>,
    pub kind: ThreeDotKind,
    pub has_red_dot: bool,
}

/// 一页会话列表。
#[derive(Debug, Clone, Default, Serialize)]
pub struct ImSessionPage {
    pub sessions: Vec<ImSession>,
    /// 下一页的游标。空表示服务端没给——与 `has_more == false` 不是一回事，
    /// 所以两个都带回去。
    pub offsets: Vec<SessionOffset>,
    pub has_more: bool,
    pub three_dot_items: Vec<ThreeDotItem>,
    /// 「…」菜单之外单独摆出来的那几项。只有主列表有。
    pub outside_items: Vec<ThreeDotItem>,
}

/// 一条私信。
///
/// `content` **原样是一段 JSON 字符串**，不在这里解：它的形状随 `msg_type`
/// 变（文字、图片、分享卡、撤回、系统提示……），而每一种的画法都是
/// Widget 的事。把它拆开等于把十几种卡片的布局搬进 Rust，那是 AGENTS.md
/// §2 明确留在宿主的一侧。
#[derive(Debug, Clone, Default, Serialize)]
pub struct ImMessage {
    pub sender_uid: i64,
    pub receiver_id: i64,
    pub msg_type: i32,
    /// 按界面画法分的类型。原始 `msg_type` 也留着——基线在调试文案里
    /// 显示它，而且 `Unsupported` 那一档需要它才能说清是哪一种。
    pub kind: MsgKind,
    /// 消息来源。基线用 8..=11 这一段判「是不是分享来的」。
    pub msg_source: i32,
    /// 1 表示已撤回。
    pub msg_status: i32,
    pub msg_key: i64,
    pub msg_seqno: i64,
    /// **秒**级时间戳。与 [`ImSession::timestamp_us`] 不是一个单位，
    /// 协议如此。
    pub timestamp: i64,
    pub content: String,
}

/// 一条私信是哪一种，按界面的画法分。
///
/// 协议的 `MsgType` 有三十来个取值，界面只对下面这些有分支（基线
/// `lib/pages/whisper_detail/widget/chat_item.dart` 的那个 switch），
/// 其余一律 [`Self::Unsupported`]——那一档画「暂不支持的消息类型」。
///
/// 做成枚举而不是透传 i32：协议加一种新消息时，Dart 那边会变成一个
/// 编译期的 switch 缺分支，而不是悄悄掉进兜底。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub enum MsgKind {
    /// 纯文字。
    Text,
    /// 图片，以及自定义表情（两者共用同一种画法）。
    Picture,
    /// 已撤回。列表里会被整条滤掉。
    DrawBack,
    /// 分享卡（v2）。
    ShareV2,
    /// 系统通知样式的消息。
    NotifyMsg,
    /// 视频卡。
    VideoCard,
    /// 专栏卡。
    ArticleCard,
    /// 图片卡。
    PictureCard,
    /// 通用分享卡。
    CommonShareCard,
    /// 提示条（灰色小字）。
    TipMessage,
    /// 协议里**没有名字**的 16。基线按数字单独画一种卡片，照搬。
    Unnamed16,
    #[default]
    Unsupported,
}

/// 协议 `MsgType` 里界面有分支的那些取值。见 [`session_type`] 的警告。
pub mod msg_type {
    pub const TEXT: i32 = 1;
    pub const PIC: i32 = 2;
    pub const DRAW_BACK: i32 = 5;
    pub const CUSTOM_FACE: i32 = 6;
    pub const SHARE_V2: i32 = 7;
    pub const NOTIFY_MSG: i32 = 10;
    pub const VIDEO_CARD: i32 = 11;
    pub const ARTICLE_CARD: i32 = 12;
    pub const PICTURE_CARD: i32 = 13;
    pub const COMMON_SHARE_CARD: i32 = 14;
    pub const TIP_MESSAGE: i32 = 18;
    /// 协议里没有这个名字，基线按数字处理。
    pub const UNNAMED_16: i32 = 16;
}

/// 把协议的 `MsgType` 取值映射成界面的画法。
#[must_use]
pub fn msg_kind(value: i32) -> MsgKind {
    match value {
        msg_type::TEXT => MsgKind::Text,
        // 图片与自定义表情在基线里走同一个分支。
        msg_type::PIC | msg_type::CUSTOM_FACE => MsgKind::Picture,
        msg_type::DRAW_BACK => MsgKind::DrawBack,
        msg_type::SHARE_V2 => MsgKind::ShareV2,
        msg_type::NOTIFY_MSG => MsgKind::NotifyMsg,
        msg_type::VIDEO_CARD => MsgKind::VideoCard,
        msg_type::ARTICLE_CARD => MsgKind::ArticleCard,
        msg_type::PICTURE_CARD => MsgKind::PictureCard,
        msg_type::COMMON_SHARE_CARD => MsgKind::CommonShareCard,
        msg_type::TIP_MESSAGE => MsgKind::TipMessage,
        msg_type::UNNAMED_16 => MsgKind::Unnamed16,
        _ => MsgKind::Unsupported,
    }
}

/// 表情包里的一条：把正文里的 `[文字]` 换成图。
#[derive(Debug, Clone, Default, Serialize)]
pub struct ImEmote {
    pub text: String,
    pub url: String,
    pub size: i32,
    pub gif_url: Option<String>,
}

/// 一页私信。
#[derive(Debug, Clone, Default, Serialize)]
pub struct ImMessagePage {
    pub messages: Vec<ImMessage>,
    pub emotes: Vec<ImEmote>,
    /// 协议里是 int32，0 表示没有更多。做成 bool 在这里就判完。
    pub has_more: bool,
}

/// 分享面板里的「最近聊过的人」。
#[derive(Debug, Clone, Default, Serialize)]
pub struct ShareTarget {
    pub talker_id: i64,
    pub name: Option<String>,
    pub avatar: Option<String>,
}

/// 私信设置页的一页。
///
/// `settings` 的 value 是协议的 `Setting`（开关 / 选择 / 跳转三种形态），
/// 这里保留 key 与那三种形态摊平后的内容。
#[derive(Debug, Clone, Default, Serialize)]
pub struct ImSettingsPage {
    pub page_title: String,
    pub settings: Vec<ImSetting>,
}

/// 设置页的一项。
#[derive(Debug, Clone, Default, Serialize)]
pub struct ImSetting {
    /// 协议里 settings 是 `map<int32, Setting>`，这是它的 key。
    pub key: i32,
    pub title: Option<String>,
    pub subtitle: Option<String>,
    /// 开关型：当前是否打开。非开关型为 `None`。
    pub switch_on: Option<bool>,
    /// 选择型：当前选中项的下标与可选项。
    pub selected_index: Option<i32>,
    pub options: Vec<ImSettingOption>,
    /// 跳转型的去向。其余形态为 `None`。
    pub redirect: Option<ImSettingRedirect>,
}

/// 跳转型设置的去向。
///
/// 协议里它是个 oneof（子设置页 / 外部页 / 弹窗 / 弹窗选择），这里摊平成
/// 几个 `Option`，按哪个有值分支——与基线的 `hasWindowSelect()` /
/// `otherPage.hasUrl()` 一一对应，顺序也一样。
///
/// **没有把子设置树带过来。** 基线读 `sub_settings` 只为算一行 trailing
/// 文字（第一个选中的选项，否则第一个打开着的开关的标题），那段计算在
/// 投影时就做完了，结果是 [`Self::trailing_text`]。带整棵树过来要么让
/// 导出面递归，要么让界面重新实现那段「算 trailing」的规则——后者正是
/// AGENTS.md §2 要避免的。
#[derive(Debug, Clone, Default, Serialize)]
pub struct ImSettingRedirect {
    /// 子设置页的 `ImSettingType`，拿它开下一页。
    pub parent_setting_type: Option<i32>,
    /// 子设置页自己的 url。
    pub page_url: Option<String>,
    /// 跳外部页面的 url。
    pub other_url: Option<String>,
    /// 弹窗选择的可选项。非该形态时为空。
    pub window_options: Vec<ImSettingOption>,
    pub window_selected_index: Option<i32>,
    /// 服务端给的「当前选的是什么」。
    pub selected_summary: Option<String>,
    /// 这一行右侧显示什么。已按基线的三级回退算好：
    /// 子设置里第一个选中的选项 → 第一个打开着的开关的标题 →
    /// `selected_summary`。
    pub trailing_text: Option<String>,
}

/// 选择型设置的一个可选项。
#[derive(Debug, Clone, Default, Serialize)]
pub struct ImSettingOption {
    pub title: Option<String>,
    pub value: i32,
}

/// 关键词屏蔽列表。
#[derive(Debug, Clone, Default, Serialize)]
pub struct KeywordBlockingList {
    pub keywords: Vec<String>,
    /// 最多能加几条。服务端给的，界面拿它拦住「再加一条」。
    pub list_limit: Option<i32>,
    /// 单条最长几个字。
    pub char_limit: Option<i32>,
}

/// 消息中心四个 Tab 的未读数（gRPC 版）。
///
/// 与 [`crate::msg::MsgFeedUnread`] 同义不同源：那条是 REST 的
/// `/x/msgfeed/unread`，这条是 gRPC `GetTotalUnread` 回的一张
/// `map<string, int64>`。首页红点用前者，私信列表页用后者。
///
/// 两边都留着是基线的现状，不是这次迁移的选择——私信列表页顺手就能从
/// 这个响应里拿到四个数，再发一次 REST 请求是白费一次往返。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct ImTotalUnread {
    pub reply: i64,
    pub at: i64,
    pub like: i64,
    pub sys_msg: i64,
}

// ======================================================== 投影规则

/// 协议 `LocalRes` 里认证标的两个值。
///
/// 来自 `bilibili.dagw.component.avatar.common.ResourceSource.LocalRes`：
/// 3 = `LOCAL_RES_ICON_PERSONAL_VERIFY`，4 = `LOCAL_RES_ICON_ENTERPRISE_VERIFY`。
const LOCAL_RES_PERSONAL_VERIFY: i32 = 3;
const LOCAL_RES_ENTERPRISE_VERIFY: i32 = 4;

/// 把认证标的 `LocalRes` 值映射成界面用的认证类型。
///
/// `PendantAvatar` 的 `officialType` 是 0 个人 / 1 机构，与 REST 那一侧
/// （`official.type`）同一套取值，所以这里要映射而不是直接传。
///
/// 其余 `LocalRes`（会员图标、默认头像、关注按钮……）不是认证标，返回
/// `None`——画成认证标会让一堆人凭空多出个认证。
#[must_use]
pub fn official_type_from_local_res(local_value: i32) -> Option<i32> {
    match local_value {
        LOCAL_RES_PERSONAL_VERIFY => Some(0),
        LOCAL_RES_ENTERPRISE_VERIFY => Some(1),
        _ => None,
    }
}

/// 折叠入口的会话类型 → 二级会话列表的页面类型。
///
/// 这张表照抄基线 `lib/pages/whisper/widgets/item.dart` 的那个 `switch`，
/// **包括它的不对称**：`GROUP` 与 `GROUP_FOLD` 都映射到 `GROUP`，
/// `CUSTOMER_FOLD` 与 `CUSTOMER_ACCOUNT` 都映射到 `CUSTOMER`。两对各自
/// 合并成一页是产品决定，不是手滑。
///
/// 返回 `None` 表示没有对应的二级页，基线此时 toast 出枚举名。
///
/// 取值来自协议的 `SessionType` 与 `SessionPageType`，两个枚举的数字
/// **不一样**——这正是这张表存在的原因，少了它就会用会话类型去开页面，
/// 打开的是另一个列表而不会报错。
#[must_use]
pub fn fold_page_type(session_type: i32) -> Option<i32> {
    Some(match session_type {
        session_type::UNKNOWN => page_type::UNKNOWN,
        session_type::GROUP | session_type::GROUP_FOLD => page_type::GROUP,
        session_type::UNFOLLOWED => page_type::UNFOLLOWED,
        session_type::STRANGER => page_type::STRANGER,
        session_type::DUSTBIN => page_type::DUSTBIN,
        session_type::CUSTOMER_FOLD | session_type::CUSTOMER_ACCOUNT => page_type::CUSTOMER,
        session_type::AI_FOLD => page_type::AI,
        _ => return None,
    })
}

/// 协议 `SessionType` 的取值。
///
/// **这些数字必须对着生成的枚举核**，不能照着名字猜。两个枚举的顺序
/// 不一样（`SessionPageType` 里 `Home` 占了 1，把后面整体顶了一位），
/// 凭直觉写出来的表几乎每一行都是错的——而错了不会报错，只会打开另一个
/// 列表。`bili_api::grpc_im` 里有一条测试拿生成的枚举钉住这两组常量。
pub mod session_type {
    pub const UNKNOWN: i32 = 0;
    pub const PRIVATE: i32 = 1;
    pub const GROUP: i32 = 2;
    pub const GROUP_FOLD: i32 = 3;
    pub const UNFOLLOWED: i32 = 4;
    pub const STRANGER: i32 = 5;
    pub const DUSTBIN: i32 = 6;
    pub const CUSTOMER_FOLD: i32 = 7;
    pub const SYSTEM: i32 = 8;
    pub const AI_FOLD: i32 = 9;
    pub const CUSTOMER_ACCOUNT: i32 = 10;
}

/// 协议 `ThreeDotItemType` 的取值。见 [`session_type`] 的警告。
pub mod three_dot_type {
    pub const READ_ALL: i32 = 1;
    pub const MSG_SETTING: i32 = 2;
    pub const UP_HELPER: i32 = 4;
    pub const FANS_GROUP_HELPER: i32 = 6;
    pub const CONTACTS: i32 = 8;
    pub const CLEAR_LIST: i32 = 9;
}

/// 协议 `ImSettingType` 里**界面会主动打开**的那四个。
///
/// 其余二十来个取值由 `ImSettingRedirect::parent_setting_type` 带过来，
/// 不需要在这边列。这四个数字相邻且无规律（0、17、18、19），见
/// [`session_type`] 的警告。
pub mod setting_page_type {
    pub const ALL: i32 = 0;
    pub const OLD_REPLY_ME: i32 = 17;
    pub const OLD_AT_ME: i32 = 18;
    pub const OLD_RECEIVE_LIKE: i32 = 19;
}

/// 把协议的 `ThreeDotItemType` 取值映射成界面有分支的那几种。
#[must_use]
pub fn three_dot_kind(value: Option<i32>) -> ThreeDotKind {
    match value {
        Some(three_dot_type::READ_ALL) => ThreeDotKind::ReadAll,
        Some(three_dot_type::CLEAR_LIST) => ThreeDotKind::ClearList,
        Some(three_dot_type::MSG_SETTING) => ThreeDotKind::MsgSetting,
        Some(three_dot_type::UP_HELPER) => ThreeDotKind::UpHelper,
        Some(three_dot_type::CONTACTS) => ThreeDotKind::Contacts,
        Some(three_dot_type::FANS_GROUP_HELPER) => ThreeDotKind::FansGroupHelper,
        _ => ThreeDotKind::Other,
    }
}

/// 协议 `SessionPageType` 的取值。见 [`session_type`] 的警告。
pub mod page_type {
    pub const UNKNOWN: i32 = 0;
    pub const HOME: i32 = 1;
    pub const UNFOLLOWED: i32 = 2;
    pub const STRANGER: i32 = 3;
    pub const DUSTBIN: i32 = 4;
    pub const GROUP: i32 = 5;
    pub const HUA_HUO: i32 = 6;
    pub const AI: i32 = 7;
    pub const CUSTOMER: i32 = 8;
}

#[cfg(test)]
mod tests {
    use super::{fold_page_type, official_type_from_local_res, page_type, session_type};

    #[test]
    fn only_the_two_verify_icons_become_official_badges() {
        assert_eq!(official_type_from_local_res(3), Some(0), "个人认证");
        assert_eq!(official_type_from_local_res(4), Some(1), "机构认证");
        // 会员图标、默认头像、关注按钮都在同一个枚举里，画成认证标的话
        // 一堆人会凭空多出个认证。
        for not_a_badge in [0, 1, 2, 5, 6, 7, 8] {
            assert_eq!(official_type_from_local_res(not_a_badge), None);
        }
    }

    #[test]
    fn fold_types_map_onto_page_types_including_the_two_merges() {
        // 两对合并是产品决定，不是手滑：群与群折叠同一页，客服折叠与
        // 客服账号同一页。
        assert_eq!(
            fold_page_type(session_type::GROUP),
            fold_page_type(session_type::GROUP_FOLD)
        );
        assert_eq!(
            fold_page_type(session_type::CUSTOMER_FOLD),
            fold_page_type(session_type::CUSTOMER_ACCOUNT)
        );
        assert_eq!(fold_page_type(session_type::GROUP), Some(page_type::GROUP));
        assert_eq!(
            fold_page_type(session_type::CUSTOMER_FOLD),
            Some(page_type::CUSTOMER)
        );
    }

    /// 这条是整张表存在的理由，也是写它时真的踩到的坑。
    ///
    /// 两个枚举的数字**几乎每一行都对不上**：`SessionPageType` 里
    /// `Home` 占了 1，把后面整体顶了一位。凭名字猜出来的表能编译、能
    /// 跑、单测还自洽，表现只是「点折叠入口打开了另一个列表」。
    #[test]
    fn session_type_numbers_are_not_page_type_numbers() {
        // 五处数字不同，逐个点名。
        assert_eq!(fold_page_type(session_type::UNFOLLOWED), Some(2));
        assert_ne!(session_type::UNFOLLOWED, 2);
        assert_eq!(fold_page_type(session_type::STRANGER), Some(3));
        assert_ne!(session_type::STRANGER, 3);
        assert_eq!(fold_page_type(session_type::DUSTBIN), Some(4));
        assert_ne!(session_type::DUSTBIN, 4);
        assert_eq!(fold_page_type(session_type::GROUP_FOLD), Some(5));
        assert_ne!(session_type::GROUP_FOLD, 5);
        assert_eq!(fold_page_type(session_type::AI_FOLD), Some(7));
        assert_ne!(session_type::AI_FOLD, 7);
    }

    #[test]
    fn an_unmapped_fold_type_is_none_not_a_guess() {
        assert_eq!(
            fold_page_type(session_type::PRIVATE),
            None,
            "私聊不该出现在折叠入口上"
        );
        assert_eq!(
            fold_page_type(session_type::SYSTEM),
            None,
            "系统消息走 system_id 那一支，不是折叠"
        );
        assert_eq!(fold_page_type(99), None);
    }
}
