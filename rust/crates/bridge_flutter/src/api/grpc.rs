//! gRPC 前置桥接：宿主仅注入账号与设置，protobuf 消息止步 Rust。
use crate::api::{init, transport::TransportSettings};
use bili_api::{
    grpc_danmaku, grpc_metadata, http::RequestContext, transport_reqwest::ReqwestTransport,
};
use bili_domain::{
    error::{DomainError, ErrorKind},
    redact::Redacted,
};
use std::{fmt, time::Duration};

#[derive(Clone)]
pub struct GrpcIdentity {
    pub access_key: Option<String>,
    pub buvid: String,
    /// 基线随机八字符值；由宿主提供随机源，编码决策留在 Rust。
    pub session_id: String,
}
impl fmt::Debug for GrpcIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GrpcIdentity")
            .field("credentials", &"<redacted>")
            .finish()
    }
}
impl GrpcIdentity {
    pub(crate) fn into_domain(self) -> grpc_metadata::GrpcIdentity {
        grpc_metadata::GrpcIdentity {
            access_key: self.access_key.map(Redacted::new),
            buvid: self.buvid,
            session_id: self.session_id,
        }
    }
}

#[derive(Debug, Clone)]
pub struct GrpcContext {
    pub base_url: String,
    pub identity: GrpcIdentity,
    pub timeout_ms: u64,
    pub transport: TransportSettings,
}
impl GrpcContext {
    pub(crate) fn split(self) -> (RequestContext, ReqwestTransport) {
        let headers = grpc_metadata::headers(&self.identity.into_domain());
        (
            RequestContext {
                base_url: self.base_url,
                cookie: None,
                csrf: None,
                headers,
                timeout: Duration::from_millis(self.timeout_ms),
            },
            ReqwestTransport::new(self.transport.to_config()),
        )
    }
}

/// 未迁的 gRPC 请求也共用同一份 metadata 编码；不用额外网络调用。
#[flutter_rust_bridge::frb(sync)]
pub fn build_grpc_headers(identity: GrpcIdentity) -> Vec<(String, String)> {
    grpc_metadata::headers(&identity.into_domain())
}

#[derive(Debug, Clone)]
pub struct GrpcBytesResult {
    pub data: Option<Vec<u8>>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}
#[derive(Debug, Clone)]
pub struct GrpcSubtitle {
    pub id: i64,
    pub lan: String,
    pub lan_doc: String,
    pub subtitle_url: String,
    pub subtitle_type: i32,
    pub is_ai: bool,
}
#[derive(Debug, Clone)]
pub struct GrpcSubtitlesResult {
    pub data: Option<Vec<GrpcSubtitle>>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

fn flatten<T>(
    result: Result<Result<T, DomainError>, tokio::task::JoinError>,
) -> Result<T, DomainError> {
    // JoinError 的 panic payload 可能含参数；不向宿主回传它的原始文本。
    result.unwrap_or_else(|_| Err(DomainError::new(ErrorKind::Protocol, "gRPC 工作任务未完成")))
}

pub async fn fetch_dm_segment(
    context: GrpcContext,
    cid: i64,
    segment_index: i64,
    kind: i32,
) -> GrpcBytesResult {
    let (ctx, transport) = context.split();
    let result = flatten(
        init::runtime()
            .spawn(async move {
                grpc_danmaku::fetch_segment(&transport, &ctx, cid, segment_index, kind).await
            })
            .await,
    );
    match result {
        Ok(data) => GrpcBytesResult {
            data: Some(data),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(error) => GrpcBytesResult {
            data: None,
            error_kind: Some(error.kind().to_string()),
            error_detail: Some(error.detail().into()),
            error_code: error.code(),
        },
    }
}

pub async fn fetch_dm_subtitles(context: GrpcContext, aid: i64, cid: i64) -> GrpcSubtitlesResult {
    let (ctx, transport) = context.split();
    let result = flatten(
        init::runtime()
            .spawn(async move { grpc_danmaku::fetch_subtitles(&transport, &ctx, aid, cid).await })
            .await,
    );
    match result {
        Ok(data) => GrpcSubtitlesResult {
            data: Some(
                data.into_iter()
                    .map(|s| GrpcSubtitle {
                        id: s.id,
                        lan: s.lan,
                        lan_doc: s.lan_doc,
                        subtitle_url: s.subtitle_url,
                        subtitle_type: s.subtitle_type,
                        is_ai: s.is_ai,
                    })
                    .collect(),
            ),
            error_kind: None,
            error_detail: None,
            error_code: None,
        },
        Err(error) => GrpcSubtitlesResult {
            data: None,
            error_kind: Some(error.kind().to_string()),
            error_detail: Some(error.detail().into()),
            error_code: error.code(),
        },
    }
}
