//! 设备元数据由 Rust 编码；账号凭据与随机会话值由宿主按请求提供。
use base64::{Engine, engine::general_purpose::STANDARD};
use bili_domain::redact::Redacted;
use bili_grpc::bilibili::metadata::{
    Metadata,
    device::Device,
    fawkes::FawkesReq,
    locale::{Locale, LocaleIds},
    network::Network,
};
use bili_protocol::grpc as protocol;
use prost::Message;

#[derive(Debug, Clone)]
pub struct GrpcIdentity {
    pub access_key: Option<Redacted<String>>,
    pub buvid: String,
    pub session_id: String,
}

fn encoded(message: impl Message) -> String {
    STANDARD.encode(message.encode_to_vec())
}

/// 返回与 Dart 基线相同的头。调用方不得把结果写入日志。
pub fn headers(identity: &GrpcIdentity) -> Vec<(String, String)> {
    let locale = || LocaleIds {
        language: Some(protocol::LOCALE_LANGUAGE.into()),
        region: Some(protocol::LOCALE_REGION.into()),
        script: Some(protocol::LOCALE_SCRIPT.into()),
    };
    let mut headers = vec![
        ("grpc-encoding".into(), "gzip".into()),
        // 这个看似拼错的名字就是线上基线，迁移不顺便修协议。
        ("gzip-accept-encoding".into(), "gzip,identity".into()),
        ("user-agent".into(), protocol::USER_AGENT.into()),
        ("x-bili-gaia-vtoken".into(), String::new()),
        ("x-bili-aurora-zone".into(), String::new()),
        ("x-bili-trace-id".into(), protocol::TRACE_ID.into()),
        ("bili-http-engine".into(), "cronet".into()),
        ("x-bili-exps-bin".into(), String::new()),
        ("buvid".into(), identity.buvid.clone()),
        (
            "x-bili-network-bin".into(),
            encoded(Network {
                r#type: Some(protocol::NETWORK_TYPE),
                ..Default::default()
            }),
        ),
        (
            "x-bili-locale-bin".into(),
            encoded(Locale {
                c_locale: Some(locale()),
                s_locale: Some(locale()),
                timezone: Some(protocol::TIMEZONE.into()),
                ..Default::default()
            }),
        ),
        (
            "x-bili-fawkes-req-bin".into(),
            encoded(FawkesReq {
                appkey: Some(protocol::MOBI_APP.into()),
                env: Some(protocol::FAWKES_ENV.into()),
                session_id: Some(identity.session_id.clone()),
            }),
        ),
        (
            "x-bili-device-bin".into(),
            encoded(Device {
                app_id: Some(protocol::APP_ID),
                build: Some(protocol::BUILD),
                buvid: Some(identity.buvid.clone()),
                mobi_app: Some(protocol::MOBI_APP.into()),
                platform: Some(protocol::DEVICE.into()),
                channel: Some(protocol::CHANNEL.into()),
                brand: Some(protocol::DEVICE.into()),
                model: Some(protocol::DEVICE.into()),
                osver: Some(protocol::OS_VERSION.into()),
                version_name: Some(protocol::VERSION_NAME.into()),
                ..Default::default()
            }),
        ),
        (
            "x-bili-metadata-bin".into(),
            encoded(Metadata {
                access_key: identity.access_key.as_ref().map(|key| key.expose().clone()),
                mobi_app: Some(protocol::MOBI_APP.into()),
                device: Some(protocol::DEVICE.into()),
                build: Some(protocol::BUILD),
                channel: Some(protocol::CHANNEL.into()),
                buvid: Some(identity.buvid.clone()),
                platform: Some(protocol::DEVICE.into()),
            }),
        ),
    ];
    if let Some(key) = &identity.access_key {
        headers.push((
            "authorization".into(),
            format!("identify_v1 {}", key.expose()),
        ));
    }
    headers
}
