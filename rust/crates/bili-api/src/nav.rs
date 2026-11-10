//! nav 接口：取 WBI 的 `img_key` / `sub_key`。
//!
//! `/x/web-interface/nav` 的主业务是返回当前登录态与用户信息，WBI 的两个
//! key 只是顺带挂在 `data.wbi_img` 下。
//!
//! ## 为什么这里不走 [`crate::envelope::unwrap_envelope`]
//!
//! 未登录时 nav 返回 `code: -101`（账号未登录），但 `data.wbi_img`
//! **照常存在**。按信封的通用规则，-101 会被判成 `Auth` 错误并丢掉
//! data——匿名用户从此拿不到 key，所有 WBI 接口随之失效，而现象是一个
//! 个业务错误码，与登录态看不出关联。基线也正是直接读 `data`、不看
//! `code`（`lib/utils/wbi_sign.dart::_getWbiKeys`）。
//!
//! 所以这里只认一件事：**`wbi_img` 在不在**。在就用，不在才报错。

use bili_domain::error::{DomainError, ErrorKind};
use bili_protocol::keys;
use serde::Deserialize;

use crate::http::{HttpGet, HttpTransport, RequestContext, check_status};
use crate::request::path;

#[derive(Deserialize)]
struct NavEnvelope {
    data: Option<NavData>,
}

#[derive(Deserialize)]
struct NavData {
    wbi_img: Option<WbiImg>,
}

#[derive(Deserialize)]
struct WbiImg {
    img_url: String,
    sub_url: String,
}

/// 从 nav 响应体解析出 `mixin_key`。
///
/// # Errors
/// JSON 不合法、缺 `wbi_img`、或 URL 形状不符 → [`ErrorKind::Protocol`]。
/// 这三种都是「我们对接口的理解过时了」，不是可重试的失败。
pub fn parse_mixin_key(body: &str) -> Result<String, DomainError> {
    let env: NavEnvelope = serde_json::from_str(body)
        .map_err(|e| DomainError::new(ErrorKind::Protocol, format!("nav 响应不是预期结构: {e}")))?;

    let img = env
        .data
        .and_then(|d| d.wbi_img)
        .ok_or_else(|| DomainError::new(ErrorKind::Protocol, "nav 响应缺少 wbi_img"))?;

    keys::mixin_key_from_nav_urls(&img.img_url, &img.sub_url).ok_or_else(|| {
        // 不把 URL 写进错误：它不含凭据，但错误会进日志，而这里能说清
        // 问题的是「形状变了」，URL 本身无助于定位。
        DomainError::new(ErrorKind::Protocol, "wbi_img 的 URL 形状不符，无法取出 key")
    })
}

/// 取 `mixin_key`。
///
/// **调用方负责缓存**：服务端按天轮换 key，每次签名都请求一次 nav 既慢
/// 又像机器人。缓存策略见 [`bili_protocol::keys::MixinKeyCache`]；放在
/// 调用方是因为「存到哪里」（Hive / 文件 / 内存）是宿主的决定，而缓存
/// 的真相源必须只有一个。
///
/// # Errors
/// 传输失败 → `Network`；非 2xx → 见 [`check_status`]；响应结构不符 →
/// `Protocol`。
pub async fn fetch_mixin_key<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
) -> Result<String, DomainError> {
    let url = format!("{}{}", ctx.base_url.trim_end_matches('/'), path::NAV);
    tracing::debug!(path = path::NAV, "fetch wbi keys");

    let resp = transport
        .get(HttpGet {
            url,
            headers: ctx.all_headers(),
            timeout: ctx.timeout,
        })
        .await?;

    check_status(resp.status)?;
    parse_mixin_key(&resp.body)
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    // 占位 key，与 WBI fixture 同源，非真实凭据。
    const ANONYMOUS_NAV: &str = r#"{
        "code": -101,
        "message": "账号未登录",
        "ttl": 1,
        "data": {
            "isLogin": false,
            "wbi_img": {
                "img_url": "https://i0.hdslb.com/bfs/wbi/7cd084941338484aae1ad9425b84077c.png",
                "sub_url": "https://i0.hdslb.com/bfs/wbi/4932caff0ff746eab6f01bf08b70ac45.png"
            }
        }
    }"#;

    #[test]
    fn anonymous_response_still_yields_keys() {
        // 这条是本模块存在的理由：-101 不能让 key 丢失。
        let got = parse_mixin_key(ANONYMOUS_NAV).expect("未登录时也必须能取到 key");
        assert_eq!(got, "ea1db124af3c7062474693fa704f4ff8");
    }

    #[test]
    fn logged_in_response_yields_same_keys() {
        let body = ANONYMOUS_NAV
            .replace("-101", "0")
            .replace("\"isLogin\": false", "\"isLogin\": true");
        let got = parse_mixin_key(&body).expect("登录态同样能取到 key");
        assert_eq!(got, "ea1db124af3c7062474693fa704f4ff8");
    }

    #[test]
    fn missing_wbi_img_is_a_protocol_error() {
        let err = parse_mixin_key(r#"{"code":0,"data":{"isLogin":true}}"#)
            .expect_err("缺 wbi_img 必须报错");
        assert_eq!(err.kind(), ErrorKind::Protocol);
    }

    #[test]
    fn malformed_url_is_a_protocol_error() {
        let body = ANONYMOUS_NAV.replace(
            "https://i0.hdslb.com/bfs/wbi/7cd084941338484aae1ad9425b84077c.png",
            "7cd084941338484aae1ad9425b84077c.png",
        );
        let err = parse_mixin_key(&body).expect_err("URL 无 / 时必须报错");
        assert_eq!(err.kind(), ErrorKind::Protocol);
    }

    #[test]
    fn short_keys_are_rejected() {
        // key 短于重排表索引时不能静默产出一个错签名。
        let body = ANONYMOUS_NAV.replace("7cd084941338484aae1ad9425b84077c", "short");
        let err = parse_mixin_key(&body).expect_err("key 过短必须报错");
        assert_eq!(err.kind(), ErrorKind::Protocol);
    }

    #[test]
    fn invalid_json_is_a_protocol_error() {
        let err = parse_mixin_key("not json").expect_err("非 JSON 必须报错");
        assert_eq!(err.kind(), ErrorKind::Protocol);
    }
}
