//! 源帧环与「这一次 vsync 该显示哪两张帧、相位多少」的选择逻辑。
//!
//! 与 GL 无关，纯数据，便于单测。时间一律用 mpv 时钟（`mpv_get_time_us`，
//! 微秒），因为源帧的 `target_time` 就在这个时基上（换算见
//! [`normalize_target_time`]）。

use std::collections::VecDeque;

/// 环里的一张源帧。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SrcFrame {
    /// 单调递增的源帧序号（跨 flush 也不回绕）。
    pub seq: u64,
    /// mpv 给的目标显示时刻。
    pub t_us: i64,
    /// GPU 侧的槽位号。
    pub slot: usize,
    /// 与前一张源帧画面完全相同（一拍二 / 一拍三的重复帧）。由 GPU 侧
    /// 异步判定，判定到达之前一律当作不同帧。
    pub dup: bool,
    /// 连续重复到这张为止有几张（不含第一张原画）。
    pub dup_run: u8,
}

/// 本次 vsync 的选择结果。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Pick {
    /// 环是空的。
    Empty,
    /// 输出时刻落在两张源帧之间：`p = 0` 是 `a`，`p → 1` 趋向 `b`。
    Pair { a: SrcFrame, b: SrcFrame, p: f32 },
    /// 输出时刻已经越过最新一张源帧，还没有下一张（**前瞻不足**，或暂停）。
    Starved { a: SrcFrame },
    /// 输出时刻早于最老的一张（刚 flush、或时钟跳变）。
    Early { a: SrcFrame },
}

/// 一次 push 的结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Push {
    /// 正常追加。
    Appended,
    /// 时间戳不连续（seek、切清晰度、循环回到片头），先清空再追加。
    Flushed,
}

/// 把 `MPV_RENDER_PARAM_NEXT_FRAME_INFO` 的 `target_time` 换算到微秒。
///
/// render.h 说它与 `mpv_get_time_us` 同一单位（微秒）；mpv 0.42（Homebrew）与
/// media_kit 给 Android 的 libmpv 实测都是**纳秒**——0.37 把内部时钟改成 ns，
/// 这个字段没跟着换算（#218 §1）。这里按量级自适应，而不是写死：五端的
/// libmpv 来源不同，哪一端先修好都不该让插帧整体错位一千倍。
///
/// 判据：同一时刻的两个读数，`raw / 1000` 离 `now_us` 更近就是纳秒。
/// `raw == 0`（mpv 没给时间，例如暂停中的重画）返回 `None`。
#[must_use]
pub fn normalize_target_time(raw: i64, now_us: i64) -> Option<i64> {
    if raw == 0 {
        return None;
    }
    let as_ns = raw / 1000;
    if (as_ns - now_us).abs() < (raw - now_us).abs() {
        Some(as_ns)
    } else {
        Some(raw)
    }
}

#[derive(Debug)]
pub struct Ring {
    frames: VecDeque<SrcFrame>,
    capacity: usize,
    next_seq: u64,
    /// 相邻源帧的间隔超过这个值就当作不连续（微秒）。
    pub gap_limit_us: i64,
    /// 下一次 push 先清空。见 [`Ring::request_flush`]。
    flush_pending: bool,
    /// 最近若干个相邻源帧间隔（微秒），估源帧率用。**跨 flush 保留**：seek
    /// 不改变片源帧率，清掉的话每次拖动之后都要重新攒样本，这期间档位判断
    /// 只能退回直通。换片源（切集、切清晰度）时由 [`Ring::reset_stream`] 清。
    intervals: VecDeque<i64>,
}

/// 估源帧率用的间隔样本数。
const INTERVAL_SAMPLES: usize = 16;
/// 至少攒到这么多个间隔才给出源帧率。
///
/// 第一版是 4：macOS 实测开播头 0.5 秒里源帧率估计在 29.4 → 30.0 之间抖，
/// 档位与 `audio-delay` 跟着改了五次（每改一次声音跳一下）。8 个间隔在
/// 24 fps 片源上也只要三分之一秒。
const MIN_INTERVAL_SAMPLES: usize = 8;

impl Ring {
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        assert!(capacity >= 3, "至少要三个槽：一对在用，一张在写");
        Self {
            frames: VecDeque::with_capacity(capacity),
            capacity,
            next_seq: 0,
            gap_limit_us: 250_000,
            flush_pending: false,
            intervals: VecDeque::with_capacity(INTERVAL_SAMPLES),
        }
    }

    #[must_use]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.frames.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    #[must_use]
    pub fn newest(&self) -> Option<SrcFrame> {
        self.frames.back().copied()
    }

    pub fn clear(&mut self) {
        self.frames.clear();
        self.flush_pending = false;
    }

    /// 宿主明确知道时间轴断了（拖动、前后台、画中画进出）：**下一张源帧到达
    /// 时**再清空，而不是立刻清。
    ///
    /// 立刻清的话，从现在到下一张源帧之间环是空的，输出 pass 没有东西可画，
    /// 屏幕会黑一两帧；留着最新那张，这段时间显示的就是它（与不插帧时 mpv
    /// 停在最后一张的表现相同）。下一张到了先清空再追加，保证插值不会跨过
    /// 断点——拖动前后的两张拿去做运动估计只会得到一张错帧。
    pub fn request_flush(&mut self) {
        self.flush_pending = true;
    }

    /// 换了片源：连源帧率样本一起清掉。
    pub fn reset_stream(&mut self) {
        self.clear();
        self.intervals.clear();
    }

    /// 最近相邻源帧间隔的中位数（微秒）。样本不够时 `None`。
    ///
    /// 用中位数不用均值：重复帧判定、seek 前后、mpv 偶尔丢的一帧都会留下
    /// 个别离群的间隔。
    #[must_use]
    pub fn source_interval_us(&self) -> Option<i64> {
        if self.intervals.len() < MIN_INTERVAL_SAMPLES {
            return None;
        }
        let mut sorted: Vec<i64> = self.intervals.iter().copied().collect();
        sorted.sort_unstable();
        Some(sorted[sorted.len() / 2])
    }

    /// 只记时间戳、不进环：直通模式下 mpv 直接画进宿主的 FBO，没有槽可写，
    /// 但档位判断仍要知道源帧率（不知道就决定不了要不要开始插帧）。
    pub fn observe(&mut self, t_us: i64, last_t_us: Option<i64>) {
        if let Some(last) = last_t_us {
            self.note_interval(t_us - last);
        }
    }

    fn note_interval(&mut self, dt: i64) {
        if dt <= 0 || dt > self.gap_limit_us {
            return;
        }
        if self.intervals.len() == INTERVAL_SAMPLES {
            self.intervals.pop_front();
        }
        self.intervals.push_back(dt);
    }

    /// 下一张源帧该写进哪个槽。环满时复用最老那张的槽——调用方必须在
    /// [`Ring::push`] 之前写槽，所以这里只算不改。
    ///
    /// 被复用的槽不会是本次 vsync 正在用的那一对：调用方每次 vsync 先收帧、
    /// 后选帧，且 `capacity` 至少比「一对 + 前瞻帧数」多一。
    #[must_use]
    pub fn slot_for_next(&self) -> usize {
        if self.frames.len() < self.capacity {
            // 槽号 0..capacity 里找一个没人用的。
            (0..self.capacity)
                .find(|slot| !self.frames.iter().any(|f| f.slot == *slot))
                .unwrap_or(0)
        } else {
            self.frames.front().map_or(0, |f| f.slot)
        }
    }

    /// 追加一张新源帧（槽位已经写好）。
    pub fn push(&mut self, t_us: i64, slot: usize) -> (SrcFrame, Push) {
        let mut result = Push::Appended;
        if std::mem::take(&mut self.flush_pending) {
            self.frames.clear();
            result = Push::Flushed;
        }
        if let Some(last) = self.frames.back()
            && (t_us <= last.t_us || t_us - last.t_us > self.gap_limit_us)
        {
            // 复用的槽可能正好是被清掉的某一张，没关系：清空后它就是唯一一张。
            self.frames.clear();
            result = Push::Flushed;
        }
        if let Some(last) = self.frames.back() {
            self.note_interval(t_us - last.t_us);
        }
        if self.frames.len() == self.capacity {
            self.frames.pop_front();
        }
        let frame = SrcFrame {
            seq: self.next_seq,
            t_us,
            slot,
            dup: false,
            dup_run: 0,
        };
        self.next_seq += 1;
        self.frames.push_back(frame);
        (frame, result)
    }

    /// 目标时刻晚于 `t` 的帧数：此刻手里真正握着的前瞻帧。
    #[must_use]
    pub fn count_after(&self, t: i64) -> usize {
        self.frames.iter().filter(|f| f.t_us > t).count()
    }

    /// GPU 判定 `seq` 这张与前一张画面相同。
    pub fn mark_dup(&mut self, seq: u64) {
        let Some(idx) = self.frames.iter().position(|f| f.seq == seq) else {
            return;
        };
        let prev_run = idx
            .checked_sub(1)
            .map(|i| self.frames[i])
            .filter(|prev| prev.dup && prev.seq + 1 == seq)
            .map_or(0, |prev| prev.dup_run);
        let frame = &mut self.frames[idx];
        frame.dup = true;
        frame.dup_run = prev_run.saturating_add(1);
    }

    /// 最长按「一拍几」跳过的重复帧数。一拍三 = 连续 2 张重复。再长就是
    /// 静止镜头，不跳：否则静止结束后的第一张新画面会和很久以前的那张配对，
    /// 插出一段慢动作。
    pub const MAX_DUP_RUN: u8 = 2;

    /// 选出输出时刻 `t_out` 要用的两张源帧。
    ///
    /// `skip_dups` 为真时跳过已判定的重复帧：一拍二的 `X X Y Y` 会被当作
    /// `X _ Y _`，X→Y 的插值铺满两个源帧间隔，而不是「动一下、停一下」。
    #[must_use]
    pub fn pick(&self, t_out: i64, skip_dups: bool) -> Pick {
        let usable = |f: &&SrcFrame| !(skip_dups && f.dup && f.dup_run <= Self::MAX_DUP_RUN);
        let mut iter = self.frames.iter().filter(usable);
        let Some(first) = iter.next() else {
            return Pick::Empty;
        };
        if t_out < first.t_us {
            return Pick::Early { a: *first };
        }
        let mut a = *first;
        for b in iter {
            if t_out < b.t_us {
                let span = (b.t_us - a.t_us) as f32;
                let p = ((t_out - a.t_us) as f32 / span).clamp(0.0, 1.0);
                return Pick::Pair { a, b: *b, p };
            }
            a = *b;
        }
        Pick::Starved { a }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ring_with(times: &[i64]) -> Ring {
        let mut ring = Ring::new(6);
        for t in times {
            let slot = ring.slot_for_next();
            ring.push(*t, slot);
        }
        ring
    }

    #[test]
    fn picks_the_pair_around_the_output_time() {
        let ring = ring_with(&[0, 33_333, 66_667]);
        let Pick::Pair { a, b, p } = ring.pick(50_000, false) else {
            panic!("应该落在第二、三帧之间");
        };
        assert_eq!((a.seq, b.seq), (1, 2));
        assert!((p - 0.5).abs() < 1e-3);
    }

    #[test]
    fn exactly_on_a_source_frame_is_phase_zero() {
        let ring = ring_with(&[0, 33_333]);
        assert!(matches!(ring.pick(0, false), Pick::Pair { p, .. } if p == 0.0));
    }

    #[test]
    fn past_the_newest_frame_is_starved() {
        let ring = ring_with(&[0, 33_333]);
        assert!(matches!(ring.pick(40_000, false), Pick::Starved { a } if a.seq == 1));
    }

    #[test]
    fn before_the_oldest_frame_is_early() {
        let ring = ring_with(&[100_000, 133_333]);
        assert!(matches!(ring.pick(0, false), Pick::Early { a } if a.seq == 0));
    }

    #[test]
    fn a_backwards_timestamp_flushes() {
        let mut ring = ring_with(&[0, 33_333, 66_667]);
        let slot = ring.slot_for_next();
        let (_, result) = ring.push(10_000, slot);
        assert_eq!(result, Push::Flushed);
        assert_eq!(ring.len(), 1);
    }

    #[test]
    fn a_large_gap_flushes() {
        let mut ring = ring_with(&[0, 33_333]);
        let slot = ring.slot_for_next();
        assert_eq!(ring.push(1_000_000, slot).1, Push::Flushed);
    }

    #[test]
    fn a_full_ring_reuses_the_oldest_slot() {
        let ring = ring_with(&[0, 1, 2, 3, 4, 5]);
        assert_eq!(ring.len(), 6);
        assert_eq!(ring.slot_for_next(), ring.frames[0].slot);
    }

    #[test]
    fn slots_stay_unique() {
        let mut ring = Ring::new(4);
        for t in 0..20 {
            let slot = ring.slot_for_next();
            ring.push(t * 1000, slot);
            let mut slots: Vec<_> = ring.frames.iter().map(|f| f.slot).collect();
            slots.sort_unstable();
            slots.dedup();
            assert_eq!(slots.len(), ring.len());
        }
    }

    #[test]
    fn skipping_dups_spans_two_intervals() {
        // 24fps 一拍二：X X Y Y
        let mut ring = ring_with(&[0, 41_667, 83_333, 125_000]);
        ring.mark_dup(1);
        ring.mark_dup(3);
        let Pick::Pair { a, b, p } = ring.pick(41_667, true) else {
            panic!("应该是 X→Y");
        };
        assert_eq!((a.seq, b.seq), (0, 2));
        assert!((p - 0.5).abs() < 1e-3);
        // 不跳过时就是 X→X 的那一对。
        assert!(matches!(ring.pick(41_667, false), Pick::Pair { a, .. } if a.seq == 1));
    }

    #[test]
    fn a_requested_flush_keeps_the_last_frame_until_the_next_arrives() {
        let mut ring = ring_with(&[0, 33_333, 66_667]);
        ring.request_flush();
        // 下一张到来之前，仍有一张可显示（不黑屏）。
        assert!(matches!(ring.pick(80_000, false), Pick::Starved { a } if a.seq == 2));
        let slot = ring.slot_for_next();
        // 时间戳连续，也要清：插值不能跨过宿主告知的断点。
        assert_eq!(ring.push(100_000, slot).1, Push::Flushed);
        assert_eq!(ring.len(), 1);
    }

    #[test]
    fn source_interval_survives_a_flush_but_not_a_stream_reset() {
        let times: Vec<i64> = (0..10).map(|i| i * 33_333).collect();
        let mut ring = Ring::new(16);
        for t in &times {
            let slot = ring.slot_for_next();
            ring.push(*t, slot);
        }
        assert_eq!(ring.source_interval_us(), Some(33_333));
        ring.request_flush();
        let slot = ring.slot_for_next();
        ring.push(5_000_000, slot);
        assert_eq!(ring.source_interval_us(), Some(33_333));
        ring.reset_stream();
        assert_eq!(ring.source_interval_us(), None);
    }

    #[test]
    fn source_interval_ignores_outliers() {
        // 24fps，其中一个间隔因为丢帧变成两倍。
        let mut ring = Ring::new(16);
        let mut t = 0;
        for i in 0..10 {
            t += if i == 3 { 83_333 } else { 41_667 };
            let slot = ring.slot_for_next();
            ring.push(t, slot);
        }
        assert_eq!(ring.source_interval_us(), Some(41_667));
    }

    #[test]
    fn observing_feeds_the_interval_without_slots() {
        let mut ring = Ring::new(4);
        let mut last = None;
        for i in 0..10 {
            let t = i * 41_667;
            ring.observe(t, last);
            last = Some(t);
        }
        assert!(ring.is_empty());
        assert_eq!(ring.source_interval_us(), Some(41_667));
    }

    #[test]
    fn target_time_in_nanoseconds_is_detected() {
        let now = 5_000_000_000; // 微秒
        assert_eq!(
            normalize_target_time(now * 1000 + 16_000_000, now),
            Some(now + 16_000)
        );
        assert_eq!(normalize_target_time(now + 16_000, now), Some(now + 16_000));
        assert_eq!(normalize_target_time(0, now), None);
    }

    #[test]
    fn a_long_still_is_not_skipped() {
        // 静止 4 张后才动：第 4 张（run=3）不该再被跳过。
        let mut ring = ring_with(&[0, 41_667, 83_333, 125_000, 166_667]);
        for seq in 1..=3 {
            ring.mark_dup(seq);
        }
        let Pick::Pair { a, b, .. } = ring.pick(150_000, true) else {
            panic!("应该有一对");
        };
        assert_eq!((a.seq, b.seq), (3, 4));
    }
}
