//! 字幕文件（`subtitle_url` 指向的 JSON）的解析。
//!
//! 形如 `{"font_size":..., "body":[{"from":1.2,"to":3.4,"content":"..."}]}`。
//! 基线只看 `body`（`VideoHttp.getSubtitles` 的 `res.data?['body'] case List`），
//! 其余字段（字号、背景色、位置）没有消费方。
//!
//! 入参是**响应正文**，不是已解出的对象：这条路径的意义就在于 Dart 不
//! 必解这份 JSON——一个长视频的字幕有几千条，基线为此把转换放进了
//! `compute`（isolate）。

use bili_domain::error::{DomainError, ErrorKind};
use bili_domain::subtitle::SubtitleCue;
use serde_json::Value;

/// 解析字幕文件正文。
///
/// 返回 `None` 表示响应里没有 `body` 数组——基线此时返回 null（不是错误，
/// 也不是空字幕）。
///
/// # Errors
/// 不是合法 JSON，或某一条缺 `from`/`to`/`content`、类型不对时返回
/// `Protocol`。基线在这些情况下会抛（`null.trim()`、`null ~/ 3600`），
/// 调用方降级回 Dart 即可得到同样的结果。
pub fn parse_cues(body: &str) -> Result<Option<Vec<SubtitleCue>>, DomainError> {
    let root: Value = serde_json::from_str(body)
        .map_err(|e| DomainError::new(ErrorKind::Protocol, format!("字幕不是合法 JSON: {e}")))?;
    let Some(items) = root.get("body").and_then(Value::as_array) else {
        return Ok(None);
    };

    let mut cues = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        let number = |key: &str| -> Result<f64, DomainError> {
            item.get(key).and_then(Value::as_f64).ok_or_else(|| {
                DomainError::new(ErrorKind::Protocol, format!("第 {index} 条字幕缺 {key}"))
            })
        };
        let content = item
            .get("content")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                DomainError::new(ErrorKind::Protocol, format!("第 {index} 条字幕缺 content"))
            })?
            .to_owned();

        cues.push(SubtitleCue {
            from: number("from")?,
            to: number("to")?,
            content,
        });
    }
    Ok(Some(cues))
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::parse_cues;

    #[test]
    fn parses_the_body_array() {
        let json = r#"{"font_size":0.4,"body":[
            {"from":1.2,"to":3.45,"location":2,"content":"第一句"},
            {"from":4,"to":5,"content":"第二句"}
        ]}"#;
        let cues = parse_cues(json).expect("合法").expect("有 body");
        assert_eq!(cues.len(), 2);
        assert!((cues[0].from - 1.2).abs() < f64::EPSILON);
        assert!((cues[1].to - 5.0).abs() < f64::EPSILON);
        assert_eq!(cues[0].content, "第一句");
    }

    #[test]
    fn missing_body_is_not_an_error() {
        // 基线此时返回 null，调用方当作「没有字幕」。
        assert!(parse_cues(r#"{"font_size":0.4}"#).expect("合法").is_none());
    }

    #[test]
    fn empty_body_is_an_empty_list() {
        assert_eq!(
            parse_cues(r#"{"body":[]}"#)
                .expect("合法")
                .expect("有 body"),
            vec![]
        );
    }

    #[test]
    fn a_cue_without_content_is_an_error() {
        // 基线会在 `item['content'].trim()` 上抛。
        assert!(parse_cues(r#"{"body":[{"from":1,"to":2}]}"#).is_err());
    }

    #[test]
    fn a_cue_without_timing_is_an_error() {
        assert!(parse_cues(r#"{"body":[{"to":2,"content":"x"}]}"#).is_err());
    }

    #[test]
    fn malformed_json_is_an_error() {
        assert!(parse_cues("not json").is_err());
    }
}
