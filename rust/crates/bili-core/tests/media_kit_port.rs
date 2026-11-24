use bili_core::media_kit_port::{
    MediaKitPort, Observation, ObservationKind as K, classify_error, media_uri,
};
use bili_domain::player::{
    MediaHeaders, MediaSource, PlaybackState, PlayerCommand, PlayerFailureKind,
};
use bili_domain::redact::Redacted;
use bili_domain::session::SessionId;

fn obs(kind: K, value: f64) -> Observation {
    Observation {
        kind,
        value,
        text: None,
        generation: None,
    }
}
fn open(port: &mut MediaKitPort) {
    let command = port.prepare(PlayerCommand::Open {
        source: MediaSource::Segments(vec![Redacted::new("fixture.mp4".into())]),
        headers: MediaHeaders {
            user_agent: "fixture".into(),
            referer: "fixture".into(),
        },
    });
    assert!(port.begin(command.generation));
    let batch = port.observe(vec![
        obs(K::StartFile, 0.0),
        obs(K::FileLoaded, 0.0),
        obs(K::Position, 1.0),
    ]);
    assert_eq!(batch.snapshot.map(|s| s.position_ms), Some(1000));
}

#[test]
fn command_reply_or_unpaired_restart_cannot_complete_seek() {
    let mut port = MediaKitPort::new(SessionId(1));
    open(&mut port);
    let command = port.prepare(PlayerCommand::Seek { position_ms: 9000 });
    assert!(port.begin(command.generation));
    let batch = port.observe(vec![obs(K::PlaybackRestart, 0.0), obs(K::Position, 2.0)]);
    assert!(batch.snapshot.is_none());
    assert!(batch.confirmed.is_empty());
    let batch = port.observe(vec![
        obs(K::Seek, 0.0),
        obs(K::PlaybackRestart, 0.0),
        obs(K::Position, 8.9),
    ]);
    assert_eq!(batch.confirmed, vec![command.generation]);
    assert_eq!(batch.snapshot.map(|s| s.position_ms), Some(8900));
    assert!(batch.seek_completed);
}

#[test]
fn superseded_seek_confirmation_only_releases_command_queue() {
    let mut port = MediaKitPort::new(SessionId(2));
    open(&mut port);
    let first = port.prepare(PlayerCommand::Seek { position_ms: 5000 });
    assert!(port.begin(first.generation));
    let second = port.prepare(PlayerCommand::Seek { position_ms: 20000 });
    let batch = port.observe(vec![
        obs(K::Seek, 0.0),
        obs(K::Position, 5.0),
        obs(K::PlaybackRestart, 0.0),
    ]);
    assert_eq!(batch.confirmed, vec![first.generation]);
    assert!(batch.snapshot.is_none());
    assert!(!port.begin(first.generation));
    assert!(port.begin(second.generation));
    assert!(port.observe(vec![obs(K::Position, 6.0)]).snapshot.is_none());
    let batch = port.observe(vec![
        obs(K::Seek, 0.0),
        obs(K::PlaybackRestart, 0.0),
        obs(K::Position, 20.0),
    ]);
    assert_eq!(batch.snapshot.map(|s| s.position_ms), Some(20000));
}

#[test]
fn open_requires_start_before_loaded_and_resets_old_properties() {
    let mut port = MediaKitPort::new(SessionId(3));
    open(&mut port);
    let command = port.prepare(PlayerCommand::Open {
        source: MediaSource::Segments(vec![]),
        headers: MediaHeaders {
            user_agent: String::new(),
            referer: String::new(),
        },
    });
    assert!(port.begin(command.generation));
    assert!(
        port.observe(vec![obs(K::FileLoaded, 0.0)])
            .snapshot
            .is_none()
    );
    let batch = port.observe(vec![
        obs(K::StartFile, 0.0),
        obs(K::FileLoaded, 0.0),
        obs(K::Position, 0.0),
    ]);
    assert_eq!(batch.snapshot.map(|s| s.position_ms), Some(0));
    assert_eq!(batch.snapshot.and_then(|s| s.duration_ms), None);
}

#[test]
fn one_batch_emits_only_latest_backend_position_without_extrapolation() {
    let mut port = MediaKitPort::new(SessionId(4));
    open(&mut port);
    let batch = port.observe((1..=100).map(|i| obs(K::Position, f64::from(i))).collect());
    assert_eq!(batch.snapshot.map(|s| s.position_ms), Some(100000));
    port.prepare(PlayerCommand::Pause);
    port.prepare(PlayerCommand::SetSpeed(2.0));
    let batch = port.observe(vec![obs(K::Paused, 1.0), obs(K::Speed, 0.5)]);
    let Some(snapshot) = batch.snapshot else {
        panic!("应收到后端快照")
    };
    assert_eq!(snapshot.position_ms, 100000);
    assert_eq!(snapshot.state, PlaybackState::Paused);
    assert_eq!(snapshot.speed, 0.5);
    let batch = port.observe(vec![obs(K::Paused, 0.0), obs(K::Buffering, 1.0)]);
    assert_eq!(
        batch.snapshot.map(|s| s.state),
        Some(PlaybackState::Buffering)
    );
    assert!(port.observe(vec![]).snapshot.is_none());
}

#[test]
fn closed_and_aborted_generations_cannot_restore_state() {
    let mut port = MediaKitPort::new(SessionId(5));
    open(&mut port);
    let seek = port.prepare(PlayerCommand::Seek { position_ms: 30000 });
    assert!(port.begin(seek.generation));
    port.abort(seek.generation);
    assert!(
        port.observe(vec![
            obs(K::Seek, 0.0),
            obs(K::PlaybackRestart, 0.0),
            obs(K::Position, 30.0)
        ])
        .snapshot
        .is_none()
    );
    port.prepare(PlayerCommand::Close);
    assert!(
        port.observe(vec![obs(K::Position, 100.0)])
            .snapshot
            .is_none()
    );
}

#[test]
fn failures_during_open_are_attributed_to_pending_generation() {
    let mut port = MediaKitPort::new(SessionId(6));
    open(&mut port);
    let seek = port.prepare(PlayerCommand::Seek { position_ms: 30000 });
    assert!(port.begin(seek.generation));
    let batch = port.observe(vec![obs(K::EndFile, -13.0)]);
    assert_eq!(
        batch.failed,
        vec![(seek.generation, PlayerFailureKind::Network)]
    );
    assert!(batch.snapshot.is_none());
}

#[test]
fn resource_encoding_counts_utf8_bytes_and_preserves_dash_tracks() {
    let source = MediaSource::Dash {
        video: Redacted::new("/视频.m4s".into()),
        audio: Redacted::new("/音频.m4s".into()),
    };
    assert_eq!(
        media_uri(&source),
        "edl://!no_chapters;%11%/视频.m4s;!new_stream;!no_chapters;%11%/音频.m4s"
    );
    assert_eq!(
        media_uri(&MediaSource::Segments(vec![Redacted::new(
            "fixture.mp4".into()
        )])),
        "fixture.mp4"
    );
}

#[test]
fn legacy_error_categories_live_in_rust() {
    assert_eq!(
        classify_error("Failed to open https://fixture.invalid"),
        Some(PlayerFailureKind::Network)
    );
    assert_eq!(
        classify_error("tcp: ffurl_read returned 0xffffff99"),
        Some(PlayerFailureKind::Network)
    );
    assert_eq!(
        classify_error("Could not open codec"),
        Some(PlayerFailureKind::Decode)
    );
    assert_eq!(
        classify_error("HTTP error 403 Forbidden"),
        Some(PlayerFailureKind::AccessDenied)
    );
    assert_eq!(classify_error("error running hook"), None);
}

#[test]
fn audio_only_and_retry_policy_do_not_depend_on_host() {
    use bili_core::media_kit_port::media_source;
    assert_eq!(
        media_uri(&media_source("video".into(), Some("audio".into()), true)),
        "audio"
    );
    assert_eq!(
        media_uri(&media_source("video".into(), Some(String::new()), true)),
        "video"
    );
    let mut port = MediaKitPort::new(SessionId(9));
    open(&mut port);
    port.set_context(true, false);
    assert_eq!(port.retry_delay_ms(PlayerFailureKind::Network), Some(3000));
    assert_eq!(port.retry_delay_ms(PlayerFailureKind::Network), None);
    assert!(port.should_retry(port.generation()));
    port.set_context(true, true);
    assert!(!port.should_retry(port.generation()));
    port.set_context(false, false);
    assert!(!port.should_retry(port.generation()));
    port.observe(vec![obs(K::Buffering, 1.0)]);
    assert!(port.should_retry(port.generation()));
}

#[test]
fn timeout_permanently_rejects_b_and_late_confirmation_of_a() {
    let mut port = MediaKitPort::new(SessionId(10));
    open(&mut port);
    let a = port.prepare(PlayerCommand::Seek { position_ms: 5000 });
    assert!(port.begin(a.generation));
    port.abort(a.generation);
    let b = port.prepare(PlayerCommand::Seek { position_ms: 20000 });
    assert!(!port.begin(b.generation));
    let batch = port.observe(vec![
        obs(K::Seek, 0.0),
        obs(K::PlaybackRestart, 0.0),
        obs(K::Position, 5.0),
    ]);
    assert!(batch.snapshot.is_none());
    assert!(batch.confirmed.is_empty());
}

#[test]
fn select_tracks_confirms_only_actual_track_ids() {
    use bili_domain::player::TrackId;
    use bili_domain::session::Generation;
    let mut port = MediaKitPort::new(SessionId(11));
    open(&mut port);
    let command = port.prepare(PlayerCommand::SelectTracks {
        video: Some(TrackId(2)),
        audio: Some(TrackId(3)),
    });
    assert!(port.begin(command.generation));
    let sample = |video| {
        vec![
            Observation {
                generation: Some(Generation(2)),
                ..obs(K::VideoTrack, video)
            },
            Observation {
                generation: Some(Generation(2)),
                ..obs(K::AudioTrack, 3.0)
            },
            Observation {
                generation: Some(Generation(2)),
                ..obs(K::Position, 5.0)
            },
        ]
    };
    assert!(port.observe(sample(1.0)).confirmed.is_empty());
    let batch = port.observe(sample(2.0));
    assert_eq!(batch.confirmed, vec![command.generation]);
    assert_eq!(batch.snapshot.and_then(|s| s.video_track), Some(TrackId(2)));
}

#[test]
fn superseded_open_failure_releases_serial_queue_without_ui_failure() {
    let mut port = MediaKitPort::new(SessionId(12));
    open(&mut port);
    let a = port.prepare(PlayerCommand::Seek { position_ms: 5000 });
    assert!(port.begin(a.generation));
    let b = port.prepare(PlayerCommand::Seek { position_ms: 10000 });
    let batch = port.observe(vec![obs(K::EndFile, -13.0)]);
    assert_eq!(batch.confirmed, vec![a.generation]);
    assert!(batch.failed.is_empty());
    assert!(port.begin(b.generation));
}

#[test]
fn previous_file_error_before_new_start_file_cannot_fail_new_open() {
    let mut port = MediaKitPort::new(SessionId(13));
    open(&mut port);
    let ticket = port.prepare(PlayerCommand::Open {
        source: MediaSource::Segments(vec![]),
        headers: MediaHeaders {
            user_agent: String::new(),
            referer: String::new(),
        },
    });
    assert!(port.begin(ticket.generation));
    let batch = port.observe(vec![obs(K::EndFile, -13.0)]);
    assert!(batch.failed.is_empty());
    assert!(batch.confirmed.is_empty());
    let batch = port.observe(vec![obs(K::StartFile, 0.0), obs(K::FileLoaded, 0.0)]);
    assert_eq!(batch.confirmed, vec![ticket.generation]);
}

#[test]
fn failed_open_can_retry_by_rebuilding_backend_but_timeout_cannot() {
    let mut port = MediaKitPort::new(SessionId(14));
    let ticket = port.prepare(PlayerCommand::Open {
        source: MediaSource::Segments(vec![]),
        headers: MediaHeaders {
            user_agent: String::new(),
            referer: String::new(),
        },
    });
    assert!(port.begin(ticket.generation));
    assert_eq!(
        port.observe(vec![obs(K::StartFile, 0.0), obs(K::EndFile, -13.0)])
            .failed
            .len(),
        1
    );
    assert_eq!(port.retry_delay_ms(PlayerFailureKind::Network), Some(3000));
    port.abort(ticket.generation);
    assert!(port.should_retry(ticket.generation));
    assert!(!port.is_current(ticket.generation));
}

#[test]
fn recoverable_decoder_log_during_seek_does_not_fail_pending_command() {
    let mut port = MediaKitPort::new(SessionId(15));
    open(&mut port);
    let seek = port.prepare(PlayerCommand::Seek { position_ms: 5000 });
    assert!(port.begin(seek.generation));
    let batch = port.observe(vec![
        obs(K::Seek, 0.0),
        Observation {
            text: Some("h264: co located POCs unavailable".into()),
            ..obs(K::Error, 0.0)
        },
        obs(K::PlaybackRestart, 0.0),
    ]);
    assert!(batch.failed.is_empty());
    assert_eq!(batch.confirmed, vec![seek.generation]);
    // 真正终止仍由 END_FILE 的 mpv 错误码判定，不能吞掉解码失败。
    assert_eq!(port.observe(vec![obs(K::EndFile, -14.0)]).failed.len(), 1);
}
