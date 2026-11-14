//! 动态功能模块接口：gRPC 读取、REST 预约和写操作。

use bili_domain::{
    dynamic::{DynamicAuthor, LikeListPage, ReserveAction, ReserveInfo, UnreadCount},
    error::{DomainError, ErrorKind},
};
use prost::Message;
use serde_json::json;

use crate::http::Idempotency;
use crate::{
    grpc,
    http::{HttpTransport, RequestContext, RetryPolicy},
    request::{CsrfIn, PreparedRequest, PreparedWrite, WriteBody},
};

pub const RESERVE_INFO_PATH: &str = "/x/new-reserve/up/reserve/info";
pub const CREATE_RESERVE_PATH: &str = "/x/new-reserve/up/reserve/create";
pub const UPDATE_RESERVE_PATH: &str = "/x/new-reserve/up/reserve/update";
pub const DYN_RESERVE_PATH: &str = "/x/dynamic/feed/reserve/click";

fn reserve_info_request(sid: i64) -> PreparedRequest {
    PreparedRequest {
        path: RESERVE_INFO_PATH,
        query: vec![
            ("from".to_owned(), "1".to_owned()),
            ("id".to_owned(), sid.to_string()),
            ("web_location".to_owned(), "333.1365".to_owned()),
        ],
    }
}

fn reserve_write(
    path: &'static str,
    sub_type: i32,
    title: &str,
    start: i64,
    sid: Option<i64>,
) -> PreparedWrite {
    let mut form = vec![
        ("type".to_owned(), "2".to_owned()),
        ("sub_type".to_owned(), sub_type.to_string()),
        ("from".to_owned(), "1".to_owned()),
        ("title".to_owned(), title.to_owned()),
        ("live_plan_start_time".to_owned(), start.to_string()),
    ];
    if let Some(sid) = sid {
        form.push(("id".to_owned(), sid.to_string()));
    }
    PreparedWrite {
        path,
        query: Vec::new(),
        body: WriteBody::Form(form),
        csrf: CsrfIn::Body,
        idempotency: Idempotency::Unsafe,
    }
}

#[must_use]
pub fn create_reserve_request(sub_type: i32, title: &str, start: i64) -> PreparedWrite {
    reserve_write(CREATE_RESERVE_PATH, sub_type, title, start, None)
}

#[must_use]
pub fn update_reserve_request(sid: i64, sub_type: i32, title: &str, start: i64) -> PreparedWrite {
    reserve_write(UPDATE_RESERVE_PATH, sub_type, title, start, Some(sid))
}

#[must_use]
pub fn dyn_reserve_request(
    reserve_id: Option<i64>,
    status: Option<i32>,
    dynamic_id: &str,
    total: Option<i64>,
) -> PreparedWrite {
    let mut body = serde_json::Map::new();
    if let Some(id) = reserve_id {
        body.insert("reserve_id".to_owned(), json!(id));
    }
    if let Some(status) = status {
        body.insert("cur_btn_status".to_owned(), json!(status));
    }
    body.insert("dynamic_id_str".to_owned(), json!(dynamic_id));
    if let Some(total) = total {
        body.insert("reserve_total".to_owned(), json!(total));
    }
    PreparedWrite {
        path: DYN_RESERVE_PATH,
        query: Vec::new(),
        body: WriteBody::Json(serde_json::Value::Object(body)),
        csrf: CsrfIn::Query,
        idempotency: Idempotency::Unsafe,
    }
}

fn unwrap_reserve<T: serde::de::DeserializeOwned>(body: &str) -> Result<T, DomainError> {
    crate::envelope::unwrap_envelope(body)
}

pub async fn fetch_reserve_info<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    sid: i64,
) -> Result<ReserveInfo, DomainError> {
    let body = crate::client::send(transport, ctx, &reserve_info_request(sid)).await?;
    unwrap_reserve(&body)
}

pub async fn send_create_reserve<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    sub_type: i32,
    title: &str,
    start: i64,
) -> Result<i64, DomainError> {
    let body = crate::client::send_write(
        transport,
        ctx,
        &create_reserve_request(sub_type, title, start),
    )
    .await?;
    let value: serde_json::Value = unwrap_reserve(&body)?;
    value
        .get("sid")
        .and_then(serde_json::Value::as_i64)
        .ok_or_else(|| DomainError::new(ErrorKind::Protocol, "创建预约响应缺少 sid"))
}

pub async fn send_update_reserve<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    sid: i64,
    sub_type: i32,
    title: &str,
    start: i64,
) -> Result<i64, DomainError> {
    let body = crate::client::send_write(
        transport,
        ctx,
        &update_reserve_request(sid, sub_type, title, start),
    )
    .await?;
    let value: serde_json::Value = unwrap_reserve(&body)?;
    value
        .get("sid")
        .and_then(serde_json::Value::as_i64)
        .ok_or_else(|| DomainError::new(ErrorKind::Protocol, "更新预约响应缺少 sid"))
}

pub async fn send_dyn_reserve<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    reserve_id: Option<i64>,
    status: Option<i32>,
    dynamic_id: &str,
    total: Option<i64>,
) -> Result<ReserveAction, DomainError> {
    let body = crate::client::send_write(
        transport,
        ctx,
        &dyn_reserve_request(reserve_id, status, dynamic_id, total),
    )
    .await?;
    unwrap_reserve(&body)
}

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
pub const PRIVATE_PUB_PATH: &str = "/x/dynamic/feed/dyn/private_pub_setting";

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

#[must_use]
pub fn private_pub_request(dynamic_id: &str, dyn_type: Option<i64>, action: &str) -> PreparedWrite {
    let mut object_id = serde_json::Map::new();
    object_id.insert("dyn_id".to_owned(), json!(dynamic_id));
    if let Some(dyn_type) = dyn_type {
        object_id.insert("dyn_type".to_owned(), json!(dyn_type));
    }
    PreparedWrite {
        path: PRIVATE_PUB_PATH,
        query: vec![("platform".to_owned(), "web".to_owned())],
        body: WriteBody::Json(json!({
            "object_id": serde_json::Value::Object(object_id).to_string(),
            "action": action,
        })),
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
) -> Result<(), DomainError> {
    crate::video_write::send_write(transport, context, &thumb_request(dynamic_id, up)).await
}

/// # Errors
/// 传输或业务信封失败时返回分类错误。
pub async fn send_top<T: HttpTransport>(
    transport: &T,
    context: &RequestContext,
    dynamic_id: &str,
    set_top: bool,
) -> Result<(), DomainError> {
    crate::video_write::send_write(transport, context, &top_request(dynamic_id, set_top)).await
}

pub async fn send_private_pub<T: HttpTransport>(
    transport: &T,
    context: &RequestContext,
    dynamic_id: &str,
    dyn_type: Option<i64>,
    action: &str,
) -> Result<(), DomainError> {
    crate::video_write::send_write(
        transport,
        context,
        &private_pub_request(dynamic_id, dyn_type, action),
    )
    .await
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

    #[test]
    fn reserve_requests_preserve_fields_and_csrf_locations() {
        let info = reserve_info_request(42);
        assert_eq!(info.path, RESERVE_INFO_PATH);
        assert_eq!(
            info.query,
            [
                ("from".to_owned(), "1".to_owned()),
                ("id".to_owned(), "42".to_owned()),
                ("web_location".to_owned(), "333.1365".to_owned()),
            ]
        );

        let create = create_reserve_request(3, "预约", 1_800_000_000);
        assert_eq!(create.path, CREATE_RESERVE_PATH);
        assert_eq!(create.csrf, CsrfIn::Body);
        assert_eq!(create.idempotency, Idempotency::Unsafe);
        assert_eq!(
            create.body,
            WriteBody::Form(vec![
                ("type".to_owned(), "2".to_owned()),
                ("sub_type".to_owned(), "3".to_owned()),
                ("from".to_owned(), "1".to_owned()),
                ("title".to_owned(), "预约".to_owned()),
                ("live_plan_start_time".to_owned(), "1800000000".to_owned()),
            ])
        );

        let update = update_reserve_request(42, 0, "更新", 1_800_000_001);
        assert_eq!(update.path, UPDATE_RESERVE_PATH);
        assert!(
            matches!(update.body, WriteBody::Form(ref fields) if fields.last() == Some(&("id".to_owned(), "42".to_owned())))
        );

        let click = dyn_reserve_request(Some(7), Some(1), "dyn-42", Some(9));
        assert_eq!(click.path, DYN_RESERVE_PATH);
        assert_eq!(click.csrf, CsrfIn::Query);
        assert_eq!(
            click.body,
            WriteBody::Json(json!({
                "reserve_id": 7, "cur_btn_status": 1,
                "dynamic_id_str": "dyn-42", "reserve_total": 9
            }))
        );
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn reserve_results_parse_typed_payloads() {
        let info: ReserveInfo = unwrap_reserve(
            r#"{"code":0,"data":{"id":42,"title":"预约","live_plan_start_time":1800000000}}"#,
        )
        .expect("reserve info should parse");
        assert_eq!(info.id, 42);
        assert_eq!(info.title, "预约");

        let action: ReserveAction = unwrap_reserve(
            r#"{"code":0,"data":{"final_btn_status":1,"reserve_update":9,"desc_update":"已预约"}}"#,
        )
        .expect("reserve action should parse");
        assert_eq!(action.final_btn_status, Some(1));
        assert_eq!(action.reserve_update, Some(9));
        assert_eq!(action.desc_update.as_deref(), Some("已预约"));
    }
}
