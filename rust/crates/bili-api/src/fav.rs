//! 收藏夹的请求构造与解析（S8 / issue #58）。
//!
//! 对应基线 `FavHttp` 的全部 34 条接口（`lib/http/fav.dart`，本片迁完已删）：
//! 12 条只读、22 条写操作。
//!
//! ## 写操作的定级都写在各自的构造函数上
//!
//! 依据按 ADR-013 决策三那三问来定。这一片里绝大多数是**置为**语义
//! （收藏/取消收藏、加入/移出、排序、把某几个资源放进某个夹子），因此
//! 是 `Idempotent`；只有两条不是，见 [`add_folder_request`] 与
//! [`fav_video_request`] 的文档。
//!
//! ## 一条在别处没遇到过的形状：csrf 被 APP 签名覆盖
//!
//! `sortFav` / `sortFavFolder` 两条在基线里是
//!
//! ```dart
//! Map<String, dynamic> data = {'media_id': mediaId, 'sort': sort, 'csrf': ...};
//! AppSign.appSign(data);   // ← csrf 已经在 map 里了
//! ```
//!
//! 也就是说 **csrf 参与了 `sign` 的计算**。ADR-013 决策四「csrf 由
//! `send_write` 统一补」在这两条上不成立：补在签名之后，签出来的 `sign`
//! 覆盖不到它，服务端一律拒。
//!
//! 所以这两条的构造函数**显式收一个 csrf 参数**，并把自己标成
//! [`CsrfIn::None`]（已经自己放进去了，不要再补一次）。这是全仓库目前
//! 唯一的例外，写在这里而不是悄悄做掉：下一个人看到「构造函数收 csrf」
//! 会以为是没按 ADR 来。
//!
//! ## 另一条：GET 也可能要 csrf
//!
//! 笔记列表（`/x/note/list`、`/x/note/publish/list/user`）是 GET，却把
//! `csrf` 放在查询串里。基线如此，照搬。它因此走 [`PreparedRequest`]，
//! csrf 由构造函数收下——与 `user_list::black_list_request` 同一个形状。

use bili_domain::error::DomainError;
use bili_domain::fav::{
    FavArticleItem, FavArticleList, FavDetail, FavFolder, FavFolderList, FavMedia, FavNote,
    FavPgcItem, FavPgcList, FavPugvItem, FavPugvList, FavSeasonDetail, FavSeasonMedia,
    FavTopicItem, FavTopicList, SpaceFavGroup,
};
use bili_domain::user::{SubItem, SubList};
use serde::Deserialize;
use serde_json::json;

use crate::http::{HttpTransport, Idempotency, RequestContext};
use crate::request::{CsrfIn, PreparedRequest, PreparedWrite, WriteBody};

/// 接口路径。与基线 `lib/http/api.dart` 保持一致。
pub mod path {
    // --- 视频收藏夹 ---
    /// 自己创建的收藏夹，分页。
    pub const USER_FAV_FOLDER: &str = "/x/v3/fav/folder/created/list";
    /// 自己创建的收藏夹，一次给全。同时兼「这个视频收在哪几个夹子里」。
    pub const FAV_FOLDER_ALL: &str = "/x/v3/fav/folder/created/list-all";
    pub const FAV_FOLDER_INFO: &str = "/x/v3/fav/folder/info";
    pub const FAV_RESOURCE_LIST: &str = "/x/v3/fav/resource/list";
    pub const ADD_FOLDER: &str = "/x/v3/fav/folder/add";
    pub const EDIT_FOLDER: &str = "/x/v3/fav/folder/edit";
    pub const DELETE_FOLDER: &str = "/x/v3/fav/folder/del";
    pub const SORT_FAV_FOLDER: &str = "/x/v3/fav/folder/sort";
    pub const SORT_FAV: &str = "/x/v3/fav/resource/sort";
    pub const CLEAN_FAV: &str = "/x/v3/fav/resource/clean";
    pub const FAV_VIDEO: &str = "/x/v3/fav/resource/batch-deal";
    pub const UNFAV_ALL: &str = "/x/v3/fav/resource/unfav-all";
    pub const COPY_FAV: &str = "/x/v3/fav/resource/copy";
    pub const MOVE_FAV: &str = "/x/v3/fav/resource/move";
    pub const COPY_TOVIEW: &str = "/x/v2/history/toview/copy";
    pub const MOVE_TOVIEW: &str = "/x/v2/history/toview/move";
    /// 收藏/取消收藏**别人的**收藏夹。
    pub const FAV_FOLDER: &str = "/x/v3/fav/folder/fav";
    pub const UNFAV_FOLDER: &str = "/x/v3/fav/folder/unfav";
    /// 个人空间的收藏页（收藏夹 + 订阅的合集）。
    pub const SPACE_FAV: &str = "/x/v3/fav/folder/space";

    // --- 合集（season）---
    pub const FAV_SEASON_LIST: &str = "/x/space/fav/season/list";
    pub const FAV_SEASON: &str = "/x/v3/fav/season/fav";
    pub const UNFAV_SEASON: &str = "/x/v3/fav/season/unfav";

    // --- 追番追剧 ---
    pub const FAV_PGC: &str = "/x/space/bangumi/follow/list";

    // --- 课程 ---
    pub const FAV_PUGV: &str = "/pugv/app/web/favorite/page";
    pub const ADD_FAV_PUGV: &str = "/pugv/app/web/favorite/add";
    pub const DEL_FAV_PUGV: &str = "/pugv/app/web/favorite/del";

    // --- 话题 ---
    pub const FAV_TOPIC_LIST: &str = "/x/topic/web/fav/list";
    pub const ADD_FAV_TOPIC: &str = "/x/topic/fav/sub/add";
    pub const DEL_FAV_TOPIC: &str = "/x/topic/fav/sub/cancel";
    pub const LIKE_TOPIC: &str = "/x/topic/like";

    // --- 专栏 / 笔记 ---
    pub const FAV_ARTICLE: &str = "/x/polymer/web-dynamic/v1/opus/feed/fav";
    pub const ADD_FAV_ARTICLE: &str = "/x/article/favorites/add";
    pub const DEL_FAV_ARTICLE: &str = "/x/article/favorites/del";
    pub const COMMUNITY_ACTION: &str = "/x/community/cosmo/interface/simple_action";
    pub const NOTE_LIST: &str = "/x/note/list";
    pub const USER_NOTE_LIST: &str = "/x/note/publish/list/user";
    pub const DEL_NOTE: &str = "/x/note/del";
    pub const DEL_PUBLISH_NOTE: &str = "/x/note/publish/del";
}

/// 基线写死的固定值。照抄，不求甚解——它们是来源标识与客户端标识，
/// 换一个值要么被拒，要么拿到不一样的分页。
mod fixed {
    /// 收藏页一律自称 web 端。基线如此。
    pub const PLATFORM_WEB: &str = "web";
    /// 话题与课程两条列表接口上的来源标识。
    pub const WEB_LOCATION: &str = "333.1387";
    /// 课程列表每页条数。基线写死。
    pub const PUGV_PAGE_SIZE: i64 = 20;
    /// 话题列表每页条数。基线写死。
    pub const TOPIC_PAGE_SIZE: i64 = 24;
    /// 专栏列表每页条数。基线写死。
    pub const ARTICLE_PAGE_SIZE: i64 = 20;
    /// 笔记列表每页条数。基线写死。
    pub const NOTE_PAGE_SIZE: i64 = 10;
    /// 点赞话题时的业务标识。
    pub const TOPIC_BUSINESS: &str = "topic";

    /// 个人空间收藏页那一组移动端标识。
    ///
    /// 这条接口在基线里是**唯一**一条伪装成 APP 客户端的只读收藏接口
    /// （还带 `bili-http-engine: cronet` 与移动端 UA 两条头）。少一个参数
    /// 服务端会换一套分组回来，不会报错。
    pub const APP_BUILD: &str = "8430300";
    pub const APP_VERSION: &str = "8.43.0";
    pub const APP_LOCALE: &str = "zh_CN";
    pub const APP_CHANNEL: &str = "master";
    pub const APP_MOBI_APP: &str = "android";
    pub const APP_PLATFORM: &str = "android";
    pub const APP_STATISTICS: &str = r#"{"appId":1,"platform":3,"version":"8.43.0","abtest":""}"#;
}

/// 收藏夹详情的排序方式。
///
/// 协议取值就是这三个名字的小写形式，与基线 `FavOrderType` 的 `name`
/// 一一对应。做成枚举而不是收一个 `String`：排序名拼错不会报错，只会
/// 拿到默认排序，而「为什么我选了最多播放还是按时间排」极难从现象反推。
///
/// 界面上的标签（「最近收藏」…）**不在这里**：那是 i18n 文案，按
/// AGENTS.md §2 留在宿主。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FavOrder {
    /// 最近收藏。
    #[default]
    Mtime,
    /// 最多播放。
    View,
    /// 最近投稿。
    Pubtime,
}

impl FavOrder {
    /// 协议里的取值。
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Mtime => "mtime",
            Self::View => "view",
            Self::Pubtime => "pubtime",
        }
    }
}

/// APP 签名的材料（`sortFav` / `sortFavFolder` 两条用）。
///
/// 与 [`crate::video_write::AppAuth`] 不是同一个：那边是
/// `app.bilibili.com` 的鉴权（带 `access_key`、不带 Cookie），这边是
/// `api.bilibili.com` 上两条**既要 Cookie 又要 APP 签名**的接口，没有
/// `access_key`。合成一个只会让一半字段常年为空。
#[derive(Debug, Clone)]
pub struct AppSignature {
    pub appkey: String,
    pub appsec: String,
    /// 签名用的 Unix 秒。由调用方注入，使构造保持纯函数、可被快照固定。
    pub ts: i64,
}

// ======================================================= 只读：视频收藏夹

/// 自己创建的收藏夹，分页。
#[must_use]
pub fn fav_folder_list_request(up_mid: i64, pn: i64, ps: i64) -> PreparedRequest {
    PreparedRequest {
        path: path::USER_FAV_FOLDER,
        query: vec![
            ("pn".to_owned(), pn.to_string()),
            ("ps".to_owned(), ps.to_string()),
            ("up_mid".to_owned(), up_mid.to_string()),
        ],
    }
}

/// 自己创建的收藏夹，一次给全（不分页）。
#[must_use]
pub fn all_fav_folders_request(up_mid: i64) -> PreparedRequest {
    PreparedRequest {
        path: path::FAV_FOLDER_ALL,
        query: vec![("up_mid".to_owned(), up_mid.to_string())],
    }
}

/// 「这个资源被收藏在哪几个夹子里」。
///
/// 与 [`all_fav_folders_request`] 是**同一条接口**，多给 `rid` / `type`
/// 两个参数时服务端会在每一项上附带 `fav_state`。基线分成两个函数，这里
/// 也分开——参数不同、调用点不同，合成一个带两个 `Option` 的版本只会让
/// 「忘了传 rid」变成一次静默的全量查询。
#[must_use]
pub fn video_in_folder_request(up_mid: i64, rid: i64, kind: Option<i64>) -> PreparedRequest {
    let mut query = vec![
        ("up_mid".to_owned(), up_mid.to_string()),
        ("rid".to_owned(), rid.to_string()),
    ];
    if let Some(kind) = kind {
        query.push(("type".to_owned(), kind.to_string()));
    }
    PreparedRequest {
        path: path::FAV_FOLDER_ALL,
        query,
    }
}

/// 单个收藏夹的信息。
#[must_use]
pub fn fav_folder_info_request(media_id: i64) -> PreparedRequest {
    PreparedRequest {
        path: path::FAV_FOLDER_INFO,
        query: vec![("media_id".to_owned(), media_id.to_string())],
    }
}

/// 收藏夹详情的一页。`keyword` 为空串表示不筛选（基线如此，不是不传）。
#[must_use]
pub fn fav_detail_request(
    media_id: i64,
    pn: i64,
    ps: i64,
    keyword: &str,
    order: FavOrder,
    kind: i64,
) -> PreparedRequest {
    PreparedRequest {
        path: path::FAV_RESOURCE_LIST,
        query: vec![
            ("media_id".to_owned(), media_id.to_string()),
            ("pn".to_owned(), pn.to_string()),
            ("ps".to_owned(), ps.to_string()),
            ("keyword".to_owned(), keyword.to_owned()),
            ("order".to_owned(), order.as_str().to_owned()),
            ("type".to_owned(), kind.to_string()),
            // 基线写死 0。它是分区筛选，界面上没有入口。
            ("tid".to_owned(), "0".to_owned()),
            ("platform".to_owned(), fixed::PLATFORM_WEB.to_owned()),
        ],
    }
}

/// 合集详情的一页。
#[must_use]
pub fn fav_season_list_request(season_id: i64, pn: i64, ps: i64) -> PreparedRequest {
    PreparedRequest {
        path: path::FAV_SEASON_LIST,
        query: vec![
            ("season_id".to_owned(), season_id.to_string()),
            ("ps".to_owned(), ps.to_string()),
            ("pn".to_owned(), pn.to_string()),
        ],
    }
}

/// 个人空间的收藏页。
///
/// 这条接口伪装成移动端客户端，见 [`fixed`] 里那一组常量。两条请求头
/// （`bili-http-engine: cronet` 与移动端 UA）的真相源在宿主，由调用方
/// 放进 [`RequestContext::headers`]。
#[must_use]
pub fn space_fav_request(up_mid: i64) -> PreparedRequest {
    PreparedRequest {
        path: path::SPACE_FAV,
        query: vec![
            ("build".to_owned(), fixed::APP_BUILD.to_owned()),
            ("version".to_owned(), fixed::APP_VERSION.to_owned()),
            ("c_locale".to_owned(), fixed::APP_LOCALE.to_owned()),
            ("channel".to_owned(), fixed::APP_CHANNEL.to_owned()),
            ("mobi_app".to_owned(), fixed::APP_MOBI_APP.to_owned()),
            ("platform".to_owned(), fixed::APP_PLATFORM.to_owned()),
            ("s_locale".to_owned(), fixed::APP_LOCALE.to_owned()),
            ("statistics".to_owned(), fixed::APP_STATISTICS.to_owned()),
            ("up_mid".to_owned(), up_mid.to_string()),
        ],
    }
}

// ============================================== 只读：追番 / 课程 / 话题 / 专栏

/// 追番/追剧列表的一页。`kind` 为 1 番剧、2 影视。
#[must_use]
pub fn fav_pgc_request(
    vmid: i64,
    kind: i64,
    follow_status: Option<i64>,
    pn: i64,
) -> PreparedRequest {
    let mut query = vec![
        ("vmid".to_owned(), vmid.to_string()),
        ("type".to_owned(), kind.to_string()),
    ];
    if let Some(status) = follow_status {
        query.push(("follow_status".to_owned(), status.to_string()));
    }
    query.push(("pn".to_owned(), pn.to_string()));
    PreparedRequest {
        path: path::FAV_PGC,
        query,
    }
}

/// 收藏的课程一页。
#[must_use]
pub fn fav_pugv_request(mid: i64, pn: i64) -> PreparedRequest {
    PreparedRequest {
        path: path::FAV_PUGV,
        query: vec![
            ("mid".to_owned(), mid.to_string()),
            ("ps".to_owned(), fixed::PUGV_PAGE_SIZE.to_string()),
            ("pn".to_owned(), pn.to_string()),
            ("web_location".to_owned(), fixed::WEB_LOCATION.to_owned()),
        ],
    }
}

/// 收藏的话题一页。
#[must_use]
pub fn fav_topic_request(page: i64) -> PreparedRequest {
    PreparedRequest {
        path: path::FAV_TOPIC_LIST,
        query: vec![
            ("page_size".to_owned(), fixed::TOPIC_PAGE_SIZE.to_string()),
            ("page_num".to_owned(), page.to_string()),
            ("web_location".to_owned(), fixed::WEB_LOCATION.to_owned()),
        ],
    }
}

/// 收藏的专栏一页。
#[must_use]
pub fn fav_article_request(page: i64) -> PreparedRequest {
    PreparedRequest {
        path: path::FAV_ARTICLE,
        query: vec![
            ("page_size".to_owned(), fixed::ARTICLE_PAGE_SIZE.to_string()),
            ("page".to_owned(), page.to_string()),
        ],
    }
}

/// 笔记列表的一页。`published` 为真时取「我发布的笔记」。
///
/// **这条 GET 要带 csrf**（基线把它放在 `queryParameters` 里）。未登录时
/// 传空串即可——与基线一致，服务端会以未登录拒绝。
#[must_use]
pub fn note_list_request(page: i64, published: bool, csrf: &str) -> PreparedRequest {
    PreparedRequest {
        path: if published {
            path::USER_NOTE_LIST
        } else {
            path::NOTE_LIST
        },
        query: vec![
            ("pn".to_owned(), page.to_string()),
            ("ps".to_owned(), fixed::NOTE_PAGE_SIZE.to_string()),
            ("csrf".to_owned(), csrf.to_owned()),
        ],
    }
}

// ================================================================ 写操作

/// 收藏 / 取消收藏**别人的**收藏夹。`fav` 是目标状态。
///
/// 定级 [`Idempotency::Idempotent`]：两条路径各自是置为语义，重发落到
/// 同一个状态。
#[must_use]
pub fn fav_folder_request(media_id: i64, fav: bool) -> PreparedWrite {
    PreparedWrite::form_only(
        if fav {
            path::FAV_FOLDER
        } else {
            path::UNFAV_FOLDER
        },
        vec![("media_id".to_owned(), media_id.to_string())],
        Idempotency::Idempotent,
    )
}

/// 收藏 / 取消收藏一个合集。`fav` 是目标状态。
///
/// 定级 `Idempotent`，同 [`fav_folder_request`]。
#[must_use]
pub fn fav_season_request(season_id: i64, fav: bool) -> PreparedWrite {
    PreparedWrite::form_only(
        if fav {
            path::FAV_SEASON
        } else {
            path::UNFAV_SEASON
        },
        vec![
            ("platform".to_owned(), fixed::PLATFORM_WEB.to_owned()),
            ("season_id".to_owned(), season_id.to_string()),
        ],
        Idempotency::Idempotent,
    )
}

/// 收藏 / 取消收藏一门课程。`fav` 是目标状态。定级 `Idempotent`。
#[must_use]
pub fn fav_pugv_write_request(season_id: i64, fav: bool) -> PreparedWrite {
    PreparedWrite::form_only(
        if fav {
            path::ADD_FAV_PUGV
        } else {
            path::DEL_FAV_PUGV
        },
        vec![("season_id".to_owned(), season_id.to_string())],
        Idempotency::Idempotent,
    )
}

/// 收藏 / 取消收藏一个话题。`fav` 是目标状态。定级 `Idempotent`。
#[must_use]
pub fn fav_topic_write_request(topic_id: i64, fav: bool) -> PreparedWrite {
    PreparedWrite::form_only(
        if fav {
            path::ADD_FAV_TOPIC
        } else {
            path::DEL_FAV_TOPIC
        },
        vec![("topic_id".to_owned(), topic_id.to_string())],
        Idempotency::Idempotent,
    )
}

/// 点赞 / 取消点赞一个话题。`like` 是**目标**状态。
///
/// ## 基线那个参数传的是当前状态
///
/// 基线签名是 `likeTopic(topicId, isLike)`，正文
/// `'action': isLike ? 'cancel_like' : 'like'`，调用点传的是
/// `this.isLike.value`——**当前**状态，成功后界面才 `toggle()`。
///
/// 这里统一成目标状态（与本片其余「`fav` 是目标状态」一致），折算放在
/// 调用点之外。写反了不报错，表现是按钮点一下没反应（ADR-013 补充五）。
///
/// 定级 `Idempotent`：`action` 是目标动作而非增量。
#[must_use]
pub fn like_topic_request(topic_id: i64, up_mid: i64, like: bool) -> PreparedWrite {
    PreparedWrite::form_only(
        path::LIKE_TOPIC,
        vec![
            (
                "action".to_owned(),
                if like { "like" } else { "cancel_like" }.to_owned(),
            ),
            ("up_mid".to_owned(), up_mid.to_string()),
            ("topic_id".to_owned(), topic_id.to_string()),
            ("business".to_owned(), fixed::TOPIC_BUSINESS.to_owned()),
        ],
        Idempotency::Idempotent,
    )
}

/// 收藏 / 取消收藏一篇专栏（旧的 article 接口）。`fav` 是目标状态。
///
/// 定级 `Idempotent`。
#[must_use]
pub fn fav_article_write_request(id: i64, fav: bool) -> PreparedWrite {
    PreparedWrite::form_only(
        if fav {
            path::ADD_FAV_ARTICLE
        } else {
            path::DEL_FAV_ARTICLE
        },
        vec![("id".to_owned(), id.to_string())],
        Idempotency::Idempotent,
    )
}

/// 图文动态（opus）的收藏 / 取消收藏。`action`：3 收藏，4 取消。
///
/// 与 [`fav_article_write_request`] 是两条不同的接口，不是同一件事的两种
/// 写法：专栏收藏页（`/x/polymer/.../opus/feed/fav`）里的项用的是 opus
/// 体系，`opus_id` 是字符串。
///
/// 形状上也是这一片唯一的异类：**体是 JSON、csrf 在查询串上**（基线把
/// csrf 放在 `queryParameters`，`data` 不带 contentType，于是吃 dio 的
/// 默认 JSON）。
///
/// 定级 `Idempotent`：`action` 是目标动作。
#[must_use]
pub fn community_action_request(opus_id: &str, action: i64) -> PreparedWrite {
    PreparedWrite {
        path: path::COMMUNITY_ACTION,
        query: Vec::new(),
        body: WriteBody::Json(json!({
            "entity": {
                "object_id_str": opus_id,
                // biz 2 = 图文动态。基线写死。
                "type": { "biz": 2 },
            },
            "action": action,
        })),
        csrf: CsrfIn::Query,
        idempotency: Idempotency::Idempotent,
    }
}

/// 删除笔记。`published` 为真时删「我发布的」，参数名与接口都不同。
///
/// `ids` 是逗号分隔的一串：界面支持多选批量删。
///
/// 定级 `Idempotent`：删第二次返回「不存在」，最终状态一致。
#[must_use]
pub fn del_note_request(ids: &str, published: bool) -> PreparedWrite {
    PreparedWrite::form_only(
        if published {
            path::DEL_PUBLISH_NOTE
        } else {
            path::DEL_NOTE
        },
        vec![(
            if published { "cvids" } else { "note_ids" }.to_owned(),
            ids.to_owned(),
        )],
        Idempotency::Idempotent,
    )
}

/// 收藏夹排序。`sort` 是逗号分隔的 id 串，顺序即目标顺序。
///
/// **csrf 由本函数放进表单**，因为它被 APP 签名覆盖，见模块文档。
///
/// 定级 `Idempotent`：整串顺序是置为语义。
#[must_use]
pub fn sort_fav_folder_request(sort: &str, csrf: &str, auth: &AppSignature) -> PreparedWrite {
    signed_form(
        path::SORT_FAV_FOLDER,
        vec![("sort".to_owned(), sort.to_owned())],
        csrf,
        auth,
    )
}

/// 收藏夹内容排序。参数同 [`sort_fav_folder_request`]，多一个 `media_id`。
#[must_use]
pub fn sort_fav_request(
    media_id: i64,
    sort: &str,
    csrf: &str,
    auth: &AppSignature,
) -> PreparedWrite {
    signed_form(
        path::SORT_FAV,
        vec![
            ("media_id".to_owned(), media_id.to_string()),
            ("sort".to_owned(), sort.to_owned()),
        ],
        csrf,
        auth,
    )
}

/// 两条 APP 签名写请求共用的形状：csrf 先进表单，再整体签名。
///
/// 顺序是**唯一**正确的那个：签名覆盖排序后的全部参数，csrf 后补等于
/// 签了一份不含它的串，服务端拒。
fn signed_form(
    path: &'static str,
    mut form: Vec<(String, String)>,
    csrf: &str,
    auth: &AppSignature,
) -> PreparedWrite {
    form.push(("csrf".to_owned(), csrf.to_owned()));
    PreparedWrite {
        path,
        query: Vec::new(),
        body: WriteBody::Form(bili_protocol::app_sign::sign(
            &form,
            &auth.appkey,
            &auth.appsec,
            auth.ts,
        )),
        // 已经自己放进去并签过了，再补一次会出现两个 csrf 字段。
        csrf: CsrfIn::None,
        idempotency: Idempotency::Idempotent,
    }
}

/// 清除收藏夹里的失效内容。
///
/// 定级 `Idempotent`：第二次清没有失效内容可清，最终状态一致。
#[must_use]
pub fn clean_fav_request(media_id: i64) -> PreparedWrite {
    PreparedWrite::form_only(
        path::CLEAN_FAV,
        vec![
            ("media_id".to_owned(), media_id.to_string()),
            ("platform".to_owned(), fixed::PLATFORM_WEB.to_owned()),
        ],
        Idempotency::Idempotent,
    )
}

/// 删除收藏夹。`media_ids` 是逗号分隔的一串，支持批量。
///
/// 定级 `Idempotent`：删第二次返回「不存在」。
#[must_use]
pub fn delete_folder_request(media_ids: &str) -> PreparedWrite {
    PreparedWrite::form_only(
        path::DELETE_FOLDER,
        vec![
            ("media_ids".to_owned(), media_ids.to_owned()),
            ("platform".to_owned(), fixed::PLATFORM_WEB.to_owned()),
        ],
        Idempotency::Idempotent,
    )
}

/// 新建收藏夹。
///
/// 定级 [`Idempotency::Unsafe`]——**这一片唯一的一条**。重发会多建一个
/// 同名收藏夹，用户在列表里直接看得见，且无从判定上一次是否已生效
/// （服务端没有「按标题查」的接口，同名收藏夹本来也允许存在）。
/// 按 ADR-013 决策一，`Unsafe` 一律不重试，失败了由用户自己再按一次。
///
/// `cover` 为空串时原样发空串（基线如此）；非空时要 URL 编码整条地址
/// ——它是一段 `https://...` 的 URL，不编码会在表单里被截断。
#[must_use]
pub fn add_folder_request(title: &str, intro: &str, privacy: i64, cover: &str) -> PreparedWrite {
    PreparedWrite::form_only(
        path::ADD_FOLDER,
        folder_form(title, intro, privacy, cover, None),
        Idempotency::Unsafe,
    )
}

/// 编辑收藏夹。
///
/// 定级 `Idempotent`：带 `media_id`，整条是置为语义，重发落到同一个
/// 结果——与 [`add_folder_request`] 的差别正在这里，所以两条分开而不是
/// 一个带 `Option<media_id>` 的函数：合成一个的话幂等级别只能取其中更
/// 保守的那一档，编辑就白白失去了重试。
#[must_use]
pub fn edit_folder_request(
    media_id: i64,
    title: &str,
    intro: &str,
    privacy: i64,
    cover: &str,
) -> PreparedWrite {
    PreparedWrite::form_only(
        path::EDIT_FOLDER,
        folder_form(title, intro, privacy, cover, Some(media_id)),
        Idempotency::Idempotent,
    )
}

/// 新建与编辑共用的表单。字段顺序照搬基线。
fn folder_form(
    title: &str,
    intro: &str,
    privacy: i64,
    cover: &str,
    media_id: Option<i64>,
) -> Vec<(String, String)> {
    let mut form = vec![
        ("title".to_owned(), title.to_owned()),
        ("intro".to_owned(), intro.to_owned()),
        ("privacy".to_owned(), privacy.to_string()),
        (
            "cover".to_owned(),
            if cover.is_empty() {
                String::new()
            } else {
                // 基线 `Uri.encodeFull`：只编码非法字符，保留 `:/?#` 等
                // URL 结构字符。与 `encodeComponent` 不是一回事，换成后者
                // 会把整条地址编成一个不可用的串。
                encode_full(cover)
            },
        ),
    ];
    if let Some(media_id) = media_id {
        form.push(("media_id".to_owned(), media_id.to_string()));
    }
    form
}

/// Dart `Uri.encodeFull` 的等价实现。
///
/// 保留集：`A-Za-z0-9` 与 `-_.!~*'()` 以及 URL 结构字符
/// `#$&+,/:;=?@`。也就是说它只把空格、中文、引号这类真正非法的字符
/// 编码掉，整条 URL 的形状保持可读。
fn encode_full(value: &str) -> String {
    use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};
    const SET: &AsciiSet = &NON_ALPHANUMERIC
        .remove(b'-')
        .remove(b'_')
        .remove(b'.')
        .remove(b'!')
        .remove(b'~')
        .remove(b'*')
        .remove(b'\'')
        .remove(b'(')
        .remove(b')')
        .remove(b'#')
        .remove(b'$')
        .remove(b'&')
        .remove(b'+')
        .remove(b',')
        .remove(b'/')
        .remove(b':')
        .remove(b';')
        .remove(b'=')
        .remove(b'?')
        .remove(b'@');
    utf8_percent_encode(value, SET).to_string()
}

/// 批量收藏 / 取消收藏某个资源。
///
/// `resources` 是 `id:type` 形式（例 `114514:2`），`add_ids` / `del_ids`
/// 是逗号分隔的收藏夹 id 串。
///
/// 定级 [`Idempotency::Idempotent`]：服务端语义是「把这个资源**放进**
/// 这几个夹子、**移出**那几个夹子」，是置为而不是加一次——同一个资源在
/// 一个夹子里只会有一份，重发落到同一个状态。
#[must_use]
pub fn fav_video_request(resources: &str, add_ids: &str, del_ids: &str) -> PreparedWrite {
    PreparedWrite::form_only(
        path::FAV_VIDEO,
        vec![
            ("resources".to_owned(), resources.to_owned()),
            ("add_media_ids".to_owned(), add_ids.to_owned()),
            ("del_media_ids".to_owned(), del_ids.to_owned()),
        ],
        Idempotency::Idempotent,
    )
}

/// 把一个资源从**所有**收藏夹里移出。定级 `Idempotent`。
#[must_use]
pub fn unfav_all_request(rid: i64, kind: i64) -> PreparedWrite {
    PreparedWrite::form_only(
        path::UNFAV_ALL,
        vec![
            ("rid".to_owned(), rid.to_string()),
            ("type".to_owned(), kind.to_string()),
        ],
        Idempotency::Idempotent,
    )
}

/// 复制 / 移动的目标体系。
///
/// 基线把这件事写成两个布尔（`isCopy` / `isFav`）嵌套三目，四个分支选出
/// 四条路径。换成两个枚举：两个 `bool` 相邻传反了编译器不会说话，而
/// 「复制到稍后再看」与「移动到收藏夹」是两条完全不同的接口。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FavMoveTarget {
    /// 收藏夹之间。
    Folder,
    /// 稍后再看。
    ToView,
}

/// 复制还是移动。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FavMoveMode {
    Copy,
    Move,
}

/// 在收藏夹之间复制 / 移动资源。
///
/// `src_media_id` 与 `mid` 可缺省（基线用 `?` 让它们为 null 时整个键不
/// 出现）：从「稍后再看」复制过来时没有来源夹子。
///
/// 定级 [`Idempotency::Idempotent`]：目标夹子里同一个资源只会有一份，
/// 复制第二次落到同一个状态；移动第二次则源头已空，服务端返回「不存在」，
/// 最终状态仍然一致。
#[must_use]
pub fn copy_or_move_fav_request(
    mode: FavMoveMode,
    target: FavMoveTarget,
    src_media_id: Option<i64>,
    tar_media_id: i64,
    mid: Option<i64>,
    resources: &str,
) -> PreparedWrite {
    let path = match (target, mode) {
        (FavMoveTarget::Folder, FavMoveMode::Copy) => path::COPY_FAV,
        (FavMoveTarget::Folder, FavMoveMode::Move) => path::MOVE_FAV,
        (FavMoveTarget::ToView, FavMoveMode::Copy) => path::COPY_TOVIEW,
        (FavMoveTarget::ToView, FavMoveMode::Move) => path::MOVE_TOVIEW,
    };

    let mut form = Vec::with_capacity(6);
    if let Some(src) = src_media_id {
        form.push(("src_media_id".to_owned(), src.to_string()));
    }
    form.push(("tar_media_id".to_owned(), tar_media_id.to_string()));
    if let Some(mid) = mid {
        form.push(("mid".to_owned(), mid.to_string()));
    }
    form.push(("resources".to_owned(), resources.to_owned()));
    form.push(("platform".to_owned(), fixed::PLATFORM_WEB.to_owned()));

    PreparedWrite::form_only(path, form, Idempotency::Idempotent)
}

// ================================================================== 解析

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct UpperDto {
    mid: Option<i64>,
    name: Option<String>,
    face: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct CntInfoDto {
    play: Option<i64>,
    danmaku: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct FavFolderDto {
    id: Option<i64>,
    fid: Option<i64>,
    mid: Option<i64>,
    attr: Option<i64>,
    title: Option<String>,
    cover: Option<String>,
    upper: Option<UpperDto>,
    intro: Option<String>,
    fav_state: Option<i64>,
    media_count: Option<i64>,
}

impl FavFolderDto {
    fn into_domain(self) -> FavFolder {
        FavFolder {
            id: self.id.unwrap_or_default(),
            fid: self.fid,
            mid: self.mid.unwrap_or_default(),
            attr: self.attr.unwrap_or_default(),
            title: self.title.unwrap_or_default(),
            cover: self.cover.unwrap_or_default(),
            upper_mid: self.upper.as_ref().and_then(|u| u.mid),
            upper_name: self.upper.as_ref().and_then(|u| u.name.clone()),
            upper_face: self.upper.and_then(|u| u.face),
            intro: self.intro,
            fav_state: self.fav_state,
            media_count: self.media_count.unwrap_or_default(),
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct FavFolderListDto {
    count: Option<i64>,
    list: Option<Vec<FavFolderDto>>,
    has_more: Option<bool>,
}

/// 解析收藏夹列表。
///
/// # Errors
/// 信封失败沿用统一分类。`list` 缺失按空列表处理。
pub fn parse_fav_folder_list(body: &str) -> Result<FavFolderList, DomainError> {
    let dto: FavFolderListDto = crate::envelope::unwrap_envelope(body)?;
    Ok(FavFolderList {
        count: dto.count,
        list: dto
            .list
            .unwrap_or_default()
            .into_iter()
            .map(FavFolderDto::into_domain)
            .collect(),
        has_more: dto.has_more,
    })
}

/// 解析单个收藏夹的信息。新建/编辑收藏夹的响应也是这个形状。
///
/// # Errors
/// 见 [`parse_fav_folder_list`]。
pub fn parse_fav_folder(body: &str) -> Result<FavFolder, DomainError> {
    let dto: FavFolderDto = crate::envelope::unwrap_envelope(body)?;
    Ok(dto.into_domain())
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct OgvDto {
    type_name: Option<String>,
    season_id: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct UgcDto {
    first_cid: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct FavMediaDto {
    id: Option<i64>,
    #[serde(rename = "type")]
    kind: Option<i64>,
    title: Option<String>,
    cover: Option<String>,
    intro: Option<String>,
    duration: Option<i64>,
    upper: Option<UpperDto>,
    attr: Option<i64>,
    cnt_info: Option<CntInfoDto>,
    fav_time: Option<i64>,
    bvid: Option<String>,
    /// 同一个字段的另一个拼法，见 [`FavMedia::bvid`]。
    bv_id: Option<String>,
    ogv: Option<OgvDto>,
    ugc: Option<UgcDto>,
}

impl FavMediaDto {
    fn into_domain(self) -> FavMedia {
        FavMedia {
            id: self.id,
            kind: self.kind,
            title: self.title,
            cover: self.cover,
            intro: self.intro,
            duration: self.duration,
            upper_mid: self.upper.as_ref().and_then(|u| u.mid),
            upper_name: self.upper.as_ref().and_then(|u| u.name.clone()),
            upper_face: self.upper.and_then(|u| u.face),
            attr: self.attr,
            play_count: self.cnt_info.as_ref().and_then(|c| c.play),
            danmaku_count: self.cnt_info.and_then(|c| c.danmaku),
            fav_time: self.fav_time,
            bvid: self.bvid.or(self.bv_id),
            ogv_type_name: self.ogv.as_ref().and_then(|o| o.type_name.clone()),
            ogv_season_id: self.ogv.and_then(|o| o.season_id),
            ugc_first_cid: self.ugc.and_then(|u| u.first_cid),
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct FavDetailDto {
    info: Option<FavFolderDto>,
    medias: Option<Vec<FavMediaDto>>,
    has_more: Option<bool>,
}

/// 解析收藏夹详情的一页。
///
/// `page_desc` 为真时**整页倒序**。这是基线
/// `FavDetailData.fromJson(..., pageDesc:)` 的行为：收藏夹详情页支持把
/// 分页方向反过来（从最后一页开始往前翻），此时每一页内部也要反过来
/// 才接得上。
///
/// 它看起来像展示规则，但按 AGENTS.md §2 的判据——换掉宿主框架时这段
/// 要不要重写——答案是「要」，所以它归 Rust。
///
/// # Errors
/// 见 [`parse_fav_folder_list`]。
pub fn parse_fav_detail(body: &str, page_desc: bool) -> Result<FavDetail, DomainError> {
    let dto: FavDetailDto = crate::envelope::unwrap_envelope(body)?;
    let mut medias: Vec<FavMedia> = dto
        .medias
        .unwrap_or_default()
        .into_iter()
        .map(FavMediaDto::into_domain)
        .collect();
    if page_desc {
        medias.reverse();
    }
    Ok(FavDetail {
        info: dto.info.map(FavFolderDto::into_domain),
        medias,
        has_more: dto.has_more,
    })
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct NewEpDto {
    index_show: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct FavPgcItemDto {
    season_id: Option<i64>,
    title: Option<String>,
    cover: Option<String>,
    is_finish: Option<i64>,
    badge: Option<String>,
    new_ep: Option<NewEpDto>,
    renewal_time: Option<String>,
    progress: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct FavPgcListDto {
    list: Option<Vec<FavPgcItemDto>>,
    total: Option<i64>,
}

/// 解析追番/追剧列表。
///
/// # Errors
/// 见 [`parse_fav_folder_list`]。
pub fn parse_fav_pgc(body: &str) -> Result<FavPgcList, DomainError> {
    let dto: FavPgcListDto = crate::envelope::unwrap_envelope(body)?;
    Ok(FavPgcList {
        list: dto
            .list
            .unwrap_or_default()
            .into_iter()
            .map(|i| FavPgcItem {
                season_id: i.season_id,
                title: i.title,
                cover: i.cover,
                is_finish: i.is_finish,
                badge: i.badge,
                new_ep_index_show: i.new_ep.and_then(|e| e.index_show),
                renewal_time: i.renewal_time,
                // 空串归一成 None，见 FavPgcItem::progress。
                progress: i.progress.filter(|p| !p.is_empty()),
            })
            .collect(),
        total: dto.total,
    })
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct PugvPageDto {
    next: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct FavPugvItemDto {
    cover: Option<String>,
    marks: Option<Vec<String>>,
    season_id: Option<i64>,
    status: Option<String>,
    title: Option<String>,
    ctime: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct FavPugvListDto {
    items: Option<Vec<FavPugvItemDto>>,
    page: Option<PugvPageDto>,
}

/// 解析收藏的课程列表。
///
/// # Errors
/// 见 [`parse_fav_folder_list`]。
pub fn parse_fav_pugv(body: &str) -> Result<FavPugvList, DomainError> {
    let dto: FavPugvListDto = crate::envelope::unwrap_envelope(body)?;
    Ok(FavPugvList {
        items: dto
            .items
            .unwrap_or_default()
            .into_iter()
            .map(|i| FavPugvItem {
                cover: i.cover,
                marks: i.marks.unwrap_or_default(),
                season_id: i.season_id,
                status: i.status,
                title: i.title,
                ctime: i.ctime,
            })
            .collect(),
        has_next: dto.page.and_then(|p| p.next),
    })
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct TopicPageInfoDto {
    total: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct TopicItemDto {
    id: Option<i64>,
    name: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct TopicListDto {
    topic_items: Option<Vec<TopicItemDto>>,
    page_info: Option<TopicPageInfoDto>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct FavTopicDto {
    topic_list: Option<TopicListDto>,
}

/// 解析收藏的话题列表。
///
/// # Errors
/// 见 [`parse_fav_folder_list`]。
pub fn parse_fav_topic(body: &str) -> Result<FavTopicList, DomainError> {
    let dto: FavTopicDto = crate::envelope::unwrap_envelope(body)?;
    let list = dto.topic_list.unwrap_or_default();
    Ok(FavTopicList {
        items: list
            .topic_items
            .unwrap_or_default()
            .into_iter()
            .map(|i| FavTopicItem {
                id: i.id,
                name: i.name,
            })
            .collect(),
        total: list.page_info.and_then(|p| p.total),
    })
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ArticleAuthorDto {
    name: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ArticleCoverDto {
    url: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ArticleStatDto {
    like: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct FavArticleItemDto {
    opus_id: Option<String>,
    content: Option<String>,
    author: Option<ArticleAuthorDto>,
    cover: Option<ArticleCoverDto>,
    stat: Option<ArticleStatDto>,
    pub_time: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct FavArticleListDto {
    items: Option<Vec<FavArticleItemDto>>,
    has_more: Option<bool>,
}

/// 解析收藏的专栏列表。
///
/// # Errors
/// 见 [`parse_fav_folder_list`]。
pub fn parse_fav_article(body: &str) -> Result<FavArticleList, DomainError> {
    let dto: FavArticleListDto = crate::envelope::unwrap_envelope(body)?;
    Ok(FavArticleList {
        items: dto
            .items
            .unwrap_or_default()
            .into_iter()
            .map(|i| FavArticleItem {
                opus_id: i.opus_id,
                content: i.content,
                author_name: i.author.and_then(|a| a.name),
                cover_url: i.cover.and_then(|c| c.url),
                like: i.stat.and_then(|s| s.like),
                pub_time: i.pub_time,
            })
            .collect(),
        has_more: dto.has_more,
    })
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct NoteArcDto {
    pic: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct FavNoteDto {
    web_url: Option<String>,
    title: Option<String>,
    summary: Option<String>,
    message: Option<String>,
    arc: Option<NoteArcDto>,
    cvid: Option<serde_json::Value>,
    note_id: Option<serde_json::Value>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct FavNoteListDto {
    list: Option<Vec<FavNoteDto>>,
}

/// 把一个可能是数字、可能是字符串的 id 统一成字符串。
///
/// 笔记 id 在基线里是 `dynamic`，直接 `join(',')` 发回去。服务端两种形态
/// 都见过，而它超出 JS 安全整数范围——按 `i64` 收再格式化回去是对的，
/// 但按 `f64` 收会丢位。用 [`serde_json::Value`] 分情况取，不整数化。
fn id_to_string(v: Option<serde_json::Value>) -> Option<String> {
    match v? {
        serde_json::Value::String(s) => Some(s),
        serde_json::Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

/// 解析笔记列表。
///
/// **`data` 可能整个缺失**：基线写的是 `res.data['data']?['list']`，并把
/// 结果声明成 `List<FavNoteItemModel>?`。所以这里用
/// [`crate::envelope::unwrap_envelope_nullable`]，缺 data 按空列表处理，
/// 不判协议错误。
///
/// # Errors
/// 信封失败沿用统一分类。
pub fn parse_fav_notes(body: &str) -> Result<Vec<FavNote>, DomainError> {
    let dto: Option<FavNoteListDto> = crate::envelope::unwrap_envelope_nullable(body)?;
    Ok(dto
        .unwrap_or_default()
        .list
        .unwrap_or_default()
        .into_iter()
        .map(|i| FavNote {
            web_url: i.web_url,
            title: i.title,
            summary: i.summary,
            message: i.message,
            pic: i.arc.and_then(|a| a.pic),
            cvid: id_to_string(i.cvid),
            note_id: id_to_string(i.note_id),
        })
        .collect())
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct MediaListResponseDto {
    count: Option<i64>,
    list: Option<Vec<SubInfoDto>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SpaceFavGroupDto {
    id: Option<i64>,
    name: Option<String>,
    /// 驼峰。这条接口是移动端形态，键名不跟 web 端的下划线走。
    #[serde(rename = "mediaListResponse")]
    media_list_response: Option<MediaListResponseDto>,
}

/// 解析个人空间的收藏页。
///
/// **载荷是一个数组而不是对象**，且可能为 null（这个人没有公开收藏）。
///
/// # Errors
/// 信封失败沿用统一分类。
pub fn parse_space_fav(body: &str) -> Result<Vec<SpaceFavGroup>, DomainError> {
    let dto: Option<Vec<SpaceFavGroupDto>> = crate::envelope::unwrap_envelope_nullable(body)?;
    Ok(dto
        .unwrap_or_default()
        .into_iter()
        .map(|g| {
            let media = g.media_list_response.unwrap_or_default();
            SpaceFavGroup {
                id: g.id,
                name: g.name,
                count: media.count,
                list: media
                    .list
                    .unwrap_or_default()
                    .into_iter()
                    .map(SubInfoDto::into_sub_item)
                    .collect(),
            }
        })
        .collect())
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SubInfoDto {
    id: Option<i64>,
    fid: Option<i64>,
    mid: Option<i64>,
    attr: Option<i64>,
    title: Option<String>,
    cover: Option<String>,
    upper: Option<UpperDto>,
    cover_type: Option<i64>,
    intro: Option<String>,
    ctime: Option<i64>,
    mtime: Option<i64>,
    state: Option<i64>,
    fav_state: Option<i64>,
    media_count: Option<i64>,
    view_count: Option<i64>,
    #[serde(rename = "type")]
    kind: Option<i64>,
    cnt_info: Option<CntInfoDto>,
}

impl SubInfoDto {
    fn into_sub_item(self) -> SubItem {
        SubItem {
            id: self.id,
            fid: self.fid,
            mid: self.mid,
            attr: self.attr,
            title: self.title,
            cover: self.cover,
            upper_mid: self.upper.as_ref().and_then(|u| u.mid),
            upper_name: self.upper.as_ref().and_then(|u| u.name.clone()),
            upper_face: self.upper.and_then(|u| u.face),
            cover_type: self.cover_type,
            intro: self.intro,
            ctime: self.ctime,
            mtime: self.mtime,
            state: self.state,
            fav_state: self.fav_state,
            media_count: self.media_count,
            view_count: self.view_count,
            kind: self.kind,
            play_count: self.cnt_info.as_ref().and_then(|c| c.play),
            // 基线 `SubItemModel` 把 `cnt_info.danmaku` 当「收藏数」读
            // （`collectCount`）。字段名对不上，但那是协议这么给的，照搬。
            collect_count: self.cnt_info.and_then(|c| c.danmaku),
        }
    }
}

/// 收藏夹列表的另一种解法：把同一份响应读成 [`SubItem`]。
///
/// 个人空间的「收藏」页要的是这个形状——它那两组（收藏夹 / 订阅的合集）
/// 共用一套卡片，而卡片里「合集」那一支点开要把 [`SubItem`] 带去详情页。
/// 基线在那一页也正是拿 `SpaceFavItemModel`（`SubItemModel` 的子类）解同
/// 一条 `/x/v3/fav/folder/created/list` 响应的，不是两条接口。
///
/// 不把 [`parse_fav_folder_list`] 改成返回这个形状：别的七处调用点要的是
/// [`FavFolder`]，那边有默认值（`id = 0`、`title = ''`）而这边全是
/// `Option`，合并会让两边都得迁就对方。
///
/// # Errors
/// 信封失败沿用统一分类。
pub fn parse_fav_folder_sub_list(body: &str) -> Result<SubList, DomainError> {
    let dto: FavFolderSubListDto = crate::envelope::unwrap_envelope(body)?;
    Ok(SubList {
        count: dto.count,
        has_more: dto.has_more,
        list: dto
            .list
            .unwrap_or_default()
            .into_iter()
            .map(SubInfoDto::into_sub_item)
            .collect(),
    })
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct FavFolderSubListDto {
    count: Option<i64>,
    has_more: Option<bool>,
    list: Option<Vec<SubInfoDto>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct FavSeasonMediaDto {
    id: Option<i64>,
    title: Option<String>,
    cover: Option<String>,
    duration: Option<i64>,
    pubtime: Option<i64>,
    bvid: Option<String>,
    cnt_info: Option<CntInfoDto>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct FavSeasonDetailDto {
    info: Option<SubInfoDto>,
    medias: Option<Vec<FavSeasonMediaDto>>,
}

/// 解析合集详情的一页。`page_desc` 的含义同 [`parse_fav_detail`]。
///
/// `info` 还原成 [`SubItem`]——与订阅列表页那一项是同一个形状，两处喂
/// 同一个界面。
///
/// # Errors
/// 见 [`parse_fav_folder_list`]。
pub fn parse_fav_season_detail(
    body: &str,
    page_desc: bool,
) -> Result<FavSeasonDetail, DomainError> {
    let dto: FavSeasonDetailDto = crate::envelope::unwrap_envelope(body)?;
    let mut medias: Vec<FavSeasonMedia> = dto
        .medias
        .unwrap_or_default()
        .into_iter()
        .map(|m| FavSeasonMedia {
            id: m.id,
            title: m.title,
            cover: m.cover,
            duration: m.duration,
            pubtime: m.pubtime,
            bvid: m.bvid,
            play_count: m.cnt_info.as_ref().and_then(|c| c.play),
            danmaku_count: m.cnt_info.and_then(|c| c.danmaku),
        })
        .collect();
    if page_desc {
        medias.reverse();
    }
    Ok(FavSeasonDetail {
        info: dto.info.map(SubInfoDto::into_sub_item),
        medias,
    })
}

// ================================================================== 发送

/// 发一条只读请求并解析。
///
/// # Errors
/// 传输、状态码、信封三类失败，外加 `parse` 自己的失败。
pub async fn fetch<T, R, F>(
    transport: &T,
    ctx: &RequestContext,
    prepared: &PreparedRequest,
    parse: F,
) -> Result<R, DomainError>
where
    T: HttpTransport,
    F: FnOnce(&str) -> Result<R, DomainError>,
{
    let body = crate::client::send(transport, ctx, prepared).await?;
    parse(&body)
}

/// 发一条没有载荷的写请求。
///
/// # Errors
/// 传输、状态码、信封三类失败。
pub async fn send_write<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    prepared: &PreparedWrite,
) -> Result<(), DomainError> {
    let body = crate::client::send_write(transport, ctx, prepared).await?;
    crate::envelope::check_envelope(&body)
}

/// 发一条写请求并解析它的载荷（新建/编辑收藏夹）。
///
/// # Errors
/// 见 [`send_write`]，外加 `parse` 自己的失败。
pub async fn send_parsed<T, R, F>(
    transport: &T,
    ctx: &RequestContext,
    prepared: &PreparedWrite,
    parse: F,
) -> Result<R, DomainError>
where
    T: HttpTransport,
    F: FnOnce(&str) -> Result<R, DomainError>,
{
    let body = crate::client::send_write(transport, ctx, prepared).await?;
    parse(&body)
}

#[cfg(test)]
// 测试里 expect 是正确做法：前提不成立时就该崩掉并指出原因。
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    /// 占位 appkey / appsec。前者是公开的移动端标识，后者取基线同一对值
    /// 的形状而非真值——签名测试只需确定性，不需要真能过服务端校验。
    const APPKEY: &str = "dfca71928277209b";
    const APPSEC: &str = "0000000000000000000000000000000f";

    fn auth() -> AppSignature {
        AppSignature {
            appkey: APPKEY.to_owned(),
            appsec: APPSEC.to_owned(),
            ts: 1_700_000_000,
        }
    }

    #[test]
    fn fav_detail_request_carries_baseline_fixed_params() {
        let req = fav_detail_request(123, 1, 20, "", FavOrder::View, 0);
        assert_eq!(req.path, path::FAV_RESOURCE_LIST);
        let q = req.query_string();
        assert!(q.contains("order=view"), "{q}");
        assert!(q.contains("tid=0"), "{q}");
        assert!(q.contains("platform=web"), "{q}");
    }

    #[test]
    fn order_values_are_pinned() {
        // 拼错一个不会报错，只会悄悄拿到默认排序。
        assert_eq!(FavOrder::Mtime.as_str(), "mtime");
        assert_eq!(FavOrder::View.as_str(), "view");
        assert_eq!(FavOrder::Pubtime.as_str(), "pubtime");
    }

    #[test]
    fn like_topic_maps_target_state_not_current() {
        // 基线传的是当前状态，这里是目标状态；反了的表现是「点一下没反应」。
        let like = like_topic_request(7, 42, true);
        assert!(like.form_string().contains("action=like"));
        let cancel = like_topic_request(7, 42, false);
        assert!(cancel.form_string().contains("action=cancel_like"));
    }

    #[test]
    fn signed_sort_covers_csrf() {
        // csrf 必须在签名之前进表单：补在后面，sign 覆盖不到它，服务端拒。
        let req = sort_fav_request(9, "3,1,2", "deadbeef", &auth());
        let fields = req.form_fields().expect("排序请求是表单体");
        let keys: Vec<&str> = fields.iter().map(|(k, _)| k.as_str()).collect();
        assert!(keys.contains(&"csrf"), "{keys:?}");
        assert_eq!(keys.last().copied(), Some("sign"), "sign 必须在最后");
        // 位置：csrf 在 sign 之前。
        let csrf_at = keys.iter().position(|k| *k == "csrf").expect("有 csrf");
        let sign_at = keys.len() - 1;
        assert!(csrf_at < sign_at);
        assert_eq!(req.csrf, CsrfIn::None, "已经自己放进去了，不能再补一次");
    }

    #[test]
    fn signed_sort_is_deterministic() {
        let a = sort_fav_folder_request("1,2", "tok", &auth());
        let b = sort_fav_folder_request("1,2", "tok", &auth());
        assert_eq!(a, b, "相同输入必须产出相同签名，否则无法被快照锁死");
    }

    #[test]
    fn add_folder_is_unsafe_edit_is_idempotent() {
        // 这一格是本片唯一的 Unsafe：重发会多建一个同名收藏夹。
        assert_eq!(
            add_folder_request("t", "", 0, "").idempotency,
            Idempotency::Unsafe
        );
        assert_eq!(
            edit_folder_request(1, "t", "", 0, "").idempotency,
            Idempotency::Idempotent
        );
    }

    #[test]
    fn folder_cover_is_encoded_but_stays_a_url() {
        let req = edit_folder_request(1, "t", "", 0, "https://i0.hdslb.com/a b.jpg");
        let form = req.form_string();
        // encodeFull 保留 `:/`，只编码空格。换成 encodeComponent 会把整条
        // 地址编成不可用的串。
        assert!(
            form.contains("cover=https://i0.hdslb.com/a%20b.jpg"),
            "{form}"
        );
    }

    #[test]
    fn empty_cover_stays_empty() {
        let req = add_folder_request("t", "", 0, "");
        assert!(
            req.form_string().ends_with("cover="),
            "{}",
            req.form_string()
        );
    }

    #[test]
    fn community_action_puts_csrf_in_query_with_json_body() {
        let req = community_action_request("114514", 4);
        assert_eq!(req.csrf, CsrfIn::Query);
        assert!(matches!(req.body, WriteBody::Json(_)));
        // object_id_str 必须是字符串：这个 id 超出 JS 安全整数范围。
        assert!(req.form_string().contains("\"object_id_str\":\"114514\""));
    }

    #[test]
    fn copy_or_move_picks_four_distinct_paths() {
        let p =
            |mode, target| copy_or_move_fav_request(mode, target, Some(1), 2, Some(3), "4:2").path;
        assert_eq!(p(FavMoveMode::Copy, FavMoveTarget::Folder), path::COPY_FAV);
        assert_eq!(p(FavMoveMode::Move, FavMoveTarget::Folder), path::MOVE_FAV);
        assert_eq!(
            p(FavMoveMode::Copy, FavMoveTarget::ToView),
            path::COPY_TOVIEW
        );
        assert_eq!(
            p(FavMoveMode::Move, FavMoveTarget::ToView),
            path::MOVE_TOVIEW
        );
    }

    #[test]
    fn copy_or_move_omits_absent_optional_ids() {
        let req = copy_or_move_fav_request(
            FavMoveMode::Copy,
            FavMoveTarget::Folder,
            None,
            2,
            None,
            "4:2",
        );
        let form = req.form_string();
        assert!(!form.contains("src_media_id"), "{form}");
        assert!(!form.contains("mid="), "{form}");
    }

    #[test]
    fn media_accepts_both_bvid_spellings() {
        let a = parse_fav_detail(
            r#"{"code":0,"data":{"medias":[{"id":1,"bvid":"BV1x"}]}}"#,
            false,
        )
        .expect("解析成功");
        assert_eq!(a.medias[0].bvid.as_deref(), Some("BV1x"));

        let b = parse_fav_detail(
            r#"{"code":0,"data":{"medias":[{"id":1,"bv_id":"BV1y"}]}}"#,
            false,
        )
        .expect("解析成功");
        assert_eq!(
            b.medias[0].bvid.as_deref(),
            Some("BV1y"),
            "少认一个的表现是卡片点不开"
        );
    }

    #[test]
    fn page_desc_reverses_the_page() {
        let body = r#"{"code":0,"data":{"medias":[{"id":1},{"id":2},{"id":3}]}}"#;
        let asc = parse_fav_detail(body, false).expect("解析成功");
        let desc = parse_fav_detail(body, true).expect("解析成功");
        assert_eq!(
            asc.medias.iter().map(|m| m.id).collect::<Vec<_>>(),
            [Some(1), Some(2), Some(3)]
        );
        assert_eq!(
            desc.medias.iter().map(|m| m.id).collect::<Vec<_>>(),
            [Some(3), Some(2), Some(1)]
        );
    }

    #[test]
    fn empty_progress_becomes_none() {
        // 不归一的话每张卡片底下都会多出一条空白。
        let list =
            parse_fav_pgc(r#"{"code":0,"data":{"list":[{"progress":""}]}}"#).expect("解析成功");
        assert_eq!(list.list[0].progress, None);
    }

    #[test]
    fn missing_note_data_is_an_empty_list_not_an_error() {
        // 基线写的是 `res.data['data']?['list']`，缺 data 不算失败。
        let notes = parse_fav_notes(r#"{"code":0}"#).expect("缺 data 不该判错");
        assert!(notes.is_empty());
    }

    #[test]
    fn note_ids_keep_their_precision() {
        let notes = parse_fav_notes(
            r#"{"code":0,"data":{"list":[{"cvid":"9007199254740993","note_id":9007199254740993}]}}"#,
        )
        .expect("解析成功");
        assert_eq!(notes[0].cvid.as_deref(), Some("9007199254740993"));
        assert_eq!(notes[0].note_id.as_deref(), Some("9007199254740993"));
    }

    #[test]
    fn null_space_fav_is_an_empty_list() {
        let groups = parse_space_fav(r#"{"code":0,"data":null}"#).expect("null 不该判错");
        assert!(groups.is_empty());
    }

    #[test]
    fn folder_defaults_match_the_baseline() {
        // 基线 FavFolderInfo 给了默认值，界面直接读不判空。
        let folder = parse_fav_folder(r#"{"code":0,"data":{}}"#).expect("解析成功");
        assert_eq!(folder.id, 0);
        assert_eq!(folder.title, "");
        assert_eq!(folder.media_count, 0);
    }
}

#[cfg(test)]
// 测试里 expect 是正确做法：前提不成立时就该崩掉并指出原因。
#[allow(clippy::expect_used)]
mod write_tests {
    use super::*;

    /// 22 条写操作的**请求面快照**（ADR-013 决策六）。
    ///
    /// 写操作的响应通常只有 `{"code":0}`，冻结它证明不了什么；真正会错、
    /// 而且错了要到真机上才暴露的是请求面：路径、字段名、取值、csrf 补在
    /// 哪里、体是表单还是 JSON。
    ///
    /// 做成一张表而不是 22 个 `#[test]`：这一片的写操作形状高度重复，分开
    /// 写的话加一条接口很容易漏掉快照，而漏掉不会报错。
    #[test]
    fn write_request_shapes_are_pinned() {
        let cases: Vec<(&str, PreparedWrite, &str, &str)> = vec![
            (
                "收藏别人的收藏夹",
                fav_folder_request(2101, true),
                path::FAV_FOLDER,
                "media_id=2101",
            ),
            (
                "取消收藏别人的收藏夹",
                fav_folder_request(2101, false),
                path::UNFAV_FOLDER,
                "media_id=2101",
            ),
            (
                "订阅合集",
                fav_season_request(990001, true),
                path::FAV_SEASON,
                "platform=web&season_id=990001",
            ),
            (
                "取消订阅合集",
                fav_season_request(990001, false),
                path::UNFAV_SEASON,
                "platform=web&season_id=990001",
            ),
            (
                "收藏课程",
                fav_pugv_write_request(77000001, true),
                path::ADD_FAV_PUGV,
                "season_id=77000001",
            ),
            (
                "取消收藏课程",
                fav_pugv_write_request(77000001, false),
                path::DEL_FAV_PUGV,
                "season_id=77000001",
            ),
            (
                "收藏话题",
                fav_topic_write_request(88000001, true),
                path::ADD_FAV_TOPIC,
                "topic_id=88000001",
            ),
            (
                "取消收藏话题",
                fav_topic_write_request(88000001, false),
                path::DEL_FAV_TOPIC,
                "topic_id=88000001",
            ),
            (
                "点赞话题",
                like_topic_request(88000001, 800000601, true),
                path::LIKE_TOPIC,
                "action=like&up_mid=800000601&topic_id=88000001&business=topic",
            ),
            (
                "收藏专栏",
                fav_article_write_request(110000001, true),
                path::ADD_FAV_ARTICLE,
                "id=110000001",
            ),
            (
                "取消收藏专栏",
                fav_article_write_request(110000001, false),
                path::DEL_FAV_ARTICLE,
                "id=110000001",
            ),
            (
                "删除未发布的笔记",
                del_note_request("1,2", false),
                path::DEL_NOTE,
                "note_ids=1,2",
            ),
            (
                "删除已发布的笔记",
                del_note_request("1,2", true),
                path::DEL_PUBLISH_NOTE,
                "cvids=1,2",
            ),
            (
                "清除失效内容",
                clean_fav_request(2101),
                path::CLEAN_FAV,
                "media_id=2101&platform=web",
            ),
            (
                "删除收藏夹",
                delete_folder_request("2102,2103"),
                path::DELETE_FOLDER,
                "media_ids=2102,2103&platform=web",
            ),
            (
                "新建收藏夹",
                add_folder_request("新夹子", "简介", 1, ""),
                path::ADD_FOLDER,
                "title=新夹子&intro=简介&privacy=1&cover=",
            ),
            (
                "编辑收藏夹",
                edit_folder_request(2102, "改名", "", 0, ""),
                path::EDIT_FOLDER,
                "title=改名&intro=&privacy=0&cover=&media_id=2102",
            ),
            (
                "批量收藏",
                fav_video_request("110000001:2", "2101", ""),
                path::FAV_VIDEO,
                "resources=110000001:2&add_media_ids=2101&del_media_ids=",
            ),
            (
                "从所有收藏夹移出",
                unfav_all_request(110000001, 2),
                path::UNFAV_ALL,
                "rid=110000001&type=2",
            ),
            (
                "复制到收藏夹",
                copy_or_move_fav_request(
                    FavMoveMode::Copy,
                    FavMoveTarget::Folder,
                    Some(2101),
                    2102,
                    Some(800000601),
                    "110000001:2",
                ),
                path::COPY_FAV,
                "src_media_id=2101&tar_media_id=2102&mid=800000601&resources=110000001:2&platform=web",
            ),
            (
                "移动到收藏夹",
                copy_or_move_fav_request(
                    FavMoveMode::Move,
                    FavMoveTarget::Folder,
                    Some(2101),
                    2102,
                    None,
                    "110000001:2",
                ),
                path::MOVE_FAV,
                "src_media_id=2101&tar_media_id=2102&resources=110000001:2&platform=web",
            ),
            (
                "从稍后再看复制",
                copy_or_move_fav_request(
                    FavMoveMode::Copy,
                    FavMoveTarget::ToView,
                    None,
                    2102,
                    Some(800000601),
                    "110000001",
                ),
                path::COPY_TOVIEW,
                "tar_media_id=2102&mid=800000601&resources=110000001&platform=web",
            ),
        ];

        for (name, req, expected_path, expected_form) in cases {
            assert_eq!(req.path, expected_path, "{name}: 路径");
            assert_eq!(req.form_string(), expected_form, "{name}: 请求体");
            assert_eq!(req.csrf, CsrfIn::Body, "{name}: csrf 补在表单里");
            assert!(req.query.is_empty(), "{name}: 这一条不该有查询串");
            assert!(
                req.form_fields()
                    .expect("这些都是表单体")
                    .iter()
                    .all(|(k, _)| k != "csrf"),
                "{name}: csrf 不该出现在构造产物里——它由 send_write 统一补，\
                 这样请求快照才不含凭据"
            );
        }
    }

    /// 两条异类单独钉：一条 JSON 体 + csrf 在查询串，两条 csrf 进了签名。
    #[test]
    fn the_three_odd_shapes_are_pinned() {
        let community = community_action_request("990000000000000001", 3);
        assert_eq!(community.path, path::COMMUNITY_ACTION);
        assert_eq!(community.csrf, CsrfIn::Query);
        assert_eq!(
            community.form_string(),
            r#"{"action":3,"entity":{"object_id_str":"990000000000000001","type":{"biz":2}}}"#
        );

        let auth = AppSignature {
            appkey: "dfca71928277209b".to_owned(),
            appsec: "0000000000000000000000000000000f".to_owned(),
            ts: 1_700_000_000,
        };
        for req in [
            sort_fav_folder_request("2101,2102", "deadbeef", &auth),
            sort_fav_request(2101, "0:0:110000001:2", "deadbeef", &auth),
        ] {
            let form = req.form_string();
            assert!(form.contains("appkey=dfca71928277209b"), "{form}");
            assert!(form.contains("ts=1700000000"), "{form}");
            assert!(form.contains("csrf=deadbeef"), "{form}");
            assert!(form.contains("&sign="), "{form}");
            assert_eq!(
                req.csrf,
                CsrfIn::None,
                "csrf 已经自己放进去并签过了，再补一次会出现两个"
            );
        }
    }

    /// 定级是协议知识的一部分，整表钉住：改级别要有人显式改测试。
    #[test]
    fn idempotency_levels_match_the_adr() {
        // 这一片只有一条不是 Idempotent。
        assert_eq!(
            add_folder_request("t", "", 0, "").idempotency,
            Idempotency::Unsafe,
            "新建收藏夹重发会多建一个同名夹子，用户直接看得见，且无从判定"
        );

        for (name, req) in [
            ("收藏收藏夹", fav_folder_request(1, true)),
            ("订阅合集", fav_season_request(1, true)),
            ("收藏课程", fav_pugv_write_request(1, true)),
            ("收藏话题", fav_topic_write_request(1, true)),
            ("点赞话题", like_topic_request(1, 2, true)),
            ("收藏专栏", fav_article_write_request(1, true)),
            ("收藏图文", community_action_request("1", 3)),
            ("删除笔记", del_note_request("1", false)),
            ("清除失效", clean_fav_request(1)),
            ("删除收藏夹", delete_folder_request("1")),
            ("编辑收藏夹", edit_folder_request(1, "t", "", 0, "")),
            ("批量收藏", fav_video_request("1:2", "3", "")),
            ("全部取消收藏", unfav_all_request(1, 2)),
            (
                "复制",
                copy_or_move_fav_request(
                    FavMoveMode::Copy,
                    FavMoveTarget::Folder,
                    None,
                    1,
                    None,
                    "1:2",
                ),
            ),
        ] {
            assert_eq!(
                req.idempotency,
                Idempotency::Idempotent,
                "{name}: 服务端语义是「置为」，重发落到同一个状态"
            );
        }
    }
}
