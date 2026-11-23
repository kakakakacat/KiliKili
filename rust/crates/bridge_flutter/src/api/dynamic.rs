//! 动态路径的 typed FRB 导出（S5 / #55）。

use std::{fmt, time::Duration};

use bili_api::{dynamic, http::RequestContext, transport_reqwest::ReqwestTransport};
use bili_domain::dynamic as domain;
use bili_domain::error::{DomainError, ErrorKind};
use bili_domain::redact::Redacted;

use crate::api::{grpc::GrpcContext, init, transport::TransportSettings, write::WriteResult};

#[derive(Clone)]
pub struct DynamicWriteContext {
    pub base_url: String,
    pub cookie: Option<String>,
    pub csrf: Option<String>,
    pub headers: Vec<(String, String)>,
    pub timeout_ms: u64,
    pub transport: TransportSettings,
}

impl fmt::Debug for DynamicWriteContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DynamicWriteContext")
            .field("base_url", &self.base_url)
            .field("credentials", &"<redacted>")
            .field("timeout_ms", &self.timeout_ms)
            .finish_non_exhaustive()
    }
}

impl DynamicWriteContext {
    fn split(self) -> (RequestContext, ReqwestTransport) {
        (
            RequestContext {
                base_url: self.base_url,
                cookie: self.cookie.map(Redacted::new),
                csrf: self.csrf.map(Redacted::new),
                headers: self.headers,
                timeout: Duration::from_millis(self.timeout_ms),
            },
            ReqwestTransport::new(self.transport.to_config()),
        )
    }
}

pub async fn thumb_dynamic(
    context: DynamicWriteContext,
    dynamic_id: String,
    up: i32,
) -> WriteResult {
    let (ctx, transport) = context.split();
    let result = init::runtime()
        .spawn(async move { dynamic::send_thumb(&transport, &ctx, &dynamic_id, up).await })
        .await
        .unwrap_or_else(|_| {
            Err(DomainError::new(
                ErrorKind::Protocol,
                "动态点赞工作任务未完成",
            ))
        });
    WriteResult::from_result(result)
}

pub async fn top_dynamic(
    context: DynamicWriteContext,
    dynamic_id: String,
    set_top: bool,
) -> WriteResult {
    let (ctx, transport) = context.split();
    let result = init::runtime()
        .spawn(async move { dynamic::send_top(&transport, &ctx, &dynamic_id, set_top).await })
        .await
        .unwrap_or_else(|_| {
            Err(DomainError::new(
                ErrorKind::Protocol,
                "动态置顶工作任务未完成",
            ))
        });
    WriteResult::from_result(result)
}

pub async fn private_pub_setting(
    context: DynamicWriteContext,
    dynamic_id: String,
    dyn_type: Option<i64>,
    action: String,
) -> WriteResult {
    let (ctx, transport) = context.split();
    let result = init::runtime()
        .spawn(async move {
            dynamic::send_private_pub(&transport, &ctx, &dynamic_id, dyn_type, &action).await
        })
        .await
        .unwrap_or_else(|_| {
            Err(DomainError::new(
                ErrorKind::Protocol,
                "动态私密设置工作任务未完成",
            ))
        });
    WriteResult::from_result(result)
}

#[derive(Debug, Clone)]
pub struct ReserveInfoResult {
    pub data: Option<ReserveCard>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct ReserveCard {
    pub id: i64,
    pub title: String,
    pub live_plan_start_time: i64,
}

#[derive(Debug, Clone)]
pub struct ReserveIdResult {
    pub data: Option<i64>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct ReserveActionResult {
    pub data: Option<ReserveActionData>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct ReserveActionData {
    pub final_btn_status: Option<i32>,
    pub reserve_update: Option<i32>,
    pub desc_update: Option<String>,
}

pub async fn reserve_info(context: DynamicWriteContext, sid: i64) -> ReserveInfoResult {
    let (ctx, transport) = context.split();
    let result = init::runtime()
        .spawn(async move { dynamic::fetch_reserve_info(&transport, &ctx, sid).await })
        .await
        .unwrap_or_else(|_| {
            Err(DomainError::new(
                ErrorKind::Protocol,
                "预约信息工作任务未完成",
            ))
        });
    match result {
        Ok(data) => ReserveInfoResult {
            data: Some(ReserveCard {
                id: data.id,
                title: data.title,
                live_plan_start_time: data.live_plan_start_time,
            }),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(error) => ReserveInfoResult {
            data: None,
            error_kind: Some(error.kind().to_string()),
            error_detail: Some(error.detail().into()),
            error_code: error.code(),
        },
    }
}

pub async fn create_reserve(
    context: DynamicWriteContext,
    sub_type: i32,
    title: String,
    start: i64,
) -> ReserveIdResult {
    let (ctx, transport) = context.split();
    let result = init::runtime()
        .spawn(async move {
            dynamic::send_create_reserve(&transport, &ctx, sub_type, &title, start).await
        })
        .await
        .unwrap_or_else(|_| {
            Err(DomainError::new(
                ErrorKind::Protocol,
                "创建预约工作任务未完成",
            ))
        });
    match result {
        Ok(data) => ReserveIdResult {
            data: Some(data),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(error) => ReserveIdResult {
            data: None,
            error_kind: Some(error.kind().to_string()),
            error_detail: Some(error.detail().into()),
            error_code: error.code(),
        },
    }
}

pub async fn update_reserve(
    context: DynamicWriteContext,
    sid: i64,
    sub_type: i32,
    title: String,
    start: i64,
) -> ReserveIdResult {
    let (ctx, transport) = context.split();
    let result = init::runtime()
        .spawn(async move {
            dynamic::send_update_reserve(&transport, &ctx, sid, sub_type, &title, start).await
        })
        .await
        .unwrap_or_else(|_| {
            Err(DomainError::new(
                ErrorKind::Protocol,
                "更新预约工作任务未完成",
            ))
        });
    match result {
        Ok(data) => ReserveIdResult {
            data: Some(data),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(error) => ReserveIdResult {
            data: None,
            error_kind: Some(error.kind().to_string()),
            error_detail: Some(error.detail().into()),
            error_code: error.code(),
        },
    }
}

pub async fn dyn_reserve(
    context: DynamicWriteContext,
    reserve_id: Option<i64>,
    status: Option<i32>,
    dynamic_id: String,
    total: Option<i64>,
) -> ReserveActionResult {
    let (ctx, transport) = context.split();
    let result = init::runtime()
        .spawn(async move {
            dynamic::send_dyn_reserve(&transport, &ctx, reserve_id, status, &dynamic_id, total)
                .await
        })
        .await
        .unwrap_or_else(|_| {
            Err(DomainError::new(
                ErrorKind::Protocol,
                "动态预约工作任务未完成",
            ))
        });
    match result {
        Ok(data) => ReserveActionResult {
            data: Some(ReserveActionData {
                final_btn_status: data.final_btn_status,
                reserve_update: data.reserve_update,
                desc_update: data.desc_update,
            }),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(error) => ReserveActionResult {
            data: None,
            error_kind: Some(error.kind().to_string()),
            error_detail: Some(error.detail().into()),
            error_code: error.code(),
        },
    }
}

#[derive(Debug, Clone)]
pub struct VoteOptionData {
    pub opt_idx: Option<i32>,
    pub opt_desc: Option<String>,
    pub cnt: i64,
    pub img_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct VoteInfoData {
    pub choice_cnt: Option<i32>,
    pub default_share: Option<i32>,
    pub desc: Option<String>,
    pub end_time: Option<i64>,
    pub status: Option<i32>,
    pub uid: Option<i64>,
    pub vote_id: Option<i64>,
    pub join_num: i64,
    pub title: Option<String>,
    pub ctime: Option<i64>,
    pub my_votes: Option<Vec<i32>>,
    pub options: Vec<VoteOptionData>,
    pub options_cnt: Option<i32>,
    pub voter_level: Option<i32>,
    pub face: Option<String>,
    pub name: Option<String>,
    pub vote_type: Option<i32>,
    pub vote_publisher: Option<i64>,
    pub duration: Option<i64>,
    pub only_fans_level: Option<i32>,
}

#[derive(Debug, Clone)]
pub struct VoteInfoResult {
    pub data: Option<VoteInfoData>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct VoteIdResult {
    pub data: Option<i64>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct FolloweeVoteData {
    pub uid: i64,
    pub name: String,
    pub face: String,
    pub votes: Vec<i32>,
    pub ctime: i64,
}

#[derive(Debug, Clone)]
pub struct FolloweeVotesResult {
    pub data: Option<Vec<FolloweeVoteData>>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

fn into_vote_data(info: bili_domain::dynamic::VoteInfo) -> VoteInfoData {
    VoteInfoData {
        choice_cnt: info.choice_cnt,
        default_share: info.default_share,
        desc: info.desc,
        end_time: info.end_time,
        status: info.status,
        uid: info.uid,
        vote_id: info.vote_id,
        join_num: info.join_num,
        title: info.title,
        ctime: info.ctime,
        my_votes: info.my_votes,
        options: info
            .options
            .into_iter()
            .map(|option| VoteOptionData {
                opt_idx: option.opt_idx,
                opt_desc: option.opt_desc,
                cnt: option.cnt,
                img_url: option.img_url,
            })
            .collect(),
        options_cnt: info.options_cnt,
        voter_level: info.voter_level,
        face: info.face,
        name: info.name,
        vote_type: info.vote_type,
        vote_publisher: info.vote_publisher,
        duration: info.duration,
        only_fans_level: info.only_fans_level,
    }
}

fn into_vote_domain(info: VoteInfoData) -> bili_domain::dynamic::VoteInfo {
    bili_domain::dynamic::VoteInfo {
        choice_cnt: info.choice_cnt,
        default_share: info.default_share,
        desc: info.desc,
        end_time: info.end_time,
        status: info.status,
        uid: info.uid,
        vote_id: info.vote_id,
        join_num: info.join_num,
        title: info.title,
        ctime: info.ctime,
        my_votes: info.my_votes,
        options: info
            .options
            .into_iter()
            .map(|option| bili_domain::dynamic::VoteOption {
                opt_idx: option.opt_idx,
                opt_desc: option.opt_desc,
                cnt: option.cnt,
                img_url: option.img_url,
            })
            .collect(),
        options_cnt: info.options_cnt,
        voter_level: info.voter_level,
        face: info.face,
        name: info.name,
        vote_type: info.vote_type,
        vote_publisher: info.vote_publisher,
        duration: info.duration,
        only_fans_level: info.only_fans_level,
    }
}

pub async fn vote_info(context: DynamicWriteContext, vote_id: i64) -> VoteInfoResult {
    let (ctx, transport) = context.split();
    let result = init::runtime()
        .spawn(async move { dynamic::fetch_vote_info(&transport, &ctx, vote_id).await })
        .await
        .unwrap_or_else(|_| {
            Err(DomainError::new(
                ErrorKind::Protocol,
                "投票信息工作任务未完成",
            ))
        });
    match result {
        Ok(info) => VoteInfoResult {
            data: Some(into_vote_data(info)),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(error) => VoteInfoResult {
            data: None,
            error_kind: Some(error.kind().to_string()),
            error_detail: Some(error.detail().into()),
            error_code: error.code(),
        },
    }
}

pub async fn do_vote(
    context: DynamicWriteContext,
    vote_id: i64,
    votes: Vec<i32>,
    anonymous: bool,
    dynamic_id: Option<i64>,
    voter_uid: Option<i64>,
) -> VoteInfoResult {
    let (ctx, transport) = context.split();
    let result = init::runtime()
        .spawn(async move {
            dynamic::send_do_vote(
                &transport, &ctx, vote_id, &votes, anonymous, dynamic_id, voter_uid,
            )
            .await
        })
        .await
        .unwrap_or_else(|_| {
            Err(DomainError::new(
                ErrorKind::Protocol,
                "投票提交工作任务未完成",
            ))
        });
    match result {
        Ok(info) => VoteInfoResult {
            data: Some(into_vote_data(info)),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(error) => VoteInfoResult {
            data: None,
            error_kind: Some(error.kind().to_string()),
            error_detail: Some(error.detail().into()),
            error_code: error.code(),
        },
    }
}

pub async fn save_vote(
    context: DynamicWriteContext,
    info: VoteInfoData,
    update: bool,
) -> VoteIdResult {
    let (ctx, transport) = context.split();
    let result = init::runtime()
        .spawn(async move {
            dynamic::send_save_vote(&transport, &ctx, &into_vote_domain(info), update).await
        })
        .await
        .unwrap_or_else(|_| {
            Err(DomainError::new(
                ErrorKind::Protocol,
                "投票保存工作任务未完成",
            ))
        });
    match result {
        Ok(data) => VoteIdResult {
            data: Some(data),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(error) => VoteIdResult {
            data: None,
            error_kind: Some(error.kind().to_string()),
            error_detail: Some(error.detail().into()),
            error_code: error.code(),
        },
    }
}

pub async fn followee_votes(context: DynamicWriteContext, vote_id: i64) -> FolloweeVotesResult {
    let (ctx, transport) = context.split();
    let result = init::runtime()
        .spawn(async move { dynamic::fetch_followee_votes(&transport, &ctx, vote_id).await })
        .await
        .unwrap_or_else(|_| {
            Err(DomainError::new(
                ErrorKind::Protocol,
                "关注者投票工作任务未完成",
            ))
        });
    match result {
        Ok(items) => FolloweeVotesResult {
            data: Some(
                items
                    .into_iter()
                    .map(|item| FolloweeVoteData {
                        uid: item.uid,
                        name: item.name,
                        face: item.face,
                        votes: item.votes,
                        ctime: item.ctime,
                    })
                    .collect(),
            ),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(error) => FolloweeVotesResult {
            data: None,
            error_kind: Some(error.kind().to_string()),
            error_detail: Some(error.detail().into()),
            error_code: error.code(),
        },
    }
}

#[derive(Debug, Clone)]
pub struct DynRedResult {
    pub data: Option<i64>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
pub struct LikeListPage {
    pub items: Vec<DynamicAuthor>,
    pub has_more: bool,
    pub total_count: i64,
}

#[derive(Debug, Clone)]
pub struct LikeListResult {
    pub data: Option<LikeListPage>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

pub async fn fetch_like_list(
    context: GrpcContext,
    dynamic_id: String,
    dyn_type: Option<i64>,
    rid: Option<i64>,
    uid_offset: Option<i64>,
    page: i32,
) -> LikeListResult {
    let (ctx, transport) = context.split();
    let result = init::runtime()
        .spawn(async move {
            dynamic::fetch_like_list(
                &transport,
                &ctx,
                &dynamic_id,
                dyn_type,
                rid,
                uid_offset,
                page,
            )
            .await
        })
        .await
        .unwrap_or_else(|_| {
            Err(DomainError::new(
                ErrorKind::Protocol,
                "动态点赞列表工作任务未完成",
            ))
        });
    match result {
        Ok(page) => LikeListResult {
            data: Some(LikeListPage {
                items: page
                    .items
                    .into_iter()
                    .map(|item| DynamicAuthor {
                        mid: item.mid,
                        name: item.name,
                        face: item.face,
                        sign: item.sign,
                        level: item.level,
                        is_senior_member: item.is_senior_member,
                        pendant_image: item.pendant_image,
                        vip_status: item.vip_status,
                        vip_type: item.vip_type,
                        official_type: item.official_type,
                    })
                    .collect(),
                has_more: page.has_more,
                total_count: page.total_count,
            }),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(error) => LikeListResult {
            data: None,
            error_kind: Some(error.kind().to_string()),
            error_detail: Some(error.detail().into()),
            error_code: error.code(),
        },
    }
}

pub async fn fetch_dyn_red(context: GrpcContext) -> DynRedResult {
    let (ctx, transport) = context.split();
    let result = init::runtime()
        .spawn(async move { dynamic::fetch_dyn_red(&transport, &ctx).await })
        .await
        .unwrap_or_else(|_| {
            Err(DomainError::new(
                ErrorKind::Protocol,
                "动态红点工作任务未完成",
            ))
        });
    match result {
        Ok(data) => DynRedResult {
            data: Some(data.count),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(error) => DynRedResult {
            data: None,
            error_kind: Some(error.kind().to_string()),
            error_detail: Some(error.detail().into()),
            error_code: error.code(),
        },
    }
}

// ============================================ S5 续单第一批（issue #150）

/// 本批几条只读接口的公共参数。
///
/// 与 [`DynamicWriteContext`] **分开**：这几条一个 csrf 都不要，合用一个
/// 结构等于每个调用点都要答一遍「这里要不要传 csrf」，而答案由接口定。
///
/// `base_url` 由 Dart 给：[`topic_top`] 挂在 app 网关上，与其余几条不同
/// 域名（基线的 `HttpString.appBaseUrl`）。
#[derive(Debug, Clone)]
pub struct DynamicReadContext {
    pub base_url: String,
    pub cookie: Option<String>,
    pub headers: Vec<(String, String)>,
    pub timeout_ms: u64,
    pub transport: TransportSettings,
    /// WBI mixin key。只有 [`article_info`] 要签名，其余传空串即可。
    pub mixin_key: String,
    /// Unix 秒，由 Dart 注入，使请求构造保持纯函数、可被快照钉住。
    pub wts: i64,
}

impl DynamicReadContext {
    fn split(self) -> (RequestContext, ReqwestTransport, String, i64) {
        let transport = ReqwestTransport::new(self.transport.to_config());
        let ctx = RequestContext {
            base_url: self.base_url,
            cookie: self.cookie.map(Redacted::new),
            csrf: None,
            headers: self.headers,
            timeout: Duration::from_millis(self.timeout_ms),
        };
        (ctx, transport, self.mixin_key, self.wts)
    }
}

#[derive(Debug, Clone)]
pub struct ArticleStatsData {
    pub favorite: Option<i64>,
    pub like: Option<i64>,
    pub reply: Option<i64>,
    pub share: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct ArticleInfoData {
    pub favorite: Option<bool>,
    pub title: Option<String>,
    pub origin_image_urls: Vec<String>,
    pub stats: Option<ArticleStatsData>,
}

#[derive(Debug, Clone)]
pub struct ArticleInfoResult {
    pub data: Option<ArticleInfoData>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct ArticleListInfoData {
    pub id: Option<i64>,
    pub name: Option<String>,
    pub image_url: Option<String>,
    pub update_time: Option<i64>,
    pub words: Option<i64>,
    pub read: Option<i64>,
    pub articles_count: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct ArticleListItemStatsData {
    pub view: Option<i64>,
    pub like: Option<i64>,
    pub reply: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct ArticleListItemData {
    pub id: Option<i64>,
    pub title: Option<String>,
    pub image_urls: Vec<String>,
    pub summary: Option<String>,
    pub dyn_id_str: Option<String>,
    pub stats: Option<ArticleListItemStatsData>,
}

#[derive(Debug, Clone)]
pub struct ArticleListData {
    pub info: Option<ArticleListInfoData>,
    pub articles: Vec<ArticleListItemData>,
    pub author_mid: Option<i64>,
    pub author_name: Option<String>,
    pub author_face: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ArticleListResult {
    pub data: Option<ArticleListData>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct TopicItemData {
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

#[derive(Debug, Clone)]
pub struct TopicCreatorData {
    pub uid: Option<i64>,
    pub face: Option<String>,
    pub name: Option<String>,
}

/// 话题页顶部。
///
/// 两个字段都为空表示「这个话题没有顶部信息」，不是错误——基线在这里给的
/// 是 `null`，界面据此不渲染。没有再套一层 `Option`，因为 FRB 的 codegen
/// 碰到 `Option<Option<T>>` 会直接 panic（AGENTS.md §4）。
#[derive(Debug, Clone)]
pub struct TopicTopData {
    pub topic: Option<TopicItemData>,
    pub creator: Option<TopicCreatorData>,
}

#[derive(Debug, Clone)]
pub struct TopicTopResult {
    pub data: Option<TopicTopData>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct TopicItemsResult {
    pub data: Option<Vec<TopicItemData>>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct BubbleBaseInfoData {
    pub is_joined: Option<bool>,
    pub id: Option<String>,
    pub title: Option<String>,
    pub sub_title: Option<String>,
    pub face_url: Option<String>,
    pub jump_uri: Option<String>,
    pub summary: Option<String>,
}

#[derive(Debug, Clone)]
pub struct BubbleDynData {
    pub dyn_id: Option<String>,
    pub title: Option<String>,
    pub author: Option<String>,
    pub time_text: Option<String>,
    pub reply_count: Option<String>,
    pub view_stat: Option<String>,
}

#[derive(Debug, Clone)]
pub struct BubbleContentData {
    pub count: Option<String>,
    pub items: Vec<BubbleDynData>,
}

#[derive(Debug, Clone)]
pub struct BubbleCategoryData {
    pub id: Option<String>,
    pub name: Option<String>,
    pub kind: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct BubbleSortItemData {
    pub sort_type: Option<i64>,
    pub text: Option<String>,
}

#[derive(Debug, Clone)]
pub struct BubbleSortInfoData {
    pub show_sort: Option<bool>,
    pub cur_sort_type: Option<i64>,
    pub items: Vec<BubbleSortItemData>,
}

#[derive(Debug, Clone)]
pub struct BubbleData {
    pub base_info: Option<BubbleBaseInfoData>,
    pub content: Option<BubbleContentData>,
    pub categories: Vec<BubbleCategoryData>,
    pub sort_info: Option<BubbleSortInfoData>,
}

#[derive(Debug, Clone)]
pub struct BubbleResult {
    pub data: Option<BubbleData>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

fn into_topic_item(item: domain::TopicItem) -> TopicItemData {
    TopicItemData {
        id: item.id,
        name: item.name,
        view: item.view,
        discuss: item.discuss,
        fav: item.fav,
        like: item.like,
        description: item.description,
        is_fav: item.is_fav,
        is_like: item.is_like,
    }
}

/// 取专栏阅读页的计数与收藏态。
pub async fn article_info(context: DynamicReadContext, cv_id: String) -> ArticleInfoResult {
    let (ctx, transport, mixin_key, wts) = context.split();
    let result = init::runtime()
        .spawn(async move {
            dynamic::fetch_article_info(&transport, &ctx, &cv_id, &mixin_key, wts).await
        })
        .await
        .unwrap_or_else(|_| {
            Err(DomainError::new(
                ErrorKind::Protocol,
                "专栏信息工作任务未完成",
            ))
        });
    match result {
        Ok(info) => ArticleInfoResult {
            data: Some(ArticleInfoData {
                favorite: info.favorite,
                title: info.title,
                origin_image_urls: info.origin_image_urls,
                stats: info.stats.map(|s| ArticleStatsData {
                    favorite: s.favorite,
                    like: s.like,
                    reply: s.reply,
                    share: s.share,
                }),
            }),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(error) => ArticleInfoResult {
            data: None,
            error_kind: Some(error.kind().to_string()),
            error_detail: Some(error.detail().into()),
            error_code: error.code(),
        },
    }
}

/// 取文集。
pub async fn article_list(context: DynamicReadContext, id: String) -> ArticleListResult {
    let (ctx, transport, _, _) = context.split();
    let result = init::runtime()
        .spawn(async move { dynamic::fetch_article_list(&transport, &ctx, &id).await })
        .await
        .unwrap_or_else(|_| Err(DomainError::new(ErrorKind::Protocol, "文集工作任务未完成")));
    match result {
        Ok(list) => ArticleListResult {
            data: Some(ArticleListData {
                info: list.info.map(|i| ArticleListInfoData {
                    id: i.id,
                    name: i.name,
                    image_url: i.image_url,
                    update_time: i.update_time,
                    words: i.words,
                    read: i.read,
                    articles_count: i.articles_count,
                }),
                articles: list
                    .articles
                    .into_iter()
                    .map(|a| ArticleListItemData {
                        id: a.id,
                        title: a.title,
                        image_urls: a.image_urls,
                        summary: a.summary,
                        dyn_id_str: a.dyn_id_str,
                        stats: a.stats.map(|s| ArticleListItemStatsData {
                            view: s.view,
                            like: s.like,
                            reply: s.reply,
                        }),
                    })
                    .collect(),
                author_mid: list.author_mid,
                author_name: list.author_name,
                author_face: list.author_face,
            }),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(error) => ArticleListResult {
            data: None,
            error_kind: Some(error.kind().to_string()),
            error_detail: Some(error.detail().into()),
            error_code: error.code(),
        },
    }
}

/// 取话题页顶部。`base_url` 要给 app 网关。
pub async fn topic_top(context: DynamicReadContext, topic_id: String) -> TopicTopResult {
    let (ctx, transport, _, _) = context.split();
    let result = init::runtime()
        .spawn(async move { dynamic::fetch_topic_top(&transport, &ctx, &topic_id).await })
        .await
        .unwrap_or_else(|_| {
            Err(DomainError::new(
                ErrorKind::Protocol,
                "话题顶部工作任务未完成",
            ))
        });
    match result {
        Ok(top) => TopicTopResult {
            data: Some(TopicTopData {
                topic: top.topic.map(into_topic_item),
                creator: top.creator.map(|c| TopicCreatorData {
                    uid: c.uid,
                    face: c.face,
                    name: c.name,
                }),
            }),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(error) => TopicTopResult {
            data: None,
            error_kind: Some(error.kind().to_string()),
            error_detail: Some(error.detail().into()),
            error_code: error.code(),
        },
    }
}

/// 取推荐话题。
pub async fn topic_rcmd(context: DynamicReadContext, page_size: i64) -> TopicItemsResult {
    let (ctx, transport, _, _) = context.split();
    let result = init::runtime()
        .spawn(async move { dynamic::fetch_topic_rcmd(&transport, &ctx, page_size).await })
        .await
        .unwrap_or_else(|_| {
            Err(DomainError::new(
                ErrorKind::Protocol,
                "推荐话题工作任务未完成",
            ))
        });
    match result {
        Ok(items) => TopicItemsResult {
            data: Some(items.into_iter().map(into_topic_item).collect()),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(error) => TopicItemsResult {
            data: None,
            error_kind: Some(error.kind().to_string()),
            error_detail: Some(error.detail().into()),
            error_code: error.code(),
        },
    }
}

/// 取动态页顶部的部落气泡。
pub async fn bubble(
    context: DynamicReadContext,
    tribe_id: String,
    category_id: Option<String>,
    sort_type: Option<i64>,
    page: i64,
) -> BubbleResult {
    let (ctx, transport, _, _) = context.split();
    let result = init::runtime()
        .spawn(async move {
            dynamic::fetch_bubble(
                &transport,
                &ctx,
                &tribe_id,
                category_id.as_deref(),
                sort_type,
                page,
            )
            .await
        })
        .await
        .unwrap_or_else(|_| {
            Err(DomainError::new(
                ErrorKind::Protocol,
                "部落气泡工作任务未完成",
            ))
        });
    match result {
        Ok(data) => BubbleResult {
            data: Some(BubbleData {
                base_info: data.base_info.map(|b| BubbleBaseInfoData {
                    is_joined: b.is_joined,
                    id: b.id,
                    title: b.title,
                    sub_title: b.sub_title,
                    face_url: b.face_url,
                    jump_uri: b.jump_uri,
                    summary: b.summary,
                }),
                content: data.content.map(|c| BubbleContentData {
                    count: c.count,
                    items: c
                        .items
                        .into_iter()
                        .map(|d| BubbleDynData {
                            dyn_id: d.dyn_id,
                            title: d.title,
                            author: d.author,
                            time_text: d.time_text,
                            reply_count: d.reply_count,
                            view_stat: d.view_stat,
                        })
                        .collect(),
                }),
                categories: data
                    .categories
                    .into_iter()
                    .map(|c| BubbleCategoryData {
                        id: c.id,
                        name: c.name,
                        kind: c.kind,
                    })
                    .collect(),
                sort_info: data.sort_info.map(|s| BubbleSortInfoData {
                    show_sort: s.show_sort,
                    cur_sort_type: s.cur_sort_type,
                    items: s
                        .items
                        .into_iter()
                        .map(|i| BubbleSortItemData {
                            sort_type: i.sort_type,
                            text: i.text,
                        })
                        .collect(),
                }),
            }),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(error) => BubbleResult {
            data: None,
            error_kind: Some(error.kind().to_string()),
            error_detail: Some(error.detail().into()),
            error_code: error.code(),
        },
    }
}

#[derive(Debug, Clone)]
pub struct TopicPubSearchData {
    pub items: Vec<TopicItemData>,
    pub has_more: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct TopicPubSearchResult {
    pub data: Option<TopicPubSearchData>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

/// 发布页的话题搜索。`base_url` 要给 app 网关。
pub async fn topic_pub_search(
    context: DynamicReadContext,
    keywords: String,
    content: String,
    page_num: i64,
) -> TopicPubSearchResult {
    let (ctx, transport, _, _) = context.split();
    let result = init::runtime()
        .spawn(async move {
            dynamic::fetch_topic_pub_search(&transport, &ctx, &keywords, &content, page_num).await
        })
        .await
        .unwrap_or_else(|_| {
            Err(DomainError::new(
                ErrorKind::Protocol,
                "话题搜索工作任务未完成",
            ))
        });
    match result {
        Ok(found) => TopicPubSearchResult {
            data: Some(TopicPubSearchData {
                items: found.items.into_iter().map(into_topic_item).collect(),
                has_more: found.has_more,
            }),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(error) => TopicPubSearchResult {
            data: None,
            error_kind: Some(error.kind().to_string()),
            error_detail: Some(error.detail().into()),
            error_code: error.code(),
        },
    }
}
