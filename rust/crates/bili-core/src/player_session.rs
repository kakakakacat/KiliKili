//! 播放会话：给命令加代际戳，并只接收当前后端的事实事件。

use bili_domain::player::{PlaybackEvent, PlayerCommand, PlayerPort, PlayerSnapshot};
use bili_domain::session::{Generation, GenerationGate, SessionId, Stamped};

/// 一个播放路由的命令/事件闸门。此处不保存独立计时器。
#[derive(Debug)]
pub struct PlayerSession<P: PlayerPort> {
    gate: GenerationGate,
    port: P,
    snapshot: Option<PlayerSnapshot>,
}

impl<P: PlayerPort> PlayerSession<P> {
    #[must_use]
    pub fn new(session_id: SessionId, port: P) -> Self {
        Self {
            gate: GenerationGate::new(session_id),
            port,
            snapshot: None,
        }
    }

    #[must_use]
    pub fn generation(&self) -> Generation {
        self.gate.current()
    }

    #[must_use]
    pub fn snapshot(&self) -> Option<PlayerSnapshot> {
        self.snapshot
    }

    #[must_use]
    pub fn port(&self) -> &P {
        &self.port
    }

    /// 切资源、Seek、切轨、关闭会作废此前在途事件。
    ///
    /// # Errors
    /// 返回底层适配器的命令错误；即使派发失败，旧代际也已作废。
    pub fn dispatch(&mut self, command: PlayerCommand) -> Result<(), P::Error> {
        if matches!(
            command,
            PlayerCommand::Open { .. }
                | PlayerCommand::Seek { .. }
                | PlayerCommand::SelectTracks { .. }
                | PlayerCommand::Close
        ) {
            self.gate.bump();
            self.snapshot = None;
        }

        self.port.dispatch(Stamped::new(
            self.gate.session(),
            self.gate.current(),
            command,
        ))
    }

    /// 只采纳当前播放会话/代际的后端事件。位置来自后端快照本身。
    pub fn accept_event(&mut self, event: Stamped<PlaybackEvent>) -> Option<PlaybackEvent> {
        let event = event.accept(&self.gate)?;
        match event {
            PlaybackEvent::Snapshot(snapshot) => self.snapshot = Some(snapshot),
            PlaybackEvent::Closed => self.snapshot = None,
            _ => {}
        }
        Some(event)
    }
}
