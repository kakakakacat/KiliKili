//! playurl 四条接口路径的 DTO 与到领域实体的映射。
//!
//! 「四条」指基线 `VideoHttp.videoUrl` 里那个 switch 的四个分支
//! （`lib/http/video.dart`）：
//!
//! | 分支 | 接口 | 流数据在哪 | 进度在哪 |
//! |---|---|---|---|
//! | UGC | `/x/player/wbi/playurl` | `data` | `data.last_play_time` |
//! | PGC/PUGV 主 | `/ogv/player/playview` | `data.video_info` | `data.watch_progress` |
//! | PGC 降级 | `/pgc/player/web/v2/playurl` | `result.video_info` | `result.play_view_business_info…` |
//! | PUGV 降级 | `/pugv/player/web/playurl` | `data` | `data.play_view_business_info…` |
//!
//! 注意 PGC 降级接口的信封用的是 `result` 而非 `data`——不是笔误，是
//! 这条接口本来就与其他三条不同。
//!
//! DTO 一律 `Option` + `#[serde(default)]`，理由同 [`crate::video_detail`]：
//! 少一个字段不该让整次取流失败。

use bili_domain::error::{DomainError, ErrorKind};
use bili_domain::playurl::{
    ClipInfo, Dash, Durl, LanguageItem, LanguageOptions, MediaStream, PlayFormat, PlayUrl,
    SegmentBase, Volume,
};
use bili_domain::video::VideoType;
use serde::Deserialize;

/// 一次响应的解析结果。
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    /// 拿到了可播放的流。
    Ready(Box<PlayUrl>),
    /// 内容是 DRM 的，主接口给不出可播放流，应改走降级接口。
    ///
    /// 基线对此只重试一次（`arc.is_drm == true` → `fallback: true`）。
    /// 做成一个**值**而不是在这里直接重发：解析层不该持有传输，调用方
    /// 才知道这次是不是已经是降级请求了。
    NeedsFallback,
}

/// 解析一次 playurl 响应。
///
/// `video_type` 与 `fallback` 决定走上面表格里的哪一行——响应本身不足以
/// 区分（PUGV 降级与 UGC 的外层形状相同）。
///
/// # Errors
/// - 信封 code 非 0 → 按码分类，并带上原始 code；
/// - 主接口既无 `video_info` 又非 DRM → `not_playable_reason` 的码，
///   或 `Remote`；
/// - 结构不符 → `Protocol`。
pub fn parse(body: &str, video_type: VideoType, fallback: bool) -> Result<Outcome, DomainError> {
    match (video_type, fallback) {
        (VideoType::Ugc, _) => {
            let dto: PlayUrlDto = unwrap_data(body)?;
            Ok(Outcome::Ready(Box::new(dto.into_domain())))
        }
        (VideoType::Pgc | VideoType::Pugv, false) => {
            let view: PlayViewDto = unwrap_data(body)?;
            view.into_outcome()
        }
        (VideoType::Pgc, true) => {
            // 这条接口把业务数据放在 `result` 里。
            let dto: PgcFallbackDto = unwrap_result(body)?;
            Ok(Outcome::Ready(Box::new(dto.into_domain())))
        }
        (VideoType::Pugv, true) => {
            let dto: PugvFallbackDto = unwrap_data(body)?;
            Ok(Outcome::Ready(Box::new(dto.into_domain())))
        }
    }
}

/// 解开 `{"code":0,"data":{…}}`。
fn unwrap_data<T: serde::de::DeserializeOwned>(body: &str) -> Result<T, DomainError> {
    crate::envelope::unwrap_envelope(body)
}

/// 解开 `{"code":0,"result":{…}}`。
///
/// 与 [`crate::envelope::unwrap_envelope`] 的差别只在载荷的键名，错误
/// 分类完全一致——复制那套分类会让两处随时间漂移，所以先把 `result`
/// 改名成 `data` 再交给它。改名发生在已解析的 JSON 值上，不是字符串
/// 替换：响应里别处出现 `"result"`（`PlayUrlModel.result` 就是一个）
/// 不会被误伤。
fn unwrap_result<T: serde::de::DeserializeOwned>(body: &str) -> Result<T, DomainError> {
    let mut value: serde_json::Value = serde_json::from_str(body).map_err(|e| {
        DomainError::new(ErrorKind::Protocol, format!("响应不是预期的信封结构: {e}"))
    })?;

    if let Some(obj) = value.as_object_mut()
        && let Some(result) = obj.remove("result")
    {
        obj.insert("data".to_owned(), result);
    }

    crate::envelope::unwrap_envelope(&value.to_string())
}

// ── 主接口（playview）外层 ────────────────────────────────────────

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct PlayViewDto {
    video_info: Option<PlayUrlDto>,
    watch_progress: Option<WatchProgressDto>,
    arc: Option<ArcDto>,
    not_playable_reason: Option<ReasonDto>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct WatchProgressDto {
    current_watch_progress: Option<i64>,
    /// 同一个意思的第二个键名。基线取的是
    /// `current_watch_progress ?? current_progress`，顺序不能换。
    current_progress: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ArcDto {
    is_drm: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ReasonDto {
    code: Option<i64>,
    message: Option<String>,
}

impl PlayViewDto {
    fn into_outcome(self) -> Result<Outcome, DomainError> {
        if let Some(info) = self.video_info {
            let mut play = info.into_domain();
            if let Some(p) = self.watch_progress {
                // 基线：外层进度覆盖流里的 last_play_time，取值走同一个
                // 归一（null / 非正数 → 0）。
                play.last_play_time =
                    normalize_progress(p.current_watch_progress.or(p.current_progress));
            }
            return Ok(Outcome::Ready(Box::new(play)));
        }

        if self.arc.and_then(|a| a.is_drm) == Some(true) {
            return Ok(Outcome::NeedsFallback);
        }

        if let Some(reason) = self.not_playable_reason {
            let code = reason.code.unwrap_or(0);
            let message = reason.message.unwrap_or_default();
            return Err(DomainError::with_code(
                crate::envelope::kind_for_code(code),
                format!("不可播放 code {code}: {message}"),
                code,
            ));
        }

        // 基线在这里是 `Error(result.toString())`——把整个 JSON 丢给 UI。
        // 不照搬：那串东西既看不懂也可能含用户信息，而 code 已经是 0，
        // 真正的信号是「成功响应里没有流」，这正是 Protocol 的定义。
        Err(DomainError::new(
            ErrorKind::Protocol,
            "playview 响应既无 video_info 也无不可播放原因",
        ))
    }
}

// ── 降级接口外层 ──────────────────────────────────────────────────

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct PgcFallbackDto {
    video_info: Option<PlayUrlDto>,
    play_view_business_info: Option<BusinessInfoDto>,
}

impl PgcFallbackDto {
    fn into_domain(self) -> PlayUrl {
        let mut play = self.video_info.unwrap_or_default().into_domain();
        play.last_play_time = normalize_progress(
            self.play_view_business_info
                .and_then(BusinessInfoDto::watch_progress),
        );
        play
    }
}

/// PUGV 降级接口：流就在 `data` 里，进度另在 `data` 的业务信息里。
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct PugvFallbackDto {
    #[serde(flatten)]
    play: PlayUrlDto,
    play_view_business_info: Option<BusinessInfoDto>,
}

impl PugvFallbackDto {
    fn into_domain(self) -> PlayUrl {
        let mut play = self.play.into_domain();
        play.last_play_time = normalize_progress(
            self.play_view_business_info
                .and_then(BusinessInfoDto::watch_progress),
        );
        play
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct BusinessInfoDto {
    user_status: Option<UserStatusDto>,
}

impl BusinessInfoDto {
    fn watch_progress(self) -> Option<i64> {
        self.user_status?.watch_progress?.current_watch_progress
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct UserStatusDto {
    watch_progress: Option<WatchProgressDto>,
}

// ── 流本体 ────────────────────────────────────────────────────────

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct PlayUrlDto {
    from: Option<String>,
    result: Option<String>,
    message: Option<String>,
    quality: Option<i64>,
    format: Option<String>,
    timelength: Option<i64>,
    accept_format: Option<String>,
    accept_description: Option<Vec<String>>,
    accept_quality: Option<Vec<i64>>,
    video_codecid: Option<i64>,
    seek_param: Option<String>,
    seek_type: Option<String>,
    support_formats: Option<Vec<FormatDto>>,
    dash: Option<DashDto>,
    durl: Option<Vec<DurlDto>>,
    volume: Option<VolumeDto>,
    last_play_time: Option<i64>,
    last_play_cid: Option<i64>,
    cur_language: Option<String>,
    language: Option<LanguageDto>,
    clip_info_list: Option<Vec<ClipDto>>,
}

impl PlayUrlDto {
    fn into_domain(self) -> PlayUrl {
        PlayUrl {
            from: self.from,
            result: self.result,
            message: self.message,
            quality: self.quality,
            format: self.format,
            time_length: self.timelength,
            accept_format: self.accept_format,
            accept_description: self.accept_description,
            accept_quality: self.accept_quality,
            video_codecid: self.video_codecid,
            seek_param: self.seek_param,
            seek_type: self.seek_type,
            support_formats: self
                .support_formats
                .map(|v| v.into_iter().map(FormatDto::into_domain).collect()),
            dash: self.dash.map(DashDto::into_domain),
            durl: self
                .durl
                .map(|v| v.into_iter().map(DurlDto::into_domain).collect()),
            volume: self.volume.map(VolumeDto::into_domain),
            last_play_time: normalize_progress(self.last_play_time),
            last_play_cid: self.last_play_cid,
            cur_language: self.cur_language,
            language: self.language.map(LanguageDto::into_domain),
            clip_info_list: self
                .clip_info_list
                .map(|v| v.into_iter().map(ClipDto::into_domain).collect()),
        }
    }
}

/// 进度归一。基线 `PlayUrlModel.lastPlayTime` 的 setter：null 与非正数
/// 一律为 0。
const fn normalize_progress(value: Option<i64>) -> i64 {
    match value {
        Some(v) if v > 0 => v,
        _ => 0,
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct FormatDto {
    quality: Option<i64>,
    format: Option<String>,
    new_description: Option<String>,
    display_desc: Option<String>,
    codecs: Option<Vec<String>>,
}

impl FormatDto {
    fn into_domain(self) -> PlayFormat {
        PlayFormat {
            quality: self.quality,
            format: self.format,
            new_desc: self.new_description,
            display_desc: self.display_desc,
            codecs: self.codecs,
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct DashDto {
    duration: Option<i64>,
    #[serde(rename = "minBufferTime")]
    min_buffer_time: Option<f64>,
    video: Option<Vec<StreamDto>>,
    audio: Option<Vec<StreamDto>>,
    flac: Option<FlacDto>,
    dolby: Option<DolbyDto>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct FlacDto {
    audio: Option<StreamDto>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct DolbyDto {
    audio: Option<Vec<StreamDto>>,
}

impl DashDto {
    fn into_domain(self) -> Dash {
        // 合并顺序 flac → dolby → 普通，与基线 `Dash.fromJson` 一致。
        // 顺序即优先级：调用方取首项作为默认音轨。
        let mut audio: Vec<MediaStream> = Vec::new();
        if let Some(flac) = self.flac.and_then(|f| f.audio) {
            audio.push(flac.into_domain());
        }
        if let Some(dolby) = self.dolby.and_then(|d| d.audio) {
            audio.extend(dolby.into_iter().map(StreamDto::into_domain));
        }
        if let Some(plain) = self.audio {
            audio.extend(plain.into_iter().map(StreamDto::into_domain));
        }

        Dash {
            duration: self.duration,
            min_buffer_time: self.min_buffer_time,
            video: self
                .video
                .map(|v| v.into_iter().map(StreamDto::into_domain).collect()),
            // 基线：三处都空时整个 audio 为 null，而不是空列表——UI 据此
            // 区分「没有音轨」与「音轨列表为空」。
            audio: if audio.is_empty() { None } else { Some(audio) },
        }
    }
}

/// 一路流。
///
/// 接口对同一个值**同时**给出两种键名：真实响应里 `baseUrl` 与
/// `base_url`、`frameRate` 与 `frame_rate`、`SegmentBase` 与
/// `segment_base` 是并存的（2026-11-10 实测）。因此不能用 serde 的
/// `alias`——两个键同时出现会被判为 `duplicate field` 而整条响应解析
/// 失败。各留一个字段、按基线 `BaseItem.fromJson` 的 `??` 顺序取用，
/// 优先 camelCase。
///
/// 这正是 shadow mode 抓到的第一个差异：fixture 里不会凭空写出两份
/// 键名，只有真实响应才有。
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct StreamDto {
    id: Option<i64>,
    #[serde(rename = "baseUrl")]
    base_url_camel: Option<String>,
    base_url: Option<String>,
    #[serde(rename = "backupUrl")]
    backup_url_camel: Option<Vec<String>>,
    backup_url: Option<Vec<String>>,
    #[serde(rename = "bandWidth")]
    band_width_camel: Option<i64>,
    bandwidth: Option<i64>,
    mime_type: Option<String>,
    codecs: Option<String>,
    width: Option<i64>,
    height: Option<i64>,
    #[serde(rename = "frameRate")]
    frame_rate_camel: Option<String>,
    frame_rate: Option<String>,
    sar: Option<String>,
    #[serde(rename = "startWithSap")]
    start_with_sap_camel: Option<i64>,
    start_with_sap: Option<i64>,
    #[serde(rename = "segmentBase")]
    segment_base_camel: Option<SegmentBaseDto>,
    #[serde(rename = "SegmentBase")]
    segment_base_pascal: Option<SegmentBaseDto>,
    segment_base: Option<SegmentBaseDto>,
    codecid: Option<i64>,
}

impl StreamDto {
    fn into_domain(self) -> MediaStream {
        MediaStream {
            // 基线把 id 声明成非空 `late int`，缺失时整次解析抛异常。
            // 这里用 0 兜底而不是报错：0 不是任何清晰度代码，上层据此
            // 看到一路「未知清晰度」的流，而不是整个视频播不了。
            id: self.id.unwrap_or(0),
            base_url: self.base_url_camel.or(self.base_url),
            backup_url: self.backup_url_camel.or(self.backup_url),
            band_width: self.band_width_camel.or(self.bandwidth),
            mime_type: self.mime_type,
            codecs: self.codecs,
            width: self.width,
            height: self.height,
            frame_rate: self.frame_rate_camel.or(self.frame_rate),
            sar: self.sar,
            start_with_sap: self.start_with_sap_camel.or(self.start_with_sap),
            // `SegmentBase` 只在 DASH 内层出现，基线不读它；放在最后，
            // 仅当另两种写法都缺失时才用。
            segment_base: self
                .segment_base_camel
                .or(self.segment_base)
                .or(self.segment_base_pascal)
                .map(SegmentBaseDto::into_domain),
            codecid: self.codecid,
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SegmentBaseDto {
    #[serde(rename = "Initialization")]
    initialization_pascal: Option<String>,
    initialization: Option<String>,
    #[serde(rename = "indexRange")]
    index_range_camel: Option<String>,
    index_range: Option<String>,
}

impl SegmentBaseDto {
    /// 同样是并存的两种键名：`SegmentBase` 里是 `Initialization` /
    /// `indexRange`，`segment_base` 里是 `initialization` /
    /// `index_range`。取法与基线一致：谁在就用谁。
    fn into_domain(self) -> SegmentBase {
        SegmentBase {
            initialization: self.initialization_pascal.or(self.initialization),
            index_range: self.index_range_camel.or(self.index_range),
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct DurlDto {
    order: Option<i64>,
    length: Option<i64>,
    size: Option<i64>,
    ahead: Option<String>,
    vhead: Option<String>,
    url: Option<String>,
    backup_url: Option<Vec<String>>,
}

impl DurlDto {
    fn into_domain(self) -> Durl {
        Durl {
            order: self.order,
            length: self.length,
            size: self.size,
            ahead: self.ahead,
            vhead: self.vhead,
            url: self.url,
            backup_url: self.backup_url,
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct VolumeDto {
    measured_i: Option<f64>,
    measured_lra: Option<f64>,
    measured_tp: Option<f64>,
    measured_threshold: Option<f64>,
    target_offset: Option<f64>,
    target_i: Option<f64>,
    target_tp: Option<f64>,
}

impl VolumeDto {
    fn into_domain(self) -> Volume {
        Volume {
            measured_i: self.measured_i.unwrap_or(0.0),
            measured_lra: self.measured_lra.unwrap_or(0.0),
            measured_tp: self.measured_tp.unwrap_or(0.0),
            measured_threshold: self.measured_threshold.unwrap_or(0.0),
            target_offset: self.target_offset.unwrap_or(0.0),
            target_i: self.target_i.unwrap_or(0.0),
            target_tp: self.target_tp.unwrap_or(0.0),
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct LanguageDto {
    support: Option<bool>,
    items: Option<Vec<LanguageItemDto>>,
}

impl LanguageDto {
    fn into_domain(self) -> LanguageOptions {
        LanguageOptions {
            support: self.support,
            items: self.items.map(|v| {
                let mut items: Vec<LanguageItem> =
                    v.into_iter().map(LanguageItemDto::into_domain).collect();
                // 中文在前，AI 配音在后。`sort_by` 是稳定排序，同级条目
                // 保持协议顺序——基线用的 Dart `List.sort` 不保证这点，
                // 稳定是更强的保证，不是行为差异。
                items.sort_by(|a, b| {
                    let a_zh = a.lang.as_deref().is_some_and(|l| l.contains("zh"));
                    let b_zh = b.lang.as_deref().is_some_and(|l| l.contains("zh"));
                    b_zh.cmp(&a_zh).then(a.is_ai.cmp(&b.is_ai))
                });
                items
            }),
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct LanguageItemDto {
    lang: Option<String>,
    title: Option<String>,
    subtitle_lang: Option<String>,
    production_type: Option<i64>,
}

impl LanguageItemDto {
    fn into_domain(self) -> LanguageItem {
        LanguageItem {
            lang: self.lang,
            title: self.title,
            subtitle_lang: self.subtitle_lang,
            production_type: self.production_type,
            is_ai: self.production_type == Some(2),
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ClipDto {
    start: Option<f64>,
    end: Option<f64>,
    #[serde(rename = "clipType", alias = "clip_type")]
    clip_type: Option<String>,
}

impl ClipDto {
    fn into_domain(self) -> ClipInfo {
        ClipInfo {
            start: self.start,
            end: self.end,
            clip_type: self.clip_type,
        }
    }
}

#[cfg(test)]
// 测试里 expect 是正确做法：前提不成立时就该崩掉并指出原因。
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    fn ready(outcome: Outcome) -> PlayUrl {
        match outcome {
            Outcome::Ready(p) => *p,
            Outcome::NeedsFallback => panic!("期望拿到流，却被要求降级"),
        }
    }

    #[test]
    fn ugc_stream_is_parsed_from_data() {
        let body = r#"{"code":0,"data":{
            "quality":80,"timelength":123456,"accept_quality":[80,64],
            "accept_description":["高清 1080P","高清 720P"],
            "dash":{"duration":120,"minBufferTime":1.5,
                "video":[{"id":80,"baseUrl":"https://cdn.invalid/v","bandWidth":100,
                          "codecid":7,"segmentBase":{"Initialization":"0-100","indexRange":"101-200"}}],
                "audio":[{"id":30280,"base_url":"https://cdn.invalid/a"}]}}}"#;
        let play = ready(parse(body, VideoType::Ugc, false).expect("应当解析成功"));

        assert_eq!(play.quality, Some(80));
        assert_eq!(play.time_length, Some(123_456));
        let dash = play.dash.expect("dash 应存在");
        let video = dash.video.expect("video 应存在");
        assert_eq!(video[0].id, 80);
        assert_eq!(video[0].band_width, Some(100), "camelCase 键名必须被认出");
        let seg = video[0].segment_base.as_ref().expect("segmentBase 应存在");
        assert_eq!(seg.initialization.as_deref(), Some("0-100"));
        let audio = dash.audio.expect("audio 应存在");
        assert_eq!(audio[0].base_url.as_deref(), Some("https://cdn.invalid/a"));
    }

    #[test]
    fn audio_tracks_are_merged_in_baseline_order() {
        // 基线 Dash.fromJson：flac → dolby → 普通。顺序即优先级，调用方
        // 取首项作默认音轨。
        let body = r#"{"code":0,"data":{"dash":{
            "audio":[{"id":30280}],
            "flac":{"audio":{"id":30251}},
            "dolby":{"audio":[{"id":30250}]}}}}"#;
        let play = ready(parse(body, VideoType::Ugc, false).expect("应当解析成功"));
        let ids: Vec<i64> = play
            .dash
            .expect("dash")
            .audio
            .expect("audio")
            .iter()
            .map(|a| a.id)
            .collect();
        assert_eq!(ids, [30251, 30250, 30280]);
    }

    #[test]
    fn absent_audio_everywhere_stays_none() {
        // 「没有音轨」与「音轨列表为空」在 UI 上是两件事。
        let body = r#"{"code":0,"data":{"dash":{"video":[]}}}"#;
        let play = ready(parse(body, VideoType::Ugc, false).expect("应当解析成功"));
        assert!(play.dash.expect("dash").audio.is_none());
    }

    #[test]
    fn non_positive_progress_is_zeroed() {
        for raw in ["null", "0", "-5"] {
            let body = format!(r#"{{"code":0,"data":{{"last_play_time":{raw}}}}}"#);
            let play = ready(parse(&body, VideoType::Ugc, false).expect("应当解析成功"));
            assert_eq!(play.last_play_time, 0, "基线 setter 把 {raw} 归零");
        }
    }

    #[test]
    fn playview_progress_overrides_the_stream_value() {
        let body = r#"{"code":0,"data":{
            "video_info":{"quality":80,"last_play_time":100},
            "watch_progress":{"current_watch_progress":4200}}}"#;
        let play = ready(parse(body, VideoType::Pgc, false).expect("应当解析成功"));
        assert_eq!(play.last_play_time, 4200);
    }

    #[test]
    fn playview_progress_falls_back_to_current_progress() {
        // 基线取的是 current_watch_progress ?? current_progress。
        let body = r#"{"code":0,"data":{
            "video_info":{"quality":80},
            "watch_progress":{"current_progress":77}}}"#;
        let play = ready(parse(body, VideoType::Pgc, false).expect("应当解析成功"));
        assert_eq!(play.last_play_time, 77);
    }

    #[test]
    fn drm_content_asks_for_fallback() {
        let body = r#"{"code":0,"data":{"arc":{"is_drm":true}}}"#;
        assert_eq!(
            parse(body, VideoType::Pgc, false).expect("应当解析成功"),
            Outcome::NeedsFallback
        );
    }

    #[test]
    fn not_playable_reason_carries_its_code() {
        let body =
            r#"{"code":0,"data":{"not_playable_reason":{"code":87008,"message":"需包月充电"}}}"#;
        let err = parse(body, VideoType::Pgc, false).expect_err("不可播放必须失败");
        assert_eq!(err.code(), Some(87008), "文案与分支都依赖原始码");
        assert!(err.detail().contains("需包月充电"));
    }

    #[test]
    fn playview_without_stream_or_reason_is_protocol() {
        // code 为 0 却没有流，是我们对接口的理解过时了。
        let body = r#"{"code":0,"data":{}}"#;
        let err = parse(body, VideoType::Pgc, false).expect_err("必须失败");
        assert_eq!(err.kind(), ErrorKind::Protocol);
    }

    #[test]
    fn pgc_fallback_reads_the_result_envelope() {
        // 这条接口的载荷在 `result` 里，不是 `data`。
        let body = r#"{"code":0,"result":{
            "video_info":{"quality":112,"result":"suee"},
            "play_view_business_info":{"user_status":{"watch_progress":{"current_watch_progress":999}}}}}"#;
        let play = ready(parse(body, VideoType::Pgc, true).expect("应当解析成功"));
        assert_eq!(play.quality, Some(112));
        assert_eq!(play.last_play_time, 999);
        assert_eq!(
            play.result.as_deref(),
            Some("suee"),
            "流里同名的 result 字段不得被信封改名误伤"
        );
    }

    #[test]
    fn pugv_fallback_reads_the_stream_from_data_itself() {
        let body = r#"{"code":0,"data":{
            "quality":64,"durl":[{"order":1,"url":"https://cdn.invalid/x","backup_url":["https://b.invalid/x"]}],
            "play_view_business_info":{"user_status":{"watch_progress":{"current_watch_progress":12}}}}}"#;
        let play = ready(parse(body, VideoType::Pugv, true).expect("应当解析成功"));
        assert_eq!(play.quality, Some(64));
        assert_eq!(play.last_play_time, 12);
        let durl = play.durl.expect("durl 应存在");
        assert_eq!(durl[0].backup_url.as_deref().map(<[_]>::len), Some(1));
    }

    #[test]
    fn business_code_is_classified_and_kept() {
        let body = r#"{"code":-404,"message":"啥都木有","data":null}"#;
        let err = parse(body, VideoType::Ugc, false).expect_err("非 0 码必须失败");
        assert_eq!(err.code(), Some(-404));
        assert_eq!(err.kind(), ErrorKind::Remote);
    }

    #[test]
    fn volume_defaults_to_zero_per_field() {
        // 基线对每个字段都用了 `?? 0`，而 Volume.isNotEmpty 正是靠
        // 「是否全为 0」决定要不要启用均衡。
        let body = r#"{"code":0,"data":{"volume":{"measured_i":-14.5}}}"#;
        let play = ready(parse(body, VideoType::Ugc, false).expect("应当解析成功"));
        let v = play.volume.expect("volume 应存在");
        assert!((v.measured_i - -14.5).abs() < f64::EPSILON);
        assert!((v.target_tp - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn language_items_are_sorted_for_display() {
        // 中文在前、AI 配音在后。顺序在这一侧定，宿主只负责画。
        let body = r#"{"code":0,"data":{"language":{"support":true,"items":[
            {"lang":"ja","title":"日语","production_type":1},
            {"lang":"zh","title":"国语AI","production_type":2},
            {"lang":"zh","title":"国语","production_type":1}]}}}"#;
        let play = ready(parse(body, VideoType::Ugc, false).expect("应当解析成功"));
        let items = play.language.expect("language").items.expect("items");
        assert_eq!(items[0].title.as_deref(), Some("国语"));
        assert!(!items[0].is_ai);
        assert_eq!(items[1].title.as_deref(), Some("国语AI"));
        assert!(items[1].is_ai);
        assert_eq!(items[2].lang.as_deref(), Some("ja"));
    }

    #[test]
    fn malformed_body_is_a_protocol_error() {
        let err = parse("not json", VideoType::Ugc, false).expect_err("必须失败");
        assert_eq!(err.kind(), ErrorKind::Protocol);
        let err = parse("not json", VideoType::Pgc, true).expect_err("必须失败");
        assert_eq!(err.kind(), ErrorKind::Protocol);
    }
}
