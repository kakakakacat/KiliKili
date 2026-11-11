//! `DmView` 字幕解析桥接。只读、单次响应；失败交还 Dart 原解析路径。
//!
//! 这里的收益**不是速度**——`DmView` 的响应只有几 KB，Dart 解它从来
//! 不是瓶颈（基线也没给它开 isolate，只给弹幕分段开了）。收益是字幕这
//! 条链路的解析与实体归 Rust：Phase 2 的 Slint 壳要的是同一份
//! `bili_domain::subtitle::Subtitle`，而不是再写一遍 protobuf 映射。
//!
//! 字节与弹幕分段同形：**解帧并解压之后**的 protobuf。

/// 字幕解析结果。失败不抛异常，由 Dart 决定降级。
#[derive(Debug, Clone)]
pub struct DmViewSubtitlesParse {
    /// 规范化后的 JSON 数组（已按基线顺序排好）。失败时为 `None`。
    pub json: Option<String>,
    /// 失败大类，见 `bili_domain::error::ErrorKind`。
    pub error_kind: Option<String>,
    /// 失败详情。**不含凭据**。
    pub error_detail: Option<String>,
}

/// 解析 `DmViewReply` 的字幕轨。
///
/// 空数组表示这个视频没有字幕（基线此时返回 null，调用方对两者的处理
/// 相同）；`json` 为 `None` 才是失败。
#[must_use]
pub fn parse_dm_view_subtitles(bytes: Vec<u8>) -> DmViewSubtitlesParse {
    match bili_api::dm_view::parse_subtitles(&bytes) {
        Ok(subtitles) => match serde_json::to_string(&subtitles) {
            Ok(json) => DmViewSubtitlesParse {
                json: Some(json),
                error_kind: None,
                error_detail: None,
            },
            Err(e) => DmViewSubtitlesParse {
                json: None,
                error_kind: Some("protocol".to_owned()),
                error_detail: Some(format!("字幕序列化失败: {e}")),
            },
        },
        Err(e) => DmViewSubtitlesParse {
            json: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
        },
    }
}
