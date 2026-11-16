//! 用户空间那一族的领域实体（S6 / issue #56）。
//!
//! 对应基线 `lib/http/member.dart`：别人的资料卡、关注/粉丝计数、
//! 播放与点赞总数、空间主页、投稿与合集列表、专栏/音频/课堂/图文/会员购、
//! 大航海与充电排行。
//!
//! 「我的」那一族（自己的 nav、硬币、流水）在 [`crate::user`]；关注分组
//! 与关注关系在 [`crate::user_list`]。这里只放**看别人**的那部分。

use serde::Serialize;

/// 别人的资料（`/x/space/wbi/acc/info` 的 `data`）。
///
/// ## 只映射有人读的字段
///
/// 基线 `MemberInfoModel` 有 12 个字段，其中 `sex` / `sign` / `top_photo` /
/// `official` / `live_room` 在整个 `lib/` 里**没有任何消费者**——这条接口
/// 只喂横屏播放页的 up 主抽屉（`lib/pages/video/member/view.dart`），那里
/// 用到的就是下面这八个。理由同 [`crate::user::UserInfo`]：抄过来只会让
/// 下一个人以为它们有用。
///
/// 空间主页要的是另一条接口（`/x/space/v2/index`），字段多得多，走
/// [`SpacePage`]，不要试图让这个类型兼任。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct MemberInfo {
    pub mid: Option<i64>,
    pub name: Option<String>,
    pub face: Option<String>,
    /// 用户等级（0–6）。
    pub level: Option<i64>,
    /// 当前账号有没有关注他。
    pub is_followed: Option<bool>,
    /// 大会员状态，`> 0` 为开通。
    pub vip_status: Option<i64>,
    /// `2` 是年度大会员，昵称要染色。
    pub vip_type: Option<i64>,
    /// `1` 为硬核会员，等级图标要换一张。
    pub is_senior_member: Option<i64>,
}

/// 关注关系计数（`/x/relation/stat` 的 `data`）。
///
/// 与 [`crate::user::UserStat`] 是**两条接口**：那条是
/// `/x/web-interface/nav/stat`，只给自己看，还多一个动态数。这条按 `vmid`
/// 看任意一个人，没有动态数。不要合并。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct MemberRelationStat {
    /// 关注数。对方设了隐私时服务端给 `-1`，**照原样留着**：界面要据此
    /// 显示「已隐藏」而不是 0，归一化会把这个区别抹掉。
    pub following: Option<i64>,
    pub follower: Option<i64>,
}

/// 投稿播放与获赞总数（`/x/space/upstat` 的 `data`）。
///
/// 需要登录才有数据，基线的调用点因此先判了 `isLogin`。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct MemberUpStat {
    /// 稿件总播放数。
    pub archive_view: Option<i64>,
    /// 专栏总阅读数。
    pub article_view: Option<i64>,
    /// 获赞总数。
    pub likes: Option<i64>,
}

/// 用户名片（`/x/web-interface/card` 的 `data`）。
///
/// 与 [`MemberInfo`] 的差别不只是字段少：这条**不需要 WBI 签名、不需要
/// 浏览器 UA**，所以关注页、分组页这些只想要一个昵称的地方用它。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct MemberCard {
    /// 这条接口的 `card.mid` 是**字符串**（别处是整数）。这是服务端的
    /// 事实，不是笔误；基线的 `Card.mid` 也是 `String?`，跳转路由时直接
    /// 拼进 URL，所以这里不转成整数——转一次再拼回去只会多一个出错点。
    pub mid: Option<String>,
    pub name: Option<String>,
    pub face: Option<String>,
    /// 认证类型，`>= 0` 时头像右下角有角标。
    pub official_type: Option<i64>,
    pub vip_status: Option<i64>,
    pub vip_type: Option<i64>,
    /// 稿件数。
    pub archive_count: Option<i64>,
    /// 粉丝数。
    pub follower: Option<i64>,
}
