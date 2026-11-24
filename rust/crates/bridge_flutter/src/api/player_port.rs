//! Player Port 的 typed 传输面（#4）。所有决定在 bili-core；此处不持平台句柄。

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use bili_core::media_kit_port::{self as core, MediaKitPort, Observation, ObservationKind};
use bili_domain::player::{
    Lifecycle, MediaHeaders, PlaybackState, PlayerCommand, PlayerFailureKind, SurfaceId, TrackId,
};
use bili_domain::session::{Generation, SessionId};
use flutter_rust_bridge::frb;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerIntentKind {
    Open,
    Play,
    Pause,
    Seek,
    SetSpeed,
    SelectTracks,
    AttachSurface,
    DetachSurface,
    SetLifecycle,
}

#[derive(Clone)]
pub struct PlayerIntent {
    pub kind: PlayerIntentKind,
    pub video: Option<String>,
    pub audio: Option<String>,
    pub user_agent: Option<String>,
    pub referer: Option<String>,
    pub position_ms: Option<u64>,
    pub speed: Option<f32>,
    pub video_track: Option<i64>,
    pub audio_track: Option<i64>,
    pub surface_id: Option<u64>,
    pub foreground: Option<bool>,
    pub audio_only: Option<bool>,
    pub is_live: Option<bool>,
    pub is_file: Option<bool>,
}

// 意图含签名 URL 和请求头，Debug 不展开它们。
impl std::fmt::Debug for PlayerIntent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PlayerIntent(<redacted>)")
    }
}

#[derive(Clone)]
pub struct PlayerCommandTicket {
    pub session_id: u64,
    pub generation: u64,
    pub intent: PlayerIntent,
    pub media_uri: Option<String>,
}
impl std::fmt::Debug for PlayerCommandTicket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PlayerCommandTicket")
            .field("session_id", &self.session_id)
            .field("generation", &self.generation)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerObservationKind {
    StartFile,
    FileLoaded,
    Seek,
    PlaybackRestart,
    EndFile,
    Overflow,
    Position,
    Duration,
    Paused,
    Buffering,
    Ended,
    Speed,
    VideoTrack,
    AudioTrack,
    Buffer,
    Error,
}
#[derive(Clone)]
pub struct PlayerObservation {
    pub kind: PlayerObservationKind,
    pub value: f64,
    pub text: Option<String>,
    pub generation: Option<u64>,
}
impl std::fmt::Debug for PlayerObservation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PlayerObservation")
            .field("kind", &self.kind)
            .field("value", &self.value)
            .finish_non_exhaustive()
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerPortState {
    Idle,
    Opening,
    Playing,
    Paused,
    Buffering,
    Ended,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerPortFailure {
    Network,
    AccessDenied,
    Decode,
    UnsupportedSource,
    Backend,
    Cancelled,
}
#[derive(Debug, Clone)]
pub struct PlayerPortSnapshot {
    pub position_ms: u64,
    pub duration_ms: Option<u64>,
    pub state: PlayerPortState,
    pub speed: f32,
    pub video_track: Option<i64>,
    pub audio_track: Option<i64>,
}
#[derive(Debug, Clone)]
pub struct PlayerPortFailureEvent {
    pub generation: u64,
    pub kind: PlayerPortFailure,
    pub retry_delay_ms: Option<u64>,
}
#[derive(Debug, Clone)]
pub struct PlayerPortBatch {
    pub session_id: u64,
    pub generation: u64,
    pub emitting_generation: u64,
    pub snapshot: Option<PlayerPortSnapshot>,
    pub buffered_ms: u64,
    pub confirmed: Vec<u64>,
    pub failures: Vec<PlayerPortFailureEvent>,
    pub seek_completed: bool,
}
#[derive(Debug, Clone)]
pub struct PlayerCommandResult {
    pub data: Option<PlayerCommandTicket>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}
#[derive(Debug, Clone)]
pub struct PlayerBatchResult {
    pub data: Option<PlayerPortBatch>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}
#[derive(Debug, Clone)]
pub struct PlayerPortActionResult {
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

fn sessions() -> &'static Mutex<HashMap<u64, MediaKitPort>> {
    static SESSIONS: OnceLock<Mutex<HashMap<u64, MediaKitPort>>> = OnceLock::new();
    SESSIONS.get_or_init(|| Mutex::new(HashMap::new()))
}
#[frb(sync)]
pub fn player_port_create() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    sessions()
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .insert(id, MediaKitPort::new(SessionId(id)));
    id
}
#[frb(sync)]
pub fn player_port_prepare(session_id: u64, intent: PlayerIntent) -> PlayerCommandResult {
    let mut guard = sessions().lock().unwrap_or_else(|p| p.into_inner());
    let Some(port) = guard.get_mut(&session_id) else {
        return command_error("closed");
    };
    let command = match intent.kind {
        PlayerIntentKind::Open => {
            let Some(video) = intent.video.as_ref().filter(|s| !s.is_empty()) else {
                return command_error("invalid_source");
            };
            let source = core::media_source(
                video.clone(),
                intent.audio.clone(),
                intent.audio_only.unwrap_or(false),
            );
            port.set_context(
                intent.is_live.unwrap_or(false),
                intent.is_file.unwrap_or(false),
            );
            PlayerCommand::Open {
                source,
                headers: MediaHeaders {
                    user_agent: intent.user_agent.clone().unwrap_or_default(),
                    referer: intent.referer.clone().unwrap_or_default(),
                },
            }
        }
        PlayerIntentKind::Play => PlayerCommand::Play,
        PlayerIntentKind::Pause => PlayerCommand::Pause,
        PlayerIntentKind::Seek => {
            let Some(position_ms) = intent.position_ms else {
                return command_error("invalid_position");
            };
            PlayerCommand::Seek { position_ms }
        }
        PlayerIntentKind::SetSpeed => {
            let Some(speed) = intent.speed.filter(|s| s.is_finite() && *s > 0.0) else {
                return command_error("invalid_speed");
            };
            PlayerCommand::SetSpeed(speed)
        }
        PlayerIntentKind::SelectTracks => PlayerCommand::SelectTracks {
            video: intent.video_track.map(TrackId),
            audio: intent.audio_track.map(TrackId),
        },
        PlayerIntentKind::AttachSurface => {
            PlayerCommand::AttachSurface(SurfaceId(intent.surface_id.unwrap_or_default()))
        }
        PlayerIntentKind::DetachSurface => PlayerCommand::DetachSurface,
        PlayerIntentKind::SetLifecycle => {
            PlayerCommand::SetLifecycle(if intent.foreground.unwrap_or(true) {
                Lifecycle::Foreground
            } else {
                Lifecycle::Background
            })
        }
    };
    if !port.is_current(port.generation()) {
        return command_error("backend_unavailable");
    }
    let media_uri = match &command {
        PlayerCommand::Open { source, .. } => Some(core::media_uri(source)),
        _ => None,
    };
    let stamped = port.prepare(command);
    PlayerCommandResult {
        data: Some(PlayerCommandTicket {
            session_id,
            generation: stamped.generation.0,
            intent,
            media_uri,
        }),
        error_kind: None,
        error_detail: None,
        error_code: None,
    }
}
fn command_error(kind: &str) -> PlayerCommandResult {
    PlayerCommandResult {
        data: None,
        error_kind: Some(kind.into()),
        error_detail: None,
        error_code: None,
    }
}
#[frb(sync)]
pub fn player_port_begin(session_id: u64, generation: u64) -> PlayerPortActionResult {
    let ready = sessions()
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .get_mut(&session_id)
        .is_some_and(|p| p.begin(Generation(generation)));
    PlayerPortActionResult {
        error_kind: (!ready).then(|| "stale_or_busy".into()),
        error_detail: None,
        error_code: None,
    }
}
#[frb(sync)]
pub fn player_port_is_current(session_id: u64, generation: u64) -> bool {
    sessions()
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .get(&session_id)
        .is_some_and(|p| p.is_current(Generation(generation)))
}
#[frb(sync)]
pub fn player_port_observe(
    session_id: u64,
    observations: Vec<PlayerObservation>,
) -> PlayerBatchResult {
    let mut guard = sessions().lock().unwrap_or_else(|p| p.into_inner());
    let Some(port) = guard.get_mut(&session_id) else {
        return PlayerBatchResult {
            data: None,
            error_kind: Some("closed".into()),
            error_detail: None,
            error_code: None,
        };
    };
    let batch = port.observe(
        observations
            .into_iter()
            .map(|o| Observation {
                kind: observation_kind(o.kind),
                value: o.value,
                text: o.text,
                generation: o.generation.map(Generation),
            })
            .collect(),
    );
    PlayerBatchResult {
        data: Some(PlayerPortBatch {
            session_id,
            generation: port.generation().0,
            emitting_generation: port.emitting_generation().0,
            snapshot: batch.snapshot.map(|s| PlayerPortSnapshot {
                position_ms: s.position_ms,
                duration_ms: s.duration_ms,
                state: state(s.state),
                speed: s.speed,
                video_track: s.video_track.map(|v| v.0),
                audio_track: s.audio_track.map(|v| v.0),
            }),
            buffered_ms: batch.buffered_ms,
            confirmed: batch.confirmed.into_iter().map(|g| g.0).collect(),
            failures: batch
                .failed
                .into_iter()
                .map(|(g, k)| PlayerPortFailureEvent {
                    generation: g.0,
                    kind: failure(k),
                    retry_delay_ms: port.retry_delay_ms(k),
                })
                .collect(),
            seek_completed: batch.seek_completed,
        }),
        error_kind: None,
        error_detail: None,
        error_code: None,
    }
}
#[frb(sync)]
pub fn player_port_should_retry(session_id: u64, generation: u64) -> bool {
    sessions()
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .get(&session_id)
        .is_some_and(|p| p.should_retry(Generation(generation)))
}
#[frb(sync)]
pub fn player_port_abort(session_id: u64, generation: u64) {
    if let Some(port) = sessions()
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .get_mut(&session_id)
    {
        port.abort(Generation(generation));
    }
}
#[frb(sync)]
pub fn player_port_close(session_id: u64) {
    sessions()
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .remove(&session_id);
}
#[frb(sync)]
pub fn player_port_session_count() -> usize {
    sessions().lock().unwrap_or_else(|p| p.into_inner()).len()
}

fn observation_kind(kind: PlayerObservationKind) -> ObservationKind {
    use PlayerObservationKind as K;
    match kind {
        K::StartFile => ObservationKind::StartFile,
        K::FileLoaded => ObservationKind::FileLoaded,
        K::Seek => ObservationKind::Seek,
        K::PlaybackRestart => ObservationKind::PlaybackRestart,
        K::EndFile => ObservationKind::EndFile,
        K::Overflow => ObservationKind::Overflow,
        K::Position => ObservationKind::Position,
        K::Duration => ObservationKind::Duration,
        K::Paused => ObservationKind::Paused,
        K::Buffering => ObservationKind::Buffering,
        K::Ended => ObservationKind::Ended,
        K::Speed => ObservationKind::Speed,
        K::VideoTrack => ObservationKind::VideoTrack,
        K::AudioTrack => ObservationKind::AudioTrack,
        K::Buffer => ObservationKind::Buffer,
        K::Error => ObservationKind::Error,
    }
}
fn state(state: PlaybackState) -> PlayerPortState {
    match state {
        PlaybackState::Idle => PlayerPortState::Idle,
        PlaybackState::Opening => PlayerPortState::Opening,
        PlaybackState::Playing => PlayerPortState::Playing,
        PlaybackState::Paused => PlayerPortState::Paused,
        PlaybackState::Buffering => PlayerPortState::Buffering,
        PlaybackState::Ended => PlayerPortState::Ended,
    }
}
fn failure(kind: PlayerFailureKind) -> PlayerPortFailure {
    match kind {
        PlayerFailureKind::Network => PlayerPortFailure::Network,
        PlayerFailureKind::AccessDenied => PlayerPortFailure::AccessDenied,
        PlayerFailureKind::Decode => PlayerPortFailure::Decode,
        PlayerFailureKind::UnsupportedSource => PlayerPortFailure::UnsupportedSource,
        PlayerFailureKind::Backend => PlayerPortFailure::Backend,
        PlayerFailureKind::Cancelled => PlayerPortFailure::Cancelled,
    }
}
