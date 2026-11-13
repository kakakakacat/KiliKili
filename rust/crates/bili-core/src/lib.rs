//! 应用层：UseCase、播放器意图编排、事件与状态快照（RFC-0001 §16.6）。
//!
//! 本 crate 不得依赖 Flutter Widget 或 Slint 具体类型。

pub mod cdn;
pub mod cover_scrim;
pub mod media_select;
pub mod playback;
pub mod player_session;
pub mod subtitle_format;

#[cfg(test)]
mod tests {
    #[test]
    fn workspace_builds() {}
}
