//! 取流响应解析的 Dart 导出（shadow mode 用）。
//!
//! 与 `play_url_fetch`（Rust 发请求）是两条独立路径，理由同
//! `video_detail.rs`：shadow 比对的是**同一份响应字节**在两种实现下的
//! 规范化结果，不重复发请求——对取流接口重复请求尤其糟糕，每次都会
//! 真的签发一组 CDN 地址并计入风控。
//!
//! 返回 JSON 字符串而非结构体，同样是因为整棵流结构有二十余个嵌套
//! 类型，而比对需要的恰好就是 JSON。

use bili_api::playurl::{self, Outcome};
use bili_domain::video::VideoType;

/// 解析结果。不用 `Result`：shadow mode 下失败也不该打断主流程。
#[derive(Debug, Clone)]
pub struct PlayUrlParse {
    /// 规范化后的 JSON。失败或需要降级时为 `None`。
    pub json: Option<String>,
    /// 内容为 DRM、应改走降级接口。此时 `json` 为 `None` 而
    /// `error_kind` 也为 `None`——它不是失败。
    pub needs_fallback: bool,
    /// 失败大类：`network|remote|protocol|auth|storage|player|cancelled`。
    pub error_kind: Option<String>,
    /// 失败详情。**不含请求参数与凭据。**
    pub error_detail: Option<String>,
}

/// 解析一次 playurl 响应。
///
/// `body` 是整个响应体（含信封）。`video_type` 取 `ugc`/`pgc`/`pugv`，
/// `fallback` 表示这次响应来自降级接口——响应本身不足以区分这两点
/// （PUGV 降级与 UGC 的外层形状相同）。
#[must_use]
pub fn parse_play_url(body: String, video_type: String, fallback: bool) -> PlayUrlParse {
    let video_type = match video_type.as_str() {
        "ugc" => VideoType::Ugc,
        "pgc" => VideoType::Pgc,
        "pugv" => VideoType::Pugv,
        _ => {
            return PlayUrlParse {
                json: None,
                needs_fallback: false,
                error_kind: Some("protocol".to_owned()),
                error_detail: Some("未知的 video_type".to_owned()),
            };
        }
    };

    match playurl::parse(&body, video_type, fallback) {
        Ok(Outcome::Ready(play)) => match serde_json::to_string(&play) {
            Ok(json) => PlayUrlParse {
                json: Some(json),
                needs_fallback: false,
                error_kind: None,
                error_detail: None,
            },
            // 只可能源于本地 bug，但不值得为此 panic 掉宿主 App。
            Err(e) => PlayUrlParse {
                json: None,
                needs_fallback: false,
                error_kind: Some("protocol".to_owned()),
                error_detail: Some(format!("实体序列化失败: {e}")),
            },
        },
        Ok(Outcome::NeedsFallback) => PlayUrlParse {
            json: None,
            needs_fallback: true,
            error_kind: None,
            error_detail: None,
        },
        Err(e) => PlayUrlParse {
            json: None,
            needs_fallback: false,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
        },
    }
}
