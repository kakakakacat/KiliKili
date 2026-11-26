//! media-kit 端口的确认闸门。mpv 命令回复不等于已完成加载或 seek。
//! 这里不持有 mpv/Flutter 类型，原生属性只作为事实输入；时间不由 Rust 外推。

use std::convert::Infallible;
use std::time::{Duration, Instant};

use bili_domain::player::{
    MediaSource, PlaybackEvent, PlaybackState, PlayerCommand, PlayerFailureKind, PlayerPort,
    PlayerSnapshot, TrackId,
};
use bili_domain::session::{Generation, SessionId, Stamped};

use crate::player_session::PlayerSession;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObservationKind {
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

#[derive(Debug, Clone)]
pub struct Observation {
    pub kind: ObservationKind,
    pub value: f64,
    pub text: Option<String>,
    pub generation: Option<Generation>,
}

#[derive(Debug, Default)]
pub struct Batch {
    pub snapshot: Option<PlayerSnapshot>,
    pub buffered_ms: u64,
    pub confirmed: Vec<Generation>,
    pub failed: Vec<(Generation, PlayerFailureKind)>,
    pub seek_completed: bool,
    /// Open/Seek/切轨已交给后端、还没等到确认。
    ///
    /// 这段时间旧代际的快照会被闸门丢弃、新代际还没有快照，宿主只能看到
    /// 最后一张「播放中」——断网 Seek 时画面冻住却显示在播（#244）。宿主
    /// 据此显示加载态；它是事实而不是计时，超时仍由宿主的确认超时判定。
    pub awaiting: bool,
}

#[derive(Debug)]
struct Transport;
impl PlayerPort for Transport {
    type Error = Infallible;
    fn dispatch(&mut self, _: Stamped<PlayerCommand>) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct Pending {
    generation: Generation,
    open: bool,
    started: bool,
    tracks: Option<(Option<TrackId>, Option<TrackId>)>,
}

#[derive(Debug)]
struct Observed {
    position_ms: u64,
    duration_ms: Option<u64>,
    paused: bool,
    buffering: bool,
    ended: bool,
    speed: f32,
    video: Option<TrackId>,
    audio: Option<TrackId>,
    buffered_ms: u64,
}
impl Default for Observed {
    fn default() -> Self {
        Self {
            position_ms: 0,
            duration_ms: None,
            paused: true,
            buffering: false,
            ended: false,
            speed: 1.0,
            video: None,
            audio: None,
            buffered_ms: 0,
        }
    }
}

#[derive(Debug)]
pub struct MediaKitPort {
    id: SessionId,
    session: PlayerSession<Transport>,
    emitting: Generation,
    pending: Option<Pending>,
    prepared: Option<Stamped<PlayerCommand>>,
    observed: Observed,
    loaded: bool,
    position_ready: bool,
    poisoned: bool,
    live: bool,
    file: bool,
    last_retry: Option<Instant>,
    retry_generation: Option<Generation>,
}

impl MediaKitPort {
    #[must_use]
    pub fn new(id: SessionId) -> Self {
        Self {
            id,
            session: PlayerSession::new(id, Transport),
            emitting: Generation(0),
            pending: None,
            prepared: None,
            observed: Observed::default(),
            loaded: false,
            position_ready: false,
            poisoned: false,
            live: false,
            file: false,
            last_retry: None,
            retry_generation: None,
        }
    }

    pub fn set_context(&mut self, live: bool, file: bool) {
        self.live = live;
        self.file = file;
    }

    pub fn retry_delay_ms(&mut self, kind: PlayerFailureKind) -> Option<u64> {
        if self.file || kind != PlayerFailureKind::Network {
            return None;
        }
        let now = Instant::now();
        if self
            .last_retry
            .is_some_and(|last| now.duration_since(last) < Duration::from_secs(10))
        {
            return None;
        }
        self.last_retry = Some(now);
        self.retry_generation = Some(self.generation());
        Some(3000)
    }

    #[must_use]
    pub fn should_retry(&self, generation: Generation) -> bool {
        if self.file {
            return false;
        }
        if self.poisoned {
            // 已确认网络失败可重建后端重试；超时本身不授予自动重试。
            return self.retry_generation == Some(generation);
        }
        self.is_current(generation)
            && (self.live || (self.observed.buffering && self.observed.buffered_ms == 0))
    }

    pub fn prepare(&mut self, command: PlayerCommand) -> Stamped<PlayerCommand> {
        let stamped = match self.session.dispatch_stamped(command) {
            Ok(stamped) => stamped,
            Err(never) => match never {},
        };
        if matches!(stamped.value, PlayerCommand::Close) {
            self.loaded = false;
        }
        if matches!(
            stamped.value,
            PlayerCommand::Open { .. }
                | PlayerCommand::Seek { .. }
                | PlayerCommand::SelectTracks { .. }
        ) {
            self.prepared = Some(stamped.clone());
        }
        stamped
    }

    #[must_use]
    pub fn generation(&self) -> Generation {
        self.session.generation()
    }

    #[must_use]
    pub fn is_current(&self, generation: Generation) -> bool {
        !self.poisoned && generation == self.generation()
    }

    #[must_use]
    pub fn emitting_generation(&self) -> Generation {
        self.pending
            .filter(|p| p.tracks.is_some())
            .map_or(self.emitting, |p| p.generation)
    }

    /// 后端串行队列在排空旧事件后调用。只为真正执行的命令等待确认。
    pub fn begin(&mut self, generation: Generation) -> bool {
        if !self.is_current(generation) {
            return false;
        }
        let Some(command) = self
            .prepared
            .as_ref()
            .filter(|c| c.generation == generation)
        else {
            return false;
        };
        let open = matches!(command.value, PlayerCommand::Open { .. });
        if open
            || matches!(
                command.value,
                PlayerCommand::Seek { .. } | PlayerCommand::SelectTracks { .. }
            )
        {
            if self.pending.is_some() {
                return false;
            }
            self.pending = Some(Pending {
                generation,
                open,
                started: false,
                tracks: match command.value {
                    PlayerCommand::SelectTracks { video, audio } => Some((video, audio)),
                    _ => None,
                },
            });
            if self.pending.is_some_and(|p| p.tracks.is_some()) {
                self.observed.video = None;
                self.observed.audio = None;
                self.position_ready = false;
            }
        }
        true
    }

    /// 超时后永久封住本后端；无命令 ID 的迟到确认不能冒充下一次操作。
    pub fn abort(&mut self, _: Generation) {
        self.poisoned = true;
        self.loaded = false;
        self.pending = None;
        self.prepare(PlayerCommand::Close);
    }

    pub fn observe(&mut self, observations: Vec<Observation>) -> Batch {
        let mut batch = Batch::default();
        if self.poisoned {
            return batch;
        }
        // 空批次也要如实报告在途命令：宿主每 40ms 轮询一次，空批次很常见，
        // 报成 false 会让加载态闪烁。
        batch.awaiting = self.pending.is_some();
        if observations.is_empty() {
            return batch;
        }
        for observation in observations {
            use ObservationKind as K;
            let value = observation.value;
            if observation
                .generation
                .is_some_and(|g| g != self.emitting_generation())
            {
                continue;
            }
            match observation.kind {
                K::StartFile => {
                    self.observed = Observed::default();
                    self.position_ready = false;
                    self.loaded = false;
                    if let Some(pending) = self.pending.as_mut().filter(|p| p.open) {
                        pending.started = true;
                    }
                }
                K::Seek => {
                    if let Some(pending) = self.pending.as_mut().filter(|p| !p.open) {
                        pending.started = true;
                    }
                }
                K::FileLoaded | K::PlaybackRestart => {
                    if let Some(pending) = self.pending.filter(|p| {
                        p.started
                            && p.tracks.is_none()
                            && p.open == (observation.kind == K::FileLoaded)
                    }) {
                        self.emitting = pending.generation;
                        self.position_ready = false;
                        self.loaded = true;
                        self.pending = None;
                        batch.confirmed.push(pending.generation);
                        batch.seek_completed = !pending.open && self.is_current(pending.generation);
                    }
                }
                K::EndFile => {
                    if value < 0.0 {
                        // 新资源尚未 START_FILE 时，这只能是旧资源的结束事件。
                        if self.pending.is_some_and(|p| p.open && !p.started) {
                            continue;
                        }
                        if let Some(pending) = self.pending.as_mut() {
                            pending.started = true;
                        }
                        self.fail(&mut batch, failure_code(value as i32));
                        self.loaded = false;
                        if let Some(pending) = self.pending.take() {
                            batch.confirmed.push(pending.generation);
                        }
                    }
                }
                K::Overflow => {
                    self.fail(&mut batch, PlayerFailureKind::Backend);
                    self.abort(self.generation());
                }
                K::Error => {
                    if let Some(kind) = observation.text.as_deref().and_then(classify_error) {
                        self.fail(&mut batch, kind);
                    }
                }
                K::Position => {
                    if let Some(ms) = millis(value) {
                        self.observed.position_ms = ms;
                        self.position_ready = true;
                    }
                }
                K::Duration => {
                    self.observed.duration_ms = millis(value).filter(|v| *v > 0);
                }
                K::Paused => self.observed.paused = value != 0.0,
                K::Buffering => self.observed.buffering = value != 0.0,
                K::Ended => self.observed.ended = value != 0.0,
                K::Speed => {
                    if value.is_finite() && value > 0.0 {
                        self.observed.speed = value as f32;
                    }
                }
                K::VideoTrack => {
                    self.observed.video = (value >= 0.0).then_some(TrackId(value as i64))
                }
                K::AudioTrack => {
                    self.observed.audio = (value >= 0.0).then_some(TrackId(value as i64))
                }
                K::Buffer => {
                    if let Some(ms) = millis(value) {
                        self.observed.buffered_ms = ms;
                    }
                }
            }
        }
        if let Some(pending) = self.pending.filter(|p| p.tracks.is_some()) {
            let (video, audio) = pending.tracks.unwrap_or_default();
            if self.position_ready
                && video.is_none_or(|id| self.observed.video == Some(id))
                && audio.is_none_or(|id| self.observed.audio == Some(id))
            {
                self.emitting = pending.generation;
                self.pending = None;
                batch.confirmed.push(pending.generation);
            }
        }
        batch.awaiting = self.pending.is_some() && !self.poisoned;
        if self.loaded && self.position_ready && !self.poisoned {
            let observed = &self.observed;
            let snapshot = PlayerSnapshot {
                position_ms: observed.position_ms,
                duration_ms: observed.duration_ms,
                state: if observed.ended {
                    PlaybackState::Ended
                } else if observed.paused {
                    PlaybackState::Paused
                } else if observed.buffering {
                    PlaybackState::Buffering
                } else {
                    PlaybackState::Playing
                },
                speed: observed.speed,
                video_track: observed.video,
                audio_track: observed.audio,
            };
            if self
                .session
                .accept_event(Stamped::new(
                    self.id,
                    self.emitting,
                    PlaybackEvent::Snapshot(snapshot),
                ))
                .is_some()
            {
                batch.snapshot = Some(snapshot);
                batch.buffered_ms = observed.buffered_ms;
            }
        }
        batch
    }

    fn fail(&mut self, batch: &mut Batch, kind: PlayerFailureKind) {
        let generation = self
            .pending
            .filter(|p| p.started)
            .map_or(self.emitting, |p| p.generation);
        if self
            .session
            .accept_event(Stamped::new(
                self.id,
                generation,
                PlaybackEvent::Failed(kind),
            ))
            .is_some()
        {
            batch.failed.push((generation, kind));
        }
    }
}

fn millis(seconds: f64) -> Option<u64> {
    (seconds.is_finite() && seconds >= 0.0).then_some((seconds * 1000.0) as u64)
}

/// EDL 是 mpv 的资源表示，长度按 UTF-8 字节计数，网络和本地文件一致。
#[must_use]
pub fn media_uri(source: &MediaSource) -> String {
    let part = |s: &str| format!("%{}%{s}", s.len());
    match source {
        MediaSource::Dash { video, audio } => format!(
            "edl://!no_chapters;{};!new_stream;!no_chapters;{}",
            part(video.expose()),
            part(audio.expose())
        ),
        MediaSource::Segments(segments) if segments.len() == 1 => segments[0].expose().clone(),
        MediaSource::Segments(segments) => format!(
            "edl://!no_chapters;{}",
            segments
                .iter()
                .map(|s| part(s.expose()))
                .collect::<Vec<_>>()
                .join(";")
        ),
    }
}

#[must_use]
pub fn classify_error(text: &str) -> Option<PlayerFailureKind> {
    if text.contains("403") || text.contains("401") {
        Some(PlayerFailureKind::AccessDenied)
    } else if text.starts_with("Failed to open https://")
        || text.starts_with("Can not open external file https://")
        || text.starts_with("tcp: ffurl_read returned ")
    {
        Some(PlayerFailureKind::Network)
    } else if text.starts_with("Could not open codec") {
        Some(PlayerFailureKind::Decode)
    } else {
        // error 级日志也可能是 seek 后可恢复的缺参考帧；不能抢在后端确认前
        // 判死会话。未知日志仍由宿主记录，真正终止由 END_FILE 错误码判定。
        None
    }
}

#[must_use]
pub const fn failure_code(code: i32) -> PlayerFailureKind {
    match code {
        -13 => PlayerFailureKind::Network,
        -14 => PlayerFailureKind::Decode,
        -17..=-15 => PlayerFailureKind::UnsupportedSource,
        _ => PlayerFailureKind::Backend,
    }
}

/// 只听音频时的资源选择同样属于可复用决策；缺音轨仍保留可播放的视频源。
#[must_use]
pub fn media_source(video: String, audio: Option<String>, audio_only: bool) -> MediaSource {
    match audio.filter(|a| !a.is_empty()) {
        Some(audio) if audio_only => MediaSource::Segments(vec![audio.into()]),
        Some(audio) => MediaSource::Dash {
            video: video.into(),
            audio: audio.into(),
        },
        None => MediaSource::Segments(vec![video.into()]),
    }
}
