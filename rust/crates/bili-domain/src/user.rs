//! 「我的」那一族的领域实体（S7 / issue #57）。
//!
//! 对应基线 `lib/http/user.dart` 里围绕账号本身的接口：登录用户信息、
//! 三个计数、硬币数与三份流水、实名、空间隐私设置、订阅的收藏夹/合集、
//! 视频标签。
//!
//! 历史记录、稍后再看与 `media_list` **不在这里**：它们的基线模型里混着
//! widget（`PBadge`、`VideoProgressIndicator`）与多选 mixin，迁移要连同
//! 那几个列表页的 item widget 一起改，是另一批。

use serde::{Deserialize, Serialize};

/// 登录用户信息（`/x/web-interface/nav` 的 `data`）。
///
/// ## 只映射有人读的字段
///
/// 基线 `UserInfoData` 有 25 个字段，其中 **17 个在整个 `lib/` 里没有任何
/// 消费者**（`official` / `officialVerify` / `pendant` / `wallet` 这几个在
/// 基线里干脆是裸 `Map`，连类型都没给）。这里只留下真正被读的八个，理由同
/// `crate::match_info`：把没人读的字段抄过来，只会让下一个人以为它们有用，
/// 并在接口改形状时产生一次假的「解析失败」。
///
/// 需要其中某个字段时**在这里加**，别在 Dart 侧另解一遍原始 JSON。
/// 本类型还兼作**本地缓存的存盘格式**：`crate::user` 的
/// `serialize_user_info_cache` / `parse_user_info_cache` 直接用它的 serde
/// 表示（S7 #57，缓存由 Hive 对象改存 JSON）。因此加字段时必须保证
/// `#[serde(default)]` 仍在——老缓存里没有新字段，少了它整份缓存会解不出来。
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct UserInfo {
    /// 是否已登录。
    ///
    /// 这条接口**未登录时也返回 `code 0`**，只是 `isLogin` 为 false——
    /// 「账号失效」因此不是一个错误码，而是这个字段。基线的两个调用点
    /// （`MineController.queryUserInfo`、`LoginUtils.onLoginMain`）都靠它
    /// 决定要不要把账号删掉，所以它不能跟着别的无人读的字段一起裁掉。
    pub is_login: Option<bool>,
    pub mid: Option<i64>,
    pub uname: Option<String>,
    pub face: Option<String>,
    /// 硬币数。基线存成 `double`——服务端给的就是小数（投币有 0.5 的情形）。
    pub money: Option<f64>,
    pub level_info: Option<LevelInfo>,
    /// 大会员状态，`> 0` 为开通。
    pub vip_status: Option<i64>,
    /// `2` 是年度大会员，昵称要染色。
    pub vip_type: Option<i64>,
    /// `1` 为硬核会员。
    pub is_senior_member: Option<i64>,
}

/// 等级与经验。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct LevelInfo {
    pub current_level: Option<i64>,
    pub current_min: Option<i64>,
    pub current_exp: Option<i64>,
    /// 升下一级所需经验。
    ///
    /// **满级（6 级）时取 `current_exp`**，不是服务端给的 `next_exp`——
    /// 满级之后 `next_exp` 是一个没有意义的值，而界面拿它当进度条分母。
    /// 这条规则来自基线 `LevelInfo.fromJson`，是「协议没规定、但两端必须
    /// 一致」的那一类，所以跟着实体走而不是留在界面里。
    pub next_exp: Option<i64>,
}

/// 「我的」页三个计数。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct UserStat {
    pub following: Option<i64>,
    pub follower: Option<i64>,
    pub dynamic_count: Option<i64>,
}

/// 硬币/经验流水里的一条。两份流水共用这个形状。
///
/// 三个字段都是**服务端排好版的展示串**，不是时间戳或数值：`delta` 形如
/// `"+0.4"`，基线也是直接 `toString()` 之后显示。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct CoinLogItem {
    pub time: String,
    pub delta: String,
    pub reason: String,
}

/// 登录日志里的一条。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct LoginLogItem {
    pub ip: String,
    pub time_at: String,
    pub geo: String,
}

/// up 主实名信息。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct UserRealName {
    pub name: Option<String>,
    /// 未通过时服务端给的说明页。
    pub reject_page: Option<RejectPage>,
}

/// 实名未通过的说明页。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct RejectPage {
    pub title: Option<String>,
    pub text: Option<String>,
}

/// 空间隐私设置的一项。
///
/// ## 为什么不在这里带文案
///
/// 基线 `Privacy.fromJson` 把十五项的**中文标题**和它们的分组写死在模型里。
/// 文案按 AGENTS.md 属于可以留在宿主的那一类（i18n），但 **`key` 与
/// [`reversed`](Self::reversed) 不是**：哪个开关是反的（`disable_following`
/// 为 0 才叫「公开」）是协议语义，读错一个就是把用户的隐私设置显示反。
/// 所以这里出 key/值/反相/分组，标题留给界面查表。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct PrivacySetting {
    pub key: String,
    pub value: Option<i64>,
    /// 为真时 `value == 0` 才算「开」。
    pub reversed: bool,
    /// 界面上的第几组（1/2/3），顺序即基线的 `list1`/`list2`/`list3`。
    pub group: i64,
}

/// 订阅的收藏夹/合集，一页。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct SubList {
    pub count: Option<i64>,
    pub has_more: Option<bool>,
    pub list: Vec<SubItem>,
}

/// 订阅里的一项。`kind` 对应协议的 `type`：11 收藏夹，21 合集。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct SubItem {
    pub id: Option<i64>,
    pub fid: Option<i64>,
    pub mid: Option<i64>,
    pub attr: Option<i64>,
    pub title: Option<String>,
    pub cover: Option<String>,
    pub upper_mid: Option<i64>,
    pub upper_name: Option<String>,
    pub upper_face: Option<String>,
    pub cover_type: Option<i64>,
    pub intro: Option<String>,
    pub ctime: Option<i64>,
    pub mtime: Option<i64>,
    pub state: Option<i64>,
    pub fav_state: Option<i64>,
    pub media_count: Option<i64>,
    pub view_count: Option<i64>,
    pub kind: Option<i64>,
    pub play_count: Option<i64>,
    pub collect_count: Option<i64>,
}

/// 视频的一个标签。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct VideoTag {
    pub tag_id: Option<i64>,
    pub tag_name: Option<String>,
    /// `bgm` / `topic` / 普通标签。界面按它决定点开去哪。
    pub tag_type: Option<String>,
    pub music_id: Option<String>,
}

// ------------------------------------------------- 历史记录 / 稍后再看

/// 一页观看历史。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct HistoryPage {
    /// 顶部的业务分页签（视频/直播/专栏…）。只有首次拉取给。
    pub tabs: Vec<HistoryTab>,
    pub list: Vec<HistoryItem>,
}

/// 一个业务分页签。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct HistoryTab {
    pub kind: Option<String>,
    pub name: Option<String>,
}

/// 一条观看历史。
///
/// ## 这里没有 `label` 与 `progressIndicator`
///
/// 基线 `HistoryItemModel` 上挂着两个返回 Widget 的 getter（角标、进度条）。
/// 它们是渲染，按 AGENTS.md §2 留在宿主；判据所需的原始值
/// （[`is_fav`](Self::is_fav)、[`badge`](Self::badge)、
/// [`live_status`](Self::live_status)、[`progress`](Self::progress)）都在这里。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct HistoryItem {
    pub title: Option<String>,
    pub cover: Option<String>,
    /// 多封面（合集类条目）。`cover` 为空时取它的第一张。
    pub covers: Vec<String>,
    pub uri: Option<String>,
    pub videos: Option<i64>,
    pub author_name: Option<String>,
    pub author_mid: Option<i64>,
    /// 观看时间，Unix 秒。
    pub view_at: Option<i64>,
    /// 观看进度，**秒**。`-1` 表示已看完。
    ///
    /// 播放器要的是毫秒，且「已看完」应当从头播而不是跳到末尾——换算规则
    /// 见 [`playback_progress_ms`](Self::playback_progress_ms)。
    pub progress: Option<i64>,
    pub badge: Option<String>,
    pub show_title: Option<String>,
    pub duration: Option<i64>,
    /// `1` 表示已收藏，界面优先显示「已收藏」角标而不是 `badge`。
    pub is_fav: Option<i64>,
    /// 历史条目的 id，删除时用它。
    pub kid: Option<i64>,
    pub tag_name: Option<String>,
    /// 直播条目的开播状态，`1` 为直播中。角标颜色按它决定。
    pub live_status: Option<i64>,
    pub history: HistoryRef,
}

/// 历史条目指向的那个东西。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct HistoryRef {
    pub oid: Option<i64>,
    pub epid: Option<i64>,
    pub bvid: Option<String>,
    pub page: Option<i64>,
    /// **`0` 归一成 `None`**：服务端对没有分 P 信息的条目给 0，而 0 不是
    /// 一个合法 cid，照搬会让播放器拿它去请求。基线 `History.fromJson`
    /// 就是这么归一的。
    pub cid: Option<i64>,
    pub business: Option<String>,
}

impl HistoryItem {
    /// 续播位置，**毫秒**；没有进度时为 `None`。
    ///
    /// 两条规则都来自基线 `HistoryItemModel.playbackProgress`，是「协议没
    /// 规定、但两端必须一致」的那一类：
    ///
    /// - 历史接口以**秒**计，播放器要毫秒；
    /// - `-1`（已看完）要换成 `0`，否则重新打开会从末尾续播，而末尾的位置
    ///   来自取流账号的另一套进度，不是这条历史。
    #[must_use]
    pub fn playback_progress_ms(&self) -> Option<i64> {
        self.progress.map(|p| if p == -1 { 0 } else { p * 1000 })
    }
}

/// 一页稍后再看。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct LaterPage {
    pub count: Option<i64>,
    pub list: Vec<LaterItem>,
}

/// 一条稍后再看。
///
/// 同样不带 Widget：基线 `LaterItemModel` 的 `label` / `progressIndicator`
/// 是渲染，留在宿主；判据所需的原始值在这里。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct LaterItem {
    pub aid: Option<i64>,
    pub bvid: Option<String>,
    pub pic: Option<String>,
    pub title: Option<String>,
    /// 番剧条目去掉季名前缀之后的分集名。
    ///
    /// 这一条**是规则不是渲染**：基线用 `title.replaceFirst('${季名} ', '')`
    /// 从标题里切掉季名，切不切得掉取决于服务端两个字段的拼法，两端必须
    /// 一致。非番剧条目为 `None`。
    pub subtitle: Option<String>,
    pub pubdate: Option<i64>,
    pub duration: Option<i64>,
    pub redirect_url: Option<String>,
    /// `1` 表示联合投稿，界面显示「合作」角标。
    pub is_cooperation: Option<i64>,
    pub owner_mid: Option<i64>,
    pub owner_name: Option<String>,
    pub owner_face: Option<String>,
    pub view: Option<i64>,
    pub danmaku: Option<i64>,
    /// 番剧的季名。
    pub season_title: Option<String>,
    pub ep_id: Option<i64>,
    pub cid: Option<i64>,
    /// 观看进度，**秒**；`-1` 已看完。
    pub progress: Option<i64>,
    pub is_pgc: Option<bool>,
    /// 空串归一成 `None`——基线判的是「有没有这个角标」，空串要当没有。
    pub pgc_label: Option<String>,
    pub is_pugv: Option<bool>,
    /// 充电专属。判据是 `charging_pay.level` 存在与否，不是某个布尔字段。
    pub is_charging: bool,
    /// **已按 `rotate` 摆正**的宽高：服务端对竖屏稿件给的是原始分辨率加一个
    /// `rotate == 1`，基线 `Dimension.fromJson` 在解析时就把两者对调了。
    /// 这是「协议没规定、但两端必须一致」的那一类——谁忘了换，封面就按错
    /// 的方向占位。所以换在这里，导出面不再带 `rotate`。
    pub width: Option<i64>,
    pub height: Option<i64>,
}

/// 播放列表（medialist），一页。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct MediaList {
    pub list: Vec<MediaListItem>,
}

/// 播放列表里的一项。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct MediaListItem {
    pub aid: Option<i64>,
    pub bvid: Option<String>,
    pub title: Option<String>,
    pub intro: Option<String>,
    pub cover: Option<String>,
    pub duration: Option<i64>,
    pub badge: Option<String>,
    /// `kind` 对应协议的 `type`。
    pub kind: Option<i64>,
    pub upper_mid: Option<i64>,
    pub upper_name: Option<String>,
    pub upper_face: Option<String>,
    pub play: Option<i64>,
    pub danmaku: Option<i64>,
    /// **第一个分 P 的 cid**。基线 `MediaListItemModel.cid` 取的就是
    /// `pages.first.id`——整个 `pages` 列表别处没人读，所以这里只留这一个
    /// 值，不把列表原样搬过来。
    pub first_page_cid: Option<i64>,
}
