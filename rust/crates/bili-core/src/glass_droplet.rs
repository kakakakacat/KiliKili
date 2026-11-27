//! 底栏选中指示「液态水滴」的形变规则（issue #266）。
//!
//! ## 为什么在 Rust
//!
//! 弹簧参数、前后缘的分工、拉伸上限、体积守恒的压扁比例——这些都是「协议
//! 没规定、但两个宿主必须一致」的决定（AGENTS.md 第 2 节）。Flutter 底栏
//! 现在用它，RFC-0001 Stage 2 的 Slint 底栏（#224）以后直接调同一套，不照
//! 着文档再抄一遍。宿主只负责三件平台相关的事：驱动帧回调、把输出的几何
//! 换算成像素、画出来（shader 或回退画法）。
//!
//! ## 模型
//!
//! 坐标单位是**格**：第 `i` 个 tab 占 `[i, i + 1)`。和像素无关，宿主乘以
//! 每格宽度即可，屏幕密度与 tab 数量都不影响形变观感。
//!
//! 水滴的左右两条边各由一根阻尼弹簧拉向目标格的两条边。移动方向上**前
//! 面那条边**用更硬、阻尼更小的弹簧（先走、冲过头再回弹），后面那条用更
//! 软、阻尼更大的弹簧（被拖在后面）。于是：
//!
//! - 起步时两条边拉开 → 水滴沿运动方向变长；
//! - 前缘到位后冲过、回弹，后缘随后赶到 → 宽度有一两次衰减的「呼吸」；
//! - 跨的格越多，两条边拉开得越远 → 拉伸越明显，不必另写按距离分档的规则。
//!
//! 第三根弹簧管整体缩放（按压鼓起，#266 验收第 2 条）。
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
//! tab，水滴不会跳变，最终停在最后一个目标上（#266 验收第 5 条）。

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

/// 前缘弹簧：硬、阻尼小，到位时冲过约 6% 的位移再回弹。
///
/// 起始值，待与 iOS 26 tab bar 的慢放录像对照后再调（#266 验收第 1 条）。
pub const LEADING_SPRING: SpringParams = SpringParams {
    response: 0.32,
    damping_ratio: 0.66,
};

// 前缘必须比后缘硬，否则拉伸方向会反过来（后面的先到）。编译期就挡住。
const _: () = assert!(LEADING_SPRING.response < TRAILING_SPRING.response);

/// 后缘弹簧：软、接近临界阻尼，被拖在前缘后面，基本不冲过。
pub const TRAILING_SPRING: SpringParams = SpringParams {
    response: 0.46,
    damping_ratio: 0.86,
};

/// 缩放弹簧：按下鼓起、松手回落，带一次明显的回弹。
pub const SCALE_SPRING: SpringParams = SpringParams {
    response: 0.28,
    damping_ratio: 0.55,
};

/// 按住时的目标缩放。
pub const PRESSED_SCALE: f64 = 1.12;

/// 宽度最多比静止时多出几格。
///
/// 两条边各自跟弹簧走的话，跨四格时能拉开两格多，看上去是一条长带子而不
/// 是一滴水。超出部分按 `tanh` 软饱和：小位移几乎线性（相邻切换的观感不
/// 受影响），大位移渐近这个上限。
pub const MAX_EXTRA_WIDTH: f64 = 0.6;

/// 高度的取值范围（相对静止高度）。
///
/// 高度按「面积守恒」从宽度推出来：变长就变扁，回弹时比静止还窄就略鼓。
/// 下限防止跨多格时扁成一条线，上限防止鼓出底栏的内边距。
pub const MIN_HEIGHT_SCALE: f64 = 0.82;
pub const MAX_HEIGHT_SCALE: f64 = 1.06;

/// 判定静止的阈值（格、格/秒）。宿主据此停掉帧回调。
///
/// 0.001 格在 86 逻辑像素宽的格子上约是 0.09 逻辑像素，3 倍屏上也不到
/// 一个物理像素，停在这里肉眼看不出与精确落点的差别。
const SETTLE_POSITION: f64 = 0.001;
const SETTLE_VELOCITY: f64 = 0.01;

/// 水滴的运动状态。宿主原样保存、每帧传回 [`step`]，自己不改字段。
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
    /// 整体缩放，静止为 1。
    pub scale: f64,
    /// 缩放的变化速度，1/秒。
    pub scale_velocity: f64,
    /// 当前目标格的左边缘。拖动时可以是小数。
    pub target: f64,
    /// 最近一次换目标时的运动方向：+1 向右，-1 向左，0 原地。决定哪条边
    /// 是前缘。
    pub direction: i8,
}

/// 一帧要画的几何，坐标同样是格。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DropletShape {
    /// 左边缘，格。已经算上了拉伸上限与缩放。
    pub left: f64,
    /// 右边缘，格。
    pub right: f64,
    /// 高度相对静止高度的比例。已经算上了缩放。
    pub height_scale: f64,
    /// 拉伸程度，0 是静止形状，1 是到了 [`MAX_EXTRA_WIDTH`]。shader 用它
    /// 决定中段收细多少；回退画法可以忽略。
    pub stretch: f64,
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
        scale: 1.0,
        scale_velocity: 0.0,
        target: index,
        direction: 0,
    }
}

/// 把水滴推进 `dt` 秒。
///
/// `target` 是目标格的左边缘（点选时是整数，拖动跟手时可以是小数）；
/// `pressed` 为 true 时整体鼓起到 [`PRESSED_SCALE`]。
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
        let center = (s.left + s.right) * 0.5;
        let delta = target + 0.5 - center;
        s.direction = if delta > 0.0 {
            1
        } else if delta < 0.0 {
            -1
        } else {
            0
        };
        s.target = target;
    }

    let scale_target = if pressed { PRESSED_SCALE } else { 1.0 };
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
        (s.scale, s.scale_velocity) =
            spring(s.scale, s.scale_velocity, scale_target, SCALE_SPRING, dt);
    }

    let mut out = frame(s);
    out.settled = is_settled(&s, scale_target);
    if out.settled {
        // 吸附到精确落点，免得停在 0.001 格的残差上：宿主下一次换目标时
        // 方向判断与「是否在原地」都以精确值为准。
        out.state = DropletState {
            target: s.target,
            scale: scale_target,
            ..rest(s.target)
        };
        out.shape = shape(&out.state);
    }
    out
}

fn frame(s: DropletState) -> DropletFrame {
    DropletFrame {
        state: s,
        shape: shape(&s),
        settled: false,
    }
}

fn is_settled(s: &DropletState, scale_target: f64) -> bool {
    let near =
        |x: f64, v: f64, t: f64| (x - t).abs() < SETTLE_POSITION && v.abs() < SETTLE_VELOCITY;
    near(s.left, s.left_velocity, s.target)
        && near(s.right, s.right_velocity, s.target + 1.0)
        && near(s.scale, s.scale_velocity, scale_target)
}

/// 由两条边与缩放推出要画的形状。
fn shape(s: &DropletState) -> DropletShape {
    let center = (s.left + s.right) * 0.5;
    let raw_width = (s.right - s.left).max(0.0);
    // 只对「比静止宽」的部分做软饱和；比静止窄（回弹时）照实画，那一下的
    // 收缩正是呼吸感的来源。
    let extra = raw_width - 1.0;
    let width = if extra > 0.0 {
        1.0 + MAX_EXTRA_WIDTH * (extra / MAX_EXTRA_WIDTH).tanh()
    } else {
        raw_width
    };
    // 面积守恒：宽 × 高 = 常数。开平方是让高度变化比宽度温和——完全按
    // 反比的话，跨两格时就已经扁到下限，相邻切换和跨格看不出差别。
    let height_scale = (1.0 / width.max(1e-6))
        .sqrt()
        .clamp(MIN_HEIGHT_SCALE, MAX_HEIGHT_SCALE);
    let stretch = ((width - 1.0) / MAX_EXTRA_WIDTH).clamp(0.0, 1.0);
    let half = width * 0.5 * s.scale;
    DropletShape {
        left: center - half,
        right: center + half,
        height_scale: height_scale * s.scale,
        stretch,
    }
}

/// 阻尼振子推进 `dt` 秒的解析解，返回新的位置与速度。
///
/// `x'' = -ω²(x - target) - 2ζω x'`，ω = 2π / response。只实现 ζ ≤ 1：
/// 本模块的参数全是欠阻尼或临界阻尼（测试钉住），过阻尼观感是「拖泥带
/// 水」，不是水滴该有的样子。传进 ζ > 1 时按临界阻尼处理。
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

    fn max_width(frames: &[DropletFrame]) -> f64 {
        frames
            .iter()
            .map(|f| f.shape.right - f.shape.left)
            .fold(0.0, f64::max)
    }

    #[test]
    fn rest_is_one_slot_wide_and_settled() {
        let f = step(rest(2.0), 2.0, false, FRAME_60);
        assert!(f.settled);
        assert_eq!(f.shape.left, 2.0);
        assert_eq!(f.shape.right, 3.0);
        assert_eq!(f.shape.height_scale, 1.0);
        assert_eq!(f.shape.stretch, 0.0);
    }

    #[test]
    fn settles_exactly_on_target() {
        let frames = run(0.0, 1.0, FRAME_60, 2.0);
        let last = frames.last().expect("至少跑了一帧");
        assert!(last.settled, "两秒内应当静止");
        assert_eq!(last.state, rest(1.0));
        assert_eq!(last.shape.left, 1.0);
        assert_eq!(last.shape.right, 2.0);
    }

    #[test]
    fn settles_within_a_second() {
        // 太慢的话连点两下就会觉得「黏」。
        let frames = run(0.0, 3.0, FRAME_60, 1.2);
        let first_settled = frames.iter().position(|f| f.settled);
        assert!(
            first_settled.is_some_and(|i| (i as f64) * FRAME_60 < 1.0),
            "跨三格应在 1 秒内静止，实际 {first_settled:?} 帧",
        );
    }

    #[test]
    fn stretches_along_the_motion() {
        let frames = run(0.0, 1.0, FRAME_60, 1.0);
        assert!(max_width(&frames) > 1.05, "相邻切换也应可见地拉长");
        let squashed = frames
            .iter()
            .map(|f| f.shape.height_scale)
            .fold(1.0, f64::min);
        assert!(squashed < 0.98, "拉长时应当变扁");
    }

    #[test]
    fn stretch_grows_with_distance_but_is_capped() {
        let near = max_width(&run(0.0, 1.0, FRAME_60, 1.0));
        let far = max_width(&run(0.0, 4.0, FRAME_60, 1.0));
        assert!(far > near + 0.1, "跨四格应比相邻拉得更长：{near} vs {far}");
        assert!(
            far <= 1.0 + MAX_EXTRA_WIDTH + 1e-9,
            "拉伸不能超过上限：{far}"
        );
    }

    #[test]
    fn leading_edge_overshoots_then_springs_back() {
        let frames = run(0.0, 1.0, FRAME_60, 1.5);
        let peak = frames.iter().map(|f| f.state.right).fold(0.0, f64::max);
        assert!(peak > 2.01, "前缘应冲过目标：峰值 {peak}");
        let width_after_peak: Vec<f64> = frames
            .iter()
            .skip_while(|f| f.state.right < peak)
            .map(|f| f.shape.right - f.shape.left)
            .collect();
        let narrowest = width_after_peak.iter().copied().fold(f64::MAX, f64::min);
        assert!(
            narrowest < 1.0,
            "回弹时应有一下比静止更窄（呼吸）：{narrowest}"
        );
    }

    #[test]
    fn moving_left_mirrors_moving_right() {
        let right = run(1.0, 2.0, FRAME_60, 0.5);
        let left = run(2.0, 1.0, FRAME_60, 0.5);
        for (r, l) in right.iter().zip(&left) {
            // 以 [1, 3) 两格的中线 2.0 镜像：右移的右边缘 ↔ 左移的左边缘。
            assert!((r.shape.right - (4.0 - l.shape.left)).abs() < 1e-9);
            assert!((r.shape.height_scale - l.shape.height_scale).abs() < 1e-12);
        }
    }

    #[test]
    fn frame_rate_does_not_change_the_trajectory() {
        let at_60 = run(0.0, 2.0, 1.0 / 60.0, 0.5);
        let at_120 = run(0.0, 2.0, 1.0 / 120.0, 0.5);
        let a = at_60.last().expect("至少跑了一帧").state;
        let b = at_120.last().expect("至少跑了一帧").state;
        assert!((a.left - b.left).abs() < 1e-9);
        assert!((a.right - b.right).abs() < 1e-9);
        assert!((a.right_velocity - b.right_velocity).abs() < 1e-7);
    }

    #[test]
    fn one_long_frame_equals_many_short_ones() {
        // 掉帧或切后台回来时 dt 很长，解析解不会因此发散。
        let mut s = rest(0.0);
        s = step(s, 3.0, false, 0.0).state;
        let long = step(s, 3.0, false, 0.25).state;
        let short = run(0.0, 3.0, 0.25 / 50.0, 0.25)
            .last()
            .expect("至少跑了一帧")
            .state;
        assert!((long.left - short.left).abs() < 1e-9);
        assert!((long.right - short.right).abs() < 1e-9);
    }

    #[test]
    fn retarget_mid_flight_is_continuous_and_ends_on_last_target() {
        let mut s = rest(0.0);
        let mut last_shape = step(s, 0.0, false, 0.0).shape;
        // 依次点 3、1、2，每次只走几帧。
        for (target, frames) in [(3.0, 6), (1.0, 4), (2.0, 120)] {
            for _ in 0..frames {
                let f = step(s, target, false, FRAME_60);
                assert!(
                    (f.shape.left - last_shape.left).abs() < 0.5,
                    "相邻两帧不应跳变",
                );
                s = f.state;
                last_shape = f.shape;
            }
        }
        let f = step(s, 2.0, false, FRAME_60);
        assert!(f.settled);
        assert_eq!(f.state, rest(2.0));
    }

    #[test]
    fn press_bulges_and_release_returns() {
        let mut s = rest(1.0);
        for _ in 0..60 {
            s = step(s, 1.0, true, FRAME_60).state;
        }
        let pressed = step(s, 1.0, true, FRAME_60);
        assert!(pressed.settled);
        assert!((pressed.shape.height_scale - PRESSED_SCALE).abs() < 1e-9);
        let pressed_width = pressed.shape.right - pressed.shape.left;
        assert!((pressed_width - PRESSED_SCALE).abs() < 1e-9);
        // 鼓起以格中心为中心，不偏到一边。
        assert!(((pressed.shape.left + pressed.shape.right) * 0.5 - 1.5).abs() < 1e-9);

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
        for p in [LEADING_SPRING, TRAILING_SPRING, SCALE_SPRING] {
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
}
