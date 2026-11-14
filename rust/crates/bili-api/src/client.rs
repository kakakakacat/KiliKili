//! 接口调用：把「构造请求 → 发送 → 解析」串起来。
//!
//! 每个接口一个函数，泛型于传输实现。泛型而非 trait object：调用点只有
//! 一两处，单态化省掉一次虚调用，更重要的是 `impl Future` 的 Port 本来
//! 就不是对象安全的。

use bili_domain::error::DomainError;
use bili_domain::playurl::PlayUrl;
use bili_domain::video::{VideoDetail, VideoType};
use bili_protocol::risk::{Entropy, RiskTokens};

use crate::http::{HttpGet, HttpPost, HttpTransport, PostBody, RequestContext, check_status};
use crate::playurl::{self, Outcome};
use crate::request::{self, CsrfIn, PlayUrlParams, PreparedRequest, PreparedWrite, WriteBody};

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

    let body = send(transport, ctx, &prepared).await?;
    crate::video_detail::parse(&body)
}

/// 取流地址。
///
/// 把基线 `VideoHttp.videoUrl`（`lib/http/video.dart`）的**三条重试分支**
/// 一并搬了过来——少一条，Rust 路径就会在基线能播的内容上失败：
///
/// 1. 主接口（playview）返回 DRM 内容 → 改走降级接口，一次；
/// 2. UGC 请求带着 `ep_id` 却被服务端以非 0 码拒绝 → 改按 PGC 请求
///    （番剧的分享链接会给出 UGC 形态的参数）；
/// 3. 两条分支都只重试一次，且互不叠加——基线的递归同样如此。
///
/// 风控 token 每次尝试都重新生成（`entropy`）：复用同一对值等于给服务端
/// 一个稳定指纹，与这些参数存在的目的相悖。`wts` 则沿用首次取的值——
/// 重试紧随其后，落在同一秒内，重新取一次只会让请求更难复现。
///
/// # Errors
/// - 参数不足以标识视频 → `Protocol`；
/// - 传输失败 → `Network`；非 2xx → 见 [`check_status`]；
/// - 业务失败 → 见 [`crate::envelope`] 与 [`crate::playurl`]。
pub async fn fetch_play_url<T: HttpTransport, E: Entropy + ?Sized>(
    transport: &T,
    ctx: &RequestContext,
    params: &PlayUrlParams,
    entropy: &mut E,
    mixin_key: &str,
    wts: i64,
) -> Result<PlayUrl, DomainError> {
    let mut params = params.clone();
    // 两条重试分支各自只允许一次，分开记：DRM 降级之后若再撞上
    // 「UGC 带 ep_id」的分支，基线也还会再试一次。
    let mut drm_fallback_used = params.fallback;
    let mut pgc_switch_used = params.video_type != VideoType::Ugc;

    loop {
        let tokens = RiskTokens::generate(entropy);
        let prepared = request::play_url(&params, &tokens, mixin_key, wts).ok_or_else(|| {
            DomainError::new(
                bili_domain::error::ErrorKind::Protocol,
                "参数不足以标识一个视频（缺 cid 或标识符）",
            )
        })?;

        let body = send(transport, ctx, &prepared).await?;

        match playurl::parse(&body, params.video_type, params.fallback) {
            Ok(Outcome::Ready(play)) => return Ok(*play),
            Ok(Outcome::NeedsFallback) if !drm_fallback_used => {
                drm_fallback_used = true;
                params.fallback = true;
            }
            Ok(Outcome::NeedsFallback) => {
                return Err(DomainError::new(
                    bili_domain::error::ErrorKind::Remote,
                    "降级接口仍然只给出 DRM 内容",
                ));
            }
            // 基线：`code != 0 && epid != null && videoType == ugc` 时改按
            // PGC 请求。条件里用的是原始业务码，不是错误大类——因此
            // DomainError 带上了 code。
            Err(e)
                if e.code().is_some_and(|c| c != 0)
                    && !pgc_switch_used
                    && params.ep_id.is_some() =>
            {
                pgc_switch_used = true;
                params.video_type = VideoType::Pgc;
            }
            Err(e) => return Err(e),
        }
    }
}

/// 发一条已构造好的请求，返回响应体。
///
/// `pub(crate)`：其余接口模块（如 [`crate::video_feed`]）要的正是同一套
/// 「拼 URL → 带上下文头 → 查状态码」，各写一遍必然在某处漏掉 `check_status`。
pub(crate) async fn send<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    prepared: &PreparedRequest,
) -> Result<String, DomainError> {
    let url = format!(
        "{}{}?{}",
        ctx.base_url.trim_end_matches('/'),
        prepared.path,
        encode_query(&prepared.query)
    );

    // 只记 path：URL 里虽无凭据，却带着标识符与签名，日志里留一份对诊断
    // 无益，对隐私有损。
    tracing::debug!(path = prepared.path, "发送请求");

    let resp = transport
        .get(HttpGet {
            url,
            headers: ctx.all_headers(),
            timeout: ctx.timeout,
        })
        .await?;

    check_status(resp.status)?;
    Ok(resp.body)
}

/// 发送一个写请求，返回响应体。
///
/// 三件事只在这里做，各自都有理由不散到调用点去：
///
/// 1. **补 csrf**。基线是 126 处各自写 `'csrf': Accounts.main.csrf`；这里
///    是唯一注入点（ADR-013 决策四）。位置由 [`PreparedWrite::csrf`] 指定
///    ——基线并不统一：多数在体里，`roomEntryAction` 在查询串上，APP 端
///    接口根本没有 csrf。未登录时 `ctx.csrf` 为 `None`，不补——服务端会以
///    `-101` 拒绝，那是正确的失败，比本地伪造一个空 csrf 要诚实。
/// 2. **挑 content-type**。由 [`WriteBody`] 决定，调用点不经手。
/// 3. **把幂等级别交给传输层**。重试规则见 [`crate::http::should_retry`]。
///
/// # Errors
/// 传输与状态码两类失败，外加 JSON 体无处放 csrf（判协议错误）。业务信封
/// 由调用方解。
pub(crate) async fn send_write<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    prepared: &PreparedWrite,
) -> Result<String, DomainError> {
    let mut query = prepared.query.clone();
    let mut body = prepared.body.clone();

    if let Some(csrf) = &ctx.csrf {
        let csrf = csrf.expose().clone();
        match (prepared.csrf, &mut body) {
            (CsrfIn::Body, WriteBody::Form(form)) => form.push(("csrf".to_owned(), csrf)),
            (CsrfIn::Body, WriteBody::Json(value)) => {
                // 非对象的 JSON 体插不进字段。当前没有这种接口；真出现了
                // 就该在构造处把 csrf 挪到查询串，而不是在这里静默丢掉它
                // ——丢掉的表现是服务端回 -111，看起来像协议问题。
                let Some(obj) = value.as_object_mut() else {
                    return Err(DomainError::new(
                        bili_domain::error::ErrorKind::Protocol,
                        "JSON 体不是对象，csrf 无处可放——构造处应改用 CsrfIn::Query",
                    ));
                };
                obj.insert("csrf".to_owned(), serde_json::Value::String(csrf));
            }
            (CsrfIn::Query, _) => query.push(("csrf".to_owned(), csrf)),
            (CsrfIn::None, _) => {}
        }
    }

    let base = ctx.base_url.trim_end_matches('/');
    let url = if query.is_empty() {
        format!("{base}{}", prepared.path)
    } else {
        format!("{base}{}?{}", prepared.path, encode_query(&query))
    };

    let post_body = match body {
        WriteBody::Form(form) => PostBody::Form(form),
        WriteBody::Json(value) => PostBody::Json(value.to_string()),
    };

    // 只记 path 与幂等级别：体里有业务标识符，csrf 更是凭据。
    tracing::debug!(
        path = prepared.path,
        idempotency = ?prepared.idempotency,
        "发送写请求"
    );

    let resp = transport
        .post(HttpPost {
            url,
            headers: ctx.all_headers(),
            body: post_body,
            timeout: ctx.timeout,
            retry_policy: prepared.idempotency.retry_policy(),
        })
        .await?;

    check_status(resp.status)?;
    Ok(resp.body)
}

/// 百分号编码查询串。
///
/// 与 WBI 签名用的是同一套编码集（RFC 3986 unreserved），否则签名覆盖的
/// 字符串与实际发送的不一致，服务端会以业务错误码拒绝，极难反推。
pub(crate) fn encode_query(params: &[(String, String)]) -> String {
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
    use crate::http::{HttpResponse, Idempotency, RequestContext};

    /// 桩传输：记录收到的请求，返回预置响应。
    ///
    /// 用桩而不是起本地 HTTP server：本层要验证的是 URL 拼装、头部透传与
    /// 错误分类，与真实套接字无关；真实网络只会让测试变慢且偶发失败。
    struct StubTransport {
        response: Result<HttpResponse, DomainError>,
        seen: Mutex<Option<HttpGet>>,
        seen_post: Mutex<Option<HttpPost>>,
    }

    impl StubTransport {
        fn ok(status: u16, body: &str) -> Self {
            Self {
                response: Ok(HttpResponse {
                    status,
                    body: body.to_owned(),
                }),
                seen: Mutex::new(None),
                seen_post: Mutex::new(None),
            }
        }

        fn failing(kind: ErrorKind, detail: &str) -> Self {
            Self {
                response: Err(DomainError::new(kind, detail)),
                seen: Mutex::new(None),
                seen_post: Mutex::new(None),
            }
        }
    }

    impl HttpTransport for StubTransport {
        async fn get(&self, request: HttpGet) -> Result<HttpResponse, DomainError> {
            *self.seen.lock().expect("锁未被毒化") = Some(request);
            self.response.clone()
        }

        async fn post(&self, request: HttpPost) -> Result<HttpResponse, DomainError> {
            *self.seen_post.lock().expect("锁未被毒化") = Some(request);
            self.response.clone()
        }
    }

    fn ctx() -> RequestContext {
        RequestContext {
            base_url: "https://api.example.invalid".to_owned(),
            cookie: Some(Redacted::new("SESSDATA=secret".to_owned())),
            csrf: None,
            headers: vec![("user-agent".to_owned(), "test-agent".to_owned())],
            timeout: Duration::from_secs(10),
        }
    }

    /// 带 csrf 的上下文。值是占位串，不是真凭据。
    fn ctx_logged_in() -> RequestContext {
        RequestContext {
            csrf: Some(Redacted::new("csrf-placeholder".to_owned())),
            ..ctx()
        }
    }

    fn a_write() -> PreparedWrite {
        PreparedWrite::form_only(
            "/x/dm/filter/user/del",
            vec![("ids".to_owned(), "42".to_owned())],
            Idempotency::Idempotent,
        )
    }

    #[test]
    fn a_write_with_a_query_string_keeps_both_halves() {
        // sortFollowTag 把 x-bili-device-req-json 放在 URL 上，参数同时
        // 分布在查询串与表单里。两边都得送到。
        let stub = StubTransport::ok(200, r#"{"code":0,"message":"0"}"#);
        let prepared = PreparedWrite {
            path: "/x/relation/tags/update_sort",
            query: vec![("x-bili-device-req-json".to_owned(), "{\"a\":1}".to_owned())],
            body: WriteBody::Form(vec![("tagids".to_owned(), "1,2".to_owned())]),
            csrf: CsrfIn::Body,
            idempotency: Idempotency::Idempotent,
        };
        block_on(send_write(&stub, &ctx_logged_in(), &prepared)).expect("桩传输必定成功");

        let seen = stub.seen_post.lock().expect("锁未被毒化");
        let seen = seen.as_ref().expect("写请求应当送到传输层");
        assert_eq!(
            seen.url,
            "https://api.example.invalid/x/relation/tags/update_sort?x-bili-device-req-json=%7B%22a%22%3A1%7D",
            "查询串要百分号编码后拼到 URL 上"
        );
        let PostBody::Form(fields) = &seen.body else {
            panic!("写请求应当是表单体");
        };
        assert!(
            fields.iter().any(|(k, v)| k == "tagids" && v == "1,2"),
            "表单字段不能因为有查询串就丢了"
        );
    }

    #[test]
    fn write_injects_csrf_into_the_form() {
        let stub = StubTransport::ok(200, r#"{"code":0,"message":"0"}"#);
        block_on(send_write(&stub, &ctx_logged_in(), &a_write())).expect("桩传输必定成功");

        let seen = stub.seen_post.lock().expect("锁未被毒化");
        let seen = seen.as_ref().expect("写请求应当送到传输层");
        let PostBody::Form(fields) = &seen.body else {
            panic!("写请求应当是表单体");
        };
        assert_eq!(
            fields,
            &vec![
                ("ids".to_owned(), "42".to_owned()),
                ("csrf".to_owned(), "csrf-placeholder".to_owned()),
            ],
            "csrf 必须由这一层补，且补在业务字段之后"
        );
    }

    /// `roomEntryAction` 的 csrf 在查询串上，体是 JSON。两件事都与
    /// 「写操作的参数都在表单里」的惯例相反，而且是基线实际的形状。
    #[test]
    fn a_json_write_puts_csrf_on_the_query_string() {
        let stub = StubTransport::ok(200, r#"{"code":0,"message":"0"}"#);
        let prepared = PreparedWrite {
            path: "/xlive/web-room/v1/index/roomEntryAction",
            query: Vec::new(),
            body: WriteBody::Json(serde_json::json!({"room_id": 123, "platform": "pc"})),
            csrf: CsrfIn::Query,
            idempotency: Idempotency::Idempotent,
        };
        block_on(send_write(&stub, &ctx_logged_in(), &prepared)).expect("桩传输必定成功");

        let seen = stub.seen_post.lock().expect("锁未被毒化");
        let seen = seen.as_ref().expect("写请求应当送到传输层");
        assert!(
            seen.url.ends_with("?csrf=csrf-placeholder"),
            "csrf 应当补在查询串上: {}",
            seen.url
        );
        let PostBody::Json(body) = &seen.body else {
            panic!("这条接口的体是 JSON，不是表单");
        };
        // room_id 必须是数字而不是字符串：基线传的是 `Object`，dio 按
        // JSON 序列化，服务端看到的是数字。
        assert!(body.contains("\"room_id\":123"), "体不对: {body}");
        assert!(!body.contains("csrf"), "csrf 不该同时出现在体里: {body}");
    }

    /// APP 端接口用 `access_key` 鉴权，没有 csrf 这回事。补上去不会报错，
    /// 只会让请求面与基线对不上——那正是决策六要盯的东西。
    #[test]
    fn a_write_marked_no_csrf_never_gets_one() {
        let stub = StubTransport::ok(200, r#"{"code":0,"message":"0"}"#);
        let prepared = PreparedWrite {
            path: "/x/v2/view/like",
            query: Vec::new(),
            body: WriteBody::Form(vec![("aid".to_owned(), "2".to_owned())]),
            csrf: CsrfIn::None,
            idempotency: Idempotency::Idempotent,
        };
        block_on(send_write(&stub, &ctx_logged_in(), &prepared)).expect("桩传输必定成功");

        let seen = stub.seen_post.lock().expect("锁未被毒化");
        let seen = seen.as_ref().expect("写请求应当送到传输层");
        assert!(!seen.url.contains("csrf"), "URL 不该有 csrf: {}", seen.url);
        let PostBody::Form(fields) = &seen.body else {
            panic!("写请求应当是表单体");
        };
        assert!(fields.iter().all(|(k, _)| k != "csrf"), "表单不该有 csrf");
    }

    #[test]
    fn write_without_a_login_sends_no_csrf() {
        // 未登录时不伪造一个空 csrf：让服务端以 -101 拒绝，那是正确的失败。
        let stub = StubTransport::ok(200, r#"{"code":0,"message":"0"}"#);
        block_on(send_write(&stub, &ctx(), &a_write())).expect("桩传输必定成功");

        let seen = stub.seen_post.lock().expect("锁未被毒化");
        let seen = seen.as_ref().expect("写请求应当送到传输层");
        let PostBody::Form(fields) = &seen.body else {
            panic!("写请求应当是表单体");
        };
        assert!(
            fields.iter().all(|(k, _)| k != "csrf"),
            "未登录却带上了 csrf 字段"
        );
    }

    #[test]
    fn write_forwards_idempotency_and_builds_a_plain_url() {
        let stub = StubTransport::ok(200, r#"{"code":0,"message":"0"}"#);
        block_on(send_write(&stub, &ctx_logged_in(), &a_write())).expect("桩传输必定成功");

        let seen = stub.seen_post.lock().expect("锁未被毒化");
        let seen = seen.as_ref().expect("写请求应当送到传输层");
        assert_eq!(
            seen.retry_policy,
            Idempotency::Idempotent.retry_policy(),
            "幂等级别必须原样传到传输层，否则重试规则就落空了"
        );
        assert_eq!(
            seen.url, "https://api.example.invalid/x/dm/filter/user/del",
            "写请求的参数在体里，URL 不该带查询串"
        );
        assert!(
            seen.headers.iter().any(|(k, _)| k == "cookie"),
            "账号头要照常透传"
        );
    }

    #[test]
    fn write_maps_status_code_failures() {
        let stub = StubTransport::ok(412, "blocked");
        let err =
            block_on(send_write(&stub, &ctx_logged_in(), &a_write())).expect_err("412 必须是错误");
        assert_eq!(err.kind(), ErrorKind::Auth, "412 是风控，不该当成可重试");
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

    /// 脚本化传输：按顺序吐出预置响应，并记下每次请求的 URL。
    ///
    /// 重试分支的验证必须看到**第二次**请求长什么样——只有一个固定响应
    /// 的桩件无法区分「重试了」与「第一次就成功了」。
    struct ScriptedTransport {
        responses: Mutex<Vec<&'static str>>,
        urls: Mutex<Vec<String>>,
    }

    impl ScriptedTransport {
        fn new(responses: &[&'static str]) -> Self {
            let mut v = responses.to_vec();
            v.reverse();
            Self {
                responses: Mutex::new(v),
                urls: Mutex::new(Vec::new()),
            }
        }

        fn urls(&self) -> Vec<String> {
            self.urls.lock().expect("锁未被毒化").clone()
        }
    }

    impl HttpTransport for ScriptedTransport {
        async fn post(&self, _request: HttpPost) -> Result<HttpResponse, DomainError> {
            unimplemented!("本桩只用于只读重试分支，没有写请求经过它")
        }

        async fn get(&self, request: HttpGet) -> Result<HttpResponse, DomainError> {
            self.urls.lock().expect("锁未被毒化").push(request.url);
            let body = self
                .responses
                .lock()
                .expect("锁未被毒化")
                .pop()
                .expect("脚本里应当还有响应——多出来的请求本身就是失败");
            Ok(HttpResponse {
                status: 200,
                body: body.to_owned(),
            })
        }
    }

    /// 固定随机源：风控 token 的内容与本层的分支无关，只要不同次调用
    /// 都能生成即可。
    struct FixedEntropy;
    impl bili_protocol::risk::Entropy for FixedEntropy {
        fn next_below(&mut self, bound: u32) -> u32 {
            bound / 2
        }
    }

    fn pgc_params() -> PlayUrlParams {
        PlayUrlParams {
            ep_id: Some(123),
            video_type: VideoType::Pgc,
            ..PlayUrlParams::ugc("BV1L94y1H7CV", 42, 80)
        }
    }

    #[test]
    fn drm_content_retries_once_through_the_fallback_api() {
        let t = ScriptedTransport::new(&[
            r#"{"code":0,"data":{"arc":{"is_drm":true}}}"#,
            r#"{"code":0,"result":{"video_info":{"quality":112}}}"#,
        ]);
        let play = block_on(fetch_play_url(
            &t,
            &ctx(),
            &pgc_params(),
            &mut FixedEntropy,
            MIXIN_KEY,
            1_702_204_169,
        ))
        .expect("降级后应当拿到流");
        assert_eq!(play.quality, Some(112));

        let urls = t.urls();
        assert_eq!(urls.len(), 2, "基线只重试一次");
        assert!(urls[0].contains("/ogv/player/playview"));
        assert!(
            urls[1].contains("/pgc/player/web/v2/playurl"),
            "降级必须换接口: {}",
            urls[1]
        );
        assert!(
            !urls[1].contains("biz_type"),
            "降级接口不认 playview 的那组参数"
        );
    }

    #[test]
    fn drm_on_the_fallback_api_is_not_retried_again() {
        let t = ScriptedTransport::new(&[
            r#"{"code":0,"data":{"arc":{"is_drm":true}}}"#,
            r#"{"code":0,"result":{}}"#,
        ]);
        // 降级接口返回的 result 里没有 video_info：此处走的是
        // PgcFallbackDto 的缺省路径，不该再触发一次降级。
        let play = block_on(fetch_play_url(
            &t,
            &ctx(),
            &pgc_params(),
            &mut FixedEntropy,
            MIXIN_KEY,
            1,
        ))
        .expect("缺字段不该让整次取流失败");
        assert_eq!(play.quality, None);
        assert_eq!(t.urls().len(), 2);
    }

    #[test]
    fn ugc_with_epid_retries_as_pgc_on_business_error() {
        // 番剧的分享链接会给出 UGC 形态的参数；基线据此改按 PGC 请求。
        let t = ScriptedTransport::new(&[
            r#"{"code":-404,"message":"啥都木有"}"#,
            r#"{"code":0,"data":{"video_info":{"quality":80}}}"#,
        ]);
        let params = PlayUrlParams {
            ep_id: Some(123),
            ..PlayUrlParams::ugc("BV1L94y1H7CV", 42, 80)
        };
        let play = block_on(fetch_play_url(
            &t,
            &ctx(),
            &params,
            &mut FixedEntropy,
            MIXIN_KEY,
            1,
        ))
        .expect("改按 PGC 后应当成功");
        assert_eq!(play.quality, Some(80));

        let urls = t.urls();
        assert!(urls[0].contains("/x/player/wbi/playurl"));
        assert!(urls[1].contains("/ogv/player/playview"));
    }

    #[test]
    fn ugc_without_epid_reports_the_business_error() {
        let t = ScriptedTransport::new(&[r#"{"code":-404,"message":"啥都木有"}"#]);
        let err = block_on(fetch_play_url(
            &t,
            &ctx(),
            &PlayUrlParams::ugc("BV1L94y1H7CV", 42, 80),
            &mut FixedEntropy,
            MIXIN_KEY,
            1,
        ))
        .expect_err("没有 ep_id 就没有第二条路可走");
        assert_eq!(err.code(), Some(-404));
        assert_eq!(t.urls().len(), 1, "不得凭空多发一次请求");
    }

    #[test]
    fn risk_tokens_differ_between_attempts() {
        // 复用同一对 token 等于给服务端一个稳定指纹。
        let t = ScriptedTransport::new(&[
            r#"{"code":0,"data":{"arc":{"is_drm":true}}}"#,
            r#"{"code":0,"result":{"video_info":{}}}"#,
        ]);
        let mut entropy = CountingEntropy(0);
        block_on(fetch_play_url(
            &t,
            &ctx(),
            &pgc_params(),
            &mut entropy,
            MIXIN_KEY,
            1,
        ))
        .expect("应当成功");

        let urls = t.urls();
        let token_of = |url: &str| {
            url.split('&')
                .find(|p| p.starts_with("dm_img_str="))
                .map(ToOwned::to_owned)
                .expect("请求里必须带风控 token")
        };
        assert_ne!(token_of(&urls[0]), token_of(&urls[1]));
    }

    /// 每次调用都给出不同值的随机源替身。
    struct CountingEntropy(u32);
    impl bili_protocol::risk::Entropy for CountingEntropy {
        fn next_below(&mut self, bound: u32) -> u32 {
            self.0 = self.0.wrapping_add(7);
            self.0 % bound
        }
    }

    #[test]
    fn missing_identifier_never_reaches_the_network() {
        let t = ScriptedTransport::new(&[]);
        let err = block_on(fetch_play_url(
            &t,
            &ctx(),
            &PlayUrlParams::ugc("", 0, 80),
            &mut FixedEntropy,
            MIXIN_KEY,
            1,
        ))
        .expect_err("参数不足必须失败");
        assert_eq!(err.kind(), ErrorKind::Protocol);
        assert!(t.urls().is_empty(), "不该发出注定失败的请求");
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
