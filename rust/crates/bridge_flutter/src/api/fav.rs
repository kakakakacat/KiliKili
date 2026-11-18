//! 收藏夹的 Dart 导出（S8 / issue #58）。
//!
//! 对应基线 `FavHttp` 的全部 34 条接口（`lib/http/fav.dart`，本片迁完已删）。
//! 请求构造、幂等定级、排序取值、分页倒序都在 `bili_api::fav` 一侧定死，
//! 这一层只负责把凭据与设置递过去、把失败折成返回值。
//!
//! ## 为什么只有一个上下文
//!
//! 与 S4 不同，这一片 34 条接口全部挂在 `api.bilibili.com`，鉴权也只有
//! 一套（Cookie + csrf）。个人空间收藏页那条虽然**参数**伪装成移动端，
//! 鉴权仍然走 Cookie——它要的两条请求头由调用方放进 [`FavContext::headers`]，
//! 不构成第二种上下文。
//!
//! 两条排序接口另外要 APP 签名材料，用 [`FavSignature`] 单独传：把
//! `appkey`/`appsec`/`ts` 塞进 [`FavContext`] 会让其余 32 条接口都带着
//! 三个用不上的字段，而「用不上的字段」下一个人一定会填。
//!
//! ## 合集详情的 `info` 复用了 [`SubItem`]
//!
//! 订阅列表页点进去就是合集详情页，两处喂同一个头部。基线为此留了
//! `lib/services/rust/sub_compat.dart` 当过渡，**本片迁完它就该删**——
//! 那个文件的注释里写着等的正是这一刻。

use std::time::Duration;

use bili_api::fav::{self, AppSignature, FavOrder as ApiFavOrder};
use bili_api::http::RequestContext;
use bili_api::request::PreparedWrite;
use bili_api::transport_reqwest::ReqwestTransport;
use bili_domain::error::DomainError;
use bili_domain::redact::Redacted;

use crate::api::init;
use crate::api::transport::TransportSettings;
use crate::api::user::{SubItem, SubList, SubListResult};
use crate::api::write::WriteResult;

// ------------------------------------------------------------------ 入参

/// 收藏夹接口的公共参数。
#[derive(Debug, Clone)]
pub struct FavContext {
    pub base_url: String,
    /// 完整的 Cookie 头值。未登录时为 `None`。**Rust 不保存**，用完即弃。
    pub cookie: Option<String>,
    /// CSRF token（cookie 里的 `bili_jct`）。写操作要，笔记列表那条 GET
    /// 也要（基线把它放在查询串里）。
    pub csrf: Option<String>,
    pub headers: Vec<(String, String)>,
    pub timeout_ms: u64,
    pub transport: TransportSettings,
}

/// 两条排序接口的 APP 签名材料。
///
/// 它们的 csrf **被签名覆盖**，所以 csrf 的注入点前移到了请求构造处
/// （见 `bili_api::fav` 的模块文档）。这里把签名材料单独递过去。
#[derive(Debug, Clone)]
pub struct FavSignature {
    pub appkey: String,
    pub appsec: String,
    /// 签名用的 Unix 秒，由调用方注入以便快照固定。
    pub ts: i64,
}

impl FavContext {
    fn split(self) -> (RequestContext, ReqwestTransport) {
        let transport = ReqwestTransport::new(self.transport.to_config());
        let ctx = RequestContext {
            base_url: self.base_url,
            cookie: self.cookie.map(Redacted::new),
            csrf: self.csrf.map(Redacted::new),
            headers: self.headers,
            timeout: Duration::from_millis(self.timeout_ms),
        };
        (ctx, transport)
    }
}

impl From<FavSignature> for AppSignature {
    fn from(value: FavSignature) -> Self {
        Self {
            appkey: value.appkey,
            appsec: value.appsec,
            ts: value.ts,
        }
    }
}

/// 收藏夹详情的排序方式。界面上的标签留在宿主，这里只有协议取值。
#[derive(Debug, Clone, Copy)]
pub enum FavOrder {
    /// 最近收藏。
    Mtime,
    /// 最多播放。
    View,
    /// 最近投稿。
    Pubtime,
}

impl From<FavOrder> for ApiFavOrder {
    fn from(value: FavOrder) -> Self {
        match value {
            FavOrder::Mtime => Self::Mtime,
            FavOrder::View => Self::View,
            FavOrder::Pubtime => Self::Pubtime,
        }
    }
}

/// 复制还是移动。
#[derive(Debug, Clone, Copy)]
pub enum FavMoveMode {
    Copy,
    Move,
}

impl From<FavMoveMode> for fav::FavMoveMode {
    fn from(value: FavMoveMode) -> Self {
        match value {
            FavMoveMode::Copy => Self::Copy,
            FavMoveMode::Move => Self::Move,
        }
    }
}

/// 目标体系：收藏夹之间，还是稍后再看。
#[derive(Debug, Clone, Copy)]
pub enum FavMoveTarget {
    Folder,
    ToView,
}

impl From<FavMoveTarget> for fav::FavMoveTarget {
    fn from(value: FavMoveTarget) -> Self {
        match value {
            FavMoveTarget::Folder => Self::Folder,
            FavMoveTarget::ToView => Self::ToView,
        }
    }
}

// ------------------------------------------------------------------ 结果

/// 一个收藏夹。
#[derive(Debug, Clone)]
pub struct FavFolder {
    pub id: i64,
    pub fid: Option<i64>,
    pub mid: i64,
    /// 属性位掩码：`& 1` 是默认收藏夹，`& 2` 是私密。
    pub attr: i64,
    pub title: String,
    pub cover: String,
    pub upper_mid: Option<i64>,
    pub upper_name: Option<String>,
    pub upper_face: Option<String>,
    pub intro: Option<String>,
    /// 当前账号是否已收藏这个收藏夹。
    pub fav_state: Option<i64>,
    pub media_count: i64,
}

/// 收藏夹列表的一页。
#[derive(Debug, Clone)]
pub struct FavFolderList {
    pub count: Option<i64>,
    pub list: Vec<FavFolder>,
    pub has_more: Option<bool>,
}

/// 收藏夹里的一项内容。`kind` 是协议的 `type`：2 视频、24 番剧…
#[derive(Debug, Clone)]
pub struct FavMedia {
    pub id: Option<i64>,
    pub kind: Option<i64>,
    pub title: Option<String>,
    pub cover: Option<String>,
    pub intro: Option<String>,
    pub duration: Option<i64>,
    pub upper_mid: Option<i64>,
    pub upper_name: Option<String>,
    pub upper_face: Option<String>,
    /// 失效标记：`9` 是 UP 主删除，`1` 是不可见。
    pub attr: Option<i64>,
    pub play_count: Option<i64>,
    pub danmaku_count: Option<i64>,
    pub fav_time: Option<i64>,
    pub bvid: Option<String>,
    pub ogv_type_name: Option<String>,
    pub ogv_season_id: Option<i64>,
    pub ugc_first_cid: Option<i64>,
}

/// 收藏夹详情的一页。
#[derive(Debug, Clone)]
pub struct FavDetail {
    pub info: Option<FavFolder>,
    pub medias: Vec<FavMedia>,
    pub has_more: Option<bool>,
}

/// 追番/追剧列表里的一项。
#[derive(Debug, Clone)]
pub struct FavPgcItem {
    pub season_id: Option<i64>,
    pub title: Option<String>,
    pub cover: Option<String>,
    pub is_finish: Option<i64>,
    pub badge: Option<String>,
    pub new_ep_index_show: Option<String>,
    pub renewal_time: Option<String>,
    /// 观看进度。服务端的空串已经归一成 `None`。
    pub progress: Option<String>,
}

/// 追番/追剧列表的一页。
#[derive(Debug, Clone)]
pub struct FavPgcList {
    pub list: Vec<FavPgcItem>,
    pub total: Option<i64>,
}

/// 收藏的一门课程。
#[derive(Debug, Clone)]
pub struct FavPugvItem {
    pub cover: Option<String>,
    pub marks: Vec<String>,
    pub season_id: Option<i64>,
    pub status: Option<String>,
    pub title: Option<String>,
    pub ctime: Option<String>,
}

/// 收藏的课程一页。`has_next` 为 `Some(false)` 时到底了。
#[derive(Debug, Clone)]
pub struct FavPugvList {
    pub items: Vec<FavPugvItem>,
    pub has_next: Option<bool>,
}

/// 一个话题。
#[derive(Debug, Clone)]
pub struct FavTopicItem {
    pub id: Option<i64>,
    pub name: Option<String>,
}

/// 收藏的话题一页。
#[derive(Debug, Clone)]
pub struct FavTopicList {
    pub items: Vec<FavTopicItem>,
    pub total: Option<i64>,
}

/// 收藏的一篇专栏。`opus_id` 是字符串，取消收藏时原样发回去。
#[derive(Debug, Clone)]
pub struct FavArticleItem {
    pub opus_id: Option<String>,
    pub content: Option<String>,
    pub author_name: Option<String>,
    pub cover_url: Option<String>,
    pub like: Option<String>,
    pub pub_time: Option<String>,
}

/// 收藏的专栏一页。
#[derive(Debug, Clone)]
pub struct FavArticleList {
    pub items: Vec<FavArticleItem>,
    pub has_more: Option<bool>,
}

/// 一条笔记。删除时发布过的用 `cvid`，没发布的用 `note_id`。
#[derive(Debug, Clone)]
pub struct FavNote {
    pub web_url: Option<String>,
    pub title: Option<String>,
    pub summary: Option<String>,
    pub message: Option<String>,
    pub pic: Option<String>,
    pub cvid: Option<String>,
    pub note_id: Option<String>,
}

/// 个人空间收藏页的一组。
#[derive(Debug, Clone)]
pub struct SpaceFavGroup {
    pub id: Option<i64>,
    pub name: Option<String>,
    pub count: Option<i64>,
    /// 组里的收藏夹或合集，按 `kind` 区分（0/11 收藏夹，21 合集）。
    ///
    /// 复用 [`SubItem`]，理由见 [`bili_domain::fav::SpaceFavGroup::list`]。
    pub list: Vec<SubItem>,
}

/// 合集里的一个稿件。
#[derive(Debug, Clone)]
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

/// 合集详情的一页。
#[derive(Debug, Clone)]
pub struct FavSeasonDetail {
    pub info: Option<SubItem>,
    pub medias: Vec<FavSeasonMedia>,
}

macro_rules! result_type {
    ($(#[$meta:meta])* $name:ident, $data:ty) => {
        $(#[$meta])*
        #[derive(Debug, Clone)]
        pub struct $name {
            pub data: Option<$data>,
            pub error_kind: Option<String>,
            pub error_detail: Option<String>,
            pub error_code: Option<i64>,
        }
    };
}

result_type!(
    /// 收藏夹列表的结果。
    FavFolderListResult,
    FavFolderList
);
result_type!(
    /// 单个收藏夹的结果。新建/编辑收藏夹也返回它。
    FavFolderResult,
    FavFolder
);
result_type!(
    /// 收藏夹详情的结果。
    FavDetailResult,
    FavDetail
);
result_type!(
    /// 追番/追剧列表的结果。
    FavPgcListResult,
    FavPgcList
);
result_type!(
    /// 课程列表的结果。
    FavPugvListResult,
    FavPugvList
);
result_type!(
    /// 话题列表的结果。
    FavTopicListResult,
    FavTopicList
);
result_type!(
    /// 专栏列表的结果。
    FavArticleListResult,
    FavArticleList
);
result_type!(
    /// 笔记列表的结果。
    FavNoteListResult,
    Vec<FavNote>
);
result_type!(
    /// 个人空间收藏页的结果。
    SpaceFavResult,
    Vec<SpaceFavGroup>
);
result_type!(
    /// 合集详情的结果。
    FavSeasonDetailResult,
    FavSeasonDetail
);

// ------------------------------------------------------------ 只读导出

/// 自己创建的收藏夹，分页。
pub async fn fetch_fav_folders(
    context: FavContext,
    up_mid: i64,
    pn: i64,
    ps: i64,
) -> FavFolderListResult {
    fetch_folder_list(context, fav::fav_folder_list_request(up_mid, pn, ps)).await
}

/// 自己创建的收藏夹，一次给全。
pub async fn fetch_all_fav_folders(context: FavContext, up_mid: i64) -> FavFolderListResult {
    fetch_folder_list(context, fav::all_fav_folders_request(up_mid)).await
}

/// 「这个资源被收藏在哪几个夹子里」。每一项会多带 `fav_state`。
pub async fn fetch_video_in_folder(
    context: FavContext,
    up_mid: i64,
    rid: i64,
    kind: Option<i64>,
) -> FavFolderListResult {
    fetch_folder_list(context, fav::video_in_folder_request(up_mid, rid, kind)).await
}

/// 单个收藏夹的信息。
pub async fn fetch_fav_folder_info(context: FavContext, media_id: i64) -> FavFolderResult {
    let (ctx, transport) = context.split();
    let joined = init::runtime()
        .spawn(async move {
            let prepared = fav::fav_folder_info_request(media_id);
            fav::fetch(&transport, &ctx, &prepared, fav::parse_fav_folder).await
        })
        .await;
    folder_result(flatten(joined))
}

/// 收藏夹详情的一页。
///
/// `page_desc` 为真时整页倒序，与基线 `pageDesc` 一致——分页方向反过来
/// 时每一页内部也要反过来才接得上。
// 八个参数都是这条接口真有的查询项（基线同样八个），打包成一个结构体
// 只会多一个 FRB 导出类型，而调用点仍然要逐项填。
#[allow(clippy::too_many_arguments)]
pub async fn fetch_fav_detail(
    context: FavContext,
    media_id: i64,
    pn: i64,
    ps: i64,
    keyword: String,
    order: FavOrder,
    kind: i64,
    page_desc: bool,
) -> FavDetailResult {
    let (ctx, transport) = context.split();
    let joined = init::runtime()
        .spawn(async move {
            let prepared = fav::fav_detail_request(media_id, pn, ps, &keyword, order.into(), kind);
            fav::fetch(&transport, &ctx, &prepared, |body| {
                fav::parse_fav_detail(body, page_desc)
            })
            .await
        })
        .await;

    match flatten(joined) {
        Ok(d) => FavDetailResult {
            data: Some(FavDetail {
                info: d.info.map(folder_to_bridge),
                medias: d.medias.into_iter().map(media_to_bridge).collect(),
                has_more: d.has_more,
            }),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => FavDetailResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 合集详情的一页。
pub async fn fetch_fav_season_detail(
    context: FavContext,
    season_id: i64,
    pn: i64,
    ps: i64,
    page_desc: bool,
) -> FavSeasonDetailResult {
    let (ctx, transport) = context.split();
    let joined = init::runtime()
        .spawn(async move {
            let prepared = fav::fav_season_list_request(season_id, pn, ps);
            fav::fetch(&transport, &ctx, &prepared, |body| {
                fav::parse_fav_season_detail(body, page_desc)
            })
            .await
        })
        .await;

    match flatten(joined) {
        Ok(d) => FavSeasonDetailResult {
            data: Some(FavSeasonDetail {
                info: d.info.map(sub_item_to_bridge),
                medias: d
                    .medias
                    .into_iter()
                    .map(|m| FavSeasonMedia {
                        id: m.id,
                        title: m.title,
                        cover: m.cover,
                        duration: m.duration,
                        pubtime: m.pubtime,
                        bvid: m.bvid,
                        play_count: m.play_count,
                        danmaku_count: m.danmaku_count,
                    })
                    .collect(),
            }),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => FavSeasonDetailResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 追番/追剧列表的一页。`kind` 为 1 番剧、2 影视。
pub async fn fetch_fav_pgc(
    context: FavContext,
    vmid: i64,
    kind: i64,
    follow_status: Option<i64>,
    pn: i64,
) -> FavPgcListResult {
    let (ctx, transport) = context.split();
    let joined = init::runtime()
        .spawn(async move {
            let prepared = fav::fav_pgc_request(vmid, kind, follow_status, pn);
            fav::fetch(&transport, &ctx, &prepared, fav::parse_fav_pgc).await
        })
        .await;

    match flatten(joined) {
        Ok(l) => FavPgcListResult {
            data: Some(FavPgcList {
                list: l
                    .list
                    .into_iter()
                    .map(|i| FavPgcItem {
                        season_id: i.season_id,
                        title: i.title,
                        cover: i.cover,
                        is_finish: i.is_finish,
                        badge: i.badge,
                        new_ep_index_show: i.new_ep_index_show,
                        renewal_time: i.renewal_time,
                        progress: i.progress,
                    })
                    .collect(),
                total: l.total,
            }),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => FavPgcListResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 收藏的课程一页。
pub async fn fetch_fav_pugv(context: FavContext, mid: i64, pn: i64) -> FavPugvListResult {
    let (ctx, transport) = context.split();
    let joined = init::runtime()
        .spawn(async move {
            let prepared = fav::fav_pugv_request(mid, pn);
            fav::fetch(&transport, &ctx, &prepared, fav::parse_fav_pugv).await
        })
        .await;

    match flatten(joined) {
        Ok(l) => FavPugvListResult {
            data: Some(FavPugvList {
                items: l
                    .items
                    .into_iter()
                    .map(|i| FavPugvItem {
                        cover: i.cover,
                        marks: i.marks,
                        season_id: i.season_id,
                        status: i.status,
                        title: i.title,
                        ctime: i.ctime,
                    })
                    .collect(),
                has_next: l.has_next,
            }),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => FavPugvListResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 收藏的话题一页。
pub async fn fetch_fav_topics(context: FavContext, page: i64) -> FavTopicListResult {
    let (ctx, transport) = context.split();
    let joined = init::runtime()
        .spawn(async move {
            let prepared = fav::fav_topic_request(page);
            fav::fetch(&transport, &ctx, &prepared, fav::parse_fav_topic).await
        })
        .await;

    match flatten(joined) {
        Ok(l) => FavTopicListResult {
            data: Some(FavTopicList {
                items: l
                    .items
                    .into_iter()
                    .map(|i| FavTopicItem {
                        id: i.id,
                        name: i.name,
                    })
                    .collect(),
                total: l.total,
            }),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => FavTopicListResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 收藏的专栏一页。
pub async fn fetch_fav_articles(context: FavContext, page: i64) -> FavArticleListResult {
    let (ctx, transport) = context.split();
    let joined = init::runtime()
        .spawn(async move {
            let prepared = fav::fav_article_request(page);
            fav::fetch(&transport, &ctx, &prepared, fav::parse_fav_article).await
        })
        .await;

    match flatten(joined) {
        Ok(l) => FavArticleListResult {
            data: Some(FavArticleList {
                items: l
                    .items
                    .into_iter()
                    .map(|i| FavArticleItem {
                        opus_id: i.opus_id,
                        content: i.content,
                        author_name: i.author_name,
                        cover_url: i.cover_url,
                        like: i.like,
                        pub_time: i.pub_time,
                    })
                    .collect(),
                has_more: l.has_more,
            }),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => FavArticleListResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 笔记列表的一页。`published` 为真时取「我发布的笔记」。
///
/// 这条 GET 要带 csrf（基线把它放在查询串里），从 [`FavContext::csrf`] 取；
/// 未登录时发空串，与基线一致。
pub async fn fetch_fav_notes(context: FavContext, page: i64, published: bool) -> FavNoteListResult {
    let csrf = context.csrf.clone().unwrap_or_default();
    let (ctx, transport) = context.split();
    let joined = init::runtime()
        .spawn(async move {
            let prepared = fav::note_list_request(page, published, &csrf);
            fav::fetch(&transport, &ctx, &prepared, fav::parse_fav_notes).await
        })
        .await;

    match flatten(joined) {
        Ok(list) => FavNoteListResult {
            data: Some(
                list.into_iter()
                    .map(|n| FavNote {
                        web_url: n.web_url,
                        title: n.title,
                        summary: n.summary,
                        message: n.message,
                        pic: n.pic,
                        cvid: n.cvid,
                        note_id: n.note_id,
                    })
                    .collect(),
            ),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => FavNoteListResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 个人空间「收藏」页的翻页：把收藏夹列表读成 [`SubItem`]。
///
/// 与 [`fetch_fav_folders`] 是**同一条接口**，只是解成另一个形状——那一页
/// 两组共用一套卡片，而卡片要的是 [`SubItem`]。基线在那里也是拿
/// `SpaceFavItemModel` 解同一份响应的。
///
/// 返回 `api::user` 的 [`SubListResult`]：订阅那一组的翻页本来就走
/// `RustUser.userSubFolder`，两组拿同一个结果类型，调用点才不用分叉。
pub async fn fetch_fav_folders_as_subs(
    context: FavContext,
    up_mid: i64,
    pn: i64,
    ps: i64,
) -> SubListResult {
    let (ctx, transport) = context.split();
    let joined = init::runtime()
        .spawn(async move {
            let prepared = fav::fav_folder_list_request(up_mid, pn, ps);
            fav::fetch(&transport, &ctx, &prepared, fav::parse_fav_folder_sub_list).await
        })
        .await;

    match flatten(joined) {
        Ok(l) => SubListResult {
            data: Some(SubList {
                count: l.count,
                has_more: l.has_more,
                list: l.list.into_iter().map(sub_item_to_bridge).collect(),
            }),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => SubListResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 个人空间的收藏页。
pub async fn fetch_space_fav(context: FavContext, up_mid: i64) -> SpaceFavResult {
    let (ctx, transport) = context.split();
    let joined = init::runtime()
        .spawn(async move {
            let prepared = fav::space_fav_request(up_mid);
            fav::fetch(&transport, &ctx, &prepared, fav::parse_space_fav).await
        })
        .await;

    match flatten(joined) {
        Ok(groups) => SpaceFavResult {
            data: Some(
                groups
                    .into_iter()
                    .map(|g| SpaceFavGroup {
                        id: g.id,
                        name: g.name,
                        count: g.count,
                        list: g.list.into_iter().map(sub_item_to_bridge).collect(),
                    })
                    .collect(),
            ),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => SpaceFavResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

// ------------------------------------------------------------ 写操作导出

/// 收藏 / 取消收藏**别人的**收藏夹。`fav` 是目标状态。
pub async fn fav_folder(context: FavContext, media_id: i64, fav: bool) -> WriteResult {
    send_plain(context, move || fav::fav_folder_request(media_id, fav)).await
}

/// 收藏 / 取消收藏一个合集。`fav` 是目标状态。
pub async fn fav_season(context: FavContext, season_id: i64, fav: bool) -> WriteResult {
    send_plain(context, move || fav::fav_season_request(season_id, fav)).await
}

/// 收藏 / 取消收藏一门课程。`fav` 是目标状态。
pub async fn fav_pugv(context: FavContext, season_id: i64, fav: bool) -> WriteResult {
    send_plain(context, move || fav::fav_pugv_write_request(season_id, fav)).await
}

/// 收藏 / 取消收藏一个话题。`fav` 是目标状态。
pub async fn fav_topic(context: FavContext, topic_id: i64, fav: bool) -> WriteResult {
    send_plain(context, move || fav::fav_topic_write_request(topic_id, fav)).await
}

/// 点赞 / 取消点赞一个话题。`like` 是**目标**状态。
///
/// 基线那个参数传的是当前状态，折算已经收在调用链的宿主一侧，见
/// [`bili_api::fav::like_topic_request`]。
pub async fn like_topic(
    context: FavContext,
    topic_id: i64,
    up_mid: i64,
    like: bool,
) -> WriteResult {
    send_plain(context, move || {
        fav::like_topic_request(topic_id, up_mid, like)
    })
    .await
}

/// 收藏 / 取消收藏一篇专栏（旧 article 接口）。`fav` 是目标状态。
pub async fn fav_article(context: FavContext, id: i64, fav: bool) -> WriteResult {
    send_plain(context, move || fav::fav_article_write_request(id, fav)).await
}

/// 图文动态（opus）的收藏 / 取消收藏。`action`：3 收藏，4 取消。
pub async fn community_action(context: FavContext, opus_id: String, action: i64) -> WriteResult {
    send_plain(context, move || {
        fav::community_action_request(&opus_id, action)
    })
    .await
}

/// 删除笔记。`ids` 是逗号分隔的一串。
pub async fn del_note(context: FavContext, ids: String, published: bool) -> WriteResult {
    send_plain(context, move || fav::del_note_request(&ids, published)).await
}

/// 收藏夹排序。`sort` 是逗号分隔的 id 串，顺序即目标顺序。
///
/// csrf 由请求构造处放进表单并参与 APP 签名，见
/// [`bili_api::fav::sort_fav_folder_request`]。
pub async fn sort_fav_folder(
    context: FavContext,
    signature: FavSignature,
    sort: String,
) -> WriteResult {
    let csrf = context.csrf.clone().unwrap_or_default();
    let auth: AppSignature = signature.into();
    send_plain(context, move || {
        fav::sort_fav_folder_request(&sort, &csrf, &auth)
    })
    .await
}

/// 收藏夹内容排序。
pub async fn sort_fav(
    context: FavContext,
    signature: FavSignature,
    media_id: i64,
    sort: String,
) -> WriteResult {
    let csrf = context.csrf.clone().unwrap_or_default();
    let auth: AppSignature = signature.into();
    send_plain(context, move || {
        fav::sort_fav_request(media_id, &sort, &csrf, &auth)
    })
    .await
}

/// 清除收藏夹里的失效内容。
pub async fn clean_fav(context: FavContext, media_id: i64) -> WriteResult {
    send_plain(context, move || fav::clean_fav_request(media_id)).await
}

/// 删除收藏夹。`media_ids` 是逗号分隔的一串。
pub async fn delete_folder(context: FavContext, media_ids: String) -> WriteResult {
    send_plain(context, move || fav::delete_folder_request(&media_ids)).await
}

/// 新建收藏夹，返回新建出来的那一个。
///
/// **不自动重试**：定级 `Unsafe`，重发会多建一个同名收藏夹
/// （ADR-013 决策一）。失败了由用户自己再按一次。
pub async fn add_folder(
    context: FavContext,
    title: String,
    intro: String,
    privacy: i64,
    cover: String,
) -> FavFolderResult {
    let (ctx, transport) = context.split();
    let joined = init::runtime()
        .spawn(async move {
            let prepared = fav::add_folder_request(&title, &intro, privacy, &cover);
            fav::send_parsed(&transport, &ctx, &prepared, fav::parse_fav_folder).await
        })
        .await;
    folder_result(flatten(joined))
}

/// 编辑收藏夹，返回改完之后的那一个。
pub async fn edit_folder(
    context: FavContext,
    media_id: i64,
    title: String,
    intro: String,
    privacy: i64,
    cover: String,
) -> FavFolderResult {
    let (ctx, transport) = context.split();
    let joined = init::runtime()
        .spawn(async move {
            let prepared = fav::edit_folder_request(media_id, &title, &intro, privacy, &cover);
            fav::send_parsed(&transport, &ctx, &prepared, fav::parse_fav_folder).await
        })
        .await;
    folder_result(flatten(joined))
}

/// 批量收藏 / 取消收藏某个资源。
///
/// `resources` 是 `id:type` 形式，`add_ids` / `del_ids` 是逗号分隔的
/// 收藏夹 id 串（空串表示这一侧不动）。
pub async fn fav_video(
    context: FavContext,
    resources: String,
    add_ids: String,
    del_ids: String,
) -> WriteResult {
    send_plain(context, move || {
        fav::fav_video_request(&resources, &add_ids, &del_ids)
    })
    .await
}

/// 把一个资源从所有收藏夹里移出。
pub async fn unfav_all(context: FavContext, rid: i64, kind: i64) -> WriteResult {
    send_plain(context, move || fav::unfav_all_request(rid, kind)).await
}

/// 在收藏夹之间复制 / 移动资源。
pub async fn copy_or_move_fav(
    context: FavContext,
    mode: FavMoveMode,
    target: FavMoveTarget,
    src_media_id: Option<i64>,
    tar_media_id: i64,
    mid: Option<i64>,
    resources: String,
) -> WriteResult {
    send_plain(context, move || {
        fav::copy_or_move_fav_request(
            mode.into(),
            target.into(),
            src_media_id,
            tar_media_id,
            mid,
            &resources,
        )
    })
    .await
}

// ------------------------------------------------------------------- 内部

fn sub_item_to_bridge(i: bili_domain::user::SubItem) -> SubItem {
    SubItem {
        id: i.id,
        fid: i.fid,
        mid: i.mid,
        attr: i.attr,
        title: i.title,
        cover: i.cover,
        upper_mid: i.upper_mid,
        upper_name: i.upper_name,
        upper_face: i.upper_face,
        cover_type: i.cover_type,
        intro: i.intro,
        ctime: i.ctime,
        mtime: i.mtime,
        state: i.state,
        fav_state: i.fav_state,
        media_count: i.media_count,
        view_count: i.view_count,
        kind: i.kind,
        play_count: i.play_count,
        collect_count: i.collect_count,
    }
}

fn folder_to_bridge(f: bili_domain::fav::FavFolder) -> FavFolder {
    FavFolder {
        id: f.id,
        fid: f.fid,
        mid: f.mid,
        attr: f.attr,
        title: f.title,
        cover: f.cover,
        upper_mid: f.upper_mid,
        upper_name: f.upper_name,
        upper_face: f.upper_face,
        intro: f.intro,
        fav_state: f.fav_state,
        media_count: f.media_count,
    }
}

fn media_to_bridge(m: bili_domain::fav::FavMedia) -> FavMedia {
    FavMedia {
        id: m.id,
        kind: m.kind,
        title: m.title,
        cover: m.cover,
        intro: m.intro,
        duration: m.duration,
        upper_mid: m.upper_mid,
        upper_name: m.upper_name,
        upper_face: m.upper_face,
        attr: m.attr,
        play_count: m.play_count,
        danmaku_count: m.danmaku_count,
        fav_time: m.fav_time,
        bvid: m.bvid,
        ogv_type_name: m.ogv_type_name,
        ogv_season_id: m.ogv_season_id,
        ugc_first_cid: m.ugc_first_cid,
    }
}

fn folder_result(r: Result<bili_domain::fav::FavFolder, DomainError>) -> FavFolderResult {
    match r {
        Ok(f) => FavFolderResult {
            data: Some(folder_to_bridge(f)),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => FavFolderResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 三条收藏夹列表接口共用的发送路径——它们只有查询参数不同。
async fn fetch_folder_list(
    context: FavContext,
    prepared: bili_api::request::PreparedRequest,
) -> FavFolderListResult {
    let (ctx, transport) = context.split();
    let joined =
        init::runtime()
            .spawn(async move {
                fav::fetch(&transport, &ctx, &prepared, fav::parse_fav_folder_list).await
            })
            .await;

    match flatten(joined) {
        Ok(l) => FavFolderListResult {
            data: Some(FavFolderList {
                count: l.count,
                list: l.list.into_iter().map(folder_to_bridge).collect(),
                has_more: l.has_more,
            }),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(e) => FavFolderListResult {
            data: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        },
    }
}

/// 没有载荷的写操作共用的发送路径。不导出给 Dart（没有 `pub`）。
async fn send_plain<F>(context: FavContext, build: F) -> WriteResult
where
    F: FnOnce() -> PreparedWrite + Send + 'static,
{
    let (ctx, transport) = context.split();
    let joined = init::runtime()
        .spawn(async move {
            let prepared = build();
            fav::send_write(&transport, &ctx, &prepared).await
        })
        .await;

    WriteResult::from_result(flatten(joined))
}

/// 把 `spawn` 的 JoinError 折进领域错误。与其余桥接模块同一份写法。
fn flatten<T>(
    joined: Result<Result<T, DomainError>, tokio::task::JoinError>,
) -> Result<T, DomainError> {
    match joined {
        Ok(inner) => inner,
        Err(e) => Err(DomainError::new(
            bili_domain::error::ErrorKind::Network,
            format!("收藏夹任务未完成: {e}"),
        )),
    }
}
