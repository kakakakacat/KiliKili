//! 回退：持续掉帧、补帧超时或热降频时降档，稳定后再试着升回去。
//!
//! #219「回退」一条：持续掉帧或热降频时降到当前屏幕能稳定呈现的档位，必要时
//! 恢复原帧率；补帧超时优先保证播放连续。「档位」就是
//! [`crate::policy::ladder`] 那张表，这里只决定往下挪几格。
//!
//! 判据按**一秒一个窗口**统计，而不是逐帧反应：单帧的卡顿（GC、通知栏下拉、
//! 系统动画）很常见，逐帧降档会让档位一直抖。
//!
//! - 一帧算「掉帧」：与上一帧的间隔超过 1.5 个显示周期（错过了一次 vsync）。
//! - 一帧算「补帧超时」：插帧这一段的 CPU 耗时超过一整个显示周期——这一帧
//!   必然错过 vsync。
//!
//!   第一版的阈值是 0.75 个周期，在 macOS（120 Hz）上 3.7 秒就降了档，而那段
//!   显示间隔 p99 只有 13.8 ms、几乎没掉帧：运动估计每对源帧做一次，那一帧
//!   的 CPU 耗时（含驱动排队）高，但 GPU 是异步的，并不一定误了 vsync。
//!   真正的信号是掉帧本身，耗时只在「已经注定赶不上」时才算数。
//! - 一个窗口算「坏」：掉帧 + 超时占比超过 10 %。连续两个坏窗口降一档。
//! - 升档：在当前档稳定 `backoff`（初始 30 秒）之后试着升一档；升上去 10 秒内
//!   又被降下来，说明这台机器扛不住那一档，`backoff` 翻倍（上限 5 分钟），
//!   免得在两档之间每半分钟来回切一次（每次切换都要改 `audio-delay`，
//!   声音会跳一下）。
//! - 温控：`Severe` 及以上直接回到原帧率；`Moderate` 不降档但禁止升档。

/// 平台温控状态，宿主从 Android `AThermal_getCurrentThermalStatus` /
/// iOS `ProcessInfo.thermalState` 换算过来。
///
/// | 这里 | Android | iOS |
/// |---|---|---|
/// | Nominal | NONE | nominal |
/// | Light | LIGHT | fair |
/// | Moderate | MODERATE | — |
/// | Severe | SEVERE | serious |
/// | Critical | CRITICAL / EMERGENCY / SHUTDOWN | critical |
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Thermal {
    #[default]
    Nominal,
    Light,
    Moderate,
    Severe,
    Critical,
}

const WINDOW_US: i64 = 1_000_000;
const BAD_RATIO: f32 = 0.10;
const BAD_WINDOWS_TO_STEP: u32 = 2;
const BACKOFF_BASE_US: i64 = 30_000_000;
const BACKOFF_MAX_US: i64 = 300_000_000;
/// 升档之后这么短时间内又被降，算「升失败」。
const FAILED_STEP_UP_US: i64 = 10_000_000;
/// 一个窗口里至少要有这么多帧才下结论（窗口刚开、或宿主暂停重绘时）。
const MIN_FRAMES_PER_WINDOW: u32 = 10;

#[derive(Debug, Clone)]
pub struct Governor {
    steps_down: u32,
    thermal: Thermal,
    window_start: Option<i64>,
    frames: u32,
    misses: u32,
    bad_streak: u32,
    /// 最近一次换档（升或降）的时刻。
    last_change: Option<i64>,
    last_step_up: Option<i64>,
    backoff_us: i64,
    /// 档位表还能不能再往下降（宿主每帧告诉我们当前表有几格）。
    can_step_down: bool,
}

impl Default for Governor {
    fn default() -> Self {
        Self {
            steps_down: 0,
            thermal: Thermal::Nominal,
            window_start: None,
            frames: 0,
            misses: 0,
            bad_streak: 0,
            last_change: None,
            last_step_up: None,
            backoff_us: BACKOFF_BASE_US,
            can_step_down: true,
        }
    }
}

impl Governor {
    /// 要在档位表里往下挪几格。
    #[must_use]
    pub fn steps_down(&self) -> u32 {
        self.steps_down
    }

    /// 温控要求回到原帧率。
    #[must_use]
    pub fn thermal_blocks(&self) -> bool {
        self.thermal >= Thermal::Severe
    }

    #[must_use]
    pub fn thermal(&self) -> Thermal {
        self.thermal
    }

    pub fn set_thermal(&mut self, thermal: Thermal) {
        self.thermal = thermal;
    }

    /// 当前升档等待时间（微秒），只用于状态展示与测试。
    #[must_use]
    pub fn backoff_us(&self) -> i64 {
        self.backoff_us
    }

    /// 告诉控制器当前档位表里在 `steps_down` 之下是否还有档（没有的话，
    /// 再降就是直通——那也是合法的「降」，所以这只用来避免无谓地增加计数）。
    pub fn set_can_step_down(&mut self, can: bool) {
        self.can_step_down = can;
    }

    /// 插帧时每个 vsync 调一次。
    ///
    /// - `interval_us`：与上一次 vsync 回调的间隔；
    /// - `period_us`：当前显示周期；
    /// - `cost_us`：上一帧插帧这一段的 CPU 耗时。
    pub fn on_frame(&mut self, now_us: i64, interval_us: i64, period_us: i64, cost_us: i64) {
        let start = *self.window_start.get_or_insert(now_us);
        self.frames += 1;
        let missed = interval_us * 2 > period_us * 3;
        let overran = cost_us > period_us;
        if missed || overran {
            self.misses += 1;
        }
        if now_us - start < WINDOW_US {
            return;
        }
        self.close_window(now_us);
    }

    /// 直通时（没有插帧可评估）也要让时间往前走：到点了就试着升档。
    pub fn tick(&mut self, now_us: i64) {
        self.window_start = None;
        self.frames = 0;
        self.misses = 0;
        self.bad_streak = 0;
        self.maybe_step_up(now_us);
    }

    fn close_window(&mut self, now_us: i64) {
        let enough = self.frames >= MIN_FRAMES_PER_WINDOW;
        let ratio = if enough {
            self.misses as f32 / self.frames as f32
        } else {
            0.0
        };
        self.window_start = Some(now_us);
        self.frames = 0;
        self.misses = 0;
        if !enough {
            return;
        }
        if ratio > BAD_RATIO {
            self.bad_streak += 1;
            if self.bad_streak >= BAD_WINDOWS_TO_STEP {
                self.step_down(now_us);
            }
        } else {
            self.bad_streak = 0;
            self.maybe_step_up(now_us);
        }
    }

    fn step_down(&mut self, now_us: i64) {
        self.bad_streak = 0;
        if !self.can_step_down && self.steps_down > 0 {
            // 已经在最低一档之下（直通），不用再加。
            return;
        }
        if self
            .last_step_up
            .is_some_and(|t| now_us - t < FAILED_STEP_UP_US)
        {
            self.backoff_us = (self.backoff_us * 2).min(BACKOFF_MAX_US);
        }
        self.steps_down += 1;
        self.last_change = Some(now_us);
        self.last_step_up = None;
        tracing::info!(steps_down = self.steps_down, "插帧降档");
    }

    fn maybe_step_up(&mut self, now_us: i64) {
        if self.steps_down == 0 || self.thermal >= Thermal::Moderate {
            return;
        }
        let since = self.last_change.map_or(i64::MAX, |t| now_us - t);
        if since < self.backoff_us {
            return;
        }
        // 在一档上稳定了两倍等待时间，说明那次降档是偶发的，等待时间复位。
        if since >= self.backoff_us * 2 {
            self.backoff_us = BACKOFF_BASE_US;
        }
        self.steps_down -= 1;
        self.last_change = Some(now_us);
        self.last_step_up = Some(now_us);
        tracing::info!(steps_down = self.steps_down, "插帧升档");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const P: i64 = 16_667;

    /// 以 60 Hz 跑 `secs` 秒，`bad` 为真时每 5 帧掉一次（20 %）。
    fn run(g: &mut Governor, t: &mut i64, secs: i64, bad: bool) {
        let frames = secs * 60;
        for i in 0..frames {
            let interval = if bad && i % 5 == 0 { 2 * P } else { P };
            *t += interval;
            g.on_frame(*t, interval, P, 2_000);
        }
    }

    #[test]
    fn an_expensive_but_on_time_frame_is_not_a_miss() {
        // 120 Hz，每 4 帧一次运动估计，那一帧 CPU 7 ms（< 8.3 ms 周期）。
        let mut g = Governor::default();
        let mut t = 0;
        for i in 0..600 {
            t += 8_333;
            g.on_frame(t, 8_333, 8_333, if i % 4 == 0 { 7_000 } else { 1_500 });
        }
        assert_eq!(g.steps_down(), 0);
    }

    #[test]
    fn a_single_bad_second_does_not_step_down() {
        let mut g = Governor::default();
        let mut t = 0;
        run(&mut g, &mut t, 3, false);
        run(&mut g, &mut t, 1, true);
        run(&mut g, &mut t, 3, false);
        assert_eq!(g.steps_down(), 0);
    }

    #[test]
    fn sustained_drops_step_down() {
        let mut g = Governor::default();
        let mut t = 0;
        run(&mut g, &mut t, 3, true);
        assert_eq!(g.steps_down(), 1);
    }

    #[test]
    fn overrunning_the_budget_counts_as_a_miss() {
        let mut g = Governor::default();
        let mut t = 0;
        for _ in 0..180 {
            t += P;
            g.on_frame(t, P, P, 17_000);
        }
        assert_eq!(g.steps_down(), 1);
    }

    #[test]
    fn it_steps_back_up_after_a_stable_period() {
        let mut g = Governor::default();
        let mut t = 0;
        run(&mut g, &mut t, 3, true);
        assert_eq!(g.steps_down(), 1);
        run(&mut g, &mut t, 27, false);
        assert_eq!(g.steps_down(), 1);
        run(&mut g, &mut t, 5, false);
        assert_eq!(g.steps_down(), 0);
    }

    #[test]
    fn a_failed_step_up_doubles_the_wait() {
        let mut g = Governor::default();
        let mut t = 0;
        run(&mut g, &mut t, 3, true);
        run(&mut g, &mut t, 32, false);
        assert_eq!(g.steps_down(), 0);
        // 升上去马上又扛不住。
        run(&mut g, &mut t, 3, true);
        assert_eq!(g.steps_down(), 1);
        assert_eq!(g.backoff_us(), 2 * BACKOFF_BASE_US);
        run(&mut g, &mut t, 35, false);
        assert_eq!(g.steps_down(), 1, "要等 60 秒");
        run(&mut g, &mut t, 30, false);
        assert_eq!(g.steps_down(), 0);
    }

    #[test]
    fn direct_mode_ticks_toward_a_step_up() {
        let mut g = Governor::default();
        let mut t = 0;
        run(&mut g, &mut t, 3, true);
        g.set_can_step_down(false);
        run(&mut g, &mut t, 3, true);
        assert_eq!(g.steps_down(), 1, "已经是直通，不再累加");
        t += 31_000_000;
        g.tick(t);
        assert_eq!(g.steps_down(), 0);
    }

    #[test]
    fn moderate_thermal_blocks_stepping_up() {
        let mut g = Governor::default();
        let mut t = 0;
        run(&mut g, &mut t, 3, true);
        g.set_thermal(Thermal::Moderate);
        run(&mut g, &mut t, 70, false);
        assert_eq!(g.steps_down(), 1);
        assert!(!g.thermal_blocks());
        g.set_thermal(Thermal::Severe);
        assert!(g.thermal_blocks());
    }

    #[test]
    fn too_few_frames_say_nothing() {
        // 宿主停了重绘（暂停、被遮住），窗口里只有几帧，间隔都很长。
        let mut g = Governor::default();
        let mut t = 0;
        for _ in 0..10 {
            t += 500_000;
            g.on_frame(t, 500_000, P, 0);
        }
        assert_eq!(g.steps_down(), 0);
    }
}
