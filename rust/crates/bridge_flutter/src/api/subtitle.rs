//! 字幕转换桥接：字幕文件正文 → WebVTT / SRT。
//!
//! 入参是**响应正文**而不是解好的对象：这条路径的意义就在于 Dart 不必
//! 解那份 JSON。一个长视频的字幕有几千条，基线为此把转换放进了
//! `compute`（isolate）——也就是说这段工作重到必须躲开 UI 线程。本函数
//! 不是 `sync`，FRB 会把它放到 Rust 工作线程，效果相同且少一次消息拷贝。
//!
//! 解析在 `bili_api::subtitle`，格式化在 `bili_core::subtitle_format`；
//! 本模块只做转换与错误映射。

use bili_core::subtitle_format::{to_srt, to_vtt};

/// 字幕转换结果。
#[derive(Debug, Clone)]
pub struct SubtitleConvert {
    /// 转换好的文本。`None` 且 `missing_body` 为假即失败。
    pub text: Option<String>,
    /// 响应里没有 `body` 数组。基线此时返回 null——**不是**错误，调用方
    /// 应当当作「这份字幕没有内容」，而不是降级重试。
    pub missing_body: bool,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
}

/// 转换字幕。
///
/// `format` 是 Dart `SubtitleFormat` 的下标：0 json / 1 vtt / 2 srt。
/// 两端的顺序一致由 [`format_order_matches_the_dart_enum`] 钉住。
///
/// `json` 与基线一样不支持（基线抛 `UnimplementedError`）：这里返回
/// 失败，调用方降级后由基线原样抛出。
#[must_use]
pub fn convert_subtitle(body: String, format: i32) -> SubtitleConvert {
    fn failure(kind: &str, detail: String) -> SubtitleConvert {
        SubtitleConvert {
            text: None,
            missing_body: false,
            error_kind: Some(kind.to_owned()),
            error_detail: Some(detail),
        }
    }

    if format != FORMAT_VTT && format != FORMAT_SRT {
        return failure("unsupported", format!("不支持的字幕格式下标 {format}"));
    }

    let cues = match bili_api::subtitle::parse_cues(&body) {
        Ok(Some(cues)) => cues,
        Ok(None) => {
            return SubtitleConvert {
                text: None,
                missing_body: true,
                error_kind: None,
                error_detail: None,
            };
        }
        Err(e) => return failure(&e.kind().to_string(), e.detail().to_owned()),
    };

    let text = if format == FORMAT_VTT {
        to_vtt(&cues)
    } else {
        to_srt(&cues)
    };
    SubtitleConvert {
        text: Some(text),
        missing_body: false,
        error_kind: None,
        error_detail: None,
    }
}

/// Dart `SubtitleFormat.vtt` 的下标。
const FORMAT_VTT: i32 = 1;
/// Dart `SubtitleFormat.srt` 的下标。
const FORMAT_SRT: i32 = 2;

#[cfg(test)]
mod tests {
    use super::{FORMAT_SRT, FORMAT_VTT, convert_subtitle};

    const BODY: &str = r#"{"body":[{"from":0,"to":1.5,"content":"一句话"}]}"#;

    /// Dart 的 `SubtitleFormat` 声明顺序是 json / vtt / srt，下标跨 FFI
    /// 传递。改了那边的顺序，这条断言不会红——但 VTT 会被当成 SRT 转出来，
    /// 那是看得见的故障。所以这里额外用**内容**来认格式。
    #[test]
    fn format_order_matches_the_dart_enum() {
        let vtt = convert_subtitle(BODY.to_owned(), FORMAT_VTT);
        assert!(vtt.text.is_some_and(|t| t.starts_with("WEBVTT")));

        let srt = convert_subtitle(BODY.to_owned(), FORMAT_SRT);
        assert!(srt.text.is_some_and(|t| t.starts_with("1\n00:00:00,000")));
    }

    #[test]
    fn the_json_format_is_unsupported_like_the_baseline() {
        let result = convert_subtitle(BODY.to_owned(), 0);
        assert!(result.text.is_none());
        assert!(!result.missing_body);
        assert_eq!(result.error_kind.as_deref(), Some("unsupported"));
    }

    #[test]
    fn a_response_without_body_is_not_a_failure() {
        let result = convert_subtitle(r#"{"font_size":0.4}"#.to_owned(), FORMAT_VTT);
        assert!(result.missing_body);
        assert!(
            result.error_kind.is_none(),
            "没有字幕不是错误，不该触发降级"
        );
    }

    #[test]
    fn malformed_json_is_a_failure() {
        let result = convert_subtitle("not json".to_owned(), FORMAT_VTT);
        assert!(!result.missing_body);
        assert_eq!(result.error_kind.as_deref(), Some("protocol"));
    }
}
