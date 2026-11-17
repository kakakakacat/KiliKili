//! 登录流程的 typed FRB 导出（S14 / #65）。
//!
//! ## 这一层只做形状转换
//!
//! 判断"登录成没成、要不要验证码、二维码过没过期"全在
//! [`bili_api::login`]；这里把领域类型摊成 FRB 能生成绑定的结构，不复述
//! 任何规则。Dart 侧因此只剩"把返回的 cookie 存进账号体系"这一件事。
//!
//! ## 凭据怎么过这条边
//!
//! access_token / refresh_token / cookie 在领域层是 `Redacted<String>`，
//! 到这里必须 `expose()` 才能跨 FFI——**这是本仓库唯一一处刻意把凭据
//! 解包的地方**，因为它们正是要交给 Dart 存起来的东西（见
//! `bili_api::login` 的模块文档）。解包点集中在 [`tokens_to_dart`] 一个
//! 函数里，便于 review；其余结构一律不含凭据。
//!
//! 明文密码只作为 [`login_by_password`] 的参数出现，不进任何结构体，
//! 因此不会被 FRB 生成的 `Debug`/日志打到。
//!
//! ## `Redacted` 的保证到此为止
//!
//! 本模块的 `LoginTokensData`、`QrPollState`、`PasswordLoginState` 等导出
//! 结构里装的是**明文**凭据，并且为了 FRB 派生了 `Debug`。这是设计使然，
//! 不是疏漏：跨 FFI 只能给明文。但也因此，领域层那层"打印即 `<redacted>`"
//! 的保护在这条边之后就没有了。
//!
//! **这些类型不得进入任何日志宏**——`tracing::debug!(?state)`、
//! `dbg!`、`println!` 一概不行。写下这条是为了让将来某次"顺手加条日志"
//! 能被认出是回归，而不是被当成新功能（issue #156）。

use std::fmt;
use std::sync::OnceLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use bili_api::http::RequestContext;
use bili_api::login::{self, LoginEnv, SmsLoginInput};
use bili_api::transport_reqwest::ReqwestTransport;
use bili_domain::error::{DomainError, ErrorKind};
use bili_domain::login::{
    CaptchaRegistration, CaptchaSolution, LoginTokens, PasswordLogin, QrPoll, RiskVerification,
    SmsSent,
};
use bili_domain::redact::Redacted;
use bili_protocol::login_ids;
use bili_protocol::risk::SystemEntropy;

use crate::api::{init, transport::TransportSettings, write::WriteResult};

/// 一次登录调用的上下文。
///
/// 前半截与其余切片的写上下文一致（主机、Cookie、csrf、头、超时、传输
/// 设置），后半截是登录接口特有的客户端标识。两者合成一个结构体是因为
/// 登录这一片的每条接口都要用到后半截。
#[derive(Clone)]
pub struct LoginContext {
    pub base_url: String,
    pub cookie: Option<String>,
    pub csrf: Option<String>,
    pub headers: Vec<(String, String)>,
    pub timeout_ms: u64,
    pub transport: TransportSettings,
    pub appkey: String,
    pub appsec: String,
    pub buvid: String,
    pub user_agent: String,
    pub statistics: String,
    pub trace_id: String,
    /// 本机时区相对 UTC 的秒数。设备号里嵌的是**本地**时间，宿主才知道
    /// 时区（见 `bili_protocol::login_ids`）。
    pub utc_offset_secs: i32,
}

impl fmt::Debug for LoginContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LoginContext")
            .field("base_url", &self.base_url)
            .field("credentials", &"<redacted>")
            .field("timeout_ms", &self.timeout_ms)
            .finish_non_exhaustive()
    }
}

/// 进程级设备号。
///
/// 基线是 `static final String deviceId = LoginUtils.genDeviceId();`——
/// 一次启动里所有登录请求共用一个值。每请求换一个等于每次都是一台新
/// 设备，那是风控最在意的形状，所以这里同样只生成一次。
///
/// 它不是凭据（不能用来代用户做任何事），但仍是设备指纹，因此不写日志、
/// 不持久化：进程重启换一个，与基线一致。
static DEVICE_ID: OnceLock<String> = OnceLock::new();

fn device_id(utc_offset_secs: i32) -> String {
    DEVICE_ID
        .get_or_init(|| {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |d| d.as_secs() as i64);
            let mut entropy = SystemEntropy;
            login_ids::device_id(&mut entropy, now, utc_offset_secs)
        })
        .clone()
}

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

impl LoginContext {
    fn split(self) -> (RequestContext, LoginEnv, ReqwestTransport) {
        let millis = now_millis();
        let env = LoginEnv {
            appkey: self.appkey,
            appsec: self.appsec,
            buvid: self.buvid,
            device_id: device_id(self.utc_offset_secs),
            user_agent: self.user_agent,
            statistics: self.statistics,
            trace_id: self.trace_id,
            ts: millis / 1000,
            now_millis: millis,
        };
        (
            RequestContext {
                base_url: self.base_url,
                cookie: self.cookie.map(Redacted::new),
                csrf: self.csrf.map(Redacted::new),
                headers: self.headers,
                timeout: Duration::from_millis(self.timeout_ms),
            },
            env,
            ReqwestTransport::new(self.transport.to_config()),
        )
    }

    /// 极验主机的上下文：同一套传输设置与超时，但**不带任何凭据**。
    ///
    /// 第三方主机上出现 bilibili 的 Cookie 是一次泄漏，不是一个 bug——
    /// 基线对这条请求显式用 `NoAccount`，这里对应地把 cookie/csrf 清空。
    fn geetest_context(&self) -> RequestContext {
        RequestContext {
            base_url: login::path::GEETEST_BASE_URL.to_owned(),
            cookie: None,
            csrf: None,
            headers: vec![("user-agent".to_owned(), self.user_agent.clone())],
            timeout: Duration::from_millis(self.timeout_ms),
        }
    }
}

// ------------------------------------------------------------------ 结果

/// 登录成功后交给 Dart 存的身份材料。
#[derive(Debug, Clone)]
pub struct LoginTokensData {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: i64,
    pub cookies: Vec<LoginCookieData>,
}

#[derive(Debug, Clone)]
pub struct LoginCookieData {
    pub name: String,
    pub value: String,
}

/// **唯一**把凭据从 `Redacted` 里取出来的地方。
fn tokens_to_dart(tokens: LoginTokens) -> LoginTokensData {
    LoginTokensData {
        access_token: tokens.access_token.into_inner(),
        refresh_token: tokens.refresh_token.into_inner(),
        expires_in: tokens.expires_in,
        cookies: tokens
            .cookies
            .into_iter()
            .map(|c| LoginCookieData {
                name: c.name,
                value: c.value.into_inner(),
            })
            .collect(),
    }
}

#[derive(Debug, Clone)]
pub struct CaptchaRegistrationData {
    pub gt: String,
    pub challenge: String,
    pub token: String,
}

impl From<CaptchaRegistration> for CaptchaRegistrationData {
    fn from(r: CaptchaRegistration) -> Self {
        Self {
            gt: r.gt,
            challenge: r.challenge,
            token: r.token,
        }
    }
}

/// 过了人机验证之后由界面回填的那组值。
#[derive(Debug, Clone, Default)]
pub struct CaptchaSolutionData {
    pub challenge: Option<String>,
    pub seccode: Option<String>,
    pub validate: Option<String>,
    pub token: Option<String>,
}

impl CaptchaSolutionData {
    fn to_domain(&self) -> CaptchaSolution {
        CaptchaSolution {
            challenge: self.challenge.clone(),
            seccode: self.seccode.clone(),
            validate: self.validate.clone(),
            token: self.token.clone(),
        }
    }
}

/// 风控验证手机号这条支线的参数。原样回传给后续三条接口。
#[derive(Debug, Clone)]
pub struct RiskVerificationData {
    pub referer_url: String,
    pub tmp_code: String,
    pub request_id: String,
    pub source: String,
    pub message: String,
}

impl RiskVerificationData {
    fn to_domain(&self) -> RiskVerification {
        RiskVerification {
            referer_url: self.referer_url.clone(),
            tmp_code: self.tmp_code.clone(),
            request_id: self.request_id.clone(),
            source: self.source.clone(),
            message: self.message.clone(),
        }
    }
}

impl From<RiskVerification> for RiskVerificationData {
    fn from(v: RiskVerification) -> Self {
        Self {
            referer_url: v.referer_url,
            tmp_code: v.tmp_code,
            request_id: v.request_id,
            source: v.source,
            message: v.message,
        }
    }
}

#[derive(Debug, Clone)]
pub struct QrCodeData {
    pub auth_code: String,
    pub url: String,
}

/// 扫码轮询的三种状态。
///
/// ## 为什么是「判别枚举 + 可空载荷」而不是带载荷的枚举
///
/// FRB 能把带载荷的 Rust 枚举生成 Dart 的 sealed 类，但那要求项目引入
/// `freezed`（codegen 当场报 `MissingDep`）。为一条切片给整个 Flutter
/// 工程加一套代码生成，代价远大于这里多出的几个可空字段，所以统一用
/// 不带载荷的枚举做判别、载荷放在同一结构体的可空字段里。
///
/// 即便如此也比基线强：基线把三种状态压进 `Map<String, dynamic>`，界面
/// 要先看 `status` 再看 `code`，86038（二维码过期）只在一处被判到。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QrPollStatus {
    /// 已确认，`tokens` 必有值。
    Confirmed,
    /// 等待扫码或确认，`code` 与 `message` 必有值。
    Pending,
    /// 二维码已失效，必须重新申请。
    Expired,
}

/// 轮询结果的载荷。字段按 [`QrPollStatus`] 取用。
#[derive(Debug, Clone)]
pub struct QrPollState {
    pub status: QrPollStatus,
    pub tokens: Option<LoginTokensData>,
    pub code: Option<i64>,
    pub message: Option<String>,
}

/// 密码登录的三种结论。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasswordLoginStatus {
    /// 登录成功，`tokens` 必有值。
    Authenticated,
    /// 风控要求先验证手机号，`risk` 必有值。
    RiskControl,
    /// 要求先过人机验证，`captcha` 必有值；验证后用同样的参数重登一次。
    CaptchaRequired,
}

#[derive(Debug, Clone)]
pub struct PasswordLoginState {
    pub status: PasswordLoginStatus,
    pub tokens: Option<LoginTokensData>,
    pub risk: Option<RiskVerificationData>,
    pub captcha: Option<CaptchaRegistrationData>,
}

/// 发短信的两种结论。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmsSendStatus {
    /// 已发出，`captcha_key` 必有值。
    Sent,
    /// 要求先过人机验证，`captcha` 必有值。
    CaptchaRequired,
}

#[derive(Debug, Clone)]
pub struct SmsSendState {
    pub status: SmsSendStatus,
    pub captcha_key: Option<String>,
    pub captcha: Option<CaptchaRegistrationData>,
}

#[derive(Debug, Clone)]
pub struct SafeCenterInfoData {
    pub hide_tel: Option<String>,
    pub hide_mail: Option<String>,
    pub tel_verify: bool,
}

#[derive(Debug, Clone)]
pub struct LoginDeviceData {
    pub device_name: Option<String>,
    pub is_current_device: bool,
    pub latest_login_at: Option<String>,
    pub source: Option<String>,
}

// 每条导出各自的结果类型。形状与其余切片一致：`data` 为空且
// `error_kind` 为空不会同时出现（ADR-011）。
macro_rules! login_result {
    ($name:ident, $data:ty, $doc:literal) => {
        #[doc = $doc]
        #[derive(Debug, Clone)]
        pub struct $name {
            pub data: Option<$data>,
            pub error_kind: Option<String>,
            pub error_detail: Option<String>,
            pub error_code: Option<i64>,
        }

        impl $name {
            fn from_result(r: Result<$data, DomainError>) -> Self {
                match r {
                    Ok(data) => Self {
                        data: Some(data),
                        error_kind: None,
                        error_detail: None,
                        error_code: None,
                    },
                    Err(e) => Self {
                        data: None,
                        error_kind: Some(e.kind().to_string()),
                        error_detail: Some(e.detail().to_owned()),
                        error_code: e.code(),
                    },
                }
            }
        }
    };
}

login_result!(QrCodeResult, QrCodeData, "申请二维码的结果。");
login_result!(QrPollResult, QrPollState, "轮询扫码状态的结果。");
login_result!(PasswordLoginResult, PasswordLoginState, "密码登录的结果。");
login_result!(SmsSendResult, SmsSendState, "发送短信验证码的结果。");
login_result!(
    LoginTokensResult,
    LoginTokensData,
    "直接拿到身份材料的结果（短信登录、换 token）。"
);
login_result!(
    SafeCenterInfoResult,
    SafeCenterInfoData,
    "风控流程的账号信息。"
);
login_result!(
    CaptchaRegistrationResult,
    CaptchaRegistrationData,
    "一组人机验证注册。"
);
login_result!(CaptchaKeyResult, String, "风控短信发出后的 captcha_key。");
login_result!(OauthCodeResult, String, "风控验证通过后换回的一次性 code。");
login_result!(LoginDevicesResult, Vec<LoginDeviceData>, "已登录设备列表。");

/// 把一次登录调用放到 Rust 运行时上跑。
///
/// 每条导出都这么收尾：工作任务被取消（运行时关停）要变成一个错误而不是
/// panic——登录路径上的 panic 会把整个 app 带走。
macro_rules! spawn_login {
    ($what:literal, $body:expr) => {
        init::runtime()
            .spawn($body)
            .await
            .unwrap_or_else(|_| Err(DomainError::new(ErrorKind::Protocol, $what)))
    };
}

// ------------------------------------------------------------------ 导出

/// 申请扫码登录的二维码。
pub async fn login_qrcode(context: LoginContext) -> QrCodeResult {
    let (ctx, env, transport) = context.split();
    let result = spawn_login!("二维码工作任务未完成", async move {
        login::fetch_qrcode(&transport, &ctx, &env).await
    });
    QrCodeResult::from_result(result.map(|qr| QrCodeData {
        auth_code: qr.auth_code,
        url: qr.url,
    }))
}

/// 轮询扫码状态。
pub async fn login_qrcode_poll(context: LoginContext, auth_code: String) -> QrPollResult {
    let (ctx, env, transport) = context.split();
    let result = spawn_login!("扫码轮询工作任务未完成", async move {
        login::poll_qrcode(&transport, &ctx, &env, &auth_code).await
    });
    QrPollResult::from_result(result.map(|poll| match poll {
        QrPoll::Confirmed(tokens) => QrPollState {
            status: QrPollStatus::Confirmed,
            tokens: Some(tokens_to_dart(*tokens)),
            code: None,
            message: None,
        },
        QrPoll::Pending { code, message } => QrPollState {
            status: QrPollStatus::Pending,
            tokens: None,
            code: Some(code),
            message: Some(message),
        },
        QrPoll::Expired => QrPollState {
            status: QrPollStatus::Expired,
            tokens: None,
            code: None,
            message: None,
        },
    }))
}

/// 密码登录。
///
/// `password` 是明文，仅在本次调用内存在：它立刻被 RSA 加密（公钥由
/// Rust 自己去取，见 `bili_api::login`），不进任何结构体。
pub async fn login_by_password(
    context: LoginContext,
    username: String,
    password: String,
    captcha: CaptchaSolutionData,
) -> PasswordLoginResult {
    let (ctx, env, transport) = context.split();
    let solution = captcha.to_domain();
    let result = spawn_login!("密码登录工作任务未完成", async move {
        let mut entropy = SystemEntropy;
        login::login_by_password(
            &transport,
            &ctx,
            &env,
            &mut entropy,
            &username,
            &password,
            &solution,
        )
        .await
    });
    PasswordLoginResult::from_result(result.map(|state| match state {
        PasswordLogin::Authenticated(tokens) => PasswordLoginState {
            status: PasswordLoginStatus::Authenticated,
            tokens: Some(tokens_to_dart(*tokens)),
            risk: None,
            captcha: None,
        },
        PasswordLogin::RiskControl(v) => PasswordLoginState {
            status: PasswordLoginStatus::RiskControl,
            tokens: None,
            risk: Some((*v).into()),
            captcha: None,
        },
        PasswordLogin::CaptchaRequired(r) => PasswordLoginState {
            status: PasswordLoginStatus::CaptchaRequired,
            tokens: None,
            risk: None,
            captcha: Some((*r).into()),
        },
    }))
}

/// 发送登录短信验证码。
pub async fn login_send_sms_code(
    context: LoginContext,
    cid: String,
    tel: String,
    captcha: CaptchaSolutionData,
) -> SmsSendResult {
    let (ctx, env, transport) = context.split();
    let solution = captcha.to_domain();
    let result = spawn_login!("短信发送工作任务未完成", async move {
        login::send_sms_code(&transport, &ctx, &env, &cid, &tel, &solution).await
    });
    SmsSendResult::from_result(result.map(|sent| match sent {
        SmsSent::Sent { captcha_key } => SmsSendState {
            status: SmsSendStatus::Sent,
            captcha_key: Some(captcha_key),
            captcha: None,
        },
        SmsSent::CaptchaRequired(r) => SmsSendState {
            status: SmsSendStatus::CaptchaRequired,
            captcha_key: None,
            captcha: Some((*r).into()),
        },
    }))
}

/// 短信验证码登录。
pub async fn login_by_sms(
    context: LoginContext,
    cid: String,
    tel: String,
    code: String,
    captcha_key: String,
) -> LoginTokensResult {
    let (ctx, env, transport) = context.split();
    let result = spawn_login!("短信登录工作任务未完成", async move {
        let mut entropy = SystemEntropy;
        login::login_by_sms(
            &transport,
            &ctx,
            &env,
            &mut entropy,
            &SmsLoginInput {
                cid,
                tel,
                code,
                captcha_key,
            },
        )
        .await
    });
    LoginTokensResult::from_result(result.map(tokens_to_dart))
}

/// 风控流程：取打码后的手机号/邮箱。
pub async fn login_safe_center_info(
    context: LoginContext,
    tmp_code: String,
) -> SafeCenterInfoResult {
    let (ctx, _env, transport) = context.split();
    let result = spawn_login!("安全中心信息工作任务未完成", async move {
        login::fetch_safe_center_info(&transport, &ctx, &tmp_code).await
    });
    SafeCenterInfoResult::from_result(result.map(|info| SafeCenterInfoData {
        hide_tel: info.hide_tel,
        hide_mail: info.hide_mail,
        tel_verify: info.tel_verify,
    }))
}

/// 申请一组人机验证注册。
pub async fn login_pre_capture(context: LoginContext) -> CaptchaRegistrationResult {
    let (ctx, _env, transport) = context.split();
    let result = spawn_login!("人机验证注册工作任务未完成", async move {
        login::pre_capture(&transport, &ctx).await
    });
    CaptchaRegistrationResult::from_result(result.map(Into::into))
}

/// 确认一组注册可用，不可用就整组换一次（#140）。
pub async fn login_resolve_captcha(
    context: LoginContext,
    registration: CaptchaRegistrationData,
    native_ios: bool,
) -> CaptchaRegistrationResult {
    let geetest = context.geetest_context();
    let (ctx, _env, transport) = context.split();
    let current = CaptchaRegistration {
        gt: registration.gt,
        challenge: registration.challenge,
        token: registration.token,
    };
    let result = spawn_login!("人机验证可用性工作任务未完成", async move {
        login::resolve_captcha(&transport, &geetest, &ctx, current, native_ios).await
    });
    CaptchaRegistrationResult::from_result(result.map(Into::into))
}

/// 风控流程：发短信。
pub async fn login_safe_center_sms(
    context: LoginContext,
    verification: RiskVerificationData,
    sms_type: Option<String>,
    captcha: CaptchaSolutionData,
) -> CaptchaKeyResult {
    let (ctx, env, transport) = context.split();
    let solution = captcha.to_domain();
    let target = verification.to_domain();
    let result = spawn_login!("安全中心短信工作任务未完成", async move {
        login::send_safe_center_sms(
            &transport,
            &ctx,
            &env,
            &target,
            sms_type.as_deref(),
            &solution,
        )
        .await
    });
    CaptchaKeyResult::from_result(result)
}

/// 风控流程：提交验证码，换一次性 code。
pub async fn login_safe_center_verify(
    context: LoginContext,
    verification: RiskVerificationData,
    code: String,
    captcha_key: String,
    sms_type: Option<String>,
) -> OauthCodeResult {
    let (ctx, env, transport) = context.split();
    let target = verification.to_domain();
    let result = spawn_login!("安全中心验证工作任务未完成", async move {
        login::verify_safe_center_sms(
            &transport,
            &ctx,
            &env,
            &target,
            &code,
            &captcha_key,
            sms_type.as_deref(),
        )
        .await
    });
    OauthCodeResult::from_result(result)
}

/// 用一次性 code 换身份材料。
pub async fn login_exchange_oauth_code(context: LoginContext, code: String) -> LoginTokensResult {
    let (ctx, env, transport) = context.split();
    let result = spawn_login!("换取登录凭证工作任务未完成", async move {
        login::exchange_oauth_code(&transport, &ctx, &env, &code).await
    });
    LoginTokensResult::from_result(result.map(tokens_to_dart))
}

/// 登出。csrf 由上下文带来，字段名 `biliCSRF` 在 Rust 侧补。
pub async fn login_logout(context: LoginContext) -> WriteResult {
    let (ctx, _env, transport) = context.split();
    let result = spawn_login!("登出工作任务未完成", async move {
        login::logout(&transport, &ctx).await
    });
    WriteResult::from_result(result)
}

/// 已登录设备列表。
pub async fn login_devices(
    context: LoginContext,
    access_key: Option<String>,
) -> LoginDevicesResult {
    let (ctx, env, transport) = context.split();
    let result = spawn_login!("登录设备工作任务未完成", async move {
        login::fetch_login_devices(&transport, &ctx, &env, access_key.as_deref()).await
    });
    LoginDevicesResult::from_result(result.map(|devices| {
        devices
            .into_iter()
            .map(|d| LoginDeviceData {
                device_name: d.device_name,
                is_current_device: d.is_current_device,
                latest_login_at: d.latest_login_at,
                source: d.source,
            })
            .collect()
    }))
}
