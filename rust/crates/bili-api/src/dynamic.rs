//! 动态功能模块接口：gRPC 读取、REST 预约和写操作。

use bili_domain::{
    dynamic::{
        ArticleInfo, ArticleList, ArticleListInfo, ArticleListItem, ArticleListItemStats,
        ArticleStats, Bubble, BubbleBaseInfo, BubbleCategory, BubbleContent, BubbleDyn,
        BubbleSortInfo, BubbleSortItem, DynamicAuthor, FolloweeVote, LikeListPage, ReserveAction,
        ReserveInfo, TopicCreator, TopicItem, TopicPubSearch, TopicTop, UnreadCount, VoteInfo,
    },
    error::{DomainError, ErrorKind},
};
use prost::Message;
use serde::Deserialize;
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
pub const VOTE_INFO_PATH: &str = "/x/vote/vote_info";
pub const DO_VOTE_PATH: &str = "/x/vote/do_vote";
pub const CREATE_VOTE_PATH: &str = "/x/vote/create";
pub const UPDATE_VOTE_PATH: &str = "/x/vote/update";
pub const FOLLOWEE_VOTES_PATH: &str = "/vote_svr/v1/vote_svr/followee_votes";

#[must_use]
pub fn vote_info_request(vote_id: i64) -> PreparedRequest {
    PreparedRequest {
        path: VOTE_INFO_PATH,
        query: vec![("vote_id".to_owned(), vote_id.to_string())],
    }
}

fn vote_json(info: &VoteInfo) -> serde_json::Value {
    let mut value = json!({
        "title": info.title,
        "desc": info.desc,
        "type": info.vote_type,
        "choice_cnt": info.choice_cnt,
        "duration": info.duration,
        "options": info.options.iter().map(|option| json!({
            "opt_desc": option.opt_desc,
            "img_url": option.img_url,
        })).collect::<Vec<_>>(),
        "only_fans_level": info.only_fans_level,
        "vote_publisher": info.vote_publisher,
        "vote_id": info.vote_id,
    });
    if info.vote_id.is_none()
        && let Some(object) = value.as_object_mut()
    {
        object.remove("vote_id");
    }
    value
}

#[must_use]
pub fn do_vote_request(
    vote_id: i64,
    votes: &[i32],
    anonymous: bool,
    dynamic_id: Option<i64>,
    voter_uid: Option<i64>,
) -> PreparedWrite {
    PreparedWrite {
        path: DO_VOTE_PATH,
        query: Vec::new(),
        body: WriteBody::Json(json!({
            "vote_id": vote_id,
            "votes": votes,
            "voter_uid": voter_uid.unwrap_or_default(),
            "status": i32::from(anonymous),
            "op_bit": 0,
            "dynamic_id": dynamic_id.unwrap_or_default(),
        })),
        csrf: CsrfIn::BodyAndQueryWithToken,
        idempotency: Idempotency::Unsafe,
    }
}

#[must_use]
pub fn save_vote_request(info: &VoteInfo, update: bool) -> PreparedWrite {
    PreparedWrite {
        path: if update {
            UPDATE_VOTE_PATH
        } else {
            CREATE_VOTE_PATH
        },
        query: Vec::new(),
        body: WriteBody::Json(json!({ "vote_info": vote_json(info) })),
        csrf: CsrfIn::Query,
        idempotency: Idempotency::Unsafe,
    }
}

#[must_use]
pub fn followee_votes_request(vote_id: i64) -> PreparedRequest {
    PreparedRequest {
        path: FOLLOWEE_VOTES_PATH,
        query: vec![("vote_id".to_owned(), vote_id.to_string())],
    }
}

pub fn parse_vote_info(body: &str) -> Result<VoteInfo, DomainError> {
    let data: serde_json::Value = crate::envelope::unwrap_envelope(body)?;
    let mut info: VoteInfo = serde_json::from_value(
        data.get("vote_info")
            .cloned()
            .ok_or_else(|| DomainError::new(ErrorKind::Protocol, "投票响应缺少 vote_info"))?,
    )
    .map_err(|_| DomainError::new(ErrorKind::Protocol, "投票信息结构无法解析"))?;
    info.my_votes = data
        .get("my_votes")
        .and_then(serde_json::Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(serde_json::Value::as_i64)
                .filter_map(|value| i32::try_from(value).ok())
                .collect()
        });
    if info.vote_id.is_none() {
        return Err(DomainError::new(ErrorKind::Protocol, "无效的投票 id"));
    }
    Ok(info)
}

pub fn parse_vote_result(body: &str) -> Result<VoteInfo, DomainError> {
    let data: serde_json::Value = crate::envelope::unwrap_envelope(body)?;
    serde_json::from_value(
        data.get("vote_info")
            .cloned()
            .ok_or_else(|| DomainError::new(ErrorKind::Protocol, "投票结果缺少 vote_info"))?,
    )
    .map_err(|_| DomainError::new(ErrorKind::Protocol, "投票结果结构无法解析"))
}

pub async fn fetch_vote_info<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    vote_id: i64,
) -> Result<VoteInfo, DomainError> {
    let body = crate::client::send(transport, ctx, &vote_info_request(vote_id)).await?;
    parse_vote_info(&body)
}

pub async fn send_do_vote<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    vote_id: i64,
    votes: &[i32],
    anonymous: bool,
    dynamic_id: Option<i64>,
    voter_uid: Option<i64>,
) -> Result<VoteInfo, DomainError> {
    let body = crate::client::send_write(
        transport,
        ctx,
        &do_vote_request(vote_id, votes, anonymous, dynamic_id, voter_uid),
    )
    .await?;
    parse_vote_result(&body)
}

pub async fn send_save_vote<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    info: &VoteInfo,
    update: bool,
) -> Result<i64, DomainError> {
    let body = crate::client::send_write(transport, ctx, &save_vote_request(info, update)).await?;
    let data: serde_json::Value = crate::envelope::unwrap_envelope(&body)?;
    data.get("vote_id")
        .and_then(serde_json::Value::as_i64)
        .or(info.vote_id)
        .ok_or_else(|| DomainError::new(ErrorKind::Protocol, "保存投票响应缺少 vote_id"))
}

pub async fn fetch_followee_votes<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    vote_id: i64,
) -> Result<Vec<FolloweeVote>, DomainError> {
    let body = crate::client::send(transport, ctx, &followee_votes_request(vote_id)).await?;
    let data: serde_json::Value = crate::envelope::unwrap_envelope(&body)?;
    serde_json::from_value(data.get("votes").cloned().unwrap_or_else(|| json!([])))
        .map_err(|_| DomainError::new(ErrorKind::Protocol, "关注者投票列表结构无法解析"))
}

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

    #[test]
    fn voting_requests_keep_legacy_csrf_and_wire_fields() {
        let read = vote_info_request(12);
        assert_eq!(read.path, VOTE_INFO_PATH);
        assert_eq!(read.query, [("vote_id".to_owned(), "12".to_owned())]);

        let vote = VoteInfo {
            choice_cnt: Some(1),
            default_share: None,
            desc: Some("desc".to_owned()),
            end_time: None,
            status: None,
            uid: None,
            vote_id: None,
            join_num: 0,
            title: Some("title".to_owned()),
            ctime: None,
            my_votes: None,
            options: vec![bili_domain::dynamic::VoteOption {
                opt_idx: None,
                opt_desc: Some("A".to_owned()),
                cnt: 0,
                img_url: None,
            }],
            options_cnt: None,
            voter_level: None,
            face: None,
            name: None,
            vote_type: Some(0),
            vote_publisher: Some(5),
            duration: Some(60),
            only_fans_level: Some(0),
        };
        let create = save_vote_request(&vote, false);
        assert_eq!(create.path, CREATE_VOTE_PATH);
        assert_eq!(create.csrf, CsrfIn::Query);
        assert_eq!(create.idempotency, Idempotency::Unsafe);
        let WriteBody::Json(body) = create.body else {
            panic!("投票保存请求为 JSON")
        };
        assert!(body["vote_info"].get("vote_id").is_none());
        assert_eq!(body["vote_info"]["options"][0]["opt_desc"], "A");

        let submit = do_vote_request(12, &[1, 3], true, Some(99), Some(5));
        assert_eq!(submit.path, DO_VOTE_PATH);
        assert_eq!(submit.csrf, CsrfIn::BodyAndQueryWithToken);
        assert_eq!(
            submit.body,
            WriteBody::Json(json!({
                "vote_id": 12, "votes": [1, 3], "voter_uid": 5,
                "status": 1, "op_bit": 0, "dynamic_id": 99
            }))
        );
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn vote_info_parser_combines_embedded_data_and_my_votes() {
        let info = parse_vote_info(r#"{"code":0,"data":{"vote_info":{"vote_id":12,"title":"poll","type":0,"options":[{"opt_idx":1,"opt_desc":"A"}]},"my_votes":[1]}}"#)
            .expect("vote response should parse");
        assert_eq!(info.vote_id, Some(12));
        assert_eq!(info.my_votes, Some(vec![1]));
        assert_eq!(info.options[0].cnt, 0);
    }
}

// ============================================ S5 续单第一批（issue #150）
//
// 本批迁的是 `lib/http/dynamics.dart` 里**不依赖多态卡片树**的那几条。
//
// 为什么这样切：`lib/models/dynamics/result.dart`（1426 行的
// `DynamicsDataModel` / `DynamicItemModel`）是动态域的枢纽——主列表、详情、
// 转发列表、话题 feed 全挂在它上面，必须整棵一起迁。而专栏、文集、话题
// 顶部、部落气泡这几条的模型自成一体，先迁它们能在不动枢纽的前提下清掉
// 一批接口，也让卡片树那个 PR 只剩一件事可审。
//
// **本批刻意没迁**（都在 #150 的后续批次里，理由写在这免得下一个人重走）：
//
// - `articleView`：它的 `opus.content.paragraphs` 挂着
//   `ArticleContentModel`（492 行的富文本树），和卡片树是同一类工作量；
// - `dynMention`：`MentionItem with MultiSelectData` 掺着 widget 层的多选
//   mixin，要连同选择页一起改（与 #57 里历史记录那一族同样的形状）；
// - `topicFeed` / `topicFold` / `dynamicDetail` / `opusDetail` /
//   `followDynamic`：全部经过 `DynamicItemModel`。

pub mod article_path {
    /// 专栏阅读页的计数与收藏态。**要 WBI 签名。**
    pub const ARTICLE_INFO: &str = "/x/article/viewinfo";
    /// 文集。
    pub const ARTICLE_LIST: &str = "/x/article/list/web/articles";
}

pub mod topic_path {
    /// 话题页顶部。挂在 **app** 网关（`HttpString.appBaseUrl`）上，
    /// 不是其余接口的 `api.bilibili.com`——base_url 由调用方给，别在
    /// 这里按习惯补成后者。
    pub const TOPIC_TOP: &str = "/x/topic/web/details/top";
    /// 推荐话题。
    pub const TOPIC_RCMD: &str = "/x/topic/web/dynamic/rcmd";
    /// 发布页的话题搜索。同样在 app 网关上。
    pub const TOPIC_PUB_SEARCH: &str = "/x/topic/pub/search";
}

pub mod bubble_path {
    /// 动态页顶部的部落气泡。
    pub const BUBBLE: &str = "/x/tribee/v1/dyn/all";
}

/// 专栏计数的请求。
///
/// 这一条**要 WBI 签名**，另外三个固定参数（`mobi_app=pc`、`from=web`、
/// `gaia_source=main_web`）是基线写死的，一并进签名——漏掉任何一个
/// 服务端都回 `-403`，而那个码看起来像没登录。
#[must_use]
pub fn article_info_request(cv_id: &str, mixin_key: &str, wts: i64) -> PreparedRequest {
    let mut query = vec![
        ("id".to_owned(), cv_id.to_owned()),
        ("mobi_app".to_owned(), "pc".to_owned()),
        ("from".to_owned(), "web".to_owned()),
        ("gaia_source".to_owned(), "main_web".to_owned()),
        ("wts".to_owned(), wts.to_string()),
    ];
    query.sort_by(|a, b| a.0.cmp(&b.0));
    let w_rid = bili_protocol::wbi::sign(&query, mixin_key);
    query.push(("w_rid".to_owned(), w_rid));
    PreparedRequest {
        path: article_path::ARTICLE_INFO,
        query,
    }
}

/// 文集的请求。`web_location` 是基线写死的常量。
#[must_use]
pub fn article_list_request(id: &str) -> PreparedRequest {
    PreparedRequest {
        path: article_path::ARTICLE_LIST,
        query: vec![
            ("id".to_owned(), id.to_owned()),
            ("web_location".to_owned(), "333.1400".to_owned()),
        ],
    }
}

/// 话题页顶部的请求。
#[must_use]
pub fn topic_top_request(
    topic_id: &str,
    access_key: Option<&str>,
    appkey: &str,
    appsec: &str,
    ts: i64,
) -> PreparedRequest {
    PreparedRequest {
        path: topic_path::TOPIC_TOP,
        query: crate::request::sign_app_query(
            vec![
                ("topic_id".to_owned(), topic_id.to_owned()),
                ("source".to_owned(), "Web".to_owned()),
            ],
            access_key,
            appkey,
            appsec,
            ts,
        ),
    }
}

/// 推荐话题的请求。
#[must_use]
pub fn topic_rcmd_request(page_size: i64) -> PreparedRequest {
    PreparedRequest {
        path: topic_path::TOPIC_RCMD,
        query: vec![
            ("source".to_owned(), "Web".to_owned()),
            ("page_size".to_owned(), page_size.to_string()),
            ("web_location".to_owned(), "333.1365".to_owned()),
        ],
    }
}

/// 部落气泡的请求。
///
/// `category_id` 与 `sort_type` 都可省：基线用 `?:` 把 null 的那一项整条
/// 从查询串里拿掉，**不是**传空串。两者语义不同——传 `category_id=` 服务端
/// 当成「分类 id 为空串」，会回一个空列表。
///
/// `x-bili-device-req-json` 放在**查询串**里而不是请求头，这是基线的写法
/// （`sortFollowTag` 也是同一形状，见 `PreparedWrite::query` 的注释）。
#[must_use]
pub fn bubble_request(
    tribe_id: &str,
    category_id: Option<&str>,
    sort_type: Option<i64>,
    page: i64,
) -> PreparedRequest {
    let mut query = vec![("tribee_id".to_owned(), tribe_id.to_owned())];
    if let Some(id) = category_id {
        query.push(("category_id".to_owned(), id.to_owned()));
    }
    if let Some(sort) = sort_type {
        query.push(("sort_type".to_owned(), sort.to_string()));
    }
    query.push(("page_size".to_owned(), "20".to_owned()));
    query.push(("page_num".to_owned(), page.to_string()));
    query.push(("web_location".to_owned(), "333.40165".to_owned()));
    query.push((
        "x-bili-device-req-json".to_owned(),
        r#"{"platform":"web","device":"pc","spmid":"333.40165"}"#.to_owned(),
    ));
    PreparedRequest {
        path: bubble_path::BUBBLE,
        query,
    }
}

// ---------------------------------------------------------------- DTO

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ArticleInfoDto {
    favorite: Option<bool>,
    title: Option<String>,
    origin_image_urls: Option<Vec<String>>,
    stats: Option<ArticleStatsDto>,
}

/// 四个计数全是展示用的，整组放宽（#166 的判据）。
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ArticleStatsDto {
    #[serde(deserialize_with = "crate::lenient::opt_i64")]
    favorite: Option<i64>,
    #[serde(deserialize_with = "crate::lenient::opt_i64")]
    like: Option<i64>,
    #[serde(deserialize_with = "crate::lenient::opt_i64")]
    reply: Option<i64>,
    #[serde(deserialize_with = "crate::lenient::opt_i64")]
    share: Option<i64>,
}

/// 解析专栏计数。
///
/// # Errors
/// 信封失败沿用统一分类。
pub fn parse_article_info(body: &str) -> Result<ArticleInfo, DomainError> {
    let dto: ArticleInfoDto = crate::envelope::unwrap_envelope(body)?;
    Ok(ArticleInfo {
        favorite: dto.favorite,
        title: dto.title,
        origin_image_urls: dto.origin_image_urls.unwrap_or_default(),
        stats: dto.stats.map(|s| ArticleStats {
            favorite: s.favorite,
            like: s.like,
            reply: s.reply,
            share: s.share,
        }),
    })
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ArticleListDto {
    list: Option<ArticleListInfoDto>,
    articles: Option<Vec<ArticleListItemDto>>,
    author: Option<ArticleAuthorDto>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ArticleListInfoDto {
    /// 文集 id，**标识符，不放宽**（#166）。
    id: Option<i64>,
    name: Option<String>,
    image_url: Option<String>,
    update_time: Option<i64>,
    #[serde(deserialize_with = "crate::lenient::opt_i64")]
    words: Option<i64>,
    #[serde(deserialize_with = "crate::lenient::opt_i64")]
    read: Option<i64>,
    #[serde(deserialize_with = "crate::lenient::opt_i64")]
    articles_count: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ArticleListItemDto {
    /// cv 号，**标识符，不放宽**。
    id: Option<i64>,
    title: Option<String>,
    image_urls: Option<Vec<String>>,
    summary: Option<String>,
    dyn_id_str: Option<String>,
    stats: Option<ArticleListItemStatsDto>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ArticleListItemStatsDto {
    #[serde(deserialize_with = "crate::lenient::opt_i64")]
    view: Option<i64>,
    #[serde(deserialize_with = "crate::lenient::opt_i64")]
    like: Option<i64>,
    #[serde(deserialize_with = "crate::lenient::opt_i64")]
    reply: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ArticleAuthorDto {
    mid: Option<i64>,
    name: Option<String>,
    face: Option<String>,
}

/// 解析文集。
///
/// # Errors
/// 信封失败沿用统一分类。`articles` 缺失按空列表处理。
pub fn parse_article_list(body: &str) -> Result<ArticleList, DomainError> {
    let dto: ArticleListDto = crate::envelope::unwrap_envelope(body)?;
    Ok(ArticleList {
        info: dto.list.map(|l| ArticleListInfo {
            id: l.id,
            name: l.name,
            image_url: l.image_url,
            update_time: l.update_time,
            words: l.words,
            read: l.read,
            articles_count: l.articles_count,
        }),
        articles: dto
            .articles
            .unwrap_or_default()
            .into_iter()
            .map(|a| ArticleListItem {
                id: a.id,
                title: a.title,
                image_urls: a.image_urls.unwrap_or_default(),
                summary: a.summary,
                dyn_id_str: a.dyn_id_str,
                stats: a.stats.map(|s| ArticleListItemStats {
                    view: s.view,
                    like: s.like,
                    reply: s.reply,
                }),
            })
            .collect(),
        author_mid: dto.author.as_ref().and_then(|a| a.mid),
        author_name: dto.author.as_ref().and_then(|a| a.name.clone()),
        author_face: dto.author.and_then(|a| a.face),
    })
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct TopicItemDto {
    /// 话题 id，**标识符，不放宽**。解不出来整条丢掉，见 [`topic_item`]。
    id: Option<i64>,
    name: Option<String>,
    #[serde(deserialize_with = "crate::lenient::opt_i64")]
    view: Option<i64>,
    #[serde(deserialize_with = "crate::lenient::opt_i64")]
    discuss: Option<i64>,
    #[serde(deserialize_with = "crate::lenient::opt_i64")]
    fav: Option<i64>,
    #[serde(deserialize_with = "crate::lenient::opt_i64")]
    like: Option<i64>,
    description: Option<String>,
    is_fav: Option<bool>,
    is_like: Option<bool>,
}

/// 把 DTO 收成实体；`id` 或 `name` 缺失则整条丢掉。
///
/// 基线把这两个字段声明成非空（`json['id']` 直接赋给 `int id`，缺了就抛），
/// 那是**对协议的判断**：没有 id 的话题点不进去，名字空着也渲染不出一行。
/// 与其补一个 0 号话题让界面显示一个点不动的条目，不如不给。四个计数的
/// `?? 0` 同样照搬。
fn topic_item(dto: TopicItemDto) -> Option<TopicItem> {
    let name = dto.name?;
    Some(TopicItem {
        id: dto.id?,
        name,
        view: dto.view.unwrap_or(0),
        discuss: dto.discuss.unwrap_or(0),
        fav: dto.fav.unwrap_or(0),
        like: dto.like.unwrap_or(0),
        description: dto.description,
        is_fav: dto.is_fav,
        is_like: dto.is_like,
    })
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct TopicCreatorDto {
    uid: Option<i64>,
    face: Option<String>,
    name: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct TopicTopDto {
    top_details: Option<TopDetailsDto>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct TopDetailsDto {
    topic_item: Option<TopicItemDto>,
    topic_creator: Option<TopicCreatorDto>,
}

/// 解析话题页顶部。
///
/// # Errors
/// 信封失败沿用统一分类。
///
/// `top_details` 缺失时返回**两个字段都为空**的 [`TopicTop`]，而不是报错
/// ——基线在这里给的是 `null`，界面据此不渲染顶部。FRB 不支持
/// `Option<Option<T>>`，所以用「里面全空」表达「没有顶部」（AGENTS.md §4）。
pub fn parse_topic_top(body: &str) -> Result<TopicTop, DomainError> {
    let dto: TopicTopDto = crate::envelope::unwrap_envelope(body)?;
    let details = dto.top_details.unwrap_or_default();
    Ok(TopicTop {
        topic: details.topic_item.and_then(topic_item),
        creator: details.topic_creator.map(|c| TopicCreator {
            uid: c.uid,
            face: c.face,
            name: c.name,
        }),
    })
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct TopicRcmdDto {
    topic_items: Option<Vec<TopicItemDto>>,
}

/// 解析推荐话题。
///
/// # Errors
/// 信封失败沿用统一分类。`topic_items` 缺失按空列表处理。
pub fn parse_topic_rcmd(body: &str) -> Result<Vec<TopicItem>, DomainError> {
    let dto: Option<TopicRcmdDto> = crate::envelope::unwrap_envelope_nullable(body)?;
    Ok(dto
        .unwrap_or_default()
        .topic_items
        .unwrap_or_default()
        .into_iter()
        .filter_map(topic_item)
        .collect())
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct BubbleDto {
    base_info: Option<BubbleBaseInfoDto>,
    content: Option<BubbleContentDto>,
    category: Option<BubbleCategoryWrapDto>,
    sort_info: Option<BubbleSortInfoDto>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct BubbleBaseInfoDto {
    /// 协议里是 `tribee_info`，两个 e。不是笔误，是服务端的事实。
    tribee_info: Option<TribeInfoDto>,
    is_joined: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct TribeInfoDto {
    id: Option<String>,
    title: Option<String>,
    sub_title: Option<String>,
    face_url: Option<String>,
    jump_uri: Option<String>,
    summary: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct BubbleContentDto {
    /// 服务端给的是格式化好的串，**不要收成数字**。
    count: Option<String>,
    dyn_list: Option<Vec<BubbleDynDto>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct BubbleDynDto {
    dyn_id: Option<String>,
    title: Option<String>,
    meta: Option<BubbleMetaDto>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct BubbleMetaDto {
    author: Option<String>,
    time_text: Option<String>,
    reply_count: Option<String>,
    view_stat: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct BubbleCategoryWrapDto {
    category_list: Option<Vec<BubbleCategoryDto>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct BubbleCategoryDto {
    /// 分类 id 在这条接口上**是字符串**，要原样带回请求里。
    id: Option<String>,
    name: Option<String>,
    #[serde(rename = "type")]
    kind: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct BubbleSortInfoDto {
    show_sort: Option<bool>,
    sort_items: Option<Vec<BubbleSortItemDto>>,
    cur_sort_type: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct BubbleSortItemDto {
    sort_type: Option<i64>,
    text: Option<String>,
}

/// 解析部落气泡。
///
/// # Errors
/// 信封失败沿用统一分类。
pub fn parse_bubble(body: &str) -> Result<Bubble, DomainError> {
    let dto: BubbleDto = crate::envelope::unwrap_envelope(body)?;
    Ok(Bubble {
        base_info: dto.base_info.map(|b| {
            let t = b.tribee_info.unwrap_or_default();
            BubbleBaseInfo {
                is_joined: b.is_joined,
                id: t.id,
                title: t.title,
                sub_title: t.sub_title,
                face_url: t.face_url,
                jump_uri: t.jump_uri,
                summary: t.summary,
            }
        }),
        content: dto.content.map(|c| BubbleContent {
            count: c.count,
            items: c
                .dyn_list
                .unwrap_or_default()
                .into_iter()
                .map(|d| {
                    let m = d.meta.unwrap_or_default();
                    BubbleDyn {
                        dyn_id: d.dyn_id,
                        title: d.title,
                        author: m.author,
                        time_text: m.time_text,
                        reply_count: m.reply_count,
                        view_stat: m.view_stat,
                    }
                })
                .collect(),
        }),
        categories: dto
            .category
            .unwrap_or_default()
            .category_list
            .unwrap_or_default()
            .into_iter()
            .map(|c| BubbleCategory {
                id: c.id,
                name: c.name,
                kind: c.kind,
            })
            .collect(),
        sort_info: dto.sort_info.map(|s| BubbleSortInfo {
            show_sort: s.show_sort,
            cur_sort_type: s.cur_sort_type,
            items: s
                .sort_items
                .unwrap_or_default()
                .into_iter()
                .map(|i| BubbleSortItem {
                    sort_type: i.sort_type,
                    text: i.text,
                })
                .collect(),
        }),
    })
}

// -------------------------------------------------------------- 取数

/// 取专栏计数。
///
/// # Errors
/// 传输或信封失败沿用统一分类。
pub async fn fetch_article_info<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    cv_id: &str,
    mixin_key: &str,
    wts: i64,
) -> Result<ArticleInfo, DomainError> {
    let req = article_info_request(cv_id, mixin_key, wts);
    let body = crate::client::send(transport, ctx, &req).await?;
    parse_article_info(&body)
}

/// 取文集。
///
/// # Errors
/// 同 [`fetch_article_info`]。
pub async fn fetch_article_list<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    id: &str,
) -> Result<ArticleList, DomainError> {
    let body = crate::client::send(transport, ctx, &article_list_request(id)).await?;
    parse_article_list(&body)
}

/// 取话题页顶部。
///
/// # Errors
/// 同 [`fetch_article_info`]。
pub async fn fetch_topic_top<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    topic_id: &str,
    access_key: Option<&str>,
    appkey: &str,
    appsec: &str,
    ts: i64,
) -> Result<TopicTop, DomainError> {
    let body = crate::client::send_app(
        transport,
        ctx,
        &topic_top_request(topic_id, access_key, appkey, appsec, ts),
    )
    .await?;
    parse_topic_top(&body)
}

/// 取推荐话题。
///
/// # Errors
/// 同 [`fetch_article_info`]。
pub async fn fetch_topic_rcmd<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    page_size: i64,
) -> Result<Vec<TopicItem>, DomainError> {
    let body = crate::client::send(transport, ctx, &topic_rcmd_request(page_size)).await?;
    parse_topic_rcmd(&body)
}

/// 取部落气泡。
///
/// # Errors
/// 同 [`fetch_article_info`]。
pub async fn fetch_bubble<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    tribe_id: &str,
    category_id: Option<&str>,
    sort_type: Option<i64>,
    page: i64,
) -> Result<Bubble, DomainError> {
    let req = bubble_request(tribe_id, category_id, sort_type, page);
    let body = crate::client::send(transport, ctx, &req).await?;
    parse_bubble(&body)
}

/// 发布页的话题搜索。也挂在 **app** 网关上。
///
/// 这一条本属搜索那一族（`lib/http/search.dart`），**刻意拉进本批**：
/// 它是 `TopicItem` 的第二个生产者，留着它就得为话题多留一层 Dart 模型
/// 加一层单向适配（ADR-011 补充十二）。多迁这一条，`topic_item.dart`
/// 就能整个删掉。
///
/// 翻页形状与别处不同：第一页给 `page_size` + `page_num`，之后给 `offset`，
/// 两者**不能同时出现**。这是基线的写法，照搬。
#[must_use]
pub fn topic_pub_search_request(
    keywords: &str,
    content: &str,
    page_num: i64,
    access_key: Option<&str>,
    appkey: &str,
    appsec: &str,
    ts: i64,
) -> PreparedRequest {
    let mut query = vec![
        ("keywords".to_owned(), keywords.to_owned()),
        ("content".to_owned(), content.to_owned()),
    ];
    if page_num <= 1 {
        query.push(("page_size".to_owned(), "20".to_owned()));
        query.push(("page_num".to_owned(), "1".to_owned()));
    } else {
        query.push(("offset".to_owned(), (20 * (page_num - 1)).to_string()));
    }
    query.push(("web_location".to_owned(), "333.1365".to_owned()));
    PreparedRequest {
        path: topic_path::TOPIC_PUB_SEARCH,
        query: crate::request::sign_app_query(query, access_key, appkey, appsec, ts),
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct TopicPubSearchDto {
    topic_items: Option<Vec<TopicItemDto>>,
    page_info: Option<PageInfoDto>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct PageInfoDto {
    has_more: Option<bool>,
}

/// 解析发布页的话题搜索。
///
/// # Errors
/// 信封失败沿用统一分类。
pub fn parse_topic_pub_search(body: &str) -> Result<TopicPubSearch, DomainError> {
    let dto: Option<TopicPubSearchDto> = crate::envelope::unwrap_envelope_nullable(body)?;
    let dto = dto.unwrap_or_default();
    Ok(TopicPubSearch {
        items: dto
            .topic_items
            .unwrap_or_default()
            .into_iter()
            .filter_map(topic_item)
            .collect(),
        has_more: dto.page_info.and_then(|p| p.has_more),
    })
}

/// 取发布页的话题搜索结果。
///
/// # Errors
/// 同 [`fetch_article_info`]。
#[allow(clippy::too_many_arguments)]
pub async fn fetch_topic_pub_search<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    keywords: &str,
    content: &str,
    page_num: i64,
    access_key: Option<&str>,
    appkey: &str,
    appsec: &str,
    ts: i64,
) -> Result<TopicPubSearch, DomainError> {
    let req = topic_pub_search_request(keywords, content, page_num, access_key, appkey, appsec, ts);
    let body = crate::client::send_app(transport, ctx, &req).await?;
    parse_topic_pub_search(&body)
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod batch1_tests {
    use super::{
        article_info_request, article_list_request, article_path, bubble_path, bubble_request,
        parse_article_info, parse_topic_rcmd, parse_topic_top, topic_path,
        topic_pub_search_request, topic_rcmd_request, topic_top_request,
    };
    use bili_domain::error::ErrorKind;

    fn query(req: &crate::request::PreparedRequest) -> String {
        req.query
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("&")
    }

    /// 协议变更记录引用的就是这一条：三个固定参数**一起进签名**。
    ///
    /// 漏掉任何一个服务端都回 `-403`，而那个码看起来像没登录——最容易
    /// 被当成「要加 Cookie」而查错方向。
    #[test]
    fn article_info_request_signs_the_three_fixed_params() {
        let req = article_info_request("cv31000001", "mixin", 1_700_000_000);
        assert_eq!(req.path, article_path::ARTICLE_INFO);

        let keys: Vec<&str> = req.query.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(
            keys,
            vec!["from", "gaia_source", "id", "mobi_app", "wts", "w_rid"],
            "签名覆盖的是排序后的串，顺序是签名的一部分；w_rid 永远在最后"
        );

        // 少一个固定参数就是另一个签名——钉住它，免得有人「清理」掉。
        let mut without = vec![
            ("id".to_owned(), "cv31000001".to_owned()),
            ("mobi_app".to_owned(), "pc".to_owned()),
            ("from".to_owned(), "web".to_owned()),
            ("wts".to_owned(), "1700000000".to_owned()),
        ];
        without.sort_by(|a, b| a.0.cmp(&b.0));
        let other = bili_protocol::wbi::sign(&without, "mixin");
        let signed = req
            .query
            .iter()
            .find(|(k, _)| k == "w_rid")
            .expect("有 w_rid");
        assert_ne!(signed.1, other, "gaia_source 必须进签名");
    }

    #[test]
    fn article_list_and_topic_requests_keep_the_baseline_constants() {
        assert_eq!(
            query(&article_list_request("rl540001")),
            "id=rl540001&web_location=333.1400"
        );
        let top = query(&topic_top_request("10001", None, "key", "sec", 1));
        assert!(top.contains("topic_id=10001") && top.contains("source=Web"));
        assert_eq!(
            query(&topic_rcmd_request(25)),
            "source=Web&page_size=25&web_location=333.1365"
        );
        assert_eq!(
            topic_top_request(
                "1",
                None,
                "dfca71928277209b",
                "b5475a8825547a4fc26c7d518eaaa02e",
                1_700_000_000
            )
            .path,
            topic_path::TOPIC_TOP
        );
    }

    /// 翻页形状：第一页给 `page_size` + `page_num`，之后给 `offset`，
    /// 两者**不能同时出现**。
    #[test]
    fn topic_pub_search_switches_from_page_num_to_offset() {
        let first = query(&topic_pub_search_request(
            "关键词",
            "",
            1,
            None,
            "dfca71928277209b",
            "b5475a8825547a4fc26c7d518eaaa02e",
            1_700_000_000,
        ));
        assert!(first.contains("page_size=20") && first.contains("page_num=1"));
        assert!(!first.contains("offset="), "第一页不带 offset");

        let third = query(&topic_pub_search_request(
            "关键词",
            "",
            3,
            None,
            "dfca71928277209b",
            "b5475a8825547a4fc26c7d518eaaa02e",
            1_700_000_000,
        ));
        assert!(third.contains("offset=40"), "第三页从第 40 条开始");
        assert!(
            !third.contains("page_num=") && !third.contains("page_size="),
            "翻页之后不再带页号"
        );
    }

    /// 可省参数用「整条不发」表达，不是传空串：传 `category_id=` 服务端
    /// 当成「分类 id 为空串」，回一个空列表。
    #[test]
    fn bubble_omits_absent_params_instead_of_sending_blanks() {
        let all = query(&bubble_request("7001", None, None, 1));
        assert!(!all.contains("category_id"), "没有分类就整条不发");
        assert!(!all.contains("sort_type"), "没有排序就整条不发");
        assert!(all.starts_with("tribee_id=7001"), "协议里是 tribee_id");
        assert!(all.contains("page_size=20&page_num=1"));
        assert!(
            all.contains("x-bili-device-req-json="),
            "这一项在查询串里，不在请求头"
        );
        assert_eq!(bubble_request("1", None, None, 1).path, bubble_path::BUBBLE);

        let narrowed = query(&bubble_request("7001", Some("11"), Some(1), 2));
        assert!(narrowed.contains("category_id=11") && narrowed.contains("sort_type=1"));
        assert!(narrowed.contains("page_num=2"));
    }

    /// `top_details` 缺失不是错误：基线在这里给 `null`，界面据此不渲染顶部。
    #[test]
    fn a_topic_without_a_header_is_not_an_error() {
        let top = parse_topic_top(r#"{"code":0,"message":"0","data":{}}"#).expect("不是错误");
        assert!(top.topic.is_none() && top.creator.is_none());
    }

    /// 缺 id 或 name 的话题整条丢掉，而不是补一个 0 号话题——那会在界面上
    /// 留下一个点不动的条目。
    #[test]
    fn a_topic_without_an_id_or_a_name_is_dropped() {
        let items = parse_topic_rcmd(
            r#"{"code":0,"message":"0","data":{"topic_items":[
                 {"id":1,"name":"留下"},{"name":"没有 id"},{"id":2},{"id":3,"name":"也留下"}]}}"#,
        )
        .expect("应解析成功");
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].id, 1);
        assert_eq!(items[1].id, 3);
        assert_eq!(items[0].view, 0, "计数缺失落 0，与基线的 ?? 0 一致");
    }

    /// 标识符给成字符串时**整条失败**，这是刻意的（#166 的判据）。
    #[test]
    fn a_string_topic_id_fails_the_whole_response() {
        let err = parse_topic_rcmd(
            r#"{"code":0,"message":"0","data":{"topic_items":[{"id":"1","name":"x"}]}}"#,
        )
        .expect_err("标识符不放宽");
        assert_eq!(err.kind(), ErrorKind::Protocol);
    }

    /// 展示用的计数放宽，不会因为服务端换个写法就整条作废。
    #[test]
    fn article_stats_accept_the_string_form() {
        let info = parse_article_info(
            r#"{"code":0,"message":"0","data":{"stats":{"like":"901","reply":23,"share":"--"}}}"#,
        )
        .expect("计数的写法不该让整条响应作废");
        let stats = info.stats.expect("有 stats");
        assert_eq!(stats.like, Some(901));
        assert_eq!(stats.reply, Some(23));
        assert_eq!(stats.share, None, "解不出来当没有");
    }
}
