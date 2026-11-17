//! 关注分组与关注关系（S6 #56 / S7 #57）。
//!
//! 对应基线 `lib/http/member.dart` 与 `lib/http/user.dart` 里围绕
//! `/x/relation/*` 的那一族接口：
//!
//! - 只读：分组清单、某个分组下的人、关注搜索、「他关注的 up」、
//!   「你们共同关注」、以及「我和这个人的关系」；
//! - 写：把人加进分组、建组/改名/删组、特别关注的加与取消。
//!
//! ## 为什么这一族要一次迁完
//!
//! 响应形状 `FollowList` 有**六个**生产者，分散在三个切片（见 issue #57
//! 的表）。S15（#66）已经迁走 `followings` / `fans` 两个，剩下四个全在
//! 这里。基线的 `FollowData` / `FollowItemModel` 只有等这四个也迁完才能
//! 删——中途每单独迁一个，都要多留一层只为别人而活的适配
//! （`lib/services/rust/follow_compat.dart` 就是那一层）。
//!
//! ## 分组列表的响应形状是个例外
//!
//! `/x/relation/tag` 的 `data` **直接就是用户数组**，没有 `{list, total}`
//! 那层包装，也因此拿不到总数。解析走
//! [`crate::follow::parse_follow_user_array`]，不要试图把它掰成另外几条
//! 的形状。

use bili_domain::error::DomainError;
use bili_domain::user_list::{FollowList, FollowTag, Relation};
use serde::Deserialize;

use crate::http::{HttpTransport, Idempotency, RequestContext};
use crate::request::{CsrfIn, PreparedRequest, PreparedWrite, WriteBody};

/// 接口路径。与基线 `lib/http/api.dart` 保持一致。
pub mod path {
    pub const FOLLOW_UP_TAGS: &str = "/x/relation/tags";
    pub const FOLLOW_UP_GROUP: &str = "/x/relation/tag";
    pub const FOLLOW_SEARCH: &str = "/x/relation/followings/search";
    pub const FOLLOWED_UP: &str = "/x/relation/followings/followed_upper";
    pub const SAME_FOLLOWING: &str = "/x/relation/same/followings";
    pub const RELATION: &str = "/x/relation";

    pub const ADD_USERS: &str = "/x/relation/tags/addUsers";
    pub const CREATE_FOLLOW_TAG: &str = "/x/relation/tag/create";
    pub const UPDATE_FOLLOW_TAG: &str = "/x/relation/tag/update";
    pub const DEL_FOLLOW_TAG: &str = "/x/relation/tag/del";
    pub const ADD_SPECIAL: &str = "/x/relation/tag/special/add";
    pub const DEL_SPECIAL: &str = "/x/relation/tag/special/del";
}

/// 基线原样发送的设备标识串，分组相关接口用的是 `333.1387`。
///
/// 与 [`crate::follow`] 里那份同形不同值：那边是关注分组排序，spmid 一样，
/// 但「他关注的 up」那两条用的是 `333.789`（见 [`DEVICE_REQ_JSON_789`]）。
/// 不合并成一个常量——spmid 是页面来源标记，合并等于谎报来源。
const DEVICE_REQ_JSON: &str = r#"{"platform":"web","device":"pc","spmid":"333.1387"}"#;

/// 「他关注的 up」与「共同关注」用的设备标识串，spmid 是 `333.789`。
const DEVICE_REQ_JSON_789: &str = r#"{"platform":"web","device":"pc","spmid":"333.789"}"#;

// ---------------------------------------------------------------- 只读：分组

/// 分组清单的请求。无参数。
#[must_use]
pub fn follow_up_tags_request() -> PreparedRequest {
    PreparedRequest {
        path: path::FOLLOW_UP_TAGS,
        query: Vec::new(),
    }
}

/// 取关注分组清单。
///
/// # Errors
/// 传输、状态码、信封三类失败，见各自的文档。
pub async fn fetch_follow_up_tags<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
) -> Result<Vec<FollowTag>, DomainError> {
    let body = crate::client::send(transport, ctx, &follow_up_tags_request()).await?;
    parse_follow_up_tags(&body)
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct FollowTagDto {
    tagid: Option<i64>,
    name: Option<String>,
    count: Option<i64>,
    tip: Option<String>,
}

/// 解析分组清单。
///
/// # Errors
/// 信封失败沿用统一分类。`data` 为 `null` 按空列表处理——基线
/// `MemberHttp.followUpTags` 把 `data` 直接当 `List` 用，服务端给 null 时
/// 它会抛；这里不复刻那个崩法，空清单不是错误。
pub fn parse_follow_up_tags(body: &str) -> Result<Vec<FollowTag>, DomainError> {
    let dto: Option<Vec<FollowTagDto>> = crate::envelope::unwrap_envelope_nullable(body)?;
    Ok(dto
        .unwrap_or_default()
        .into_iter()
        .map(|t| FollowTag {
            tagid: t.tagid,
            name: t.name,
            count: t.count,
            tip: t.tip,
        })
        .collect())
}

/// 某个分组下的人，一页。
///
/// 四个参数在基线里都是可空的，`null` 由 Dio 整条丢掉，这里照搬：
/// `mid` 省掉表示看自己的分组，`tagid` 省掉表示「全部关注」那一栏。
/// `order_type` 则**总是**发出去（空串＝最近关注，`attention`＝最常访问），
/// 与基线默认值一致。
#[must_use]
pub fn follow_up_group_request(
    mid: Option<i64>,
    tagid: Option<i64>,
    page: Option<i64>,
    page_size: i64,
    order_type: &str,
) -> PreparedRequest {
    let mut query = Vec::with_capacity(5);
    if let Some(mid) = mid {
        query.push(("mid".to_owned(), mid.to_string()));
    }
    if let Some(tagid) = tagid {
        query.push(("tagid".to_owned(), tagid.to_string()));
    }
    if let Some(page) = page {
        query.push(("pn".to_owned(), page.to_string()));
    }
    query.push(("ps".to_owned(), page_size.to_string()));
    query.push(("order_type".to_owned(), order_type.to_owned()));
    PreparedRequest {
        path: path::FOLLOW_UP_GROUP,
        query,
    }
}

/// 取某个分组下的一页人。
///
/// # Errors
/// 同 [`fetch_follow_up_tags`]。注意结果的 `total` 恒为 `None`——这条接口
/// 不给总数，见模块文档。
pub async fn fetch_follow_up_group<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    mid: Option<i64>,
    tagid: Option<i64>,
    page: Option<i64>,
    page_size: i64,
    order_type: &str,
) -> Result<FollowList, DomainError> {
    let req = follow_up_group_request(mid, tagid, page, page_size, order_type);
    let body = crate::client::send(transport, ctx, &req).await?;
    crate::follow::parse_follow_user_array(&body)
}

/// 在自己的关注里按昵称搜索，一页。WBI 签名。
///
/// `order=desc` / `order_type=attention` / `gaia_source=main_web` /
/// `web_location=333.999` 都是基线写死的固定值。`web_location` 在基线是个
/// Dart `double`，序列化成 `333.999`；这里直接写成字符串，是为了让签名覆盖
/// 的字节与基线一致——换成 `333.999_f64` 再格式化，小数位数由格式化器决定，
/// 签名随时可能对不上。
#[must_use]
pub fn follow_search_request(
    vmid: i64,
    page: i64,
    page_size: i64,
    name: &str,
    mixin_key: &str,
    wts: i64,
) -> PreparedRequest {
    let mut query = vec![
        ("vmid".to_owned(), vmid.to_string()),
        ("pn".to_owned(), page.to_string()),
        ("ps".to_owned(), page_size.to_string()),
        ("order".to_owned(), "desc".to_owned()),
        ("order_type".to_owned(), "attention".to_owned()),
        ("gaia_source".to_owned(), "main_web".to_owned()),
        ("name".to_owned(), name.to_owned()),
        ("web_location".to_owned(), "333.999".to_owned()),
        ("wts".to_owned(), wts.to_string()),
    ];
    query.sort_by(|a, b| a.0.cmp(&b.0));
    let w_rid = bili_protocol::wbi::sign(&query, mixin_key);
    query.push(("w_rid".to_owned(), w_rid));
    PreparedRequest {
        path: path::FOLLOW_SEARCH,
        query,
    }
}

/// 搜索自己的关注。
///
/// # Errors
/// 同 [`fetch_follow_up_tags`]。
// 八个参数：传输与上下文两个是本 crate 所有取数函数的固定前缀，剩下六个
// 全是接口自己的入参。为此造一个只用一次的参数结构，只是把同样的六个值
// 换个地方写。
#[allow(clippy::too_many_arguments)]
pub async fn fetch_follow_search<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    vmid: i64,
    page: i64,
    page_size: i64,
    name: &str,
    mixin_key: &str,
    wts: i64,
) -> Result<FollowList, DomainError> {
    let req = follow_search_request(vmid, page, page_size, name, mixin_key, wts);
    let body = crate::client::send(transport, ctx, &req).await?;
    crate::follow::parse_follow_list(&body)
}

/// 「他关注的 up 主」一页。
///
/// 只读却要 csrf，和黑名单一样是服务端的要求；凭据的真相源在 Dart 的
/// 账号体系，Rust 不存。
#[must_use]
pub fn followed_up_request(vmid: i64, page: i64, csrf: &str) -> PreparedRequest {
    PreparedRequest {
        path: path::FOLLOWED_UP,
        query: vec![
            ("csrf".to_owned(), csrf.to_owned()),
            ("pn".to_owned(), page.to_string()),
            ("vmid".to_owned(), vmid.to_string()),
            ("web_location".to_owned(), "333.789".to_owned()),
            (
                "x-bili-device-req-json".to_owned(),
                DEVICE_REQ_JSON_789.to_owned(),
            ),
        ],
    }
}

/// 「你们共同关注」一页。
///
/// 与 [`followed_up_request`] 只差路径与「`pn` 可以整条省掉」：基线
/// `UserHttp.sameFollowing` 的 `pn` 是可空的，不传时服务端给第一页。
#[must_use]
pub fn same_following_request(vmid: i64, page: Option<i64>, csrf: &str) -> PreparedRequest {
    let mut query = vec![("csrf".to_owned(), csrf.to_owned())];
    if let Some(page) = page {
        query.push(("pn".to_owned(), page.to_string()));
    }
    query.push(("vmid".to_owned(), vmid.to_string()));
    query.push(("web_location".to_owned(), "333.789".to_owned()));
    query.push((
        "x-bili-device-req-json".to_owned(),
        DEVICE_REQ_JSON_789.to_owned(),
    ));
    PreparedRequest {
        path: path::SAME_FOLLOWING,
        query,
    }
}

/// 取「他关注的 up 主」一页。
///
/// # Errors
/// 同 [`fetch_follow_up_tags`]。
pub async fn fetch_followed_up<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    vmid: i64,
    page: i64,
    csrf: &str,
) -> Result<FollowList, DomainError> {
    let body = crate::client::send(transport, ctx, &followed_up_request(vmid, page, csrf)).await?;
    crate::follow::parse_follow_list(&body)
}

/// 取「共同关注」一页。
///
/// # Errors
/// 同 [`fetch_follow_up_tags`]。
pub async fn fetch_same_following<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    vmid: i64,
    page: Option<i64>,
    csrf: &str,
) -> Result<FollowList, DomainError> {
    let req = same_following_request(vmid, page, csrf);
    let body = crate::client::send(transport, ctx, &req).await?;
    crate::follow::parse_follow_list(&body)
}

// ---------------------------------------------------------------- 只读：关系

/// 「我和这个人的关系」的请求。
#[must_use]
pub fn relation_request(fid: i64) -> PreparedRequest {
    PreparedRequest {
        path: path::RELATION,
        query: vec![("fid".to_owned(), fid.to_string())],
    }
}

/// 取与某个人的关系。
///
/// # Errors
/// 同 [`fetch_follow_up_tags`]。
pub async fn fetch_relation<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    fid: i64,
) -> Result<Relation, DomainError> {
    let body = crate::client::send(transport, ctx, &relation_request(fid)).await?;
    parse_relation(&body)
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RelationDto {
    attribute: Option<i64>,
    mtime: Option<i64>,
    tag: Option<Vec<i64>>,
    special: Option<i64>,
}

/// 解析关系。
///
/// # Errors
/// 信封失败沿用统一分类。`tag` 的 `None` 与 `[]` **不归一**，理由见
/// [`Relation::tag`](bili_domain::user_list::Relation::tag)。
pub fn parse_relation(body: &str) -> Result<Relation, DomainError> {
    let dto: RelationDto = crate::envelope::unwrap_envelope(body)?;
    Ok(Relation {
        attribute: dto.attribute,
        mtime: dto.mtime,
        tag: dto.tag,
        special: dto.special,
    })
}

// -------------------------------------------------------------------- 写操作

/// 把若干人设置到若干分组。
///
/// `fids` 与 `tagids` 都是逗号分隔的 id 串。定级 [`Idempotency::Idempotent`]：
/// 服务端语义是「这些人的分组**置为**这一组」，不是「追加一个分组」，
/// 重发落到同一个状态。
#[must_use]
pub fn add_users_request(fids: &str, tagids: &str) -> PreparedWrite {
    PreparedWrite {
        path: path::ADD_USERS,
        query: vec![(
            "x-bili-device-req-json".to_owned(),
            DEVICE_REQ_JSON.to_owned(),
        )],
        body: WriteBody::Form(vec![
            ("fids".to_owned(), fids.to_owned()),
            ("tagids".to_owned(), tagids.to_owned()),
        ]),
        csrf: CsrfIn::Body,
        idempotency: Idempotency::Idempotent,
    }
}

/// 新建一个关注分组。
///
/// 定级 [`Idempotency::Unsafe`]：重发会**多建一个同名分组**，而服务端允许
/// 重名，事后从响应看不出哪一个是这次建的。这与「改名」「删组」不同，
/// 别顺手调成 Idempotent（ADR-013 决策二）。
#[must_use]
pub fn create_follow_tag_request(tag: &str) -> PreparedWrite {
    PreparedWrite {
        path: path::CREATE_FOLLOW_TAG,
        query: vec![(
            "x-bili-device-req-json".to_owned(),
            DEVICE_REQ_JSON.to_owned(),
        )],
        body: WriteBody::Form(vec![("tag".to_owned(), tag.to_owned())]),
        csrf: CsrfIn::Body,
        idempotency: Idempotency::Unsafe,
    }
}

/// 给分组改名。置为语义，重发安全。
#[must_use]
pub fn update_follow_tag_request(tagid: i64, name: &str) -> PreparedWrite {
    PreparedWrite {
        path: path::UPDATE_FOLLOW_TAG,
        query: vec![(
            "x-bili-device-req-json".to_owned(),
            DEVICE_REQ_JSON.to_owned(),
        )],
        body: WriteBody::Form(vec![
            ("tagid".to_owned(), tagid.to_string()),
            ("name".to_owned(), name.to_owned()),
        ]),
        csrf: CsrfIn::Body,
        idempotency: Idempotency::Idempotent,
    }
}

/// 删除一个分组。重发打到同一个 id，结果仍是「它不在了」。
#[must_use]
pub fn del_follow_tag_request(tagid: i64) -> PreparedWrite {
    PreparedWrite {
        path: path::DEL_FOLLOW_TAG,
        query: vec![(
            "x-bili-device-req-json".to_owned(),
            DEVICE_REQ_JSON.to_owned(),
        )],
        body: WriteBody::Form(vec![("tagid".to_owned(), tagid.to_string())]),
        csrf: CsrfIn::Body,
        idempotency: Idempotency::Idempotent,
    }
}

/// 特别关注的加与取消。`add` 为 false 时打的是取消那条路径。
///
/// 基线把两条路径塞在同一个函数里用布尔选，这里保留那个形状——它确实是
/// 同一个开关的两面，拆成两个函数只会让调用点自己写 if。
#[must_use]
pub fn special_action_request(fid: i64, add: bool) -> PreparedWrite {
    PreparedWrite::form_only(
        if add {
            path::ADD_SPECIAL
        } else {
            path::DEL_SPECIAL
        },
        vec![("fid".to_owned(), fid.to_string())],
        Idempotency::Idempotent,
    )
}

/// 提交「设置分组」。
///
/// # Errors
/// 传输、状态码、信封三类失败。
pub async fn add_users<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    fids: &str,
    tagids: &str,
) -> Result<(), DomainError> {
    let body = crate::client::send_write(transport, ctx, &add_users_request(fids, tagids)).await?;
    crate::envelope::check_envelope(&body)
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct CreatedTagDto {
    tagid: Option<i64>,
}

/// 新建分组，返回服务端分配的 `tagid`。
///
/// # Errors
/// 传输、状态码、信封三类失败；成功但没给 `tagid` 也算失败——基线
/// `MemberHttp.createFollowTag` 直接取 `data['tagid']`，拿不到就是空指针，
/// 而调用点拿它去选中新建的那一组，没有 id 就什么也做不了。
pub async fn create_follow_tag<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    tag: &str,
) -> Result<i64, DomainError> {
    let body = crate::client::send_write(transport, ctx, &create_follow_tag_request(tag)).await?;
    let dto: CreatedTagDto = crate::envelope::unwrap_envelope(&body)?;
    dto.tagid.ok_or_else(|| {
        DomainError::new(
            bili_domain::error::ErrorKind::Protocol,
            "新建分组成功但未返回 tagid",
        )
    })
}

/// 给分组改名。
///
/// # Errors
/// 传输、状态码、信封三类失败。
pub async fn update_follow_tag<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    tagid: i64,
    name: &str,
) -> Result<(), DomainError> {
    let body =
        crate::client::send_write(transport, ctx, &update_follow_tag_request(tagid, name)).await?;
    crate::envelope::check_envelope(&body)
}

/// 删除分组。
///
/// # Errors
/// 传输、状态码、信封三类失败。
pub async fn del_follow_tag<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    tagid: i64,
) -> Result<(), DomainError> {
    let body = crate::client::send_write(transport, ctx, &del_follow_tag_request(tagid)).await?;
    crate::envelope::check_envelope(&body)
}

/// 加/取消特别关注。
///
/// # Errors
/// 传输、状态码、信封三类失败。
pub async fn special_action<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    fid: i64,
    add: bool,
) -> Result<(), DomainError> {
    let body = crate::client::send_write(transport, ctx, &special_action_request(fid, add)).await?;
    crate::envelope::check_envelope(&body)
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::{
        DEVICE_REQ_JSON, DEVICE_REQ_JSON_789, add_users_request, create_follow_tag_request,
        del_follow_tag_request, follow_search_request, follow_up_group_request,
        followed_up_request, parse_follow_up_tags, parse_relation, path, same_following_request,
        special_action_request, update_follow_tag_request,
    };
    use crate::http::Idempotency;

    #[test]
    fn group_request_drops_absent_ids_but_always_sends_order_type() {
        let all = follow_up_group_request(None, None, None, 20, "").query_string();
        assert_eq!(
            all, "ps=20&order_type=",
            "三个可空参数缺席时整条丢掉，空的 order_type 仍要发 — {all}"
        );

        let one = follow_up_group_request(Some(1), Some(2), Some(3), 20, "attention");
        let q = one.query_string();
        assert!(
            q.contains("mid=1") && q.contains("tagid=2") && q.contains("pn=3"),
            "{q}"
        );
        assert!(q.contains("order_type=attention"), "{q}");
    }

    #[test]
    fn follow_search_pins_the_baseline_fixed_params() {
        let q = follow_search_request(42, 1, 20, "甲", "key", 1_700_000_000).query_string();
        for expect in [
            "vmid=42",
            "pn=1",
            "ps=20",
            "order=desc",
            "order_type=attention",
            "gaia_source=main_web",
            "name=甲",
            "web_location=333.999",
            "wts=1700000000",
        ] {
            assert!(q.contains(expect), "缺 {expect} — {q}");
        }
        assert!(q.contains("w_rid="), "必须签名 — {q}");
    }

    #[test]
    fn followed_up_and_same_following_use_the_789_spmid() {
        let a = followed_up_request(1, 2, "token").query_string();
        assert!(a.contains("web_location=333.789"), "{a}");
        assert!(a.contains(DEVICE_REQ_JSON_789), "{a}");
        assert!(
            !a.contains(DEVICE_REQ_JSON),
            "别和分组那族的 333.1387 混用 — {a}"
        );
        assert!(a.contains("csrf=token"), "只读却要 csrf — {a}");

        let without_page = same_following_request(1, None, "token").query_string();
        assert!(
            !without_page.contains("pn="),
            "基线的 pn 可空，不传就整条丢掉 — {without_page}"
        );
        assert!(
            same_following_request(1, Some(2), "t")
                .query_string()
                .contains("pn=2")
        );
    }

    #[test]
    fn tag_list_tolerates_null_data() {
        let tags = parse_follow_up_tags(r#"{"code":0,"message":"0","data":null}"#)
            .expect("空分组清单不是错误");
        assert!(tags.is_empty());
    }

    #[test]
    fn relation_keeps_null_tag_distinct_from_empty() {
        let none = parse_relation(r#"{"code":0,"message":"0","data":{"attribute":2}}"#)
            .expect("应解析成功");
        assert_eq!(none.tag, None, "没有分组是 null");

        let default_group =
            parse_relation(r#"{"code":0,"message":"0","data":{"attribute":2,"tag":[0]}}"#)
                .expect("应解析成功");
        assert_eq!(
            default_group.tag,
            Some(vec![0]),
            "默认分组是 [0]，与 null 不是一回事"
        );
    }

    #[test]
    fn write_shapes_are_pinned() {
        let add = add_users_request("1,2", "3");
        assert_eq!(add.path, path::ADD_USERS);
        assert_eq!(add.form_string(), "fids=1,2&tagids=3");
        assert_eq!(
            add.query,
            vec![(
                "x-bili-device-req-json".to_owned(),
                DEVICE_REQ_JSON.to_owned()
            )],
            "设备标识串在查询串上"
        );

        assert_eq!(
            create_follow_tag_request("新分组").idempotency,
            Idempotency::Unsafe,
            "重发会多建一个同名分组"
        );
        assert_eq!(
            update_follow_tag_request(7, "改名").idempotency,
            Idempotency::Idempotent
        );
        assert_eq!(del_follow_tag_request(7).form_string(), "tagid=7");

        assert_eq!(special_action_request(9, true).path, path::ADD_SPECIAL);
        assert_eq!(special_action_request(9, false).path, path::DEL_SPECIAL);
        assert_eq!(special_action_request(9, true).form_string(), "fid=9");
    }
}
