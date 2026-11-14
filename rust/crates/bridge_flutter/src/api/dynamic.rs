//! 动态路径的 typed FRB 导出（S5 / #55）。

use bili_api::dynamic;
use bili_domain::error::{DomainError, ErrorKind};

use crate::api::{grpc::GrpcContext, init};

#[derive(Debug, Clone)]
pub struct DynRedResult {
    pub data: Option<i64>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
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
