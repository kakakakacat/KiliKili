//! 不带视频的后端，实现同一个 [`PlayerPort`]。
//!
//! 两个用处：
//!
//! 1. **没有 libmpv 也能跑弹幕层**——Slint 的长列表/文本渲染能力与 mpv
//!    无关，这部分结论不该被「mac 上还没装 mpv」挡住；
//! 2. 给出**弹幕层单独的帧率基线**。只有拿它和带视频的那次对照，才能说清
//!    掉帧是渲染文本造成的还是视频合成造成的。本次没有这个对照就只能写
//!    「两者合计」，那等于没测出原因。
//!
//! 它同样走「发了 Seek 不等于 seek 完成」那套：seek 之后先停一拍再报
//! `SeekCompleted`，这样上层的 seek 处理逻辑在两个后端下走的是同一条路。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::Result;
use bili_domain::player::{
    PlaybackEvent, PlaybackState, PlayerCommand, PlayerEventSink, PlayerPort, PlayerSnapshot,
};
use bili_domain::session::{Generation, GenerationGate, Stamped};

#[derive(Debug)]
struct State {
    playing: bool,
    position_ms: u64,
    duration_ms: u64,
    speed: f32,
    emit: Generation,
    target: Generation,
    awaiting_restart: bool,
    last_tick: Instant,
}

pub struct FakePlayer {
    gate: GenerationGate,
    state: Arc<Mutex<State>>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl std::fmt::Debug for FakePlayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FakePlayer").finish_non_exhaustive()
    }
}

impl FakePlayer {
    pub fn new(
        gate: GenerationGate,
        duration_ms: u64,
        sink: Box<dyn PlayerEventSink>,
    ) -> Result<Self> {
        let generation = gate.current();
        let state = Arc::new(Mutex::new(State {
            playing: false,
            position_ms: 0,
            duration_ms,
            speed: 1.0,
            emit: generation,
            target: generation,
            awaiting_restart: false,
            last_tick: Instant::now(),
        }));
        let stop = Arc::new(AtomicBool::new(false));
        let thread = {
            let state = Arc::clone(&state);
            let stop = Arc::clone(&stop);
            let session = gate.session();
            std::thread::Builder::new()
                .name("fake-player".to_owned())
                .spawn(move || {
                    let mut sink = sink;
                    while !stop.load(Ordering::Acquire) {
                        std::thread::sleep(Duration::from_millis(40));
                        let Ok(mut state) = state.lock() else { break };
                        let now = Instant::now();
                        let delta = now.duration_since(state.last_tick);
                        state.last_tick = now;
                        if state.playing {
                            let advance = (delta.as_secs_f32() * 1000.0 * state.speed) as u64;
                            state.position_ms =
                                (state.position_ms + advance).min(state.duration_ms);
                        }
                        if state.awaiting_restart {
                            state.awaiting_restart = false;
                            state.emit = state.target;
                            let position = state.position_ms;
                            sink.emit(Stamped::new(
                                session,
                                state.emit,
                                PlaybackEvent::SeekCompleted {
                                    position_ms: position,
                                },
                            ));
                        }
                        let snapshot = PlayerSnapshot {
                            position_ms: state.position_ms,
                            duration_ms: Some(state.duration_ms),
                            state: if state.position_ms >= state.duration_ms {
                                PlaybackState::Ended
                            } else if state.playing {
                                PlaybackState::Playing
                            } else {
                                PlaybackState::Paused
                            },
                            speed: state.speed,
                            video_track: None,
                            audio_track: None,
                        };
                        sink.emit(Stamped::new(
                            session,
                            state.emit,
                            PlaybackEvent::Snapshot(snapshot),
                        ));
                    }
                })?
        };
        Ok(Self {
            gate,
            state,
            stop,
            thread: Some(thread),
        })
    }
}

impl Drop for FakePlayer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl PlayerPort for FakePlayer {
    type Error = anyhow::Error;

    fn dispatch(&mut self, command: Stamped<PlayerCommand>) -> Result<()> {
        let Some(command) = command.accept(&self.gate) else {
            return Ok(());
        };
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("fake player 状态被毒化"))?;
        match command {
            PlayerCommand::Open { .. } => {
                state.position_ms = 0;
                state.playing = true;
                state.emit = self.gate.current();
                state.target = state.emit;
            }
            PlayerCommand::Play => state.playing = true,
            PlayerCommand::Pause => state.playing = false,
            PlayerCommand::Seek { position_ms } => {
                state.position_ms = position_ms.min(state.duration_ms);
                state.target = self.gate.current();
                state.awaiting_restart = true;
            }
            PlayerCommand::SetSpeed(speed) => state.speed = speed,
            PlayerCommand::Close => state.playing = false,
            _ => {}
        }
        Ok(())
    }
}
