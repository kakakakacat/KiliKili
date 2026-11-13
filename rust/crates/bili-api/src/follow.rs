//! 关注与粉丝（S15 / issue #66）。
//!
//! 对应基线 `lib/http/follow.dart` 与 `lib/http/fan.dart`：
//!
//! - 只读的分页列表 `FollowHttp.followings` / `FanHttp.fans`——两条共用同一
//!   个响应形状（`bili_domain::user_list::FollowList`），**一起迁**；
//! - 写操作 `FollowHttp.sortFollowTag`（关注分组排序，写语义见 ADR-013）。
//!
//! ## 分组排序这条接口特别在哪
//!
//! 它是目前唯一一条**参数同时分布在查询串与表单里**的写操作：
//! `x-bili-device-req-json` 在 URL 上，`tagids` 在体里。`PreparedWrite` 的
//! `query` 字段就是为它加的——「写操作的参数都在体里」只是惯例，不是协议
//! 保证。

use bili_domain::error::DomainError;
use bili_domain::user_list::{FollowList, FollowUser, OfficialVerify};
use serde::Deserialize;

use crate::http::{HttpTransport, Idempotency, RequestContext};
use crate::request::{PreparedRequest, PreparedWrite};

/// 接口路径。与基线 `lib/http/api.dart` 保持一致。
pub mod path {
    pub const FOLLOWINGS: &str = "/x/relation/followings";
    pub const FANS: &str = "/x/relation/fans";
    pub const SORT_FOLLOW_TAG: &str = "/x/relation/tags/update_sort";
}

// ------------------------------------------------------------ 只读：分页列表

/// 关注列表一页的请求。
///
/// `order=desc` 是基线写死的；`order_type` 为空串表示「最近关注」，
/// `attention` 表示「最常访问」。**空串也要发出去**——基线
/// `FollowHttp.followings` 的默认值就是空串且无条件带上，服务端对「有这个
/// 参数但为空」和「没这个参数」未必一视同仁，而那不是本切片要验证的事。
#[must_use]
pub fn followings_request(
    vmid: i64,
    page: i64,
    page_size: i64,
    order_type: &str,
) -> PreparedRequest {
    PreparedRequest {
        path: path::FOLLOWINGS,
        query: vec![
            ("vmid".to_owned(), vmid.to_string()),
            ("pn".to_owned(), page.to_string()),
            ("ps".to_owned(), page_size.to_string()),
            ("order".to_owned(), "desc".to_owned()),
            ("order_type".to_owned(), order_type.to_owned()),
        ],
    }
}

/// 粉丝列表一页的请求。
///
/// 与 [`followings_request`] 的唯一区别是 `order_type` **可以整条省掉**：
/// 基线 `FanHttp.fans` 的参数是 `String?`，Dio 会把值为 null 的键丢掉。
/// 保留这个区别而不是统一成空串，是因为统一等于偷偷改了其中一条的排序，
/// 而本切片不验证排序。
#[must_use]
pub fn fans_request(
    vmid: i64,
    page: i64,
    page_size: i64,
    order_type: Option<&str>,
) -> PreparedRequest {
    let mut query = vec![
        ("vmid".to_owned(), vmid.to_string()),
        ("pn".to_owned(), page.to_string()),
        ("ps".to_owned(), page_size.to_string()),
        ("order".to_owned(), "desc".to_owned()),
    ];
    if let Some(order_type) = order_type {
        query.push(("order_type".to_owned(), order_type.to_owned()));
    }
    PreparedRequest {
        path: path::FANS,
        query,
    }
}

/// 取一页关注列表。
///
/// # Errors
/// 传输、状态码、信封三类失败，见各自的文档。
pub async fn fetch_followings<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    vmid: i64,
    page: i64,
    page_size: i64,
    order_type: &str,
) -> Result<FollowList, DomainError> {
    let req = followings_request(vmid, page, page_size, order_type);
    let body = crate::client::send(transport, ctx, &req).await?;
    parse_follow_list(&body)
}

/// 取一页粉丝列表。
///
/// # Errors
/// 同 [`fetch_followings`]。
pub async fn fetch_fans<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    vmid: i64,
    page: i64,
    page_size: i64,
    order_type: Option<&str>,
) -> Result<FollowList, DomainError> {
    let req = fans_request(vmid, page, page_size, order_type);
    let body = crate::client::send(transport, ctx, &req).await?;
    parse_follow_list(&body)
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct FollowListDto {
    list: Option<Vec<FollowUserDto>>,
    total: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct FollowUserDto {
    mid: Option<i64>,
    attribute: Option<i64>,
    uname: Option<String>,
    face: Option<String>,
    sign: Option<String>,
    official_verify: Option<OfficialVerifyDto>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct OfficialVerifyDto {
    #[serde(rename = "type")]
    kind: Option<i64>,
    desc: Option<String>,
}

/// 解析一页关注/粉丝。
///
/// # Errors
/// 信封失败沿用统一分类。`list` 缺失按空列表处理：「这一页没有数据」与
/// 「列表为空」在基线里都是 `list == null`，不是错误。
///
/// `sign` **刻意不做空串归一**（与本仓库别处的归一相反）：基线
/// `FollowItemModel` 原样存，界面 `FollowItem` 判的是 `sign != null`。归一
/// 成 `None` 会把「签名是空串」的人从「多一行空白」变成「少一行」，是可见
/// 的行为变化。
pub fn parse_follow_list(body: &str) -> Result<FollowList, DomainError> {
    let dto: FollowListDto = crate::envelope::unwrap_envelope(body)?;
    Ok(FollowList {
        list: dto
            .list
            .unwrap_or_default()
            .into_iter()
            .map(|i| FollowUser {
                mid: i.mid.unwrap_or(0),
                attribute: i.attribute,
                uname: i.uname,
                face: i.face,
                sign: i.sign,
                official_verify: i.official_verify.map(|v| OfficialVerify {
                    kind: v.kind,
                    desc: v.desc,
                }),
            })
            .collect(),
        total: dto.total,
    })
}

// ------------------------------------------------------------ 写：分组排序

/// 基线原样发送的设备标识串。
///
/// 不是可省的装饰：缺了它服务端会按另一套风控规则处理。内容是固定常量，
/// 与基线 `lib/http/follow.dart` 一字不差。
const DEVICE_REQ_JSON: &str = r#"{"platform":"web","device":"pc","spmid":"333.1387"}"#;

/// 关注分组排序的写请求。
///
/// `tagids` 是逗号分隔的分组 id，顺序即目标顺序。
///
/// 定级 [`Idempotency::Idempotent`]：整串顺序是**置为**语义，重发落到同一
/// 个顺序。
#[must_use]
pub fn sort_follow_tag_request(tagids: &str) -> PreparedWrite {
    PreparedWrite {
        path: path::SORT_FOLLOW_TAG,
        query: vec![(
            "x-bili-device-req-json".to_owned(),
            DEVICE_REQ_JSON.to_owned(),
        )],
        form: vec![("tagids".to_owned(), tagids.to_owned())],
        idempotency: Idempotency::Idempotent,
    }
}

/// 提交关注分组的新顺序。
///
/// # Errors
/// 传输、状态码、信封三类失败。
pub async fn sort_follow_tag<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    tagids: &str,
) -> Result<(), DomainError> {
    let body = crate::client::send_write(transport, ctx, &sort_follow_tag_request(tagids)).await?;
    crate::envelope::check_envelope(&body)
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::{
        DEVICE_REQ_JSON, fans_request, followings_request, parse_follow_list, path,
        sort_follow_tag_request,
    };
    use crate::http::Idempotency;

    #[test]
    fn followings_request_always_carries_order_type() {
        let q = followings_request(1, 2, 20, "").query_string();
        assert!(
            q.contains("vmid=1") && q.contains("pn=2") && q.contains("ps=20"),
            "{q}"
        );
        assert!(q.contains("order=desc"), "{q}");
        assert!(
            q.contains("order_type="),
            "空串也要发：基线无条件带上这个键 — {q}"
        );
    }

    #[test]
    fn fans_request_drops_order_type_when_absent() {
        let with = fans_request(1, 1, 20, Some("attention")).query_string();
        assert!(with.contains("order_type=attention"), "{with}");

        let without = fans_request(1, 1, 20, None).query_string();
        assert!(
            !without.contains("order_type"),
            "基线的 Dio 会把 null 的键整条丢掉 — {without}"
        );
    }

    #[test]
    fn empty_list_is_not_an_error() {
        let list = parse_follow_list(r#"{"code":0,"message":"0","data":{"total":0}}"#)
            .expect("空列表不是错误");
        assert!(list.list.is_empty());
        assert_eq!(list.total, Some(0));
    }

    #[test]
    fn missing_mid_falls_back_to_zero_and_blank_sign_is_kept() {
        let list = parse_follow_list(
            r#"{"code":0,"message":"0","data":{"list":[{"uname":"甲","sign":""}]}}"#,
        )
        .expect("应解析成功");
        assert_eq!(list.list[0].mid, 0, "基线 FollowItemModel 缺 mid 时填 0");
        assert_eq!(
            list.list[0].sign.as_deref(),
            Some(""),
            "空签名不归一成 None，否则界面少一行"
        );
    }

    #[test]
    fn official_verify_type_is_renamed_to_kind() {
        let list = parse_follow_list(
            r#"{"code":0,"message":"0","data":{"list":[{"mid":1,"official_verify":{"type":0,"desc":"某某"}}]}}"#,
        )
        .expect("应解析成功");
        let v = list.list[0].official_verify.as_ref().expect("有认证");
        assert_eq!(v.kind, Some(0));
        assert_eq!(v.desc.as_deref(), Some("某某"));
    }

    #[test]
    fn sort_request_shape_is_pinned() {
        let req = sort_follow_tag_request("3,1,2");
        assert_eq!(req.path, path::SORT_FOLLOW_TAG);
        assert_eq!(req.form_string(), "tagids=3,1,2");
        assert_eq!(
            req.query,
            vec![(
                "x-bili-device-req-json".to_owned(),
                DEVICE_REQ_JSON.to_owned()
            )],
            "设备标识串在查询串上，不在表单里——这条是唯一的例外，别顺手挪进体里"
        );
        assert_eq!(
            req.idempotency,
            Idempotency::Idempotent,
            "整串顺序是置为语义"
        );
    }
}
