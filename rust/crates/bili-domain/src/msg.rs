//! 消息中心与私信的领域实体（S12 / issue #63）。
//!
//! 对应基线 `lib/http/msg.dart` 与 `lib/models_new/msg/**`、
//! `lib/models_new/{msgfeed_unread,single_unread}/`。
//!
//! ## 三条 feed 为什么不合并成一个类型
//!
//! 「回复我的」「@我的」「收到的赞」在界面上是三个并列的 Tab，响应也长得
//! 很像——但**不一样**，而且差异恰好落在翻页上：
//!
//! - 回复与 @ 是一层 `{cursor, items}`；赞是两段 `{latest, total}`，
//!   `latest` 根本没有游标，`total` 才有；
//! - 回复带 `last_view_at`（用来画「以下为已读」的分隔线），@ 没有；
//! - 赞的一条是**一群人赞了同一个东西**（`users` 是列表、`counts` 是人数），
//!   回复与 @ 的一条只有一个人。
//!
//! 合成一个带一堆 `Option` 的「消息项」，三处调用点都要再问一遍「这个字段
//! 这个 Tab 有没有」，而这正是类型本该替它们答的问题。
//!
//! ## 没有「私信消息」实体
//!
//! 私信的收发在 gRPC 那一侧（`lib/grpc/im.dart`），实体见
//! [`crate::reply_grpc`] 同级的 IM 模块。本模块只覆盖 REST 的那半边：
//! 消息中心的三条 feed、系统通知、未读数与会话设置。

use serde::Serialize;

/// 三条 feed 共用的翻页游标。
///
/// 三个字段都可能缺：服务端在「没有下一页」时只回 `is_end`。基线把它们
/// 都声明成可空，这里照搬——`id`/`time` 缺失与「等于 0」在翻页上的含义
/// 不同（前者是不要再翻了，后者是从头开始），用 `0` 顶替会把第二页请求
/// 发成第一页。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct FeedCursor {
    pub is_end: Option<bool>,
    pub id: Option<i64>,
    pub time: Option<i64>,
}

/// feed 里的一位用户。
///
/// 三条 feed 的 `user` 子树字段名完全一致（`mid`/`nickname`/`avatar`），
/// 所以共用一个类型；基线为每条 feed 各抄了一个同名的 `User` 类，于是
/// 三个文件里有三个 `User`，import 时要靠路径区分。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct FeedUser {
    pub mid: Option<i64>,
    pub nickname: Option<String>,
    pub avatar: Option<String>,
}

// ------------------------------------------------------------ 回复我的

/// 「回复我的」一页。
#[derive(Debug, Clone, Default, Serialize)]
pub struct ReplyFeed {
    pub cursor: Option<FeedCursor>,
    pub items: Vec<ReplyFeedItem>,
    /// 上次查看的时间戳，界面用它画「以下为已读」的分隔线。
    pub last_view_at: Option<i64>,
}

/// 「回复我的」里的一条。
#[derive(Debug, Clone, Default, Serialize)]
pub struct ReplyFeedItem {
    pub id: Option<i64>,
    pub user: Option<FeedUser>,
    pub item: Option<ReplyFeedContent>,
    pub counts: Option<i64>,
    /// 非 0 表示「还有 N 条类似回复」，界面据此折叠。
    pub is_multi: Option<i64>,
    pub reply_time: Option<i64>,
}

/// 被回复的那条内容。
#[derive(Debug, Clone, Default, Serialize)]
pub struct ReplyFeedContent {
    pub subject_id: Option<i64>,
    pub business_id: Option<i64>,
    pub business: Option<String>,
    pub native_uri: Option<String>,
    pub root_reply_content: Option<String>,
    pub source_content: Option<String>,
    pub target_reply_content: Option<String>,
}

// -------------------------------------------------------------- @ 我的

/// 「@我的」一页。
#[derive(Debug, Clone, Default, Serialize)]
pub struct AtFeed {
    pub cursor: Option<FeedCursor>,
    pub items: Vec<AtFeedItem>,
}

/// 「@我的」里的一条。
#[derive(Debug, Clone, Default, Serialize)]
pub struct AtFeedItem {
    pub id: Option<i64>,
    pub user: Option<FeedUser>,
    pub item: Option<AtFeedContent>,
    pub at_time: Option<i64>,
}

/// @ 发生在哪条内容里。
#[derive(Debug, Clone, Default, Serialize)]
pub struct AtFeedContent {
    pub business: Option<String>,
    pub image: Option<String>,
    pub source_content: Option<String>,
    pub native_uri: Option<String>,
}

// ------------------------------------------------------------ 收到的赞

/// 「收到的赞」一页。两段结构：`latest` 是最近未读的一批（无游标），
/// `total` 是全部（带游标）。界面把两段拼起来显示。
#[derive(Debug, Clone, Default, Serialize)]
pub struct LikeFeed {
    pub latest: Option<LikeLatest>,
    pub total: Option<LikeTotal>,
}

/// 最近未读的一批赞。没有游标——它永远只有一页。
#[derive(Debug, Clone, Default, Serialize)]
pub struct LikeLatest {
    pub items: Vec<LikeFeedItem>,
    pub last_view_at: Option<i64>,
}

/// 全部赞，可翻页。
#[derive(Debug, Clone, Default, Serialize)]
pub struct LikeTotal {
    pub cursor: Option<FeedCursor>,
    pub items: Vec<LikeFeedItem>,
}

/// 「收到的赞」里的一条：**一群人**赞了同一个东西。
#[derive(Debug, Clone, Default, Serialize)]
pub struct LikeFeedItem {
    pub id: Option<i64>,
    /// 点赞的人。顺序即服务端给的顺序，界面按这个顺序排头像。
    pub users: Vec<FeedUser>,
    pub item: Option<LikeFeedContent>,
    /// 总人数。可能大于 `users.len()`——服务端只回前几个人的信息。
    pub counts: Option<i64>,
    pub like_time: Option<i64>,
    /// 这条的「接收通知」开关状态，由 `msg_set_notice` 改。
    pub notice_state: Option<i64>,
}

/// 被赞的那条内容。
#[derive(Debug, Clone, Default, Serialize)]
pub struct LikeFeedContent {
    pub business: Option<String>,
    pub title: Option<String>,
    pub image: Option<String>,
    pub native_uri: Option<String>,
}

// -------------------------------------------------------- 赞的明细页

/// 某一条「收到的赞」展开后的人名单。
#[derive(Debug, Clone, Default, Serialize)]
pub struct LikeDetail {
    pub page: Option<LikeDetailPage>,
    pub card: Option<LikeDetailCard>,
    pub items: Vec<LikeDetailItem>,
}

/// 明细页的翻页状态。只有 `is_end`——这个接口按页码翻，不用游标。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct LikeDetailPage {
    pub is_end: Option<bool>,
}

/// 明细页顶部那张「被赞的是什么」的卡片。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct LikeDetailCard {
    pub business: Option<String>,
    pub title: Option<String>,
}

/// 明细页里的一个人。
#[derive(Debug, Clone, Default, Serialize)]
pub struct LikeDetailItem {
    pub user: Option<FeedUser>,
    pub like_time: Option<i64>,
}

// ---------------------------------------------------------- 系统通知

/// 一条系统通知。
///
/// `content` 在协议里是一段**可能是 JSON 的字符串**：服务端对多端分别给了
/// 文案（`{"web": "...", "android": "..."}`），也可能直接就是纯文本。取哪
/// 一个是「协议没规定、但两端必须一致」的决定，所以解析时就定下来（见
/// `bili_api::msg::parse_sys_msg`），领域实体里只剩最终要显示的那一段。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct SysMsgItem {
    pub id: Option<i64>,
    /// 翻页用的游标，同时也是「已读到哪」的标记（`msg_sys_update_cursor`）。
    pub cursor: Option<i64>,
    pub title: Option<String>,
    pub content: Option<String>,
    /// 形如 `2024-01-15 10:35`，服务端已经格式化好，不是时间戳。
    pub time_at: Option<String>,
}

// ---------------------------------------------------------- 未读数

/// 私信未读数的各个分桶。
///
/// 八个字段全部给默认值 0 而不是 `Option`：它们是计数，`null` 与 0 在界面
/// 上是同一件事（不显示红点），基线也是 `json['x'] ?? 0`。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct SingleUnread {
    pub unfollow_unread: i64,
    pub follow_unread: i64,
    pub unfollow_push_msg: i64,
    pub dustbin_push_msg: i64,
    pub dustbin_unread: i64,
    pub biz_msg_unfollow_unread: i64,
    pub biz_msg_follow_unread: i64,
    pub custom_unread: i64,
}

impl SingleUnread {
    /// 私信这一项算进红点的总数。
    ///
    /// 基线把这个加法写在 `lib/pages/main/controller.dart` 的 `_msgUnread`
    /// 里。**哪几个桶算进红点是产品决定**，不是展示细节：换掉 Flutter 这层
    /// 时它得原样存在，所以按 AGENTS.md §2 的判据下沉到这里。
    ///
    /// 算六项，**刻意漏掉两个 `dustbin_*`**——垃圾桶里的消息不提醒。这是
    /// 基线的既有行为，不是这次迁移的判断；如果哪天认为该算，改这里一处，
    /// 而不是改调用点。
    #[must_use]
    pub fn badge_count(&self) -> i64 {
        self.follow_unread
            + self.unfollow_unread
            + self.biz_msg_follow_unread
            + self.biz_msg_unfollow_unread
            + self.unfollow_push_msg
            + self.custom_unread
    }
}

/// 消息中心四个 Tab 的未读数。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct MsgFeedUnread {
    pub at: i64,
    pub like: i64,
    pub reply: i64,
    pub sys_msg: i64,
}

/// 消息中心入口的红点要算哪几个 Tab。
///
/// 用户在设置页逐项勾选（基线 `Pref.msgUnReadTypes` 与
/// `Pref.disableLikeMsg`），所以这是**输入**而不是常量。做成结构体而不是
/// 四个 bool 参数：四个同类型的位置参数传反了编译器不会说话，而「@ 和赞
/// 调了个个儿」在界面上只是数字小了一点，不会有人注意到。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FeedBadgeSelection {
    pub reply: bool,
    pub at: bool,
    pub like: bool,
    pub sys_msg: bool,
}

impl MsgFeedUnread {
    /// 消息中心入口上那个数字。
    ///
    /// 与 [`SingleUnread::badge_count`] 同理，这条加法属于 Rust；但它比
    /// 私信那条多一层：**算哪几项由用户勾选**，所以选择集从外面传进来。
    ///
    /// 基线把「赞」的开关做成了两级（先看 `msgUnReadTypes` 里有没有 like，
    /// 再看 `disableLikeMsg` 有没有关掉），这里合并成
    /// [`FeedBadgeSelection::like`] 一个布尔——两级的与运算在调用点一眼
    /// 看得见，拆成两个字段只会让这里多一个要解释的参数。
    #[must_use]
    pub fn badge_count(&self, pick: FeedBadgeSelection) -> i64 {
        let mut n = 0;
        if pick.reply {
            n += self.reply;
        }
        if pick.at {
            n += self.at;
        }
        if pick.like {
            n += self.like;
        }
        if pick.sys_msg {
            n += self.sys_msg;
        }
        n
    }
}

// ------------------------------------------------------ 私信会话设置

/// 一位 IM 用户的资料。私信设置页顶部那张卡片用它。
///
/// `vip`/`pendant`/`official` 三棵子树在基线里复用了 `model_avatar.dart`
/// 的公共类型，而那几个类型还有别的切片在用——这里只取**这张卡片实际
/// 显示的那几个字段**，不把整棵公共子树搬过来：搬了就得和 S6/S7 抢同一
/// 组导出类型的所有权，而这张卡片只画了头像、会员色和认证标。
///
/// 字段名跟的是**基线实际读的那几个**（`BaseOfficialVerify` 读 `type` 与
/// `desc`，不是 `role`/`title`）。`desc` 没取：`PendantAvatar` 只要
/// `officialType` 画那个小标，`desc` 在这张卡片上没有落点。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ImUserInfo {
    pub mid: Option<i64>,
    pub name: Option<String>,
    pub face: Option<String>,
    pub sign: Option<String>,
    /// 会员状态：0 无、>0 是会员。基线对缺失当作 0。
    pub vip_status: Option<i64>,
    /// 会员类型：2 是年度及以上，昵称才染色。
    pub vip_type: Option<i64>,
    /// 挂件图。没有挂件时服务端给空串，解析时已归一成 `None`。
    pub pendant_image: Option<String>,
    /// 认证类型：-1 无认证、0 个人、1 机构。
    pub official_type: Option<i64>,
}

/// 与某人的会话设置。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct SessionSs {
    /// 关注状态。128 是「已拉黑」，`None` 表示没有关系。
    pub follow_status: Option<i64>,
    /// 内容推送开关：1 开、0 关。
    pub push_setting: Option<i64>,
    /// 是否显示推送开关。某些账号类型没有这个开关。
    pub show_push_setting: Option<i64>,
}

/// 对某个 uid 的免打扰设置。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct MsgDndSetting {
    /// 1 为接收消息提醒，0 为免打扰。
    pub setting: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::{FeedBadgeSelection, MsgFeedUnread, SingleUnread};

    /// 用 2 的幂当桶值：漏算或多算哪一个，从差值一眼看得出是哪个桶。
    const UNREAD: SingleUnread = SingleUnread {
        follow_unread: 1,
        unfollow_unread: 2,
        biz_msg_follow_unread: 4,
        biz_msg_unfollow_unread: 8,
        unfollow_push_msg: 16,
        custom_unread: 32,
        dustbin_push_msg: 64,
        dustbin_unread: 128,
    };

    #[test]
    fn pm_badge_sums_the_six_buckets_the_baseline_sums() {
        assert_eq!(UNREAD.badge_count(), 63);
    }

    #[test]
    fn dustbin_buckets_stay_out_of_the_badge() {
        // 垃圾桶里的两项合计 192。它们进了红点的话这里会是 255。
        assert_eq!(UNREAD.badge_count(), 63, "垃圾桶不该提醒");
    }

    const FEED: MsgFeedUnread = MsgFeedUnread {
        at: 1,
        like: 2,
        reply: 4,
        sys_msg: 8,
    };

    #[test]
    fn feed_badge_counts_only_what_the_user_picked() {
        let all = FeedBadgeSelection {
            reply: true,
            at: true,
            like: true,
            sys_msg: true,
        };
        assert_eq!(FEED.badge_count(all), 15);

        let no_like = FeedBadgeSelection { like: false, ..all };
        assert_eq!(FEED.badge_count(no_like), 13, "关掉赞应当正好少 2");

        assert_eq!(
            FEED.badge_count(FeedBadgeSelection::default()),
            0,
            "一项都没勾时不显示红点"
        );
    }
}
