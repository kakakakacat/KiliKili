//! 插帧的控制面：与 GL 无关的那一半，便于单测。
//!
//! 每个 vsync（宿主渲染回调）里：
//!
//! ```text
//! begin_vsync(now, wall)          → 测刷新率、喂回退控制器、定档 → Plan
//!   Plan::Direct：宿主照旧让 mpv 直接画进自己的 FBO；有新帧就 observe_direct
//!   Plan::Interp：每张新源帧 slot_for → mpv 画进槽 → push_source
//!                 pick(now) → 画输出
//! end_vsync(cost)                 → 记下这一帧插帧段的耗时，下一帧交给回退控制器
//! ```
//!
//! 时间有两条轴，不能混：
//!
//! - `now_us`：mpv 时钟（`mpv_get_time_us`），源帧的 `target_time` 在这条轴上；
//! - `wall_us`：宿主的单调时钟，只用来量 vsync 间隔与回退的时间窗。

use crate::governor::{Governor, Thermal};
use crate::policy::{self, Cadence, Decision, DirectReason, Setting};
use crate::refresh::{Debounced, RefreshMeter};
use crate::sched::{self, Pick, Push, Ring};

/// 宿主告诉我们的生命周期事件（#219「生命周期」）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// 拖动进度条。
    Seek,
    /// 切集、切清晰度：片源换了，帧率可能也换了。
    SourceChanged,
    /// 退到后台（且没进画中画）。
    Background,
    Foreground,
    PipEnter,
    PipExit,
    /// 渲染上下文没了（Android 表面重建、Slint `RenderingTeardown`），
    /// 环里的纹理随之失效。
    ContextLost,
}

/// 这一帧宿主该走哪条路。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Plan {
    /// mpv 直接画进宿主的 FBO（ADR-004 原路径）。
    Direct,
    /// 新源帧画进环，再由我们合成输出。
    Interp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Active,
    Background,
    Pip,
}

/// 播放信息里显示的状态（#219：「目标 60 FPS · 当前 30 FPS」）。
/// 文案与格式留在宿主。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Status {
    pub setting: Setting,
    /// 用户选的目标（「跟随」时为当前刷新率与上限取小）。关闭时 `None`。
    pub requested_fps: Option<f32>,
    /// 屏幕实际刷新率（平台读数优先，否则按 vsync 间隔测）。
    pub display_hz: Option<f32>,
    pub source_fps: Option<f32>,
    /// 实际输出的不同画面数 / 秒。直通时等于源帧率。
    pub output_fps: Option<f32>,
    pub decision: Decision,
    pub thermal: Thermal,
    /// 当前给音频加的延迟（微秒）。
    pub av_delay_us: i64,
    /// 做过运动估计、统计已回读的帧对数，以及其中判为重复帧的对数。
    pub pairs_measured: u64,
    pub dup_pairs: u64,
}

/// 双向插帧推迟画面时，在「一个（或两个）源帧间隔」之外再留的余量。
/// #218 §7：delay = 一个源帧间隔 + 约 15 ms。
const DELAY_MARGIN_US: i64 = 15_000;
/// 想要的延迟与当前值相差不到这个比例，就不改 `audio-delay`（每改一次声音
/// 都会跳一下）。只有片源帧率真的变了（倍速、换片源）才值得改。
const DELAY_HYSTERESIS: f64 = 0.10;
/// 源帧率的新估计要连续这么多张源帧不变才采纳。
const SRC_FPS_STABLE: u32 = 6;
/// 非零延迟改成另一个非零值之前，新值要稳定这么多个 vsync。
const DELAY_STABLE_VSYNCS: u32 = 30;

#[derive(Debug)]
pub struct Controller {
    setting: Setting,
    follow_cap: f32,
    skip_dups: bool,
    dup_thresh: f32,
    ring: Ring,
    governor: Governor,
    phase: Phase,
    gpu_available: bool,

    reported_hz: Debounced,
    meter: RefreshMeter,

    vsync_count: u64,
    decision: Decision,
    last_t_out: Option<i64>,
    last_direct_t: Option<i64>,
    pending_cost_us: i64,

    av_delay_us: i64,
    av_delay_taken: Option<i64>,
    /// 测量得到、已采纳的源帧率（墙钟，已吸附），见 [`Controller::source_fps`]。
    src_fps: Option<f32>,
    /// 宿主从 mpv 读到的片源帧率（**媒体时间**，`container-fps`）。
    fps_hint: Option<f32>,
    /// 与 `src_fps` 不同的新估计，及它已连续出现的源帧数。
    src_fps_candidate: Option<(f32, u32)>,
    /// 延迟从一个非零值改到另一个非零值之前，新值已经连续出现了几个 vsync。
    av_delay_candidate: Option<(i64, u32)>,
    /// 下一次重算延迟时直接采纳（倍速、片源这种宿主明确告知的变化，不是
    /// 估计抖动，不用去抖）。
    av_delay_force: bool,
    /// mpv 的 `speed`。`target_time` 是墙钟，所以源帧间隔已经含倍速；
    /// 只有换算 `audio-delay`（媒体时间）时要用它。
    speed: f64,

    out_window_start: Option<i64>,
    out_distinct: u32,
    out_last: Option<(u64, u64, u32)>,
    output_fps: Option<f32>,

    pairs_measured: u64,
    dup_pairs: u64,
}

impl Controller {
    /// `ring_capacity` 至少 3（一对在用、一张在写）；`skip_dups` 打开时跳过
    /// 一拍二 / 一拍三的重复帧，代价是前瞻多一个源帧间隔（#218 §3）。
    #[must_use]
    pub fn new(ring_capacity: usize, skip_dups: bool, dup_thresh: f32) -> Self {
        Self {
            setting: Setting::Off,
            follow_cap: policy::FOLLOW_CAP_UNVERIFIED,
            skip_dups,
            dup_thresh,
            ring: Ring::new(ring_capacity.max(3)),
            governor: Governor::default(),
            phase: Phase::Active,
            gpu_available: false,
            reported_hz: Debounced::default(),
            meter: RefreshMeter::default(),
            vsync_count: 0,
            decision: Decision::Direct(DirectReason::Off),
            last_t_out: None,
            last_direct_t: None,
            pending_cost_us: 0,
            av_delay_us: 0,
            av_delay_taken: None,
            av_delay_candidate: None,
            av_delay_force: false,
            src_fps: None,
            src_fps_candidate: None,
            fps_hint: None,
            speed: 1.0,
            out_window_start: None,
            out_distinct: 0,
            out_last: None,
            output_fps: None,
            pairs_measured: 0,
            dup_pairs: 0,
        }
    }

    // ---- 宿主喂进来的设置与平台读数 ----

    pub fn set_setting(&mut self, setting: Setting) {
        if self.setting != setting {
            self.setting = setting;
            // 新档位重新评估；温控读数保留。
            let thermal = self.governor.thermal();
            self.governor = Governor::default();
            self.governor.set_thermal(thermal);
        }
    }

    #[must_use]
    pub fn setting(&self) -> Setting {
        self.setting
    }

    /// 「跟随屏幕刷新率」的上限。默认 [`policy::FOLLOW_CAP_UNVERIFIED`]。
    pub fn set_follow_cap(&mut self, cap: f32) {
        self.follow_cap = cap;
    }

    #[must_use]
    pub fn follow_cap(&self) -> f32 {
        self.follow_cap
    }

    /// 平台报告的刷新率（Android 刷新率回调、iOS `maximumFramesPerSecond` /
    /// display link 的实际间隔）。给了就以它为准，否则按 vsync 间隔测。
    ///
    /// 平台读数优先的理由：宿主某一帧画慢了，测出来的间隔是两个周期，60 Hz
    /// 会被误判成 30 Hz；平台读数不受这个影响。读数同样要去抖，见
    /// [`crate::refresh`] 模块文档里 iPhone 的 40 / 48 Hz 交替。
    ///
    /// 宿主可以每帧都调（iOS 从 display link 的帧间隔取），也可以只在平台回调
    /// 时调（Android 刷新率回调）。
    pub fn set_display_refresh(&mut self, hz: Option<f32>) {
        self.reported_hz.feed(hz.filter(|hz| *hz > 1.0));
    }

    pub fn set_thermal(&mut self, thermal: Thermal) {
        self.governor.set_thermal(thermal);
    }

    /// 播放倍速变了（宿主观察 mpv 的 `speed` 属性）。
    ///
    /// 倍速改变源帧在墙钟上的间隔：旧的间隔样本作废，环里的帧与新帧之间
    /// 也不能插值（`target_time` 的步长突变）。与拖动一样等下一张源帧到达
    /// 再清，期间显示最新一张；重新攒够样本前走直通。
    pub fn set_speed(&mut self, speed: f64) {
        let speed = if speed.is_finite() && speed > 0.0 {
            speed
        } else {
            1.0
        };
        if (speed - self.speed).abs() < 1e-6 {
            return;
        }
        self.speed = speed;
        self.av_delay_force = true;
        self.governor.reset_load();
        self.ring.reset_rate();
        self.src_fps = None;
        self.src_fps_candidate = None;
        self.ring.request_flush();
        self.last_t_out = None;
        self.last_direct_t = None;
    }

    #[must_use]
    pub fn speed(&self) -> f64 {
        self.speed
    }

    pub fn set_gpu_available(&mut self, available: bool) {
        self.gpu_available = available;
    }

    pub fn on_event(&mut self, event: Event) {
        match event {
            Event::Seek => self.ring.request_flush(),
            Event::SourceChanged => {
                // 新片源的 container-fps 由宿主重新给。
                self.fps_hint = None;
                self.av_delay_force = true;
                self.governor.reset_load();
                self.ring.reset_stream();
                self.src_fps = None;
                self.src_fps_candidate = None;
                self.last_direct_t = None;
            }
            Event::Background => {
                self.phase = Phase::Background;
                self.ring.clear();
            }
            Event::PipEnter => {
                self.phase = Phase::Pip;
                self.ring.clear();
            }
            Event::Foreground | Event::PipExit => {
                self.phase = Phase::Active;
                self.ring.request_flush();
                self.last_direct_t = None;
            }
            Event::ContextLost => {
                self.ring.clear();
                self.last_direct_t = None;
            }
        }
        self.last_t_out = None;
    }

    // ---- 每个 vsync ----

    /// 显示周期（微秒）。还没测出来时按 60 Hz。
    #[must_use]
    pub fn period_us(&self) -> i64 {
        match self.display_hz() {
            Some(hz) => (1e6 / f64::from(hz)) as i64,
            None => 16_667,
        }
    }

    #[must_use]
    pub fn display_hz(&self) -> Option<f32> {
        self.reported_hz.value().or_else(|| self.meter.hz())
    }

    /// 片源帧率（墙钟上的，含倍速；已吸附到「常见帧率 × 倍速」）。
    ///
    /// 宿主给了 mpv 的 `container-fps`（[`Controller::set_source_fps_hint`]）
    /// 且它是常见帧率时以它为准：媒体时间上的帧率不受起播追帧、vsync 对齐、
    /// 改倍速后的过渡期影响，开播与改倍速后立刻可用。只有测量已经稳定、又与
    /// 它差出 15% 以上（VFR、容器标错）时才信测量。
    #[must_use]
    pub fn source_fps(&self) -> Option<f32> {
        let hinted = self
            .fps_hint
            .map(|media| policy::snap_source_fps(media * self.speed as f32, self.speed))
            .filter(|fps| policy::is_common_source_fps(*fps, self.speed));
        match (hinted, self.src_fps) {
            (Some(h), Some(m)) if (m / h - 1.0).abs() > 0.15 => Some(m),
            (Some(h), _) => Some(h),
            (None, m) => m,
        }
    }

    /// mpv 的 `container-fps`（媒体时间，与倍速无关）。宿主在**渲染线程以外**
    /// 读（读法同视频尺寸），换片源后重新给；读不到给 `None`。
    pub fn set_source_fps_hint(&mut self, media_fps: Option<f32>) {
        self.fps_hint = media_fps.filter(|f| f.is_finite() && *f > 1.0);
    }

    /// 每收到一张源帧重估一次源帧率；新估计要连续 [`SRC_FPS_STABLE`] 张源帧
    /// 都是它才采纳（第一次也一样）。
    ///
    /// 只看一次估计不够：起播头一秒 mpv 的 `target_time` 很不规整（iPhone 上
    /// 头一秒只出 13 帧），估计依次报 26.7、25、24，档位与 `audio-delay`
    /// 跟着改三次，每改一次声音跳一下。
    fn update_source_fps(&mut self) {
        let Some(us) = self.ring.source_interval_us() else {
            self.src_fps = None;
            self.src_fps_candidate = None;
            return;
        };
        let fps = policy::snap_source_fps((1e6 / us as f64) as f32, self.speed);
        // 吸附不上的（VFR、怪帧率）要攒满一整窗。
        if !policy::is_common_source_fps(fps, self.speed)
            && self.ring.interval_samples() < sched::INTERVAL_SAMPLES
        {
            return;
        }
        let same = |a: f32| (fps / a - 1.0).abs() < 0.01;
        if self.src_fps.is_some_and(same) {
            self.src_fps_candidate = None;
            return;
        }
        let streak = match self.src_fps_candidate {
            Some((c, n)) if same(c) => n + 1,
            _ => 1,
        };
        if streak >= SRC_FPS_STABLE {
            self.src_fps = Some(fps);
            self.src_fps_candidate = None;
        } else {
            self.src_fps_candidate = Some((fps, streak));
        }
    }

    pub fn begin_vsync(&mut self, wall_us: i64) -> Plan {
        self.meter.on_vsync(wall_us);

        let before = self.decision;
        self.decision = self.decide();
        self.governor
            .set_can_step_down(matches!(self.decision, Decision::Interp(_)));
        match self.decision {
            Decision::Interp(_) => {
                self.governor.on_frame(
                    wall_us,
                    self.meter.last_interval_us(),
                    self.period_us(),
                    self.pending_cost_us,
                );
            }
            Decision::Direct(DirectReason::Fallback) => self.governor.tick(wall_us),
            Decision::Direct(_) => {}
        }
        self.pending_cost_us = 0;

        let was_interp = matches!(before, Decision::Interp(_));
        let is_interp = matches!(self.decision, Decision::Interp(_));
        if was_interp != is_interp {
            // 直通期间环里没有新帧，里面的都是过期的；反过来也一样。
            self.ring.clear();
            self.last_t_out = None;
            self.last_direct_t = None;
            self.out_last = None;
            // 输出帧率的统计窗口不能跨过直通期，否则切回来第一秒报出「当前 9 FPS」。
            self.out_window_start = None;
            self.out_distinct = 0;
            self.output_fps = None;
        }
        self.update_av_delay();
        self.vsync_count = self.vsync_count.wrapping_add(1);
        if is_interp {
            Plan::Interp
        } else {
            Plan::Direct
        }
    }

    fn decide(&self) -> Decision {
        if self.phase == Phase::Background {
            return Decision::Direct(DirectReason::Background);
        }
        if self.phase == Phase::Pip {
            return Decision::Direct(DirectReason::Pip);
        }
        if self.setting == Setting::Off {
            return Decision::Direct(DirectReason::Off);
        }
        if !self.gpu_available {
            return Decision::Direct(DirectReason::GpuUnavailable);
        }
        if self.governor.thermal_blocks() {
            return Decision::Direct(DirectReason::Thermal);
        }
        policy::decide(
            self.setting,
            self.source_fps(),
            self.display_hz(),
            self.follow_cap,
            self.governor.steps_down(),
        )
    }

    fn update_av_delay(&mut self) {
        let mut snapped = false;
        let wanted = match self.decision {
            Decision::Interp(_) => {
                let fps = self.source_fps();
                snapped = fps.is_some_and(|f| policy::is_common_source_fps(f, self.speed));
                let interval = fps.map_or(41_708, |fps| (1e6 / f64::from(fps)).round() as i64);
                let spans = if self.skip_dups { 2 } else { 1 };
                interval * spans + DELAY_MARGIN_US
            }
            Decision::Direct(_) => 0,
        };
        let switching = (wanted == 0) != (self.av_delay_us == 0);
        // 吸附上了常见帧率的估计是稳定的，变了就是片源真变了（或倍速刚改、
        // 第一次估计混进了旧倍速的帧），照改；吸附不上的（VFR）才需要滞回。
        let changed = if snapped {
            (wanted - self.av_delay_us).abs() > 1_000
        } else {
            (wanted - self.av_delay_us).abs() as f64 / self.av_delay_us.max(1) as f64
                > DELAY_HYSTERESIS
        };
        // 只管紧接着的这一次重算：之后的变化照常去抖。
        let force = std::mem::take(&mut self.av_delay_force);
        if switching || (force && changed) {
            self.av_delay_us = wanted;
            self.av_delay_candidate = None;
            return;
        }
        if !changed {
            self.av_delay_candidate = None;
            return;
        }
        // 非零改非零：新值要连续 DELAY_STABLE_VSYNCS 个 vsync 都是它才改。
        // 倍速刚改回来时头几次估计混着过渡期的间隔，会在两个常见帧率之间
        // 来回（实测一拍二的 24 fps 在 0.4 秒里 95 / 98 ms 改了四次）。
        let streak = match self.av_delay_candidate {
            Some((v, n)) if (v - wanted).abs() <= 1_000 => n + 1,
            _ => 1,
        };
        if streak >= DELAY_STABLE_VSYNCS {
            self.av_delay_us = wanted;
            self.av_delay_candidate = None;
        } else {
            self.av_delay_candidate = Some((wanted, streak));
        }
    }

    /// 给音频加的延迟有变化时返回新值（微秒，**媒体时间**），宿主把它设成
    /// mpv 的 `audio-delay`（秒）。**异步设**：不能从渲染线程发同步的 mpv
    /// 命令（ADR-004 决策三）。
    ///
    /// 插帧推迟的是墙钟时间；mpv 的 `audio-delay` 加在 pts 上，是媒体时间，
    /// 2 倍速下 0.1 s 的 `audio-delay` 在墙钟上只有 50 ms。所以这里乘倍速。
    pub fn take_av_delay_change(&mut self) -> Option<i64> {
        let media = (self.av_delay_us as f64 * self.speed).round() as i64;
        if self.av_delay_taken == Some(media) {
            return None;
        }
        self.av_delay_taken = Some(media);
        Some(media)
    }

    /// 直通模式下 mpv 交出了一张新帧（不含重画）：只记时间戳，用来估源帧率。
    pub fn observe_direct(&mut self, raw_target_time: i64, now_us: i64) {
        let t = sched::normalize_target_time(raw_target_time, now_us).unwrap_or(now_us);
        self.ring.observe(t, self.last_direct_t);
        self.update_source_fps();
        self.last_direct_t = Some(t);
    }

    /// 下一张源帧写进哪个槽。`redraw`（暂停中改了参数之类的重画）覆盖最新
    /// 那张，不进时间轴。
    #[must_use]
    pub fn slot_for(&self, redraw: bool) -> usize {
        match (redraw, self.ring.newest()) {
            (true, Some(newest)) => newest.slot,
            _ => self.ring.slot_for_next(),
        }
    }

    /// 槽写好之后登记这张源帧。重画返回 `None`。
    pub fn push_source(
        &mut self,
        raw_target_time: i64,
        now_us: i64,
        slot: usize,
        redraw: bool,
    ) -> Option<Push> {
        if redraw && self.ring.newest().is_some() {
            return None;
        }
        let t = sched::normalize_target_time(raw_target_time, now_us).unwrap_or(now_us);
        let pushed = self.ring.push(t, slot).1;
        self.update_source_fps();
        Some(pushed)
    }

    /// GPU 重新分配了环（源帧尺寸变了）。
    pub fn on_ring_reallocated(&mut self) {
        self.ring.clear();
        self.last_t_out = None;
    }

    /// 这一 vsync 显示哪一对、相位多少。
    ///
    /// 输出时刻 = now + 一个显示周期（这一帧真正上屏的时刻）− 前瞻延迟。
    /// 档位要求每 k 个 vsync 才出一张新帧时，其余的 vsync 沿用上一次的输出
    /// 时刻，得到同一对、同一相位：运动场复用，只重跑一次输出 pass（宿主
    /// 每帧都会清自己的 FBO，所以不能什么都不画）。
    pub fn pick(&mut self, now_us: i64) -> Pick {
        let divisor = match self.decision {
            Decision::Interp(Cadence { divisor, .. }) => u64::from(divisor.max(1)),
            Decision::Direct(_) => 1,
        };
        let t_out = match self.last_t_out {
            Some(last) if divisor > 1 && !self.vsync_count.is_multiple_of(divisor) => last,
            _ => now_us + self.period_us() - self.av_delay_us,
        };
        self.last_t_out = Some(t_out);
        let pick = self.ring.pick(t_out, self.skip_dups);
        self.count_output(now_us, &pick);
        pick
    }

    fn count_output(&mut self, now_us: i64, pick: &Pick) {
        let key = match *pick {
            Pick::Empty => None,
            Pick::Pair { a, b, p } => Some((a.seq, b.seq, p.to_bits())),
            Pick::Starved { a } | Pick::Early { a } => Some((a.seq, a.seq, 0)),
        };
        if key.is_some() && key != self.out_last {
            self.out_distinct += 1;
            self.out_last = key;
        }
        let start = *self.out_window_start.get_or_insert(now_us);
        let elapsed = now_us - start;
        if elapsed >= 1_000_000 {
            self.output_fps = Some(self.out_distinct as f32 * 1e6 / elapsed as f32);
            self.out_distinct = 0;
            self.out_window_start = Some(now_us);
        }
    }

    /// GPU 回读到的全帧统计：`changed` 低于阈值就把这一对的后一张标成重复帧。
    pub fn on_pair_changed(&mut self, b_seq: u64, changed: f32) {
        self.pairs_measured += 1;
        if changed < self.dup_thresh {
            self.dup_pairs += 1;
            self.ring.mark_dup(b_seq);
        }
    }

    /// 这一帧插帧段（收帧 + 运动估计 + 输出）的 CPU 耗时，下一帧交给回退
    /// 控制器判「补帧超时」。
    pub fn end_vsync(&mut self, cost_us: i64) {
        self.pending_cost_us = cost_us;
    }

    #[must_use]
    pub fn decision(&self) -> Decision {
        self.decision
    }

    #[must_use]
    pub fn ring_capacity(&self) -> usize {
        self.ring.capacity()
    }

    #[must_use]
    pub fn status(&self) -> Status {
        let display_hz = self.display_hz();
        let source_fps = self.source_fps();
        Status {
            setting: self.setting,
            requested_fps: self
                .setting
                .requested_fps(display_hz.unwrap_or(60.0), self.follow_cap),
            display_hz,
            source_fps,
            output_fps: match self.decision {
                Decision::Interp(_) => self.output_fps,
                Decision::Direct(_) => source_fps,
            },
            decision: self.decision,
            thermal: self.governor.thermal(),
            av_delay_us: self.av_delay_us,
            pairs_measured: self.pairs_measured,
            dup_pairs: self.dup_pairs,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const P60: i64 = 16_667;
    const SRC30: i64 = 33_333;

    /// 60 Hz 屏幕、30 fps 片源，跑 `vsyncs` 个 vsync。片源时间戳 = 收帧时刻
    /// + 一个周期（mpv 提前交一张）。返回每个 vsync 的 Pick。
    struct Sim {
        c: Controller,
        wall: i64,
        now: i64,
        next_src: i64,
        /// 源帧在墙钟上的间隔（倍速时 = 媒体间隔 / 倍速）。
        src_dt: i64,
        /// 每张源帧时间戳上叠加的抖动：第 i 张加 `jitter[i % len]`。
        jitter: Vec<i64>,
        src_count: usize,
    }

    impl Sim {
        fn new(setting: Setting) -> Self {
            let mut c = Controller::new(8, false, 0.0015);
            c.set_setting(setting);
            c.set_gpu_available(true);
            Self {
                c,
                wall: 0,
                now: 1_000_000,
                next_src: 1_000_000,
                src_dt: SRC30,
                jitter: vec![0],
                src_count: 0,
            }
        }

        fn step(&mut self) -> (Plan, Option<Pick>) {
            self.wall += P60;
            self.now += P60;
            let plan = self.c.begin_vsync(self.wall);
            let mut pick = None;
            // mpv 最多提前交一张。
            while self.next_src <= self.now + self.src_dt {
                let t = self.next_src + self.jitter[self.src_count % self.jitter.len()];
                self.src_count += 1;
                match plan {
                    Plan::Direct => self.c.observe_direct(t, self.now),
                    Plan::Interp => {
                        let slot = self.c.slot_for(false);
                        self.c.push_source(t, self.now, slot, false);
                    }
                }
                self.next_src += self.src_dt;
            }
            if plan == Plan::Interp {
                pick = Some(self.c.pick(self.now));
            }
            self.c.end_vsync(1_000);
            (plan, pick)
        }

        fn run(&mut self, n: usize) -> Vec<(Plan, Option<Pick>)> {
            (0..n).map(|_| self.step()).collect()
        }
    }

    #[test]
    fn starts_direct_then_interpolates_once_rates_are_known() {
        let mut sim = Sim::new(Setting::Fps60);
        let first = sim.step();
        assert_eq!(first.0, Plan::Direct);
        assert_eq!(sim.c.decision(), Decision::Direct(DirectReason::Measuring));
        let later = sim.run(60);
        assert_eq!(later.last().map(|s| s.0), Some(Plan::Interp));
        assert_eq!(sim.c.status().display_hz, Some(60.0));
        assert!((sim.c.status().source_fps.unwrap_or(0.0) - 30.0).abs() < 0.1);
    }

    #[test]
    fn steady_state_is_mostly_pairs_at_sixty_distinct_frames() {
        let mut sim = Sim::new(Setting::Fps60);
        let steps = sim.run(300);
        let pairs = steps[120..]
            .iter()
            .filter(|(_, p)| matches!(p, Some(Pick::Pair { .. })))
            .count();
        assert!(pairs > 170, "pairs = {pairs}");
        let out = sim.c.status().output_fps.unwrap_or(0.0);
        assert!((out - 60.0).abs() < 2.0, "output_fps = {out}");
    }

    #[test]
    fn audio_delay_follows_the_mode() {
        let mut sim = Sim::new(Setting::Fps60);
        sim.run(60);
        assert_eq!(sim.c.take_av_delay_change(), Some(SRC30 + DELAY_MARGIN_US));
        assert_eq!(sim.c.take_av_delay_change(), None);
        sim.c.on_event(Event::PipEnter);
        sim.run(1);
        assert_eq!(sim.c.decision(), Decision::Direct(DirectReason::Pip));
        assert_eq!(sim.c.take_av_delay_change(), Some(0));
        sim.c.on_event(Event::PipExit);
        sim.run(1);
        assert_eq!(sim.c.take_av_delay_change(), Some(SRC30 + DELAY_MARGIN_US));
    }

    #[test]
    fn a_jittery_source_rate_snaps_and_keeps_the_delay_still() {
        let mut sim = Sim::new(Setting::Fps60);
        // target_time 带着 vsync 对齐，间隔在 30.6–36.1 ms 之间抖。
        sim.jitter = vec![0, 2_000, -700, 1_500, -1_200, 300];
        sim.run(60);
        assert_eq!(sim.c.source_fps(), Some(30.0));
        let first = sim.c.take_av_delay_change();
        assert_eq!(first, Some(33_333 + DELAY_MARGIN_US));
        sim.run(1_000);
        assert_eq!(
            sim.c.take_av_delay_change(),
            None,
            "播放中途不再改 audio-delay"
        );
    }

    #[test]
    fn audio_delay_is_media_time_under_speed() {
        // 24 fps 片源 1.25 倍速：墙钟上 30 fps。
        let mut sim = Sim::new(Setting::Fps60);
        sim.c.set_speed(1.25);
        sim.run(60);
        assert!(matches!(sim.c.decision(), Decision::Interp(_)));
        assert_eq!(sim.c.source_fps(), Some(30.0));
        // 墙钟上推迟 33.3 + 15 ms；mpv 的 audio-delay 是媒体时间，乘倍速。
        assert_eq!(
            sim.c.take_av_delay_change(),
            Some(((33_333 + DELAY_MARGIN_US) as f64 * 1.25).round() as i64)
        );
    }

    #[test]
    fn changing_speed_remeasures_the_source() {
        let mut sim = Sim::new(Setting::Fps60);
        sim.run(60);
        assert!(matches!(sim.c.decision(), Decision::Interp(_)));
        let _ = sim.c.take_av_delay_change();
        // 30 fps 片源调到 2 倍速：墙钟上 60 fps，60 Hz 屏幕上没得插。
        sim.c.set_speed(2.0);
        sim.src_dt = P60;
        sim.run(1);
        assert_eq!(sim.c.decision(), Decision::Direct(DirectReason::Measuring));
        sim.run(60);
        assert_eq!(
            sim.c.decision(),
            Decision::Direct(DirectReason::SourceFastEnough)
        );
        assert_eq!(sim.c.take_av_delay_change(), Some(0));
        // 调回 1 倍速，重新插帧。
        sim.c.set_speed(1.0);
        sim.src_dt = SRC30;
        sim.run(60);
        assert!(matches!(sim.c.decision(), Decision::Interp(_)));
        assert_eq!(sim.c.take_av_delay_change(), Some(SRC30 + DELAY_MARGIN_US));
    }

    #[test]
    fn a_speed_change_forgets_earlier_step_downs() {
        let mut sim = Sim::new(Setting::Fps60);
        sim.run(60);
        // 每帧都超时，两秒后降档。
        for _ in 0..200 {
            sim.step();
            sim.c.end_vsync(40_000);
        }
        assert_eq!(sim.c.decision(), Decision::Direct(DirectReason::Fallback));
        sim.c.set_speed(1.5);
        sim.c.set_speed(1.0);
        sim.run(60);
        assert!(
            matches!(sim.c.decision(), Decision::Interp(Cadence { fps, .. }) if fps == 60.0),
            "{:?}",
            sim.c.decision()
        );
    }

    #[test]
    fn the_container_fps_hint_wins_over_a_noisy_start() {
        // 起播头一秒 mpv 在追帧：墙钟上稳定地 33 fps，实际是 30 fps 片源。
        let mut sim = Sim::new(Setting::Fps60);
        sim.c.set_source_fps_hint(Some(29.97));
        sim.src_dt = 30_000;
        sim.run(30);
        assert_eq!(sim.c.source_fps(), Some(30.0));
        assert_eq!(sim.c.take_av_delay_change(), Some(SRC30 + DELAY_MARGIN_US));
        sim.src_dt = SRC30;
        sim.run(200);
        assert_eq!(sim.c.take_av_delay_change(), None);
        // 倍速：立刻按 container-fps × 倍速，audio-delay 只改一次。
        sim.c.set_speed(1.25);
        assert!((sim.c.source_fps().unwrap_or(0.0) - 37.5).abs() < 1e-3);
        sim.src_dt = 26_667;
        sim.run(1);
        let wall = 26_667 + DELAY_MARGIN_US;
        assert_eq!(
            sim.c.take_av_delay_change(),
            Some((wall as f64 * 1.25).round() as i64)
        );
        sim.run(200);
        assert_eq!(sim.c.take_av_delay_change(), None);
    }

    #[test]
    fn a_wrong_hint_loses_to_a_stable_measurement() {
        // 容器标 60，实际 30（VFR 片源常见）。
        let mut sim = Sim::new(Setting::Fps60);
        sim.c.set_source_fps_hint(Some(60.0));
        sim.run(60);
        assert_eq!(sim.c.source_fps(), Some(30.0));
    }

    #[test]
    fn skipping_dups_doubles_the_lookahead() {
        let mut c = Controller::new(8, true, 0.0015);
        c.set_setting(Setting::Fps60);
        c.set_gpu_available(true);
        let mut sim = Sim::new(Setting::Fps60);
        sim.c = c;
        sim.run(60);
        assert_eq!(
            sim.c.take_av_delay_change(),
            Some(2 * SRC30 + DELAY_MARGIN_US)
        );
    }

    #[test]
    fn background_and_off_are_direct() {
        let mut sim = Sim::new(Setting::Fps60);
        sim.run(60);
        sim.c.on_event(Event::Background);
        assert_eq!(sim.step().0, Plan::Direct);
        assert_eq!(sim.c.decision(), Decision::Direct(DirectReason::Background));
        sim.c.on_event(Event::Foreground);
        assert_eq!(sim.step().0, Plan::Interp);
        sim.c.set_setting(Setting::Off);
        assert_eq!(sim.step().0, Plan::Direct);
    }

    #[test]
    fn no_gpu_is_direct() {
        let mut sim = Sim::new(Setting::Fps60);
        sim.c.set_gpu_available(false);
        sim.run(60);
        assert_eq!(
            sim.c.decision(),
            Decision::Direct(DirectReason::GpuUnavailable)
        );
    }

    #[test]
    fn severe_thermal_goes_back_to_the_source_rate() {
        let mut sim = Sim::new(Setting::Fps60);
        sim.run(60);
        sim.c.set_thermal(Thermal::Severe);
        assert_eq!(sim.step().0, Plan::Direct);
        assert_eq!(sim.c.decision(), Decision::Direct(DirectReason::Thermal));
    }

    #[test]
    fn a_seek_keeps_showing_the_last_frame_then_restarts_the_pair() {
        let mut sim = Sim::new(Setting::Fps60);
        sim.run(120);
        sim.c.on_event(Event::Seek);
        // 下一张源帧到来之前不能是 Empty（会黑屏）。
        let (_, pick) = sim.step();
        assert!(!matches!(pick, Some(Pick::Empty)), "{pick:?}");
    }

    #[test]
    fn a_120_hz_display_draws_a_new_frame_every_other_vsync() {
        let mut c = Controller::new(8, false, 0.0015);
        c.set_setting(Setting::Fps60);
        c.set_gpu_available(true);
        c.set_display_refresh(Some(120.0));
        let mut wall = 0;
        let mut now = 1_000_000;
        let mut next_src = now;
        let mut picks = Vec::new();
        for _ in 0..600 {
            wall += 8_333;
            now += 8_333;
            let plan = c.begin_vsync(wall);
            while next_src <= now + SRC30 {
                if plan == Plan::Interp {
                    let slot = c.slot_for(false);
                    c.push_source(next_src, now, slot, false);
                } else {
                    c.observe_direct(next_src, now);
                }
                next_src += SRC30;
            }
            if plan == Plan::Interp {
                picks.push(c.pick(now));
            }
        }
        assert_eq!(
            c.decision(),
            Decision::Interp(Cadence {
                fps: 60.0,
                divisor: 2
            })
        );
        // 稳态里相邻两个 vsync 成对相同。
        let tail = &picks[picks.len() - 100..];
        let same = tail.windows(2).filter(|w| w[0] == w[1]).count();
        assert!(same >= 45, "same = {same}");
        let out = c.status().output_fps.unwrap_or(0.0);
        assert!((out - 60.0).abs() < 2.0, "output_fps = {out}");
    }

    #[test]
    fn redraws_do_not_enter_the_timeline() {
        let mut c = Controller::new(4, false, 0.0015);
        let slot = c.slot_for(false);
        assert!(c.push_source(1_000_000, 1_000_000, slot, false).is_some());
        let slot2 = c.slot_for(true);
        assert_eq!(slot2, slot);
        assert!(c.push_source(0, 1_010_000, slot2, true).is_none());
    }

    #[test]
    fn status_reports_target_and_current() {
        let mut sim = Sim::new(Setting::Fps60);
        sim.run(10);
        // 开播头几帧还在测：直通，当前 = 源帧率（可能尚未测出）。
        let s = sim.c.status();
        assert_eq!(s.requested_fps, Some(60.0));
        sim.run(200);
        let s = sim.c.status();
        assert_eq!(s.requested_fps, Some(60.0));
        assert!(matches!(s.decision, Decision::Interp(_)));
    }
}
