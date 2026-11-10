use bili_core::player_session::PlayerSession;
use bili_domain::player::{
    PlaybackEvent, PlaybackState, PlayerCommand, PlayerPort, PlayerSnapshot,
};
use bili_domain::session::{SessionId, Stamped};

#[derive(Default)]
struct Port {
    sent: Vec<Stamped<PlayerCommand>>,
}

impl PlayerPort for Port {
    type Error = ();

    fn dispatch(&mut self, command: Stamped<PlayerCommand>) -> Result<(), Self::Error> {
        self.sent.push(command);
        Ok(())
    }
}

fn snapshot(position_ms: u64, state: PlaybackState) -> PlayerSnapshot {
    PlayerSnapshot {
        position_ms,
        duration_ms: Some(60_000),
        state,
        speed: 1.0,
        video_track: None,
        audio_track: None,
    }
}

#[test]
fn seek_rejects_old_snapshot_and_uses_backend_position() {
    let mut session = PlayerSession::new(SessionId(1), Port::default());
    let old_generation = session.generation();
    assert!(
        session
            .dispatch(PlayerCommand::Seek {
                position_ms: 10_000
            })
            .is_ok()
    );
    let stale = Stamped::new(
        SessionId(1),
        old_generation,
        PlaybackEvent::Snapshot(snapshot(1_000, PlaybackState::Playing)),
    );
    assert!(session.accept_event(stale).is_none());
    assert!(session.snapshot().is_none());

    let actual = Stamped::new(
        SessionId(1),
        session.generation(),
        PlaybackEvent::Snapshot(snapshot(9_600, PlaybackState::Buffering)),
    );
    assert!(session.accept_event(actual).is_some());
    assert_eq!(session.snapshot().map(|s| s.position_ms), Some(9_600));
}

#[test]
fn route_disposal_and_cross_session_events_cannot_restore_playback() {
    let mut session = PlayerSession::new(SessionId(2), Port::default());
    let old_generation = session.generation();
    assert!(session.dispatch(PlayerCommand::Close).is_ok());
    assert!(
        session
            .accept_event(Stamped::new(
                SessionId(2),
                old_generation,
                PlaybackEvent::Snapshot(snapshot(100, PlaybackState::Playing))
            ))
            .is_none()
    );
    assert!(
        session
            .accept_event(Stamped::new(
                SessionId(3),
                session.generation(),
                PlaybackEvent::Snapshot(snapshot(200, PlaybackState::Playing))
            ))
            .is_none()
    );
    assert!(session.snapshot().is_none());
}

#[test]
fn pause_and_speed_intents_do_not_fabricate_player_state() {
    let mut session = PlayerSession::new(SessionId(4), Port::default());
    assert!(session.dispatch(PlayerCommand::Pause).is_ok());
    assert!(session.dispatch(PlayerCommand::SetSpeed(1.5)).is_ok());
    assert!(session.snapshot().is_none());
    assert_eq!(session.port().sent.len(), 2);
}
