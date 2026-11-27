//! 底栏液态玻璃透镜运动规则的 Dart 导出（issue #266）。
//!
//! 每帧调用一次，所以是 `#[frb(sync)]`：纯计算、几十次浮点运算，改成
//! Future 反而要多排一次事件循环，动画必然晚一帧。
//!
//! 规则在 `bili_core::glass_droplet`，本模块只做类型转换（RFC-0001 §3：
//! 桥接层不承载业务逻辑）。坐标单位是「格」，见那边的模块文档。

use bili_core::glass_droplet as core;
use flutter_rust_bridge::frb;

/// 透镜的运动状态。宿主原样保存，每帧传回 [`glass_droplet_step`]，不要
/// 自己改字段。
#[derive(Debug, Clone, Copy)]
pub struct GlassDropletState {
    pub left: f64,
    pub left_velocity: f64,
    pub right: f64,
    pub right_velocity: f64,
    pub lift: f64,
    pub lift_velocity: f64,
    pub target: f64,
    pub direction: i8,
}

/// 一帧要画的几何，单位是格：第 `i` 个 tab 占 `[i, i + 1)`。宿主乘以每
/// 格宽度换算成像素。
#[derive(Debug, Clone, Copy)]
pub struct GlassDropletShape {
    pub left: f64,
    pub right: f64,
    /// 高度相对静止胶囊的比例，只随升起变化。
    pub height_scale: f64,
    /// 移动中的变宽程度 0..1。
    pub stretch: f64,
    /// 升起程度 0..1：0 是静止的灰色胶囊，1 是完全升起的透镜。
    pub lift: f64,
    /// 透镜下方内容的放大倍数；等于 1 时宿主不叠透镜层。
    pub lens_zoom: f64,
}

#[derive(Debug, Clone, Copy)]
pub struct GlassDropletFrame {
    pub state: GlassDropletState,
    pub shape: GlassDropletShape,
    /// 已静止：宿主可以停掉帧回调，直到下次换目标或按压。
    pub settled: bool,
}

impl From<GlassDropletState> for core::DropletState {
    fn from(s: GlassDropletState) -> Self {
        Self {
            left: s.left,
            left_velocity: s.left_velocity,
            right: s.right,
            right_velocity: s.right_velocity,
            lift: s.lift,
            lift_velocity: s.lift_velocity,
            target: s.target,
            direction: s.direction,
        }
    }
}

impl From<core::DropletState> for GlassDropletState {
    fn from(s: core::DropletState) -> Self {
        Self {
            left: s.left,
            left_velocity: s.left_velocity,
            right: s.right,
            right_velocity: s.right_velocity,
            lift: s.lift,
            lift_velocity: s.lift_velocity,
            target: s.target,
            direction: s.direction,
        }
    }
}

impl From<core::DropletFrame> for GlassDropletFrame {
    fn from(f: core::DropletFrame) -> Self {
        Self {
            state: f.state.into(),
            shape: GlassDropletShape {
                left: f.shape.left,
                right: f.shape.right,
                height_scale: f.shape.height_scale,
                stretch: f.shape.stretch,
                lift: f.shape.lift,
                lens_zoom: f.shape.lens_zoom,
            },
            settled: f.settled,
        }
    }
}

/// 静止停在第 `index` 格的状态。
#[frb(sync)]
#[must_use]
pub fn glass_droplet_rest(index: f64) -> GlassDropletState {
    core::rest(index).into()
}

/// 把透镜推进 `dt_seconds` 秒。`target` 是目标格（拖动时可为小数），
/// `pressed` 为 true 时透镜保持升起。
#[frb(sync)]
#[must_use]
pub fn glass_droplet_step(
    state: GlassDropletState,
    target: f64,
    pressed: bool,
    dt_seconds: f64,
) -> GlassDropletFrame {
    core::step(state.into(), target, pressed, dt_seconds).into()
}

/// 手指按下时透镜的目标格。`pointer` 单位是格。
#[frb(sync)]
#[must_use]
pub fn glass_droplet_press_target(pointer: f64, count: u32) -> f64 {
    core::press_target(pointer, count)
}

/// 拖动时透镜跟手的目标（中心对准手指，夹在首尾两格之间）。
#[frb(sync)]
#[must_use]
pub fn glass_droplet_drag_target(pointer: f64, count: u32) -> f64 {
    core::drag_target(pointer, count)
}

/// 松手后吸附到的 tab 下标。
#[frb(sync)]
#[must_use]
pub fn glass_droplet_snap_index(target: f64, count: u32) -> u32 {
    core::snap_index(target, count)
}
