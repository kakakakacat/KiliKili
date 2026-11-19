//! 评论区的 gRPC 取数（S2 续单 / issue #176）。
//!
//! 对应基线 `lib/grpc/reply.dart` 的 `ReplyGrpc` 六条方法：`mainList`、
//! `detailList`、`foldList`、`dialogList`、`searchItem`、`translateReply`。
//!
//! ## 这里为什么用 prost 而不是手写 Reader
//!
//! 与 [`crate::grpc_space`] 相反。那边只需要读十来个字段，而且响应里嵌着
//! 一棵上百字段的 `Arc`；这边界面要吃的是整条 `ReplyInfo`（正文、表情、
//! 链接、图片、装扮、粉丝牌、投票、翻译……），ADR-012 决策四 4.1 说的
//! 「手写读取器在 97 条 message 上不成立」指的就是这棵树。
//!
//! 生成的消息止步在本模块：对外只给 [`bili_domain::reply_grpc`] 的实体
//! （决策五）。
//!
//! ## 过滤规则在 Rust 的哪一层
//!
//! 规则本身是 [`bili_domain::reply_filter`]（#52 已落地），本模块只负责
//! **在哪些响应、哪些位置上应用它**。那是基线逐条写死的，照抄：
//!
//! | 方法 | 过滤哪些 |
//! | --- | --- |
//! | `MainList` | UP 置顶及其楼中楼、根评论及其楼中楼 |
//! | `DetailList` | `root.replies`（被删掉的要带回去，线程化视图拿它占位）|
//! | `FoldList` / `DialogList` | `replies` |
//! | `SearchItem` / `TranslateReply` | 不过滤 |
//!
//! 规则编译失败（关键词正则两端语义不同或 Rust 编不过）时**整条请求
//! 失败**，不是「没有要过滤的」。理由见 ADR-010 决策二：放行意味着用户
//! 明确屏蔽过的评论照常显示且没有任何提示，比少看一页更糟。
//!
//! ## 整页被过滤光就自动翻下一页
//!
//! 基线 `mainList` 里那段递归跟着搬过来了（[`fetch_main_list`]）。它是
//! 「协议没规定、两端必须一致」的决定：留在宿主就意味着换一套 UI 要再写
//! 一遍，而写错的表现是「某些视频的评论区看起来是空的」。

use bili_domain::error::{DomainError, ErrorKind};
use bili_domain::reply_filter::{ReplyFilter, ReplyFilterItem, ReplyUrlExtra};
use bili_domain::reply_grpc::{
    ReplyCard, ReplyContent, ReplyControlInfo, ReplyDetailPage, ReplyEmote, ReplyEntity,
    ReplyFlatPage, ReplyFoldCard, ReplyMainPage, ReplyMedal, ReplyMember, ReplyNote,
    ReplyParentMember, ReplyPicture, ReplySearchItem, ReplySearchPage, ReplySubject, ReplyUrl,
    ReplyVote, ReplyVoteCard, ReplyVoteOption,
};
use bili_grpc::bilibili::main::community::reply::v1 as pb;
use bili_grpc::bilibili::pagination::FeedPagination;
use bili_protocol::grpc as protocol;
use prost::Message;

use crate::http::{HttpTransport, RequestContext, RetryPolicy};

/// 根评论列表的排序。基线 `Mode.MAIN_LIST_TIME` / `MAIN_LIST_HOT`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplyMode {
    Time,
    Hot,
}

impl ReplyMode {
    const fn to_pb(self) -> i32 {
        match self {
            Self::Time => pb::Mode::MainListTime as i32,
            Self::Hot => pb::Mode::MainListHot as i32,
        }
    }
}

/// 评论区搜索的条目类型。基线 `SearchItemType.VIDEO` / `ARTICLE`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplySearchKind {
    Video,
    Article,
}

impl ReplySearchKind {
    const fn to_pb(self) -> i32 {
        match self {
            Self::Video => pb::SearchItemType::Video as i32,
            Self::Article => pb::SearchItemType::Article as i32,
        }
    }
}

fn protocol_error(detail: impl Into<String>) -> DomainError {
    DomainError::new(ErrorKind::Protocol, detail)
}

/// 首页传 `None`，翻页传上一页的 `next_offset`。
///
/// 空串按没有：基线 `offset == null ? null : FeedPagination(offset: offset)`
/// 里 `nextOffset` 的初值就是 `null`，而翻页时它恒非空。
fn pagination(offset: Option<&str>) -> Option<FeedPagination> {
    offset
        .filter(|s| !s.is_empty())
        .map(|offset| FeedPagination {
            offset: Some(offset.to_owned()),
            ..FeedPagination::default()
        })
}

// ─────────────────────────── 请求 ───────────────────────────

#[must_use]
pub fn main_list_request(
    oid: i64,
    kind: i64,
    rpid: Option<i64>,
    mode: ReplyMode,
    offset: Option<&str>,
) -> Vec<u8> {
    pb::MainListReq {
        oid: Some(oid),
        r#type: Some(kind),
        rpid,
        mode: Some(mode.to_pb()),
        pagination: pagination(offset),
        ..pb::MainListReq::default()
    }
    .encode_to_vec()
}

#[must_use]
pub fn detail_list_request(
    oid: i64,
    kind: i64,
    root: i64,
    rpid: i64,
    mode: ReplyMode,
    offset: Option<&str>,
) -> Vec<u8> {
    pb::DetailListReq {
        oid: Some(oid),
        r#type: Some(kind),
        root: Some(root),
        rpid: Some(rpid),
        // 基线固定 `DetailListScene.REPLY`，不是默认值。
        scene: Some(pb::DetailListScene::Reply as i32),
        mode: Some(mode.to_pb()),
        pagination: pagination(offset),
        ..pb::DetailListReq::default()
    }
    .encode_to_vec()
}

/// `offset` 来自 [`ReplyFoldCard::pagination_offset`]，宿主原样回填。
#[must_use]
pub fn fold_list_request(oid: i64, kind: i64, offset: &str) -> Vec<u8> {
    pb::FoldListReq {
        oid: Some(oid),
        r#type: Some(kind),
        pagination: pagination(Some(offset)),
        ..pb::FoldListReq::default()
    }
    .encode_to_vec()
}

#[must_use]
pub fn dialog_list_request(
    oid: i64,
    kind: i64,
    root: i64,
    dialog: i64,
    offset: Option<&str>,
) -> Vec<u8> {
    pb::DialogListReq {
        oid: Some(oid),
        r#type: Some(kind),
        root: Some(root),
        dialog: Some(dialog),
        pagination: pagination(offset),
        ..pb::DialogListReq::default()
    }
    .encode_to_vec()
}

#[must_use]
pub fn search_item_request(
    oid: i64,
    kind: i64,
    page: i64,
    item_kind: ReplySearchKind,
    keyword: &str,
) -> Vec<u8> {
    pb::SearchItemReq {
        cursor: Some(pb::SearchItemCursorReq {
            next: Some(page),
            item_type: Some(item_kind.to_pb()),
        }),
        oid: Some(oid),
        r#type: Some(kind),
        keyword: Some(keyword.to_owned()),
    }
    .encode_to_vec()
}

/// 基线一次只译一条，`rpids` 里就一个元素。照抄。
#[must_use]
pub fn translate_request(oid: i64, kind: i64, rpid: i64) -> Vec<u8> {
    pb::TranslateReplyReq {
        r#type: Some(kind),
        oid: Some(oid),
        rpids: vec![rpid],
    }
    .encode_to_vec()
}

// ─────────────────────────── 投影 ───────────────────────────

fn project_emote(emote: &pb::Emote) -> ReplyEmote {
    ReplyEmote {
        size: emote.size.unwrap_or_default(),
        // 基线 `webpUrl ?? gifUrl ?? url` 的回退链。空串按「没给」处理：
        // protobuf 的 `hasWebpUrl()` 对显式空串是 true，但拿空串去加载
        // 图片与没有这个字段没有区别，而回退链的下一档是能用的。
        display_url: [&emote.webp_url, &emote.gif_url, &emote.url]
            .into_iter()
            .flatten()
            .find(|s| !s.is_empty())
            .cloned()
            .unwrap_or_default(),
        jump_url: emote.jump_url.clone().filter(|s| !s.is_empty()),
    }
}

fn project_url(url: &pb::Url) -> ReplyUrl {
    ReplyUrl {
        title: url.title.clone().unwrap_or_default(),
        prefix_icon: url.prefix_icon.clone().filter(|s| !s.is_empty()),
        app_url_schema: url.app_url_schema.clone().unwrap_or_default(),
        is_word_search: url
            .extra
            .as_ref()
            .and_then(|e| e.is_word_search)
            .unwrap_or_default(),
        // 基线判的是 `url.clickReport.startsWith('{"cvid')`：服务端没有
        // 给笔记链接单独的标志位，只能从埋点串的形状认。照抄。
        is_note: url
            .click_report
            .as_deref()
            .is_some_and(|r| r.starts_with("{\"cvid")),
    }
}

fn project_member(member: &pb::Member) -> ReplyMember {
    ReplyMember {
        mid: member.mid.unwrap_or_default(),
        name: member.name.clone().unwrap_or_default(),
        face: member.face.clone().unwrap_or_default(),
        level: member.level.unwrap_or_default(),
        is_senior: member.is_senior_member == Some(1),
        vip_status: member.vip_status.unwrap_or_default(),
        vip_type: member.vip_type.unwrap_or_default(),
        // 基线用 `hasOfficialVerifyType()` 区分「没有认证」与「认证类型 0」，
        // 0 是一种认证（个人认证）。不能折叠成 `unwrap_or(0)`。
        official_verify_type: member.official_verify_type,
        pendant_image: member.garb_pendant_image.clone().filter(|s| !s.is_empty()),
        // 基线靠 `hasFansMedalLevel()` 判有没有粉丝牌。
        medal: member.fans_medal_level.map(|level| ReplyMedal {
            name: member.fans_medal_name.clone().unwrap_or_default(),
            level,
            color: member.fans_medal_color.unwrap_or_default(),
            color_name: member.fans_medal_color_name.unwrap_or_default(),
        }),
    }
}

/// `member_v2.garb` → 头像右侧那张装扮卡。
///
/// 基线的判据是 `hasMemberV2() && memberV2.garb.hasCardImage()`，编号再
/// 单独判 `hasCardNumber()`。没有卡图就整张卡都不画，所以这里返回 `None`。
fn project_card(member_v2: Option<&pb::MemberV2>) -> Option<ReplyCard> {
    let garb = member_v2?.garb.as_ref()?;
    let image = garb.card_image.clone().filter(|s| !s.is_empty())?;
    Some(ReplyCard {
        image,
        number: garb.card_number.clone(),
        fan_num_prefix: garb.fan_num_prefix.clone().unwrap_or_default(),
        fan_color: garb.card_fan_color.clone().unwrap_or_default(),
    })
}

fn project_note(rich_text: Option<&pb::RichText>) -> Option<ReplyNote> {
    let note = match rich_text?.item.as_ref()? {
        pb::rich_text::Item::Note(note) => ReplyNote {
            click_url: note.click_url.clone().filter(|s| !s.is_empty()),
            opus_id: None,
        },
        pb::rich_text::Item::Opus(opus) => ReplyNote {
            click_url: None,
            opus_id: opus.opus_id,
        },
    };
    Some(note)
}

fn project_content(content: &pb::Content) -> ReplyContent {
    ReplyContent {
        message: content.message.clone().unwrap_or_default(),
        emotes: content
            .emotes
            .iter()
            .map(|(key, emote)| (key.clone(), project_emote(emote)))
            .collect(),
        topics: content.topics.keys().cloned().collect(),
        at_name_to_mid: content.at_name_to_mid.clone().into_iter().collect(),
        urls: content
            .urls
            .iter()
            .map(|(key, url)| (key.clone(), project_url(url)))
            .collect(),
        vote: content.vote.as_ref().map(|vote| ReplyVote {
            id: vote.id.unwrap_or_default(),
            title: vote.title.clone().unwrap_or_default(),
        }),
        pictures: content
            .pictures
            .iter()
            .map(|p| ReplyPicture {
                img_src: p.img_src.clone().unwrap_or_default(),
                img_width: p.img_width.unwrap_or_default(),
                img_height: p.img_height.unwrap_or_default(),
                img_size: p.img_size.unwrap_or_default(),
            })
            .collect(),
        note: project_note(content.rich_text.as_ref()),
        members: content
            .members
            .iter()
            .map(|(key, member)| (key.clone(), project_member(member)))
            .collect(),
    }
}

fn project_control(control: &pb::ReplyControl) -> ReplyControlInfo {
    ReplyControlInfo {
        action: control.action.unwrap_or_default(),
        is_up_top: control.is_up_top.unwrap_or_default(),
        show_translation: control.show_translation.unwrap_or_default(),
        up_reply: control.up_reply.unwrap_or_default(),
        is_note: control.is_note.unwrap_or_default(),
        is_note_v2: control.is_note_v2.unwrap_or_default(),
        location: control.location.clone().filter(|s| !s.is_empty()),
        can_translate: control.translation_switch
            == Some(pb::TranslationSwitch::ShowTranslation as i32),
        card_labels: control
            .card_labels
            .iter()
            .map(|label| label.text_content.clone().unwrap_or_default())
            .collect(),
        vote_option: control.vote_option.as_ref().map(|o| ReplyVoteOption {
            label_kind: i64::from(o.label_kind.unwrap_or_default()),
            desc: o.desc.clone().unwrap_or_default(),
        }),
    }
}

pub(crate) fn project_reply(reply: &pb::ReplyInfo) -> ReplyEntity {
    ReplyEntity {
        id: reply.id.unwrap_or_default(),
        oid: reply.oid.unwrap_or_default(),
        kind: reply.r#type.unwrap_or_default(),
        mid: reply.mid.unwrap_or_default(),
        root: reply.root.unwrap_or_default(),
        parent: reply.parent.unwrap_or_default(),
        dialog: reply.dialog.unwrap_or_default(),
        ctime: reply.ctime.unwrap_or_default(),
        like: reply.like.unwrap_or_default(),
        count: reply.count.unwrap_or_default(),
        content: reply
            .content
            .as_ref()
            .map(project_content)
            .unwrap_or_default(),
        translated_content: reply.translated_content.as_ref().map(project_content),
        member: reply
            .member
            .as_ref()
            .map(project_member)
            .unwrap_or_default(),
        card: project_card(reply.member_v2.as_ref()),
        control: reply
            .reply_control
            .as_ref()
            .map(project_control)
            .unwrap_or_default(),
        parent_member: reply
            .parent_reply_member
            .as_ref()
            .map(|m| ReplyParentMember {
                mid: m.mid,
                name: m.name.clone().unwrap_or_default(),
            }),
        replies: reply.replies.iter().map(project_reply).collect(),
    }
}

fn project_subject(subject: Option<&pb::SubjectControl>) -> ReplySubject {
    let Some(subject) = subject else {
        return ReplySubject::default();
    };
    ReplySubject {
        up_mid: subject.up_mid.unwrap_or_default(),
        count: subject.count.unwrap_or_default(),
        title: subject.title.clone().unwrap_or_default(),
        show_title: subject.show_title.unwrap_or_default(),
        input_disable: subject.input_disable.unwrap_or_default(),
        // 基线用 `hasRootText()` 判，空串也算有——但空串弹不出有意义的提示。
        root_text: subject.root_text.clone().filter(|s| !s.is_empty()),
    }
}

/// 从 `mixed_cards` 里挑折叠卡。基线 `getFoldCard` 找的是 `type == FOLD`。
fn project_fold_card(cards: &[pb::MixedCard]) -> Option<ReplyFoldCard> {
    cards.iter().find_map(|card| {
        if card.r#type != Some(pb::mixed_card::Type::Fold as i32) {
            return None;
        }
        // 生成面上只留了折叠这一个分支（热点与问卷没有消费者），所以这里
        // 不再有别的 case 可分。
        let pb::mixed_card::Item::Fold(fold) = card.item.as_ref()?;
        Some(ReplyFoldCard {
            bottom_text: fold.bottom_text.clone().unwrap_or_default(),
            pagination_offset: fold
                .fold_pagination
                .as_ref()
                .and_then(|p| p.offset.clone())
                .unwrap_or_default(),
        })
    })
}

fn project_search_item(item: &pb::SearchItem) -> Option<ReplySearchItem> {
    let url = item.url.clone().unwrap_or_default();
    match item.item.as_ref()? {
        pb::search_item::Item::Video(video) => match video.video_item.as_ref()? {
            pb::video_search_item::VideoItem::Ugc(ugc) => Some(ReplySearchItem {
                url,
                title: ugc.title.clone().unwrap_or_default(),
                cover: ugc.cover.clone().unwrap_or_default(),
                up_nickname: ugc.up_nickname.clone(),
                category: None,
                duration: ugc.duration,
            }),
            pb::video_search_item::VideoItem::Pgc(pgc) => Some(ReplySearchItem {
                url,
                title: pgc.title.clone().unwrap_or_default(),
                cover: pgc.cover.clone().unwrap_or_default(),
                up_nickname: None,
                category: pgc.category.clone(),
                duration: None,
            }),
        },
        pb::search_item::Item::Article(article) => Some(ReplySearchItem {
            url,
            title: article.title.clone().unwrap_or_default(),
            // 基线取 `covers.firstOrNull ?? ''`。
            cover: article.covers.first().cloned().unwrap_or_default(),
            up_nickname: article.up_nickname.clone(),
            category: None,
            duration: None,
        }),
    }
}

// ─────────────────────────── 过滤 ───────────────────────────

fn filter_input(reply: &pb::ReplyInfo) -> ReplyFilterItem {
    let content = reply.content.as_ref();
    ReplyFilterItem {
        mid: reply.mid.unwrap_or_default(),
        message: content.and_then(|c| c.message.clone()).unwrap_or_default(),
        at_names: content
            .map(|c| c.at_name_to_mid.keys().cloned().collect())
            .unwrap_or_default(),
        has_emotes: content.is_some_and(|c| !c.emotes.is_empty()),
        has_pictures: content.is_some_and(|c| !c.pictures.is_empty()),
        url_extras: content
            .map(|c| {
                c.urls
                    .values()
                    // 基线 `_hasGoods` 第一步就是 `url.hasExtra()`。
                    .filter_map(|url| url.extra.as_ref())
                    .map(|extra| ReplyUrlExtra {
                        goods_cm_control: extra.goods_cm_control.unwrap_or_default(),
                        has_goods_item_id: extra.goods_item_id.is_some(),
                        has_goods_prefetched_cache: extra.goods_prefetched_cache.is_some(),
                    })
                    .collect()
            })
            .unwrap_or_default(),
        root: reply.root.unwrap_or_default(),
        parent: reply.parent.unwrap_or_default(),
    }
}

fn removes(filter: &ReplyFilter, reply: &pb::ReplyInfo) -> bool {
    filter.removes(&filter_input(reply))
}

/// 就地过滤一串评论，被删掉的追加进 `removed`。
fn retain(
    filter: &ReplyFilter,
    replies: &mut Vec<pb::ReplyInfo>,
    removed: &mut Vec<pb::ReplyInfo>,
) {
    let mut kept = Vec::with_capacity(replies.len());
    for reply in std::mem::take(replies) {
        if removes(filter, &reply) {
            removed.push(reply);
        } else {
            kept.push(reply);
        }
    }
    *replies = kept;
}

/// 根评论与它带出来的楼中楼。
///
/// 一条根评论被删时它的楼中楼**跟着消失**（基线 `removeWhere` 的语义），
/// 所以这里先判根、判不中才去判子，而不是先把子过一遍。
fn retain_nested(filter: &ReplyFilter, replies: &mut Vec<pb::ReplyInfo>) {
    let mut kept = Vec::with_capacity(replies.len());
    for mut reply in std::mem::take(replies) {
        if removes(filter, &reply) {
            continue;
        }
        let mut dropped = Vec::new();
        retain(filter, &mut reply.replies, &mut dropped);
        kept.push(reply);
    }
    *replies = kept;
}

// ─────────────────────────── 取数 ───────────────────────────

async fn unary<T: HttpTransport>(
    transport: &T,
    context: &RequestContext,
    path: &str,
    payload: Vec<u8>,
) -> Result<Vec<u8>, DomainError> {
    crate::grpc::unary(transport, context, path, &payload, RetryPolicy::Read).await
}

fn decode<M: Message + Default>(bytes: &[u8], what: &str) -> Result<M, DomainError> {
    M::decode(bytes).map_err(|_| protocol_error(format!("{what}响应不是合法的 protobuf")))
}

/// 翻页到底了没有。
///
/// 三个判据合在一起（基线 `handlePagination`）：游标说到底了、服务端没给
/// 下一页的 offset、或者给回来的 offset 与这次请求用的**一样**。最后一条
/// 看着多余，实则是防服务端原地踏步把列表刷成无限循环。
fn is_end(cursor: Option<&pb::CursorReply>, next_offset: &str, requested: Option<&str>) -> bool {
    cursor.and_then(|c| c.is_end).unwrap_or_default()
        || next_offset.is_empty()
        || requested.is_some_and(|offset| offset == next_offset)
}

fn next_offset(reply: Option<&bili_grpc::bilibili::pagination::FeedPaginationReply>) -> String {
    reply
        .and_then(|p| p.next_offset.clone())
        .unwrap_or_default()
}

/// 取一页根评论。
///
/// `is_refresh` 决定要不要把 UP 置顶并进列表头部——基线只在刷新那一次做，
/// 翻页时置顶已经在列表里了，再并一次就会出现两条。
///
/// 整页被过滤光且还有下一页时**自动往下翻**（基线 `mainList` 的递归）。
/// 简介判定（[`ReplyFilter::removes_as_intro`]）刻意放在递归之后：基线里
/// 它在 `ReplyController` 而不是 `ReplyGrpc`，不参与「这页是不是空的」。
///
/// # Errors
/// 过滤规则编不过、传输失败、网关状态非 0、响应不是合法 protobuf。
#[allow(clippy::too_many_arguments)]
pub async fn fetch_main_list<T: HttpTransport>(
    transport: &T,
    context: &RequestContext,
    filter: &ReplyFilter,
    oid: i64,
    kind: i64,
    rpid: Option<i64>,
    mode: ReplyMode,
    offset: Option<&str>,
    is_refresh: bool,
) -> Result<ReplyMainPage, DomainError> {
    let mut offset = offset.map(str::to_owned);
    // 翻到底为止。基线写成递归，这里写成循环：同一件事，但不必担心一条
    // 评论区全是带货时把栈递到底。
    loop {
        let bytes = unary(
            transport,
            context,
            protocol::REPLY_MAIN_LIST,
            main_list_request(oid, kind, rpid, mode, offset.as_deref()),
        )
        .await?;
        match project_main_list(&bytes, filter, is_refresh, offset.as_deref())? {
            MainListOutcome::Page(page) => return Ok(*page),
            MainListOutcome::EmptyPage { next } => offset = Some(next),
        }
    }
}

/// 一页根评论过滤之后的两种结局。
#[derive(Debug)]
pub enum MainListOutcome {
    /// 有内容（或者已经到底了），交给界面。
    Page(Box<ReplyMainPage>),
    /// 整页被过滤光了，但还有下一页。`next` 是下一页的 offset。
    EmptyPage { next: String },
}

/// 解码并投影一页根评论。网络那一段不参与。
///
/// 单独拆出来是为了能在没有网络的情况下把「过滤 → 合并置顶 → 剔除简介
/// 复读 → 投影」整条链路钉住（`tests/reply_grpc_golden.rs`）。
///
/// # Errors
/// 响应不是合法的 protobuf。
pub fn project_main_list(
    bytes: &[u8],
    filter: &ReplyFilter,
    is_refresh: bool,
    requested_offset: Option<&str>,
) -> Result<MainListOutcome, DomainError> {
    let mut data: pb::MainListReply = decode(bytes, "评论列表")?;

    let mut up_top = data.up_top.take();
    if let Some(top) = &mut up_top {
        if removes(filter, top) {
            up_top = None;
        } else {
            let mut dropped = Vec::new();
            retain(filter, &mut top.replies, &mut dropped);
        }
    }
    retain_nested(filter, &mut data.replies);

    let next = next_offset(data.pagination_reply.as_ref());
    let end = is_end(data.cursor.as_ref(), &next, requested_offset);

    if data.replies.is_empty() && up_top.is_none() && !end {
        return Ok(MainListOutcome::EmptyPage { next });
    }

    let has_up_top = up_top.is_some();
    let mut replies: Vec<ReplyEntity> = Vec::with_capacity(data.replies.len() + 1);
    if is_refresh {
        replies.extend(up_top.as_ref().map(project_reply));
    }
    replies.extend(data.replies.iter().map(project_reply));
    replies.retain(|reply| !filter.removes_as_intro(&reply.content.message));

    Ok(MainListOutcome::Page(Box::new(ReplyMainPage {
        replies,
        has_up_top,
        subject: project_subject(data.subject_control.as_ref()),
        vote_card: data.vote_card.as_ref().map(|card| ReplyVoteCard {
            vote_id: card.vote_id.unwrap_or_default(),
            title: card.title.clone().unwrap_or_default(),
            count: card.count.unwrap_or_default(),
        }),
        fold_card: project_fold_card(&data.mixed_cards),
        has_mixed_cards: !data.mixed_cards.is_empty(),
        next_offset: next,
        is_end: end,
    })))
}

/// 取一页楼中楼。
///
/// # Errors
/// 同 [`fetch_main_list`]。
#[allow(clippy::too_many_arguments)]
pub async fn fetch_detail_list<T: HttpTransport>(
    transport: &T,
    context: &RequestContext,
    filter: &ReplyFilter,
    oid: i64,
    kind: i64,
    root: i64,
    rpid: i64,
    mode: ReplyMode,
    offset: Option<&str>,
) -> Result<ReplyDetailPage, DomainError> {
    let bytes = unary(
        transport,
        context,
        protocol::REPLY_DETAIL_LIST,
        detail_list_request(oid, kind, root, rpid, mode, offset),
    )
    .await?;
    let mut data: pb::DetailListReply = decode(&bytes, "楼中楼列表")?;

    let mut root_reply = data.root.take().unwrap_or_default();
    let mut removed = Vec::new();
    retain(filter, &mut root_reply.replies, &mut removed);

    let next = next_offset(data.pagination_reply.as_ref());
    Ok(ReplyDetailPage {
        replies: root_reply.replies.iter().map(project_reply).collect(),
        root: project_reply(&root_reply),
        subject: project_subject(data.subject_control.as_ref()),
        fold_card: project_fold_card(&data.mixed_cards),
        has_mixed_cards: !data.mixed_cards.is_empty(),
        removed: removed.iter().map(project_reply).collect(),
        is_end: is_end(data.cursor.as_ref(), &next, offset),
        next_offset: next,
    })
}

/// 取一页对话。
///
/// # Errors
/// 同 [`fetch_main_list`]。
#[allow(clippy::too_many_arguments)]
pub async fn fetch_dialog_list<T: HttpTransport>(
    transport: &T,
    context: &RequestContext,
    filter: &ReplyFilter,
    oid: i64,
    kind: i64,
    root: i64,
    dialog: i64,
    offset: Option<&str>,
) -> Result<ReplyFlatPage, DomainError> {
    let bytes = unary(
        transport,
        context,
        protocol::REPLY_DIALOG_LIST,
        dialog_list_request(oid, kind, root, dialog, offset),
    )
    .await?;
    let mut data: pb::DialogListReply = decode(&bytes, "对话列表")?;

    let mut removed = Vec::new();
    retain(filter, &mut data.replies, &mut removed);

    let next = next_offset(data.pagination_reply.as_ref());
    Ok(ReplyFlatPage {
        replies: data.replies.iter().map(project_reply).collect(),
        subject: project_subject(data.subject_control.as_ref()),
        title: None,
        is_end: is_end(data.cursor.as_ref(), &next, offset),
        next_offset: next,
    })
}

/// 取一页折叠评论。
///
/// 折叠列表**没有游标**（`FoldListResp` 里没有 `cursor`），到底与否只看
/// `next_offset`。
///
/// # Errors
/// 同 [`fetch_main_list`]。
pub async fn fetch_fold_list<T: HttpTransport>(
    transport: &T,
    context: &RequestContext,
    filter: &ReplyFilter,
    oid: i64,
    kind: i64,
    offset: &str,
) -> Result<ReplyFlatPage, DomainError> {
    let bytes = unary(
        transport,
        context,
        protocol::REPLY_FOLD_LIST,
        fold_list_request(oid, kind, offset),
    )
    .await?;
    let mut data: pb::FoldListResp = decode(&bytes, "折叠评论列表")?;

    let mut removed = Vec::new();
    retain(filter, &mut data.replies, &mut removed);

    let next = next_offset(data.pagination_reply.as_ref());
    Ok(ReplyFlatPage {
        replies: data.replies.iter().map(project_reply).collect(),
        subject: project_subject(data.subject_control.as_ref()),
        // 基线 `response.hasTitle()` 时 toast 一下。
        title: data.title.clone().filter(|s| !s.is_empty()),
        is_end: is_end(None, &next, Some(offset)),
        next_offset: next,
    })
}

/// 评论区里搜稿件 / 专栏（发评论时插链接用）。
///
/// # Errors
/// 传输失败、网关状态非 0、响应不是合法 protobuf。
pub async fn fetch_search_item<T: HttpTransport>(
    transport: &T,
    context: &RequestContext,
    oid: i64,
    kind: i64,
    page: i64,
    item_kind: ReplySearchKind,
    keyword: &str,
) -> Result<ReplySearchPage, DomainError> {
    let bytes = unary(
        transport,
        context,
        protocol::REPLY_SEARCH_ITEM,
        search_item_request(oid, kind, page, item_kind, keyword),
    )
    .await?;
    let data: pb::SearchItemReply = decode(&bytes, "评论区搜索")?;
    Ok(ReplySearchPage {
        items: data.items.iter().filter_map(project_search_item).collect(),
        has_next: data
            .cursor
            .as_ref()
            .and_then(|c| c.has_next)
            .unwrap_or_default(),
    })
}

/// 翻译一条评论。没有译文时返回 `None`（基线此时 toast「翻译结果为空」）。
///
/// # Errors
/// 传输失败、网关状态非 0、响应不是合法 protobuf。
pub async fn fetch_translation<T: HttpTransport>(
    transport: &T,
    context: &RequestContext,
    oid: i64,
    kind: i64,
    rpid: i64,
) -> Result<Option<ReplyContent>, DomainError> {
    let bytes = unary(
        transport,
        context,
        protocol::REPLY_TRANSLATE,
        translate_request(oid, kind, rpid),
    )
    .await?;
    let data: pb::TranslateReplyResp = decode(&bytes, "评论翻译")?;
    Ok(data
        .translated_replies
        .get(&rpid)
        .and_then(|reply| reply.translated_content.as_ref())
        .map(project_content))
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::{
        ReplyMode, ReplySearchKind, detail_list_request, fold_list_request, main_list_request,
        project_emote, project_reply, project_url, retain_nested, search_item_request,
    };
    use bili_domain::reply_filter::{ReplyFilter, ReplyFilterSettings};
    use bili_grpc::bilibili::main::community::reply::v1 as pb;
    use prost::Message;

    fn filter(settings: ReplyFilterSettings) -> ReplyFilter {
        ReplyFilter::compile(&settings).expect("设置可编译")
    }

    fn reply(id: i64, mid: i64, message: &str) -> pb::ReplyInfo {
        pb::ReplyInfo {
            id: Some(id),
            mid: Some(mid),
            content: Some(pb::Content {
                message: Some(message.to_owned()),
                ..pb::Content::default()
            }),
            ..pb::ReplyInfo::default()
        }
    }

    #[test]
    fn the_first_page_carries_no_pagination() {
        let decoded =
            pb::MainListReq::decode(main_list_request(1, 1, None, ReplyMode::Hot, None).as_slice())
                .expect("应能解回来");
        assert!(decoded.pagination.is_none(), "首页不带 pagination");
        assert_eq!(decoded.mode, Some(pb::Mode::MainListHot as i32));

        // 空串与 null 等价：基线的 `nextOffset` 初值就是 null。
        let decoded = pb::MainListReq::decode(
            main_list_request(1, 1, None, ReplyMode::Time, Some("")).as_slice(),
        )
        .expect("应能解回来");
        assert!(decoded.pagination.is_none());
    }

    #[test]
    fn detail_list_pins_the_reply_scene() {
        let decoded = pb::DetailListReq::decode(
            detail_list_request(1, 1, 2, 3, ReplyMode::Time, Some("x")).as_slice(),
        )
        .expect("应能解回来");
        assert_eq!(decoded.scene, Some(pb::DetailListScene::Reply as i32));
        assert_eq!(
            decoded.pagination.and_then(|p| p.offset).as_deref(),
            Some("x")
        );
    }

    #[test]
    fn fold_list_passes_the_offset_through_untouched() {
        let decoded = pb::FoldListReq::decode(fold_list_request(1, 1, "opaque-cursor").as_slice())
            .expect("应能解回来");
        assert_eq!(
            decoded.pagination.and_then(|p| p.offset).as_deref(),
            Some("opaque-cursor")
        );
    }

    #[test]
    fn search_item_carries_the_item_type_on_the_cursor() {
        let decoded = pb::SearchItemReq::decode(
            search_item_request(1, 1, 2, ReplySearchKind::Article, "关键词").as_slice(),
        )
        .expect("应能解回来");
        let cursor = decoded.cursor.expect("有 cursor");
        assert_eq!(cursor.next, Some(2));
        assert_eq!(cursor.item_type, Some(pb::SearchItemType::Article as i32));
    }

    #[test]
    fn emote_url_falls_back_webp_then_gif_then_plain() {
        let full = pb::Emote {
            url: Some("plain".into()),
            gif_url: Some("gif".into()),
            webp_url: Some("webp".into()),
            ..pb::Emote::default()
        };
        assert_eq!(project_emote(&full).display_url, "webp");

        let no_webp = pb::Emote {
            webp_url: Some(String::new()),
            ..full.clone()
        };
        assert_eq!(
            project_emote(&no_webp).display_url,
            "gif",
            "显式空串要落到下一档，拿空串去加载图片与没有这个字段没有区别"
        );

        let plain_only = pb::Emote {
            url: Some("plain".into()),
            ..pb::Emote::default()
        };
        assert_eq!(project_emote(&plain_only).display_url, "plain");
    }

    #[test]
    fn note_urls_are_recognised_by_the_click_report_prefix() {
        let note = pb::Url {
            click_report: Some(r#"{"cvid":123}"#.into()),
            ..pb::Url::default()
        };
        assert!(project_url(&note).is_note);

        let other = pb::Url {
            click_report: Some(r#"{"aid":123}"#.into()),
            ..pb::Url::default()
        };
        assert!(!project_url(&other).is_note);
        assert!(!project_url(&pb::Url::default()).is_note);
    }

    #[test]
    fn official_verify_type_zero_is_not_the_same_as_absent() {
        let verified = pb::ReplyInfo {
            member: Some(pb::Member {
                official_verify_type: Some(0),
                ..pb::Member::default()
            }),
            ..pb::ReplyInfo::default()
        };
        assert_eq!(
            project_reply(&verified).member.official_verify_type,
            Some(0),
            "0 是「个人认证」，不是「没有认证」"
        );
        assert_eq!(
            project_reply(&pb::ReplyInfo::default())
                .member
                .official_verify_type,
            None
        );
    }

    #[test]
    fn dropping_a_root_reply_takes_its_nested_replies_with_it() {
        let f = filter(ReplyFilterSettings {
            blocked_mids: vec![9],
            ..ReplyFilterSettings::default()
        });
        let mut replies = vec![
            pb::ReplyInfo {
                replies: vec![reply(11, 1, "子一"), reply(12, 9, "子二")],
                ..reply(1, 9, "被拉黑的根评论")
            },
            pb::ReplyInfo {
                replies: vec![reply(21, 1, "子一"), reply(22, 9, "子二")],
                ..reply(2, 1, "正常的根评论")
            },
        ];
        retain_nested(&f, &mut replies);

        assert_eq!(replies.len(), 1);
        assert_eq!(replies[0].id, Some(2));
        assert_eq!(
            replies[0]
                .replies
                .iter()
                .filter_map(|r| r.id)
                .collect::<Vec<_>>(),
            vec![21],
            "留下来的根评论里，被拉黑的楼中楼要删掉"
        );
    }

    #[test]
    fn unknown_fields_do_not_fail_the_page() {
        // 上游随时会加字段；加了就整页判错会让评论区整个打不开。
        let mut bytes = pb::MainListReply::default().encode_to_vec();
        // field 1000, varint
        bytes.extend_from_slice(&[0xC0, 0x3E, 0x01]);
        let decoded = pb::MainListReply::decode(bytes.as_slice()).expect("应解析成功");
        assert!(decoded.replies.is_empty());
    }
}
