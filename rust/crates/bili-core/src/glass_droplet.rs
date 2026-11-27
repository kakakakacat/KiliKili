//! 底栏选中指示「液态玻璃透镜」的运动规则（issue #266）。
//!
//! ## 为什么在 Rust
//!
//! 弹簧参数、前后缘的分工、拉宽上限、什么时候升起透镜、按下落在哪格、
//! 拖动吸附到哪格——这些都是「协议没规定、但两个宿主必须一致」的决定
//! （AGENTS.md 第 2 节）。Flutter 底栏现在用它，RFC-0001 Stage 2 的 Slint
//! 底栏（#224）以后直接调同一套，不照着文档再抄一遍。宿主只负责三件平台
//! 相关的事：驱动帧回调、把输出的几何换算成像素、画出来。
//!
//! ## 观感参照：iOS 26 tab bar
//!
//! 所有者给的 iOS 录屏（120fps）逐帧看下来，选中指示有**两种形态**：
//!
//! - **静止**：一块哑光灰色胶囊，没有玻璃、没有折射。
//! - **切换或按住**：灰色褪掉，**升起**一块透明透镜——比静止胶囊大一圈，
//!   放大下面的图标与文字，边缘折射很强。移到目标后透镜**落下**，变回灰色
//!   胶囊。相邻切换从起步到落定约 400ms，其中移动只占前 170ms 左右；透镜
//!   最宽约 1.4 格，**不压扁、不收细**。
//!
//! 所以这里输出两组量：位置（两条边）与升起程度 [`DropletShape::lift`]。
//! 宿主按 `lift` 在两种形态之间过渡：灰色胶囊淡出、透镜淡入并放大。
//!
//! ## 模型
//!
//! 坐标单位是**格**：第 `i` 个 tab 占 `[i, i + 1)`。和像素无关，宿主乘以
//! 每格宽度即可。
//!
//! 左右两条边各由一根阻尼弹簧拉向目标格的两条边，移动方向上**前面那条边**
//! 更硬（先走），后面那条更软（被拖在后面），于是移动中透镜沿运动方向变宽，
//! 跨的格越多越宽（有上限）。
//!
//! 第三根弹簧管升起程度：在移动或被按住时拉向 1（快、不回弹），到位后拉
//! 向 0（稍慢，「落下」）。
//!
//! ## 按时间推，不按帧推
//!
//! 每一步用阻尼振子的**解析解**推进 `dt` 秒，而不是数值积分。所以 60Hz、
//! 120Hz、掉帧时的长 `dt` 走出来的轨迹完全相同（测试钉住），与 ADR-016
//! 决策三「动画状态按时间推，不按帧计数」同一个理由。
//!
//! ## 打断
//!
//! 换目标时每根弹簧从**当前位置与当前速度**继续，不重置。快速连点几个
//! tab，透镜不会跳变，最终停在最后一个目标上（#266 验收第 5 条）。

/// 一根弹簧的参数：`response` 是无阻尼时振荡一周的秒数（越小越硬），
/// `damping_ratio` 是阻尼比（< 1 会冲过目标再回弹，= 1 是临界阻尼）。
///
/// 用这两个量而不是刚度/阻尼系数，是因为它们直接对应观感，也与 SwiftUI
/// `spring(response:dampingFraction:)` 同口径，拿 iOS 录屏对照调参时少一
/// 层换算。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpringParams {
    pub response: f64,
    pub damping_ratio: f64,
}

/// 前缘弹簧。iOS 录屏里透镜约 170ms 到位、几乎不冲过，所以比一般的「弹」
/// 硬得多，阻尼也接近临界。
pub const LEADING_SPRING: SpringParams = SpringParams {
    response: 0.24,
    damping_ratio: 0.8,
};

/// 后缘弹簧：比前缘软一些，被拖在后面，形成移动中的变宽。
pub const TRAILING_SPRING: SpringParams = SpringParams {
    response: 0.32,
    damping_ratio: 0.9,
};

// 前缘必须比后缘硬，否则变宽的方向会反过来（后面的先到）。编译期就挡住。
const _: () = assert!(LEADING_SPRING.response < TRAILING_SPRING.response);

/// 透镜升起：录屏里一起步就已经是透镜，所以很快、不回弹。
pub const LIFT_UP_SPRING: SpringParams = SpringParams {
    response: 0.12,
    damping_ratio: 1.0,
};

/// 透镜落下：到位后约 200ms 变回灰色胶囊。
pub const LIFT_DOWN_SPRING: SpringParams = SpringParams {
    response: 0.26,
    damping_ratio: 1.0,
};

/// 透镜中心离目标格中心还有多远（格）就算「还在移动」，透镜保持升起。
///
/// 太小的话透镜要等完全停稳才落，拖拖拉拉；太大则还没到位就开始落。
/// 录屏里透镜是在「到位」那一刻开始落的。
pub const MOVING_THRESHOLD: f64 = 0.04;

/// 升起时透镜比静止胶囊宽出多少格、高出多少比例。
///
/// 静止胶囊高 56（底栏 64 减上下各 4 的内边距），录屏里透镜升起后几乎
/// 撑满底栏，还略微超出：取 1.18 倍约 66 逻辑像素。
pub const LIFT_EXTRA_WIDTH: f64 = 0.16;
pub const LIFT_HEIGHT_SCALE: f64 = 1.18;

/// 移动中宽度最多比静止时多出几格（不含升起本身的加宽）。
///
/// 两条边各自跟弹簧走的话，跨四格时会拉开一格多，看上去是一条长带子。
/// 超出部分按 `tanh` 软饱和：小位移几乎线性，大位移渐近这个上限。录屏里
/// 相邻切换时透镜最宽约 1.4 格，扣掉升起的 0.16，移动带来的约 0.25。
pub const MAX_EXTRA_WIDTH: f64 = 0.45;

/// 透镜完全升起时，下面内容的放大倍数。
pub const LENS_MAX_ZOOM: f64 = 1.2;

/// 判定静止的阈值（格、格/秒）。宿主据此停掉帧回调。
///
/// 0.001 格在 86 逻辑像素宽的格子上约是 0.09 逻辑像素，3 倍屏上也不到
/// 一个物理像素，停在这里肉眼看不出与精确落点的差别。
const SETTLE_POSITION: f64 = 0.001;
const SETTLE_VELOCITY: f64 = 0.01;

/// 透镜的运动状态。宿主原样保存、每帧传回 [`step`]，自己不改字段。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DropletState {
    /// 左边缘位置，格。
    pub left: f64,
    /// 左边缘速度，格/秒。
    pub left_velocity: f64,
    /// 右边缘位置，格。
    pub right: f64,
    /// 右边缘速度，格/秒。
    pub right_velocity: f64,
    /// 升起程度，0 是静止的灰色胶囊，1 是完全升起的透镜。
    pub lift: f64,
    /// 升起程度的变化速度，1/秒。
    pub lift_velocity: f64,
    /// 当前目标格的左边缘。拖动时可以是小数。
    pub target: f64,
    /// 最近一次换目标时的运动方向：+1 向右，-1 向左，0 原地。决定哪条边
    /// 是前缘。
    pub direction: i8,
}

/// 一帧要画的几何，坐标同样是格。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DropletShape {
    /// 左边缘，格。已经算上了移动中的变宽与升起的加宽。
    pub left: f64,
    /// 右边缘，格。
    pub right: f64,
    /// 高度相对静止胶囊的比例。只随升起变化。
    pub height_scale: f64,
    /// 移动中的变宽程度，0 是不变宽，1 是到了 [`MAX_EXTRA_WIDTH`]。
    pub stretch: f64,
    /// 升起程度 0..1，已夹取。宿主按它让灰色胶囊淡出、透镜淡入。
    pub lift: f64,
    /// 透镜下方内容的放大倍数，1 表示不放大。
    pub lens_zoom: f64,
}

/// 一次 [`step`] 的结果。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DropletFrame {
    pub state: DropletState,
    pub shape: DropletShape,
    /// 三根弹簧都已到位。为 true 时宿主可以停掉帧回调，直到下次换目标或
    /// 按压。
    pub settled: bool,
}

/// 静止停在第 `index` 格的状态。
#[must_use]
pub fn rest(index: f64) -> DropletState {
    DropletState {
        left: index,
        left_velocity: 0.0,
        right: index + 1.0,
        right_velocity: 0.0,
        lift: 0.0,
        lift_velocity: 0.0,
        target: index,
        direction: 0,
    }
}

/// 把透镜推进 `dt` 秒。
///
/// `target` 是目标格的左边缘（点选时是整数，拖动跟手时可以是小数）；
/// `pressed` 为 true 时透镜保持升起。
///
/// `dt` 不是有限正数（首帧、时钟回拨）时不推进，只按当前状态返回形状；
/// 目标与方向照常更新，下一帧从这里接着走。
#[must_use]
pub fn step(state: DropletState, target: f64, pressed: bool, dt: f64) -> DropletFrame {
    let mut s = state;
    if !target.is_finite() {
        return frame(s);
    }
    if target != s.target {
        let delta = target + 0.5 - center(&s);
        s.direction = if delta > 0.0 {
            1
        } else if delta < 0.0 {
            -1
        } else {
            0
        };
        s.target = target;
    }

    if dt.is_finite() && dt > 0.0 {
        // 向右走时右边缘在前；向左走时左边缘在前；原地（只是按压或初始）
        // 两边都用前缘参数，回位最快。
        let (left_params, right_params) = match s.direction {
            1 => (TRAILING_SPRING, LEADING_SPRING),
            -1 => (LEADING_SPRING, TRAILING_SPRING),
            _ => (LEADING_SPRING, LEADING_SPRING),
        };
        (s.left, s.left_velocity) = spring(s.left, s.left_velocity, s.target, left_params, dt);
        (s.right, s.right_velocity) =
            spring(s.right, s.right_velocity, s.target + 1.0, right_params, dt);
        let lift_target = lift_target(&s, pressed);
        let lift_params = if lift_target > s.lift {
            LIFT_UP_SPRING
        } else {
            LIFT_DOWN_SPRING
        };
        (s.lift, s.lift_velocity) = spring(s.lift, s.lift_velocity, lift_target, lift_params, dt);
    }

    let mut out = frame(s);
    out.settled = is_settled(&s, pressed);
    if out.settled {
        // 吸附到精确落点，免得停在 0.001 格的残差上：宿主下一次换目标时
        // 方向判断与「是否在原地」都以精确值为准。
        out.state = DropletState {
            lift: if pressed { 1.0 } else { 0.0 },
            ..rest(s.target)
        };
        out.shape = shape(&out.state);
    }
    out
}

fn center(s: &DropletState) -> f64 {
    (s.left + s.right) * 0.5
}

/// 按住，或离目标还远，透镜就保持升起。
fn lift_target(s: &DropletState, pressed: bool) -> f64 {
    let moving = (center(s) - (s.target + 0.5)).abs() > MOVING_THRESHOLD;
    if pressed || moving { 1.0 } else { 0.0 }
}

fn frame(s: DropletState) -> DropletFrame {
    DropletFrame {
        state: s,
        shape: shape(&s),
        settled: false,
    }
}

fn is_settled(s: &DropletState, pressed: bool) -> bool {
    let near =
        |x: f64, v: f64, t: f64| (x - t).abs() < SETTLE_POSITION && v.abs() < SETTLE_VELOCITY;
    let lift_goal = if pressed { 1.0 } else { 0.0 };
    near(s.left, s.left_velocity, s.target)
        && near(s.right, s.right_velocity, s.target + 1.0)
        && near(s.lift, s.lift_velocity, lift_goal)
}

/// 由两条边与升起程度推出要画的形状。
fn shape(s: &DropletState) -> DropletShape {
    let c = center(s);
    let raw_width = (s.right - s.left).max(0.0);
    // 只对「比静止宽」的部分做软饱和；比静止窄（后缘追上前缘时）照实画。
    let extra = raw_width - 1.0;
    let moving_width = if extra > 0.0 {
        1.0 + MAX_EXTRA_WIDTH * (extra / MAX_EXTRA_WIDTH).tanh()
    } else {
        raw_width
    };
    let stretch = ((moving_width - 1.0) / MAX_EXTRA_WIDTH).clamp(0.0, 1.0);
    let lift = s.lift.clamp(0.0, 1.0);
    let half = (moving_width + LIFT_EXTRA_WIDTH * lift) * 0.5;
    DropletShape {
        left: c - half,
        right: c + half,
        height_scale: 1.0 + (LIFT_HEIGHT_SCALE - 1.0) * lift,
        stretch,
        lift,
        lens_zoom: 1.0 + (LENS_MAX_ZOOM - 1.0) * lift,
    }
}

/// 手指按下时落在哪一格。`pointer` 是手指位置，单位是格（宿主按
/// `(x - 内边距) / 每格宽度` 换算），`count` 是 tab 数。
///
/// 按下就让透镜跑过去（#266 所有者定：不等松手），所以这里的结果直接当
/// [`step`] 的目标。超出首尾的按压归到首尾那格。
#[must_use]
pub fn press_target(pointer: f64, count: u32) -> f64 {
    if count == 0 || !pointer.is_finite() {
        return 0.0;
    }
    pointer.floor().clamp(0.0, f64::from(count - 1))
}

/// 拖动时透镜跟手的目标：让透镜的**中心**对准手指，而不是左边缘。
///
/// 结果是小数，夹在首尾两格之间——透镜不会被拖出底栏。
#[must_use]
pub fn drag_target(pointer: f64, count: u32) -> f64 {
    if count == 0 || !pointer.is_finite() {
        return 0.0;
    }
    (pointer - 0.5).clamp(0.0, f64::from(count - 1))
}

/// 松手后吸附到哪一格：离透镜当前目标最近的那格。正好在两格中间时归右边
/// （`f64::round` 远离零取整，目标都非负）。
#[must_use]
pub fn snap_index(target: f64, count: u32) -> u32 {
    if count == 0 || !target.is_finite() {
        return 0;
    }
    // 夹取之后在 [0, count - 1] 内，转 u32 不会截断。
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let index = target.round().clamp(0.0, f64::from(count - 1)) as u32;
    index
}

/// 阻尼振子推进 `dt` 秒的解析解，返回新的位置与速度。
///
/// `x'' = -ω²(x - target) - 2ζω x'`，ω = 2π / response。只实现 ζ ≤ 1：
/// 本模块的参数全是欠阻尼或临界阻尼（测试钉住），过阻尼观感是「拖泥带
/// 水」。传进 ζ > 1 时按临界阻尼处理。
fn spring(x: f64, v: f64, target: f64, p: SpringParams, dt: f64) -> (f64, f64) {
    let omega = std::f64::consts::TAU / p.response;
    let zeta = p.damping_ratio.min(1.0);
    let x0 = x - target;
    let decay = (-zeta * omega * dt).exp();
    if (1.0 - zeta) < 1e-6 {
        let b = v + omega * x0;
        let nx = (x0 + b * dt) * decay;
        let nv = (v - omega * b * dt) * decay;
        return (target + nx, nv);
    }
    let wd = omega * (1.0 - zeta * zeta).sqrt();
    let (sin, cos) = (wd * dt).sin_cos();
    let nx = decay * (x0 * cos + (v + zeta * omega * x0) / wd * sin);
    let nv = decay * (v * cos - (zeta * omega * v + omega * omega * x0) / wd * sin);
    (target + nx, nv)
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    const FRAME_60: f64 = 1.0 / 60.0;

    /// 以固定帧长从 `from` 格切到 `to` 格，返回每一帧。
    fn run(from: f64, to: f64, dt: f64, seconds: f64) -> Vec<DropletFrame> {
        let mut s = rest(from);
        let mut out = Vec::new();
        let n = (seconds / dt).round() as usize;
        for _ in 0..n {
            let f = step(s, to, false, dt);
            s = f.state;
            out.push(f);
        }
        out
    }

    fn width(f: &DropletFrame) -> f64 {
        f.shape.right - f.shape.left
    }

    fn max_width(frames: &[DropletFrame]) -> f64 {
        frames.iter().map(width).fold(0.0, f64::max)
    }

    #[test]
    fn rest_is_a_flat_one_slot_pill() {
        let f = step(rest(2.0), 2.0, false, FRAME_60);
        assert!(f.settled);
        assert_eq!(f.shape.left, 2.0);
        assert_eq!(f.shape.right, 3.0);
        assert_eq!(f.shape.height_scale, 1.0);
        assert_eq!(f.shape.lift, 0.0);
        assert_eq!(f.shape.lens_zoom, 1.0);
    }

    #[test]
    fn lifts_while_moving_and_lands_on_arrival() {
        let frames = run(0.0, 1.0, FRAME_60, 1.0);
        // 起步几帧内就升起大半（录屏里一起步就是透镜）。
        assert!(frames[4].shape.lift > 0.6, "lift={}", frames[4].shape.lift);
        let peak = frames.iter().map(|f| f.shape.lift).fold(0.0, f64::max);
        assert!(peak > 0.9);
        let last = frames.last().expect("至少跑了一帧");
        assert!(last.settled);
        assert_eq!(last.shape.lift, 0.0);
    }

    #[test]
    fn adjacent_switch_matches_the_ios_timing() {
        let frames = run(0.0, 1.0, FRAME_60, 1.0);
        // 移动部分约 170ms：到 0.25s 时中心应已在目标附近。
        let at = &frames[(0.25 / FRAME_60) as usize];
        let c = (at.shape.left + at.shape.right) * 0.5;
        assert!((c - 1.5).abs() < 0.05, "0.25s 时中心 {c}");
        // 整体（含落下）在 0.6s 内静止。
        let settled = frames.iter().position(|f| f.settled).expect("应当静止");
        assert!(
            (settled as f64) * FRAME_60 < 0.6,
            "静止于 {}s",
            settled as f64 * FRAME_60
        );
    }

    #[test]
    fn settles_exactly_on_target() {
        let frames = run(0.0, 1.0, FRAME_60, 2.0);
        let last = frames.last().expect("至少跑了一帧");
        assert!(last.settled, "两秒内应当静止");
        assert_eq!(last.state, rest(1.0));
    }

    #[test]
    fn widens_while_moving_but_does_not_squash() {
        let frames = run(0.0, 1.0, FRAME_60, 1.0);
        let w = max_width(&frames);
        // 录屏里相邻切换最宽约 1.4 格。
        assert!(w > 1.25 && w < 1.55, "最宽 {w}");
        for f in &frames {
            assert!(f.shape.height_scale >= 1.0, "不应压扁");
        }
    }

    #[test]
    fn widening_grows_with_distance_but_is_capped() {
        let near = max_width(&run(0.0, 1.0, FRAME_60, 1.0));
        let far = max_width(&run(0.0, 4.0, FRAME_60, 1.0));
        assert!(far > near + 0.05, "跨四格应比相邻更宽：{near} vs {far}");
        assert!(
            far <= 1.0 + MAX_EXTRA_WIDTH + LIFT_EXTRA_WIDTH + 1e-9,
            "不能超过上限：{far}"
        );
    }

    #[test]
    fn moving_left_mirrors_moving_right() {
        let right = run(1.0, 2.0, FRAME_60, 0.5);
        let left = run(2.0, 1.0, FRAME_60, 0.5);
        for (r, l) in right.iter().zip(&left) {
            // 以 [1, 3) 两格的中线 2.0 镜像。
            assert!((r.shape.right - (4.0 - l.shape.left)).abs() < 1e-9);
            assert!((r.shape.lift - l.shape.lift).abs() < 1e-12);
        }
    }

    #[test]
    fn frame_rate_does_not_change_the_trajectory() {
        // 升起/落下的切换点按阈值判断，判断时刻随帧长不同会差一帧之内，
        // 所以这里只比位置：两条边的弹簧与升起无关。
        let at_60 = run(0.0, 2.0, 1.0 / 60.0, 0.5);
        let at_120 = run(0.0, 2.0, 1.0 / 120.0, 0.5);
        let a = at_60.last().expect("至少跑了一帧").state;
        let b = at_120.last().expect("至少跑了一帧").state;
        assert!((a.left - b.left).abs() < 1e-9);
        assert!((a.right - b.right).abs() < 1e-9);
    }

    #[test]
    fn retarget_mid_flight_is_continuous_and_ends_on_last_target() {
        let mut s = rest(0.0);
        let mut last_shape = step(s, 0.0, false, 0.0).shape;
        for (target, frames) in [(3.0, 6), (1.0, 4), (2.0, 120)] {
            for _ in 0..frames {
                let f = step(s, target, false, FRAME_60);
                assert!((f.shape.left - last_shape.left).abs() < 0.5, "不应跳变");
                s = f.state;
                last_shape = f.shape;
            }
        }
        let f = step(s, 2.0, false, FRAME_60);
        assert!(f.settled);
        assert_eq!(f.state, rest(2.0));
    }

    #[test]
    fn press_lifts_in_place_and_release_lands() {
        let mut s = rest(1.0);
        for _ in 0..60 {
            s = step(s, 1.0, true, FRAME_60).state;
        }
        let pressed = step(s, 1.0, true, FRAME_60);
        assert!(pressed.settled);
        assert!((pressed.shape.lift - 1.0).abs() < 1e-9);
        assert!((pressed.shape.lens_zoom - LENS_MAX_ZOOM).abs() < 1e-9);
        assert!((pressed.shape.height_scale - LIFT_HEIGHT_SCALE).abs() < 1e-9);
        // 升起以格中心为中心，不偏到一边。
        let c = (pressed.shape.left + pressed.shape.right) * 0.5;
        assert!((c - 1.5).abs() < 1e-9);

        for _ in 0..60 {
            s = step(s, 1.0, false, FRAME_60).state;
        }
        let released = step(s, 1.0, false, FRAME_60);
        assert!(released.settled);
        assert_eq!(released.state, rest(1.0));
    }

    #[test]
    fn invalid_dt_or_target_does_not_move() {
        let s = step(rest(0.0), 2.0, false, FRAME_60).state;
        for dt in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            let f = step(s, 2.0, false, dt);
            assert_eq!(f.state.left, s.left);
            assert_eq!(f.state.right, s.right);
            assert!(!f.settled);
        }
        let f = step(s, f64::NAN, false, FRAME_60);
        assert_eq!(f.state, s);
    }

    #[test]
    fn params_are_never_overdamped() {
        for p in [
            LEADING_SPRING,
            TRAILING_SPRING,
            LIFT_UP_SPRING,
            LIFT_DOWN_SPRING,
        ] {
            assert!(p.damping_ratio > 0.0 && p.damping_ratio <= 1.0);
            assert!(p.response > 0.0);
        }
    }

    #[test]
    fn critical_damping_branch_matches_the_limit() {
        let crit = SpringParams {
            response: 0.4,
            damping_ratio: 1.0,
        };
        let almost = SpringParams {
            response: 0.4,
            damping_ratio: 1.0 - 1e-5,
        };
        let a = spring(0.0, 0.0, 1.0, crit, 0.1);
        let b = spring(0.0, 0.0, 1.0, almost, 0.1);
        assert!((a.0 - b.0).abs() < 1e-4);
        assert!((a.1 - b.1).abs() < 1e-3);
    }

    #[test]
    fn press_target_picks_the_slot_under_the_finger() {
        assert_eq!(press_target(0.0, 4), 0.0);
        assert_eq!(press_target(2.99, 4), 2.0);
        assert_eq!(press_target(-0.3, 4), 0.0);
        assert_eq!(press_target(4.2, 4), 3.0);
        assert_eq!(press_target(f64::NAN, 4), 0.0);
        assert_eq!(press_target(1.0, 0), 0.0);
    }

    #[test]
    fn drag_centres_the_lens_on_the_finger_and_stays_inside() {
        assert_eq!(drag_target(1.5, 4), 1.0);
        assert!((drag_target(2.25, 4) - 1.75).abs() < 1e-12);
        assert_eq!(drag_target(0.1, 4), 0.0);
        assert_eq!(drag_target(9.0, 4), 3.0);
    }

    #[test]
    fn snap_rounds_to_the_nearest_slot() {
        assert_eq!(snap_index(1.49, 4), 1);
        assert_eq!(snap_index(1.5, 4), 2);
        assert_eq!(snap_index(-2.0, 4), 0);
        assert_eq!(snap_index(7.0, 4), 3);
        assert_eq!(snap_index(f64::NAN, 4), 0);
        assert_eq!(snap_index(1.0, 0), 0);
    }

    #[test]
    fn dragging_follows_a_fractional_target_with_the_lens_up() {
        let mut s = rest(0.0);
        for _ in 0..90 {
            s = step(s, drag_target(2.25, 4), true, FRAME_60).state;
        }
        let f = step(s, drag_target(2.25, 4), true, FRAME_60);
        let c = (f.shape.left + f.shape.right) * 0.5;
        assert!((c - 2.25).abs() < 1e-3, "透镜中心应对准手指：{c}");
        assert!((f.shape.lift - 1.0).abs() < 1e-3, "拖动中透镜保持升起");
    }
}
