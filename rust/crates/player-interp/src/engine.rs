//! 控制面 + GPU 侧合在一起，给宿主的渲染回调用。
//!
//! 宿主在渲染回调里的写法（mpv 部分是宿主的，这里不依赖 libmpv）：
//!
//! ```text
//! RenderingSetup     → interp.attach_gl(loader)
//! BeforeRendering    →
//!     let cost = Instant::now();
//!     match interp.begin_vsync(wall_us) {
//!         Plan::Direct => {
//!             if mpv 有新帧 && 不是重画 { interp.observe_direct(target_time, now_us) }
//!             mpv.render(宿主 FBO)
//!         }
//!         Plan::Interp => {
//!             while mpv 有新帧 {
//!                 let t = interp.source_target(视频尺寸, 是否重画)?;
//!                 mpv.render(t.fbo, t.width, t.height);
//!                 interp.commit_source(t, target_time, now_us);
//!             }
//!             if interp.present(宿主 FBO, 窗口尺寸, now_us) == Presented::Nothing {
//!                 mpv.render(宿主 FBO)    // 环还空着：照直通画，不黑屏
//!             }
//!         }
//!     }
//!     interp.end_vsync(cost.elapsed())
//!     if let Some(d) = interp.take_av_delay_change() { 异步设 mpv audio-delay = d }
//! RenderingTeardown  → interp.detach_gl()
//! ```
//!
//! 三个 #218 §5 的坑在这一层已经避开：渲染线程上不取 mpv 属性（视频尺寸由
//! 宿主后台线程取好传进来）、不用 `glReadPixels`（统计走 transform feedback）、
//! 着色器循环边界是编译期常量。

use std::ffi::{CStr, c_void};

use crate::controller::{Controller, Event, Plan, Status};
use crate::governor::Thermal;
use crate::gpu::{Gpu, GpuError, Mode, Params};
use crate::policy::Setting;
use crate::sched::Pick;

/// 环的槽数：一对在用 + mpv 提前交的一张 + 跳重复帧时多跨的一到两张，
/// 再留一张余量（#218 实验台 Android / iOS 都用 8）。
pub const RING_CAPACITY: usize = 8;

/// mpv 下一张源帧该画进哪里。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceTarget {
    /// 交给 `MPV_RENDER_PARAM_OPENGL_FBO`。
    pub fbo: i32,
    pub width: i32,
    pub height: i32,
    slot: usize,
    redraw: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Presented {
    /// 画好了。
    Frame,
    /// 环里还没有帧（刚从直通切过来），宿主照直通画这一帧。
    Nothing,
}

#[derive(Debug)]
pub struct Interpolator {
    ctl: Controller,
    gpu: Option<Gpu>,
    params: Params,
    mode: Mode,
}

impl Interpolator {
    /// 默认参数：运动补偿、R=4、四分之一像素、SAD 隔点、跳重复帧。
    ///
    /// R=4 而不是实验台默认的 6：BBB 六段上 SSIM 0.937 对 0.938，几乎一样，
    /// Adreno 610 上运动估计 4.9 ms 对 7.8 ms（#218 §2.2、§4.2）。每帧最大位移
    /// 32 源像素；大位移场景的取舍留给 #219 真机验收的番剧素材。
    #[must_use]
    pub fn new() -> Self {
        Self::with(
            Params {
                radius: 4,
                ..Params::default()
            },
            Mode::Mc,
            true,
        )
    }

    #[must_use]
    pub fn with(params: Params, mode: Mode, skip_dups: bool) -> Self {
        Self {
            ctl: Controller::new(RING_CAPACITY, skip_dups, params.dup_thresh),
            gpu: None,
            params,
            mode,
        }
    }

    #[must_use]
    pub fn controller(&self) -> &Controller {
        &self.ctl
    }

    pub fn controller_mut(&mut self) -> &mut Controller {
        &mut self.ctl
    }

    #[must_use]
    pub fn params(&self) -> &Params {
        &self.params
    }

    #[must_use]
    pub fn gpu(&self) -> Option<&Gpu> {
        self.gpu.as_ref()
    }

    pub fn set_setting(&mut self, setting: Setting) {
        self.ctl.set_setting(setting);
    }

    pub fn set_thermal(&mut self, thermal: Thermal) {
        self.ctl.set_thermal(thermal);
    }

    pub fn set_display_refresh(&mut self, hz: Option<f32>) {
        self.ctl.set_display_refresh(hz);
    }

    pub fn on_event(&mut self, event: Event) {
        self.ctl.on_event(event);
    }

    #[must_use]
    pub fn status(&self) -> Status {
        self.ctl.status()
    }

    pub fn take_av_delay_change(&mut self) -> Option<i64> {
        self.ctl.take_av_delay_change()
    }

    /// 渲染上下文建好时调用。失败时插帧整体退回直通，错误交给宿主记录。
    ///
    /// # Safety
    /// GL 上下文必须是 current 的，`loader` 能解析当前上下文的函数。
    pub unsafe fn attach_gl(
        &mut self,
        loader: &dyn Fn(&CStr) -> *const c_void,
    ) -> Result<(), GpuError> {
        self.gpu = None;
        let made = unsafe { Gpu::new(loader, self.ctl.ring_capacity()) };
        self.ctl.set_gpu_available(made.is_ok());
        self.gpu = Some(made?);
        Ok(())
    }

    /// 渲染上下文要销毁了（GL 上下文此时仍是 current 的，纹理能正常释放）。
    pub fn detach_gl(&mut self) {
        self.gpu = None;
        self.ctl.set_gpu_available(false);
        self.ctl.on_event(Event::ContextLost);
    }

    pub fn begin_vsync(&mut self, wall_us: i64) -> Plan {
        if self.mode == Mode::Direct {
            return Plan::Direct;
        }
        self.ctl.begin_vsync(wall_us)
    }

    pub fn observe_direct(&mut self, raw_target_time: i64, now_us: i64) {
        self.ctl.observe_direct(raw_target_time, now_us);
    }

    /// 准备接收一张源帧。`video` 是视频的显示尺寸（宿主在后台线程取 mpv 的
    /// `dwidth` / `dheight`，渲染线程上不要同步取属性，#218 §5 坑 1）。
    pub fn source_target(
        &mut self,
        video: (i32, i32),
        redraw: bool,
    ) -> Result<SourceTarget, GpuError> {
        let Some(gpu) = self.gpu.as_mut() else {
            return Err(GpuError::from("GPU 侧还没初始化"));
        };
        if gpu.ensure_size(video.0, video.1)? {
            self.ctl.on_ring_reallocated();
        }
        let slot = self.ctl.slot_for(redraw);
        Ok(SourceTarget {
            fbo: gpu.slot_fbo(slot),
            width: video.0,
            height: video.1,
            slot,
            redraw,
        })
    }

    /// mpv 画完这张源帧之后调用。
    pub fn commit_source(&mut self, target: SourceTarget, raw_target_time: i64, now_us: i64) {
        if let Some(gpu) = self.gpu.as_mut() {
            gpu.prepare_slot(target.slot);
        }
        self.ctl
            .push_source(raw_target_time, now_us, target.slot, target.redraw);
    }

    /// 合成这一帧，画进宿主的 FBO（`fbo` 为 0 表示默认帧缓冲），按比例居中。
    pub fn present(&mut self, fbo: i32, size: (i32, i32), now_us: i64) -> Presented {
        let Some(gpu) = self.gpu.as_mut() else {
            return Presented::Nothing;
        };
        let pick = self.ctl.pick(now_us);
        let out = Gpu::framebuffer(fbo);
        let presented = match pick {
            Pick::Empty => Presented::Nothing,
            Pick::Pair { a, b, p } => {
                if self.mode == Mode::Mc {
                    gpu.ensure_field(&a, &b, &self.params);
                }
                gpu.present(out, size, &a, &b, p, self.mode, &self.params);
                Presented::Frame
            }
            Pick::Starved { a } | Pick::Early { a } => {
                gpu.present(out, size, &a, &a, 0.0, Mode::Hold, &self.params);
                Presented::Frame
            }
        };
        let (_, stats) = gpu.poll();
        for s in stats {
            self.ctl.on_pair_changed(s.pair.1, s.changed);
        }
        presented
    }

    /// 这一帧插帧段的 CPU 耗时（微秒）。
    pub fn end_vsync(&mut self, cost_us: i64) {
        self.ctl.end_vsync(cost_us);
    }
}

impl Default for Interpolator {
    fn default() -> Self {
        Self::new()
    }
}
