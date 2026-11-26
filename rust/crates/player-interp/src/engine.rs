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

/// 源帧槽的尺寸。
///
/// 默认是视频的显示尺寸：mpv 只做解码与色彩转换，缩放交给我们的输出 pass，
/// 运动估计也在源分辨率上做（最便宜）。
///
/// 用户开了**放大类** mpv 着色器（Anime4K 之类，`glsl-shaders` 里带
/// `WHEN OUTPUT.w > HOOKED.w` 条件的）时这样做等于把它们关掉了——mpv 往源尺寸
/// 的槽里画，输出不比输入大，放大着色器的条件不成立。这时槽改成视频按比例
/// 放进窗口后的尺寸：mpv 在槽里做完放大，我们在放大后的画面上插帧，输出
/// pass 再 1:1 贴上去。
///
/// 代价：运动估计在 1/4 亮度上做，槽变大多少成本就跟着平方地涨；搜索半径
/// 按槽像素算，放大 2 倍时能跟上的源像素位移减半。所以只放大、不缩小，
/// 也不超过 `max_pixels`（宿主按机型给，例如低端机 1280×720）。
#[must_use]
pub fn slot_size(
    video: (i32, i32),
    window: (i32, i32),
    upscale_shaders: bool,
    max_pixels: i64,
) -> (i32, i32) {
    let (vw, vh) = video;
    let (ww, wh) = window;
    if !upscale_shaders || vw <= 0 || vh <= 0 || ww <= 0 || wh <= 0 {
        return video;
    }
    let fit = (f64::from(ww) / f64::from(vw)).min(f64::from(wh) / f64::from(vh));
    let cap = (max_pixels as f64 / (f64::from(vw) * f64::from(vh))).sqrt();
    let scale = fit.min(cap);
    if scale <= 1.0 {
        return video;
    }
    // 偶数尺寸：1/4 亮度那一级按 2×2 取。
    let even = |x: f64| ((x.round() as i32) / 2 * 2).max(2);
    (even(f64::from(vw) * scale), even(f64::from(vh) * scale))
}

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

    /// 见 [`Controller::set_source_fps_hint`]。
    pub fn set_source_fps_hint(&mut self, media_fps: Option<f32>) {
        self.ctl.set_source_fps_hint(media_fps);
    }

    /// 见 [`Controller::set_speed`]。
    pub fn set_speed(&mut self, speed: f64) {
        self.ctl.set_speed(speed);
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

    /// 准备接收一张源帧。`video` 是槽的尺寸：一般是视频的显示尺寸（宿主在
    /// 后台线程取 mpv 的 `dwidth` / `dheight`，渲染线程上不要同步取属性，
    /// #218 §5 坑 1）；开了放大着色器时见 [`slot_size`]。
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

#[cfg(test)]
mod tests {
    use super::slot_size;

    const BIG: i64 = i64::MAX;

    #[test]
    fn without_upscale_shaders_the_slot_is_the_video() {
        assert_eq!(
            slot_size((1280, 720), (2796, 1290), false, BIG),
            (1280, 720)
        );
    }

    #[test]
    fn upscale_shaders_get_the_fitted_window_size() {
        // iPhone 16 Pro Max 横屏，720p 片源：按高度放大 1.79 倍。
        assert_eq!(
            slot_size((1280, 720), (2796, 1290), true, BIG),
            (2292, 1290)
        );
    }

    #[test]
    fn never_smaller_than_the_video() {
        assert_eq!(
            slot_size((1920, 1080), (1280, 720), true, BIG),
            (1920, 1080)
        );
    }

    #[test]
    fn capped_by_pixels() {
        let (w, h) = slot_size((640, 360), (2400, 1080), true, 1280 * 720);
        assert_eq!((w, h), (1280, 720));
    }
}
