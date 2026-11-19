//! 评论区 gRPC 取数的 Dart 导出（S2 续单 / issue #176）。
//!
//! 基线 `lib/grpc/reply.dart` 的六条方法全在这里，外加「我的评论」的存档
//! 读写（基线靠 protobuf 生成类做，pb 子树删掉后没人实现了）。
//!
//! ## 为什么这里又抄了一遍实体
//!
//! `bili-domain` 不认识 flutter_rust_bridge（RFC-0001 §2.1，crate 文档里
//! 写死的那条）。所以导出面上的结构是领域实体的**镜像**，由本模块翻译。
//! 这是 Stage 1 的成本，Phase 2 连同 FRB 一起消失。
//!
//! ## 哪些字段在 Dart 侧可写
//!
//! 标了 `#[frb(non_final)]` 的**字段**生成出来的 Dart 字段非 final
//! （这个属性作用在字段上，标到结构上不报错也不生效）。这不是图方便：点赞、点踩、置顶、
//! 翻译、发楼中楼在基线里都是就地改 protobuf 对象，列表项靠
//! `markNeedsBuild` 重建。改成「每次 copyWith 一个新对象」要连带改掉
//! 列表的身份判定（`reverseIdentityIndexOf` 按 `identical` 找元素），
//! 那是另一件事，不该夹在取数迁移里做。
//!
//! 没标的字段是只读的。挨个补上「以防万一」会把「哪些状态会被宿主改」
//! 这条信息从类型上抹掉——而那正是下一个人最需要的那条信息。
//!
//! ## 过滤规则编不过时整条请求失败
//!
//! `error_kind == "unsupported"`。宿主必须把它当成一次加载失败，不能当成
//! 「没有要过滤的」继续——理由见 ADR-010 决策二，与 `RustVideoFeed` 同。

use bili_api::grpc_reply::{self, ReplyMode, ReplySearchKind};
use bili_api::reply_grpc_store;
use bili_domain::error::{DomainError, ErrorKind};
use bili_domain::reply_filter::{self, ReplyFilter};
use bili_domain::reply_grpc as domain;
use flutter_rust_bridge::frb;

use crate::api::grpc::GrpcContext;
use crate::api::init;

// ─────────────────────────── 导出的实体 ───────────────────────────

/// 一条评论。
///
/// `like` / `count` / `replies` / `translated_content` 由宿主就地改写。
#[derive(Debug, Clone, PartialEq, Default)]
// 结构上这个 `#[frb]` 不是装饰：字段上的 `#[frb(non_final)]` 是属性宏，
// 只有外层的宏展开时才会把它们吃掉，否则 rustc 直接报「不是非宏属性」。
#[frb]
pub struct ReplyEntity {
    pub id: i64,
    pub oid: i64,
    /// 评论区类型（1 = 视频稿件，12 = 专栏，17 = 动态……）。
    pub kind: i64,
    pub mid: i64,
    /// 根评论的 rpid；0 表示它自己就是根评论。
    pub root: i64,
    pub parent: i64,
    pub dialog: i64,
    /// Unix 秒。
    pub ctime: i64,
    #[frb(non_final)]
    pub like: i64,
    #[frb(non_final)]
    pub count: i64,
    pub content: ReplyContent,
    #[frb(non_final)]
    pub translated_content: Option<ReplyContent>,
    pub member: ReplyUser,
    pub card: Option<ReplyCard>,
    pub control: ReplyControlInfo,
    pub parent_member: Option<ReplyParentMember>,
    #[frb(non_final)]
    pub replies: Vec<ReplyEntity>,
}

/// 评论正文及其可点元素。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ReplyContent {
    pub message: String,
    /// 占位文本（`[doge]`）→ 表情。
    pub emotes: std::collections::HashMap<String, ReplyEmote>,
    /// 话题名，不带两侧的 `#`。
    pub topics: Vec<String>,
    pub at_name_to_mid: std::collections::HashMap<String, i64>,
    pub urls: std::collections::HashMap<String, ReplyUrl>,
    pub vote: Option<ReplyVote>,
    pub pictures: Vec<ReplyPicture>,
    pub note: Option<ReplyNote>,
    /// 昵称 → 成员。线程化视图合成占位父评论时用它认人。
    pub members: std::collections::HashMap<String, ReplyUser>,
}

/// 一个表情。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ReplyEmote {
    /// 1 为小尺寸，界面按它乘一个基准边长。
    pub size: i64,
    /// 实际要加载的那一个（`webp → gif → 原图` 的回退链已经在 Rust 算完）。
    pub display_url: String,
    pub jump_url: Option<String>,
}

/// 正文里一个可点的链接。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ReplyUrl {
    pub title: String,
    pub prefix_icon: Option<String>,
    /// 空串表示服务端没给 scheme，宿主按字面量自己猜跳哪。
    pub app_url_schema: String,
    pub is_word_search: bool,
    /// 笔记链接，显示成「[笔记]」并挪到正文最前面。
    pub is_note: bool,
}

/// 正文里引用的投票。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ReplyVote {
    pub id: i64,
    pub title: String,
}

/// 评论配图。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ReplyPicture {
    pub img_src: String,
    pub img_width: f64,
    pub img_height: f64,
    pub img_size: f64,
}

/// 笔记评论的跳转目标。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ReplyNote {
    pub click_url: Option<String>,
    pub opus_id: Option<i64>,
}

/// 评论的发布者。
///
/// 名字里不叫 `ReplyMember` 是为了避开 `api::reply` 的同名结构：FRB 的
/// 导出面按**类型名**而不是模块路径去重，重名时它「随便挑一个」，而那两个
/// 结构的字段并不一样。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ReplyUser {
    pub mid: i64,
    pub name: String,
    pub face: String,
    pub level: i64,
    pub is_senior: bool,
    pub vip_status: i64,
    pub vip_type: i64,
    /// 没有认证时为 `None`（0 是一种认证，不能拿它当「没有」）。
    pub official_verify_type: Option<i64>,
    pub pendant_image: Option<String>,
    pub medal: Option<ReplyMedal>,
}

/// 粉丝牌。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ReplyMedal {
    pub name: String,
    pub level: i64,
    /// 十进制整数形式的颜色。
    pub color: i64,
    pub color_name: i64,
}

/// 头像右侧的装扮卡。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ReplyCard {
    pub image: String,
    pub number: Option<String>,
    pub fan_num_prefix: String,
    pub fan_color: String,
}

/// 被回复者。古早评论可能只有昵称没有 mid。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ReplyParentMember {
    pub mid: Option<i64>,
    pub name: String,
}

/// 一条评论的交互状态。
///
/// `action` / `is_up_top` / `show_translation` 由宿主就地改写。
#[derive(Debug, Clone, PartialEq, Default)]
// 结构上这个 `#[frb]` 不是装饰：字段上的 `#[frb(non_final)]` 是属性宏，
// 只有外层的宏展开时才会把它们吃掉，否则 rustc 直接报「不是非宏属性」。
#[frb]
pub struct ReplyControlInfo {
    /// 当前账号的态度：0 无、1 赞、2 踩。
    #[frb(non_final)]
    pub action: i64,
    #[frb(non_final)]
    pub is_up_top: bool,
    #[frb(non_final)]
    pub show_translation: bool,
    /// 楼中楼里有 UP 主参与。
    pub up_reply: bool,
    pub is_note: bool,
    pub is_note_v2: bool,
    pub location: Option<String>,
    /// 这条评论要不要显示翻译按钮。
    pub can_translate: bool,
    pub card_labels: Vec<String>,
    pub vote_option: Option<ReplyVoteOption>,
}

/// 评论参与的投票选项（红蓝方那种）。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ReplyVoteOption {
    /// 0 无、1 红方、2 蓝方。
    pub label_kind: i64,
    pub desc: String,
}

/// 评论区本身的状态。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ReplySubject {
    pub up_mid: i64,
    pub count: i64,
    pub title: String,
    pub show_title: bool,
    pub input_disable: bool,
    pub root_text: Option<String>,
}

/// 置顶投票卡。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ReplyVoteCard {
    pub vote_id: i64,
    pub title: String,
    pub count: i64,
}

/// 折叠评论的入口卡。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ReplyFoldCard {
    pub bottom_text: String,
    /// 原样回填给 `foldList` 的游标，宿主不解释它的内容。
    pub pagination_offset: String,
}

/// 一页根评论。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ReplyMainPage {
    pub replies: Vec<ReplyEntity>,
    pub has_up_top: bool,
    pub subject: ReplySubject,
    pub vote_card: Option<ReplyVoteCard>,
    pub fold_card: Option<ReplyFoldCard>,
    /// 这一页下发了混合卡。为 false 时宿主应当**沿用上一页的折叠入口**，
    /// 而不是把它清掉——基线 `getFoldCard` 就是这么写的。
    pub has_mixed_cards: bool,
    pub next_offset: String,
    pub is_end: bool,
}

/// 一页楼中楼。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ReplyDetailPage {
    pub root: ReplyEntity,
    pub replies: Vec<ReplyEntity>,
    pub subject: ReplySubject,
    pub fold_card: Option<ReplyFoldCard>,
    /// 同 [`ReplyMainPage::has_mixed_cards`]。
    pub has_mixed_cards: bool,
    /// 被过滤规则删掉的那些；线程化视图要用它们占位。
    pub removed: Vec<ReplyEntity>,
    pub next_offset: String,
    pub is_end: bool,
}

/// 一页对话或一页折叠评论。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ReplyFlatPage {
    pub replies: Vec<ReplyEntity>,
    pub subject: ReplySubject,
    /// 折叠列表成功后要 toast 的标题。
    pub title: Option<String>,
    pub next_offset: String,
    pub is_end: bool,
}

/// 评论区搜索出来的一条稿件 / 专栏。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ReplySearchItem {
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
///
/// 名字里不叫 `ReplySearchPage` 是为了避开宿主自己的
/// `lib/pages/video/reply_search_item/view.dart` 里那个同名的页面 widget：
/// Dart 没有命名空间，两者会直接撞上。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ReplySearchList {
    pub items: Vec<ReplySearchItem>,
    pub has_next: bool,
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

result_type!(ReplyMainPageResult, ReplyMainPage);
result_type!(ReplyDetailPageResult, ReplyDetailPage);
result_type!(ReplyFlatPageResult, ReplyFlatPage);
result_type!(ReplySearchListResult, ReplySearchList);
result_type!(ReplyTranslationResult, ReplyContent);
result_type!(ReplyEntityResult, ReplyEntity);
result_type!(ReplyBytesResult, Vec<u8>);
result_type!(ReplyTextResult, String);
result_type!(ReplyImportResult, Vec<ReplyImportEntry>);

/// 导入出来的一条：盒子的键与要写进去的字节。
#[derive(Debug, Clone)]
pub struct ReplyImportEntry {
    pub key: String,
    pub bytes: Vec<u8>,
}

// ─────────────────────────── 领域 → 导出 ───────────────────────────

fn member_from(member: domain::ReplyMember) -> ReplyUser {
    ReplyUser {
        mid: member.mid,
        name: member.name,
        face: member.face,
        level: member.level,
        is_senior: member.is_senior,
        vip_status: member.vip_status,
        vip_type: member.vip_type,
        official_verify_type: member.official_verify_type,
        pendant_image: member.pendant_image,
        medal: member.medal.map(|m| ReplyMedal {
            name: m.name,
            level: m.level,
            color: m.color,
            color_name: m.color_name,
        }),
    }
}

fn content_from(content: domain::ReplyContent) -> ReplyContent {
    ReplyContent {
        message: content.message,
        emotes: content
            .emotes
            .into_iter()
            .map(|(key, emote)| {
                (
                    key,
                    ReplyEmote {
                        size: emote.size,
                        display_url: emote.display_url,
                        jump_url: emote.jump_url,
                    },
                )
            })
            .collect(),
        topics: content.topics,
        at_name_to_mid: content.at_name_to_mid,
        urls: content
            .urls
            .into_iter()
            .map(|(key, url)| {
                (
                    key,
                    ReplyUrl {
                        title: url.title,
                        prefix_icon: url.prefix_icon,
                        app_url_schema: url.app_url_schema,
                        is_word_search: url.is_word_search,
                        is_note: url.is_note,
                    },
                )
            })
            .collect(),
        vote: content.vote.map(|v| ReplyVote {
            id: v.id,
            title: v.title,
        }),
        pictures: content
            .pictures
            .into_iter()
            .map(|p| ReplyPicture {
                img_src: p.img_src,
                img_width: p.img_width,
                img_height: p.img_height,
                img_size: p.img_size,
            })
            .collect(),
        note: content.note.map(|n| ReplyNote {
            click_url: n.click_url,
            opus_id: n.opus_id,
        }),
        members: content
            .members
            .into_iter()
            .map(|(key, member)| (key, member_from(member)))
            .collect(),
    }
}

fn control_from(control: domain::ReplyControlInfo) -> ReplyControlInfo {
    ReplyControlInfo {
        action: control.action,
        is_up_top: control.is_up_top,
        show_translation: control.show_translation,
        up_reply: control.up_reply,
        is_note: control.is_note,
        is_note_v2: control.is_note_v2,
        location: control.location,
        can_translate: control.can_translate,
        card_labels: control.card_labels,
        vote_option: control.vote_option.map(|o| ReplyVoteOption {
            label_kind: o.label_kind,
            desc: o.desc,
        }),
    }
}

fn entity_from(entity: domain::ReplyEntity) -> ReplyEntity {
    ReplyEntity {
        id: entity.id,
        oid: entity.oid,
        kind: entity.kind,
        mid: entity.mid,
        root: entity.root,
        parent: entity.parent,
        dialog: entity.dialog,
        ctime: entity.ctime,
        like: entity.like,
        count: entity.count,
        content: content_from(entity.content),
        translated_content: entity.translated_content.map(content_from),
        member: member_from(entity.member),
        card: entity.card.map(|c| ReplyCard {
            image: c.image,
            number: c.number,
            fan_num_prefix: c.fan_num_prefix,
            fan_color: c.fan_color,
        }),
        control: control_from(entity.control),
        parent_member: entity.parent_member.map(|m| ReplyParentMember {
            mid: m.mid,
            name: m.name,
        }),
        replies: entity.replies.into_iter().map(entity_from).collect(),
    }
}

fn subject_from(subject: domain::ReplySubject) -> ReplySubject {
    ReplySubject {
        up_mid: subject.up_mid,
        count: subject.count,
        title: subject.title,
        show_title: subject.show_title,
        input_disable: subject.input_disable,
        root_text: subject.root_text,
    }
}

fn fold_from(card: domain::ReplyFoldCard) -> ReplyFoldCard {
    ReplyFoldCard {
        bottom_text: card.bottom_text,
        pagination_offset: card.pagination_offset,
    }
}

fn main_page_from(page: domain::ReplyMainPage) -> ReplyMainPage {
    ReplyMainPage {
        replies: page.replies.into_iter().map(entity_from).collect(),
        has_up_top: page.has_up_top,
        subject: subject_from(page.subject),
        vote_card: page.vote_card.map(|c| ReplyVoteCard {
            vote_id: c.vote_id,
            title: c.title,
            count: c.count,
        }),
        fold_card: page.fold_card.map(fold_from),
        has_mixed_cards: page.has_mixed_cards,
        next_offset: page.next_offset,
        is_end: page.is_end,
    }
}

fn detail_page_from(page: domain::ReplyDetailPage) -> ReplyDetailPage {
    ReplyDetailPage {
        root: entity_from(page.root),
        replies: page.replies.into_iter().map(entity_from).collect(),
        subject: subject_from(page.subject),
        fold_card: page.fold_card.map(fold_from),
        has_mixed_cards: page.has_mixed_cards,
        removed: page.removed.into_iter().map(entity_from).collect(),
        next_offset: page.next_offset,
        is_end: page.is_end,
    }
}

fn flat_page_from(page: domain::ReplyFlatPage) -> ReplyFlatPage {
    ReplyFlatPage {
        replies: page.replies.into_iter().map(entity_from).collect(),
        subject: subject_from(page.subject),
        title: page.title,
        next_offset: page.next_offset,
        is_end: page.is_end,
    }
}

fn search_page_from(page: domain::ReplySearchPage) -> ReplySearchList {
    ReplySearchList {
        items: page
            .items
            .into_iter()
            .map(|item| ReplySearchItem {
                url: item.url,
                title: item.title,
                cover: item.cover,
                up_nickname: item.up_nickname,
                category: item.category,
                duration: item.duration,
            })
            .collect(),
        has_next: page.has_next,
    }
}

// ─────────────────────────── 导出 → 领域 ───────────────────────────

fn member_into(member: ReplyUser) -> domain::ReplyMember {
    domain::ReplyMember {
        mid: member.mid,
        name: member.name,
        face: member.face,
        level: member.level,
        is_senior: member.is_senior,
        vip_status: member.vip_status,
        vip_type: member.vip_type,
        official_verify_type: member.official_verify_type,
        pendant_image: member.pendant_image,
        medal: member.medal.map(|m| domain::ReplyMedal {
            name: m.name,
            level: m.level,
            color: m.color,
            color_name: m.color_name,
        }),
    }
}

fn content_into(content: ReplyContent) -> domain::ReplyContent {
    domain::ReplyContent {
        message: content.message,
        emotes: content
            .emotes
            .into_iter()
            .map(|(key, emote)| {
                (
                    key,
                    domain::ReplyEmote {
                        size: emote.size,
                        display_url: emote.display_url,
                        jump_url: emote.jump_url,
                    },
                )
            })
            .collect(),
        topics: content.topics,
        at_name_to_mid: content.at_name_to_mid,
        urls: content
            .urls
            .into_iter()
            .map(|(key, url)| {
                (
                    key,
                    domain::ReplyUrl {
                        title: url.title,
                        prefix_icon: url.prefix_icon,
                        app_url_schema: url.app_url_schema,
                        is_word_search: url.is_word_search,
                        is_note: url.is_note,
                    },
                )
            })
            .collect(),
        vote: content.vote.map(|v| domain::ReplyVote {
            id: v.id,
            title: v.title,
        }),
        pictures: content
            .pictures
            .into_iter()
            .map(|p| domain::ReplyPicture {
                img_src: p.img_src,
                img_width: p.img_width,
                img_height: p.img_height,
                img_size: p.img_size,
            })
            .collect(),
        note: content.note.map(|n| domain::ReplyNote {
            click_url: n.click_url,
            opus_id: n.opus_id,
        }),
        members: content
            .members
            .into_iter()
            .map(|(key, member)| (key, member_into(member)))
            .collect(),
    }
}

fn entity_into(entity: ReplyEntity) -> domain::ReplyEntity {
    domain::ReplyEntity {
        id: entity.id,
        oid: entity.oid,
        kind: entity.kind,
        mid: entity.mid,
        root: entity.root,
        parent: entity.parent,
        dialog: entity.dialog,
        ctime: entity.ctime,
        like: entity.like,
        count: entity.count,
        content: content_into(entity.content),
        translated_content: entity.translated_content.map(content_into),
        member: member_into(entity.member),
        card: entity.card.map(|c| domain::ReplyCard {
            image: c.image,
            number: c.number,
            fan_num_prefix: c.fan_num_prefix,
            fan_color: c.fan_color,
        }),
        control: domain::ReplyControlInfo {
            action: entity.control.action,
            is_up_top: entity.control.is_up_top,
            show_translation: entity.control.show_translation,
            up_reply: entity.control.up_reply,
            is_note: entity.control.is_note,
            is_note_v2: entity.control.is_note_v2,
            location: entity.control.location,
            can_translate: entity.control.can_translate,
            card_labels: entity.control.card_labels,
            vote_option: entity.control.vote_option.map(|o| domain::ReplyVoteOption {
                label_kind: o.label_kind,
                desc: o.desc,
            }),
        },
        parent_member: entity.parent_member.map(|m| domain::ReplyParentMember {
            mid: m.mid,
            name: m.name,
        }),
        replies: entity.replies.into_iter().map(entity_into).collect(),
    }
}

// ─────────────────────────── 上下文与过滤 ───────────────────────────

/// 过滤规则的取值。每次请求由宿主传入——用户随时能在设置页改，Rust 缓存
/// 一份必然漂移（同 `FeedFilterSettings`）。
#[derive(Debug, Clone, Default)]
pub struct ReplyFilterInput {
    pub blocked_mids: Vec<i64>,
    pub anti_goods: bool,
    pub block_at_only: bool,
    /// `None` = 不启用。
    pub block_at_count: Option<i64>,
    /// 空串 = 不启用。
    pub ban_word: String,
    /// 稿件简介。与之逐字相同的根评论会被移除；`None` / 空串 = 不启用。
    pub intro: Option<String>,
}

impl ReplyFilterInput {
    fn compile(self) -> Result<ReplyFilter, DomainError> {
        ReplyFilter::compile(&reply_filter::ReplyFilterSettings {
            blocked_mids: self.blocked_mids,
            anti_goods: self.anti_goods,
            block_at_only: self.block_at_only,
            block_at_count: self.block_at_count,
            ban_word: self.ban_word,
            intro: self.intro,
        })
    }
}

/// 根评论列表的排序。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplySortMode {
    Time,
    Hot,
}

impl ReplySortMode {
    const fn into_domain(self) -> ReplyMode {
        match self {
            Self::Time => ReplyMode::Time,
            Self::Hot => ReplyMode::Hot,
        }
    }
}

/// 评论区搜索的条目类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplySearchItemKind {
    Video,
    Article,
}

impl ReplySearchItemKind {
    const fn into_domain(self) -> ReplySearchKind {
        match self {
            Self::Video => ReplySearchKind::Video,
            Self::Article => ReplySearchKind::Article,
        }
    }
}

fn flatten<T>(
    result: Result<Result<T, DomainError>, tokio::task::JoinError>,
) -> Result<T, DomainError> {
    // JoinError 的 panic payload 可能含参数；不向宿主回传它的原始文本。
    result.unwrap_or_else(|_| Err(DomainError::new(ErrorKind::Protocol, "评论工作任务未完成")))
}

/// 空串按「首页」。宿主侧的 `nextOffset` 初值是 null，过桥时统一成空串。
fn offset_of(offset: &str) -> Option<&str> {
    Some(offset).filter(|s| !s.is_empty())
}

// ─────────────────────────── 六条取数 ───────────────────────────

/// 取一页根评论（`MainList`）。
///
/// `is_refresh` 为真时把 UP 置顶并进列表头部；翻页时置顶已经在列表里了，
/// 再并一次会出现两条。
///
/// 整页被过滤光且还有下一页时自动往下翻，与基线一致。
#[allow(clippy::too_many_arguments)]
pub async fn fetch_reply_main_list(
    context: GrpcContext,
    filter: ReplyFilterInput,
    oid: i64,
    kind: i64,
    rpid: Option<i64>,
    mode: ReplySortMode,
    offset: String,
    is_refresh: bool,
) -> ReplyMainPageResult {
    let (ctx, transport) = context.split();
    let mode = mode.into_domain();
    ReplyMainPageResult::from_result(
        flatten(
            init::runtime()
                .spawn(async move {
                    let filter = filter.compile()?;
                    grpc_reply::fetch_main_list(
                        &transport,
                        &ctx,
                        &filter,
                        oid,
                        kind,
                        rpid,
                        mode,
                        offset_of(&offset),
                        is_refresh,
                    )
                    .await
                })
                .await,
        )
        .map(main_page_from),
    )
}

/// 取一页楼中楼（`DetailList`）。
#[allow(clippy::too_many_arguments)]
pub async fn fetch_reply_detail_list(
    context: GrpcContext,
    filter: ReplyFilterInput,
    oid: i64,
    kind: i64,
    root: i64,
    rpid: i64,
    mode: ReplySortMode,
    offset: String,
) -> ReplyDetailPageResult {
    let (ctx, transport) = context.split();
    let mode = mode.into_domain();
    ReplyDetailPageResult::from_result(
        flatten(
            init::runtime()
                .spawn(async move {
                    let filter = filter.compile()?;
                    grpc_reply::fetch_detail_list(
                        &transport,
                        &ctx,
                        &filter,
                        oid,
                        kind,
                        root,
                        rpid,
                        mode,
                        offset_of(&offset),
                    )
                    .await
                })
                .await,
        )
        .map(detail_page_from),
    )
}

/// 取一页对话（`DialogList`）。
pub async fn fetch_reply_dialog_list(
    context: GrpcContext,
    filter: ReplyFilterInput,
    oid: i64,
    kind: i64,
    root: i64,
    dialog: i64,
    offset: String,
) -> ReplyFlatPageResult {
    let (ctx, transport) = context.split();
    ReplyFlatPageResult::from_result(
        flatten(
            init::runtime()
                .spawn(async move {
                    let filter = filter.compile()?;
                    grpc_reply::fetch_dialog_list(
                        &transport,
                        &ctx,
                        &filter,
                        oid,
                        kind,
                        root,
                        dialog,
                        offset_of(&offset),
                    )
                    .await
                })
                .await,
        )
        .map(flat_page_from),
    )
}

/// 取一页折叠评论（`FoldList`）。`offset` 来自折叠卡，原样回填。
pub async fn fetch_reply_fold_list(
    context: GrpcContext,
    filter: ReplyFilterInput,
    oid: i64,
    kind: i64,
    offset: String,
) -> ReplyFlatPageResult {
    let (ctx, transport) = context.split();
    ReplyFlatPageResult::from_result(
        flatten(
            init::runtime()
                .spawn(async move {
                    let filter = filter.compile()?;
                    grpc_reply::fetch_fold_list(&transport, &ctx, &filter, oid, kind, &offset).await
                })
                .await,
        )
        .map(flat_page_from),
    )
}

/// 评论区里搜稿件 / 专栏（发评论时插链接用）。不过滤。
pub async fn fetch_reply_search_item(
    context: GrpcContext,
    oid: i64,
    kind: i64,
    page: i64,
    item_kind: ReplySearchItemKind,
    keyword: String,
) -> ReplySearchListResult {
    let (ctx, transport) = context.split();
    let item_kind = item_kind.into_domain();
    ReplySearchListResult::from_result(
        flatten(
            init::runtime()
                .spawn(async move {
                    grpc_reply::fetch_search_item(
                        &transport, &ctx, oid, kind, page, item_kind, &keyword,
                    )
                    .await
                })
                .await,
        )
        .map(search_page_from),
    )
}

/// 翻译一条评论。没有译文时 `data` 为 `None` 且也没有错误——基线此时
/// toast「翻译结果为空」，那是一种正常结果，不是失败。
pub async fn fetch_reply_translation(
    context: GrpcContext,
    oid: i64,
    kind: i64,
    rpid: i64,
) -> ReplyTranslationResult {
    let (ctx, transport) = context.split();
    let result =
        flatten(
            init::runtime()
                .spawn(async move {
                    grpc_reply::fetch_translation(&transport, &ctx, oid, kind, rpid).await
                })
                .await,
        );
    match result {
        Ok(content) => ReplyTranslationResult {
            data: content.map(content_from),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(error) => ReplyTranslationResult::from_result(Err(error)),
    }
}

// ─────────────────────────── 「我的评论」存档 ───────────────────────────

/// 读一条本地保存的评论。老的 protobuf 字节也认（一次性迁移）。
#[frb(sync)]
pub fn decode_stored_reply(bytes: Vec<u8>) -> ReplyEntityResult {
    ReplyEntityResult::from_result(reply_grpc_store::decode_stored(&bytes).map(entity_from))
}

/// 写一条本地保存的评论。
///
/// `#[frb(sync)]`：调用点是长按菜单里的「保存评论」与发评论成功之后，
/// 都在一次点击的回调里，跨线程排队只会让那一下变慢。
#[frb(sync)]
pub fn encode_stored_reply(entity: ReplyEntity) -> ReplyBytesResult {
    ReplyBytesResult::from_result(reply_grpc_store::encode_stored(&entity_into(entity)))
}

/// 把整盒评论导出成 JSON 数组。
#[frb(sync)]
pub fn export_stored_replies(items: Vec<Vec<u8>>) -> ReplyTextResult {
    ReplyTextResult::from_result(reply_grpc_store::export_json(&items))
}

/// 从 JSON 数组导入。返回的 `key` 是盒子的键。
#[frb(sync)]
pub fn import_stored_replies(json: String) -> ReplyImportResult {
    ReplyImportResult::from_result(reply_grpc_store::import_json(&json).map(|items| {
        items
            .into_iter()
            .map(|(key, bytes)| ReplyImportEntry { key, bytes })
            .collect()
    }))
}

/// 把 `/x/v2/reply/add` 返回的 `data.reply` 变成一条评论。
///
/// 请求本身还在宿主侧（写操作按 ADR-013 单独迁），这里只接管字段映射。
#[frb(sync)]
pub fn reply_from_rest_json(json: String) -> ReplyEntityResult {
    ReplyEntityResult::from_result(reply_grpc_store::from_rest_reply(&json).map(entity_from))
}
