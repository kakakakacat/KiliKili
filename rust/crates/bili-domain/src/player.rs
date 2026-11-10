//! 播放器后端契约。真实播放状态只由后端报告；Rust 不自行推进时钟。
//!
//! Phase 1 的 media-kit 与未来的原生后端实现同一端口。命令和事件都带
//! `session_id`/`generation`，消费事件前用 [`Stamped::accept`] 丢弃旧代际。
//! 高频 position 由适配器合并后发快照，不逐帧跨 FFI。

use std::fmt;

use crate::redact::Redacted;
use crate::session::Stamped;

/// 播放器实际使用的媒体资源。DASH 音视频分轨，不在领域层编码 mpv EDL。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediaSource {
    Dash {
        video: Redacted<String>,
        audio: Redacted<String>,
    },
    /// 旧格式的一个或多个顺序片段；适配器决定如何拼接。
    Segments(Vec<Redacted<String>>),
}

/// 传给播放器的请求头。日志输出始终脱敏，避免签名 URL 或请求信息泄漏。
#[derive(Clone, PartialEq, Eq)]
pub struct MediaHeaders {
    pub user_agent: String,
    pub referer: String,
}

impl fmt::Debug for MediaHeaders {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("MediaHeaders(<redacted>)")
    }
}

/// UI/平台创建的渲染表面标识。具体句柄只保存在平台适配器中。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SurfaceId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lifecycle {
    Foreground,
    Background,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TrackId(pub i64);

/// Rust 发出的意图。Seek 和切换资源前应先 bump 代际。
#[derive(Debug, Clone, PartialEq)]
pub enum PlayerCommand {
    Open {
        source: MediaSource,
        headers: MediaHeaders,
    },
    Play,
    Pause,
    Seek {
        position_ms: u64,
    },
    SetSpeed(f32),
    SelectTracks {
        video: Option<TrackId>,
        audio: Option<TrackId>,
    },
    AttachSurface(SurfaceId),
    DetachSurface,
    SetLifecycle(Lifecycle),
    Close,
}

/// 后端报告的播放状态，而不是 Rust 推测的状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackState {
    Idle,
    Opening,
    Playing,
    Paused,
    Buffering,
    Ended,
}

/// 后端权威快照。弹幕以 position_ms 为时钟；缓冲时不得外推位置。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayerSnapshot {
    pub position_ms: u64,
    pub duration_ms: Option<u64>,
    pub state: PlaybackState,
    pub speed: f32,
    pub video_track: Option<TrackId>,
    pub audio_track: Option<TrackId>,
}

/// 与播放器具体日志文本无关的失败类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerFailureKind {
    Network,
    AccessDenied,
    Decode,
    UnsupportedSource,
    Backend,
    Cancelled,
}

/// 后端事件。Seek 完成是事实事件，不因发送 Seek 命令就假定完成。
#[derive(Debug, Clone, PartialEq)]
pub enum PlaybackEvent {
    Snapshot(PlayerSnapshot),
    SeekCompleted { position_ms: u64 },
    Failed(PlayerFailureKind),
    SurfaceLost,
    Closed,
}

/// 后端事件接收端。具体传输可以是 Dart Stream、Rust channel 或平台回调。
/// 适配器应先合并高频 position，再通过此接口发送权威快照。
pub trait PlayerEventSink: Send {
    fn emit(&mut self, event: Stamped<PlaybackEvent>);
}

/// 平台播放器的最小命令端口。适配器通过 [`PlayerEventSink`] 发事件，
/// 调用方按同一代际闸门过滤。
pub trait PlayerPort: Send {
    type Error;

    fn dispatch(&mut self, command: Stamped<PlayerCommand>) -> Result<(), Self::Error>;
}
