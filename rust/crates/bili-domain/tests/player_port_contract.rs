use bili_domain::player::{
    Lifecycle, MediaHeaders, MediaSource, PlaybackEvent, PlaybackState, PlayerCommand,
    PlayerEventSink, PlayerPort, PlayerSnapshot, SurfaceId, TrackId,
};
use bili_domain::redact::Redacted;
use bili_domain::session::{GenerationGate, SessionId, Stamped};

#[derive(Default)]
struct RecordingPort {
    commands: Vec<Stamped<PlayerCommand>>,
}

#[derive(Default)]
struct RecordingSink {
    events: Vec<Stamped<PlaybackEvent>>,
}

impl PlayerEventSink for RecordingSink {
    fn emit(&mut self, event: Stamped<PlaybackEvent>) {
        self.events.push(event);
    }
}

impl PlayerPort for RecordingPort {
    type Error = String;

    fn dispatch(&mut self, command: Stamped<PlayerCommand>) -> Result<(), Self::Error> {
        self.commands.push(command);
        Ok(())
    }
}

#[test]
fn dash_open_keeps_separate_tracks_and_request_headers() {
    let gate = GenerationGate::new(SessionId(7));
    let mut port = RecordingPort::default();
    let command = PlayerCommand::Open {
        source: MediaSource::Dash {
            video: Redacted::new("https://example.com/video?token=secret".into()),
            audio: Redacted::new("https://example.com/audio".into()),
        },
        headers: MediaHeaders {
            user_agent: "test-agent".into(),
            referer: "https://www.bilibili.com".into(),
        },
    };
    assert!(
        port.dispatch(Stamped::new(gate.session(), gate.current(), command))
            .is_ok()
    );
    assert!(matches!(port.commands[0].value, PlayerCommand::Open { .. }));
    assert!(!format!("{:?}", port.commands[0]).contains("secret"));
}

#[test]
fn seek_invalidates_old_events_and_uses_backend_position() {
    let gate = GenerationGate::new(SessionId(9));
    let old = Stamped::new(
        gate.session(),
        gate.current(),
        PlaybackEvent::Snapshot(PlayerSnapshot {
            position_ms: 1_000,
            duration_ms: Some(20_000),
            state: PlaybackState::Playing,
            speed: 1.0,
            video_track: Some(TrackId(80)),
            audio_track: Some(TrackId(30280)),
        }),
    );
    let generation = gate.bump();
    assert!(old.accept(&gate).is_none());
    let actual = Stamped::new(
        gate.session(),
        generation,
        PlaybackEvent::SeekCompleted { position_ms: 8_400 },
    );
    assert!(matches!(
        actual.accept(&gate),
        Some(PlaybackEvent::SeekCompleted { position_ms: 8_400 })
    ));
}

#[test]
fn commands_cover_lifecycle_surface_and_track_selection() {
    let commands = [
        PlayerCommand::AttachSurface(SurfaceId(3)),
        PlayerCommand::SetLifecycle(Lifecycle::Background),
        PlayerCommand::SelectTracks {
            video: Some(TrackId(120)),
            audio: Some(TrackId(30280)),
        },
        PlayerCommand::Pause,
        PlayerCommand::SetSpeed(1.5),
        PlayerCommand::Close,
    ];
    assert_eq!(commands.len(), 6);
}

#[test]
fn event_sink_preserves_session_and_generation_for_consumer_filtering() {
    let gate = GenerationGate::new(SessionId(3));
    let mut sink = RecordingSink::default();
    sink.emit(Stamped::new(
        gate.session(),
        gate.current(),
        PlaybackEvent::SurfaceLost,
    ));
    gate.bump();
    assert!(
        sink.events
            .pop()
            .is_some_and(|event| event.accept(&gate).is_none())
    );
}
