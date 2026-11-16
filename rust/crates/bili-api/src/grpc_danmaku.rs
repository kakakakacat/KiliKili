//! 已有手写解析的弹幕与字幕路径；这里只补完整网络边界，不生成响应消息树。
use crate::{
    grpc,
    http::{HttpTransport, RequestContext, RetryPolicy},
};
use bili_domain::{error::DomainError, subtitle::Subtitle};
use bili_grpc::bilibili::community::service::dm::v1::{DmSegMobileReq, DmViewReq};
use bili_protocol::grpc as protocol;
use prost::Message;

pub fn segment_request(cid: i64, segment_index: i64, kind: i32) -> Vec<u8> {
    DmSegMobileReq {
        oid: Some(cid),
        segment_index: Some(segment_index),
        r#type: Some(kind),
        ..Default::default()
    }
    .encode_to_vec()
}
pub fn view_request(aid: i64, cid: i64) -> Vec<u8> {
    DmViewReq {
        pid: Some(aid),
        oid: Some(cid),
        r#type: Some(1),
        ..Default::default()
    }
    .encode_to_vec()
}
pub async fn fetch_segment<T: HttpTransport>(
    transport: &T,
    context: &RequestContext,
    cid: i64,
    segment_index: i64,
    kind: i32,
) -> Result<Vec<u8>, DomainError> {
    grpc::unary(
        transport,
        context,
        protocol::DM_SEG_MOBILE,
        &segment_request(cid, segment_index, kind),
        RetryPolicy::Read,
    )
    .await
}
pub async fn fetch_subtitles<T: HttpTransport>(
    transport: &T,
    context: &RequestContext,
    aid: i64,
    cid: i64,
) -> Result<Vec<Subtitle>, DomainError> {
    let bytes = grpc::unary(
        transport,
        context,
        protocol::DM_VIEW,
        &view_request(aid, cid),
        RetryPolicy::Read,
    )
    .await?;
    crate::dm_view::parse_subtitles(&bytes)
}
