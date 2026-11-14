//! 动态 gRPC 接口。当前先迁首页红点，其他接口随 #55 后续路径接入。

use bili_domain::{
    dynamic::{DynamicAuthor, LikeListPage, UnreadCount},
    error::{DomainError, ErrorKind},
};
use prost::Message;
use serde_json::json;

use crate::http::Idempotency;
use crate::{
    grpc,
    http::{HttpTransport, RequestContext, RetryPolicy},
    request::{CsrfIn, PreparedWrite, WriteBody},
};
use bili_domain::error::DomainError as DynamicError;

/// 与 Dart 的 `DynRedReq(tabOffset: [TabOffset(tab: 1)])` 字节一致。
/// 这里只需四个字段，保留局部消息定义，避免引入整棵动态协议树。
#[derive(Clone, PartialEq, Message)]
struct TabOffset {
    #[prost(int32, optional, tag = "1")]
    tab: Option<i32>,
}

#[derive(Clone, PartialEq, Message)]
struct DynRedReq {
    #[prost(message, repeated, tag = "1")]
    tab_offset: Vec<TabOffset>,
}

#[derive(Clone, PartialEq, Message)]
struct DynRedItem {
    #[prost(int64, optional, tag = "1")]
    count: Option<i64>,
}

#[derive(Clone, PartialEq, Message)]
struct DynRedReply {
    #[prost(message, optional, tag = "2")]
    dyn_red_item: Option<DynRedItem>,
}

pub const DYN_RED_PATH: &str = "/bilibili.app.dynamic.v1.Dynamic/DynRed";
pub const LIKE_LIST_PATH: &str = "/bilibili.app.dynamic.v2.Dynamic/LikeList";
pub const THUMB_PATH: &str = "/x/dynamic/feed/dyn/thumb";
pub const SET_TOP_PATH: &str = "/x/dynamic/feed/space/set_top";
pub const RM_TOP_PATH: &str = "/x/dynamic/feed/space/rm_top";

/// 点赞与取消赞都是目标状态（1 / 2），重试是幂等的。
#[must_use]
pub fn thumb_request(dynamic_id: &str, up: i32) -> PreparedWrite {
    PreparedWrite {
        path: THUMB_PATH,
        query: Vec::new(),
        body: WriteBody::Json(json!({
            "dyn_id_str": dynamic_id,
            "up": up,
            "spmid": "333.1365.0.0",
        })),
        csrf: CsrfIn::Query,
        idempotency: Idempotency::Idempotent,
    }
}

/// 置顶与取消置顶是两条不同路径，请求体相同。
#[must_use]
pub fn top_request(dynamic_id: &str, set_top: bool) -> PreparedWrite {
    PreparedWrite {
        path: if set_top { SET_TOP_PATH } else { RM_TOP_PATH },
        query: Vec::new(),
        body: WriteBody::Json(json!({ "dyn_str": dynamic_id })),
        csrf: CsrfIn::Query,
        idempotency: Idempotency::Idempotent,
    }
}

/// # Errors
/// 传输或业务信封失败时返回分类错误。
pub async fn send_thumb<T: HttpTransport>(
    transport: &T,
    context: &RequestContext,
    dynamic_id: &str,
    up: i32,
) -> Result<(), DynamicError> {
    crate::video_write::send_write(transport, context, &thumb_request(dynamic_id, up)).await
}

/// # Errors
/// 传输或业务信封失败时返回分类错误。
pub async fn send_top<T: HttpTransport>(
    transport: &T,
    context: &RequestContext,
    dynamic_id: &str,
    set_top: bool,
) -> Result<(), DynamicError> {
    crate::video_write::send_write(transport, context, &top_request(dynamic_id, set_top)).await
}

#[derive(Clone, PartialEq, Message)]
struct LikeListReq {
    #[prost(string, optional, tag = "1")]
    dynamic_id: Option<String>,
    #[prost(int64, optional, tag = "2")]
    dyn_type: Option<i64>,
    #[prost(int64, optional, tag = "3")]
    rid: Option<i64>,
    #[prost(int64, optional, tag = "4")]
    uid_offset: Option<i64>,
    #[prost(int32, optional, tag = "5")]
    page: Option<i32>,
}

#[derive(Clone, PartialEq, Message)]
struct LikeListReply {
    #[prost(message, repeated, tag = "1")]
    list: Vec<ModuleAuthor>,
    #[prost(bool, optional, tag = "2")]
    has_more: Option<bool>,
    #[prost(int64, optional, tag = "3")]
    total_count: Option<i64>,
}

#[derive(Clone, PartialEq, Message)]
struct ModuleAuthor {
    #[prost(int64, optional, tag = "1")]
    mid: Option<i64>,
    #[prost(message, optional, tag = "3")]
    author: Option<UserInfo>,
}

#[derive(Clone, PartialEq, Message)]
struct UserInfo {
    #[prost(int64, optional, tag = "1")]
    mid: Option<i64>,
    #[prost(string, optional, tag = "2")]
    name: Option<String>,
    #[prost(string, optional, tag = "3")]
    face: Option<String>,
    #[prost(message, optional, tag = "4")]
    official: Option<OfficialVerify>,
    #[prost(message, optional, tag = "5")]
    vip: Option<VipInfo>,
    #[prost(message, optional, tag = "8")]
    pendant: Option<UserPendant>,
    #[prost(int32, optional, tag = "10")]
    level: Option<i32>,
    #[prost(string, optional, tag = "11")]
    sign: Option<String>,
    #[prost(int32, optional, tag = "15")]
    is_senior_member: Option<i32>,
}

#[derive(Clone, PartialEq, Message)]
struct OfficialVerify {
    #[prost(int32, optional, tag = "1")]
    r#type: Option<i32>,
}

#[derive(Clone, PartialEq, Message)]
struct VipInfo {
    #[prost(int32, optional, tag = "1")]
    r#type: Option<i32>,
    #[prost(int32, optional, tag = "2")]
    status: Option<i32>,
}

#[derive(Clone, PartialEq, Message)]
struct UserPendant {
    #[prost(string, optional, tag = "3")]
    image: Option<String>,
}

/// 不传的可选字段必须整个缺席，不能填 protobuf 零值：基线构造器就是如此。
#[must_use]
pub fn like_list_request(
    dynamic_id: &str,
    dyn_type: Option<i64>,
    rid: Option<i64>,
    uid_offset: Option<i64>,
    page: i32,
) -> Vec<u8> {
    LikeListReq {
        dynamic_id: Some(dynamic_id.to_owned()),
        dyn_type,
        rid,
        uid_offset,
        page: Some(page),
    }
    .encode_to_vec()
}

/// # Errors
/// protobuf 消息损坏时返回协议错误。
pub fn parse_like_list(bytes: &[u8]) -> Result<LikeListPage, DomainError> {
    let reply = LikeListReply::decode(bytes)
        .map_err(|_| DomainError::new(ErrorKind::Protocol, "动态点赞列表 protobuf 解码失败"))?;
    let items = reply
        .list
        .into_iter()
        .map(|item| {
            let user = item.author.unwrap_or_default();
            DynamicAuthor {
                mid: item.mid.unwrap_or_default(),
                name: user.name.unwrap_or_default(),
                face: user.face.unwrap_or_default(),
                sign: user.sign.unwrap_or_default(),
                level: user.level.unwrap_or_default(),
                is_senior_member: user.is_senior_member.unwrap_or_default() == 1,
                pendant_image: user.pendant.map(|p| p.image.unwrap_or_default()),
                vip_status: user.vip.as_ref().map(|v| v.status.unwrap_or_default()),
                vip_type: user.vip.map(|v| v.r#type.unwrap_or_default()),
                official_type: user.official.map(|o| o.r#type.unwrap_or_default()),
            }
        })
        .collect();
    Ok(LikeListPage {
        items,
        has_more: reply.has_more.unwrap_or_default(),
        total_count: reply.total_count.unwrap_or_default(),
    })
}

/// # Errors
/// 传输、gRPC 状态或 protobuf 解析失败时返回分类错误。
pub async fn fetch_like_list<T: HttpTransport>(
    transport: &T,
    context: &RequestContext,
    dynamic_id: &str,
    dyn_type: Option<i64>,
    rid: Option<i64>,
    uid_offset: Option<i64>,
    page: i32,
) -> Result<LikeListPage, DomainError> {
    let bytes = grpc::unary(
        transport,
        context,
        LIKE_LIST_PATH,
        &like_list_request(dynamic_id, dyn_type, rid, uid_offset, page),
        RetryPolicy::Read,
    )
    .await?;
    parse_like_list(&bytes)
}

#[must_use]
pub fn dyn_red_request() -> Vec<u8> {
    DynRedReq {
        tab_offset: vec![TabOffset { tab: Some(1) }],
    }
    .encode_to_vec()
}

/// protobuf.dart 对缺失的嵌套消息和 int64 标量均返回零值。
/// 解码失败才是协议错误，不把它误当成没有新动态。
pub fn parse_dyn_red(bytes: &[u8]) -> Result<UnreadCount, DomainError> {
    let reply = DynRedReply::decode(bytes)
        .map_err(|_| DomainError::new(ErrorKind::Protocol, "动态红点响应 protobuf 解码失败"))?;
    Ok(UnreadCount {
        count: reply.dyn_red_item.and_then(|item| item.count).unwrap_or(0),
    })
}

pub async fn fetch_dyn_red<T: HttpTransport>(
    transport: &T,
    context: &RequestContext,
) -> Result<UnreadCount, DomainError> {
    let bytes = grpc::unary(
        transport,
        context,
        DYN_RED_PATH,
        &dyn_red_request(),
        RetryPolicy::Read,
    )
    .await?;
    parse_dyn_red(&bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_matches_dart_protobuf() {
        assert_eq!(dyn_red_request(), [0x0a, 0x02, 0x08, 0x01]);
    }

    #[test]
    fn count_and_missing_fields_match_protobuf_defaults() {
        assert!(matches!(
            parse_dyn_red(&[0x12, 0x02, 0x08, 0x2a]),
            Ok(UnreadCount { count: 42 })
        ));
        assert!(matches!(parse_dyn_red(&[]), Ok(UnreadCount { count: 0 })));
        assert!(matches!(
            parse_dyn_red(&[0x12, 0x00]),
            Ok(UnreadCount { count: 0 })
        ));
        assert!(parse_dyn_red(&[0x12, 0x02, 0x08]).is_err());
    }

    #[test]
    fn like_list_request_keeps_optional_fields_absent() {
        assert_eq!(
            like_list_request("d", None, None, None, 1),
            [0x0a, 0x01, b'd', 0x28, 0x01]
        );
    }

    #[test]
    fn like_list_projects_author_and_page() {
        // LikeListReply{list:[ModuleAuthor{mid:7,author:{name:"A",sign:"hi"}}],
        // has_more:true,total_count:1} 的 protobuf 字节，不依赖本模块的编码器。
        let bytes = [
            0x0a, 0x0b, 0x08, 0x07, 0x1a, 0x07, 0x12, 0x01, b'A', 0x5a, 0x02, b'h', b'i', 0x10,
            0x01, 0x18, 0x01,
        ];
        assert!(matches!(
            parse_like_list(&bytes),
            Ok(LikeListPage { items, has_more: true, total_count: 1 })
                if items.len() == 1 && items[0].mid == 7 && items[0].name == "A"
                    && items[0].sign == "hi"
        ));
    }

    #[test]
    fn dynamic_write_requests_keep_csrf_in_query_and_json_body() {
        let thumb = thumb_request("dyn-1", 2);
        assert_eq!(thumb.path, THUMB_PATH);
        assert_eq!(thumb.csrf, CsrfIn::Query);
        assert_eq!(thumb.idempotency, Idempotency::Idempotent);
        assert_eq!(
            thumb.body,
            WriteBody::Json(json!({
                "dyn_id_str": "dyn-1", "up": 2, "spmid": "333.1365.0.0"
            }))
        );
        for (set_top, path) in [(true, SET_TOP_PATH), (false, RM_TOP_PATH)] {
            let top = top_request("dyn-1", set_top);
            assert_eq!(top.path, path);
            assert_eq!(top.csrf, CsrfIn::Query);
            assert_eq!(top.idempotency, Idempotency::Idempotent);
            assert_eq!(top.body, WriteBody::Json(json!({ "dyn_str": "dyn-1" })));
        }
    }
}
