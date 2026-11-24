//! 插帧的 GPU 侧与选帧。#218 时这里用 `#[path]` 引 slint-mpv 的源文件；
//! #219 起它们提成了正式 crate `player-interp`，这里只是转一下名字，让
//! #218 的手工路径（`SPIKE_INTERP`）照旧能编。

pub use player_interp::{gpu, sched};
