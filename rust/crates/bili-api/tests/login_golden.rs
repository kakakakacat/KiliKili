//! 登录流程的 golden 快照（S14 / issue #65）。
//!
//! 对照机制同 ADR-011 决策五：本测试冻结 Rust 的解析结果，
//! `test/services/rust/login_parity_test.dart` 用迁移前那套判断的逐行
//! 转写解析同一份 raw，与快照逐字段比对。
//!
//! ## fixture 全部是手工构造的，且刻意不含真实凭据
//!
//! 登录响应**就是凭据本身**（access_token、refresh_token、SESSDATA）。
//! 抓一份真实响应放进仓库等于把一个账号交出去，脱敏之后剩下的也正是这里
//! 手写的东西。所以这几份 raw 里的 token 一律是 `fake-*` 字面量，形状按
//! 协议写：键名、层级、以及几种"半截"情形（cookie 只有名没有值、
//! `token_info` 为 null、`captcha_key` 为空串而 `recaptcha_url` 非空）。
//!
//! 它们证明的是**映射与分支一致**，不能证明我们对协议的理解正确。真机
//! 确认按 #67 的约定留到 Phase 2（人工用例见 `docs/qa/RELEASE-VALIDATION.md`）。
//!
//! ## 快照里不写凭据值
//!
//! 即便 fixture 里是假 token，快照也只记形状：token 写成 `<redacted>`，
//! cookie 只记名字与条数。这样这套文件日后被人拿真实响应重新生成时，
//! 不会顺手把一份真凭据提交进仓库。
//!
//! 重新生成：
//! ```sh
//! UPDATE_GOLDEN=1 cargo test -p bili-api --test login_golden
//! ```

#![allow(clippy::expect_used)]

use std::fs;
use std::path::{Path, PathBuf};

use bili_domain::login::{LoginTokens, PasswordLogin, QrPoll, SmsSent};
use serde_json::{Value, json};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/login")
        .join(name)
}

fn check(raw_name: &str, snapshot_name: &str, parse: impl Fn(&str) -> Value) {
    let raw = fs::read_to_string(fixture(raw_name)).expect("raw fixture 可读");
    let got = parse(&raw);

    let snapshot_path = fixture(snapshot_name);
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        let pretty = serde_json::to_string_pretty(&got).expect("可序列化");
        fs::write(&snapshot_path, format!("{pretty}\n")).expect("快照可写");
        return;
    }

    let expected: Value = serde_json::from_str(
        &fs::read_to_string(&snapshot_path).expect("缺少快照，用 UPDATE_GOLDEN=1 重新生成"),
    )
    .expect("快照必须是合法 JSON");

    assert_eq!(got, expected, "{raw_name} 的解析结果与冻结快照不一致");
}

/// 身份材料的**脱敏投影**：证明"解析出了几条、叫什么名字"，不记值。
fn tokens_shape(tokens: &LoginTokens) -> Value {
    json!({
        "access_token": if tokens.access_token.expose().is_empty() { "<empty>" } else { "<redacted>" },
        "refresh_token": if tokens.refresh_token.expose().is_empty() { "<empty>" } else { "<redacted>" },
        "expires_in": tokens.expires_in,
        "cookie_names": tokens.cookies.iter().map(|c| c.name.clone()).collect::<Vec<_>>(),
    })
}

#[test]
fn login_devices_match_the_frozen_snapshot() {
    check("login_devices.raw.json", "login_devices.rust.json", |raw| {
        let devices = bili_api::login::parse_login_devices(raw).expect("样本应解析成功");
        Value::Array(
            devices
                .iter()
                .map(|d| {
                    json!({
                        "device_name": d.device_name,
                        "is_current_device": d.is_current_device,
                        "latest_login_at": d.latest_login_at,
                        "source": d.source,
                    })
                })
                .collect(),
        )
    });
}

#[test]
fn qr_poll_states_match_the_frozen_snapshot() {
    for (raw, snapshot) in [
        ("qr_poll_confirmed.raw.json", "qr_poll_confirmed.rust.json"),
        ("qr_poll_pending.raw.json", "qr_poll_pending.rust.json"),
        ("qr_poll_expired.raw.json", "qr_poll_expired.rust.json"),
    ] {
        check(
            raw,
            snapshot,
            |body| match bili_api::login::parse_qr_poll(body).expect("样本应解析成功") {
                QrPoll::Confirmed(tokens) => json!({
                    "state": "confirmed",
                    "tokens": tokens_shape(&tokens),
                }),
                QrPoll::Pending { code, message } => json!({
                    "state": "pending",
                    "code": code,
                    "message": message,
                }),
                QrPoll::Expired => json!({"state": "expired"}),
            },
        );
    }
}

#[test]
fn password_login_branches_match_the_frozen_snapshot() {
    for (raw, snapshot) in [
        ("password_risk.raw.json", "password_risk.rust.json"),
        ("password_captcha.raw.json", "password_captcha.rust.json"),
    ] {
        check(
            raw,
            snapshot,
            |body| match bili_api::login::parse_password_login(body).expect("样本应解析成功")
            {
                PasswordLogin::Authenticated(tokens) => json!({
                    "state": "authenticated",
                    "tokens": tokens_shape(&tokens),
                }),
                PasswordLogin::RiskControl(v) => json!({
                    "state": "risk_control",
                    "tmp_code": v.tmp_code,
                    "request_id": v.request_id,
                    "source": v.source,
                    "referer_url": v.referer_url,
                    "message": v.message,
                }),
                PasswordLogin::CaptchaRequired(r) => json!({
                    "state": "captcha_required",
                    "gt": r.gt,
                    "challenge": r.challenge,
                    "token": r.token,
                }),
            },
        );
    }
}

/// 短信发送的两种结论。
///
/// `sms_send_captcha` 那份是**成功响应里带着非空 `recaptcha_url`** 的情形：
/// `code` 是 0，照 `check_envelope` 的标准它是成功，而实际含义是"还得先
/// 过人机验证"。这条分支在基线里只有一处 `&& data['recaptcha_url'] == ""`
/// 守着，漏掉就会把一次没发出去的短信报成"发送成功"。
#[test]
fn sms_send_branches_match_the_frozen_snapshot() {
    for (raw, snapshot) in [
        ("sms_send_ok.raw.json", "sms_send_ok.rust.json"),
        ("sms_send_captcha.raw.json", "sms_send_captcha.rust.json"),
    ] {
        check(
            raw,
            snapshot,
            |body| match bili_api::login::parse_sms_send(body).expect("样本应解析成功") {
                Some(SmsSent::Sent { captcha_key }) => json!({
                    "state": "sent",
                    "captcha_key": captcha_key,
                }),
                Some(SmsSent::CaptchaRequired(r)) => json!({
                    "state": "captcha_required",
                    "gt": r.gt,
                    "challenge": r.challenge,
                    "token": r.token,
                }),
                None => json!({"state": "needs_new_registration"}),
            },
        );
    }
}
