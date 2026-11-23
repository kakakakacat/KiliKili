//! 动态业务实体。协议消息只在 `bili-api` 内部出现。

use serde::{Deserialize, Serialize};

/// 首页动态红点的有效更新数。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct UnreadCount {
    pub count: i64,
}

/// 动态点赞列表中的作者信息。只保留调用点消费的字段。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DynamicAuthor {
    pub mid: i64,
    pub name: String,
    pub face: String,
    pub sign: String,
    pub level: i32,
    pub is_senior_member: bool,
    pub pendant_image: Option<String>,
    pub vip_status: Option<i32>,
    pub vip_type: Option<i32>,
    pub official_type: Option<i32>,
}

/// 点赞列表与下一页游标。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LikeListPage {
    pub items: Vec<DynamicAuthor>,
    pub has_more: bool,
    pub total_count: i64,
}

/// 预约卡片在创建页展示的数据。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReserveInfo {
    pub id: i64,
    pub title: String,
    pub live_plan_start_time: i64,
}

/// 动态预约按钮操作后的状态。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReserveAction {
    pub final_btn_status: Option<i32>,
    pub reserve_update: Option<i32>,
    pub desc_update: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VoteOption {
    pub opt_idx: Option<i32>,
    pub opt_desc: Option<String>,
    #[serde(default)]
    pub cnt: i64,
    pub img_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VoteInfo {
    pub choice_cnt: Option<i32>,
    pub default_share: Option<i32>,
    pub desc: Option<String>,
    pub end_time: Option<i64>,
    pub status: Option<i32>,
    pub uid: Option<i64>,
    pub vote_id: Option<i64>,
    #[serde(default)]
    pub join_num: i64,
    pub title: Option<String>,
    pub ctime: Option<i64>,
    pub my_votes: Option<Vec<i32>>,
    #[serde(default)]
    pub options: Vec<VoteOption>,
    pub options_cnt: Option<i32>,
    pub voter_level: Option<i32>,
    pub face: Option<String>,
    pub name: Option<String>,
    #[serde(rename = "type")]
    pub vote_type: Option<i32>,
    pub vote_publisher: Option<i64>,
    pub duration: Option<i64>,
    pub only_fans_level: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FolloweeVote {
    pub uid: i64,
    pub name: String,
    pub face: String,
    pub votes: Vec<i32>,
    pub ctime: i64,
}

// ------------------------------------------------- 专栏（S5 续单 / #150）

/// 专栏阅读页顶部的计数与收藏态（`/x/article/viewinfo`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ArticleInfo {
    pub favorite: Option<bool>,
    pub title: Option<String>,
    /// 原图地址。界面用它做长按保存，顺序即展示顺序。
    pub origin_image_urls: Vec<String>,
    pub stats: Option<ArticleStats>,
}

/// 专栏的四个计数。
///
/// 与 [`ArticleListItemStats`] **刻意不合并**：那边是 `view`/`like`/`reply`
/// 三个，这边是 `favorite`/`like`/`reply`/`share` 四个，两个接口给的本来
/// 就不是同一组。合成一个带四个 `Option` 的结构，等于让调用点自己去猜
/// 哪几个在这条路径上有值。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ArticleStats {
    pub favorite: Option<i64>,
    pub like: Option<i64>,
    pub reply: Option<i64>,
    pub share: Option<i64>,
}

/// 文集（`/x/article/list/web/articles`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ArticleList {
    pub info: Option<ArticleListInfo>,
    pub articles: Vec<ArticleListItem>,
    /// 作者。基线套的是共享的 `Owner`，这里**摊平成三个字段**——`Owner`
    /// 还有一堆别的生产者，把它一起拖进本批会让迁移面无限扩散，而文集
    /// 只读这三个（与 #57 对 `SubItemDto` 的处理同理）。
    pub author_mid: Option<i64>,
    pub author_name: Option<String>,
    pub author_face: Option<String>,
}

/// 文集本身的信息。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ArticleListInfo {
    pub id: Option<i64>,
    pub name: Option<String>,
    pub image_url: Option<String>,
    pub update_time: Option<i64>,
    pub words: Option<i64>,
    pub read: Option<i64>,
    pub articles_count: Option<i64>,
}

/// 文集里的一篇。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ArticleListItem {
    pub id: Option<i64>,
    pub title: Option<String>,
    pub image_urls: Vec<String>,
    pub summary: Option<String>,
    /// 对应的动态 id。点进去走动态详情，**是字符串不是数字**。
    pub dyn_id_str: Option<String>,
    pub stats: Option<ArticleListItemStats>,
}

/// 文集条目的三个计数。见 [`ArticleStats`] 为何不合并。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ArticleListItemStats {
    pub view: Option<i64>,
    pub like: Option<i64>,
    pub reply: Option<i64>,
}

// ------------------------------------------------- 话题（S5 续单 / #150）

/// 一个话题。
///
/// 基线把 `id`/`name` 声明成非空而四个计数 `?? 0`——那是**对协议的判断**
/// （话题没有 id 就不是一个话题），照搬过来：解不出 `id` 或 `name` 的条目
/// 在 [`crate::dynamic`] 的解析里整条丢掉，而不是补一个 0 号话题。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TopicItem {
    pub id: i64,
    pub name: String,
    pub view: i64,
    pub discuss: i64,
    pub fav: i64,
    pub like: i64,
    pub description: Option<String>,
    pub is_fav: Option<bool>,
    pub is_like: Option<bool>,
}

/// 话题页顶部：话题本身 + 创建者。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TopicTop {
    pub topic: Option<TopicItem>,
    pub creator: Option<TopicCreator>,
}

/// 发布页的话题搜索结果。
///
/// 基线套了一层 `PageInfo` 只为一个 `has_more`，这里摊平——跨 FFI 多一个
/// 导出类型，而它里面只有一个布尔。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TopicPubSearch {
    pub items: Vec<TopicItem>,
    pub has_more: Option<bool>,
}

/// 话题创建者。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TopicCreator {
    pub uid: Option<i64>,
    pub face: Option<String>,
    pub name: Option<String>,
}

// ----------------------------------------- 部落气泡（S5 续单 / #150）

/// 动态页顶部的「部落」入口（`/x/tribee/v1/dyn/all`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Bubble {
    pub base_info: Option<BubbleBaseInfo>,
    pub content: Option<BubbleContent>,
    pub categories: Vec<BubbleCategory>,
    pub sort_info: Option<BubbleSortInfo>,
}

/// 部落本身与是否已加入。
///
/// 基线的 `BaseInfo` 只套了一层 `TribeInfo`，这里**摊平**：多一层壳跨
/// FFI 要多一个导出类型，而它除了包住 `TribeInfo` 没有别的内容。
/// 注意协议里的键是 `tribee_info`（两个 e），不是 `tribe_info`。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BubbleBaseInfo {
    pub is_joined: Option<bool>,
    pub id: Option<String>,
    pub title: Option<String>,
    pub sub_title: Option<String>,
    pub face_url: Option<String>,
    pub jump_uri: Option<String>,
    pub summary: Option<String>,
}

/// 部落里的动态列表。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BubbleContent {
    /// 服务端给的是**已格式化好的串**（「1.2万」这类），不是数字。
    pub count: Option<String>,
    pub items: Vec<BubbleDyn>,
}

/// 部落动态的一条。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BubbleDyn {
    pub dyn_id: Option<String>,
    pub title: Option<String>,
    pub author: Option<String>,
    pub time_text: Option<String>,
    /// 与 `view_stat` 一样是**服务端格式化过的串**，不要收成数字。
    pub reply_count: Option<String>,
    pub view_stat: Option<String>,
}

/// 部落分类。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BubbleCategory {
    pub id: Option<String>,
    pub name: Option<String>,
    pub kind: Option<i64>,
}

/// 排序选项。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BubbleSortInfo {
    pub show_sort: Option<bool>,
    pub cur_sort_type: Option<i64>,
    pub items: Vec<BubbleSortItem>,
}

/// 一个排序选项。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BubbleSortItem {
    pub sort_type: Option<i64>,
    pub text: Option<String>,
}
