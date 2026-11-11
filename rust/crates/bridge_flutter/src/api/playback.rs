//! 起播决策的 Dart 导出：补档、CDN 选路、音量均衡、片头片尾（issue #79）。
//!
//! 这些规则此前住在基线 Dart 里（`PlayUrlModel` 的派生 getter、
//! `VideoUtils.getCdnUrl`、`Volume.format`）。按 RFC §2.3 的判据，它们
//! 换掉宿主框架时一个字都不会变，所以全部搬到 `bili-core`，这里只负责
//! 把它们接出去。
//!
//! ## 全部 `#[frb(sync)]`
//!
//! 纯计算、没有 I/O、几十条记录，而基线那几个调用点全是同步的（起播
//! 路径里内联拼地址）。生成成异步只会逼着起播与下载两条链路都改成
//! 异步，换不来任何东西。

use flutter_rust_bridge::frb;

use bili_core::cdn::{CDN_SERVICES, CdnService, pick_cdn_url as core_pick_cdn_url};
use bili_core::playback;

use crate::api::play_url::{ClipInfo, MediaStream, Volume, to_domain_clip, to_domain_stream};
use crate::api::sponsor_block::SkipSegment;

// ------------------------------------------------------------------ 补档

/// 低于最高档、但 DASH 里没给出的最高一档清晰度。没有可补的则为 `None`。
///
/// 入参刻意是**两组 id** 而不是整份取流结果：算法只看档位号，送整棵结构
/// 过来等于为两个数字拷贝二十余个嵌套类型。
#[frb(sync)]
#[must_use]
pub fn missing_video_quality_below_highest(
    video_ids: Vec<i64>,
    support_qualities: Vec<i64>,
) -> Option<i64> {
    // 只用到 id，拿最小代价拼出算法要的形状。
    let videos: Vec<bili_domain::playurl::MediaStream> =
        video_ids.into_iter().map(stream_with_id).collect();
    playback::missing_video_quality_below_highest(&videos, &support_qualities)
}

fn stream_with_id(id: i64) -> bili_domain::playurl::MediaStream {
    bili_domain::playurl::MediaStream {
        id,
        base_url: None,
        backup_url: None,
        band_width: None,
        mime_type: None,
        codecs: None,
        width: None,
        height: None,
        frame_rate: None,
        sar: None,
        start_with_sap: None,
        segment_base: None,
        codecid: None,
    }
}

/// 把补档那次取流的视频流并进原列表，按清晰度从高到低排好。
///
/// 这一条必须送整份流：合并结果要原样回到播放器，`base_url` 之类都得在。
#[frb(sync)]
#[must_use]
pub fn merge_video_streams(base: Vec<MediaStream>, extra: Vec<MediaStream>) -> Vec<MediaStream> {
    let base = base.into_iter().map(to_domain_stream).collect();
    let extra: Vec<_> = extra.into_iter().map(to_domain_stream).collect();
    playback::merge_video_streams(base, &extra)
        .into_iter()
        .map(crate::api::play_url::media_stream)
        .collect()
}

// ------------------------------------------------------------------ CDN

/// 一个可选 CDN。`host` 为空的两项不是真 CDN，是「怎么挑」的指令。
#[derive(Debug, Clone)]
pub struct CdnServiceEntry {
    /// 持久化用的标识，也是 [`pick_cdn_url`] 的入参。
    pub key: String,
    /// 下拉框展示名。
    pub desc: String,
    pub host: Option<String>,
}

/// 可选 CDN 的全表，**顺序即下拉框顺序**。
///
/// 宿主不要再抄一份：host 与展示名必须一一对应，抄两份就会有一份过期。
#[frb(sync)]
#[must_use]
pub fn cdn_services() -> Vec<CdnServiceEntry> {
    CDN_SERVICES
        .iter()
        .map(|(key, desc, host)| CdnServiceEntry {
            key: (*key).to_owned(),
            desc: (*desc).to_owned(),
            host: host.map(str::to_owned),
        })
        .collect()
}

/// 从候选地址里挑一个真正要播的。
///
/// `urls` 的顺序即协议顺序（主地址在前、备用在后）；`service` 取
/// [`cdn_services`] 给出的 `key`。候选为空时返回空串。
#[frb(sync)]
#[must_use]
pub fn pick_cdn_url(
    urls: Vec<String>,
    service: String,
    is_audio: bool,
    disable_audio_cdn: bool,
) -> String {
    core_pick_cdn_url(
        &urls,
        CdnService::from_key(&service),
        is_audio,
        disable_audio_cdn,
    )
}

/// 一条流的可播地址候选顺序：主地址在前，备用地址依次在后。
///
/// 与 [`pick_cdn_url`] 分开导出，是因为下载那条路径要的是**全部候选**
/// （一个地址取不到就换下一个），播放那条路径要的是**挑好的一个**。
#[frb(sync)]
#[must_use]
pub fn stream_play_urls(stream: MediaStream) -> Vec<String> {
    let mut urls = Vec::new();
    if let Some(base) = stream.base_url.filter(|s| !s.is_empty()) {
        urls.push(base);
    }
    if let Some(backup) = stream.backup_url {
        urls.extend(backup.into_iter().filter(|s| !s.is_empty()));
    }
    urls
}

/// [`stream_play_urls`] 的整段流版本。`durl` 与 DASH 流的字段名不同
/// （`url` / `backup_url`），规则一样。
#[frb(sync)]
#[must_use]
pub fn durl_play_urls(url: Option<String>, backup_url: Option<Vec<String>>) -> Vec<String> {
    let mut urls = Vec::new();
    if let Some(url) = url.filter(|s| !s.is_empty()) {
        urls.push(url);
    }
    if let Some(backup) = backup_url {
        urls.extend(backup.into_iter().filter(|s| !s.is_empty()));
    }
    urls
}

// ------------------------------------------------------------------ 音量

/// loudnorm 的配置项。缺省项由服务端给的测量值补。
#[derive(Debug, Clone, Copy)]
pub struct LoudnormConfig {
    pub lra: Option<f64>,
    pub i: Option<f64>,
    pub tp: Option<f64>,
    pub offset: Option<f64>,
}

/// 拼出 FFmpeg `loudnorm` 滤镜的参数串。
#[frb(sync)]
#[must_use]
pub fn loudnorm_params(volume: Volume, config: LoudnormConfig) -> String {
    playback::loudnorm_params(
        &to_domain_volume(volume),
        &playback::LoudnormConfig {
            lra: config.lra,
            i: config.i,
            tp: config.tp,
            offset: config.offset,
        },
    )
}

/// 服务端是否给了有效的均衡参数。全为 0 即视为没给。
#[frb(sync)]
#[must_use]
pub fn has_volume_measurement(volume: Volume) -> bool {
    playback::has_volume_measurement(&to_domain_volume(volume))
}

fn to_domain_volume(v: Volume) -> bili_domain::playurl::Volume {
    bili_domain::playurl::Volume {
        measured_i: v.measured_i,
        measured_lra: v.measured_lra,
        measured_tp: v.measured_tp,
        measured_threshold: v.measured_threshold,
        target_offset: v.target_offset,
        target_i: v.target_i,
        target_tp: v.target_tp,
    }
}

// ------------------------------------------------------------------ 片段

/// PGC 的片头片尾 → 可跳过片段。
///
/// 返回的是 SponsorBlock 那条路径同一个类型：番剧的片头片尾与用户提交的
/// 片段在播放器里走同一套跳过逻辑，宿主因此只需要一条消费路径。
#[frb(sync)]
#[must_use]
pub fn pgc_clip_segments(clips: Vec<ClipInfo>, time_length_ms: Option<i64>) -> Vec<SkipSegment> {
    let clips: Vec<_> = clips.into_iter().map(to_domain_clip).collect();
    playback::pgc_clip_segments(&clips, time_length_ms)
        .into_iter()
        .map(SkipSegment::from_domain)
        .collect()
}
