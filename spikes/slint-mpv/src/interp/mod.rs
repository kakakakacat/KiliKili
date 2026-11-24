//! #218 插帧 spike 的应用层。GPU 侧、选帧、档位与回退已经提成正式 crate
//! `player-interp`（#219，`rust/crates/player-interp`），这里只剩实验台：
//! 参数、播放循环、实验室模式、记录。
//!
//! 两条播放路径：
//!
//! - 不带 `--setting`：#218 的手工路径，`--mode` / `--delay-ms` / `--skip-dups`
//!   这些旋钮直接拧 GPU 与选帧，用来做参数对照；
//! - 带 `--setting`：走 `player_interp::Interpolator`，档位、延迟、回退、
//!   生命周期都由它决定，与正式接入的形态相同。#219 的真机验收用这一条。

pub mod app;

pub use player_interp::gpu::Stage;
pub use player_interp::sched;
pub use player_interp::{Gpu, Mode, Params};
