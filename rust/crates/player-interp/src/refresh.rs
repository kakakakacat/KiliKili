//! 按宿主渲染回调的间隔测屏幕刷新率。平台读数（Android 刷新率回调、iOS
//! display link）优先，这里是没有平台读数时的退路。
//!
//! 第一版是间隔的指数平均再吸附到常见刷新率。macOS 的 ProMotion 屏幕上
//! 实测不行：系统按内容动态切 60 / 120 Hz，直通期间的间隔混着 8.3 ms 和
//! 16.7 ms，平均值落在 64–87 Hz 之间，吸附不上，于是出了 37.67 fps 这种
//! 档位。现在的做法：
//!
//! - 取最近 [`WINDOW`] 个间隔的**中位数**，两种间隔混着时它落在其中一种上，
//!   而不是两者之间；
//! - 只接受能吸附到常见刷新率的结果，吸附不上就保持上一个结论；
//! - 新结论要连续 [`STABLE_VSYNCS`] 帧不变才采纳，免得在 60 / 120 之间每帧
//!   来回跳（每跳一次档位与输出节奏都要变）。
//!
//! 平台读数也要同样去抖（[`Debounced`]）：iPhone 16 Pro Max 上给 CADisplayLink
//! 要 45 Hz，ProMotion 没有这一档，系统在 40 与 48 Hz 之间**逐帧交替**；不去抖
//! 的话 48 Hz 那几帧判「屏幕无法均匀呈现 45」→ 直通，`audio-delay` 每零点几秒
//! 在 0 与 98 ms 之间来回切。

use std::collections::VecDeque;

use crate::policy;

const WINDOW: usize = 32;
const STABLE_VSYNCS: u32 = 30;
/// 开播时第一次给出结论只需要攒够这么多个间隔，不等稳定期。
const FIRST_SAMPLES: usize = 10;

/// 一个读数的去抖：第一个值直接采纳，之后的新值要连续 [`STABLE_VSYNCS`] 次
/// 相同才采纳。
#[derive(Debug, Default, Clone, Copy)]
pub struct Debounced {
    accepted: Option<f32>,
    candidate: Option<f32>,
    streak: u32,
}

impl Debounced {
    pub fn feed(&mut self, value: Option<f32>) {
        let Some(value) = value else {
            // 平台不再报告：撤销，退回测量。
            *self = Self::default();
            return;
        };
        if self.accepted.is_none() || self.accepted == Some(value) {
            self.accepted = Some(value);
            self.candidate = None;
            self.streak = 0;
            return;
        }
        if self.candidate == Some(value) {
            self.streak += 1;
        } else {
            self.candidate = Some(value);
            self.streak = 1;
        }
        if self.streak >= STABLE_VSYNCS {
            self.accepted = Some(value);
            self.candidate = None;
            self.streak = 0;
        }
    }

    #[must_use]
    pub fn value(&self) -> Option<f32> {
        self.accepted
    }
}

#[derive(Debug, Default)]
pub struct RefreshMeter {
    last_wall: Option<i64>,
    intervals: VecDeque<i64>,
    last_interval_us: i64,
    accepted: Option<f32>,
    candidate: Option<f32>,
    candidate_streak: u32,
}

impl RefreshMeter {
    pub fn on_vsync(&mut self, wall_us: i64) {
        if let Some(last) = self.last_wall {
            let dt = wall_us - last;
            self.last_interval_us = dt;
            // 超过 100 ms 的是暂停重绘、切后台之类，不是刷新周期。
            if dt > 0 && dt < 100_000 {
                if self.intervals.len() == WINDOW {
                    self.intervals.pop_front();
                }
                self.intervals.push_back(dt);
                self.evaluate();
            }
        }
        self.last_wall = Some(wall_us);
    }

    fn evaluate(&mut self) {
        if self.intervals.len() < FIRST_SAMPLES {
            return;
        }
        let mut sorted: Vec<i64> = self.intervals.iter().copied().collect();
        sorted.sort_unstable();
        let median = sorted[sorted.len() / 2];
        let hz = 1e6 / median as f32;
        let snapped = policy::snap_refresh_rate(hz);
        if !policy::is_common_refresh_rate(snapped) {
            self.candidate = None;
            self.candidate_streak = 0;
            return;
        }
        if self.accepted.is_none() {
            self.accepted = Some(snapped);
            return;
        }
        if self.accepted == Some(snapped) {
            self.candidate = None;
            self.candidate_streak = 0;
            return;
        }
        if self.candidate == Some(snapped) {
            self.candidate_streak += 1;
        } else {
            self.candidate = Some(snapped);
            self.candidate_streak = 1;
        }
        if self.candidate_streak >= STABLE_VSYNCS {
            self.accepted = Some(snapped);
            self.candidate = None;
            self.candidate_streak = 0;
        }
    }

    #[must_use]
    pub fn hz(&self) -> Option<f32> {
        self.accepted
    }

    /// 与上一次回调的间隔（微秒），给回退控制器判掉帧。
    #[must_use]
    pub fn last_interval_us(&self) -> i64 {
        self.last_interval_us
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed(m: &mut RefreshMeter, wall: &mut i64, intervals: impl IntoIterator<Item = i64>) {
        for dt in intervals {
            *wall += dt;
            m.on_vsync(*wall);
        }
    }

    #[test]
    fn a_steady_60_hz() {
        let mut m = RefreshMeter::default();
        let mut wall = 0;
        feed(&mut m, &mut wall, std::iter::repeat_n(16_667, 12));
        assert_eq!(m.hz(), Some(60.0));
    }

    #[test]
    fn occasional_misses_do_not_halve_the_rate() {
        let mut m = RefreshMeter::default();
        let mut wall = 0;
        feed(
            &mut m,
            &mut wall,
            (0..200).map(|i| if i % 5 == 0 { 33_333 } else { 16_667 }),
        );
        assert_eq!(m.hz(), Some(60.0));
    }

    #[test]
    fn a_vrr_mix_never_yields_an_in_between_rate() {
        // ProMotion：8.3 / 16.7 ms 交替出现。
        let mut m = RefreshMeter::default();
        let mut wall = 0;
        feed(
            &mut m,
            &mut wall,
            (0..400).map(|i| if (i / 7) % 2 == 0 { 8_333 } else { 16_667 }),
        );
        let hz = m.hz();
        assert!(hz == Some(60.0) || hz == Some(120.0), "{hz:?}");
    }

    #[test]
    fn a_real_mode_change_is_adopted_after_it_settles() {
        let mut m = RefreshMeter::default();
        let mut wall = 0;
        feed(&mut m, &mut wall, std::iter::repeat_n(16_667, 60));
        assert_eq!(m.hz(), Some(60.0));
        feed(&mut m, &mut wall, std::iter::repeat_n(8_333, 20));
        assert_eq!(m.hz(), Some(60.0), "还没稳定");
        feed(&mut m, &mut wall, std::iter::repeat_n(8_333, 60));
        assert_eq!(m.hz(), Some(120.0));
    }

    #[test]
    fn a_dithering_platform_report_sticks_to_the_first_value() {
        // iPhone 要 45 Hz：系统在 40 / 48 之间逐帧交替。
        let mut d = Debounced::default();
        for i in 0..200 {
            d.feed(Some(if i % 3 == 2 { 48.0 } else { 40.0 }));
        }
        assert_eq!(d.value(), Some(40.0));
        for _ in 0..STABLE_VSYNCS {
            d.feed(Some(120.0));
        }
        assert_eq!(d.value(), Some(120.0));
        d.feed(None);
        assert_eq!(d.value(), None);
    }

    #[test]
    fn long_pauses_are_ignored() {
        let mut m = RefreshMeter::default();
        let mut wall = 0;
        feed(&mut m, &mut wall, std::iter::repeat_n(500_000, 20));
        assert_eq!(m.hz(), None);
    }
}
