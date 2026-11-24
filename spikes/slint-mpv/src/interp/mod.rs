//! #218 插帧 spike 的 GPU 侧：源帧环的纹理、亮度金字塔、运动场、输出合成，
//! 以及 GPU 计时。调度（选哪两帧、相位多少）在 [`sched`]，与 GL 无关。
//!
//! 每个 vsync 的流程（都在 Slint 的 `BeforeRendering` 里）：
//!
//! ```text
//! mpv 有新帧 → 画进环里的一个槽（视频原始尺寸）→ 该槽的 1/4 亮度
//! 选帧（sched）→ 这一对还没有运动场就估计一次（同一对的多张输出复用）
//!             → 输出 pass 画进 Slint 的 FBO（letterbox），Slint 再画 UI
//! ```
//!
//! 我们的 GL 调用夹在 mpv 和 Slint 之间，所以进出都保存 / 恢复会被改动的
//! GL 状态，免得 Slint（femtovg）拿到一个被换掉的 VAO 或 program。

pub mod app;
pub mod gpu;
pub mod sched;
pub mod shaders;

pub use gpu::{Gpu, Mode, Params, Stage};
