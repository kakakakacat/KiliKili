//! 对照实验：这台机器上 Slint **最快**能呈现多少帧。
//!
//! 为什么要有它：主实验台量出「滚动时 403 次属性变更只换来 44 帧」，
//! 单看那一个数分不清是哪一种情况——
//!
//! 1. 两万行的 `ListView` 让 Slint 来不及重绘（那是 Phase 2 的坏消息）；
//! 2. 这台机器的合成/驱动把呈现限在 ~7fps（那和 Slint 没关系）。
//!
//! 这里把列表、数据、脚本、外部驱动全部拿掉，只留一个每 16ms 换一次
//! 文本的窗口。它跑出来的帧率就是**本机的上限**，主实验台的数字要减去
//! 这一层才算数。
//!
//! 用法：`frame-floor --seconds 8`，结果打到 stdout。

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use clap::Parser;
use slint::ComponentHandle;
use spike_harness::metrics::{Durations, FrameMeter};

slint::slint! {
    // 固定按 1.19 的语言行为编译：不写的话 slint 会警告"用的是 1.19 之前
    // 的行为"，而这份 spike 的结论都是按 1.19 下的。
    #[edition = "1.19"]
    import { ListView } from "std-widgets.slint";

    export component FloorWindow inherits Window {
        in property <string> label;
        in property <length> box-x;
        in property <length> scroll-y;
        in property <[string]> rows;
        // 0 = 只改文本与方块位置；1 = 只改 ListView 的 content-y。
        in property <int> mode;
        width: 640px;
        height: 400px;
        title: "frame-floor";
        Rectangle {
            background: #202020;
            if root.mode == 0: Rectangle {
                Text {
                    y: 20px;
                    color: #e0e0e0;
                    text: root.label;
                }
                // 一个会动的方块：纯文本有可能被某一层缓存掉，几何变化不会。
                Rectangle {
                    x: root.box-x;
                    y: 120px;
                    width: 60px;
                    height: 60px;
                    background: #4f8cff;
                }
            }
            // 滚动模式下窗口里**只有**这个列表，而且每帧只动 content-y。
            // 别的东西一概不改，这样「有没有重绘」只由 content-y 决定。
            if root.mode == 1: ListView {
                content-y: -root.scroll-y;
                for row in root.rows: Rectangle {
                    height: 48px;
                    background: #2a2a2a;
                    Text {
                        x: 8px;
                        color: #e0e0e0;
                        text: row;
                    }
                }
            }
        }
    }
}

#[derive(Parser, Debug)]
#[command(version, about = "量这台机器上 Slint 的呈现上限", long_about = None)]
struct Args {
    /// 跑多少秒。
    #[arg(long, default_value_t = 8)]
    seconds: u64,

    /// `text` = 每帧改一次文本与方块位置；`scroll` = 每帧只改 ListView
    /// 的 `content-y`，别的什么都不动。
    #[arg(long, default_value = "text")]
    mode: String,

    /// `scroll` 模式下列表里放多少行。
    #[arg(long, default_value_t = 20_000)]
    rows: usize,
}

struct State {
    meter: FrameMeter,
    draw: Durations,
    started_draw: Option<Instant>,
    ticks: u64,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let scroll_mode = args.mode == "scroll";
    let ui = FloorWindow::new()?;
    ui.set_mode(i32::from(scroll_mode));
    if scroll_mode {
        let rows: Vec<slint::SharedString> = (0..args.rows)
            .map(|i| slint::SharedString::from(format!("第 {i} 行")))
            .collect();
        ui.set_rows(std::rc::Rc::new(slint::VecModel::from(rows)).into());
    }

    let state = Rc::new(RefCell::new(State {
        meter: FrameMeter::new(),
        draw: Durations::new(),
        started_draw: None,
        ticks: 0,
    }));

    {
        let state = Rc::clone(&state);
        ui.window().set_rendering_notifier(move |rendering, _| {
            let mut state = state.borrow_mut();
            match rendering {
                slint::RenderingState::BeforeRendering => {
                    state.started_draw = Some(Instant::now());
                }
                slint::RenderingState::AfterRendering => {
                    let now = Instant::now();
                    if let Some(started) = state.started_draw.take() {
                        state
                            .draw
                            .push(now.duration_since(started).as_secs_f32() * 1000.0);
                    }
                    state.meter.tick(now);
                }
                _ => {}
            }
        })?;
    }

    let timer = slint::Timer::default();
    {
        let ui_weak = ui.as_weak();
        let state = Rc::clone(&state);
        let started = Instant::now();
        let deadline = Duration::from_secs(args.seconds);
        timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(16),
            move || {
                let Some(ui) = ui_weak.upgrade() else { return };
                let mut inner = state.borrow_mut();
                inner.ticks += 1;
                let ticks = inner.ticks;
                drop(inner);
                if scroll_mode {
                    // 每帧 38.4px，和主实验台的 2400px/s 一致。
                    ui.set_scroll_y(ticks as f32 * 38.4);
                } else {
                    ui.set_label(slint::SharedString::from(format!("tick {ticks}")));
                    // 0..560 之间来回走，保证每一帧的几何都和上一帧不同。
                    let phase = (ticks % 120) as f32;
                    let x = if phase < 60.0 { phase } else { 120.0 - phase };
                    ui.set_box_x(x * 9.3);
                }
                if started.elapsed() >= deadline {
                    let _ = slint::quit_event_loop();
                }
            },
        );
    }

    ui.run()?;

    let state = state.borrow();
    let secs = state.meter.elapsed().as_secs_f32();
    println!(
        "{:.1}s：{} 帧 / {} 次 tick，呈现 {:.1} fps；\
         渲染耗时 p50 {:.2} / p95 {:.2} / 最慢 {:.2} ms；\
         帧间隔 p50 {:.1} / p95 {:.1} ms",
        secs,
        state.meter.frames(),
        state.ticks,
        state.meter.frames() as f32 / secs.max(0.001),
        state.draw.percentile(0.5).unwrap_or(0.0),
        state.draw.percentile(0.95).unwrap_or(0.0),
        state.draw.max(),
        state.meter.percentile(0.5).unwrap_or(0.0),
        state.meter.percentile(0.95).unwrap_or(0.0),
    );
    Ok(())
}
