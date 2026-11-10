//! 接口调用：把「构造请求 → 发送 → 解析」串起来。
//!
//! 每个接口一个函数，泛型于传输实现。泛型而非 trait object：调用点只有
//! 一两处，单态化省掉一次虚调用，更重要的是 `impl Future` 的 Port 本来
//! 就不是对象安全的。

use bili_domain::error::DomainError;
use bili_domain::video::VideoDetail;

use crate::http::{HttpGet, HttpTransport, RequestContext, check_status};
use crate::request;

/// 取视频详情。
///
/// `mixin_key` 由 nav 接口的 key 推导（[`bili_protocol::wbi`]），`wts` 为
/// 当前 Unix 秒。两者都由调用方给出：前者有按日缓存策略（见 issue #7），
/// 后者让请求构造保持可被 fixture 固定的纯函数。
///
/// # Errors
/// - `bvid` 为空 → [`bili_domain::error::ErrorKind::Protocol`]；
/// - 传输失败 → `Network`；
/// - 非 2xx → 见 [`check_status`]；
/// - 业务 code 非 0 → 见 [`crate::envelope`]。
pub async fn fetch_video_detail<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    bvid: &str,
    mixin_key: &str,
    wts: i64,
) -> Result<VideoDetail, DomainError> {
    let prepared = request::video_detail(bvid, mixin_key, wts).ok_or_else(|| {
        DomainError::new(
            bili_domain::error::ErrorKind::Protocol,
            "bvid 为空，请求不会有有效结果",
        )
    })?;

    let url = format!(
        "{}{}?{}",
        ctx.base_url.trim_end_matches('/'),
        prepared.path,
        encode_query(&prepared.query)
    );

    // 只记 path，不记完整 URL：URL 里虽无凭据，但带着 bvid 与签名，
    // 日志里留一份对诊断无益，对隐私有损。
    tracing::debug!(path = prepared.path, "fetch video detail");

    let resp = transport
        .get(HttpGet {
            url,
            headers: ctx.all_headers(),
            timeout: ctx.timeout,
        })
        .await?;

    check_status(resp.status)?;
    crate::video_detail::parse(&resp.body)
}

/// 百分号编码查询串。
///
/// 与 WBI 签名用的是同一套编码集（RFC 3986 unreserved），否则签名覆盖的
/// 字符串与实际发送的不一致，服务端会以业务错误码拒绝，极难反推。
fn encode_query(params: &[(String, String)]) -> String {
    use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};
    const SET: &AsciiSet = &NON_ALPHANUMERIC
        .remove(b'-')
        .remove(b'.')
        .remove(b'_')
        .remove(b'~');
    params
        .iter()
        .map(|(k, v)| {
            format!(
                "{}={}",
                utf8_percent_encode(k, SET),
                utf8_percent_encode(v, SET)
            )
        })
        .collect::<Vec<_>>()
        .join("&")
}

#[cfg(test)]
// 测试里 expect 是正确做法：前提不成立时就该崩掉并指出原因。
#[allow(clippy::expect_used)]
mod tests {
    use std::sync::Mutex;
    use std::time::Duration;

    use bili_domain::error::ErrorKind;
    use bili_domain::redact::Redacted;

    use super::*;
    use crate::http::{HttpResponse, RequestContext};

    /// 桩传输：记录收到的请求，返回预置响应。
    ///
    /// 用桩而不是起本地 HTTP server：本层要验证的是 URL 拼装、头部透传与
    /// 错误分类，与真实套接字无关；真实网络只会让测试变慢且偶发失败。
    struct StubTransport {
        response: Result<HttpResponse, DomainError>,
        seen: Mutex<Option<HttpGet>>,
    }

    impl StubTransport {
        fn ok(status: u16, body: &str) -> Self {
            Self {
                response: Ok(HttpResponse {
                    status,
                    body: body.to_owned(),
                }),
                seen: Mutex::new(None),
            }
        }

        fn failing(kind: ErrorKind, detail: &str) -> Self {
            Self {
                response: Err(DomainError::new(kind, detail)),
                seen: Mutex::new(None),
            }
        }
    }

    impl HttpTransport for StubTransport {
        async fn get(&self, request: HttpGet) -> Result<HttpResponse, DomainError> {
            *self.seen.lock().expect("锁未被毒化") = Some(request);
            self.response.clone()
        }
    }

    fn ctx() -> RequestContext {
        RequestContext {
            base_url: "https://api.example.invalid".to_owned(),
            cookie: Some(Redacted::new("SESSDATA=secret".to_owned())),
            headers: vec![("user-agent".to_owned(), "test-agent".to_owned())],
            timeout: Duration::from_secs(10),
        }
    }

    const MIXIN_KEY: &str = "ea1db124af3c7062474693fa704f4ff8";

    fn block_on<F: Future>(f: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("测试运行时可创建")
            .block_on(f)
    }

    #[test]
    fn builds_signed_url_and_forwards_headers() {
        let t = StubTransport::ok(200, r#"{"code":0,"data":{"bvid":"BV1"}}"#);
        let detail = block_on(fetch_video_detail(
            &t,
            &ctx(),
            "BV1L94y1H7CV",
            MIXIN_KEY,
            1_702_204_169,
        ))
        .expect("解析应成功");
        assert_eq!(detail.bvid.as_deref(), Some("BV1"));

        let seen = t
            .seen
            .lock()
            .expect("锁未被毒化")
            .clone()
            .expect("传输应被调用一次");
        assert!(
            seen.url.starts_with(
                "https://api.example.invalid/x/web-interface/wbi/view?bvid=BV1L94y1H7CV&wts="
            ),
            "URL 拼装错误: {}",
            seen.url
        );
        assert!(seen.url.contains("&w_rid="), "签名必须随请求发出");
        assert!(
            seen.headers
                .iter()
                .any(|(k, v)| k == "cookie" && v == "SESSDATA=secret"),
            "Cookie 必须透传给传输层"
        );
        assert!(
            seen.headers
                .iter()
                .any(|(k, v)| k == "user-agent" && v == "test-agent"),
        );
    }

    #[test]
    fn credentials_never_appear_in_debug_output() {
        // RequestContext 会出现在错误上报与调试打印里，整个结构体被打印
        // 时也不能泄漏 Cookie。
        let printed = format!("{:?}", ctx());
        assert!(!printed.contains("SESSDATA=secret"), "凭据泄漏: {printed}");
        assert!(printed.contains("<redacted>"));

        let summary = ctx().redacted_headers().join("; ");
        assert!(summary.contains("cookie: <redacted>"));
        assert!(summary.contains("user-agent: test-agent"));
    }

    #[test]
    fn transport_failure_is_propagated_as_network() {
        let t = StubTransport::failing(ErrorKind::Network, "connection reset");
        let err = block_on(fetch_video_detail(&t, &ctx(), "BV1L94y1H7CV", MIXIN_KEY, 1))
            .expect_err("传输失败必须上报");
        assert_eq!(err.kind(), ErrorKind::Network);
    }

    #[test]
    fn risk_control_status_is_auth() {
        let t = StubTransport::ok(412, "");
        let err = block_on(fetch_video_detail(&t, &ctx(), "BV1L94y1H7CV", MIXIN_KEY, 1))
            .expect_err("412 必须失败");
        assert_eq!(
            err.kind(),
            ErrorKind::Auth,
            "风控拦截重试无意义，必须与可重试错误区分"
        );
    }

    #[test]
    fn server_error_is_retryable() {
        let t = StubTransport::ok(503, "");
        let err = block_on(fetch_video_detail(&t, &ctx(), "BV1L94y1H7CV", MIXIN_KEY, 1))
            .expect_err("503 必须失败");
        assert!(err.kind().is_retryable());
    }

    #[test]
    fn empty_bvid_never_reaches_the_network() {
        let t = StubTransport::ok(200, r#"{"code":0,"data":{}}"#);
        let err = block_on(fetch_video_detail(&t, &ctx(), "", MIXIN_KEY, 1))
            .expect_err("空 bvid 必须失败");
        assert_eq!(err.kind(), ErrorKind::Protocol);
        assert!(
            t.seen.lock().expect("锁未被毒化").is_none(),
            "不该发出注定失败的请求"
        );
    }
}
