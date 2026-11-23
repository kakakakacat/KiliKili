//! 登录流程（S14 / issue #65）。
//!
//! 对应基线 `lib/http/login.dart` 的 13 条接口：扫码、密码、短信、风控
//! 验证手机号、已登录设备、登出，以及极验注册是否还可用的探测。
//!
//! ## 凭据边界（本切片要重新审视的那条约定）
//!
//! RFC-0001 §2.2 原话是「凭据不落 Rust、按请求传入、用完即弃」。登录是
//! 凭据的产生地，照搬那句话会得出一个不成立的结论（"Rust 不该碰凭据"），
//! 所以在这里写死新的边界：
//!
//! 1. **不保存**：本模块没有任何静态变量、缓存或磁盘写入。登录结果沿
//!    返回值交给 Dart 的账号体系存，真相源仍然只有一处。
//! 2. **不打印**：凭据字段一律由 [`bili_domain::login`] 用 `Redacted`
//!    包住；错误里只有 code 与服务端文案，不含请求参数（见
//!    [`crate::envelope`]）。
//! 3. **明文密码不进结构体**：它作为 `&str` 参数进来，立刻交给
//!    [`bili_protocol::login_crypto`] 加密，不被任何字段持有。
//! 4. **web key 不出 Rust**：基线先让 UI 调一次 `getWebKey` 拿 RSA 公钥
//!    和盐，再把它们传给登录接口。这里把取公钥并入登录函数内部——
//!    Dart 从此拿不到也不需要这两个值，加密材料与明文密码在同一次调用
//!    内生灭。少两条跨 FFI 的边，也少两处可能把盐记进日志的地方。
//!
//! ## 为什么没有降级开关
//!
//! ADR-011 决策四：迁一条路径就删掉 Dart 实现。登录尤其不能留两套——
//! 两套各自生成 `device_id`、各自签名，出问题时无法判断用户是从哪条路
//! 登进来的。代价是这片没有兜底，所以它单独走一次 `/security-review`
//! 并单独合（issue #65）。
//!
//! ## 重试定级
//!
//! 这一片**没有一条是可自动重试的**（见各函数的 [`Idempotency`]）：
//! 短信重发会让用户收到两条，验证码与 oauth code 都是一次性的，而
//! 重新申请一次人机验证注册会让界面手里的 gt/challenge/token 失配——
//! 那正是 #140 那个"验证码弹空白页"的成因。

use bili_domain::error::{DomainError, ErrorKind};
use bili_domain::login::{
    CaptchaRegistration, CaptchaSolution, LoginCookie, LoginDevice, LoginTokens, PasswordLogin,
    QrCode, QrPoll, RiskVerification, SafeCenterInfo, SmsSent,
};
use bili_domain::redact::Redacted;
use bili_protocol::{login_crypto, login_ids};
use serde::Deserialize;

use crate::envelope::RemoteFailure;
use crate::http::{HttpTransport, Idempotency, RequestContext};
use crate::request::{CsrfIn, PreparedRequest, PreparedWrite, WriteBody};

/// 接口路径。与基线 `lib/http/api.dart` 保持一致。
///
/// 除最后一条外都挂在 `passport.bilibili.com`；base_url 由调用方给出
/// （与本 crate 其余模块一致，不在两处各写一份主机名）。
pub mod path {
    pub const TV_QRCODE: &str = "/x/passport-tv-login/qrcode/auth_code";
    pub const TV_QRCODE_POLL: &str = "/x/passport-tv-login/qrcode/poll";
    pub const WEB_KEY: &str = "/x/passport-login/web/key";
    pub const SMS_SEND: &str = "/x/passport-login/sms/send";
    pub const LOGIN_BY_SMS: &str = "/x/passport-login/login/sms";
    pub const LOGIN_BY_PASSWORD: &str = "/x/passport-login/oauth2/login";
    pub const OAUTH2_ACCESS_TOKEN: &str = "/x/passport-login/oauth2/access_token";
    pub const SAFE_CENTER_INFO: &str = "/x/safecenter/user/info";
    pub const SAFE_CENTER_PRE_CAPTURE: &str = "/x/safecenter/captcha/pre";
    pub const SAFE_CENTER_SMS_SEND: &str = "/x/safecenter/common/sms/send";
    pub const SAFE_CENTER_SMS_VERIFY: &str = "/x/safecenter/login/tel/verify";
    pub const LOGIN_DEVICES: &str = "/x/safecenter/user_login_devices";
    pub const LOGOUT: &str = "/login/exit/v2";
    /// 极验自己的配置探测接口，挂在 `api.geetest.com`。
    pub const GEETEST_TYPE: &str = "/gettype.php";
    /// 极验的主机。与其余接口不同，它不是 bilibili 的域，基线也是把整条
    /// URL 写死在调用点的；放在这里是为了让"换了主机"这件事有一处可改。
    pub const GEETEST_BASE_URL: &str = "https://api.geetest.com";
}

/// 登录请求里的固定常量。值照抄基线，不求甚解——它们是客户端指纹的一
/// 部分，换一个值换来的是风控分支，不是参数错误。
mod fixed {
    pub const BUILD: &str = "2001100";
    pub const MOBI_APP: &str = "android_hd";
    pub const PLATFORM: &str = "android";
    pub const CHANNEL: &str = "master";
    pub const LOCALE: &str = "zh_CN";
    pub const DISABLE_RCMD: &str = "0";
    pub const DEVICE: &str = "phone";
    pub const DEVICE_NAME: &str = "vivo";
    pub const DEVICE_PLATFORM: &str = "Android14vivo";
    pub const PERMISSION: &str = "ALL";
    /// 风控验证手机号时的默认短信类型。
    pub const SMS_TYPE_LOGIN_TEL_CHECK: &str = "loginTelCheck";

    /// 密码登录的来源埋点。
    pub const PWD_FROM_PV: &str = "main.homepage.avatar-nologin.all.click";
    /// 基线写的是 `Uri.encodeComponent('bilibili://pegasus/promo')`，
    /// 编码后的串才是真正发出去的值；这里直接写编码后的形态，免得在
    /// 「什么时候编码」上再绕一圈。
    pub const PWD_FROM_URL: &str = "bilibili%3A%2F%2Fpegasus%2Fpromo";
    /// 短信登录的来源埋点。
    pub const SMS_FROM_PV: &str = "main.my-information.my-login.0.click";
    /// 同上，`bilibili://user_center/mine` 的编码形态。
    pub const SMS_FROM_URL: &str = "bilibili%3A%2F%2Fuser_center%2Fmine";
}

/// 一次登录请求所需的客户端标识。
///
/// 与 [`crate::video_write::AppAuth`] 同类：appkey/appsec 等值的真相源在
/// 宿主（基线 `Constants`），按请求传入。**没有任何用户凭据**——那些走
/// [`RequestContext`]。
#[derive(Clone)]
pub struct LoginEnv {
    pub appkey: String,
    pub appsec: String,
    /// 匿名账号的 buvid。同时充当 `local_id`。
    pub buvid: String,
    /// 设备号（`device_id` / `bili_local_id`）。
    ///
    /// 由 [`bili_protocol::login_ids::device_id`] 生成，**每个进程一次**
    /// ——基线是 `static final deviceId`，同一次启动里所有登录请求共用
    /// 一个值。每请求换一个等于每次都是一台新设备，是风控最在意的形状。
    pub device_id: String,
    pub user_agent: String,
    pub statistics: String,
    pub trace_id: String,
    /// 签名用的 Unix 秒。由调用方注入，使请求构造保持纯函数。
    pub ts: i64,
    /// 毫秒时间戳，用于 `login_session_id` 与短信接口的 `ts`。
    pub now_millis: i64,
}

impl std::fmt::Debug for LoginEnv {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // buvid 在 `CREDENTIAL_KEYS` 里；device_id 同样是设备指纹。
        f.debug_struct("LoginEnv")
            .field("device", &"<redacted>")
            .field("ts", &self.ts)
            .finish_non_exhaustive()
    }
}

impl LoginEnv {
    /// APP 登录接口额外带的那组头（基线 `LoginHttp.headers`）。
    ///
    /// 只有短信发送、密码登录、短信登录、换 token 四条带它；其余接口用
    /// 调用方给的默认头。多带或少带都会换来不同的风控分支，故逐条照抄。
    ///
    /// **不含 `content-type`**：表单的 content-type 由传输层按
    /// [`crate::http::PostBody`] 决定，这里再写一份会出现两个同名头。
    #[must_use]
    pub fn app_headers(&self) -> Vec<(String, String)> {
        vec![
            ("buvid".to_owned(), self.buvid.clone()),
            ("env".to_owned(), "prod".to_owned()),
            ("app-key".to_owned(), fixed::MOBI_APP.to_owned()),
            ("user-agent".to_owned(), self.user_agent.clone()),
            ("x-bili-trace-id".to_owned(), self.trace_id.clone()),
            ("x-bili-aurora-eid".to_owned(), String::new()),
            ("x-bili-aurora-zone".to_owned(), String::new()),
            ("bili-http-engine".to_owned(), "cronet".to_owned()),
        ]
    }

    fn sign(&self, params: Vec<(String, String)>) -> Vec<(String, String)> {
        bili_protocol::app_sign::sign(&params, &self.appkey, &self.appsec, self.ts)
    }

    fn login_session_id(&self) -> String {
        login_ids::login_session_id(&self.buvid, self.now_millis)
    }
}

/// 把 [`LoginEnv::app_headers`] 合进上下文。
///
/// 同名头以登录头为准：reqwest 会保留同名头的**多个**值，而基线 Dio 是
/// 逐键覆盖，只发最终值（与 `RustVideoWrite._webContextWithHeaders` 里
/// 的理由相同）。
fn with_app_headers(ctx: &RequestContext, env: &LoginEnv) -> RequestContext {
    let extra = env.app_headers();
    let overridden: Vec<String> = extra.iter().map(|(k, _)| k.to_ascii_lowercase()).collect();
    let mut headers: Vec<(String, String)> = ctx
        .headers
        .iter()
        .filter(|(k, _)| !overridden.contains(&k.to_ascii_lowercase()))
        .cloned()
        .collect();
    headers.extend(extra);
    RequestContext {
        base_url: ctx.base_url.clone(),
        cookie: ctx.cookie.clone(),
        csrf: ctx.csrf.clone(),
        headers,
        timeout: ctx.timeout,
    }
}

/// 加一个请求头（风控那三条接口要带 `Referer`，值是服务端给的 URL）。
fn with_header(ctx: &RequestContext, name: &str, value: &str) -> RequestContext {
    let mut headers: Vec<(String, String)> = ctx
        .headers
        .iter()
        .filter(|(k, _)| !k.eq_ignore_ascii_case(name))
        .cloned()
        .collect();
    headers.push((name.to_owned(), value.to_owned()));
    RequestContext {
        base_url: ctx.base_url.clone(),
        cookie: ctx.cookie.clone(),
        csrf: ctx.csrf.clone(),
        headers,
        timeout: ctx.timeout,
    }
}

// ------------------------------------------------------------- 信封：三态

/// 登录这一片的信封解法。
///
/// 与 [`crate::envelope::unwrap_envelope`] 的区别：**`code != 0` 不一定是
/// 失败**。二维码未扫（86039）、密码登录要人机验证（-105）、短信发送被
/// 风控拦（也是 -105）都带着有用的 `data`，基线正是靠它们继续走流程。
/// 这里把三者统一成「code + message + data」，由各接口自己判定。
/// #166 的核对结论：`code` **不放宽，也不改成 `Option`**。它是信封本身，
/// 读不出来就没有任何可依据的结论可下；把它变成 `None` 只会让后面每一条
/// 分支都在猜。当前没有把 `code` 发成字符串的实测样本，有了再议。
#[derive(Debug)]
struct Envelope {
    code: i64,
    message: String,
    data: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct RawEnvelope {
    code: i64,
    #[serde(default)]
    message: String,
    #[serde(default)]
    data: Option<serde_json::Value>,
}

fn split_envelope(body: &str) -> Result<Envelope, DomainError> {
    let raw: RawEnvelope = serde_json::from_str(body).map_err(|e| {
        DomainError::new(ErrorKind::Protocol, format!("响应不是预期的信封结构: {e}"))
    })?;
    Ok(Envelope {
        code: raw.code,
        message: raw.message,
        data: raw.data,
    })
}

impl Envelope {
    /// 按「只有 code==0 才继续」处理，并取出 data。
    fn into_data(self) -> Result<serde_json::Value, DomainError> {
        if self.code != 0 {
            return Err(RemoteFailure {
                code: self.code,
                message: self.message,
            }
            .into_domain_error());
        }
        self.data
            .ok_or_else(|| DomainError::new(ErrorKind::Protocol, "code 为 0 但 data 缺失"))
    }

    fn failure(self) -> DomainError {
        RemoteFailure {
            code: self.code,
            message: self.message,
        }
        .into_domain_error()
    }
}

fn from_value<T: serde::de::DeserializeOwned>(value: serde_json::Value) -> Result<T, DomainError> {
    serde_json::from_value(value)
        .map_err(|e| DomainError::new(ErrorKind::Protocol, format!("响应结构与预期不符: {e}")))
}

// ------------------------------------------------------------------ DTO

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(default)]
struct CookieDto {
    name: Option<String>,
    value: Option<String>,
}

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(default)]
struct CookieInfoDto {
    cookies: Option<Vec<CookieDto>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct TokenInfoDto {
    access_token: Option<String>,
    refresh_token: Option<String>,
    /// **放宽**（#166）。这个字段整个 `lib/` 没有任何消费者：它跨 FFI 传到
    /// `LoginTokens.expiresIn` 就没人再读了（续期靠 refresh_token，不靠它
    /// 算时间）。一个没人用的字段如果类型猜错，代价是**整条登录响应**判成
    /// 协议错误、用户登不进去——#161 的 `next_exp` 就是这个形状。
    #[serde(deserialize_with = "crate::lenient::opt_i64")]
    expires_in: Option<i64>,
}

/// 扫码轮询成功时的载荷：token 字段直接在 data 上，不在 `token_info` 里。
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct QrPollDto {
    access_token: Option<String>,
    refresh_token: Option<String>,
    /// 同 [`TokenInfoDto::expires_in`]：没有消费者，放宽（#166）。
    #[serde(deserialize_with = "crate::lenient::opt_i64")]
    expires_in: Option<i64>,
    cookie_info: Option<CookieInfoDto>,
}

/// 密码登录 / 换 token 的载荷。
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct LoginDto {
    /// 2 表示风控要求验证手机号。
    ///
    /// **不放宽**（#166）：它决定走哪条分支——继续登录，还是跳验证手机号的
    /// 页面。分支选错比报错难查得多，这种字段宁可让整条响应失败。缺失本身
    /// 是合法的（普通登录成功时就没有），`Option` 已经表达了这一点。
    status: Option<i64>,
    message: Option<String>,
    url: Option<String>,
    token_info: Option<TokenInfoDto>,
    cookie_info: Option<CookieInfoDto>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct WebKeyDto {
    hash: Option<String>,
    key: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SmsSendDto {
    captcha_key: Option<String>,
    recaptcha_url: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct PreCaptureDto {
    gee_gt: Option<String>,
    gee_challenge: Option<String>,
    recaptcha_token: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct AccountInfoDto {
    hide_tel: Option<String>,
    hide_mail: Option<String>,
    tel_verify: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SafeCenterInfoDto {
    account_info: Option<AccountInfoDto>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct CaptchaKeyDto {
    captcha_key: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct OauthCodeDto {
    code: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct LoginDeviceDto {
    device_name: Option<String>,
    is_current_device: Option<bool>,
    latest_login_at: Option<String>,
    source: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct LoginDevicesDto {
    devices: Option<Vec<LoginDeviceDto>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct QrCodeDto {
    auth_code: Option<String>,
    url: Option<String>,
}

fn cookies_from(info: Option<CookieInfoDto>) -> Vec<LoginCookie> {
    info.and_then(|i| i.cookies)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|c| {
            // 名或值缺一就整条丢掉：半截 cookie 存进去的后果是登录态
            // 时灵时不灵，比当场少一条难查得多。
            Some(LoginCookie {
                name: c.name?,
                value: Redacted::new(c.value?),
            })
        })
        .collect()
}

/// `token_info` + `cookie_info` → 领域实体。
///
/// 两者缺一即判协议错误：基线在控制器里检查
/// `data['token_info'] == null || data['cookie_info'] == null` 并提示
/// "接口未返回身份信息"，那是对的，只是位置错了——这是协议判断。
fn tokens_from(
    token: Option<TokenInfoDto>,
    cookie: Option<CookieInfoDto>,
) -> Result<LoginTokens, DomainError> {
    let token = token.ok_or_else(|| {
        DomainError::new(
            ErrorKind::Auth,
            "接口未返回身份信息（token_info 为空），可能是账号风控",
        )
    })?;
    let cookies = cookies_from(cookie);
    if cookies.is_empty() {
        return Err(DomainError::new(
            ErrorKind::Auth,
            "接口未返回登录 Cookie，可能是账号风控",
        ));
    }
    let (Some(access), Some(refresh)) = (token.access_token, token.refresh_token) else {
        return Err(DomainError::new(
            ErrorKind::Auth,
            "接口返回的身份信息不完整",
        ));
    };
    Ok(LoginTokens {
        access_token: Redacted::new(access),
        refresh_token: Redacted::new(refresh),
        expires_in: token.expires_in.unwrap_or(0),
        cookies,
    })
}

// --------------------------------------------------------------- URL 查询

/// 从 URL 的查询串里取一个参数（百分号解码后）。
///
/// 自己实现而不是引入 `url` crate：这里只需要「取三个键」，而多一个依赖
/// 要跟着进五个平台的构建。
fn query_param(url: &str, key: &str) -> Option<String> {
    let query = url.split_once('?').map(|(_, q)| q)?;
    query.split('&').find_map(|pair| {
        let (k, v) = pair.split_once('=')?;
        if k != key {
            return None;
        }
        Some(
            percent_encoding::percent_decode_str(v)
                .decode_utf8_lossy()
                .into_owned(),
        )
    })
}

/// 从「要求人机验证」的 URL 里取出整组注册信息。
///
/// 三个值必须同时存在：它们是一组，拼用两次注册的字段会得到一个加载不
/// 出来的验证码（#140）。
fn registration_from_url(url: &str) -> Option<CaptchaRegistration> {
    let reg = CaptchaRegistration {
        gt: query_param(url, "gee_gt")?,
        challenge: query_param(url, "gee_challenge")?,
        token: query_param(url, "recaptcha_token")?,
    };
    reg.is_usable().then_some(reg)
}

// -------------------------------------------------------------- 请求构造

fn signed_query_post(
    path: &'static str,
    params: Vec<(String, String)>,
    env: &LoginEnv,
    idempotency: Idempotency,
) -> PreparedWrite {
    PreparedWrite {
        path,
        query: env.sign(params),
        // 基线这两条是 `Request().post(url, queryParameters: ...)`：
        // 参数在 URL 上，体是空的。
        body: WriteBody::Form(Vec::new()),
        csrf: CsrfIn::None,
        idempotency,
    }
}

fn signed_form_post(
    path: &'static str,
    params: Vec<(String, String)>,
    env: &LoginEnv,
    idempotency: Idempotency,
) -> PreparedWrite {
    PreparedWrite {
        path,
        query: Vec::new(),
        body: WriteBody::Form(env.sign(params)),
        // 登录类接口靠签名与一次性凭证鉴权，没有 csrf。
        csrf: CsrfIn::None,
        idempotency,
    }
}

/// 把人机验证的结果摊进参数表。
///
/// 基线用 `'gee_challenge': ?geeChallenge` 这种空安全展开：**值为 null
/// 的键整条不出现**。少判一次就会发出 `gee_challenge=`（空串），服务端
/// 对两者的处理未必相同。
fn push_captcha(params: &mut Vec<(String, String)>, captcha: &CaptchaSolution) {
    for (key, value) in [
        ("gee_challenge", &captcha.challenge),
        ("gee_seccode", &captcha.seccode),
        ("gee_validate", &captcha.validate),
        ("recaptcha_token", &captcha.token),
    ] {
        if let Some(v) = value {
            params.push((key.to_owned(), v.clone()));
        }
    }
}

/// 申请 TV 端二维码。
#[must_use]
pub fn qrcode_request(env: &LoginEnv) -> PreparedWrite {
    signed_query_post(
        path::TV_QRCODE,
        vec![
            ("local_id".to_owned(), "0".to_owned()),
            ("platform".to_owned(), fixed::PLATFORM.to_owned()),
            ("mobi_app".to_owned(), fixed::MOBI_APP.to_owned()),
        ],
        env,
        // 重发会换来一个新二维码，而界面手里还是旧的 auth_code——轮询
        // 会一直等一个没人会扫的码。
        Idempotency::Unsafe,
    )
}

/// 轮询扫码状态。
#[must_use]
pub fn qrcode_poll_request(env: &LoginEnv, auth_code: &str) -> PreparedWrite {
    signed_query_post(
        path::TV_QRCODE_POLL,
        vec![
            ("auth_code".to_owned(), auth_code.to_owned()),
            ("local_id".to_owned(), "0".to_owned()),
        ],
        env,
        // 纯查询，重发无副作用。
        Idempotency::Idempotent,
    )
}

/// 取密码加密用的公钥与盐。
#[must_use]
pub fn web_key_request() -> PreparedRequest {
    PreparedRequest {
        path: path::WEB_KEY,
        query: Vec::new(),
    }
}

/// 发送登录短信验证码。
#[must_use]
pub fn sms_send_request(
    env: &LoginEnv,
    cid: &str,
    tel: &str,
    captcha: &CaptchaSolution,
) -> PreparedWrite {
    let mut params = vec![
        ("build".to_owned(), fixed::BUILD.to_owned()),
        ("buvid".to_owned(), env.buvid.clone()),
        ("c_locale".to_owned(), fixed::LOCALE.to_owned()),
        ("channel".to_owned(), fixed::CHANNEL.to_owned()),
        ("cid".to_owned(), cid.to_owned()),
        ("disable_rcmd".to_owned(), fixed::DISABLE_RCMD.to_owned()),
        ("local_id".to_owned(), env.buvid.clone()),
        ("login_session_id".to_owned(), env.login_session_id()),
        ("mobi_app".to_owned(), fixed::MOBI_APP.to_owned()),
        ("platform".to_owned(), fixed::PLATFORM.to_owned()),
        ("s_locale".to_owned(), fixed::LOCALE.to_owned()),
        ("statistics".to_owned(), env.statistics.clone()),
        ("tel".to_owned(), tel.to_owned()),
        // 基线这里用的是同一个毫秒时间戳折算的秒，与签名的 ts 同源。
        ("ts".to_owned(), (env.now_millis / 1000).to_string()),
    ];
    push_captcha(&mut params, captcha);
    // 重发 = 用户再收到一条短信。
    signed_form_post(path::SMS_SEND, params, env, Idempotency::Unsafe)
}

/// 风控验证手机号前的人机验证注册。
///
/// 基线 `Request().post(Api.preCapture)` 不带任何参数，也不签名。
#[must_use]
pub const fn pre_capture_request() -> PreparedWrite {
    PreparedWrite {
        path: path::SAFE_CENTER_PRE_CAPTURE,
        query: Vec::new(),
        body: WriteBody::Form(Vec::new()),
        csrf: CsrfIn::None,
        // 重发会发一组新的 gt/challenge/token，而界面可能已经拿着上一组
        // 去加载验证码了——#140 那个空白页就是这么来的。
        idempotency: Idempotency::Unsafe,
    }
}

/// 风控验证手机号：发短信。
#[must_use]
pub fn safe_center_sms_request(
    env: &LoginEnv,
    tmp_code: &str,
    sms_type: Option<&str>,
    captcha: &CaptchaSolution,
) -> PreparedWrite {
    let mut params = vec![
        ("disable_rcmd".to_owned(), fixed::DISABLE_RCMD.to_owned()),
        (
            "sms_type".to_owned(),
            sms_type
                .unwrap_or(fixed::SMS_TYPE_LOGIN_TEL_CHECK)
                .to_owned(),
        ),
        ("tmp_code".to_owned(), tmp_code.to_owned()),
    ];
    push_captcha(&mut params, captcha);
    signed_form_post(path::SAFE_CENTER_SMS_SEND, params, env, Idempotency::Unsafe)
}

/// 风控验证手机号：提交验证码。
#[must_use]
pub fn safe_center_verify_request(
    env: &LoginEnv,
    verification: &RiskVerification,
    code: &str,
    captcha_key: &str,
    sms_type: Option<&str>,
) -> PreparedWrite {
    signed_form_post(
        path::SAFE_CENTER_SMS_VERIFY,
        vec![
            (
                "type".to_owned(),
                sms_type
                    .unwrap_or(fixed::SMS_TYPE_LOGIN_TEL_CHECK)
                    .to_owned(),
            ),
            ("code".to_owned(), code.to_owned()),
            ("tmp_code".to_owned(), verification.tmp_code.clone()),
            ("request_id".to_owned(), verification.request_id.clone()),
            ("source".to_owned(), verification.source.clone()),
            ("captcha_key".to_owned(), captcha_key.to_owned()),
        ],
        env,
        // 验证码一次性：重发第二次必然失败，还会多算一次错误尝试。
        Idempotency::Unsafe,
    )
}

/// 用 oauth code 换身份材料。
#[must_use]
pub fn oauth_token_request(env: &LoginEnv, code: &str) -> PreparedWrite {
    signed_form_post(
        path::OAUTH2_ACCESS_TOKEN,
        vec![
            ("build".to_owned(), fixed::BUILD.to_owned()),
            ("buvid".to_owned(), env.buvid.clone()),
            ("code".to_owned(), code.to_owned()),
            ("disable_rcmd".to_owned(), fixed::DISABLE_RCMD.to_owned()),
            ("grant_type".to_owned(), "authorization_code".to_owned()),
            ("local_id".to_owned(), env.buvid.clone()),
            ("mobi_app".to_owned(), fixed::MOBI_APP.to_owned()),
            ("platform".to_owned(), fixed::PLATFORM.to_owned()),
        ],
        env,
        // code 一次性。
        Idempotency::Unsafe,
    )
}

/// 登出。
///
/// csrf 的字段名是 `biliCSRF` 而不是 `csrf`——这是本仓库唯一一条这样的
/// 接口，用 [`CsrfIn::BodyNamed`] 表达，而不是在构造处把凭据拼进去。
#[must_use]
pub const fn logout_request() -> PreparedWrite {
    PreparedWrite {
        path: path::LOGOUT,
        query: Vec::new(),
        body: WriteBody::Form(Vec::new()),
        csrf: CsrfIn::BodyNamed("biliCSRF"),
        // 重发只会再登出一次，状态一致。
        idempotency: Idempotency::Idempotent,
    }
}

/// 已登录设备列表。
///
/// `csrf` 与 `access_key` 在**查询串**里且参与签名，所以不走
/// [`RequestContext::csrf`] 那条统一注入——它只处理写请求。
#[must_use]
pub fn login_devices_request(
    env: &LoginEnv,
    access_key: Option<&str>,
    csrf: &str,
) -> PreparedRequest {
    let mut params = vec![
        ("local_id".to_owned(), env.buvid.clone()),
        ("buvid".to_owned(), env.buvid.clone()),
        ("device_name".to_owned(), fixed::PLATFORM.to_owned()),
        ("device_platform".to_owned(), fixed::PLATFORM.to_owned()),
        ("csrf".to_owned(), csrf.to_owned()),
        ("mobi_app".to_owned(), fixed::MOBI_APP.to_owned()),
        ("platform".to_owned(), fixed::PLATFORM.to_owned()),
        ("statistics".to_owned(), env.statistics.clone()),
    ];
    if let Some(key) = access_key {
        params.push(("access_key".to_owned(), key.to_owned()));
    }
    PreparedRequest {
        path: path::LOGIN_DEVICES,
        query: env.sign(params),
    }
}

/// 极验配置探测。`native_ios` 对应基线的原生 SDK 分支（#140）。
#[must_use]
pub fn geetest_type_request(gt: &str, native_ios: bool) -> PreparedRequest {
    let mut query = vec![("gt".to_owned(), gt.to_owned())];
    if native_ios {
        query.push(("client_type".to_owned(), "ios".to_owned()));
        query.push(("pt".to_owned(), "20".to_owned()));
    }
    PreparedRequest {
        path: path::GEETEST_TYPE,
        query,
    }
}

// ------------------------------------------------------------------ 调用

/// 申请二维码。
///
/// # Errors
/// 传输、状态码、信封三类失败。
pub async fn fetch_qrcode<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    env: &LoginEnv,
) -> Result<QrCode, DomainError> {
    let body = crate::client::send_write(transport, ctx, &qrcode_request(env)).await?;
    let dto: QrCodeDto = from_value(split_envelope(&body)?.into_data()?)?;
    let (Some(auth_code), Some(url)) = (dto.auth_code, dto.url) else {
        return Err(DomainError::new(
            ErrorKind::Protocol,
            "二维码接口未返回 auth_code 或 url",
        ));
    };
    Ok(QrCode { auth_code, url })
}

/// 二维码失效的业务码。基线在控制器里直接写 86038。
const QR_EXPIRED: i64 = 86_038;

/// 轮询扫码状态。
///
/// 返回 `Err` 的只有传输与协议问题：**未扫、未确认、已失效都是正常状态**
/// ，由 [`QrPoll`] 表达。基线把它们压成一个 `{'status': bool}` 的 Map，
/// 于是界面要靠 `value['code'] == 86038` 再判一次。
///
/// # Errors
/// 传输、状态码、JSON 结构三类失败。
pub async fn poll_qrcode<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    env: &LoginEnv,
    auth_code: &str,
) -> Result<QrPoll, DomainError> {
    let body =
        crate::client::send_write(transport, ctx, &qrcode_poll_request(env, auth_code)).await?;
    parse_qr_poll(&body)
}

/// 解析一次轮询响应。
///
/// 单独拆出来是为了让它能被 golden fixture 钉住：三种状态的分界（尤其是
/// 86038 这个失效码）错一处，界面就会一直等一个没人会扫的二维码。
///
/// # Errors
/// JSON 结构不符，或「已确认」却没给全身份信息。
pub fn parse_qr_poll(body: &str) -> Result<QrPoll, DomainError> {
    let envelope = split_envelope(body)?;
    if envelope.code == QR_EXPIRED {
        return Ok(QrPoll::Expired);
    }
    if envelope.code != 0 {
        return Ok(QrPoll::Pending {
            code: envelope.code,
            message: envelope.message,
        });
    }
    let data = envelope
        .data
        .ok_or_else(|| DomainError::new(ErrorKind::Protocol, "扫码成功但未返回身份信息"))?;
    let dto: QrPollDto = from_value(data)?;
    let tokens = tokens_from(
        Some(TokenInfoDto {
            access_token: dto.access_token,
            refresh_token: dto.refresh_token,
            expires_in: dto.expires_in,
        }),
        dto.cookie_info,
    )?;
    Ok(QrPoll::Confirmed(Box::new(tokens)))
}

/// 取公钥与盐。**不导出**：加密材料不出 Rust（见模块文档第 4 条）。
async fn fetch_web_key<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
) -> Result<(String, String), DomainError> {
    let body = crate::client::send(transport, ctx, &web_key_request()).await?;
    let dto: WebKeyDto = from_value(split_envelope(&body)?.into_data()?)?;
    let (Some(key), Some(hash)) = (dto.key, dto.hash) else {
        return Err(DomainError::new(
            ErrorKind::Protocol,
            "公钥接口未返回 key 或 hash",
        ));
    };
    Ok((key, hash))
}

/// 需要人机验证的业务码。密码登录与短信发送共用。
const CAPTCHA_REQUIRED: i64 = -105;
/// 风控要求验证手机号时 `data.status` 的取值。
const RISK_VERIFY_REQUIRED: i64 = 2;

/// 密码登录。
///
/// 内部先取公钥再加密，调用方只给用户名与密码。`entropy` 用于 `dt` 的
/// 随机串。
///
/// # Errors
/// 传输、状态码、加密、信封四类失败；**要人机验证和要验证手机号不是
/// 错误**，见 [`PasswordLogin`]。
pub async fn login_by_password<T: HttpTransport, E: bili_protocol::risk::Entropy + ?Sized>(
    transport: &T,
    ctx: &RequestContext,
    env: &LoginEnv,
    entropy: &mut E,
    username: &str,
    password: &str,
    captcha: &CaptchaSolution,
) -> Result<PasswordLogin, DomainError> {
    let (key_pem, salt) = fetch_web_key(transport, ctx).await?;
    let key = login_crypto::parse_public_key(&key_pem)
        .map_err(|e| DomainError::new(ErrorKind::Protocol, e.to_string()))?;
    let encrypted = login_crypto::encrypt_password(&key, &salt, password)
        .map_err(|e| DomainError::new(ErrorKind::Protocol, e.to_string()))?;
    let dt = login_crypto::encrypt_dt(&key, &login_ids::random_string(entropy, 16))
        .map_err(|e| DomainError::new(ErrorKind::Protocol, e.to_string()))?;

    let mut params = vec![
        ("bili_local_id".to_owned(), env.device_id.clone()),
        ("build".to_owned(), fixed::BUILD.to_owned()),
        ("buvid".to_owned(), env.buvid.clone()),
        ("c_locale".to_owned(), fixed::LOCALE.to_owned()),
        ("channel".to_owned(), fixed::CHANNEL.to_owned()),
        ("device".to_owned(), fixed::DEVICE.to_owned()),
        ("device_id".to_owned(), env.device_id.clone()),
        ("device_name".to_owned(), fixed::DEVICE_NAME.to_owned()),
        (
            "device_platform".to_owned(),
            fixed::DEVICE_PLATFORM.to_owned(),
        ),
        ("disable_rcmd".to_owned(), fixed::DISABLE_RCMD.to_owned()),
        ("dt".to_owned(), dt),
        ("from_pv".to_owned(), fixed::PWD_FROM_PV.to_owned()),
        ("from_url".to_owned(), fixed::PWD_FROM_URL.to_owned()),
        ("local_id".to_owned(), env.buvid.clone()),
        ("mobi_app".to_owned(), fixed::MOBI_APP.to_owned()),
        ("password".to_owned(), encrypted),
        ("permission".to_owned(), fixed::PERMISSION.to_owned()),
        ("platform".to_owned(), fixed::PLATFORM.to_owned()),
        ("s_locale".to_owned(), fixed::LOCALE.to_owned()),
        ("statistics".to_owned(), env.statistics.clone()),
        ("username".to_owned(), username.to_owned()),
    ];
    push_captcha(&mut params, captcha);

    let request = signed_form_post(
        path::LOGIN_BY_PASSWORD,
        params,
        env,
        // 重发会再用掉一次人机验证结果，并多算一次密码错误尝试。
        Idempotency::Unsafe,
    );
    let body = crate::client::send_write(transport, &with_app_headers(ctx, env), &request).await?;
    parse_password_login(&body)
}

/// 解析密码登录的响应。
///
/// # Errors
/// JSON 结构不符、业务失败，或风控地址缺参数。
pub fn parse_password_login(body: &str) -> Result<PasswordLogin, DomainError> {
    let envelope = split_envelope(body)?;

    if envelope.code == CAPTCHA_REQUIRED {
        let url = envelope
            .data
            .as_ref()
            .and_then(|d| d.get("url"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned();
        return registration_from_url(&url).map_or_else(
            || {
                Err(DomainError::with_code(
                    ErrorKind::Auth,
                    "服务端要求人机验证，但未给出完整的验证参数",
                    CAPTCHA_REQUIRED,
                ))
            },
            |reg| Ok(PasswordLogin::CaptchaRequired(Box::new(reg))),
        );
    }
    let data = envelope.into_data()?;
    let dto: LoginDto = from_value(data)?;

    if dto.status == Some(RISK_VERIFY_REQUIRED) {
        let url = dto.url.unwrap_or_default();
        let (Some(tmp_code), Some(request_id), Some(source)) = (
            query_param(&url, "tmp_token"),
            query_param(&url, "request_id"),
            query_param(&url, "source"),
        ) else {
            return Err(DomainError::new(
                ErrorKind::Protocol,
                "风控验证地址缺少 tmp_token/request_id/source",
            ));
        };
        return Ok(PasswordLogin::RiskControl(Box::new(RiskVerification {
            referer_url: url,
            tmp_code,
            request_id,
            source,
            message: dto.message.unwrap_or_default(),
        })));
    }

    tokens_from(dto.token_info, dto.cookie_info).map(|t| PasswordLogin::Authenticated(Box::new(t)))
}

/// 发送登录短信验证码。
///
/// # Errors
/// 传输、状态码、信封三类失败；**要人机验证不是错误**，见 [`SmsSent`]。
pub async fn send_sms_code<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    env: &LoginEnv,
    cid: &str,
    tel: &str,
    captcha: &CaptchaSolution,
) -> Result<SmsSent, DomainError> {
    let request = sms_send_request(env, cid, tel, captcha);
    let body = crate::client::send_write(transport, &with_app_headers(ctx, env), &request).await?;
    match parse_sms_send(&body)? {
        Some(sent) => Ok(sent),
        // 响应说"要先过人机验证"却没给全那一组参数时，基线改去调
        // `preCapture` 另取一组（`sendSmsCode` 里 `!isGeeArgumentValid(...)`
        // 那条分支）。这是编排规则，换宿主时一行都不该重写，所以留在这里
        // 而不是控制器里。
        //
        // 取不到就让这次失败：拿着半组参数去加载验证码，换来的是一个永远
        // 转圈的弹窗（#140）。
        None => pre_capture(transport, ctx)
            .await
            .map(|reg| SmsSent::CaptchaRequired(Box::new(reg))),
    }
}

/// 解析短信发送的响应。
///
/// 返回 `Ok(None)` 表示"服务端要求人机验证，但这条响应里的注册参数不全"
/// ——调用方应当另取一组，见 [`send_sms_code`]。把它做成 `None` 而不是
/// 在这里发请求，是为了让解析保持纯函数、可被 golden fixture 钉住。
///
/// # Errors
/// JSON 结构不符、业务失败，或"已发送"却没给 `captcha_key`。
pub fn parse_sms_send(body: &str) -> Result<Option<SmsSent>, DomainError> {
    let envelope = split_envelope(body)?;

    // 基线判成功的条件是 `code == 0 && data.recaptcha_url == ""`：成功
    // 响应里带着一个非空的 recaptcha_url，意思是"还得先过人机验证"。
    let dto: SmsSendDto = envelope
        .data
        .clone()
        .map_or_else(|| Ok(SmsSendDto::default()), from_value)?;
    let recaptcha_url = dto.recaptcha_url.unwrap_or_default();

    if envelope.code == 0 && recaptcha_url.is_empty() {
        return dto.captcha_key.map_or_else(
            || {
                Err(DomainError::new(
                    ErrorKind::Protocol,
                    "短信已发送但未返回 captcha_key",
                ))
            },
            |captcha_key| Ok(Some(SmsSent::Sent { captcha_key })),
        );
    }

    if envelope.code == 0 || envelope.code == CAPTCHA_REQUIRED {
        return Ok(registration_from_url(&recaptcha_url)
            .map(|reg| SmsSent::CaptchaRequired(Box::new(reg))));
    }

    Err(envelope.failure())
}

/// 短信登录的四个输入。
///
/// 合成一个结构体而不是四个参数：它们必须来自**同一次**发送流程——
/// `captcha_key` 是那条短信的凭证，配上另一个手机号的验证码只会换来一个
/// 含义不明的失败。摆在一起时漏传一个会编译不过。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmsLoginInput {
    /// 国家区号 id。
    pub cid: String,
    pub tel: String,
    /// 用户填的验证码。
    pub code: String,
    /// 发送短信时服务端给的凭证。
    pub captcha_key: String,
}

/// 短信验证码登录。
///
/// `dt` 同样需要公钥，所以这里也先取一次 web key——与基线一致（基线在
/// 控制器里先调 `getWebKey`）。
///
/// # Errors
/// 传输、状态码、加密、信封四类失败。
pub async fn login_by_sms<T: HttpTransport, E: bili_protocol::risk::Entropy + ?Sized>(
    transport: &T,
    ctx: &RequestContext,
    env: &LoginEnv,
    entropy: &mut E,
    input: &SmsLoginInput,
) -> Result<LoginTokens, DomainError> {
    let SmsLoginInput {
        cid,
        tel,
        code,
        captcha_key,
    } = input;
    let (key_pem, _salt) = fetch_web_key(transport, ctx).await?;
    let key = login_crypto::parse_public_key(&key_pem)
        .map_err(|e| DomainError::new(ErrorKind::Protocol, e.to_string()))?;
    let dt = login_crypto::encrypt_dt(&key, &login_ids::random_string(entropy, 16))
        .map_err(|e| DomainError::new(ErrorKind::Protocol, e.to_string()))?;

    let params = vec![
        ("bili_local_id".to_owned(), env.device_id.clone()),
        ("build".to_owned(), fixed::BUILD.to_owned()),
        ("buvid".to_owned(), env.buvid.clone()),
        ("c_locale".to_owned(), fixed::LOCALE.to_owned()),
        ("captcha_key".to_owned(), captcha_key.to_owned()),
        ("channel".to_owned(), fixed::CHANNEL.to_owned()),
        ("cid".to_owned(), cid.to_owned()),
        ("code".to_owned(), code.to_owned()),
        ("device".to_owned(), fixed::DEVICE.to_owned()),
        ("device_id".to_owned(), env.device_id.clone()),
        ("device_name".to_owned(), fixed::DEVICE_NAME.to_owned()),
        (
            "device_platform".to_owned(),
            fixed::DEVICE_PLATFORM.to_owned(),
        ),
        ("disable_rcmd".to_owned(), fixed::DISABLE_RCMD.to_owned()),
        ("dt".to_owned(), dt),
        ("from_pv".to_owned(), fixed::SMS_FROM_PV.to_owned()),
        ("from_url".to_owned(), fixed::SMS_FROM_URL.to_owned()),
        ("local_id".to_owned(), env.buvid.clone()),
        ("mobi_app".to_owned(), fixed::MOBI_APP.to_owned()),
        ("platform".to_owned(), fixed::PLATFORM.to_owned()),
        ("s_locale".to_owned(), fixed::LOCALE.to_owned()),
        ("statistics".to_owned(), env.statistics.clone()),
        ("tel".to_owned(), tel.to_owned()),
    ];

    let request = signed_form_post(path::LOGIN_BY_SMS, params, env, Idempotency::Unsafe);
    let body = crate::client::send_write(transport, &with_app_headers(ctx, env), &request).await?;
    parse_tokens(&body)
}

/// 解析一条「直接给身份材料」的响应（短信登录、换 token）。
///
/// # Errors
/// JSON 结构不符、业务失败，或身份信息不全。
pub fn parse_tokens(body: &str) -> Result<LoginTokens, DomainError> {
    let dto: LoginDto = from_value(split_envelope(body)?.into_data()?)?;
    tokens_from(dto.token_info, dto.cookie_info)
}

/// 风控流程：取账号信息（打码的手机号/邮箱）。
///
/// # Errors
/// 传输、状态码、信封三类失败。
pub async fn fetch_safe_center_info<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    tmp_code: &str,
) -> Result<SafeCenterInfo, DomainError> {
    let request = PreparedRequest {
        path: path::SAFE_CENTER_INFO,
        query: vec![("tmp_code".to_owned(), tmp_code.to_owned())],
    };
    let body = crate::client::send(transport, ctx, &request).await?;
    let dto: SafeCenterInfoDto = from_value(split_envelope(&body)?.into_data()?)?;
    let info = dto.account_info.unwrap_or_default();
    Ok(SafeCenterInfo {
        hide_tel: info.hide_tel,
        hide_mail: info.hide_mail,
        // 缺这个字段按"不支持"处理：让流程停在一句明确的提示上，
        // 好过发一条注定被拒的短信请求。
        tel_verify: info.tel_verify.unwrap_or(false),
    })
}

/// 申请一组人机验证注册。
///
/// # Errors
/// 传输、状态码、信封三类失败；返回的三个值缺任一项即判协议错误。
pub async fn pre_capture<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
) -> Result<CaptchaRegistration, DomainError> {
    let body = crate::client::send_write(transport, ctx, &pre_capture_request()).await?;
    let dto: PreCaptureDto = from_value(split_envelope(&body)?.into_data()?)?;
    let reg = CaptchaRegistration {
        gt: dto.gee_gt.unwrap_or_default(),
        challenge: dto.gee_challenge.unwrap_or_default(),
        token: dto.recaptcha_token.unwrap_or_default(),
    };
    if reg.is_usable() {
        Ok(reg)
    } else {
        Err(DomainError::new(
            ErrorKind::Protocol,
            "人机验证接口返回的参数不完整",
        ))
    }
}

/// 风控流程：发送短信验证码。返回登录时要回填的 `captcha_key`。
///
/// # Errors
/// 传输、状态码、信封三类失败。
pub async fn send_safe_center_sms<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    env: &LoginEnv,
    verification: &RiskVerification,
    sms_type: Option<&str>,
    captcha: &CaptchaSolution,
) -> Result<String, DomainError> {
    let request = safe_center_sms_request(env, &verification.tmp_code, sms_type, captcha);
    // Referer 必须是服务端给的那个 URL，少了会被当成跨站请求拒掉。
    let ctx = with_header(ctx, "Referer", &verification.referer_url);
    let body = crate::client::send_write(transport, &ctx, &request).await?;
    let dto: CaptchaKeyDto = from_value(split_envelope(&body)?.into_data()?)?;
    dto.captcha_key
        .ok_or_else(|| DomainError::new(ErrorKind::Protocol, "短信已发送但未返回 captcha_key"))
}

/// 风控流程：提交短信验证码，换回一个 oauth code。
///
/// # Errors
/// 传输、状态码、信封三类失败。
pub async fn verify_safe_center_sms<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    env: &LoginEnv,
    verification: &RiskVerification,
    code: &str,
    captcha_key: &str,
    sms_type: Option<&str>,
) -> Result<String, DomainError> {
    let request = safe_center_verify_request(env, verification, code, captcha_key, sms_type);
    let ctx = with_header(ctx, "Referer", &verification.referer_url);
    let body = crate::client::send_write(transport, &ctx, &request).await?;
    let dto: OauthCodeDto = from_value(split_envelope(&body)?.into_data()?)?;
    dto.code
        .ok_or_else(|| DomainError::new(ErrorKind::Protocol, "验证通过但未返回 code"))
}

/// 用 oauth code 换身份材料。
///
/// # Errors
/// 传输、状态码、信封三类失败。
pub async fn exchange_oauth_code<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    env: &LoginEnv,
    code: &str,
) -> Result<LoginTokens, DomainError> {
    let request = oauth_token_request(env, code);
    let body = crate::client::send_write(transport, &with_app_headers(ctx, env), &request).await?;
    parse_tokens(&body)
}

/// 登出。
///
/// # Errors
/// 传输、状态码、信封三类失败。
pub async fn logout<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
) -> Result<(), DomainError> {
    let body = crate::client::send_write(transport, ctx, &logout_request()).await?;
    crate::envelope::check_envelope(&body)
}

/// 已登录设备列表。
///
/// # Errors
/// 传输、状态码、信封三类失败。
pub async fn fetch_login_devices<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    env: &LoginEnv,
    access_key: Option<&str>,
) -> Result<Vec<LoginDevice>, DomainError> {
    let csrf = ctx
        .csrf
        .as_ref()
        .map(|c| c.expose().clone())
        .unwrap_or_default();
    let request = login_devices_request(env, access_key, &csrf);
    let body = crate::client::send(transport, ctx, &request).await?;
    parse_login_devices(&body)
}

/// 解析已登录设备列表。
///
/// # Errors
/// JSON 结构不符或业务失败。
pub fn parse_login_devices(body: &str) -> Result<Vec<LoginDevice>, DomainError> {
    let dto: LoginDevicesDto = from_value(split_envelope(body)?.into_data()?)?;
    Ok(dto
        .devices
        .unwrap_or_default()
        .into_iter()
        .map(|d| LoginDevice {
            device_name: d.device_name,
            is_current_device: d.is_current_device.unwrap_or(false),
            latest_login_at: d.latest_login_at,
            source: d.source,
        })
        .collect())
}

/// 确认一组人机验证注册当前可用，不可用则**整组**换一次（#140）。
///
/// 基线 `resolveLoginCaptcha`（`lib/models/login/captcha_config.dart`）的
/// 逻辑，原样搬过来——它是编排规则，不是展示规则：
///
/// 1. 当前 `gt` 还能加载 → 原样用；
/// 2. 不能 → 调一次 `preCapture` 换**一整组** gt/challenge/token；
/// 3. 新的一组还是不能加载 → 失败，让界面提示换一种登录方式。
///
/// 关键在第 2 步的"整组"：`gt` 与 challenge/token 成组绑定，拿旧
/// challenge 配新 gt 会得到一个永远加载不出来的验证码——那正是 #140
/// 的现象。只换一次也是刻意的：换不出来就是极验侧的问题，循环重试只会
/// 把用户卡在一个转圈的弹窗里。
///
/// `geetest` 与 `passport` 是两个主机的上下文，由调用方分别给出。
///
/// # Errors
/// 新注册取不到、或两组都不可用 → [`ErrorKind::Auth`]。
pub async fn resolve_captcha<T: HttpTransport>(
    transport: &T,
    geetest: &RequestContext,
    passport: &RequestContext,
    current: CaptchaRegistration,
    native_ios: bool,
) -> Result<CaptchaRegistration, DomainError> {
    if !is_captcha_config_unavailable(transport, geetest, &current.gt, native_ios).await {
        return Ok(current);
    }
    let updated = pre_capture(transport, passport).await?;
    if is_captcha_config_unavailable(transport, geetest, &updated.gt, native_ios).await {
        return Err(DomainError::new(ErrorKind::Auth, "验证码服务暂时不可用"));
    }
    Ok(updated)
}

/// 极验的某个 `gt` 对应的配置是否已失效。
///
/// 响应不是 bilibili 的信封，而是一段 JSONP：`({"status":"error",...})`。
/// 判定条件照抄基线 `isMissingCaptchaConfig`。
///
/// **传输失败一律判成"没失效"**：连不上极验不能证明这次注册作废，照
/// 基线的 `catch (_) => false`。否则网络一抖就把用户推去扫码登录。
pub async fn is_captcha_config_unavailable<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    gt: &str,
    native_ios: bool,
) -> bool {
    let request = geetest_type_request(gt, native_ios);
    match crate::client::send(transport, ctx, &request).await {
        Ok(body) => is_missing_captcha_config(&body),
        Err(_) => false,
    }
}

/// 解析极验 `gettype.php` 的响应。
#[must_use]
pub fn is_missing_captcha_config(response: &str) -> bool {
    let mut data = response.trim();
    // JSONP 外壳：`(...)`。
    if data.starts_with('(') && data.ends_with(')') {
        data = &data[1..data.len() - 1];
    }
    let Ok(value) = serde_json::from_str::<serde_json::Value>(data) else {
        // 解析不出来不代表配置失效——基线的 catch 分支同样返回 false。
        return false;
    };
    value.get("status").and_then(serde_json::Value::as_str) == Some("error")
        && value.get("error_code").and_then(serde_json::Value::as_str) == Some("error_34")
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    use crate::http::{HttpGet, HttpPost, HttpResponse};
    use std::sync::Mutex;
    use std::time::Duration;

    /// 按「请求路径 → 响应体队列」回放的桩传输，并记下请求的路径顺序。
    ///
    /// resolve_captcha 的全部意义在于**调用顺序**（先查旧的、再换一组、
    /// 再查新的），而同一条路径前后两次要给不同答案，所以按路径排队取，
    /// 队列只剩一条时重复使用它。
    struct ScriptedTransport {
        responses: Mutex<Vec<(String, Vec<String>)>>,
        seen: Mutex<Vec<String>>,
    }

    impl ScriptedTransport {
        fn new(responses: &[(&str, &[&str])]) -> Self {
            Self {
                responses: Mutex::new(
                    responses
                        .iter()
                        .map(|(p, bodies)| {
                            (
                                (*p).to_owned(),
                                bodies.iter().map(|b| (*b).to_owned()).collect(),
                            )
                        })
                        .collect(),
                ),
                seen: Mutex::new(Vec::new()),
            }
        }

        fn reply(&self, url: &str) -> Result<HttpResponse, DomainError> {
            let path = url
                .split_once("://")
                .and_then(|(_, rest)| rest.split_once('/'))
                .map_or_else(String::new, |(_, p)| {
                    format!("/{}", p.split('?').next().unwrap_or(""))
                });
            self.seen.lock().expect("锁未被毒化").push(path.clone());
            let mut scripts = self.responses.lock().expect("锁未被毒化");
            let Some((_, bodies)) = scripts.iter_mut().find(|(p, _)| *p == path) else {
                return Err(DomainError::new(
                    ErrorKind::Network,
                    format!("桩件没有 {path} 的脚本"),
                ));
            };
            let body = if bodies.len() > 1 {
                bodies.remove(0)
            } else {
                bodies.first().cloned().unwrap_or_default()
            };
            Ok(HttpResponse { status: 200, body })
        }

        fn paths(&self) -> Vec<String> {
            self.seen.lock().expect("锁未被毒化").clone()
        }
    }

    impl HttpTransport for ScriptedTransport {
        async fn get(&self, request: HttpGet) -> Result<HttpResponse, DomainError> {
            self.reply(&request.url)
        }

        async fn post(&self, request: HttpPost) -> Result<HttpResponse, DomainError> {
            self.reply(&request.url)
        }
    }

    const GEETEST_OK: &str = r#"({"status":"success","data":{"type":"fullpage"}})"#;
    const PRE_CAPTURE_NEW: &str = r#"{"code":0,"data":{"gee_gt":"new-gt","gee_challenge":"new-challenge","recaptcha_token":"new-token"}}"#;
    const GEETEST_GONE: &str = r#"({"status":"error","error_code":"error_34"})"#;

    fn ctx(base: &str) -> RequestContext {
        RequestContext {
            base_url: base.to_owned(),
            cookie: None,
            csrf: None,
            headers: Vec::new(),
            timeout: Duration::from_secs(5),
        }
    }

    fn registration(tag: &str) -> CaptchaRegistration {
        CaptchaRegistration {
            gt: format!("{tag}-gt"),
            challenge: format!("{tag}-challenge"),
            token: format!("{tag}-token"),
        }
    }

    fn block_on<F: Future>(f: F) -> F::Output {
        // 这几个用例只在桩件上跑，没有定时器也没有 IO，单线程运行时足够。
        tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("构建测试运行时")
            .block_on(f)
    }

    #[test]
    fn a_usable_registration_is_not_refreshed() {
        let transport = ScriptedTransport::new(&[(path::GEETEST_TYPE, &[GEETEST_OK])]);
        let current = registration("old");
        let got = block_on(resolve_captcha(
            &transport,
            &ctx(path::GEETEST_BASE_URL),
            &ctx("https://passport.example.invalid"),
            current.clone(),
            true,
        ))
        .expect("还能用就该原样返回");
        assert_eq!(got, current);
        assert_eq!(
            transport.paths(),
            vec![path::GEETEST_TYPE.to_owned()],
            "能用就不该再去申请一组"
        );
    }

    #[test]
    fn an_expired_registration_is_replaced_as_a_whole() {
        // 旧 gt 已失效 → 换一组 → 新 gt 可用。三个字段必须**整组**换掉：
        // 新 gt 配旧 challenge 会得到一个永远加载不出来的验证码（#140）。
        let transport = ScriptedTransport::new(&[
            (path::GEETEST_TYPE, &[GEETEST_GONE, GEETEST_OK]),
            (path::SAFE_CENTER_PRE_CAPTURE, &[PRE_CAPTURE_NEW]),
        ]);
        let got = block_on(resolve_captcha(
            &transport,
            &ctx(path::GEETEST_BASE_URL),
            &ctx("https://passport.example.invalid"),
            registration("old"),
            true,
        ))
        .expect("换到的新注册可用");
        assert_eq!(got, registration("new"));
        assert_eq!(
            transport.paths(),
            vec![
                path::GEETEST_TYPE.to_owned(),
                path::SAFE_CENTER_PRE_CAPTURE.to_owned(),
                path::GEETEST_TYPE.to_owned(),
            ]
        );
    }

    #[test]
    fn a_fresh_registration_that_is_also_gone_fails_instead_of_looping() {
        let transport = ScriptedTransport::new(&[
            (path::GEETEST_TYPE, &[GEETEST_GONE]),
            (path::SAFE_CENTER_PRE_CAPTURE, &[PRE_CAPTURE_NEW]),
        ]);
        let err = block_on(resolve_captcha(
            &transport,
            &ctx(path::GEETEST_BASE_URL),
            &ctx("https://passport.example.invalid"),
            registration("old"),
            true,
        ))
        .expect_err("两组都不可用必须失败");
        assert_eq!(err.kind(), ErrorKind::Auth);
        // 只换一次：查旧、换新、查新，到此为止。
        assert_eq!(
            transport.paths(),
            vec![
                path::GEETEST_TYPE.to_owned(),
                path::SAFE_CENTER_PRE_CAPTURE.to_owned(),
                path::GEETEST_TYPE.to_owned(),
            ]
        );
    }

    #[test]
    fn the_old_registration_is_not_reused_when_the_refresh_fails() {
        let transport = ScriptedTransport::new(&[
            (path::GEETEST_TYPE, &[GEETEST_GONE]),
            (
                path::SAFE_CENTER_PRE_CAPTURE,
                &[r#"{"code":-500,"message":"服务端错误"}"#],
            ),
        ]);
        let err = block_on(resolve_captcha(
            &transport,
            &ctx(path::GEETEST_BASE_URL),
            &ctx("https://passport.example.invalid"),
            registration("old"),
            true,
        ))
        .expect_err("换不到新的就不能接着用旧的");
        assert_eq!(err.code(), Some(-500));
    }

    #[test]
    fn a_transport_failure_does_not_condemn_the_registration() {
        // 连不上极验不能证明注册作废，否则网络一抖就把用户推去扫码登录。
        let transport = ScriptedTransport::new(&[]);
        let current = registration("old");
        let got = block_on(resolve_captcha(
            &transport,
            &ctx(path::GEETEST_BASE_URL),
            &ctx("https://passport.example.invalid"),
            current.clone(),
            false,
        ))
        .expect("探测失败应按仍可用处理");
        assert_eq!(got, current);
    }

    #[test]
    fn sms_send_falls_back_to_pre_capture_when_the_url_is_incomplete() {
        // 基线 `sendSmsCode` 的 `if (!isGeeArgumentValid(...))` 分支。
        let transport = ScriptedTransport::new(&[
            (
                path::SMS_SEND,
                &[
                    r#"{"code":-105,"message":"需要验证","data":{"recaptcha_url":"https://x?gee_gt=GT"}}"#,
                ],
            ),
            (path::SAFE_CENTER_PRE_CAPTURE, &[PRE_CAPTURE_NEW]),
        ]);
        let got = block_on(send_sms_code(
            &transport,
            &ctx("https://passport.example.invalid"),
            &env(),
            "86",
            "13800000000",
            &CaptchaSolution::default(),
        ))
        .expect("半组参数时应另取一组");
        match got {
            SmsSent::CaptchaRequired(reg) => assert_eq!(*reg, registration("new")),
            SmsSent::Sent { .. } => panic!("这条响应不该判成已发送"),
        }
    }

    /// 一把 1024 位的公钥，与 `/x/passport-login/web/key` 当前下发的同样
    /// 长度、同样形状（SPKI、base64 以 `MIGf` 开头）。本地生成的，不是
    /// 线上那把——这里验的是长度与格式能不能走通整条链路。
    const WEB_KEY_1024: &str = r#"{"code":0,"data":{"hash":"salt-","key":"-----BEGIN PUBLIC KEY-----\nMIGfMA0GCSqGSIb3DQEBAQUAA4GNADCBiQKBgQDkaUc5IwwPGT1fVNsjN5wyUFQr\nMjjbYq1GoUnRMJydqCA3EBPWJmFvqxxJJTDceyA4pvmPQbEcgw7fGlx8kw8+lI0W\nu8E6hZi148hvWXGmoaLAoJDBFWf0WQXS8Kr2h26cKCIWbMbsp4neqJGa5eHeWhQ/\nLIgsbG8EiDbt8qLjeQIDAQAB\n-----END PUBLIC KEY-----\n"}}"#;

    /// 短信登录成功的最小响应：身份信息两半都在。
    const SMS_LOGIN_OK: &str = r#"{"code":0,"data":{"token_info":{"access_token":"at","refresh_token":"rt","expires_in":100},"cookie_info":{"cookies":[{"name":"SESSDATA","value":"v"}]}}}"#;

    struct FixedEntropy;

    impl bili_protocol::risk::Entropy for FixedEntropy {
        fn next_below(&mut self, _bound: u32) -> u32 {
            0
        }
    }

    /// issue #161 的回归：短信登录要先取公钥给 `dt` 加密，而服务端发的是
    /// **1024 位**公钥。#156 曾把可接受的最小模数定成 2048 位，于是这条
    /// 链路在取到真实公钥之后一律倒在「登录公钥无法解析」上，密码登录同理。
    ///
    /// 这个用例从公钥接口一路跑到登录接口，长度判断错了就过不去——
    /// `login_crypto` 那边只验单个函数，拦不住「阈值与线上不符」这类错。
    #[test]
    fn sms_login_works_with_the_1024_bit_key_the_server_actually_sends() {
        let transport = ScriptedTransport::new(&[
            (path::WEB_KEY, &[WEB_KEY_1024]),
            (path::LOGIN_BY_SMS, &[SMS_LOGIN_OK]),
        ]);
        let input = SmsLoginInput {
            cid: "86".to_owned(),
            tel: "13800000000".to_owned(),
            code: "123456".to_owned(),
            captcha_key: "ck".to_owned(),
        };
        let tokens = block_on(login_by_sms(
            &transport,
            &ctx("https://passport.example.invalid"),
            &env(),
            &mut FixedEntropy,
            &input,
        ))
        .expect("1024 位公钥必须走得通");

        assert_eq!(tokens.expires_in, 100);
        assert_eq!(
            transport.paths(),
            vec![path::WEB_KEY.to_owned(), path::LOGIN_BY_SMS.to_owned()],
            "先取公钥再登录，顺序与基线一致"
        );
    }

    /// 密码登录与短信登录共用同一次公钥解析，一并钉住。
    #[test]
    fn password_login_works_with_the_1024_bit_key_too() {
        let transport = ScriptedTransport::new(&[
            (path::WEB_KEY, &[WEB_KEY_1024]),
            (path::LOGIN_BY_PASSWORD, &[SMS_LOGIN_OK]),
        ]);
        let got = block_on(login_by_password(
            &transport,
            &ctx("https://passport.example.invalid"),
            &env(),
            &mut FixedEntropy,
            "user",
            "hunter2",
            &CaptchaSolution::default(),
        ))
        .expect("1024 位公钥必须走得通");
        assert!(
            matches!(got, PasswordLogin::Authenticated(_)),
            "两半身份信息都在，应判成功"
        );
    }

    fn env() -> LoginEnv {
        LoginEnv {
            appkey: "dfca71928277209b".to_owned(),
            appsec: "b5475a8825547a4fc26c7d518eaaa02e".to_owned(),
            buvid: "XY0123456789ABCDEF".to_owned(),
            device_id: "deadbeef".to_owned(),
            user_agent: "UA".to_owned(),
            statistics: r#"{"appId":5}"#.to_owned(),
            trace_id: "trace".to_owned(),
            ts: 1_700_000_000,
            now_millis: 1_700_000_000_123,
        }
    }

    fn form_of(w: &PreparedWrite) -> Vec<(String, String)> {
        match &w.body {
            WriteBody::Form(f) => f.clone(),
            WriteBody::Json(_) => panic!("登录接口一律用表单"),
        }
    }

    fn get<'a>(params: &'a [(String, String)], key: &str) -> Option<&'a str> {
        params
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    #[test]
    fn qrcode_params_go_on_the_query_string_not_the_body() {
        // 基线是 `post(url, queryParameters: params)`，体是空的。
        let req = qrcode_request(&env());
        assert!(form_of(&req).is_empty());
        assert_eq!(get(&req.query, "local_id"), Some("0"));
        assert!(get(&req.query, "sign").is_some(), "必须签名");
    }

    #[test]
    fn a_missing_captcha_field_is_dropped_entirely() {
        // 基线的 `'gee_challenge': ?geeChallenge` 在值为 null 时整条不出现。
        let solution = CaptchaSolution {
            challenge: Some("ch".to_owned()),
            ..CaptchaSolution::default()
        };
        let req = sms_send_request(&env(), "86", "13800000000", &solution);
        let form = form_of(&req);
        assert_eq!(get(&form, "gee_challenge"), Some("ch"));
        assert!(
            get(&form, "gee_seccode").is_none(),
            "没有的字段不能以空串出现"
        );
    }

    #[test]
    fn sms_send_is_never_auto_retried() {
        let req = sms_send_request(&env(), "86", "13800000000", &CaptchaSolution::default());
        assert_eq!(req.idempotency, Idempotency::Unsafe, "重发等于再发一条短信");
    }

    #[test]
    fn logout_puts_the_token_under_its_own_field_name() {
        // 这条接口的字段名是 biliCSRF，不是 csrf。
        assert!(matches!(
            logout_request().csrf,
            CsrfIn::BodyNamed("biliCSRF")
        ));
    }

    #[test]
    fn login_session_id_is_derived_from_buvid_and_now() {
        let e = env();
        let form = form_of(&sms_send_request(
            &e,
            "86",
            "1",
            &CaptchaSolution::default(),
        ));
        assert_eq!(
            get(&form, "login_session_id"),
            Some(login_ids::login_session_id(&e.buvid, e.now_millis).as_str())
        );
        // ts 用的是同一个毫秒时间戳折算的秒。
        assert_eq!(get(&form, "ts"), Some("1700000000"));
    }

    #[test]
    fn registration_fields_must_come_from_one_url() {
        let url = "https://passport.bilibili.com/x?gee_gt=GT&gee_challenge=CH&recaptcha_token=TK";
        let reg = registration_from_url(url).expect("三项齐全");
        assert_eq!(reg.gt, "GT");
        assert_eq!(reg.challenge, "CH");
        assert_eq!(reg.token, "TK");
        // 少一项就整组作废，不能拿半组去加载验证码（#140）。
        assert!(registration_from_url("https://x?gee_gt=GT&gee_challenge=CH").is_none());
    }

    #[test]
    fn query_params_are_percent_decoded() {
        let url = "https://x?source=risk%2Flogin&tmp_token=abc";
        assert_eq!(query_param(url, "source").as_deref(), Some("risk/login"));
    }

    #[test]
    fn geetest_jsonp_wrapper_is_stripped() {
        assert!(is_missing_captcha_config(
            r#"({"status":"error","error_code":"error_34"})"#
        ));
        assert!(is_missing_captcha_config(
            r#"{"status":"error","error_code":"error_34"}"#
        ));
        assert!(!is_missing_captcha_config(r#"({"status":"success"})"#));
        // 不是 JSON 时不能当成"已失效"，否则网络页面一返回 HTML 就把
        // 用户推去扫码登录。
        assert!(!is_missing_captcha_config("<html>502</html>"));
    }

    #[test]
    fn ios_native_check_adds_its_own_parameters() {
        assert_eq!(geetest_type_request("gt", false).query.len(), 1);
        let ios = geetest_type_request("gt", true);
        assert_eq!(get(&ios.query, "client_type"), Some("ios"));
        assert_eq!(get(&ios.query, "pt"), Some("20"));
    }

    #[test]
    fn devices_request_drops_access_key_when_absent() {
        let e = env();
        let with = login_devices_request(&e, Some("ak"), "csrf-value");
        assert_eq!(get(&with.query, "access_key"), Some("ak"));
        let without = login_devices_request(&e, None, "csrf-value");
        assert!(get(&without.query, "access_key").is_none());
        // csrf 在查询串里且参与签名：它不是 send_write 那条统一注入。
        assert_eq!(get(&without.query, "csrf"), Some("csrf-value"));
    }

    #[test]
    fn app_headers_override_the_contexts_own() {
        let ctx = RequestContext {
            base_url: "https://passport.bilibili.com".to_owned(),
            cookie: None,
            csrf: None,
            headers: vec![("user-agent".to_owned(), "Dart/3.6".to_owned())],
            timeout: std::time::Duration::from_secs(10),
        };
        let merged = with_app_headers(&ctx, &env());
        let uas: Vec<&String> = merged
            .headers
            .iter()
            .filter(|(k, _)| k.eq_ignore_ascii_case("user-agent"))
            .map(|(_, v)| v)
            .collect();
        assert_eq!(uas.len(), 1, "同名头只能剩一个，否则 reqwest 会两条都发");
        assert_eq!(uas[0], "UA");
    }

    #[test]
    fn qr_poll_states_are_not_errors() {
        // 未扫码：code 非 0 但不是失效，界面继续轮询。
        let body = r#"{"code":86039,"message":"二维码尚未确认"}"#;
        let envelope = split_envelope(body).expect("信封");
        assert_eq!(envelope.code, 86_039);
    }

    /// #166：`expires_in` 全 `lib/` 没有消费者，它的写法不该拦住登录。
    ///
    /// 收紧成只认数字的话，服务端哪天把它发成 `"2592000"`，用户就直接登不
    /// 进去——而那个值本来就没人读。
    #[test]
    fn a_string_expires_in_does_not_block_the_login() {
        let poll = parse_qr_poll(
            r#"{"code":0,"message":"0","data":{"access_token":"at","refresh_token":"rt","expires_in":"2592000","cookie_info":{"cookies":[{"name":"SESSDATA","value":"v"}]}}}"#,
        )
        .expect("这个字段的写法不该让整条扫码响应作废");
        let QrPoll::Confirmed(tokens) = poll else {
            panic!("确认后应拿到身份");
        };
        assert_eq!(tokens.expires_in, 2_592_000);

        // 解不出来的形状落到 0，登录照样走得完。
        let poll = parse_qr_poll(
            r#"{"code":0,"message":"0","data":{"access_token":"at","refresh_token":"rt","expires_in":"--","cookie_info":{"cookies":[{"name":"SESSDATA","value":"v"}]}}}"#,
        )
        .expect("同上");
        let QrPoll::Confirmed(tokens) = poll else {
            panic!("有身份");
        };
        assert_eq!(tokens.expires_in, 0);
    }

    #[test]
    fn tokens_require_both_halves_of_the_identity() {
        let cookie = Some(CookieInfoDto {
            cookies: Some(vec![CookieDto {
                name: Some("SESSDATA".to_owned()),
                value: Some("v".to_owned()),
            }]),
        });
        // 只有 cookie 没有 token：基线提示"接口未返回身份信息"。
        let err = tokens_from(None, cookie.clone()).expect_err("缺 token_info 必须失败");
        assert_eq!(err.kind(), ErrorKind::Auth);
        // 只有 token 没有 cookie 同样不成立。
        let err = tokens_from(
            Some(TokenInfoDto {
                access_token: Some("a".to_owned()),
                refresh_token: Some("r".to_owned()),
                expires_in: Some(1),
            }),
            None,
        )
        .expect_err("缺 cookie 必须失败");
        assert_eq!(err.kind(), ErrorKind::Auth);
    }

    #[test]
    fn half_a_cookie_is_dropped_rather_than_stored() {
        let cookies = cookies_from(Some(CookieInfoDto {
            cookies: Some(vec![
                CookieDto {
                    name: Some("SESSDATA".to_owned()),
                    value: None,
                },
                CookieDto {
                    name: Some("bili_jct".to_owned()),
                    value: Some("ok".to_owned()),
                },
            ]),
        }));
        assert_eq!(cookies.len(), 1);
        assert_eq!(cookies[0].name, "bili_jct");
    }
}
