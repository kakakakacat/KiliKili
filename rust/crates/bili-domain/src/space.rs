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

// ------------------------------------------------------- 合集与列表、充电

/// 一页「合集和列表」（`/x/polymer/web-space/seasons_series_list`）。
///
/// 合集（season）与列表（series）分两个数组下发，**这里保持分开**：
/// 基线的界面把两者拼成一条列表显示，但合集与列表跳转到的是不同的页面，
/// 区分靠的就是 `season_id` / `series_id` 哪个有值。拼接是界面的事，
/// 协议的分法留在协议层。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct SpaceSeasonSeriesPage {
    /// 两个数组加起来的总数，用于判断翻到底没有。
    pub total: Option<i64>,
    pub seasons: Vec<SpaceSeasonSeries>,
    pub series: Vec<SpaceSeasonSeries>,
}

/// 一个合集或列表。
///
/// 基线的 `SpaceSsModel` 只有一个 `meta` 字段，这里把它摊平——多一层只为
/// 复刻服务端的嵌套，没有第二个字段跟它并列。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct SpaceSeasonSeries {
    pub cover: Option<String>,
    pub name: Option<String>,
    /// 最近更新时间，Unix 秒。
    pub ptime: Option<i64>,
    /// 里面有多少个稿件。
    pub total: Option<i64>,
    /// 合集 id。与 [`Self::series_id`] 必有其一，界面据此决定跳哪个页面。
    pub season_id: Option<i64>,
    /// 列表 id。
    pub series_id: Option<i64>,
}

/// 一页「他投币过/点赞过的视频」（`/x/v2/space/coinarc`、`/x/v2/space/likearc`）。
///
/// 两条接口的响应形状完全一样，只换路径。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct CoinLikeArcPage {
    pub count: Option<i64>,
    pub items: Vec<CoinLikeArcItem>,
}

/// 投币/点赞列表里的一个稿件。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct CoinLikeArcItem {
    pub title: Option<String>,
    pub cover: Option<String>,
    /// 形如 `bilibili://video/123`，由界面交给路由解析。
    pub uri: Option<String>,
    /// 稿件 id，**字符串**（这条接口如此）。
    pub param: Option<String>,
    pub duration: Option<i64>,
    /// 互动视频。
    pub is_steins: Option<bool>,
    /// 联合投稿。
    pub is_cooperation: Option<bool>,
    pub is_pgc: Option<bool>,
    pub play: Option<i64>,
    pub danmaku: Option<i64>,
    /// 投币/点赞的时间，Unix 秒。不是稿件发布时间。
    pub ctime: Option<i64>,
}

/// 一页大航海名单（`/xlive/app-ucenter/v1/guard/MainGuardCardAll`）。
///
/// 挂在直播域上，base_url 由调用方给。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct MemberGuardPage {
    /// `1` 表示还有下一页。服务端给的就是整数，不是布尔。
    pub has_more: Option<i64>,
    pub items: Vec<GuardItem>,
}

/// 一位舰长/提督/总督。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct GuardItem {
    pub uid: Option<i64>,
    pub username: Option<String>,
    pub face: Option<String>,
    /// 1 总督、2 提督、3 舰长。
    pub guard_level: Option<i64>,
}

/// 充电排行（`/x/upower/up/member/rank/v2`）。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct UpowerRank {
    pub items: Vec<UpowerRankItem>,
    /// 当前这一页是哪个档位的。
    pub privilege_type: Option<i64>,
    pub tabs: Vec<i64>,
    /// 各档位的名字与人数。只有一个档位时界面不显示 tab 条。
    pub levels: Vec<UpowerLevel>,
}

/// 充电排行里的一个人。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct UpowerRankItem {
    pub mid: Option<i64>,
    pub nickname: Option<String>,
    pub avatar: Option<String>,
    /// 已充电天数。
    pub day: Option<i64>,
}

/// 一个充电档位。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct UpowerLevel {
    pub privilege_type: Option<i64>,
    pub name: Option<String>,
    pub member_total: Option<i64>,
}
