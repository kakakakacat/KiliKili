//! 与 Flutter 无窗口用例使用同一素材和断言；复用原型本身，不复制原生适配器。
#[allow(dead_code)]
#[path = "../src/mpv/mod.rs"]
mod mpv;
mod runtime_http;

use bili_domain::player::{
    MediaHeaders, MediaSource, PlaybackEvent, PlaybackState, PlayerCommand, PlayerEventSink,
    PlayerPort, PlayerSnapshot,
};
use bili_domain::redact::Redacted;
use bili_domain::session::{GenerationGate, SessionId, Stamped};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

struct Sink(mpsc::Sender<Stamped<PlaybackEvent>>);
impl PlayerEventSink for Sink {
    fn emit(&mut self, event: Stamped<PlaybackEvent>) {
        let _ = self.0.send(event);
    }
}

struct Probe {
    player: mpv::MpvPlayer,
    gate: GenerationGate,
    events: mpsc::Receiver<Stamped<PlaybackEvent>>,
    accepted: u64,
    stale: u64,
}
impl Probe {
    fn command(&mut self, command: PlayerCommand) {
        if matches!(
            command,
            PlayerCommand::Open { .. } | PlayerCommand::Seek { .. }
        ) {
            self.gate.bump();
        }
        self.player
            .dispatch(Stamped::new(
                self.gate.session(),
                self.gate.current(),
                command,
            ))
            .unwrap();
    }
    fn open(&mut self, source: &str) {
        self.command(PlayerCommand::Open {
            source: MediaSource::Segments(vec![Redacted::new(source.into())]),
            headers: MediaHeaders {
                user_agent: String::new(),
                referer: String::new(),
            },
        });
    }
    fn wait(&mut self, accept: impl Fn(PlayerSnapshot) -> bool) -> PlayerSnapshot {
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut last = None;
        while Instant::now() < deadline {
            let Ok(event) = self.events.recv_timeout(Duration::from_millis(100)) else {
                continue;
            };
            let Some(event) = event.accept(&self.gate) else {
                self.stale += 1;
                continue;
            };
            self.accepted += 1;
            match event {
                PlaybackEvent::Snapshot(snapshot) => {
                    last = Some(snapshot);
                    if accept(snapshot) {
                        return snapshot;
                    }
                }
                PlaybackEvent::Failed(failure) => panic!("原生适配器失败: {failure:?}"),
                _ => {}
            }
        }
        panic!(
            "原生适配器等待超时: {last:?}; accepted={} stale={} path={:?} pos={:?} vo={:?} pause={:?}",
            self.accepted,
            self.stale,
            self.player.property_string("path"),
            self.player.property_f64("time-pos"),
            self.player.property_string("vo"),
            self.player.property_string("pause")
        );
    }
}

#[test]
fn native_matches_flutter_clock_and_resolution_scenarios() {
    if std::env::var("KILIKILI_NATIVE_PLAYER_RUNTIME_TESTS").as_deref() != Ok("1") {
        return;
    }
    let media = std::env::var("KILIKILI_PLAYER_MEDIA").unwrap();
    let quality = std::path::Path::new(&media).with_file_name("quality.mp4");
    let library = std::env::var("KILIKILI_LIBMPV").unwrap();
    let lib = Arc::new(mpv::sys::MpvLib::load(Some(&library)).unwrap());
    let gate = GenerationGate::new(SessionId(400));
    let (tx, events) = mpsc::channel();
    let player = mpv::MpvPlayer::new(
        lib,
        &mpv::MpvConfig {
            hwdec: "no".into(),
            volume: 0,
            ..Default::default()
        },
        gate.clone(),
        Box::new(Sink(tx)),
    )
    .unwrap();
    // 本用例只对照契约；表面与弹幕由另一个真实 Slint 窗口脚本验收。
    player.debug_set_property("vo", "null").unwrap();
    player.debug_set_property("ao", "null").unwrap();
    let mut probe = Probe {
        player,
        gate,
        events,
        accepted: 0,
        stale: 0,
    };
    probe.open(&media);
    probe.command(PlayerCommand::Play);
    probe.wait(|s| s.state == PlaybackState::Playing && s.position_ms > 200);
    assert_eq!(probe.player.property_i64("width"), Some(320));
    probe.command(PlayerCommand::Pause);
    let paused = probe.wait(|s| s.state == PlaybackState::Paused);
    std::thread::sleep(Duration::from_millis(1400));
    // 原生后端只在属性变化时发快照；暂停后不能等一个本来就不该出现的新事件。
    let still = (probe.player.property_f64("time-pos").unwrap() * 1000.0).round() as u64;
    assert_eq!(paused.position_ms, still);
    for position_ms in [5000, 1000] {
        probe.command(PlayerCommand::Seek { position_ms });
        probe.wait(|s| s.position_ms.abs_diff(position_ms) < 150);
    }
    for speed in [2.0, 0.5] {
        probe.command(PlayerCommand::SetSpeed(speed));
        probe.wait(|s| s.speed == speed);
    }
    probe.open(quality.to_str().unwrap());
    probe.wait(|s| s.duration_ms.is_some_and(|d| d > 29000));
    assert_eq!(probe.player.property_i64("width"), Some(640));
    probe.command(PlayerCommand::Seek { position_ms: 5000 });
    probe.wait(|s| s.position_ms.abs_diff(5000) < 150);
    probe.command(PlayerCommand::Play);
    probe.wait(|s| s.state == PlaybackState::Playing && s.position_ms > 5200);
    let server = runtime_http::Server::new(std::fs::read(&quality).unwrap());
    for (key, value) in [
        ("cache", "yes"),
        ("demuxer-max-bytes", "1048576"),
        ("cache-secs", "1"),
        ("cache-pause-wait", "0.2"),
    ] {
        probe.player.debug_set_property(key, value).unwrap();
    }
    probe.open(&server.url);
    probe.command(PlayerCommand::Play);
    probe.wait(|s| s.state == PlaybackState::Playing && s.position_ms < 1000);
    probe.command(PlayerCommand::Seek { position_ms: 5000 });
    probe.wait(|s| s.state == PlaybackState::Playing && s.position_ms > 5400);
    server
        .blocked
        .store(true, std::sync::atomic::Ordering::Release);
    let buffering = probe.wait(|s| s.state == PlaybackState::Buffering);
    std::thread::sleep(Duration::from_millis(500));
    let position = (probe.player.property_f64("time-pos").unwrap() * 1000.0) as u64;
    assert!(position.abs_diff(buffering.position_ms) < 100);
    server
        .blocked
        .store(false, std::sync::atomic::Ordering::Release);
    probe
        .wait(|s| s.state == PlaybackState::Playing && s.position_ms > buffering.position_ms + 200);
    probe.command(PlayerCommand::Close);
    let (accepted, stale) = (probe.accepted, probe.stale);
    drop(probe);
    println!(
        "NATIVE_PLAYER_RUNTIME pause/seek ±/0.5×/2×/320→640/buffering/close PASS; accepted={}, stale={}",
        accepted, stale
    );
}
