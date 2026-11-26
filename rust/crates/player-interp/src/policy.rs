//! 档位：用户设置 + 片源帧率 + 屏幕实际刷新率 → 这一刻插不插、插到多少。
//!
//! 规则出自 #211「帧率与播放规则」那张表：
//!
//! - **关闭**：直通。
//! - **45 / 60 FPS**：低于目标的片源重采样到目标；片源达到或超过目标时直通。
//! - **跟随屏幕刷新率**：以实际刷新率为目标，实测通过前上限 60
//!   （[`FOLLOW_CAP_UNVERIFIED`]）。
//! - **只做能均匀呈现的帧率。** 输出帧率必须是屏幕刷新率的整数分之一（每 k 个
//!   vsync 出一张新帧），否则 45 fps 放在 60 Hz 上会是「两帧一张、一帧一张」
//!   交替，比不插还抖。所以「目标」是上限，实际取刷新率的约数里不超过它的
//!   最大一个；一个都没有（且都不高于片源）就直通。
//!
//! 这些都是「协议没规定、但五端必须一致」的决定（AGENTS.md §2），所以放在
//! Rust，宿主只负责把屏幕刷新率、温控状态这些平台读数喂进来。

/// 用户设置。默认关闭（#219「设置项」）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Setting {
    #[default]
    Off,
    Fps45,
    Fps60,
    /// 跟随屏幕刷新率。上限见 [`FOLLOW_CAP_UNVERIFIED`]。
    FollowDisplay,
}

/// 「跟随屏幕刷新率」在真机验收通过前的上限（#211、#219：「实测通过前，
/// 『跟随』最高取 60」）。90 / 120 的验收结果出来之后由宿主传更高的值，
/// 不改这里的默认。
pub const FOLLOW_CAP_UNVERIFIED: f32 = 60.0;

/// 片源帧率至少要比输出低这么多（比例）才值得插：23.976 → 24 这种差别
/// 插出来只是平白多一份前瞻延迟。
const MIN_GAIN: f32 = 1.05;

/// 比较帧率时的容差（比例）：59.94 Hz 的屏幕要能接 60 FPS 的档位。
const RATE_TOLERANCE: f32 = 1.02;

/// 一张新帧最多跨几个 vsync。再大就是「15 Hz 上插到 30」这类没意义的情形。
const MAX_DIVISOR: u32 = 4;

impl Setting {
    /// 用户要的目标帧率（上限）。`None` = 关闭。
    #[must_use]
    pub fn requested_fps(self, display_hz: f32, follow_cap: f32) -> Option<f32> {
        match self {
            Self::Off => None,
            Self::Fps45 => Some(45.0),
            Self::Fps60 => Some(60.0),
            Self::FollowDisplay => Some(display_hz.min(follow_cap)),
        }
    }

    /// 向系统请求的帧率（Android `ANativeWindow_setFrameRate`、iOS
    /// `CADisplayLink.preferredFrameRateRange.preferred`）。
    ///
    /// 请求的是**内容帧率**而不是刷新率：系统自己挑一个整数倍的显示模式
    /// （45 → 90 Hz，60 → 60 或 120 Hz）。「跟随」不请求——它的意思就是
    /// 屏幕现在是多少就用多少，主动改模式反而违背设置的字面意思。系统最终
    /// 给了什么以实测为准（[`decide`] 吃的是实际刷新率）。
    #[must_use]
    pub fn preferred_display_rate(self) -> Option<f32> {
        match self {
            Self::Off | Self::FollowDisplay => None,
            Self::Fps45 => Some(45.0),
            Self::Fps60 => Some(60.0),
        }
    }
}

impl Setting {
    /// 宿主**必须**给出一个速率时（iOS 的 `CADisplayLink.preferredFrameRateRange`：
    /// ProMotion 屏幕没有「系统默认刷新率」可跟，速率就是 App 自己要的）该要多少。
    ///
    /// - 45 / 60：同 [`Setting::preferred_display_rate`]；
    /// - 跟随：屏幕最高刷新率与上限取小（实测通过前 120 Hz 屏幕要 60）；
    /// - 关闭：同样按上限要，直通时出帧节奏跟着 mpv 交帧走，速率只是轮询频率。
    #[must_use]
    pub fn display_link_rate(self, screen_max_hz: f32, follow_cap: f32) -> f32 {
        self.preferred_display_rate()
            .unwrap_or_else(|| screen_max_hz.min(follow_cap))
    }
}

/// 输出节奏：每 `divisor` 个 vsync 出一张新帧，帧率 = 刷新率 / divisor。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cadence {
    pub fps: f32,
    pub divisor: u32,
}

/// 为什么没在插帧。宿主据此给出界面文案（文案本身留在宿主，§2）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectReason {
    /// 设置是关闭。
    Off,
    /// 还没攒够源帧间隔样本，或屏幕刷新率还没测出来。开播后零点几秒内。
    Measuring,
    /// 片源已经达到或超过目标。
    SourceFastEnough,
    /// 目标高于片源，但当前刷新率的约数里没有一个既不超过目标、又高于片源
    /// （例：60 Hz 屏幕、45 FPS 档、30 fps 片源）。
    DisplayCannotPresent,
    /// 持续掉帧或补帧超时，回退到了原帧率。
    Fallback,
    /// 温控：热降频。
    Thermal,
    /// 画中画：走原帧率（iOS PiP 走自己的图层，Android 小窗太小不值得补帧）。
    Pip,
    /// 在后台（未进画中画）。
    Background,
    /// GPU 侧初始化失败（缺扩展、着色器编不过）。
    GpuUnavailable,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Decision {
    Direct(DirectReason),
    Interp(Cadence),
}

/// 能均匀呈现、且确实高于片源的输出节奏，从高到低。
///
/// [`crate::governor::Governor`] 的「降一档」就是在这张表里往后挪一格，挪过
/// 最后一格就是直通。
#[must_use]
pub fn ladder(requested_fps: f32, source_fps: f32, display_hz: f32) -> Vec<Cadence> {
    (1..=MAX_DIVISOR)
        .map(|divisor| Cadence {
            fps: display_hz / divisor as f32,
            divisor,
        })
        .filter(|c| c.fps <= requested_fps * RATE_TOLERANCE)
        .filter(|c| c.fps >= source_fps * MIN_GAIN)
        .collect()
}

/// 这一刻的档位。
///
/// `steps_down` 是回退控制器要求降的档数（0 = 不降）。
#[must_use]
pub fn decide(
    setting: Setting,
    source_fps: Option<f32>,
    display_hz: Option<f32>,
    follow_cap: f32,
    steps_down: u32,
) -> Decision {
    if setting == Setting::Off {
        return Decision::Direct(DirectReason::Off);
    }
    let (Some(source), Some(display)) = (source_fps, display_hz) else {
        return Decision::Direct(DirectReason::Measuring);
    };
    let Some(requested) = setting.requested_fps(display, follow_cap) else {
        return Decision::Direct(DirectReason::Off);
    };
    if source * MIN_GAIN > requested {
        return Decision::Direct(DirectReason::SourceFastEnough);
    }
    let options = ladder(requested, source, display);
    if options.is_empty() {
        return Decision::Direct(DirectReason::DisplayCannotPresent);
    }
    match options.get(steps_down as usize) {
        Some(cadence) => Decision::Interp(*cadence),
        None => Decision::Direct(DirectReason::Fallback),
    }
}

/// 常见显示刷新率。测出来的 vsync 间隔落在其中一个的 ±3% 内就取它，免得
/// 59.7 / 60.3 这种抖动让档位来回跳。
const COMMON_RATES: [f32; 12] = [
    24.0, 30.0, 48.0, 50.0, 60.0, 72.0, 90.0, 96.0, 100.0, 120.0, 144.0, 165.0,
];

/// 把测得的刷新率吸附到最近的常见值。
#[must_use]
pub fn snap_refresh_rate(hz: f32) -> f32 {
    COMMON_RATES
        .iter()
        .copied()
        .find(|r| (hz / r - 1.0).abs() < 0.03)
        .unwrap_or(hz)
}

/// 常见片源帧率（媒体时间）。NTSC 的 23.976 / 29.97 / 59.94 并进整数档：
/// 与整数只差 0.1%，十几个带抖动的间隔分辨不出来，硬分的话会在两者之间
/// 跳。并进去也没有代价——这个值只用来定档位与 `audio-delay`，每张输出帧
/// 插在哪里按源帧自己的 `target_time` 算。
const COMMON_SOURCE_FPS: [f32; 8] = [12.0, 15.0, 24.0, 25.0, 30.0, 48.0, 50.0, 60.0];

/// 片源帧率测量值离常见值不超过这个比例就取常见值（取最近的那个；
/// 常见值之间最近的 24 / 25、48 / 50 也差 4%）。1.5% 不够：开播头一秒的
/// 窗口里混着起播时的间隔，实测 24 fps 报 23.5、30 fps 报 29.85。
const SOURCE_SNAP_TOLERANCE: f32 = 0.03;

/// 把测得的源帧率（**墙钟**，即 `target_time` 间隔换算的，已含倍速）吸附到
/// 「常见片源帧率 × 倍速」。
///
/// 为什么要吸附：中位数在 16 个间隔上仍会在 29.4–30.3 之间抖（mpv 的
/// `target_time` 带着 vsync 对齐与音频时钟校正），抖到档位判断的边界上时
/// 档位与 `audio-delay` 都会跟着变，每变一次声音跳一下。吸附不上（VFR、
/// 怪帧率）就原样返回。
#[must_use]
pub fn snap_source_fps(wall_fps: f32, speed: f64) -> f32 {
    let speed = if speed > 0.0 { speed as f32 } else { 1.0 };
    let media = wall_fps / speed;
    COMMON_SOURCE_FPS
        .iter()
        .copied()
        .min_by(|a, b| (media / a - 1.0).abs().total_cmp(&(media / b - 1.0).abs()))
        .filter(|r| (media / r - 1.0).abs() < SOURCE_SNAP_TOLERANCE)
        .map_or(wall_fps, |r| r * speed)
}

/// [`snap_source_fps`] 的结果是不是吸附上了常见帧率。
#[must_use]
pub fn is_common_source_fps(wall_fps: f32, speed: f64) -> bool {
    let speed = if speed > 0.0 { speed as f32 } else { 1.0 };
    COMMON_SOURCE_FPS
        .iter()
        .any(|r| (wall_fps / speed / r - 1.0).abs() < 1e-4)
}

/// 是不是常见刷新率之一（[`snap_refresh_rate`] 吸附上了）。
#[must_use]
pub fn is_common_refresh_rate(hz: f32) -> bool {
    COMMON_RATES.contains(&hz)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn interp(d: Decision) -> Option<(f32, u32)> {
        match d {
            Decision::Interp(c) => Some((c.fps, c.divisor)),
            Decision::Direct(_) => None,
        }
    }

    #[test]
    fn off_is_direct() {
        assert_eq!(
            decide(Setting::Off, Some(30.0), Some(60.0), 60.0, 0),
            Decision::Direct(DirectReason::Off)
        );
    }

    #[test]
    fn thirty_to_sixty_on_sixty_hz() {
        let d = decide(Setting::Fps60, Some(30.0), Some(60.0), 60.0, 0);
        assert_eq!(interp(d), Some((60.0, 1)));
    }

    #[test]
    fn sixty_fps_on_120_hz_draws_every_other_vsync() {
        let d = decide(Setting::Fps60, Some(24.0), Some(120.0), 60.0, 0);
        assert_eq!(interp(d), Some((60.0, 2)));
    }

    #[test]
    fn forty_five_on_ninety_hz() {
        let d = decide(Setting::Fps45, Some(30.0), Some(90.0), 60.0, 0);
        assert_eq!(interp(d), Some((45.0, 2)));
    }

    #[test]
    fn forty_five_on_sixty_hz_cannot_present_a_thirty_fps_source() {
        // 60 的约数里不超过 45 的是 30、20、15，都不高于片源。
        assert_eq!(
            decide(Setting::Fps45, Some(30.0), Some(60.0), 60.0, 0),
            Decision::Direct(DirectReason::DisplayCannotPresent)
        );
    }

    #[test]
    fn forty_five_on_sixty_hz_still_lifts_a_24_fps_source_to_30() {
        let d = decide(Setting::Fps45, Some(23.976), Some(60.0), 60.0, 0);
        assert_eq!(interp(d), Some((30.0, 2)));
    }

    #[test]
    fn a_source_at_or_above_target_is_direct() {
        assert_eq!(
            decide(Setting::Fps60, Some(59.94), Some(60.0), 60.0, 0),
            Decision::Direct(DirectReason::SourceFastEnough)
        );
        assert_eq!(
            decide(Setting::Fps45, Some(50.0), Some(90.0), 60.0, 0),
            Decision::Direct(DirectReason::SourceFastEnough)
        );
    }

    #[test]
    fn follow_is_capped_until_verified() {
        let d = decide(Setting::FollowDisplay, Some(30.0), Some(120.0), 60.0, 0);
        assert_eq!(interp(d), Some((60.0, 2)));
        // 验收通过后宿主放开上限。
        let d = decide(Setting::FollowDisplay, Some(30.0), Some(120.0), 120.0, 0);
        assert_eq!(interp(d), Some((120.0, 1)));
    }

    #[test]
    fn follow_on_90_hz_is_45_while_capped() {
        // 90 的约数里不超过 60 的最大是 45。
        let d = decide(Setting::FollowDisplay, Some(24.0), Some(90.0), 60.0, 0);
        assert_eq!(interp(d), Some((45.0, 2)));
    }

    #[test]
    fn a_59_94_hz_display_accepts_the_60_fps_tier() {
        let d = decide(Setting::Fps60, Some(29.97), Some(59.94), 60.0, 0);
        assert_eq!(interp(d), Some((59.94, 1)));
    }

    #[test]
    fn stepping_down_walks_the_ladder_then_goes_direct() {
        // 24 fps 片源、120 Hz、跟随且上限 120：120 → 60 → 40 → 30 → 直通。
        let fps: Vec<_> = (0..5)
            .map(|steps| {
                interp(decide(
                    Setting::FollowDisplay,
                    Some(24.0),
                    Some(120.0),
                    120.0,
                    steps,
                ))
            })
            .collect();
        assert_eq!(
            fps,
            vec![
                Some((120.0, 1)),
                Some((60.0, 2)),
                Some((40.0, 3)),
                Some((30.0, 4)),
                None
            ]
        );
        assert_eq!(
            decide(Setting::FollowDisplay, Some(24.0), Some(120.0), 120.0, 4),
            Decision::Direct(DirectReason::Fallback)
        );
    }

    #[test]
    fn a_thirty_fps_source_on_sixty_hz_has_one_step() {
        assert_eq!(
            decide(Setting::Fps60, Some(30.0), Some(60.0), 60.0, 1),
            Decision::Direct(DirectReason::Fallback)
        );
    }

    #[test]
    fn unknown_rates_wait() {
        assert_eq!(
            decide(Setting::Fps60, None, Some(60.0), 60.0, 0),
            Decision::Direct(DirectReason::Measuring)
        );
        assert_eq!(
            decide(Setting::Fps60, Some(30.0), None, 60.0, 0),
            Decision::Direct(DirectReason::Measuring)
        );
    }

    #[test]
    fn source_rates_snap_to_common_values_times_speed() {
        assert_eq!(snap_source_fps(29.6, 1.0), 30.0);
        assert_eq!(snap_source_fps(23.98, 1.0), 24.0);
        assert_eq!(snap_source_fps(24.1, 1.0), 24.0);
        assert_eq!(snap_source_fps(36.1, 1.5), 36.0);
        // 吸附不上的原样返回（VFR、怪帧率）。
        assert_eq!(snap_source_fps(27.0, 1.0), 27.0);
        assert!(is_common_source_fps(snap_source_fps(36.1, 1.5), 1.5));
        assert!(!is_common_source_fps(27.0, 1.0));
    }

    #[test]
    fn preferred_rate_is_the_content_rate() {
        assert_eq!(Setting::Fps45.preferred_display_rate(), Some(45.0));
        assert_eq!(Setting::Fps60.preferred_display_rate(), Some(60.0));
        assert_eq!(Setting::FollowDisplay.preferred_display_rate(), None);
        assert_eq!(Setting::Off.preferred_display_rate(), None);
    }

    #[test]
    fn display_link_rate_always_names_a_rate() {
        assert_eq!(Setting::Fps45.display_link_rate(120.0, 60.0), 45.0);
        assert_eq!(Setting::FollowDisplay.display_link_rate(120.0, 60.0), 60.0);
        assert_eq!(
            Setting::FollowDisplay.display_link_rate(120.0, 120.0),
            120.0
        );
        assert_eq!(Setting::Off.display_link_rate(60.0, 60.0), 60.0);
    }

    #[test]
    fn refresh_rates_snap() {
        assert_eq!(snap_refresh_rate(59.7), 60.0);
        assert_eq!(snap_refresh_rate(119.0), 120.0);
        assert_eq!(snap_refresh_rate(89.5), 90.0);
        assert_eq!(snap_refresh_rate(75.0), 75.0);
    }
}
