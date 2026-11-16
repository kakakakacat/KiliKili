//! 用户空间那一族的请求构造与解析（S6 / issue #56）。
//!
//! 对应基线 `lib/http/member.dart`。本模块按「看别人」这条线组织：
//! 资料卡、计数、名片、举报。
//!
//! ## 请求头不在这里
//!
//! 这一族里好几条要显式带浏览器 UA 与 `space.bilibili.com` 的
//! `origin`/`referer`（服务端据此判断是不是网页空间页）。[`PreparedRequest`]
//! 只有 path 与 query，没有头——头由 `lib/services/rust/rust_space.dart`
//! 按条补上，同 `rust_search.dart` / `rust_video_feed.dart` 的既有做法。
//!
//! 按 AGENTS.md「例外要写下来」：这是**宿主在拼协议**，本该在 Rust。
//! 给 `PreparedRequest` 加 headers 会动到全部已迁切片的导出面，不在本
//! 切片范围内；**由统一改造请求表示的那个 PR 收口**（见 issue #67）。

use bili_domain::error::DomainError;
use bili_domain::space::{MemberCard, MemberInfo, MemberRelationStat, MemberUpStat};
use bili_protocol::risk::RiskTokens;
use serde::Deserialize;

use crate::http::{HttpTransport, Idempotency, RequestContext};
use crate::request::{PreparedRequest, PreparedWrite};

/// 接口路径。与基线 `lib/http/api.dart` 保持一致。
pub mod path {
    pub const MEMBER_INFO: &str = "/x/space/wbi/acc/info";
    pub const MEMBER_RELATION_STAT: &str = "/x/relation/stat";
    pub const MEMBER_UP_STAT: &str = "/x/space/upstat";
    pub const MEMBER_CARD: &str = "/x/web-interface/card";

    /// 举报。**挂在 `space.bilibili.com` 上**，不是 api 域，base_url 由
    /// 调用方给。
    pub const REPORT_MEMBER: &str = "/ajax/report/add";
}

/// `web_location`：空间页的页面来源标记，服务端据此分流风控策略。
/// 与别处的 `333.1387`（空间内各 tab）不是一回事，不要合并。
const WEB_LOCATION_SPACE: &str = "1550101";

// ------------------------------------------------------------------ 资料卡

/// 别人的资料的请求。WBI 签名。
///
/// `token` 在基线里恒为空串，但**照样发出去**——它参与签名，省掉会换一个
/// 不同的 `w_rid`。
#[must_use]
pub fn member_info_request(
    mid: i64,
    token: &str,
    tokens: &RiskTokens,
    mixin_key: &str,
    wts: i64,
) -> PreparedRequest {
    let mut query = vec![
        ("mid".to_owned(), mid.to_string()),
        ("token".to_owned(), token.to_owned()),
        ("platform".to_owned(), "web".to_owned()),
        ("web_location".to_owned(), WEB_LOCATION_SPACE.to_owned()),
        (
            "dm_img_list".to_owned(),
            bili_protocol::risk::DM_IMG_LIST.to_owned(),
        ),
        ("dm_img_str".to_owned(), tokens.dm_img_str.clone()),
        (
            "dm_cover_img_str".to_owned(),
            tokens.dm_cover_img_str.clone(),
        ),
        (
            "dm_img_inter".to_owned(),
            bili_protocol::risk::DM_IMG_INTER.to_owned(),
        ),
        ("wts".to_owned(), wts.to_string()),
    ];
    query.sort_by(|a, b| a.0.cmp(&b.0));
    let w_rid = bili_protocol::wbi::sign(&query, mixin_key);
    query.push(("w_rid".to_owned(), w_rid));
    PreparedRequest {
        path: path::MEMBER_INFO,
        query,
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct MemberInfoDto {
    mid: Option<i64>,
    name: Option<String>,
    face: Option<String>,
    level: Option<i64>,
    is_followed: Option<bool>,
    vip: Option<VipDto>,
    is_senior_member: Option<i64>,
}

/// 大会员信息。
///
/// 基线的 `Vip.fromJson` 对每个字段都试两个键
/// （`type ?? vipType`、`status ?? vipStatus`）：同一个结构在不同接口下
/// 有两种键名，`Vip` 被所有接口共用，于是两套都认。这里照搬——这条接口
/// 现在给的是哪一套没有人验证过，改成只认一套是在赌。
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct VipDto {
    #[serde(rename = "type")]
    kind: Option<i64>,
    #[serde(rename = "vipType")]
    kind_alt: Option<i64>,
    status: Option<i64>,
    #[serde(rename = "vipStatus")]
    status_alt: Option<i64>,
}

impl VipDto {
    fn kind(&self) -> Option<i64> {
        self.kind.or(self.kind_alt)
    }

    /// 基线的 `status` 是 `late int`，缺了就落到 0（不是 null）。两个显示
    /// 判据都是 `status > 0`，所以 0 与 null 在界面上同义；保留 `Option`
    /// 只会让每个调用点多写一次 `?? 0`。
    fn status(&self) -> i64 {
        self.status.or(self.status_alt).unwrap_or(0)
    }
}

/// 解析别人的资料。
///
/// # Errors
/// 信封失败沿用统一分类。
pub fn parse_member_info(body: &str) -> Result<MemberInfo, DomainError> {
    let dto: MemberInfoDto = crate::envelope::unwrap_envelope(body)?;
    Ok(MemberInfo {
        mid: dto.mid,
        name: dto.name,
        face: dto.face,
        level: dto.level,
        is_followed: dto.is_followed,
        vip_status: dto.vip.as_ref().map(VipDto::status),
        vip_type: dto.vip.as_ref().and_then(VipDto::kind),
        is_senior_member: dto.is_senior_member,
    })
}

/// 取别人的资料。
///
/// # Errors
/// 传输、状态码、信封三类失败，见各自的文档。
pub async fn fetch_member_info<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    mid: i64,
    token: &str,
    tokens: &RiskTokens,
    mixin_key: &str,
    wts: i64,
) -> Result<MemberInfo, DomainError> {
    let req = member_info_request(mid, token, tokens, mixin_key, wts);
    let body = crate::client::send(transport, ctx, &req).await?;
    parse_member_info(&body)
}

// -------------------------------------------------------------------- 计数

/// 关注关系计数的请求。
#[must_use]
pub fn member_relation_stat_request(mid: i64) -> PreparedRequest {
    PreparedRequest {
        path: path::MEMBER_RELATION_STAT,
        query: vec![("vmid".to_owned(), mid.to_string())],
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct MemberRelationStatDto {
    following: Option<i64>,
    follower: Option<i64>,
}

/// 解析关注关系计数。
///
/// # Errors
/// 信封失败沿用统一分类。
pub fn parse_member_relation_stat(body: &str) -> Result<MemberRelationStat, DomainError> {
    let dto: MemberRelationStatDto = crate::envelope::unwrap_envelope(body)?;
    Ok(MemberRelationStat {
        following: dto.following,
        follower: dto.follower,
    })
}

/// 取关注关系计数。
///
/// # Errors
/// 同 [`fetch_member_info`]。
pub async fn fetch_member_relation_stat<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    mid: i64,
) -> Result<MemberRelationStat, DomainError> {
    let req = member_relation_stat_request(mid);
    let body = crate::client::send(transport, ctx, &req).await?;
    parse_member_relation_stat(&body)
}

/// 播放与获赞总数的请求。
#[must_use]
pub fn member_up_stat_request(mid: i64) -> PreparedRequest {
    PreparedRequest {
        path: path::MEMBER_UP_STAT,
        query: vec![("mid".to_owned(), mid.to_string())],
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct MemberUpStatDto {
    archive: Option<ViewDto>,
    article: Option<ViewDto>,
    likes: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ViewDto {
    view: Option<i64>,
}

/// 解析播放与获赞总数。
///
/// # Errors
/// 信封失败沿用统一分类。
pub fn parse_member_up_stat(body: &str) -> Result<MemberUpStat, DomainError> {
    let dto: MemberUpStatDto = crate::envelope::unwrap_envelope(body)?;
    Ok(MemberUpStat {
        archive_view: dto.archive.and_then(|a| a.view),
        article_view: dto.article.and_then(|a| a.view),
        likes: dto.likes,
    })
}

/// 取播放与获赞总数。需要登录。
///
/// # Errors
/// 同 [`fetch_member_info`]。
pub async fn fetch_member_up_stat<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    mid: i64,
) -> Result<MemberUpStat, DomainError> {
    let req = member_up_stat_request(mid);
    let body = crate::client::send(transport, ctx, &req).await?;
    parse_member_up_stat(&body)
}

// -------------------------------------------------------------------- 名片

/// 用户名片的请求。
///
/// `photo=false` 是基线写死的：要的只是昵称与计数，不要空间头图。
#[must_use]
pub fn member_card_request(mid: i64) -> PreparedRequest {
    PreparedRequest {
        path: path::MEMBER_CARD,
        query: vec![
            ("mid".to_owned(), mid.to_string()),
            ("photo".to_owned(), "false".to_owned()),
        ],
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct MemberCardDataDto {
    card: Option<MemberCardDto>,
    archive_count: Option<i64>,
    follower: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct MemberCardDto {
    mid: Option<String>,
    name: Option<String>,
    face: Option<String>,
    /// 这条接口的认证信息键名是**大写开头**的 `Official`。服务端的事实。
    #[serde(rename = "Official")]
    official: Option<OfficialDto>,
    vip: Option<VipDto>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct OfficialDto {
    #[serde(rename = "type")]
    kind: Option<i64>,
}

/// 解析用户名片。
///
/// # Errors
/// 信封失败沿用统一分类。
pub fn parse_member_card(body: &str) -> Result<MemberCard, DomainError> {
    let dto: MemberCardDataDto = crate::envelope::unwrap_envelope(body)?;
    let card = dto.card.unwrap_or_default();
    Ok(MemberCard {
        mid: card.mid,
        name: card.name,
        face: card.face,
        official_type: card.official.and_then(|o| o.kind),
        vip_status: card.vip.as_ref().map(VipDto::status),
        vip_type: card.vip.as_ref().and_then(VipDto::kind),
        archive_count: dto.archive_count,
        follower: dto.follower,
    })
}

/// 取用户名片。
///
/// # Errors
/// 同 [`fetch_member_info`]。
pub async fn fetch_member_card<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    mid: i64,
) -> Result<MemberCard, DomainError> {
    let req = member_card_request(mid);
    let body = crate::client::send(transport, ctx, &req).await?;
    parse_member_card(&body)
}

// -------------------------------------------------------------------- 举报

/// 举报用户。
///
/// 定级 [`Idempotency::Unsafe`]：重发会重复提交举报工单。服务端没有去重
/// 承诺，而「误报一次」对被举报的人是实打实的代价，按 ADR-013 从严。
///
/// `reason_v2` 在基线里是可空的（选了预置理由才有），省掉时整条参数不发。
#[must_use]
pub fn report_member_request(mid: i64, reason: &str, reason_v2: Option<i64>) -> PreparedWrite {
    let mut body = vec![
        ("mid".to_owned(), mid.to_string()),
        ("reason".to_owned(), reason.to_owned()),
    ];
    if let Some(v2) = reason_v2 {
        body.push(("reason_v2".to_owned(), v2.to_string()));
    }
    PreparedWrite::form_only(path::REPORT_MEMBER, body, Idempotency::Unsafe)
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ReportResultDto {
    status: Option<bool>,
}

/// 解析举报结果。
///
/// **这条不是标准信封**：响应顶层就是 `{"status": true}`，没有
/// `code`/`message`/`data`。基线也是直接读 `res.data['status']`。所以不能
/// 走 [`crate::envelope`]——那会把一次成功的举报判成协议错误。
///
/// # Errors
/// 响应不是合法 JSON 时为协议错误。`status` 为 false 或缺失都算失败，
/// 返回 `Ok(false)`：服务端没有给出原因，调用方能做的只是提示失败。
pub fn parse_report_result(body: &str) -> Result<bool, DomainError> {
    let dto: ReportResultDto = serde_json::from_str(body).map_err(|e| {
        DomainError::new(
            bili_domain::error::ErrorKind::Protocol,
            format!("举报响应不是合法 JSON：{e}"),
        )
    })?;
    Ok(dto.status.unwrap_or(false))
}

/// 举报用户。返回服务端是否受理。
///
/// # Errors
/// 传输、状态码两类失败；响应不是 JSON 时为协议错误。
pub async fn report_member<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    mid: i64,
    reason: &str,
    reason_v2: Option<i64>,
) -> Result<bool, DomainError> {
    let req = report_member_request(mid, reason, reason_v2);
    let body = crate::client::send_write(transport, ctx, &req).await?;
    parse_report_result(&body)
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::{
        member_card_request, member_info_request, parse_member_card, parse_member_info,
        parse_member_up_stat, parse_report_result, report_member_request,
    };
    use crate::http::Idempotency;
    use crate::request::{CsrfIn, WriteBody};
    use bili_protocol::risk::RiskTokens;

    fn tokens() -> RiskTokens {
        RiskTokens {
            dm_img_str: "AAAA".to_owned(),
            dm_cover_img_str: "BBBB".to_owned(),
        }
    }

    #[test]
    fn an_empty_token_is_still_sent_because_it_is_signed() {
        let q = member_info_request(1, "", &tokens(), "key", 1).query_string();
        assert!(
            q.contains("token=&"),
            "空 token 省掉会换一个不同的 w_rid — {q}"
        );
    }

    #[test]
    fn the_vip_status_falls_back_to_the_alternate_key() {
        let info =
            parse_member_info(r#"{"code":0,"data":{"mid":1,"vip":{"vipStatus":1,"vipType":2}}}"#)
                .expect("应解析成功");
        assert_eq!(info.vip_status, Some(1), "status 缺失时认 vipStatus");
        assert_eq!(info.vip_type, Some(2), "type 缺失时认 vipType");
    }

    #[test]
    fn a_missing_vip_block_is_not_a_zero_status() {
        let info = parse_member_info(r#"{"code":0,"data":{"mid":1}}"#).expect("应解析成功");
        assert_eq!(info.vip_status, None, "整块 vip 都没有时是 null，不是 0");
    }

    #[test]
    fn the_card_official_key_is_capitalised() {
        let card = parse_member_card(r#"{"code":0,"data":{"card":{"Official":{"type":1}}}}"#)
            .expect("应解析成功");
        assert_eq!(card.official_type, Some(1), "这条接口的键是 `Official`");
    }

    #[test]
    fn the_card_mid_stays_a_string() {
        let card =
            parse_member_card(r#"{"code":0,"data":{"card":{"mid":"123"}}}"#).expect("应解析成功");
        assert_eq!(card.mid.as_deref(), Some("123"), "服务端给的就是字符串");
    }

    #[test]
    fn the_card_request_asks_for_no_photo() {
        let q = member_card_request(7).query_string();
        assert_eq!(q, "mid=7&photo=false", "要的只是昵称与计数 — {q}");
    }

    #[test]
    fn up_stat_views_come_from_two_nested_blocks() {
        let stat = parse_member_up_stat(
            r#"{"code":0,"data":{"archive":{"view":10},"article":{"view":2},"likes":3}}"#,
        )
        .expect("应解析成功");
        assert_eq!(stat.archive_view, Some(10));
        assert_eq!(stat.article_view, Some(2));
        assert_eq!(stat.likes, Some(3));
    }

    #[test]
    fn reporting_is_unsafe_to_resend() {
        let w = report_member_request(1, "spam", None);
        assert_eq!(w.idempotency, Idempotency::Unsafe, "重发会重复提交举报工单");
        assert_eq!(w.csrf, CsrfIn::Body, "csrf 在表单里");
    }

    #[test]
    fn an_absent_reason_v2_is_omitted_entirely() {
        let WriteBody::Form(body) = report_member_request(1, "spam", None).body else {
            panic!("举报是表单体");
        };
        assert!(
            !body.iter().any(|(k, _)| k == "reason_v2"),
            "基线用 `?reasonV2`，null 时整条不发"
        );
    }

    #[test]
    fn a_report_without_a_status_field_counts_as_failed() {
        assert!(
            !parse_report_result("{}").expect("空对象是合法 JSON"),
            "没给 status 就不能当成功"
        );
        assert!(parse_report_result(r#"{"status":true}"#).expect("应解析成功"));
    }
}
