//! 底栏液态水滴形变的 Dart 导出（issue #266）。
//!
//! 每帧调用一次，所以是 `#[frb(sync)]`：纯计算、几十次浮点运算，改成
//! Future 反而要多排一次事件循环，动画必然晚一帧。
//!
//! 规则在 `bili_core::glass_droplet`，本模块只做类型转换（RFC-0001 §3：
//! 桥接层不承载业务逻辑）。坐标单位是「格」，见那边的模块文档。

use bili_core::glass_droplet as core;
use flutter_rust_bridge::frb;

/// 水滴的运动状态。宿主原样保存，每帧传回 [`glass_droplet_step`]，不要
/// 自己改字段。
#[derive(Debug, Clone, Copy)]
pub struct GlassDropletState {
    pub left: f64,
    pub left_velocity: f64,
    pub right: f64,
    pub right_velocity: f64,
    pub scale: f64,
    pub scale_velocity: f64,
    pub target: f64,
    pub direction: i8,
}

/// 一帧要画的几何，单位是格：第 `i` 个 tab 占 `[i, i + 1)`。宿主乘以每
/// 格宽度换算成像素。
#[derive(Debug, Clone, Copy)]
pub struct GlassDropletShape {
    pub left: f64,
    pub right: f64,
    /// 高度相对静止高度的比例，已含按压缩放。
    pub height_scale: f64,
    /// 0 = 静止形状，1 = 拉到上限。供 shader 决定中段收细多少。
    pub stretch: f64,
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
            scale: s.scale,
            scale_velocity: s.scale_velocity,
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
            scale: s.scale,
            scale_velocity: s.scale_velocity,
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

/// 把水滴推进 `dt_seconds` 秒。`target` 是目标格（拖动时可为小数），
/// `pressed` 为 true 时整体鼓起。
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
