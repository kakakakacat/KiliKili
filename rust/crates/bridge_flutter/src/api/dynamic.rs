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
