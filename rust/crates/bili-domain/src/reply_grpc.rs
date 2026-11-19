//! 评论 gRPC 路径的领域实体（S2 续单 / issue #176）。
//!
//! ## 与 [`crate::reply`] 的关系
//!
//! 两者是**两条不同的接口**，不是一个实体的两种写法：`reply` 是 REST
//! 的 `/x/v2/reply*`（S1 / #51），本模块对应 gRPC 的
//! `bilibili.main.community.reply.v1.Reply`。REST 那条路径在迁移时只有
//! 一个消费者（shadow ban 自检只读 rpid），实体因此很瘦；gRPC 这条是
//! 评论区本身，界面要画头像挂件、粉丝牌、投票、图片、笔记跳转，实体就
//! 宽。ADR-011 补充四说的「实体宽度由消费者决定」在这里一次给出了两个
//! 不同的答案，不要把它们合并。
//!
//! ## 为什么不是 pb 形状的镜像
//!
//! ADR-012 决策五要求 prost 生成的类型止步于 `bili-grpc`。所以这里带的
//! 是界面真正读的那些字段，并且**顺手把规则算完**：
//!
//! - [`ReplyEmote::display_url`]：基线 `webpUrl ?? gifUrl ?? url` 的回退链
//!   （AGENTS.md §2 点名的「档位/名称的回退链」）；
//! - [`ReplyUrl::is_note`]：基线按 `clickReport` 以 `{"cvid` 开头判笔记；
//! - [`ReplyControlInfo::card_labels`]：界面只读 `text_content`，这里就只
//!   给字符串，不把 `ReplyCardLabel` 整个搬过来。
//!
//! 这些不是「顺手优化」：它们是协议没规定、两端必须一致的决定，换掉宿主
//! 框架时不该再写一遍。
//!
//! ## 哪些字段会被宿主就地改写
//!
//! 点赞、点踩、置顶、翻译、楼中楼的增删都在宿主侧改对象本身（基线靠
//! protobuf 生成类的 setter）。本模块只是把这件事记在文档里：真正让
//! Dart 侧字段可写的 `#[frb(non_final)]` 标在 `bridge_flutter` 的导出
//! 结构上——领域层不认识 FRB（见 crate 文档与 RFC-0001 §2.1）。
//!
//! ## 为什么只派生 `Serialize`
//!
//! 同 [`crate::video`]：fixture 对照需要规范化序列化，但实体能被直接
//! 反序列化就会有人跳过 DTO 层。本地保存的评论（「我的评论」）因此也
//! 不是直接反序列化到实体，而是走 `bili_api::reply_grpc_store` 的 DTO。

use std::collections::HashMap;

/// 一条评论。
///
/// `like` / `count` / `replies` / `translated_content` 会被宿主就地改写：
/// 点赞改 `like`，发楼中楼改 `count` 与 `replies`，翻译改
/// `translated_content`（与 [`ReplyControlInfo::show_translation`] 配对）。
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize)]
pub struct ReplyEntity {
    pub id: i64,
    pub oid: i64,
    /// 评论区类型（1 = 视频稿件，12 = 专栏，17 = 动态……）。基线叫 `type`。
    pub kind: i64,
    pub mid: i64,
    /// 根评论的 rpid；0 表示它自己就是根评论。
    pub root: i64,
    pub parent: i64,
    pub dialog: i64,
    /// Unix 秒。
    pub ctime: i64,
    pub like: i64,
    /// 楼中楼总数（含未下发的）。
    pub count: i64,
    pub content: ReplyContent,
    /// 翻译结果。`TranslateReply` 返回后由宿主填进来。
    pub translated_content: Option<ReplyContent>,
    pub member: ReplyMember,
    /// 装扮卡（`member_v2.garb`）。没有装扮时为 `None`。
    pub card: Option<ReplyCard>,
    pub control: ReplyControlInfo,
    /// 被回复者。线程化视图合成占位父评论时要用它认人。
    pub parent_member: Option<ReplyParentMember>,
    /// 根评论下带出来的前几条楼中楼。
    pub replies: Vec<ReplyEntity>,
}

/// 评论正文及其可点元素。
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize)]
pub struct ReplyContent {
    pub message: String,
    /// 占位文本（`[doge]`）→ 表情。
    pub emotes: HashMap<String, ReplyEmote>,
    /// 话题名，不带两侧的 `#`。基线是 `map<string, Topic>` 而只用到键。
    pub topics: Vec<String>,
    /// `@` 后面的昵称 → mid。
    pub at_name_to_mid: HashMap<String, i64>,
    /// 正文里出现的字面量 → 跳转信息。
    pub urls: HashMap<String, ReplyUrl>,
    pub vote: Option<ReplyVote>,
    pub pictures: Vec<ReplyPicture>,
    /// 笔记评论的跳转目标。两个字段都可能为空（此时只显示「[笔记]」不可点）。
    pub note: Option<ReplyNote>,
    /// 昵称 → 成员。线程化视图按它把 `parent_member` 的名字还原成一个人。
    pub members: HashMap<String, ReplyMember>,
}

/// 一个表情。
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize)]
pub struct ReplyEmote {
    /// 1 为小尺寸，界面按它乘一个基准边长。
    pub size: i64,
    /// 实际要加载的那一个。回退链 `webp → gif → 原图` 在投影时就算完了。
    pub display_url: String,
    /// 点开表情后的跳转，没有则为 `None`。
    pub jump_url: Option<String>,
}

/// 正文里一个可点的链接。
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize)]
pub struct ReplyUrl {
    pub title: String,
    pub prefix_icon: Option<String>,
    /// 空串表示服务端没给 scheme，宿主按字面量自己猜跳哪。
    pub app_url_schema: String,
    /// 词条搜索（设置里可关）。
    pub is_word_search: bool,
    /// 笔记链接。基线按 `click_report` 以 `{"cvid` 开头判定，显示成
    /// 「[笔记]」并挪到正文最前面。
    pub is_note: bool,
}

/// 正文里引用的投票。
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize)]
pub struct ReplyVote {
    pub id: i64,
    pub title: String,
}

/// 评论配图。
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize)]
pub struct ReplyPicture {
    pub img_src: String,
    pub img_width: f64,
    pub img_height: f64,
    pub img_size: f64,
}

/// 笔记评论的跳转目标。
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize)]
pub struct ReplyNote {
    pub click_url: Option<String>,
    pub opus_id: Option<i64>,
}

/// 评论的发布者。
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize)]
pub struct ReplyMember {
    pub mid: i64,
    pub name: String,
    pub face: String,
    pub level: i64,
    /// 基线是 `is_senior_member == 1`，这里直接给布尔。
    pub is_senior: bool,
    pub vip_status: i64,
    pub vip_type: i64,
    /// 认证类型。没有认证时为 `None`（0 是一种认证，不能拿它当「没有」）。
    pub official_verify_type: Option<i64>,
    pub pendant_image: Option<String>,
    pub medal: Option<ReplyMedal>,
}

/// 粉丝牌。
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize)]
pub struct ReplyMedal {
    pub name: String,
    pub level: i64,
    /// 十进制整数形式的颜色，宿主自己转成平台的颜色类型。
    pub color: i64,
    pub color_name: i64,
}

/// 头像右侧的装扮卡。
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize)]
pub struct ReplyCard {
    pub image: String,
    /// 粉丝编号。没有编号的装扮卡为 `None`。
    pub number: Option<String>,
    pub fan_num_prefix: String,
    pub fan_color: String,
}

/// 被回复者。古早评论可能只有昵称没有 mid。
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize)]
pub struct ReplyParentMember {
    pub mid: Option<i64>,
    pub name: String,
}

/// 一条评论的交互状态。
///
/// `action` / `is_up_top` / `show_translation` 会被宿主就地改写。
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize)]
pub struct ReplyControlInfo {
    /// 当前账号的态度：0 无、1 赞、2 踩。
    pub action: i64,
    pub is_up_top: bool,
    pub show_translation: bool,
    /// 楼中楼里有 UP 主参与。
    pub up_reply: bool,
    pub is_note: bool,
    pub is_note_v2: bool,
    /// IP 属地，形如「IP属地：上海」。没有则为 `None`。
    pub location: Option<String>,
    /// 是否给这条评论显示翻译按钮。基线判的是
    /// `TRANSLATION_SWITCH_SHOW_TRANSLATION`。
    pub can_translate: bool,
    /// 卡片标签的文案。界面只读文案，不读图标与跳转。
    pub card_labels: Vec<String>,
    pub vote_option: Option<ReplyVoteOption>,
}

/// 评论参与的投票选项（红蓝方那种）。
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize)]
pub struct ReplyVoteOption {
    /// 0 无、1 红方、2 蓝方。
    pub label_kind: i64,
    pub desc: String,
}

/// 评论区本身的状态（标题、总数、能否发评论）。
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize)]
pub struct ReplySubject {
    pub up_mid: i64,
    pub count: i64,
    pub title: String,
    pub show_title: bool,
    pub input_disable: bool,
    /// 输入框被禁用时要弹的提示；没有则为 `None`。
    pub root_text: Option<String>,
}

/// 置顶投票卡（评论区顶部那张）。
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize)]
pub struct ReplyVoteCard {
    pub vote_id: i64,
    pub title: String,
    pub count: i64,
}

/// 折叠评论的入口卡。
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize)]
pub struct ReplyFoldCard {
    pub bottom_text: String,
    /// 原样回填给 `FoldList` 的分页游标。宿主不解释它的内容。
    pub pagination_offset: String,
}

/// 一页根评论（`MainList`）。
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize)]
pub struct ReplyMainPage {
    pub replies: Vec<ReplyEntity>,
    /// UP 置顶。已经并进 [`Self::replies`] 的第一条，这里只标记有没有。
    pub has_up_top: bool,
    pub subject: ReplySubject,
    pub vote_card: Option<ReplyVoteCard>,
    pub fold_card: Option<ReplyFoldCard>,
    /// 这一页下发了混合卡。
    ///
    /// 用来区分「这一页说没有折叠卡」与「这一页根本没提折叠卡」：基线
    /// 只在前者清掉折叠入口，后者沿用上一页的（`getFoldCard` 的
    /// `if (mixedCards.isNotEmpty)`）。跨页的状态在宿主手里，所以这条
    /// 规则留在宿主，Rust 只把事实带过去。
    pub has_mixed_cards: bool,
    /// 下一页的 offset。为空表示没有下一页。
    pub next_offset: String,
    pub is_end: bool,
}

/// 一页楼中楼（`DetailList`）。
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize)]
pub struct ReplyDetailPage {
    /// 这一楼的根评论。
    pub root: ReplyEntity,
    pub replies: Vec<ReplyEntity>,
    pub subject: ReplySubject,
    pub fold_card: Option<ReplyFoldCard>,
    /// 同 [`ReplyMainPage::has_mixed_cards`]。
    pub has_mixed_cards: bool,
    /// 被过滤规则删掉的那些。线程化视图要用它们占位，否则子树会断。
    pub removed: Vec<ReplyEntity>,
    pub next_offset: String,
    pub is_end: bool,
}

/// 一页对话（`DialogList`）或一页折叠评论（`FoldList`）。
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize)]
pub struct ReplyFlatPage {
    pub replies: Vec<ReplyEntity>,
    pub subject: ReplySubject,
    /// `FoldList` 成功后要 toast 的标题；没有则为 `None`。
    pub title: Option<String>,
    pub next_offset: String,
    pub is_end: bool,
}

/// 评论区搜索出来的一条稿件 / 专栏。
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize)]
pub struct ReplySearchItem {
    /// 选中后回填进输入框的链接。
    pub url: String,
    pub title: String,
    pub cover: String,
    pub up_nickname: Option<String>,
    /// 番剧分区名，仅 PGC 有。
    pub category: Option<String>,
    /// 秒；仅 UGC 有。
    pub duration: Option<i64>,
}

/// 一页搜索结果。
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize)]
pub struct ReplySearchPage {
    pub items: Vec<ReplySearchItem>,
    pub has_next: bool,
}
