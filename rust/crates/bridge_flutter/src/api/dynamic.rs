//! 动态路径的 typed FRB 导出（S5 / #55）。

use std::{fmt, time::Duration};

use bili_api::{dynamic, http::RequestContext, transport_reqwest::ReqwestTransport};
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
