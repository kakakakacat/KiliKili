//! SponsorBlock API：请求构造、状态码语义与到领域实体的映射。
//!
//! 这是第一条按 **typed 直连**迁移的路径（issue #64，样板），与之前七条
//! 「Rust 解析 → JSON 字符串 → Dart 还原成模型」的路径形态不同：实体直接
//! 以 typed 结构跨 FFI，Dart 侧的模型随之删除。
//!
//! ## 与 bilibili 接口的三点不同
//!
//! 1. **没有业务信封**。成败只看 HTTP 状态码，所以本模块不用
//!    [`crate::envelope`]，也不能用 [`crate::http::check_status`]——那个
//!    函数把 404 归成 `Protocol`，而这里 404 是「这个视频没有片段」，是
//!    正常结果之一。
//! 2. **基地址可配**。用户能在设置里换自建服务器（基线 `Pref.blockServer`），
//!    因此由调用方按请求传入，Rust 不存。
//! 3. **错误文案是协议的一部分**。上游 wiki 对每个状态码都给了确切含义
//!    （400 参数错误、403 被自动审核拒绝、409 重复提交…），基线把这张表
//!    写在 `SponsorBlock.getErrMsg` 里。表留在 Rust：它是对协议的理解，
//!    不是界面措辞；`error_code` 仍带回 HTTP 状态码，Dart 要分支照样能分。

use bili_domain::error::{DomainError, ErrorKind};
use bili_domain::sponsor_block::{SkipSegment, SponsorUserInfo};
use serde::Deserialize;

use crate::http::{HttpGet, HttpTransport, RequestContext};
use crate::request::PreparedRequest;

/// 接口路径。与基线 `lib/http/sponsor_block_api.dart` 保持一致。
pub mod path {
    pub const SKIP_SEGMENTS: &str = "skipSegments";
    pub const USER_INFO: &str = "userInfo";
    pub const UPTIME_STATUS: &str = "status/uptime";
    pub const PORT_VIDEO: &str = "portVideo";
}

/// 取一个视频的可跳过片段。
///
/// `base_url` 是用户配置的服务器地址（基线 `Pref.blockServer`）。
///
/// # Errors
/// 404 表示该视频没有片段，同样以错误返回（`error_code` 为 404）——基线
/// 的调用点正是靠这个码把"没有数据"与"真的出错了"区分开的。
pub async fn fetch_skip_segments<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    base_url: &str,
    bvid: &str,
    cid: i64,
) -> Result<Vec<SkipSegment>, DomainError> {
    let req = PreparedRequest {
        path: path::SKIP_SEGMENTS,
        query: vec![
            ("videoID".to_owned(), bvid.to_owned()),
            ("cid".to_owned(), cid.to_string()),
        ],
    };
    let body = send(transport, ctx, base_url, &req).await?;
    parse_segments(&body)
}

/// 取提交者统计。`values` 是要查询的字段名列表，原样交给服务端。
///
/// # Errors
/// 见 [`status_error`]。
pub async fn fetch_user_info<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    base_url: &str,
    user_id: &str,
    values: &[String],
) -> Result<SponsorUserInfo, DomainError> {
    // 基线用 jsonEncode(query) 拼这个参数，服务端要的就是一个 JSON 数组
    // 字面量，不是重复的同名参数。
    let values = serde_json::to_string(values)
        .map_err(|e| DomainError::new(ErrorKind::Protocol, format!("values 无法序列化: {e}")))?;

    let req = PreparedRequest {
        path: path::USER_INFO,
        query: vec![
            ("userID".to_owned(), user_id.to_owned()),
            ("values".to_owned(), values),
        ],
    };
    let body = send(transport, ctx, base_url, &req).await?;
    let dto: UserInfoDto = serde_json::from_str(&body)
        .map_err(|e| DomainError::new(ErrorKind::Protocol, format!("响应结构不符: {e}")))?;
    Ok(dto.into_domain())
}

/// 服务器是否在线。
///
/// 基线的判定是「200 且响应体是纯数字」——那串数字是服务端的 uptime。
/// 返回 `Ok(())` 即在线。
///
/// # Errors
/// 见 [`status_error`]；响应体不是数字时为 `Protocol`。
pub async fn fetch_uptime_status<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    base_url: &str,
) -> Result<(), DomainError> {
    let req = PreparedRequest {
        path: path::UPTIME_STATUS,
        query: Vec::new(),
    };
    let body = send(transport, ctx, base_url, &req).await?;
    if body.trim().parse::<f64>().is_ok() {
        Ok(())
    } else {
        Err(DomainError::new(ErrorKind::Protocol, "uptime 响应不是数字"))
    }
}

/// 查这个稿件绑定的 YouTube 视频 id。
///
/// # Errors
/// 见 [`status_error`]；200 但没有 `ytbID` 时为 `Protocol`。
pub async fn fetch_port_video<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    base_url: &str,
    bvid: &str,
    cid: i64,
) -> Result<String, DomainError> {
    let req = PreparedRequest {
        path: path::PORT_VIDEO,
        query: vec![
            ("videoID".to_owned(), bvid.to_owned()),
            ("cid".to_owned(), cid.to_string()),
        ],
    };
    let body = send(transport, ctx, base_url, &req).await?;

    #[derive(Deserialize)]
    struct PortVideoDto {
        #[serde(rename = "ytbID")]
        ytb_id: Option<String>,
    }

    let dto: PortVideoDto = serde_json::from_str(&body)
        .map_err(|e| DomainError::new(ErrorKind::Protocol, format!("响应结构不符: {e}")))?;
    dto.ytb_id
        .filter(|s| !s.is_empty())
        .ok_or_else(|| DomainError::new(ErrorKind::Protocol, "响应里没有 ytbID"))
}

/// 解析片段列表。
///
/// # Errors
/// 结构不符为 `Protocol`。
pub fn parse_segments(body: &str) -> Result<Vec<SkipSegment>, DomainError> {
    let dtos: Vec<SegmentDto> = serde_json::from_str(body)
        .map_err(|e| DomainError::new(ErrorKind::Protocol, format!("响应结构不符: {e}")))?;
    Ok(dtos
        .into_iter()
        .filter_map(SegmentDto::into_domain)
        .collect())
}

/// 发一条请求并按状态码判成败。
///
/// 不走 [`crate::http::check_status`]：那套分类是给 bilibili 的信封接口用
/// 的，这里 404 是正常结果之一（见 [`fetch_skip_segments`]）。
async fn send<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    base_url: &str,
    req: &PreparedRequest,
) -> Result<String, DomainError> {
    let query = if req.query.is_empty() {
        String::new()
    } else {
        format!("?{}", crate::client::encode_query(&req.query))
    };
    let url = format!(
        "{}/api/{}{}",
        base_url.trim_end_matches('/'),
        req.path,
        query
    );

    tracing::debug!(path = req.path, "sponsor block 请求");

    let resp = transport
        .get(HttpGet {
            url,
            headers: ctx.all_headers(),
            timeout: ctx.timeout,
        })
        .await?;

    if resp.status == 200 {
        Ok(resp.body)
    } else {
        Err(status_error(resp.status, &resp.body))
    }
}

/// 状态码 → 领域错误。文案来自上游 wiki，与基线
/// `SponsorBlock.getErrMsg` 逐条对应。
///
/// `error_code` 带回原始 HTTP 状态码：调用点要靠它区分 404（没有片段，
/// 静默）与其余失败（提示用户）。
fn status_error(status: u16, body: &str) -> DomainError {
    let reason = match status {
        400 => "参数错误",
        403 => "被自动审核机制拒绝",
        404 => "未找到数据",
        409 => "重复提交",
        429 => "提交太快（触发速率控制）",
        500 => "服务器无法获取信息",
        _ => "请求失败",
    };

    // 基线会把短响应体附在文案后面——这类 API 的 400/403 常把真正的原因
    // 写在 body 里。长响应体不附：那基本是一页 HTML。
    let detail = if !body.is_empty() && body.len() < 200 {
        format!("{reason}（{status}）：{body}")
    } else {
        format!("{reason}（{status}）")
    };

    let kind = match status {
        403 | 429 => ErrorKind::Auth,
        500..=599 => ErrorKind::Remote,
        _ => ErrorKind::Remote,
    };
    DomainError::with_code(kind, detail, i64::from(status))
}

#[derive(Debug, Deserialize)]
struct SegmentDto {
    #[serde(rename = "UUID")]
    uuid: Option<String>,
    category: Option<String>,
    #[serde(rename = "actionType")]
    action_type: Option<String>,
    /// `[起, 止]`，单位秒。
    segment: Option<Vec<f64>>,
    #[serde(rename = "videoDuration")]
    video_duration: Option<f64>,
    votes: Option<i64>,
    cid: Option<String>,
}

impl SegmentDto {
    /// 缺 uuid / 分类 / 起止的片段直接丢掉。
    ///
    /// 基线在这里会抛异常（`json["UUID"]` 赋给非空 `String`），整次查询
    /// 作废。丢掉单条更合适：其余片段仍然可用，而一条残缺片段本来也无从
    /// 跳转。
    fn into_domain(self) -> Option<SkipSegment> {
        let segment = self.segment?;
        let (start, end) = (*segment.first()?, *segment.get(1)?);

        Some(SkipSegment {
            uuid: self.uuid?,
            category: self.category?,
            action_type: self.action_type,
            // 基线：(秒 * 1000).round()
            start_ms: round_ms(start),
            end_ms: round_ms(end),
            // 基线此处不取整，保持一致。
            video_duration_ms: self.video_duration.map(|d| d * 1000.0),
            votes: self.votes,
            cid: self.cid,
        })
    }
}

/// 秒 → 毫秒，四舍五入。对应基线的 `(e as num * 1000).round()`。
#[allow(clippy::cast_possible_truncation)]
fn round_ms(seconds: f64) -> i64 {
    (seconds * 1000.0).round() as i64
}

#[derive(Debug, Deserialize)]
struct UserInfoDto {
    #[serde(rename = "viewCount")]
    view_count: Option<i64>,
    #[serde(rename = "minutesSaved")]
    minutes_saved: Option<f64>,
    #[serde(rename = "segmentCount")]
    segment_count: Option<i64>,
}

impl UserInfoDto {
    fn into_domain(self) -> SponsorUserInfo {
        // 基线这三个字段都是非空读取，缺失即抛异常。统计信息缺一项不该让
        // 整个页面打不开，按 0 处理。
        SponsorUserInfo {
            view_count: self.view_count.unwrap_or(0),
            minutes_saved: self.minutes_saved.unwrap_or(0.0),
            segment_count: self.segment_count.unwrap_or(0),
        }
    }
}

#[cfg(test)]
// 测试里 expect 是正确做法：前提不成立时就该崩掉并指出原因。
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn seconds_become_rounded_milliseconds() {
        // 基线 SegmentItemModel.fromJson：(秒 * 1000).round()
        let body = r#"[{"UUID":"u1","category":"sponsor","actionType":"skip",
                        "segment":[12.3456,78.9],"videoDuration":600.5,"votes":3}]"#;
        let segs = parse_segments(body).expect("应当解析成功");
        assert_eq!(segs.len(), 1);
        assert_eq!(segs[0].start_ms, 12_346, "四舍五入，不是截断");
        assert_eq!(segs[0].end_ms, 78_900);
        // 时长基线不取整
        assert!((segs[0].video_duration_ms.expect("有时长") - 600_500.0).abs() < f64::EPSILON);
    }

    #[test]
    fn unknown_category_is_kept_as_is() {
        // 服务端随时会加新分类。收成枚举会让新分类变成解析错误，整条
        // 列表拿不到；保持字符串，由 UI 自行忽略。
        let body = r#"[{"UUID":"u1","category":"brand_new_category","segment":[0,1]}]"#;
        let segs = parse_segments(body).expect("应当解析成功");
        assert_eq!(segs[0].category, "brand_new_category");
    }

    #[test]
    fn incomplete_segment_is_dropped_not_fatal() {
        // 基线在这里会抛异常，整次查询作废。丢掉单条更合适：其余片段
        // 仍然可用，而一条残缺片段本来也无从跳转。
        let body = r#"[{"UUID":"u1","category":"sponsor","segment":[0,1]},
                       {"category":"sponsor","segment":[2,3]},
                       {"UUID":"u3","category":"sponsor"},
                       {"UUID":"u4","category":"sponsor","segment":[5]}]"#;
        let segs = parse_segments(body).expect("应当解析成功");
        let ids: Vec<&str> = segs.iter().map(|s| s.uuid.as_str()).collect();
        assert_eq!(ids, ["u1"], "残缺的三条应被丢掉，完整的那条要留下");
    }

    #[test]
    fn empty_list_is_not_an_error() {
        assert!(parse_segments("[]").expect("空列表是合法响应").is_empty());
    }

    #[test]
    fn malformed_body_is_a_protocol_error() {
        let err = parse_segments("not json").expect_err("必须失败");
        assert_eq!(err.kind(), ErrorKind::Protocol);
    }

    #[test]
    fn status_errors_carry_the_http_code_and_upstream_wording() {
        // 调用点要靠 404 区分「这个视频没有片段」与「真的出错了」。
        let e = status_error(404, "Not Found");
        assert_eq!(e.code(), Some(404));
        assert!(e.detail().contains("未找到数据"));

        let e = status_error(429, "");
        assert_eq!(e.kind(), ErrorKind::Auth, "速率控制重试只会继续被拒");
        assert!(e.detail().contains("提交太快"));

        let e = status_error(403, "rejected by auto moderator");
        assert!(e.detail().contains("被自动审核机制拒绝"));
        assert!(
            e.detail().contains("rejected by auto moderator"),
            "短响应体要附上"
        );
    }

    #[test]
    fn long_error_body_is_not_appended() {
        // 长响应体基本是一页 HTML，附上只会把日志刷满。
        let html = "x".repeat(500);
        let e = status_error(500, &html);
        assert!(!e.detail().contains(&html));
        assert!(e.detail().contains("服务器无法获取信息"));
    }

    #[test]
    fn user_info_missing_fields_default_to_zero() {
        // 基线三个字段都是非空读取，缺失即抛异常。统计信息缺一项不该让
        // 整个页面打不开。
        let dto: UserInfoDto = serde_json::from_str(r#"{"viewCount":12}"#).expect("可解析");
        let info = dto.into_domain();
        assert_eq!(info.view_count, 12);
        assert_eq!(info.segment_count, 0);
        assert!((info.minutes_saved - 0.0).abs() < f64::EPSILON);
    }
}
