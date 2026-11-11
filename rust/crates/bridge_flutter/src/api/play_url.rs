//! 取流结果的 typed FRB 镜像（issue #79 / ADR-011）。
//!
//! 本模块只放**实体**，取数函数在 [`crate::api::play_url_fetch`]。两者
//! 分开是因为实体有十个嵌套类型、三百余行，混在取数模块里会把那条本就
//! 分支繁多的请求路径埋掉。
//!
//! ## 为什么是镜像而不是直接导出 `bili_domain::playurl`
//!
//! 同 S9 以来每一片的做法：`bili-domain` 不依赖 flutter_rust_bridge，
//! 领域层不该为了一个宿主而带上 codegen 的属性与约束。镜像的代价是一组
//! 平铺直叙的转换函数，收益是领域层能被其他宿主（CLI、测试）原样使用。
//!
//! ## 刻意保留的 `Option`
//!
//! 字段可空性逐条照抄领域层，不在这里收窄。取流响应的缺字段是常态
//! （试看片段没有 `dash`、老视频没有 `volume`、UGC 没有 `clip_info_list`），
//! 把它们填成默认值等于让 Dart 分不清「服务端没给」与「给了零」。
//!
//! 唯一的例外是 `last_play_time`，理由见领域层注释：基线把 null 与非正数
//! 一律归零，多一种「未知」只会让每个调用点各写一遍兜底。

use bili_domain::playurl as domain;

/// 一次取流的结果。对应被删除的基线 `PlayUrlModel`。
#[derive(Debug, Clone)]
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
    /// 上次播放进度（毫秒）。null 与非正数已在解析时归零。
    pub last_play_time: i64,
    pub last_play_cid: Option<i64>,
    /// 当前音轨语言（接口字段 `cur_language`）。
    pub cur_language: Option<String>,
    pub language: Option<LanguageOptions>,
    /// 片头片尾等可跳过片段（PGC）。
    pub clip_info_list: Option<Vec<ClipInfo>>,
}

/// 可选清晰度的描述。
#[derive(Debug, Clone)]
pub struct PlayFormat {
    pub quality: Option<i64>,
    pub format: Option<String>,
    /// 接口字段 `new_description`。
    pub new_desc: Option<String>,
    pub display_desc: Option<String>,
    pub codecs: Option<Vec<String>>,
}

/// DASH 流。
#[derive(Debug, Clone)]
pub struct Dash {
    pub duration: Option<i64>,
    pub min_buffer_time: Option<f64>,
    pub video: Option<Vec<MediaStream>>,
    /// 音频流。接口把无损、杜比与普通音轨放在三处，解析时已按
    /// flac → dolby → 普通的顺序合并成一个列表——顺序是行为的一部分
    /// （列表首项即默认音轨）。
    pub audio: Option<Vec<MediaStream>>,
}

/// 一路 DASH 流。视频与音频共用一种形状，差别只在哪些字段有值。
#[derive(Debug, Clone)]
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
#[derive(Debug, Clone)]
pub struct SegmentBase {
    pub initialization: Option<String>,
    pub index_range: Option<String>,
}

/// 整段流。
#[derive(Debug, Clone)]
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
/// 而「是否全为 0」正是要不要启用均衡的判据。
#[derive(Debug, Clone)]
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
#[derive(Debug, Clone)]
pub struct LanguageOptions {
    pub support: Option<bool>,
    /// 条目**未排序**：排序与「（AI）」标注是展示规则，留在 Dart。
    pub items: Option<Vec<LanguageItem>>,
}

/// 一条音轨。
#[derive(Debug, Clone)]
pub struct LanguageItem {
    pub lang: Option<String>,
    pub title: Option<String>,
    pub subtitle_lang: Option<String>,
    /// 制作方式。基线只认 `2 == AI 配音`。
    pub production_type: Option<i64>,
    /// 是否 AI 配音。解析时就判定好，宿主不再认那个魔数。
    pub is_ai: bool,
}

/// 可跳过片段。
#[derive(Debug, Clone)]
pub struct ClipInfo {
    /// 起止（秒）。转成毫秒是展示侧的事，此处保持接口单位。
    pub start: Option<f64>,
    pub end: Option<f64>,
    /// `CLIP_TYPE_OP` / `CLIP_TYPE_ED` / 其他。
    pub clip_type: Option<String>,
}

// ------------------------------------------------------------------ 转换

pub(crate) fn play_url(value: domain::PlayUrl) -> PlayUrl {
    PlayUrl {
        from: value.from,
        result: value.result,
        message: value.message,
        quality: value.quality,
        format: value.format,
        time_length: value.time_length,
        accept_format: value.accept_format,
        accept_description: value.accept_description,
        accept_quality: value.accept_quality,
        video_codecid: value.video_codecid,
        seek_param: value.seek_param,
        seek_type: value.seek_type,
        support_formats: value
            .support_formats
            .map(|v| v.into_iter().map(play_format).collect()),
        dash: value.dash.map(dash),
        durl: value.durl.map(|v| v.into_iter().map(durl).collect()),
        volume: value.volume.map(volume),
        last_play_time: value.last_play_time,
        last_play_cid: value.last_play_cid,
        cur_language: value.cur_language,
        language: value.language.map(language_options),
        clip_info_list: value
            .clip_info_list
            .map(|v| v.into_iter().map(clip_info).collect()),
    }
}

fn play_format(value: domain::PlayFormat) -> PlayFormat {
    PlayFormat {
        quality: value.quality,
        format: value.format,
        new_desc: value.new_desc,
        display_desc: value.display_desc,
        codecs: value.codecs,
    }
}

fn dash(value: domain::Dash) -> Dash {
    Dash {
        duration: value.duration,
        min_buffer_time: value.min_buffer_time,
        video: value
            .video
            .map(|v| v.into_iter().map(media_stream).collect()),
        audio: value
            .audio
            .map(|v| v.into_iter().map(media_stream).collect()),
    }
}

pub(crate) fn media_stream(value: domain::MediaStream) -> MediaStream {
    MediaStream {
        id: value.id,
        base_url: value.base_url,
        backup_url: value.backup_url,
        band_width: value.band_width,
        mime_type: value.mime_type,
        codecs: value.codecs,
        width: value.width,
        height: value.height,
        frame_rate: value.frame_rate,
        sar: value.sar,
        start_with_sap: value.start_with_sap,
        segment_base: value.segment_base.map(segment_base),
        codecid: value.codecid,
    }
}

fn segment_base(value: domain::SegmentBase) -> SegmentBase {
    SegmentBase {
        initialization: value.initialization,
        index_range: value.index_range,
    }
}

fn durl(value: domain::Durl) -> Durl {
    Durl {
        order: value.order,
        length: value.length,
        size: value.size,
        ahead: value.ahead,
        vhead: value.vhead,
        url: value.url,
        backup_url: value.backup_url,
    }
}

fn volume(value: domain::Volume) -> Volume {
    Volume {
        measured_i: value.measured_i,
        measured_lra: value.measured_lra,
        measured_tp: value.measured_tp,
        measured_threshold: value.measured_threshold,
        target_offset: value.target_offset,
        target_i: value.target_i,
        target_tp: value.target_tp,
    }
}

fn language_options(value: domain::LanguageOptions) -> LanguageOptions {
    LanguageOptions {
        support: value.support,
        items: value
            .items
            .map(|v| v.into_iter().map(language_item).collect()),
    }
}

fn language_item(value: domain::LanguageItem) -> LanguageItem {
    LanguageItem {
        lang: value.lang,
        title: value.title,
        subtitle_lang: value.subtitle_lang,
        production_type: value.production_type,
        is_ai: value.is_ai,
    }
}

fn clip_info(value: domain::ClipInfo) -> ClipInfo {
    ClipInfo {
        start: value.start,
        end: value.end,
        clip_type: value.clip_type,
    }
}

// ------------------------------------------------- 反向转换（宿主 → 领域）
//
// 只为起播决策那几个函数而设（见 `crate::api::playback`）：算法住在
// `bili-core`，而宿主手上拿着的是本模块的镜像类型，总得有一侧做转换。
// **不是**给宿主「构造协议数据」用的——那是 ADR-011 明确不做的事。

pub(crate) fn to_domain_stream(value: MediaStream) -> domain::MediaStream {
    domain::MediaStream {
        id: value.id,
        base_url: value.base_url,
        backup_url: value.backup_url,
        band_width: value.band_width,
        mime_type: value.mime_type,
        codecs: value.codecs,
        width: value.width,
        height: value.height,
        frame_rate: value.frame_rate,
        sar: value.sar,
        start_with_sap: value.start_with_sap,
        segment_base: value.segment_base.map(|v| domain::SegmentBase {
            initialization: v.initialization,
            index_range: v.index_range,
        }),
        codecid: value.codecid,
    }
}

pub(crate) fn to_domain_clip(value: ClipInfo) -> domain::ClipInfo {
    domain::ClipInfo {
        start: value.start,
        end: value.end,
        clip_type: value.clip_type,
    }
}
