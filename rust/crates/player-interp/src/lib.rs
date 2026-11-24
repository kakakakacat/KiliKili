//! 播放器的 GPU 视频插帧层（#211）。算法与实测在 #218
//! （`docs/m0/spikes/2026-11-24-interp-218.md`），接入在 #219。
//!
//! 插在 mpv 和宿主 UI 之间：
//!
//! ```text
//! mpv 有新帧 → 画进环里的一个槽（视频原始尺寸 RGBA8）→ 该槽的 1/4 亮度
//! 选帧（sched）→ 这一对还没有运动场就估计一次（同一对的多张输出复用）
//!             → 输出 pass 画进宿主的 FBO（letterbox）→ 宿主再画弹幕与控件
//! ```
//!
//! 分层：
//!
//! - [`policy`]：设置档位 + 片源帧率 + 屏幕刷新率 → 插不插、插到多少；
//! - [`governor`]：持续掉帧、补帧超时、热降频时降档，稳定后回升；
//! - [`sched`]：源帧环与选帧；
//! - [`refresh`]：没有平台读数时按渲染回调间隔测刷新率；
//! - [`controller`]：把上面三样串成每个 vsync 的决定，处理生命周期事件与
//!   音画延迟。与 GL 无关，单测覆盖；
//! - [`gpu`] / [`shaders`]：GLSL 块匹配与合成，一份源码跑 GL 3.3 / GLES 3.0；
//! - [`engine`]：控制面 + GPU 侧，给宿主渲染回调的接口。
//!
//! 宿主负责的（AGENTS.md §2「平台 API 调用」）：让 mpv 画进给定的 FBO、
//! 异步设 `audio-delay`、向系统请求刷新率并把实际刷新率与温控状态报进来、
//! 把生命周期事件转成 [`controller::Event`]。界面文案也留在宿主。

// GL 调用全是 unsafe（glow 的函数指针表）。范围限于 gpu.rs 与 engine.rs 的
// attach_gl，其余模块是纯数据。
#![allow(unsafe_code)]

pub mod controller;
pub mod engine;
pub mod governor;
pub mod gpu;
pub mod policy;
pub mod refresh;
pub mod sched;
pub mod shaders;

pub use controller::{Controller, Event, Plan, Status};
pub use engine::{Interpolator, Presented, SourceTarget};
pub use governor::Thermal;
pub use gpu::{Gpu, GpuError, Mode, Params};
pub use policy::{Cadence, Decision, DirectReason, Setting};
