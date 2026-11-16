//! 登录流程的领域实体（S14 / issue #65）。
//!
//! ## 凭据在这一层的位置
//!
//! RFC-0001 §2.2 定的是「Rust 不**保存**凭据，按请求传入、用完即弃」。
//! 登录流程是凭据的**产生地**，这条约定在这里要说得更细（见 issue #65
//! 的提醒）：
//!
//! - Rust 仍然不持久化任何凭据：没有静态变量、没有缓存、没有磁盘写入。
//!   登录结果沿调用栈返回给 Dart，由 `lib/utils/accounts/` 存起来——
//!   真相源仍然只有一处。
//! - 但登录结果**必然**经过 Rust：access_token、refresh_token 与
//!   cookie 就是接口的响应体。因此这些字段一律用
//!   [`Redacted`](crate::redact::Redacted) 包住，误把整个结构写进日志
//!   或 `error_detail` 也不会泄漏。
//! - 明文密码**不进任何结构体**：它只作为函数参数传给加密函数
//!   （`bili_protocol::login_crypto`），加密后原文即随栈帧消失。本模块
//!   没有任何字段存放它。
//!
//! ## 为什么这些判断在 Rust 而不在控制器里
//!
//! 「返回了 token 就是登录成功、status==2 是风控、-105 要验证码」这类
//! 分支是协议语义，不是展示规则：换掉宿主框架时它们一行都不该重写
//! （AGENTS §2）。基线把它们散在 `lib/pages/login/controller.dart` 的
//! 若干个 `if (res['status'])` 里，判断依据是裸 Map 的键。

use crate::redact::Redacted;

/// 一条登录 cookie。只保留 Dart 侧 `BiliCookieJar.fromList` 真正用到的
/// 两个字段——基线建 Cookie 时同样只读 `name` 与 `value`，其余（domain、
/// expires）由本地按 bilibili 域补齐。
#[derive(Clone, PartialEq, Eq)]
pub struct LoginCookie {
    pub name: String,
    pub value: Redacted<String>,
}

impl std::fmt::Debug for LoginCookie {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // 连 name 都不打：cookie 的键名本身就是「这里有一份凭据」的指路牌。
        f.write_str("LoginCookie(<redacted>)")
    }
}

/// 登录成功后拿到的身份材料。
#[derive(Clone, PartialEq, Eq)]
pub struct LoginTokens {
    pub access_token: Redacted<String>,
    pub refresh_token: Redacted<String>,
    /// 有效期（秒）。非凭据，排查续期问题时要看。
    pub expires_in: i64,
    pub cookies: Vec<LoginCookie>,
}

impl std::fmt::Debug for LoginTokens {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoginTokens")
            .field("expires_in", &self.expires_in)
            .field("cookies", &self.cookies.len())
            .finish_non_exhaustive()
    }
}

/// 人机验证（极验）的一次注册。
///
/// 三个值是**一组**：`gt` 属于它自己的 challenge 与 token，不能把两次
/// 注册的字段拼着用（见 #140 与 `resolve_captcha`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptchaRegistration {
    pub gt: String,
    pub challenge: String,
    pub token: String,
}

impl CaptchaRegistration {
    /// 三项齐全才算一次可用的注册。基线在控制器里用
    /// `isGeeArgumentValid` 做同样的检查，但只在部分分支上做。
    #[must_use]
    pub fn is_usable(&self) -> bool {
        !self.gt.is_empty() && !self.challenge.is_empty() && !self.token.is_empty()
    }
}

/// 过了人机验证之后要回填给登录/短信接口的那组值。
///
/// 由宿主的 WebView（极验 SDK）产出：那是平台 API，留在宿主
/// （AGENTS §2「可以留在宿主」的第三条）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CaptchaSolution {
    pub challenge: Option<String>,
    pub seccode: Option<String>,
    pub validate: Option<String>,
    pub token: Option<String>,
}

/// 扫码登录的二维码。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QrCode {
    pub auth_code: String,
    pub url: String,
}

/// 扫码轮询的结论。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QrPoll {
    /// 已确认，拿到身份材料。
    Confirmed(Box<LoginTokens>),
    /// 仍在等待扫码或确认。`message` 供界面展示。
    Pending { code: i64, message: String },
    /// 二维码已失效（86038），必须重新申请。界面据此停掉轮询定时器。
    Expired,
}

/// 密码登录的结论。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PasswordLogin {
    /// 直接登录成功。
    Authenticated(Box<LoginTokens>),
    /// 风控：要求先验证手机号（接口 `data.status == 2`）。
    ///
    /// 基线从 `data.url` 的查询串里现扒 `tmp_token`/`request_id`/`source`
    /// 并直接 `!` 断言；这里在解析时就取出来，缺任一项即判协议错误——
    /// 少一个值，后面三条接口会以「参数错误」的形式失败。
    RiskControl(Box<RiskVerification>),
    /// 需要人机验证（-105）。注册信息来自 `data.url` 的查询串。
    CaptchaRequired(Box<CaptchaRegistration>),
}

/// 风控验证手机号这条支线所需的全部参数。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RiskVerification {
    /// 原始 URL。后续三条接口要把它原样放进 `Referer`。
    pub referer_url: String,
    pub tmp_code: String,
    pub request_id: String,
    pub source: String,
    /// 服务端给的说明文案，直接展示。
    pub message: String,
}

/// 风控流程里展示给用户的账号信息（打码后的手机号/邮箱）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SafeCenterInfo {
    pub hide_tel: Option<String>,
    pub hide_mail: Option<String>,
    /// 该账号是否支持短信验证。为 false 时整条支线走不下去。
    pub tel_verify: bool,
}

/// 发短信验证码的结论。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SmsSent {
    /// 已发出，`captcha_key` 是登录时要回填的凭证。
    Sent { captcha_key: String },
    /// 服务端要求先过人机验证。
    ///
    /// 基线把这种情形藏在「成功响应里 `recaptcha_url` 非空」与「失败码
    /// -105」两处，控制器各写一遍解析；这里合成同一个分支。
    CaptchaRequired(Box<CaptchaRegistration>),
}

/// 短信登录成功后的身份材料，与密码登录共用。
pub type SmsLogin = LoginTokens;

/// 一台已登录设备。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginDevice {
    pub device_name: Option<String>,
    pub is_current_device: bool,
    pub latest_login_at: Option<String>,
    pub source: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credentials_never_appear_in_debug_output() {
        let tokens = LoginTokens {
            access_token: Redacted::new("access-xyz".to_owned()),
            refresh_token: Redacted::new("refresh-xyz".to_owned()),
            expires_in: 2_592_000,
            cookies: vec![LoginCookie {
                name: "SESSDATA".to_owned(),
                value: Redacted::new("sess-xyz".to_owned()),
            }],
        };
        let printed = format!("{tokens:?}");
        assert!(!printed.contains("access-xyz"));
        assert!(!printed.contains("refresh-xyz"));
        assert!(!printed.contains("sess-xyz"));
        // 连 cookie 名都不该出现。
        assert!(!printed.contains("SESSDATA"));
        // 非凭据字段仍要可见，否则排查续期问题时只剩一团 <redacted>。
        assert!(printed.contains("2592000"));
    }

    #[test]
    fn a_registration_missing_any_field_is_unusable() {
        let full = CaptchaRegistration {
            gt: "gt".to_owned(),
            challenge: "ch".to_owned(),
            token: "tk".to_owned(),
        };
        assert!(full.is_usable());
        for broken in [
            CaptchaRegistration {
                gt: String::new(),
                ..full.clone()
            },
            CaptchaRegistration {
                challenge: String::new(),
                ..full.clone()
            },
            CaptchaRegistration {
                token: String::new(),
                ..full.clone()
            },
        ] {
            assert!(!broken.is_usable(), "缺字段的注册不能当成可用");
        }
    }
}
