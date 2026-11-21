//! 弹幕轨道与轨迹计算。**这一层是 Rust 的**，不是 UI 的。
//!
//! 按 AGENTS.md §2 的判据：换掉 Slint 之后，分轨、防重叠、淘汰、上限这些
//! 规则一行都不该重写，所以它们不能待在 .slint 里。UI 只拿到一串
//! 「在 (x, y) 用多大字号画什么颜色的什么字」。
//!
//! ## 时钟
//!
//! 入参 `now` 是**只在播放时推进的秒数**，由调用方维护。它不是第二个权威
//! 时钟：弹幕什么时候**出生**由后端报告的 `position_ms` 决定（ADR-003
//! 「真实位置只由后端报告」），出生之后那 8 秒的平移是纯视图动画，和进度条
//! 没有逐帧对齐的义务——基线（`canvas_danmaku`）也是这么做的。暂停时调用方
//! 停止推进 `now`，于是整屏静止。
//!
//! ## 刻意没做的事
//!
//! - **不处理 mode 7（高级弹幕）与 mode 9（BAS）**：它们的 content 是一段
//!   脚本/JSON，要的是一个小型动画解释器。基线也单独走一条路径。本 spike
//!   要回答的是「几百条普通弹幕能不能保持帧率」，高级弹幕不改变这个答案，
//!   只会污染测量。计数记在 `skipped_advanced`，报告里要写出来。
//! - **文本宽度是估算的**，没有问字体引擎。估算只影响分轨紧凑度，不影响
//!   帧率结论；真要做产品时应改成向渲染后端查 text metrics（见 README 的
//!   「已知偏差」）。

use bili_danmaku::engine::ProcessedElem;

/// 一条正在屏幕上的弹幕。
#[derive(Debug, Clone, PartialEq)]
pub struct ActiveItem {
    pub text: String,
    /// 十进制 RGB，不含 alpha（与 `DanmakuElem::color` 同源）。
    pub color: u32,
    pub font_px: f32,
    pub x: f32,
    pub y: f32,
    pub alpha: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// 右进左出。
    Rolling,
    Top,
    Bottom,
}

#[derive(Debug)]
struct Item {
    text: String,
    color: u32,
    font_px: f32,
    width_px: f32,
    kind: Kind,
    lane: usize,
    born: f64,
}

#[derive(Debug, Clone, Copy)]
pub struct OverlayOptions {
    /// 滚动弹幕从右边缘到完全离开左边缘的时长。
    pub rolling_secs: f64,
    /// 顶部/底部弹幕的停留时长。
    pub static_secs: f64,
    /// 字号缩放（基线的「弹幕字号」设置）。
    pub font_scale: f32,
    /// 弹幕占用的屏幕高度比例（基线的「显示区域」设置）。
    pub area_ratio: f32,
    /// 同屏上限。超出直接丢弃新的——基线也有这道闸门，没有它时
    /// 一条高能弹幕就能把帧率打到地板，测出来的数字也不代表真实体验。
    pub max_active: usize,
    pub opacity: f32,
}

impl Default for OverlayOptions {
    fn default() -> Self {
        Self {
            // 8s / 5s 取自基线 canvas_danmaku 的默认 duration。
            rolling_secs: 8.0,
            static_secs: 5.0,
            font_scale: 1.0,
            area_ratio: 1.0,
            max_active: 200,
            opacity: 1.0,
        }
    }
}

/// 一次 `advance` 之后的统计，HUD 与报告都用它。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct OverlayStats {
    pub active: usize,
    pub spawned: u64,
    /// 因同屏上限被丢弃。
    pub dropped_cap: u64,
    /// 因没有空轨道被丢弃。
    pub dropped_lane: u64,
    pub skipped_advanced: u64,
}

#[derive(Debug)]
pub struct Overlay {
    opts: OverlayOptions,
    width: f32,
    height: f32,
    items: Vec<Item>,
    /// 每条轨道「下一条可以出生」的时刻。
    rolling_lanes: Vec<f64>,
    top_lanes: Vec<f64>,
    bottom_lanes: Vec<f64>,
    /// 统一行高。**不按每条弹幕自己的字号算 y**：fixture 里混着 25px 与
    /// 64px，各算各的会让 5 号轨道的大字跑到 10 号轨道的位置上去，屏幕上
    /// 就是两条字叠在一起（第一次跑 no-video 实验时肉眼可见）。
    lane_px: f32,
    stats: OverlayStats,
    rendered: Vec<ActiveItem>,
}

/// 轨道高度按字号推，留 20% 行距。
fn lane_height(font_px: f32) -> f32 {
    font_px * 1.2
}

/// 估算文本像素宽度。CJK 与全角按一个字宽算，其余按 0.55。
fn estimate_width(text: &str, font_px: f32) -> f32 {
    let units: f32 = text
        .chars()
        .map(|c| if is_wide(c) { 1.0 } else { 0.55 })
        .sum();
    units * font_px
}

const fn is_wide(c: char) -> bool {
    matches!(c as u32,
        0x1100..=0x115F | 0x2E80..=0xA4CF | 0xAC00..=0xD7A3 |
        0xF900..=0xFAFF | 0xFE30..=0xFE6F | 0xFF00..=0xFF60 |
        0xFFE0..=0xFFE6 | 0x1F300..=0x1FAFF)
}

impl Overlay {
    #[must_use]
    pub fn new(opts: OverlayOptions, width: f32, height: f32) -> Self {
        let mut overlay = Self {
            opts,
            width,
            height,
            items: Vec::new(),
            rolling_lanes: Vec::new(),
            top_lanes: Vec::new(),
            bottom_lanes: Vec::new(),
            stats: OverlayStats::default(),
            rendered: Vec::new(),
            lane_px: 1.0,
        };
        overlay.rebuild_lanes();
        overlay
    }

    pub fn resize(&mut self, width: f32, height: f32) {
        if (self.width - width).abs() < f32::EPSILON && (self.height - height).abs() < f32::EPSILON
        {
            return;
        }
        self.width = width;
        self.height = height;
        // 窗口变化后旧轨道号可能越界，直接重建并清屏。缩放时丢一屏弹幕
        // 是可接受的——继续用旧轨道会把弹幕画到屏幕外，那才是 bug。
        self.rebuild_lanes();
        self.items.clear();
    }

    fn rebuild_lanes(&mut self) {
        let usable = (self.height * self.opts.area_ratio).max(0.0);
        let lane = lane_height(25.0 * self.opts.font_scale).max(1.0);
        let count = ((usable / lane) as usize).max(1);
        self.lane_px = lane;
        self.rolling_lanes = vec![f64::NEG_INFINITY; count];
        self.top_lanes = vec![f64::NEG_INFINITY; count];
        self.bottom_lanes = vec![f64::NEG_INFINITY; count];
    }

    /// Seek、换源、关闭：清空在途弹幕并放开所有轨道。
    ///
    /// ADR-003 的验证项 1 要求 seek 后不得出现旧代际的弹幕，这就是执行点。
    pub fn reset(&mut self) {
        self.items.clear();
        for lane in self
            .rolling_lanes
            .iter_mut()
            .chain(&mut self.top_lanes)
            .chain(&mut self.bottom_lanes)
        {
            *lane = f64::NEG_INFINITY;
        }
    }

    #[must_use]
    pub const fn stats(&self) -> OverlayStats {
        self.stats
    }

    /// 轨道条数。给测试用：轨道算错时屏幕上只是「弹幕有点挤」，
    /// 不会报错，得有断言钉住。
    #[allow(dead_code)]
    #[must_use]
    pub fn lane_count(&self) -> usize {
        self.rolling_lanes.len()
    }

    /// 把一个 100ms 桶里的弹幕放上屏。
    pub fn feed(&mut self, now: f64, bucket: &[ProcessedElem]) {
        for elem in bucket {
            let kind = match elem.elem.mode {
                1 | 2 | 3 | 6 => Kind::Rolling,
                4 => Kind::Bottom,
                5 => Kind::Top,
                _ => {
                    self.stats.skipped_advanced += 1;
                    continue;
                }
            };
            if self.items.len() >= self.opts.max_active {
                self.stats.dropped_cap += 1;
                continue;
            }
            self.spawn(now, kind, elem);
        }
    }

    fn spawn(&mut self, now: f64, kind: Kind, elem: &ProcessedElem) {
        let font_px = if elem.elem.fontsize > 0 {
            elem.elem.fontsize as f32 * self.opts.font_scale
        } else {
            25.0 * self.opts.font_scale
        };
        // 合并角标：基线在 count > 1 时显示重复条数。这里直接拼进文本，
        // 省掉一个 UI 元素——spike 要测的是文本绘制量，拼在一起只多几个字。
        let text = if elem.count > 1 {
            format!("{}  ×{}", elem.elem.content, elem.count)
        } else {
            elem.elem.content.clone()
        };
        let width_px = estimate_width(&text, font_px);

        let Some(lane) = self.pick_lane(now, kind, width_px) else {
            self.stats.dropped_lane += 1;
            return;
        };

        self.items.push(Item {
            text,
            color: elem.elem.color,
            font_px,
            width_px,
            kind,
            lane,
            born: now,
        });
        self.stats.spawned += 1;
    }

    /// 选一条此刻不会和前一条叠上的轨道。没有就返回 `None`（丢弃该条）。
    fn pick_lane(&mut self, now: f64, kind: Kind, width_px: f32) -> Option<usize> {
        let lanes = match kind {
            Kind::Rolling => &mut self.rolling_lanes,
            Kind::Top => &mut self.top_lanes,
            Kind::Bottom => &mut self.bottom_lanes,
        };
        let idx = lanes.iter().position(|free_at| now >= *free_at)?;
        lanes[idx] = match kind {
            // 下一条要等到这一条的**尾巴完全进屏**之后，否则会直接叠字。
            // 这仍然挡不住「后面的短弹幕追上前面的长弹幕」，基线也挡不住；
            // 要彻底解决得按速度算安全间隔，不在 spike 范围内。
            Kind::Rolling => {
                let span = self.width + width_px;
                now + self.opts.rolling_secs * f64::from(width_px / span)
            }
            Kind::Top | Kind::Bottom => now + self.opts.static_secs,
        };
        Some(idx)
    }

    /// 推进到 `now`，淘汰过期条目，返回这一帧要画的东西。
    pub fn advance(&mut self, now: f64) -> &[ActiveItem] {
        let rolling = self.opts.rolling_secs;
        let static_secs = self.opts.static_secs;
        let width = self.width;
        let height = self.height;
        self.items.retain(|item| {
            let age = now - item.born;
            match item.kind {
                Kind::Rolling => age <= rolling,
                Kind::Top | Kind::Bottom => age <= static_secs,
            }
        });

        self.rendered.clear();
        let lane_h = self.lane_px;
        for item in &self.items {
            let age = (now - item.born).max(0.0);
            let (x, y) = match item.kind {
                Kind::Rolling => {
                    let span = width + item.width_px;
                    let progress = (age / rolling) as f32;
                    (width - span * progress, item.lane as f32 * lane_h)
                }
                Kind::Top => ((width - item.width_px) / 2.0, item.lane as f32 * lane_h),
                Kind::Bottom => (
                    (width - item.width_px) / 2.0,
                    height - (item.lane + 1) as f32 * lane_h,
                ),
            };
            // 静态弹幕最后 0.5s 淡出；滚动的不淡，基线也不淡。
            let alpha = match item.kind {
                Kind::Rolling => self.opts.opacity,
                Kind::Top | Kind::Bottom => {
                    let left = static_secs - age;
                    if left < 0.5 {
                        self.opts.opacity * (left.max(0.0) / 0.5) as f32
                    } else {
                        self.opts.opacity
                    }
                }
            };
            self.rendered.push(ActiveItem {
                text: item.text.clone(),
                color: item.color,
                font_px: item.font_px,
                x,
                y,
                alpha,
            });
        }
        self.stats.active = self.rendered.len();
        &self.rendered
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bili_domain::danmaku::DanmakuElem;

    fn elem(mode: i32, content: &str) -> ProcessedElem {
        ProcessedElem {
            elem: DanmakuElem {
                mode,
                fontsize: 25,
                color: 0xFF_FFFF,
                content: content.to_owned(),
                ..DanmakuElem::default()
            },
            count: 0,
            is_self: false,
        }
    }

    #[test]
    fn rolling_item_leaves_after_its_duration() {
        let mut overlay = Overlay::new(OverlayOptions::default(), 1280.0, 720.0);
        overlay.feed(0.0, &[elem(1, "测试")]);
        assert_eq!(overlay.advance(0.0).len(), 1);
        assert_eq!(overlay.advance(7.9).len(), 1);
        assert_eq!(overlay.advance(8.1).len(), 0);
    }

    #[test]
    fn rolling_item_enters_from_the_right_edge() {
        let mut overlay = Overlay::new(OverlayOptions::default(), 1280.0, 720.0);
        overlay.feed(0.0, &[elem(1, "测试")]);
        let x0 = overlay.advance(0.0)[0].x;
        let x1 = overlay.advance(1.0)[0].x;
        assert!((x0 - 1280.0).abs() < 0.001, "出生时贴右边缘，实际 {x0}");
        assert!(x1 < x0, "应当向左移动：{x1} < {x0}");
    }

    #[test]
    fn two_items_in_one_bucket_take_different_lanes() {
        let mut overlay = Overlay::new(OverlayOptions::default(), 1280.0, 720.0);
        overlay.feed(0.0, &[elem(1, "甲"), elem(1, "乙")]);
        let items = overlay.advance(0.0);
        assert_eq!(items.len(), 2);
        assert!(
            (items[0].y - items[1].y).abs() > 1.0,
            "两条叠在了同一轨道上：{:?}",
            (items[0].y, items[1].y)
        );
    }

    #[test]
    fn a_lane_is_reused_once_its_tail_is_on_screen() {
        let mut overlay = Overlay::new(OverlayOptions::default(), 1280.0, 720.0);
        overlay.feed(0.0, &[elem(1, "甲")]);
        let first_y = overlay.advance(0.0)[0].y;
        // 尾巴进屏后同一轨道就能再发。
        overlay.feed(4.0, &[elem(1, "乙")]);
        let ys: Vec<f32> = overlay.advance(4.0).iter().map(|i| i.y).collect();
        assert_eq!(ys.len(), 2);
        assert!(ys.contains(&first_y), "第二条没有复用 0 号轨道：{ys:?}");
    }

    #[test]
    fn static_items_are_centered_top_and_bottom() {
        let mut overlay = Overlay::new(OverlayOptions::default(), 1280.0, 720.0);
        overlay.feed(0.0, &[elem(5, "顶"), elem(4, "底")]);
        let items = overlay.advance(0.0).to_vec();
        let top = &items[0];
        let bottom = &items[1];
        assert!(top.y < 100.0, "顶部弹幕应在上方：{}", top.y);
        assert!(bottom.y > 600.0, "底部弹幕应在下方：{}", bottom.y);
        let expected_x = (1280.0 - estimate_width("顶", 25.0)) / 2.0;
        assert!((top.x - expected_x).abs() < 0.001);
    }

    #[test]
    fn advanced_modes_are_skipped_and_counted() {
        let mut overlay = Overlay::new(OverlayOptions::default(), 1280.0, 720.0);
        overlay.feed(0.0, &[elem(7, "[1,1,\"25\"]"), elem(9, "bas")]);
        assert!(overlay.advance(0.0).is_empty());
        assert_eq!(overlay.stats().skipped_advanced, 2);
    }

    #[test]
    fn the_active_cap_drops_the_flood_and_counts_it() {
        let opts = OverlayOptions {
            max_active: 3,
            ..OverlayOptions::default()
        };
        let mut overlay = Overlay::new(opts, 1280.0, 720.0);
        let flood: Vec<ProcessedElem> = (0..10).map(|_| elem(1, "洪水")).collect();
        overlay.feed(0.0, &flood);
        assert_eq!(overlay.advance(0.0).len(), 3);
        assert_eq!(overlay.stats().dropped_cap, 7);
    }

    #[test]
    fn a_full_lane_set_drops_the_item_and_counts_it() {
        let opts = OverlayOptions {
            // 只留一条轨道：第二条同刻弹幕无轨可用。
            area_ratio: 0.04,
            ..OverlayOptions::default()
        };
        let mut overlay = Overlay::new(opts, 1280.0, 720.0);
        assert_eq!(overlay.lane_count(), 1);
        overlay.feed(0.0, &[elem(1, "甲"), elem(1, "乙")]);
        assert_eq!(overlay.advance(0.0).len(), 1);
        assert_eq!(overlay.stats().dropped_lane, 1);
    }

    #[test]
    fn reset_clears_items_and_frees_lanes() {
        let mut overlay = Overlay::new(OverlayOptions::default(), 1280.0, 720.0);
        overlay.feed(0.0, &[elem(1, "seek 前")]);
        overlay.reset();
        assert!(overlay.advance(0.1).is_empty());
        // 轨道也必须放开：否则 seek 后第一屏会因为「轨道还占着」被丢掉。
        overlay.feed(0.1, &[elem(1, "seek 后")]);
        assert_eq!(overlay.advance(0.1).len(), 1);
    }

    #[test]
    fn merge_count_goes_into_the_text() {
        let mut overlay = Overlay::new(OverlayOptions::default(), 1280.0, 720.0);
        let mut merged = elem(1, "哈哈");
        merged.count = 7;
        overlay.feed(0.0, &[merged]);
        assert!(overlay.advance(0.0)[0].text.contains("×7"));
    }

    #[test]
    fn resize_rebuilds_lanes_and_clears_the_screen() {
        let mut overlay = Overlay::new(OverlayOptions::default(), 1280.0, 720.0);
        overlay.feed(0.0, &[elem(1, "甲")]);
        overlay.resize(640.0, 360.0);
        assert!(overlay.advance(0.0).is_empty());
        assert!(overlay.lane_count() < 24);
    }

    #[test]
    fn wide_characters_estimate_a_full_em() {
        // 这条是分轨紧凑度的前提：按 0.55 估中文会让轨道提前放行。
        // 逐字比，不是整串比——四个 ASCII 本来就比两个汉字宽。
        assert!(estimate_width("中中", 25.0) > estimate_width("aa", 25.0));
        assert!((estimate_width("中", 25.0) - 25.0).abs() < f32::EPSILON);
    }
}
