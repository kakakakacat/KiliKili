//! 帧率、帧耗时分位数、CPU 与 RSS 采样。
//!
//! SPIKE-PLAN 的实验记录模板要的就是这几个数。平均帧率会把卡顿平均掉，
//! 所以**主结论看 p95/p99 与掉帧数**，平均值只用来对照。

use std::time::{Duration, Instant};

use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

/// 帧耗时统计。只留最近 `CAPACITY` 帧，长跑时内存不涨。
#[derive(Debug)]
pub struct FrameMeter {
    samples: Vec<f32>,
    cursor: usize,
    filled: usize,
    last_frame: Option<Instant>,
    frames: u64,
    started: Instant,
    /// 超过 33.3ms（两个 60Hz 刷新周期）记一次。
    janks: u64,
}

const CAPACITY: usize = 600;

impl Default for FrameMeter {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameMeter {
    #[must_use]
    pub fn new() -> Self {
        Self {
            samples: vec![0.0; CAPACITY],
            cursor: 0,
            filled: 0,
            last_frame: None,
            frames: 0,
            started: Instant::now(),
            janks: 0,
        }
    }

    /// 丢弃下一个间隔。截图这类**测量本身造成的**停顿不该算进帧耗时分位数，
    /// 否则报告里的 p99 量的是 PNG 压缩速度。
    pub fn skip_next_interval(&mut self) {
        self.last_frame = None;
    }

    /// 每帧呈现时调用一次，记录与上一帧的间隔。
    pub fn tick(&mut self, now: Instant) {
        self.frames += 1;
        if let Some(previous) = self.last_frame.replace(now) {
            let ms = now.duration_since(previous).as_secs_f32() * 1000.0;
            self.samples[self.cursor] = ms;
            self.cursor = (self.cursor + 1) % CAPACITY;
            self.filled = (self.filled + 1).min(CAPACITY);
            if ms > 33.3 {
                self.janks += 1;
            }
        }
    }

    #[must_use]
    pub const fn frames(&self) -> u64 {
        self.frames
    }

    #[must_use]
    pub const fn janks(&self) -> u64 {
        self.janks
    }

    #[must_use]
    pub fn elapsed(&self) -> Duration {
        self.started.elapsed()
    }

    /// 整段运行的平均帧率。
    #[must_use]
    pub fn average_fps(&self) -> f32 {
        let secs = self.started.elapsed().as_secs_f32();
        if secs <= 0.0 {
            0.0
        } else {
            self.frames as f32 / secs
        }
    }

    /// 最近窗口内的帧耗时分位数（毫秒）。没有样本时返回 `None`。
    #[must_use]
    pub fn percentile(&self, q: f32) -> Option<f32> {
        if self.filled == 0 {
            return None;
        }
        let mut window: Vec<f32> = self.samples[..self.filled].to_vec();
        window.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let idx = ((window.len() - 1) as f32 * q).round() as usize;
        window.get(idx).copied()
    }

    /// 最近窗口的瞬时帧率（由 p50 帧耗时推算，比「过去一秒计数」稳）。
    #[must_use]
    pub fn recent_fps(&self) -> Option<f32> {
        self.percentile(0.5)
            .filter(|ms| *ms > 0.0)
            .map(|ms| 1000.0 / ms)
    }
}

/// 本进程的 CPU 与常驻内存。
#[derive(Debug)]
pub struct ProcSampler {
    /// sysinfo 0.40 起 `System::new()` 会失败（容器里拿不到 /proc 之类）。
    /// 采不到就只是报告里少两行，不该让整个实验跑不起来。
    system: Option<System>,
    pid: Pid,
    last: Instant,
    cpu_percent: f32,
    rss_bytes: u64,
    peak_rss_bytes: u64,
}

impl ProcSampler {
    #[must_use]
    pub fn new() -> Self {
        Self {
            system: System::new().ok(),
            pid: Pid::from_u32(std::process::id()),
            // 让第一次 sample 立刻生效。
            last: Instant::now() - Duration::from_secs(10),
            cpu_percent: 0.0,
            rss_bytes: 0,
            peak_rss_bytes: 0,
        }
    }

    /// 最快 500ms 采一次——sysinfo 的 CPU 占用是两次刷新之间的差值，
    /// 采太密会得到一堆 0 或者噪声。
    pub fn sample(&mut self) {
        if self.last.elapsed() < Duration::from_millis(500) {
            return;
        }
        self.last = Instant::now();
        let Some(system) = self.system.as_mut() else {
            return;
        };
        system.refresh_processes_specifics(
            ProcessesToUpdate::Some(&[self.pid]),
            true,
            ProcessRefreshKind::nothing().with_cpu().with_memory(),
        );
        if let Some(process) = system.process(self.pid) {
            self.cpu_percent = process.cpu_usage();
            self.rss_bytes = process.memory();
            self.peak_rss_bytes = self.peak_rss_bytes.max(self.rss_bytes);
        }
    }

    #[must_use]
    pub const fn cpu_percent(&self) -> f32 {
        self.cpu_percent
    }

    #[must_use]
    pub const fn rss_mib(&self) -> f32 {
        self.rss_bytes as f32 / (1024.0 * 1024.0)
    }

    #[must_use]
    pub const fn peak_rss_mib(&self) -> f32 {
        self.peak_rss_bytes as f32 / (1024.0 * 1024.0)
    }
}

impl Default for ProcSampler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentiles_need_at_least_one_interval() {
        let mut meter = FrameMeter::new();
        let start = Instant::now();
        assert_eq!(meter.percentile(0.5), None);
        meter.tick(start);
        // 第一帧只记时刻，没有间隔。
        assert_eq!(meter.percentile(0.5), None);
        meter.tick(start + Duration::from_millis(16));
        assert!(meter.percentile(0.5).is_some());
    }

    #[test]
    fn a_long_frame_counts_as_a_jank() {
        let mut meter = FrameMeter::new();
        let start = Instant::now();
        meter.tick(start);
        meter.tick(start + Duration::from_millis(16));
        assert_eq!(meter.janks(), 0);
        meter.tick(start + Duration::from_millis(116));
        assert_eq!(meter.janks(), 1);
    }

    #[test]
    fn the_ring_buffer_keeps_the_most_recent_window() {
        let mut meter = FrameMeter::new();
        let start = Instant::now();
        let mut at = start;
        // 先灌满一圈慢帧，再灌一圈快帧：窗口里只应剩快帧。
        for _ in 0..=CAPACITY {
            at += Duration::from_millis(50);
            meter.tick(at);
        }
        for _ in 0..CAPACITY {
            at += Duration::from_millis(10);
            meter.tick(at);
        }
        let p95 = meter.percentile(0.95).expect("应当有样本");
        assert!(p95 < 20.0, "旧的慢帧没有被挤出窗口：p95={p95}");
        assert_eq!(meter.frames(), (CAPACITY * 2 + 1) as u64);
    }
}
