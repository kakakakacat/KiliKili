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

// ==================================================== 空间主页与投稿列表

/// 空间主页（`/x/v2/space` 的 `data`，挂在 app 域）。
///
/// ## 只映射有消费者的字段
///
/// 服务端给 31 个顶层键，基线 `SpaceData` 抄了 26 个，而整个 `lib/` 真正
/// 读到的只有下面这些。两处差得最远的：
///
/// - `tab`（23 个布尔开关）**一个消费者都没有**——界面用的是 `tab2`，
///   那才是服务端排好序、带标题的那份。基线的 `SpaceTab` 整个删掉。
/// - `cheese` / `ugc_season` / `series` 三块在基线里各自映射成一串
///   `Item`（31 个字段），而界面只问了「`ugc_season.count` 是几」和
///   「`series.item` 空不空」两件事。于是 `Item` 整个删掉，两件事各收成
///   一个标量。
///
/// 删掉的字段里有几个带诱惑：`play_game`、`entry`、`fans_effect`、
/// `nft_face_button`。它们在基线里也没抄，抄过来只会让下一个人以为有用。
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
pub struct SpacePage {
    /// 当前账号与他的关系。`-1` 是被对方拉黑，`-999` 是看不到。
    pub relation: Option<i64>,
    /// 对方与当前账号的关系。`relation == -999` 时界面改看这个。
    pub guest_relation: Option<i64>,
    /// `1` 表示特别关注。
    pub rel_special: Option<i64>,
    /// 粉丝勋章墙的开关值。
    pub medal: Option<i64>,
    /// 默认落在哪个 tab 上，取值与 [`SpaceTab2::param`] 对齐。
    ///
    /// 基线在控制器里把 `video` 就地改写成 `contribute` 再去找下标——
    /// FRB 结构不可变，这个改写移到了控制器的局部变量上。
    pub default_tab: Option<String>,
    /// 八个内容块里还有没有东西。
    ///
    /// 基线在 `SpaceData.fromJson` 末尾算这个值，用来决定「主页」tab 要不
    /// 要整个去掉。算法下沉：换个宿主框架它也得一模一样。
    pub has_item: bool,
    /// 有没有合集或列表。投稿页据此决定要不要多一个「全部合集/列表」tab。
    ///
    /// 基线写成 `ugcSeason.count != 0 || series.item 非空`，照搬。
    pub has_season_or_series: bool,
    /// 隐私设置。**只有看自己的空间时服务端才下发**，界面据此判断
    /// 「是不是本人」，所以 `None` 与全 0 不是一回事。
    pub setting: Option<SpaceSetting>,
    pub card: Option<SpaceCard>,
    pub images: Option<SpaceImages>,
    pub live: Option<SpaceLive>,
    pub elec: Option<SpaceElec>,
    pub guard: Option<SpaceGuard>,
    /// 投稿视频。
    pub archive: Option<SpaceVideoBlock>,
    /// 追番。
    pub season: Option<SpaceVideoBlock>,
    /// 最近投币的视频。
    pub coin_archive: Option<SpaceVideoBlock>,
    /// 最近点赞的视频。
    pub like_archive: Option<SpaceVideoBlock>,
    /// 漫画。
    pub comic: Option<SpaceVideoBlock>,
    pub article: Option<SpaceArticleBlock>,
    pub audio: Option<SpaceAudioBlock>,
    pub favourite: Option<SpaceFavBlock>,
    pub tab2: Vec<SpaceTab2>,
    pub reservations: Vec<SpaceReservation>,
}

/// 空间隐私设置。
///
/// 基线 `SpaceSetting` 有 19 个字段，这里只留主页用到的四个开关——它们
/// 决定每一块标题右边是「眼睛」还是「划掉的眼睛」。改这些值是另一条接口
/// （`RustUser.spaceSettingMod`），那条有它自己的类型。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct SpaceSetting {
    pub fav_video: Option<i64>,
    pub coins_video: Option<i64>,
    pub likes_video: Option<i64>,
    pub bangumi: Option<i64>,
}

/// 空间主页的资料卡。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct SpaceCard {
    /// 这条接口的 `card.mid` 也是**字符串**，同 [`MemberCard::mid`]。
    pub mid: Option<String>,
    pub name: Option<String>,
    pub face: Option<String>,
    pub sign: Option<String>,
    pub fans: Option<i64>,
    pub attention: Option<i64>,
    /// 累计获赞数。
    pub like_num: Option<i64>,
    /// 用户等级（0–6）。
    pub level: Option<i64>,
    /// `2` 为硬核会员，等级图标要换一张。
    pub level_identity: Option<i64>,
    /// 头像挂件图。空串表示没有。
    pub pendant_image: Option<String>,
    /// 认证类型，`0` 是个人认证、`1` 是机构认证。
    pub official_type: Option<i64>,
    pub official_desc: Option<String>,
    /// 带「bilibili UP主认证：」前缀的那一条，展开时才显示。
    pub official_splice_title: Option<String>,
    /// `> 0` 为大会员。
    pub vip_status: Option<i64>,
    /// `2` 是年度大会员，昵称要染色。
    pub vip_type: Option<i64>,
    /// 大会员标签文案（「十年大会员」之类）。缺省时界面写「大会员」。
    pub vip_label_text: Option<String>,
    /// 封禁状态，`1` 为封禁中。
    pub silence: Option<i64>,
    pub relation: Option<SpaceRelation>,
    /// 他戴着的直播粉丝勋章。
    pub live_medal: Option<SpaceLiveMedal>,
    /// 空间页的标签。基线只取 `location` 与 `real_name` 两类，其余丢掉
    /// ——**过滤规则下沉到这里**，不是界面的事。
    pub tags: Vec<SpaceCardTag>,
    /// 推广信息。
    pub pr_info: Option<SpacePrInfo>,
    /// 「你关注的人也关注了 TA」。
    pub followed_by: Option<SpaceFollowedBy>,
}

/// 关注关系三元组。
///
/// 三个字段都来自同一个 `relation` 对象，但**服务端经常只给其中一个**
/// （看别人时只有 `status`）。控制器把三者拼成一个 `relation` 值，拼法
/// 照抄基线。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct SpaceRelation {
    pub status: Option<i64>,
    pub is_follow: Option<i64>,
    pub is_followed: Option<i64>,
}

/// 直播粉丝勋章（`card.live_fans_wearing.detail_v2`）。
///
/// 只留界面画得出勋章所需的四个。颜色是 `#RRGGBB` 字符串，解析留在宿主
/// ——那是 Flutter 的 `Color` 的事，换宿主要重写的只有解析那一行。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct SpaceLiveMedal {
    pub name: Option<String>,
    pub level: Option<i64>,
    /// 勋章底色。
    pub color: Option<String>,
    /// 勋章上文字的颜色。
    pub name_color: Option<String>,
}

/// 空间页标签（常住地、实名认证）。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct SpaceCardTag {
    pub title: Option<String>,
    /// 有值时整条可点。
    pub uri: Option<String>,
}

/// 推广信息（`card.pr_info`）。
///
/// 基线的 `SpacePrInfo.fromJson` 只有在 `content` 非空时才去读其余字段，
/// 颜色各有一个默认值。这里把那个「content 为空就当整块不存在」的判断
/// 下沉成 `Option<SpacePrInfo>`：服务端在没有推广时给的是 `{}`，映射成
/// 一个字段全空的结构只会让界面再判一次。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct SpacePrInfo {
    pub content: Option<String>,
    pub url: Option<String>,
    pub icon: Option<String>,
    pub icon_night: Option<String>,
    /// 下面四个颜色**带默认值**，取自基线的字面量。
    pub text_color: String,
    pub bg_color: String,
    pub text_color_night: String,
    pub bg_color_night: String,
}

/// 「你关注的人也关注了 TA」。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct SpaceFollowedBy {
    pub items: Vec<SpaceSimpleUser>,
}

/// 一个只有昵称与头像的人。充电榜、大航海、共同关注共用。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct SpaceSimpleUser {
    pub mid: Option<i64>,
    pub name: Option<String>,
    pub face: Option<String>,
}

/// 空间头图与置顶合集的封面轮播（`images`）。
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
pub struct SpaceImages {
    /// 亮色头图。
    pub img_url: Option<String>,
    /// 暗色头图。为空时界面回落到 [`Self::img_url`]。
    pub night_img_url: Option<String>,
    /// 置顶合集的封面。非空时整个头图位置换成这组图。
    pub top_images: Vec<SpaceTopImage>,
}

/// 置顶合集的一张封面。
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
pub struct SpaceTopImage {
    /// 列表里显示的那张（`item.image.default_image`，缺省回落到 `cover`）。
    pub header: String,
    /// 点开大图时用的原图。
    pub full_cover: String,
    /// 纵向裁切位置，`-1.0`～`1.0`，直接喂给对齐参数。
    ///
    /// 服务端给的是 `location`（形如 `0-120-360-...`）与 `height`，要算一下
    /// 才能用。**这段算术下沉到这里**：它不是排版，是对服务端约定的解读，
    /// 换个宿主框架照样得算一遍。算不出来时按 `0.0`（居中）。
    pub dy: f64,
    pub title: Option<SpaceTopTitle>,
}

/// 置顶合集封面上的标题。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct SpaceTopTitle {
    pub title: Option<String>,
    pub sub_title: Option<String>,
    /// 副标题的渐变色。
    pub sub_title_colors: Vec<String>,
}

/// 直播间状态（`live`）。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct SpaceLive {
    /// `1` 表示他有直播间。
    pub room_status: Option<i64>,
    /// `1` 表示正在直播。
    pub live_status: Option<i64>,
    pub room_id: Option<i64>,
}

/// 充电榜（`elec`）。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct SpaceElec {
    /// 充电总人数。
    pub total: Option<i64>,
    pub list: Vec<SpaceSimpleUser>,
}

/// 大航海（`guard`）。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct SpaceGuard {
    /// 人数。服务端只给一句「2人加入大航海」，数字得从里面抠出来——
    /// **这个抠字符串的规则下沉到这里**，基线是在模型的构造函数里跑正则。
    pub count: Option<String>,
    pub items: Vec<SpaceSimpleUser>,
}

/// 主页上的一块视频（投稿 / 追番 / 投币 / 点赞 / 漫画共用）。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct SpaceVideoBlock {
    /// 这一块一共有多少条。界面显示在标题右边。
    pub count: Option<i64>,
    pub items: Vec<SpaceArchiveItem>,
}

/// 主页上的一块专栏。
///
/// 条目类型沿用 [`crate::space::SpaceArticleItem`]：专栏那条独立接口
/// （`/x/v2/space/article`）还没迁，它与这里产出的是同一个界面组件。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct SpaceArticleBlock {
    pub count: Option<i64>,
    pub items: Vec<SpaceArticleItem>,
}

/// 主页上的一条专栏。
///
/// 只有主页那个组件读到的五个字段。专栏页自己那条接口给的字段多得多，
/// 迁它的时候再说（见 issue #56 的批次 5）。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct SpaceArticleItem {
    pub title: Option<String>,
    /// 封面图。取 `origin_image_urls` 的第一张。
    pub cover: Option<String>,
    pub publish_time_text: Option<String>,
    pub view: Option<i64>,
    pub reply: Option<i64>,
    /// 点开后跳哪里。
    pub uri: Option<String>,
}

/// 主页上的一块音频。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct SpaceAudioBlock {
    pub count: Option<i64>,
    pub items: Vec<SpaceAudioItem>,
}

/// 主页上的一条音频。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct SpaceAudioItem {
    /// 音频 id（`sid`），播放页要它。
    pub id: Option<i64>,
    /// 上传者 mid。
    pub uid: Option<i64>,
    pub title: Option<String>,
    pub cover: Option<String>,
    /// 发布时间，**Unix 秒**。
    ///
    /// 音频页那条接口给的是毫秒，两边的组件是同一个，所以它按「有没有
    /// `statistic` 块」来决定除不除 1000。这里不碰那个判断——单位是服务端
    /// 的事实，统一成一种反而要改另一条还没迁的路径。
    pub ctime: Option<i64>,
}

/// 主页上的一块收藏夹。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct SpaceFavBlock {
    pub count: Option<i64>,
    pub items: Vec<SpaceFavItem>,
}

/// 主页上的一个收藏夹。
///
/// 条目仍落在基线的 `SpaceFavItemModel` 上：收藏夹那条独立接口
/// （`/x/v3/fav/folder/space`）不在本切片里，两边是同一个界面组件。
/// 只映射那个组件读到的五个字段。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct SpaceFavItem {
    /// 路由用的是 `media_id`，不是 `id`。
    pub media_id: Option<i64>,
    pub title: Option<String>,
    pub cover: Option<String>,
    /// 夹子里有多少条内容。
    pub count: Option<i64>,
    /// `1` 为私密。基线把判断写成 `isPublic == 1 ? '私密' : '公开'`
    /// ——键名与含义是反的，这是服务端的事实，照抄。
    pub is_public: Option<i64>,
}

/// 空间的 tab 结构（`tab2`）。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct SpaceTab2 {
    pub title: Option<String>,
    /// 路由用的标识（`home` / `dynamic` / `contribute` / `favorite`…）。
    pub param: Option<String>,
    pub items: Vec<SpaceTab2Item>,
}

/// 二级 tab。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct SpaceTab2Item {
    pub title: Option<String>,
    pub param: Option<String>,
    /// 图文页的筛选项。
    pub filters: Vec<SpaceTabFilter>,
    pub season_id: Option<i64>,
    pub series_id: Option<i64>,
}

/// 图文页的一个筛选项。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct SpaceTabFilter {
    pub text: Option<String>,
    /// 发给服务端的那个值。服务端省掉时按 `all`，默认值照抄基线。
    pub meta: String,
    pub tab_name: Option<String>,
}

/// 一条预约（`reservation_card_list`）。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct SpaceReservation {
    pub sid: Option<i64>,
    pub name: Option<String>,
    /// 已预约人数。服务端少给时按 0——这个数字要参与加减。
    pub total: i64,
    pub is_follow: bool,
    /// 副标题第一段（开播时间之类）。空串按没有。
    pub desc_text: Option<String>,
    /// 对应的动态 id，有值时多一个「看动态」按钮。
    pub dynamic_id: Option<String>,
    pub lottery: Option<SpaceReservationLottery>,
}

/// 预约附带的抽奖。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct SpaceReservationLottery {
    pub text: Option<String>,
    pub jump_url: Option<String>,
}

// -------------------------------------------------------- 投稿列表（分页）

/// 一页投稿（`/x/v2/space/archive/*` 这一族的 `data`）。
///
/// 六条接口（视频 / 充电专属 / 合集 / 列表 / 追番 / 漫画）共用这一个形状，
/// 差别只在路径与参数，所以共用一个类型，不按 tab 拆六份。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct SpaceArchivePage {
    /// 总条数。追番那条不给，界面改用本页长度。
    pub count: Option<i64>,
    pub items: Vec<SpaceArchiveItem>,
    /// 「播放全部」按钮。
    pub episodic_button: Option<SpaceEpisodicButton>,
    pub has_next: Option<bool>,
    /// 还能不能往**前**翻。从视频页跳进来定位到某一条时才有意义。
    pub has_prev: Option<bool>,
    /// 下一页的游标。`0` 表示到底了。
    pub next: Option<i64>,
}

/// 「播放全部 / 继续播放」按钮。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct SpaceEpisodicButton {
    pub text: Option<String>,
    /// 点它要跳的地址，`oid` 等参数在查询串里。
    pub uri: Option<String>,
}

/// 投稿列表里的一条。
///
/// 空间主页的五个块（投稿 / 追番 / 投币 / 点赞 / 漫画）与投稿列表那六条
/// 接口给的是同一个形状，所以只有这一个类型。
///
/// ## 没有 `owner`
///
/// 基线的 `SpaceArchiveItem` 继承 `BaseSimpleVideoItemModel`，于是必须有
/// 一个 `owner`，而服务端只给一个 `author` 字符串——基线因此造了
/// `Owner(mid: 0, name: author)` 这个**假 owner**，再在视频卡片的长按菜单
/// 里用 `videoItem is! SpaceArchiveItem` 把「访问 / 拉黑作者」两项特判掉，
/// 因为那个 mid 是假的。两处一起删：这里只给 `author`，菜单改判
/// `owner.mid == null`，对其他没有作者 mid 的列表同样成立。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct SpaceArchiveItem {
    pub title: Option<String>,
    pub cover: Option<String>,
    pub bvid: Option<String>,
    /// 首个分 P 的 cid。没有时界面要另外取一次。
    pub cid: Option<i64>,
    /// 稿件 id 或番剧 ep/season id，**字符串**：`goto` 不同时它的含义不同。
    pub param: Option<String>,
    /// `av` / `bangumi` / `comic`…，决定点进去跳哪里。
    pub goto: Option<String>,
    pub uri: Option<String>,
    /// 时长，秒。服务端不给时按 `-1`（基线如此），界面据此不显示时长角标。
    pub duration: i64,
    pub play: Option<i64>,
    pub danmaku: Option<i64>,
    /// 作者昵称。**没有 mid**，见类型文档。
    pub author: Option<String>,
    pub publish_time_text: Option<String>,
    /// 角标文案（「充电专属」之类）。
    pub badges: Vec<String>,
    pub is_steins: Option<bool>,
    pub is_cooperation: Option<bool>,
    pub is_pgc: Option<bool>,
    pub is_pugv: Option<bool>,
    /// 看到哪儿了。进度条与「已看完」都靠它。
    pub history: Option<SpaceArchiveHistory>,
    /// 合集里这一条的更新时间，Unix 秒。有它时界面用它代替发布时间文案。
    pub season_mtime: Option<i64>,
    /// 漫画的题材（「热血」之类）。
    pub styles: Option<String>,
    /// 漫画的更新状态。
    pub label: Option<String>,
}

/// 一条投稿的观看进度。
///
/// 两个字段要一起用：界面只有在 `duration > 0` 时才画进度条，
/// `progress == duration` 时写「已看完」。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct SpaceArchiveHistory {
    /// 看到的位置，秒。
    pub progress: Option<i64>,
    /// 这条视频的总时长，秒。**与 [`SpaceArchiveItem::duration`] 不是同一个
    /// 来源**：那个是列表自己的时长字段，这个来自观看记录，服务端可能只给
    /// 其中一个。
    pub duration: Option<i64>,
}
