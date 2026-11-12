//! 播放进度游标：一帧进度该不该取桶、要不要先清屏。
//!
//! ## 这个模块为什么存在（issue #89）
//!
//! 基线 `videoPositionListen` 只有一道闸门：
//!
//! ```dart
//! if (currentPosition == latestAddedPosition) return;
//! ```
//!
//! 它挡**重复**，不挡**倒退**。播放器的 position 流在退到后台再回前台时
//! 可能先吐一个很早的旧值（甚至 0）再追上来，那一帧不等于上一次的值，
//! 于是视频开头那一桶弹幕被喂进渲染器——用户看到的就是「飘出开头的弹幕」。
//!
//! 判据只有三条，但它是**策略**，按 RFC-0001 §2.3 归 Rust：
//!
//! 1. 进度**没有前进**（`<=` 上次发过的桶）——跳过。宁可漏掉倒退期间的
//!    几桶，也不能把不属于当前进度的弹幕放出去：漏掉是「这 0.1 秒没人
//!    发弹幕」，放错是用户看得见的错乱。
//! 2. 进度**前进**——正常取桶。
//! 3. **seek**（由调用方递增 `epoch` 宣告）——先清屏，再按新位置取桶。
//!    这是第 1 条唯一的例外，也是它必须有例外的原因：真正的向后 seek
//!    与「后台回来的回跳」在 position 流里长得一模一样，只能靠播放器
//!    主动宣告来区分（`PlPlayerController.seekTo` 自增 epoch）。
//!
//! ## 为什么状态由调用方持有
//!
//! 本模块是**纯函数**：没有全局表、不跟会话绑定。原因是它必须对两条
//! 路径都生效——Rust 弹幕引擎开着时取 Rust 的桶，引擎降级回 Dart 时
//! 取 `_dmSegMap` 的桶，而降级恰恰发生在 Rust 会话建不起来的时候
//! （见 `crate::rule`）。把游标挂在会话上，降级路径就没人看门了。
//!
//! 调用方持有的 [`CursorState`] 只是两个数字，不含判断——判断全在
//! [`step`] 里，这正是「状态可以在宿主，策略不行」的分界。

/// 分桶粒度：100ms，与 [`crate::engine::Buckets`] 一致。
pub const BUCKET_MS: i32 = 100;

/// 游标状态。由调用方持有并原样传回，本身不含任何判断。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CursorState {
    /// 上一次**发出过**的桶（整百毫秒）。`None` 表示还没发过。
    pub last_bucket_ms: Option<i32>,
    /// 已经看到过的 seek 代际。调用方每次 seek 自增，游标据此认出不连续。
    pub seen_epoch: i64,
}

/// 一帧进度的处理方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressStep {
    /// 跳过这一帧：不取桶，也不要触发分段请求。
    Skip,
    /// 按 `bucket_ms` 取桶并渲染。
    Emit { bucket_ms: i32 },
    /// 不连续：先清屏，再按 `bucket_ms` 取桶。
    Restart { bucket_ms: i32 },
}

/// [`step`] 的结果：下一步动作 + 调用方该存回去的状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CursorOutcome {
    pub step: ProgressStep,
    pub state: CursorState,
}

/// 判定这一帧进度该怎么处理。
///
/// `position_ms` 是播放器报的原始毫秒，`epoch` 是调用方当前的 seek 代际。
/// 负数进度按 0 处理（播放器偶尔会在起播前报负值）。
#[must_use]
pub fn step(state: CursorState, position_ms: i32, epoch: i64) -> CursorOutcome {
    let bucket_ms = bucket_of(position_ms);

    // seek：唯一允许进度倒退的入口。先清屏再按新位置开始。
    if epoch != state.seen_epoch {
        return CursorOutcome {
            step: ProgressStep::Restart { bucket_ms },
            state: CursorState {
                last_bucket_ms: Some(bucket_ms),
                seen_epoch: epoch,
            },
        };
    }

    // 没前进就不发。`==` 是基线那道去重闸门，`<` 是 issue #89 补上的那道。
    if let Some(last) = state.last_bucket_ms
        && bucket_ms <= last
    {
        return CursorOutcome {
            step: ProgressStep::Skip,
            state,
        };
    }

    CursorOutcome {
        step: ProgressStep::Emit { bucket_ms },
        state: CursorState {
            last_bucket_ms: Some(bucket_ms),
            seen_epoch: epoch,
        },
    }
}

/// 取整百毫秒。基线写作 `currentPosition -= currentPosition % 100`。
#[must_use]
fn bucket_of(position_ms: i32) -> i32 {
    if position_ms <= 0 {
        return 0;
    }
    position_ms - position_ms % BUCKET_MS
}

#[cfg(test)]
mod tests {
    use super::{CursorState, ProgressStep, step};

    /// 喂一串进度，返回每一帧的动作；状态在内部滚动，模拟调用方。
    fn run(frames: &[(i32, i64)]) -> Vec<ProgressStep> {
        let mut state = CursorState::default();
        frames
            .iter()
            .map(|&(position_ms, epoch)| {
                let outcome = step(state, position_ms, epoch);
                state = outcome.state;
                outcome.step
            })
            .collect()
    }

    #[test]
    fn normal_playback_emits_each_new_bucket() {
        assert_eq!(
            run(&[(0, 0), (120, 0), (250, 0)]),
            vec![
                ProgressStep::Emit { bucket_ms: 0 },
                ProgressStep::Emit { bucket_ms: 100 },
                ProgressStep::Emit { bucket_ms: 200 },
            ]
        );
    }

    #[test]
    fn the_same_bucket_is_not_emitted_twice() {
        // 基线原有的去重：一个 100ms 桶内的多帧只发一次。
        assert_eq!(
            run(&[(1_000, 0), (1_040, 0), (1_099, 0)]),
            vec![
                ProgressStep::Emit { bucket_ms: 1_000 },
                ProgressStep::Skip,
                ProgressStep::Skip,
            ]
        );
    }

    #[test]
    fn a_rewind_without_a_seek_emits_nothing_until_it_catches_up() {
        // issue #89 的现象：退到后台再回来，position 先吐一个 0 再追上。
        // 回跳那几帧一桶都不许发，否则开头的弹幕会飘出来。
        let steps = run(&[
            (600_000, 0),
            (0, 0),
            (100, 0),
            (300_000, 0),
            (600_000, 0),
            (600_100, 0),
        ]);
        assert_eq!(
            steps,
            vec![
                ProgressStep::Emit { bucket_ms: 600_000 },
                ProgressStep::Skip,
                ProgressStep::Skip,
                ProgressStep::Skip,
                ProgressStep::Skip,
                ProgressStep::Emit { bucket_ms: 600_100 },
            ],
            "回跳期间不得发桶，追上之后要恢复正常"
        );
    }

    #[test]
    fn a_backward_seek_restarts_at_the_new_position() {
        // 真正的向后 seek 走同一条 position 流，但调用方递增了 epoch。
        assert_eq!(
            run(&[(600_000, 0), (10_000, 1), (10_100, 1)]),
            vec![
                ProgressStep::Emit { bucket_ms: 600_000 },
                ProgressStep::Restart { bucket_ms: 10_000 },
                ProgressStep::Emit { bucket_ms: 10_100 },
            ]
        );
    }

    #[test]
    fn a_forward_seek_also_restarts() {
        // 向前 seek 本来就满足「前进」，但仍要清屏：屏上飘着的是旧位置
        // 的弹幕，留着就是跳转后那一两秒的错乱。
        assert_eq!(
            run(&[(10_000, 0), (600_000, 1)]),
            vec![
                ProgressStep::Emit { bucket_ms: 10_000 },
                ProgressStep::Restart { bucket_ms: 600_000 },
            ]
        );
    }

    #[test]
    fn seeking_to_the_bucket_we_are_already_in_still_restarts() {
        // 原地 seek（或回到同一桶）：去重闸门不能把它吃掉，否则清屏之后
        // 这一桶再也不会被补上。
        assert_eq!(
            run(&[(10_000, 0), (10_050, 1)]),
            vec![
                ProgressStep::Emit { bucket_ms: 10_000 },
                ProgressStep::Restart { bucket_ms: 10_000 },
            ]
        );
    }

    #[test]
    fn a_negative_position_is_clamped_to_zero() {
        assert_eq!(run(&[(-20, 0)]), vec![ProgressStep::Emit { bucket_ms: 0 }]);
    }
}
