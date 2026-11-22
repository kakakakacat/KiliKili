//! 脚本化实验流程。
//!
//! 理由同 SPIKE-001：手动跑两次，滚动速度、停留时长、截图时机都不一样，
//! 两份数字摆在一起说明不了问题。脚本跑出来的两份（比如 Windows 与 macOS）
//! 才能逐行对照。
//!
//! 时间点**刻意留得宽**：外部驱动（切焦点、发按键、抓 UIA 树）是另起一个
//! PowerShell 进程做的，起进程本身就要几百毫秒。掐太紧量到的是进程启动
//! 速度，不是被测的东西。

// `Scroll` 带 f32，所以只派生 PartialEq：f32 没有 Eq。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Action {
    /// 切场景：0 长列表 / 1 富文本 / 2 输入 / 3 无障碍。
    Scene(i32, &'static str),
    /// 换一个帧耗时统计分段。各段分开统计，否则滚动的代价会被静止的帧
    /// 平均掉。
    Phase(&'static str),
    /// 以 px/s 匀速滚动若干毫秒。
    Scroll {
        speed: f32,
        ms: u64,
    },
    /// 直接跳到列表末尾：虚拟化列表最容易在这里露馅。
    JumpToEnd,
    JumpToTop,
    Shot(&'static str),
    /// 只把窗口切到前台，不发按键。
    ///
    /// **必须排在所有测帧耗时的分段之前。** 窗口不在前台时 Windows 会把
    /// 合成降到两三帧每秒，量到的 p95 是系统的省电策略，不是 Slint 的
    /// 渲染代价——2026-11-22 第一轮就是这么量出「滚动 p95 500ms」的。
    Activate,
    /// 外部驱动发一串按键（SendKeys 语法）。
    Keys(&'static str),
    /// 切到某个键盘布局（KLID）。`00000409` 是美式英文，`00000804` 是
    /// 简体中文（本机装的是微软拼音）。
    ///
    /// **英文那一段之前必须显式切回英文。** 切过去的输入法不会自己复位，
    /// 它跨进程、跨次运行地留着，下一轮的「英文直接输入」就在中文输入法
    /// 下进行：`abc` 上屏成 `ABC`、`def` 上屏成「得分」。这种污染是静默
    /// 的，而且只在连着跑两轮时才出现，单跑一轮永远看不见。
    Ime(&'static str, &'static str),
    /// 抓一次 UIA 树。
    Uia(&'static str),
    Minimize,
    Restore,
    Resize(u32, u32),
    Fullscreen(bool),
    Quit,
}

#[derive(Debug, Clone, Copy)]
pub struct Step {
    pub at_ms: u64,
    pub action: Action,
    /// 写进记录的「预期」。
    pub note: &'static str,
}

/// 默认脚本。
#[must_use]
pub fn default_script(
    scroll_speed: f32,
    ime_keys: &'static str,
    typed_keys: &'static str,
) -> Vec<Step> {
    vec![
        Step {
            at_ms: 0,
            action: Action::Scene(0, "长列表"),
            note: "整份模型一次交给 ListView，不分页",
        },
        Step {
            at_ms: 100,
            action: Action::Activate,
            note: "先把窗口切到前台，否则后面所有帧耗时量的都是后台限频",
        },
        Step {
            at_ms: 2_000,
            action: Action::Phase("列表静止"),
            note: "静止时的帧耗时基线",
        },
        Step {
            at_ms: 2_600,
            action: Action::Shot("01-list-top"),
            note: "列表顶部：头像、昵称、正文、配图、计数都在",
        },
        Step {
            at_ms: 2_900,
            action: Action::Uia("列表"),
            note: "两万行的列表在无障碍树里长什么样",
        },
        Step {
            at_ms: 5_700,
            action: Action::Phase("列表滚动"),
            note: "匀速滚动段，主结论看这一段的 p95/p99",
        },
        Step {
            at_ms: 5_800,
            action: Action::Scroll {
                speed: scroll_speed,
                ms: 6_000,
            },
            note: "匀速滚动 6 秒",
        },
        Step {
            at_ms: 12_000,
            action: Action::Shot("02-list-scrolled"),
            note: "滚动中：行高不一的内容应当稳定排布，无空白、无重叠",
        },
        Step {
            at_ms: 12_300,
            action: Action::Phase("跳到末尾"),
            note: "一次跳到底，看有没有长帧",
        },
        Step {
            at_ms: 12_500,
            action: Action::JumpToEnd,
            note: "滚到最后一行",
        },
        Step {
            at_ms: 13_300,
            action: Action::Shot("03-list-end"),
            note: "末尾：最后一行应当贴着底，不应当是空白",
        },
        Step {
            at_ms: 13_700,
            action: Action::JumpToTop,
            note: "回到顶部",
        },
        Step {
            at_ms: 14_300,
            action: Action::Scene(1, "富文本"),
            note: "同一条评论的三种画法并排",
        },
        Step {
            at_ms: 14_400,
            action: Action::Phase("富文本"),
            note: "富文本场景的帧耗时",
        },
        Step {
            at_ms: 15_300,
            action: Action::Shot("04-richtext"),
            note: "① 纯 Text ② StyledText ③ 手写排布；看表情、颜色、换行",
        },
        Step {
            at_ms: 15_900,
            action: Action::Scene(2, "输入与焦点"),
            note: "三个输入框",
        },
        Step {
            at_ms: 16_000,
            action: Action::Phase("输入"),
            note: "输入场景的帧耗时",
        },
        Step {
            at_ms: 16_100,
            action: Action::Ime("00000409", "切回英文键盘"),
            note: "先复位输入法，否则英文这一段会在上一轮留下的中文输入法里打",
        },
        Step {
            at_ms: 17_800,
            action: Action::Keys(typed_keys),
            note: "英文直接输入 + Tab 轮转焦点",
        },
        Step {
            at_ms: 21_200,
            action: Action::Ime("00000804", "切中文输入法"),
            note: "切中文输入法",
        },
        Step {
            at_ms: 22_700,
            action: Action::Keys(ime_keys),
            note: "打拼音并上屏：输入框里应当是中文，不是拼音字母",
        },
        Step {
            at_ms: 27_200,
            action: Action::Shot("05-input"),
            note: "三个框的内容与焦点",
        },
        Step {
            at_ms: 27_900,
            action: Action::Scene(3, "无障碍"),
            note: "带 role/label/value 的控件",
        },
        Step {
            at_ms: 28_000,
            action: Action::Phase("无障碍"),
            note: "无障碍场景的帧耗时",
        },
        Step {
            at_ms: 28_500,
            action: Action::Uia("控件"),
            note: "按钮/勾选/滑块/输入框是否各自带名字与取值",
        },
        Step {
            at_ms: 31_700,
            action: Action::Shot("06-a11y"),
            note: "无障碍场景画面",
        },
        Step {
            at_ms: 32_200,
            action: Action::Resize(980, 620),
            note: "缩小窗口：布局应当跟着重排",
        },
        Step {
            at_ms: 33_200,
            action: Action::Shot("07-resized"),
            note: "缩放后的布局",
        },
        Step {
            at_ms: 33_700,
            action: Action::Fullscreen(true),
            note: "全屏",
        },
        Step {
            at_ms: 35_200,
            action: Action::Shot("08-fullscreen"),
            note: "全屏下的布局",
        },
        Step {
            at_ms: 35_700,
            action: Action::Fullscreen(false),
            note: "退出全屏",
        },
        Step {
            at_ms: 36_500,
            action: Action::Minimize,
            note: "最小化：窗口不可见期间不应当继续满帧渲染",
        },
        Step {
            at_ms: 38_000,
            action: Action::Restore,
            note: "还原：画面应当立刻回来，不应当黑屏或丢状态",
        },
        Step {
            at_ms: 39_200,
            action: Action::Scene(0, "长列表（回到列表）"),
            note: "回到列表：之前的滚动位置与模型都还在",
        },
        Step {
            at_ms: 40_200,
            action: Action::Shot("09-back-to-list"),
            note: "回到列表后的画面",
        },
        Step {
            at_ms: 40_900,
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
        let script = default_script(2_400.0, "nihao ", "hello{TAB}world{TAB}1234567890");
        assert!(
            script.windows(2).all(|w| w[0].at_ms <= w[1].at_ms),
            "脚本的时间点必须单调不减，否则执行器会跳过步骤"
        );
        assert_eq!(script.last().map(|s| s.action), Some(Action::Quit));
    }

    #[test]
    fn every_shot_has_a_distinct_name() {
        let script = default_script(2_400.0, "nihao ", "hello{TAB}world{TAB}1234567890");
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

    #[test]
    fn the_window_is_activated_before_the_first_metered_phase() {
        // 这条不是洁癖：窗口不在前台时 Windows 把合成降到两三帧每秒，
        // 于是「滚动 p95」量出来是 500ms，看着像 Slint 画不动。
        let script = default_script(2_400.0, "nihao ", "hello{TAB}world{TAB}1234567890");
        let activate = script
            .iter()
            .position(|s| s.action == Action::Activate)
            .expect("脚本里必须有切前台这一步");
        let first_phase = script
            .iter()
            .position(|s| matches!(s.action, Action::Phase(_)))
            .expect("脚本里必须有分段");
        assert!(
            activate < first_phase,
            "切前台必须排在第一个计时分段之前，否则量到的是后台限频"
        );
    }

    #[test]
    fn the_scroll_segment_fits_inside_its_phase() {
        // 滚动必须整段落在「列表滚动」这个分段里，否则 p95 量到的是
        // 一半滚动一半静止。
        let script = default_script(2_400.0, "nihao ", "hello{TAB}world{TAB}1234567890");
        let phase_at = |name: &str| {
            script
                .iter()
                .find(|s| matches!(s.action, Action::Phase(p) if p == name))
                .map(|s| s.at_ms)
                .expect("分段应当存在")
        };
        let scroll = script
            .iter()
            .find(|s| matches!(s.action, Action::Scroll { .. }))
            .expect("应当有滚动步骤");
        let Action::Scroll { ms, .. } = scroll.action else {
            unreachable!()
        };
        assert!(phase_at("列表滚动") <= scroll.at_ms);
        assert!(scroll.at_ms + ms <= phase_at("跳到末尾"));
    }
}
