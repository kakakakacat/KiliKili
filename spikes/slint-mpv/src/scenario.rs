//! 脚本化实验流程。
//!
//! 为什么不靠人按键：实验记录要可比。手动跑两次，暂停时长、seek 位置、
//! 截图时机都不一样，两份数字摆在一起没法说明问题。脚本跑出来的两份
//! （比如 macOS 与 Windows）才能逐行对照。
//!
//! 人工交互并没有被取消——不带 `--scripted` 就是手动模式，键位见 README。

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// 打开媒体并开始播放。
    Open,
    Play,
    Pause,
    /// 绝对位置（毫秒）。
    Seek(u64),
    Fullscreen(bool),
    /// 逻辑像素。
    Resize(u32, u32),
    /// 下一帧渲染完成后截图。
    Shot(&'static str),
    /// 记一次当前位置，供「暂停是否真的冻结了位置」判定。
    SamplePosition(&'static str),
    Quit,
}

#[derive(Debug, Clone, Copy)]
pub struct Step {
    pub at_ms: u64,
    pub action: Action,
    /// 写进记录的「预期」。
    pub note: &'static str,
}

/// 默认脚本。时间点取得宽松：每一步都要留出后端真正完成的时间，
/// 掐得太紧测的就是「命令发出后 50ms 内有没有生效」，那不是本 spike 的问题。
#[must_use]
pub fn default_script(seek_target_ms: u64) -> Vec<Step> {
    vec![
        Step {
            at_ms: 0,
            action: Action::Open,
            note: "打开 DASH 音视频并播放",
        },
        Step {
            at_ms: 3_000,
            action: Action::Shot("01-playing"),
            note: "播放中：视频在下、弹幕在上、HUD 在最上",
        },
        Step {
            at_ms: 5_000,
            action: Action::Pause,
            note: "暂停",
        },
        Step {
            at_ms: 5_400,
            action: Action::SamplePosition("pause-a"),
            note: "暂停后第一次取位置",
        },
        Step {
            at_ms: 6_800,
            action: Action::SamplePosition("pause-b"),
            note: "1.4 秒后位置应当不变（弹幕也应静止）",
        },
        Step {
            at_ms: 7_000,
            action: Action::Shot("02-paused"),
            note: "暂停画面",
        },
        Step {
            at_ms: 7_500,
            action: Action::Play,
            note: "恢复播放",
        },
        Step {
            at_ms: 10_000,
            action: Action::Seek(seek_target_ms),
            note: "跳转；应收到 SeekCompleted，旧代际事件被丢弃",
        },
        Step {
            at_ms: 12_000,
            action: Action::Shot("03-after-seek"),
            note: "跳转后：不应残留跳转前的弹幕",
        },
        Step {
            at_ms: 13_000,
            action: Action::Fullscreen(true),
            note: "进入全屏",
        },
        Step {
            at_ms: 15_500,
            action: Action::Shot("04-fullscreen"),
            note: "全屏下视频与弹幕都应跟着铺满",
        },
        Step {
            at_ms: 16_500,
            action: Action::Fullscreen(false),
            note: "退出全屏",
        },
        Step {
            at_ms: 18_000,
            action: Action::Resize(960, 540),
            note: "缩小窗口",
        },
        Step {
            at_ms: 20_000,
            action: Action::Shot("05-resized"),
            note: "缩放后视频与弹幕的布局",
        },
        Step {
            at_ms: 21_000,
            action: Action::Resize(1280, 720),
            note: "恢复窗口尺寸",
        },
        Step {
            at_ms: 24_000,
            action: Action::Shot("06-final"),
            note: "收尾画面",
        },
        Step {
            at_ms: 25_000,
            action: Action::Quit,
            note: "写记录并退出",
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_script_is_ordered_and_terminates() {
        let script = default_script(30_000);
        assert!(
            script.windows(2).all(|w| w[0].at_ms <= w[1].at_ms),
            "脚本的时间点必须单调不减，否则执行器会跳过步骤"
        );
        assert_eq!(
            script.last().map(|s| s.action),
            Some(Action::Quit),
            "脚本必须以 Quit 结尾，否则 --scripted 不会自己退出"
        );
    }

    #[test]
    fn the_pause_window_is_long_enough_to_detect_drift() {
        let script = default_script(30_000);
        let sample_times: Vec<u64> = script
            .iter()
            .filter(|s| matches!(s.action, Action::SamplePosition(_)))
            .map(|s| s.at_ms)
            .collect();
        assert_eq!(sample_times.len(), 2);
        // 间隔太短的话，就算位置在偷偷推进也看不出来。
        assert!(sample_times[1] - sample_times[0] >= 1_000);
    }

    #[test]
    fn every_shot_has_a_distinct_name() {
        let script = default_script(30_000);
        let mut names: Vec<&str> = script
            .iter()
            .filter_map(|s| match s.action {
                Action::Shot(name) => Some(name),
                _ => None,
            })
            .collect();
        let total = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), total, "截图重名会互相覆盖");
    }
}
