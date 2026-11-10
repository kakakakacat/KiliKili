//! 取流结果实体。
//!
//! 与 [`crate::video`] 同样的约定：这是**领域实体**，不是接口响应的
//! 镜像。playurl 有四条接口路径（UGC / OGV playview / PGC 降级 / PUGV
//! 降级），响应外层各不相同——外层的差异全部关在 `bili-api` 的 DTO 层，
//! 到这里只剩一种形状。
//!
//! ## 为什么字段这么碎
//!
//! 基线 `PlayUrlModel`（`lib/models/video/play/url.dart`）消费的就是这些
//! 字段，播放器、清晰度切换、音量均衡、跳过片头片尾各取一部分。收窄成
//! 「播放器需要的三五个字段」要先确定播放器后端（RFC §13 问题 4 未决，
//! 见 issue #12），现在收窄等于提前替那个决策下注。
//!
//! ## 刻意不做的归一
//!
//! 清晰度描述（`VideoQuality` / `AudioQuality`）、多语言条目的排序与
//! 「（AI）」后缀，都留在 Dart：它们是**展示**规则，不是协议。Rust 若
//! 一并做掉，shadow 比对就得连带比对展示规则，差异来源会变得不可归因。

use serde::Serialize;

/// 一次取流的结果。对应基线 `PlayUrlModel`。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PlayUrl {
    pub from: Option<String>,
    pub result: Option<String>,
    pub message: Option<String>,
    /// 当前清晰度代码。
    pub quality: Option<i64>,
    pub format: Option<String>,
    /// 时长（毫秒）。接口字段名是 `timelength`。
    pub time_length: Option<i64>,
    pub accept_format: Option<String>,
    pub accept_description: Option<Vec<String>>,
    pub accept_quality: Option<Vec<i64>>,
    pub video_codecid: Option<i64>,
    pub seek_param: Option<String>,
    pub seek_type: Option<String>,
    pub support_formats: Option<Vec<PlayFormat>>,
    pub dash: Option<Dash>,
    /// 非 DASH 的整段流（老视频、试看片段）。与 `dash` 互斥，但协议上
    /// 两者都可能缺失，因此各自可空。
    pub durl: Option<Vec<Durl>>,
    pub volume: Option<Volume>,
    /// 上次播放进度（毫秒）。
    ///
    /// 非 `Option`：基线的 setter 把 null 与非正数一律归零
    /// （`PlayUrlModel.lastPlayTime`），此处照搬——UI 直接拿它去 seek，
    /// 多一种「未知」状态只会让每个调用点各写一遍兜底。
    pub last_play_time: i64,
    pub last_play_cid: Option<i64>,
    /// 当前音轨语言（接口字段 `cur_language`）。
    pub cur_language: Option<String>,
    pub language: Option<LanguageOptions>,
    /// 片头片尾等可跳过片段（PGC）。
    pub clip_info_list: Option<Vec<ClipInfo>>,
}

/// 可选清晰度的描述。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PlayFormat {
    pub quality: Option<i64>,
    pub format: Option<String>,
    /// 接口字段 `new_description`。
    pub new_desc: Option<String>,
    pub display_desc: Option<String>,
    pub codecs: Option<Vec<String>>,
}

/// DASH 流。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Dash {
    pub duration: Option<i64>,
    pub min_buffer_time: Option<f64>,
    pub video: Option<Vec<MediaStream>>,
    /// 音频流。
    ///
    /// **已合并**：接口把无损（`flac.audio`）、杜比（`dolby.audio`）与
    /// 普通音轨（`audio`）放在三处，基线按 flac → dolby → 普通的顺序拼成
    /// 一个列表（`Dash.fromJson`）。顺序是基线行为的一部分（列表首项即
    /// 默认音轨），此处保持一致。
    pub audio: Option<Vec<MediaStream>>,
}

/// 一路 DASH 流。视频与音频共用一种形状，差别只在哪些字段有值。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MediaStream {
    /// 视频是清晰度代码，音频是音质代码。
    pub id: i64,
    pub base_url: Option<String>,
    pub backup_url: Option<Vec<String>>,
    pub band_width: Option<i64>,
    pub mime_type: Option<String>,
    pub codecs: Option<String>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    /// 帧率。接口给的是字符串（可能是 `"16000/672"` 这类分数）。
    pub frame_rate: Option<String>,
    pub sar: Option<String>,
    pub start_with_sap: Option<i64>,
    pub segment_base: Option<SegmentBase>,
    pub codecid: Option<i64>,
}

/// DASH 的初始化段与索引段范围。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SegmentBase {
    pub initialization: Option<String>,
    pub index_range: Option<String>,
}

/// 整段流。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Durl {
    pub order: Option<i64>,
    /// 本段时长（毫秒）。
    pub length: Option<i64>,
    pub size: Option<i64>,
    pub ahead: Option<String>,
    pub vhead: Option<String>,
    pub url: Option<String>,
    pub backup_url: Option<Vec<String>>,
}

/// 音量均衡参数（FFmpeg loudnorm）。
///
/// 全部非空：基线对每个字段都用了 `?? 0`，缺失按 0 处理是既定行为，
/// 而 `Volume.isNotEmpty` 正是靠「是否全为 0」判断要不要启用均衡。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Volume {
    pub measured_i: f64,
    pub measured_lra: f64,
    pub measured_tp: f64,
    pub measured_threshold: f64,
    pub target_offset: f64,
    pub target_i: f64,
    pub target_tp: f64,
}

/// 多语言音轨。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LanguageOptions {
    pub support: Option<bool>,
    /// 条目**未排序**：排序与「（AI）」标注是 Dart 的展示规则，见本模块
    /// 文档。
    pub items: Option<Vec<LanguageItem>>,
}

/// 一条音轨。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LanguageItem {
    pub lang: Option<String>,
    pub title: Option<String>,
    pub subtitle_lang: Option<String>,
    /// 制作方式。基线只认 `2 == AI 配音`。
    pub production_type: Option<i64>,
}

/// 可跳过片段。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ClipInfo {
    /// 起止（秒）。基线转成毫秒是在模型里做的，此处保持接口单位。
    pub start: Option<f64>,
    pub end: Option<f64>,
    /// `CLIP_TYPE_OP` / `CLIP_TYPE_ED` / 其他。
    pub clip_type: Option<String>,
}
