//! #218 插帧 spike 的 GPU 侧，原样复用 `spikes/slint-mpv/src/interp/`。
//!
//! 那边的 `gpu.rs` 只依赖 `super::sched`、`super::shaders`、glow 与 anyhow，
//! 所以这里把三个文件按同样的相对位置挂起来就能编。iOS 的 GLES 3.0 上下文
//! 跑的是与 Android（GLES 3.2）、macOS（GL 4.1）同一份 GLSL。

// 共用代码里有桌面 / Android 才用到的部分。
#[allow(dead_code)]
#[path = "../../slint-mpv/src/interp/gpu.rs"]
pub mod gpu;
#[allow(dead_code)]
#[path = "../../slint-mpv/src/interp/sched.rs"]
pub mod sched;
#[allow(dead_code)]
#[path = "../../slint-mpv/src/interp/shaders.rs"]
pub mod shaders;
