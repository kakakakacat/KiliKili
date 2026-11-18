//! 收藏夹相关的领域实体（S8 / issue #58）。
//!
//! 对应基线 `lib/http/fav.dart` 的全部只读接口，以及 `lib/models_new/fav/`
//! 下那一批模型（本片迁完已删）。
//!
//! ## 「收藏夹」在这条协议里是五件不同的事
//!
//! 基线把它们堆在同一个 `FavHttp` 里，很容易以为是一套接口的变体。实际
//! 分属五棵完全不同的响应树：
//!
//! | 界面上的页签 | 接口 | 实体 |
//! | --- | --- | --- |
//! | 视频收藏夹 | `/x/v3/fav/folder/*`、`/x/v3/fav/resource/list` | [`FavFolder`] / [`FavDetail`] |
//! | 追番追剧 | `/x/space/bangumi/follow/list` | [`FavPgcList`] |
//! | 课程 | `/pugv/app/web/favorite/page` | [`FavPugvList`] |
//! | 话题 | `/x/topic/web/fav/list` | [`FavTopicList`] |
//! | 专栏 / 笔记 | `/x/polymer/...opus/feed/fav`、`/x/note/list` | [`FavArticleList`] / [`FavNote`] |
//!
//! 它们之间**没有共享字段**，所以这里也不去造一个「收藏项」的公共父类
//! ——基线也没有，那样只会得到一个每页只用三分之一字段的结构。
//!
//! ## 嵌套一律摊平
//!
//! `upper`（作者）、`cnt_info`（播放/弹幕计数）、`ogv`/`ugc`、`new_ep`、
//! `page` 这些单字段或双字段的子对象在这里全部摊平成前缀字段，理由同
//! [`crate::user::SubItem`]：基线给它们各起了一个类（`Owner`、`CntInfo`、
//! `Ogv`…），而那几个类还有别的生产者，连它们一起搬会让迁移面无限扩散。
//!
//! ## 字段为什么几乎全是 `Option`
//!
//! 与其余切片同理（ADR-011）：服务端少回一个字段不该让整页列表判失败。
//! 例外只有 [`FavFolder`] 的几个——基线那边就给了默认值（`id = 0`、
//! `title = ''`），界面直接读不判空，照搬。

use serde::Serialize;

// ------------------------------------------------------------- 视频收藏夹

/// 一个收藏夹的信息。对应基线 `FavFolderInfo`。
///
/// 它既是收藏夹列表的一项，也是收藏夹详情页顶部那一块——基线是同一个类，
/// 服务端也确实回同一棵子树。
///
/// 非 `Option` 的那几个字段照搬基线的默认值：界面拿 `id` 拼路由、拿
/// `title` 当标题，`null` 和 `0`/`''` 在那里本来就是一回事，包一层
/// `Option` 只会让每个调用点多一个 `?? ''`。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct FavFolder {
    pub id: i64,
    /// 不含用户 mid 后缀的短 id。批量操作的参数用它。
    pub fid: Option<i64>,
    pub mid: i64,
    /// 属性位掩码。基线用它判是否默认收藏夹（`attr & 1`）与是否私密
    /// （`attr & 2`）。
    pub attr: i64,
    pub title: String,
    pub cover: String,
    pub upper_mid: Option<i64>,
    pub upper_name: Option<String>,
    pub upper_face: Option<String>,
    pub intro: Option<String>,
    /// 当前登录账号是否已收藏这个收藏夹（订阅别人的收藏夹用）。
    pub fav_state: Option<i64>,
    pub media_count: i64,
}

/// 收藏夹列表的一页。对应基线 `FavFolderData`。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct FavFolderList {
    pub count: Option<i64>,
    pub list: Vec<FavFolder>,
    pub has_more: Option<bool>,
}

/// 收藏夹详情的一页：夹子信息 + 这一页的内容。对应基线 `FavDetailData`。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct FavDetail {
    pub info: Option<FavFolder>,
    pub medias: Vec<FavMedia>,
    pub has_more: Option<bool>,
}

/// 收藏夹里的一项内容。对应基线 `FavDetailItemModel`。
///
/// 一个收藏夹里可以同时有稿件、番剧、专栏，`kind`（协议的 `type`）区分：
/// 2 视频、12 音频、24 番剧（ogv）… 基线按它决定点开去哪。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct FavMedia {
    pub id: Option<i64>,
    /// 协议里的 `type`。Rust 关键字，改了名。
    pub kind: Option<i64>,
    pub title: Option<String>,
    pub cover: Option<String>,
    pub intro: Option<String>,
    pub duration: Option<i64>,
    pub upper_mid: Option<i64>,
    pub upper_name: Option<String>,
    pub upper_face: Option<String>,
    /// 失效标记。基线判 `attr == 9`（UP 主删除）与 `attr == 1`（不可见）
    /// 来给卡片打上「已失效」。
    pub attr: Option<i64>,
    pub play_count: Option<i64>,
    pub danmaku_count: Option<i64>,
    /// 收藏时间，Unix 秒。
    pub fav_time: Option<i64>,
    /// **`bvid` 与 `bv_id` 两个键都要认。**
    ///
    /// 基线写的是 `json['bvid'] ?? json['bv_id']`：收藏夹详情给的是
    /// `bvid`，搜索收藏夹那条路径给的是 `bv_id`。少认一个的表现不是报错，
    /// 是卡片点不开。
    pub bvid: Option<String>,
    /// 番剧项才有：`ogv.type_name`（「番剧」「电影」…）。
    pub ogv_type_name: Option<String>,
    /// 番剧项才有：`ogv.season_id`，点开要用它。
    pub ogv_season_id: Option<i64>,
    /// 稿件项才有：`ugc.first_cid`，直接起播要用它。
    pub ugc_first_cid: Option<i64>,
}

// --------------------------------------------------------------- 追番追剧

/// 追番/追剧列表的一页。对应基线 `FavPgcData`。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct FavPgcList {
    pub list: Vec<FavPgcItem>,
    pub total: Option<i64>,
}

/// 追番/追剧列表里的一项。对应基线 `FavPgcItemModel`。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct FavPgcItem {
    pub season_id: Option<i64>,
    pub title: Option<String>,
    pub cover: Option<String>,
    /// 1 为已完结。
    pub is_finish: Option<i64>,
    pub badge: Option<String>,
    /// `new_ep.index_show`，形如「更新至第 12 话」。
    pub new_ep_index_show: Option<String>,
    pub renewal_time: Option<String>,
    /// 观看进度，形如「看到第 3 话」。
    ///
    /// **空串归一成 `None`**：服务端对没看过的季回的是 `""`，基线显式写了
    /// `json['progress'] == '' ? null : json['progress']`，界面据此决定要不
    /// 要显示那一行。不归一的话每张卡片底下都会多出一条空白。
    pub progress: Option<String>,
}

// ----------------------------------------------------------------- 课程

/// 收藏的课程一页。对应基线 `SpaceCheeseData`。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct FavPugvList {
    pub items: Vec<FavPugvItem>,
    /// `page.next`。为 `Some(false)` 时到底了。
    ///
    /// 不折算成 `is_end`：服务端缺这个字段时基线是「继续翻页」，而
    /// `is_end: bool` 的默认值会把它变成「到底了」，少一页内容。
    pub has_next: Option<bool>,
}

/// 收藏的一门课程。对应基线 `SpaceCheeseItem`。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct FavPugvItem {
    pub cover: Option<String>,
    /// 角标文案（「限时优惠」之类），服务端给一个字符串数组。
    pub marks: Vec<String>,
    pub season_id: Option<i64>,
    pub status: Option<String>,
    pub title: Option<String>,
    pub ctime: Option<String>,
}

// ----------------------------------------------------------------- 话题

/// 收藏的话题一页。对应基线 `FavTopicData`，中间那两层
/// （`topic_list.topic_items` / `topic_list.page_info.total`）摊平了。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct FavTopicList {
    pub items: Vec<FavTopicItem>,
    pub total: Option<i64>,
}

/// 一个话题。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct FavTopicItem {
    pub id: Option<i64>,
    pub name: Option<String>,
}

// ------------------------------------------------------------ 专栏 / 笔记

/// 收藏的专栏（图文动态）一页。对应基线 `FavArticleData`。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct FavArticleList {
    pub items: Vec<FavArticleItem>,
    pub has_more: Option<bool>,
}

/// 收藏的一篇专栏。对应基线 `FavArticleItemModel`。
///
/// `opus_id` 是**字符串**：这条接口给的 id 超出 JS 安全整数范围，服务端
/// 因此回字符串，取消收藏时也原样发回去。转成数字再转回来是白白给自己
/// 找一次精度丢失。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct FavArticleItem {
    pub opus_id: Option<String>,
    pub content: Option<String>,
    pub author_name: Option<String>,
    pub cover_url: Option<String>,
    /// `stat.like`，服务端给的就是字符串（可能是「1.2万」这种缩写）。
    pub like: Option<String>,
    pub pub_time: Option<String>,
}

/// 一条笔记。对应基线 `FavNoteItemModel`。
///
/// 「我的笔记」与「我发布的笔记」是两条接口、同一个形状，删除时各用各的
/// id：发布过的用 `cvid`，没发布的用 `note_id`。两个都是**字符串**——
/// 基线声明成 `dynamic` 并直接 `join(',')`，服务端这里给的是字符串形态的
/// 大整数，按数字收会在边界上丢精度。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct FavNote {
    pub web_url: Option<String>,
    pub title: Option<String>,
    pub summary: Option<String>,
    pub message: Option<String>,
    /// `arc.pic`，关联稿件的封面。
    pub pic: Option<String>,
    pub cvid: Option<String>,
    pub note_id: Option<String>,
}

// ------------------------------------------------- 个人空间的收藏夹 / 合集

/// 个人空间「收藏」页的一组。对应基线 `SpaceFavData`。
///
/// 服务端把「收藏夹」和「订阅的合集」分成若干组回来，每组一个标题
/// （`name`）。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct SpaceFavGroup {
    pub id: Option<i64>,
    pub name: Option<String>,
    pub count: Option<i64>,
    /// 组里的收藏夹或合集。
    ///
    /// 复用 [`crate::user::SubItem`] 而不是另造一个：基线这里用的
    /// `SpaceFavItemModel` 继承自 `SubItemModel`，只多了 `media_id` /
    /// `count` / `is_public` 三个字段，而**这个页面一个都没用到**（卡片读
    /// 的是 id、type、media_count、attr、upper、view_count、state）。
    ///
    /// 复用还有一个实际好处：组里的合集点开就是合集详情页，那一页要的正是
    /// `SubItem`。另造一个类型的话，这里就得在 Dart 侧拼一个 `SubItem` 去
    /// 冒充取数结果——那正是 `sub_compat.dart` 当初写明不做的方向。
    pub list: Vec<crate::user::SubItem>,
}

/// 合集（season）详情的一页。对应基线 `SubDetailData`。
///
/// `info` 复用 [`crate::user::SubItem`]：订阅列表页点进来就是这一页，
/// 两处喂的是同一个头部（基线 `SubDetailController.subInfo` 两处都赋值）。
/// 这也正是 `lib/services/rust/sub_compat.dart` 等着被删的那条依赖。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct FavSeasonDetail {
    pub info: Option<crate::user::SubItem>,
    pub medias: Vec<FavSeasonMedia>,
}

/// 合集里的一个稿件。对应基线 `SubDetailItemModel`。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct FavSeasonMedia {
    pub id: Option<i64>,
    pub title: Option<String>,
    pub cover: Option<String>,
    pub duration: Option<i64>,
    pub pubtime: Option<i64>,
    pub bvid: Option<String>,
    pub play_count: Option<i64>,
    pub danmaku_count: Option<i64>,
}
