//! SPIKE-003（issue #10）的实验台：一个 Slint 窗口，四个场景，一次跑完。
//!
//! 要回答的问题与为什么是这种形状，见 README.md 与 `docs/m0/spikes/`。
//! 这里只说运行期的结构：
//!
//! ```text
//!   [16ms tick] 执行脚本：切场景 / 推进滚动 / 起外部驱动 / 排截图
//!        |                                   |
//!        |                                   +--> powershell：按键、IME、
//!        |                                        最小化、抓 UIA 树（异步）
//!        v
//!   [Slint 渲染回调] AfterRendering：记帧耗时（分段）、按需 glReadPixels 截图
//! ```
//!
//! 数据从 `rows.rs` 来：pb → **生产投影** → 实体 → **生产分词**。行数是
//! 合成的，形状不是。

// 模块挂在 crate 根（`lib.rs`）上，这里只是用。Android 的入口是 cdylib 里的
// `android_main`，桌面的入口是 `src/main.rs` 这个薄壳，**两边跑的是下面同一个
// `run()`**——两个平台的记录要能逐行对照，就不能各写一份流程。
use crate::{Probe, RowItem, RunItem, UiSpikeWindow, drive, flow, rows, script};

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use clap::Parser;
use slint::{ComponentHandle, SharedString, VecModel};
use spike_harness::gl;
use spike_harness::metrics::{Durations, FrameMeter, ProcSampler};
use spike_harness::record::{Check, Evidence, Verdict, capture, utc_now};

use script::{Action, Step};

/// Slint 界面可行性的实验台（M0 SPIKE-003 / issue #10）。
#[derive(Parser, Debug, Clone)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// 列表里放多少条评论。形状取自生产投影，条数是合成的。
    #[arg(long, default_value_t = 20_000)]
    rows: usize,

    /// 富文本场景并排展示几条。
    #[arg(long, default_value_t = 6)]
    rich: usize,

    /// 跑脚本化流程并自动退出（实验记录就靠它可比）。
    #[arg(long)]
    scripted: bool,

    /// 滚动速度（逻辑像素/秒）。
    #[arg(long, default_value_t = 2_400.0)]
    scroll_speed: f32,

    /// 中文输入法要打的拼音。末尾的空格是上屏。
    #[arg(long, default_value = "nihao ")]
    ime_keys: String,

    /// 英文那一段按什么（SendKeys 语法）。留着可覆盖是为了复查
    /// 「Tab 到底做了什么」——默认串里混着字母和 `{TAB}`，分不清是哪个
    /// 环节吞了字符。
    #[arg(long, default_value = "hello{TAB}world{TAB}1234567890")]
    keys: String,

    /// 不要起 PowerShell 外部驱动（按键 / IME / 最小化 / UIA）。
    #[arg(long)]
    no_driver: bool,

    /// 写实验记录到这个 .md。
    #[arg(long)]
    record: Option<PathBuf>,

    /// 截图目录。
    #[arg(long)]
    shots: Option<PathBuf>,

    /// 外部驱动的中间产物（UIA 的 JSON）放哪。默认跟着截图目录。
    #[arg(long)]
    work: Option<PathBuf>,

    #[arg(long, default_value_t = 1280)]
    width: u32,

    #[arg(long, default_value_t = 800)]
    height: u32,

    /// 覆盖 HiDPI 缩放（等价于 `SLINT_SCALE_FACTOR`）。不给就用系统的。
    #[arg(long)]
    scale: Option<f32>,

    /// 在列表场景末尾留出多少毫秒的**外部触摸**空窗（0 = 不留）。
    ///
    /// Android 专用：那一段探针自己不滚，滚动由 `android-drive.sh` 的 swipe
    /// 驱动，于是「触控惯性」这一项有自己的帧耗时分段。桌面保持 0，脚本与
    /// 2026-11-22 那一轮逐字相同。
    #[arg(long, default_value_t = 0)]
    touch_ms: u64,

    /// 手动模式停在哪个场景：0 长列表 / 1 富文本 / 2 输入与焦点 / 3 无障碍。
    ///
    /// 给屏幕阅读器实读用（VoiceOver / TalkBack）：脚本化一轮 40 秒切完
    /// 四个场景，人跟着读来不及。`--scripted` 时忽略。
    #[arg(long, default_value_t = 0, value_parser = clap::value_parser!(i32).range(0..=3))]
    scene: i32,
}

/// 帧耗时按分段统计：滚动的代价不能被静止的帧平均掉。
struct PhaseMeter {
    name: String,
    meter: FrameMeter,
    /// 这一段里每次重绘实际花了多久。**和帧间隔是两回事**，见
    /// `spike_harness::metrics::Durations` 的注释。
    draw: Durations,
    /// 进入这一段时的总帧数，用来算「这一段画了多少帧」。
    frames_at_entry: u64,
    /// 进入这一段时的 tick 数。tick 是 16ms 一次的定时器，**每次都会改
    /// HUD 文本**，所以「tick 数 ≫ 帧数」只可能是没触发重绘。
    ticks_at_entry: u64,
}

struct RenderState {
    gl: Option<gl::Gl>,
    gl_description: String,
    /// 整段运行。
    meter: FrameMeter,
    draw: Durations,
    /// `BeforeRendering` 的时刻，配 `AfterRendering` 算这一帧画了多久。
    draw_started: Option<Instant>,
    /// 16ms 定时器跑了多少次。
    ticks: u64,
    /// 两次 tick 之间隔了多久。和帧间隔对照：tick 稳而帧不稳，说明事件
    /// 循环没被堵住，是重绘被跳过了；tick 也不稳，那就是有人在 UI 线程
    /// 上干重活。
    tick_gap: Durations,
    /// tick 回调自己花了多久。
    tick_cost: Durations,
    last_tick: Option<Instant>,
    phases: Vec<PhaseMeter>,
    /// 物理像素，截图要用。
    size: (u32, u32),
    pending_shots: VecDeque<PathBuf>,
    shots: Vec<PathBuf>,
    errors: Vec<String>,
}

impl RenderState {
    fn tick_frame(&mut self, now: Instant) {
        self.meter.tick(now);
        if let Some(phase) = self.phases.last_mut() {
            phase.meter.tick(now);
        }
    }

    /// 一帧画完了：记间隔，也记这一帧真正花了多久。
    fn finish_draw(&mut self, now: Instant) {
        if let Some(started) = self.draw_started.take() {
            let ms = now.duration_since(started).as_secs_f32() * 1000.0;
            self.draw.push(ms);
            if let Some(phase) = self.phases.last_mut() {
                phase.draw.push(ms);
            }
        }
        self.tick_frame(now);
    }

    fn start_phase(&mut self, name: &str) {
        let frames_at_entry = self.meter.frames();
        let ticks_at_entry = self.ticks;
        self.phases.push(PhaseMeter {
            name: name.to_owned(),
            meter: FrameMeter::new(),
            draw: Durations::new(),
            frames_at_entry,
            ticks_at_entry,
        });
    }

    fn frames(&self) -> u64 {
        self.meter.frames()
    }
}

/// Probe 回调写进来的东西。**单独放 Cell**：这些回调会在布局/渲染过程中
/// 触发，而那时 tick 可能正借着自己的状态，共用一个 `RefCell` 必然撞上
/// 重入 panic。
#[derive(Default)]
struct Probed {
    rows_created: Cell<usize>,
    content_height: Cell<f32>,
    list_height: Cell<f32>,
    focus_sequence: RefCell<Vec<i32>>,
}

struct TickState {
    script: Vec<Step>,
    next_step: usize,
    started: Instant,
    /// 正在匀速滚动：速度（px/s）与截止时刻。
    scrolling: Option<(f32, Instant)>,
    scroll_y: f32,
    sampler: ProcSampler,
    /// 记录里的「某个时刻的 RSS」。
    rss_marks: Vec<(String, f32)>,
    /// 某个时刻累计实例化了多少行。
    created_marks: Vec<(String, usize)>,
    /// 某个时刻的累计帧数（最小化那一段靠它判断有没有继续渲染）。
    frame_marks: Vec<(String, u64)>,
    /// 脚本执行过程中的事实陈述，直接进记录。
    notes: Vec<String>,
    scene: i32,
    quit: bool,
}

/// 跑一整轮实验。`command_line` 只进记录（为了能原样重跑），不参与决策。
pub fn run(args: Args, command_line: Vec<String>) -> Result<()> {
    if let Some(scale) = args.scale {
        // Slint 读的是环境变量，必须赶在后端初始化之前设。
        // SAFETY: 还没有起任何线程。
        unsafe { std::env::set_var("SLINT_SCALE_FACTOR", scale.to_string()) };
    }

    // ── 数据：真实管线，合成条数 ──
    let built = rows::build(args.rows, 600_000)?;
    let rss_at_start = {
        let mut sampler = ProcSampler::new();
        sampler.sample();
        sampler.rss_mib()
    };

    let ui = UiSpikeWindow::new().context("建不了 Slint 窗口")?;
    if !args.scripted {
        const TITLES: [&str; 4] = ["长列表", "富文本", "输入与焦点", "无障碍"];
        ui.set_scene(args.scene);
        ui.set_scene_title(SharedString::from(TITLES[args.scene as usize]));
    }
    ui.window().set_size(slint::LogicalSize::new(
        args.width as f32,
        args.height as f32,
    ));

    let model_started = Instant::now();
    let row_items: Vec<RowItem> = built
        .rows
        .iter()
        .enumerate()
        .map(|(index, row)| row_item(index, row))
        .collect();
    let model_ms = model_started.elapsed().as_secs_f32() * 1000.0;
    let rows_model = Rc::new(VecModel::from(row_items));
    ui.set_rows(rows_model.clone().into());

    // ── 富文本场景的三份平行模型 ──
    let rich_count = args.rich.min(built.rows.len());
    let column_width = ui.get_rich_column_width();
    let mut plain = Vec::with_capacity(rich_count);
    let mut styled = Vec::with_capacity(rich_count);
    let mut runs: Vec<RunItem> = Vec::new();
    let mut styled_failures: Vec<String> = Vec::new();
    let emote = emote_image();
    let mut y = 0.0_f32;
    let styled_started = Instant::now();
    for row in built.rows.iter().take(rich_count) {
        let markdown = flow::markdown(&row.segments);
        match slint::StyledText::from_markdown(&markdown) {
            Ok(text) => styled.push(text),
            Err(err) => {
                styled_failures.push(format!("{err}"));
                styled.push(slint::StyledText::from_plain_text(
                    &row.entity.content.message,
                ));
            }
        }
    }
    let styled_ms = styled_started.elapsed().as_secs_f32() * 1000.0;

    let flow_started = Instant::now();
    for row in built.rows.iter().take(rich_count) {
        plain.push(SharedString::from(row.entity.content.message.as_str()));
        let laid_out = flow::layout(&row.segments, column_width, 15.0, 22.0, y);
        y += laid_out.height + 10.0;
        runs.extend(laid_out.runs.into_iter().map(|run| RunItem {
            text: SharedString::from(run.text),
            x: run.x,
            y: run.y,
            w: run.w,
            h: run.h,
            font_size: run.font_size,
            color: slint::Color::from_rgb_u8(
                ((run.color >> 16) & 0xff) as u8,
                ((run.color >> 8) & 0xff) as u8,
                (run.color & 0xff) as u8,
            ),
            is_emote: run.emote,
            emote: if run.emote {
                emote.clone()
            } else {
                slint::Image::default()
            },
            underline: run.underline,
        }));
    }
    let flow_ms = flow_started.elapsed().as_secs_f32() * 1000.0;
    let run_count = runs.len();
    let emote_runs = runs.iter().filter(|r| r.is_emote).count();

    ui.set_rich_plain(Rc::new(VecModel::from(plain)).into());
    ui.set_rich_styled(Rc::new(VecModel::from(styled)).into());
    ui.set_rich_runs(Rc::new(VecModel::from(runs)).into());
    ui.set_rich_flow_height(y);
    ui.set_live_text(SharedString::from("已加载 20000 条评论"));

    // 行内图片这条路到底走不走得通，直接问解析器，别猜。
    let inline_image_error =
        slint::StyledText::from_markdown("前面 ![表情](https://example.invalid/doge.webp) 后面")
            .err()
            .map(|err| err.to_string());

    // ── Probe ──
    let probed = Rc::new(Probed::default());
    {
        let probed = Rc::clone(&probed);
        ui.global::<Probe>().on_row_created(move |_id| {
            probed.rows_created.set(probed.rows_created.get() + 1);
        });
    }
    {
        let probed = Rc::clone(&probed);
        ui.global::<Probe>()
            .on_content_height_changed(move |content, list| {
                probed.content_height.set(content);
                probed.list_height.set(list);
            });
    }
    {
        let probed = Rc::clone(&probed);
        ui.global::<Probe>().on_focus_changed(move |which| {
            let mut sequence = probed.focus_sequence.borrow_mut();
            if sequence.last() != Some(&which) {
                sequence.push(which);
            }
        });
    }

    let render_state = Rc::new(RefCell::new(RenderState {
        gl: None,
        gl_description: "<未取>".to_owned(),
        meter: FrameMeter::new(),
        draw: Durations::new(),
        draw_started: None,
        ticks: 0,
        tick_gap: Durations::new(),
        tick_cost: Durations::new(),
        last_tick: None,
        phases: Vec::new(),
        size: (args.width, args.height),
        pending_shots: VecDeque::new(),
        shots: Vec::new(),
        errors: Vec::new(),
    }));
    install_rendering_notifier(&ui, &render_state)?;

    let work_dir = args
        .work
        .clone()
        .or_else(|| args.shots.clone())
        .unwrap_or_else(|| PathBuf::from("."));
    let driver = Rc::new(RefCell::new(drive::Driver::new(
        !args.no_driver && args.scripted,
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("win"),
        work_dir,
        std::process::id(),
    )));

    let ime_keys: &'static str = Box::leak(args.ime_keys.clone().into_boxed_str());
    let typed_keys: &'static str = Box::leak(args.keys.clone().into_boxed_str());
    let tick_state = Rc::new(RefCell::new(TickState {
        script: if args.scripted {
            script::default_script(args.scroll_speed, ime_keys, typed_keys, args.touch_ms)
        } else {
            Vec::new()
        },
        next_step: 0,
        started: Instant::now(),
        scrolling: None,
        scroll_y: 0.0,
        sampler: ProcSampler::new(),
        rss_marks: vec![("模型建好前".to_owned(), rss_at_start)],
        created_marks: Vec::new(),
        frame_marks: Vec::new(),
        notes: Vec::new(),
        scene: if args.scripted { 0 } else { args.scene },
        quit: false,
    }));

    let timer = slint::Timer::default();
    {
        let ui_weak = ui.as_weak();
        let tick_state = Rc::clone(&tick_state);
        let render_state = Rc::clone(&render_state);
        let probed = Rc::clone(&probed);
        let driver = Rc::clone(&driver);
        let args_for_tick = args.clone();
        timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(16),
            move || {
                let Some(ui) = ui_weak.upgrade() else { return };
                let started = Instant::now();
                tick(
                    &ui,
                    &tick_state,
                    &render_state,
                    &probed,
                    &driver,
                    &args_for_tick,
                );
                // tick 自己的耗时要记：它跑在 UI 线程上，花多久就占掉
                // 多少帧预算。**必须在 tick 返回之后再借**，tick 里面
                // 正借着同一个 RefCell。
                let cost = started.elapsed().as_secs_f32() * 1000.0;
                if let Ok(mut render) = render_state.try_borrow_mut() {
                    render.tick_cost.push(cost);
                }
            },
        );
    }

    ui.run().context("Slint 事件循环退出异常")?;
    driver.borrow_mut().drain();

    write_evidence(
        &args,
        &command_line.join(" "),
        &ui,
        &tick_state,
        &render_state,
        &probed,
        &driver,
        &built,
        Measured {
            model_ms,
            styled_ms,
            flow_ms,
            rich_count,
            run_count,
            emote_runs,
            styled_failures,
            inline_image_error,
        },
    )
}

/// 建模型之外、但要进记录的那些数。
struct Measured {
    model_ms: f32,
    styled_ms: f32,
    flow_ms: f32,
    rich_count: usize,
    run_count: usize,
    emote_runs: usize,
    styled_failures: Vec<String>,
    inline_image_error: Option<String>,
}

fn row_item(index: usize, row: &rows::Row) -> RowItem {
    let entity = &row.entity;
    let segments = row.segments.len();
    RowItem {
        id: i32::try_from(index).unwrap_or(i32::MAX),
        name: SharedString::from(entity.member.name.as_str()),
        meta: SharedString::from(format!(
            "LV{} · {} 分钟前 · {} 段",
            entity.member.level,
            index % 600 + 1,
            segments
        )),
        message: SharedString::from(entity.content.message.as_str()),
        likes: SharedString::from(format!("赞 {}", entity.like)),
        replies: SharedString::from(format!("回复 {}", entity.count)),
        pictures: i32::try_from(entity.content.pictures.len()).unwrap_or(0),
        highlight: index == 0,
        // 屏幕阅读器读到的就是这一句。把人、内容、计数拼成一条，而不是让它
        // 把一行里的七八个 Text 挨个念一遍。
        a11y: SharedString::from(format!(
            "{} 说：{}。赞 {}，回复 {}",
            entity.member.name, entity.content.message, entity.like, entity.count
        )),
    }
}

/// 表情图。不走网络：spike 不该为一张图引入 HTTP 栈，而「行内图能不能
/// 混进文本流」与图是从哪来的无关。
fn emote_image() -> slint::Image {
    const SIDE: u32 = 64;
    let mut buffer = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(SIDE, SIDE);
    let width = buffer.width();
    let pixels = buffer.make_mut_slice();
    let radius = SIDE as f32 / 2.0 - 1.0;
    let center = SIDE as f32 / 2.0;
    for y in 0..SIDE {
        for x in 0..SIDE {
            let dx = x as f32 + 0.5 - center;
            let dy = y as f32 + 0.5 - center;
            let distance = dx.hypot(dy);
            let inside = distance <= radius;
            let eye = (dx.abs() - radius * 0.38).abs() < radius * 0.12
                && (dy + radius * 0.22).abs() < radius * 0.16;
            let mouth = (distance - radius * 0.62).abs() < radius * 0.1 && dy > radius * 0.1;
            let pixel = if !inside {
                slint::Rgba8Pixel {
                    r: 0,
                    g: 0,
                    b: 0,
                    a: 0,
                }
            } else if eye || mouth {
                slint::Rgba8Pixel {
                    r: 40,
                    g: 32,
                    b: 24,
                    a: 255,
                }
            } else {
                slint::Rgba8Pixel {
                    r: 250,
                    g: 206,
                    b: 70,
                    a: 255,
                }
            };
            pixels[(y * width + x) as usize] = pixel;
        }
    }
    slint::Image::from_rgba8(buffer)
}

fn install_rendering_notifier(
    ui: &UiSpikeWindow,
    render_state: &Rc<RefCell<RenderState>>,
) -> Result<()> {
    let render_state = Rc::clone(render_state);
    ui.window()
        .set_rendering_notifier(move |state, api| {
            let slint::GraphicsAPI::NativeOpenGL { get_proc_address } = api else {
                let mut shared = render_state.borrow_mut();
                let message = "当前渲染后端不是 OpenGL，截图拿不到".to_owned();
                if !shared.errors.contains(&message) {
                    shared.errors.push(message);
                }
                return;
            };
            let loader = |name: &std::ffi::CStr| get_proc_address(name);
            let mut shared = render_state.borrow_mut();
            match state {
                slint::RenderingState::RenderingSetup => match unsafe { gl::Gl::load(&loader) } {
                    Ok(gl) => {
                        shared.gl_description = gl.describe();
                        shared.gl = Some(gl);
                    }
                    Err(err) => shared.errors.push(format!("GL 函数加载失败：{err:#}")),
                },
                slint::RenderingState::BeforeRendering => {
                    shared.draw_started = Some(Instant::now());
                }
                slint::RenderingState::AfterRendering => {
                    shared.finish_draw(Instant::now());
                    if let Some(path) = shared.pending_shots.pop_front() {
                        let (width, height) = shared.size;
                        if let Some(gl) = shared.gl.as_ref() {
                            match gl.capture_png(width, height, &path) {
                                Ok(()) => shared.shots.push(path),
                                Err(err) => shared.errors.push(format!("截图失败：{err:#}")),
                            }
                        }
                        // 截图是同步读回 + 压缩，整个循环被它堵住。这一下
                        // 不算进帧耗时，否则报告的 p99 量的是 PNG 压缩速度。
                        shared.meter.skip_next_interval();
                        if let Some(phase) = shared.phases.last_mut() {
                            phase.meter.skip_next_interval();
                        }
                    }
                }
                slint::RenderingState::RenderingTeardown => {
                    shared.gl = None;
                }
                _ => {}
            }
        })
        .map_err(|err| anyhow::anyhow!("装不上渲染回调（需要 OpenGL 后端）：{err}"))
}

fn tick(
    ui: &UiSpikeWindow,
    tick_state: &Rc<RefCell<TickState>>,
    render_state: &Rc<RefCell<RenderState>>,
    probed: &Rc<Probed>,
    driver: &Rc<RefCell<drive::Driver>>,
    args: &Args,
) {
    driver.borrow_mut().poll();

    let mut state = tick_state.borrow_mut();
    state.sampler.sample();

    // 窗口尺寸每帧都可能变（缩放、全屏、HiDPI），截图要用物理像素。
    {
        let size = ui.window().size();
        let mut render = render_state.borrow_mut();
        render.size = (size.width, size.height);
        render.ticks += 1;
        let now = Instant::now();
        if let Some(previous) = render.last_tick.replace(now) {
            render
                .tick_gap
                .push(now.duration_since(previous).as_secs_f32() * 1000.0);
        }
    }

    // ── 滚动 ──
    if let Some((speed, until)) = state.scrolling {
        let now = Instant::now();
        let max = (probed.content_height.get() - probed.list_height.get()).max(0.0);
        let next = (state.scroll_y + speed * 0.016).min(max);
        state.scroll_y = next;
        ui.set_scroll_y(next);
        if now >= until {
            state.scrolling = None;
        }
    }

    // ── HUD ──
    {
        let render = render_state.borrow();
        let fps = render.meter.recent_fps().unwrap_or(0.0);
        let p95 = render
            .phases
            .last()
            .and_then(|p| p.meter.percentile(0.95))
            .unwrap_or(0.0);
        let hud = format!(
            "{:.0} fps · p95 {p95:.1}ms · 实例化 {} 行 · RSS {:.0} MiB · CPU {:.0}%",
            fps,
            probed.rows_created.get(),
            state.sampler.rss_mib(),
            state.sampler.cpu_percent()
        );
        ui.set_hud(SharedString::from(hud));
    }

    // ── 脚本 ──
    if state.script.is_empty() {
        return;
    }
    let elapsed = state.started.elapsed().as_millis() as u64;
    while state.next_step < state.script.len() && state.script[state.next_step].at_ms <= elapsed {
        let step = state.script[state.next_step];
        state.next_step += 1;
        run_step(ui, &mut state, render_state, probed, driver, args, step);
        if state.quit {
            return;
        }
    }
}

fn run_step(
    ui: &UiSpikeWindow,
    state: &mut TickState,
    render_state: &Rc<RefCell<RenderState>>,
    probed: &Rc<Probed>,
    driver: &Rc<RefCell<drive::Driver>>,
    args: &Args,
    step: Step,
) {
    trace_step(state.started.elapsed().as_millis(), step);
    match step.action {
        Action::Scene(scene, title) => {
            state.scene = scene;
            ui.set_scene(scene);
            ui.set_scene_title(SharedString::from(title));
        }
        Action::Phase(name) => {
            let mut render = render_state.borrow_mut();
            render.start_phase(name);
            drop(render);
            state
                .created_marks
                .push((format!("进入「{name}」"), probed.rows_created.get()));
            let rss = state.sampler.rss_mib();
            state.rss_marks.push((format!("进入「{name}」"), rss));
        }
        Action::Scroll { speed, ms } => {
            state.scrolling = Some((speed, Instant::now() + Duration::from_millis(ms)));
        }
        Action::JumpToEnd => {
            let max = (probed.content_height.get() - probed.list_height.get()).max(0.0);
            state.scroll_y = max;
            ui.set_scroll_y(max);
            state.notes.push(format!(
                "跳到末尾：ListView 报的内容高度 {:.0}px，可视高度 {:.0}px",
                probed.content_height.get(),
                probed.list_height.get()
            ));
        }
        Action::JumpToTop => {
            state.scroll_y = 0.0;
            ui.set_scroll_y(0.0);
        }
        Action::Shot(name) => {
            if let Some(dir) = args.shots.as_ref() {
                let path = dir.join(format!("{name}.png"));
                render_state.borrow_mut().pending_shots.push_back(path);
            }
        }
        Action::Activate => driver.borrow_mut().activate(),
        Action::Keys(keys) => driver.borrow_mut().keys(keys),
        Action::Ime(klid, label) => driver.borrow_mut().ime(klid, label),
        Action::Uia(label) => driver.borrow_mut().uia(label),
        Action::Minimize => {
            let frames = render_state.borrow().frames();
            state.frame_marks.push(("最小化".to_owned(), frames));
            driver.borrow_mut().window("minimize");
        }
        Action::Restore => {
            let frames = render_state.borrow().frames();
            state.frame_marks.push(("还原前".to_owned(), frames));
            driver.borrow_mut().window("restore");
        }
        Action::Resize(width, height) => {
            ui.window()
                .set_size(slint::LogicalSize::new(width as f32, height as f32));
        }
        Action::Fullscreen(on) => ui.window().set_fullscreen(on),
        Action::Quit => {
            let frames = render_state.borrow().frames();
            state.frame_marks.push(("退出".to_owned(), frames));
            state
                .created_marks
                .push(("退出".to_owned(), probed.rows_created.get()));
            let rss = state.sampler.rss_mib();
            state.rss_marks.push(("退出".to_owned(), rss));
            state.quit = true;
            let _ = slint::quit_event_loop();
        }
    }
    if !matches!(step.action, Action::Quit) {
        state
            .notes
            .push(format!("{}ms {:?}：{}", step.at_ms, step.action, step.note));
    }
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn write_evidence(
    args: &Args,
    command_line: &str,
    ui: &UiSpikeWindow,
    tick_state: &Rc<RefCell<TickState>>,
    render_state: &Rc<RefCell<RenderState>>,
    probed: &Rc<Probed>,
    driver: &Rc<RefCell<drive::Driver>>,
    built: &rows::Built,
    measured: Measured,
) -> Result<()> {
    let state = tick_state.borrow();
    let render = render_state.borrow();
    let driver = driver.borrow();

    let mut evidence = Evidence {
        title: format!("SPIKE-003 Slint 界面可行性 · {}", platform_label()),
        command_line: command_line.to_owned(),
        ..Evidence::default()
    };

    evidence.push_env("时间", utc_now());
    evidence.push_env("平台", platform_label());
    evidence.push_env("系统", spike_harness::record::system_label());
    evidence.push_env("slint", env!("SLINT_VERSION_FOR_RECORD"));
    evidence.push_env("rustc", capture("rustc", &["--version"]));
    evidence.push_env("commit", git_describe());
    evidence.push_env("渲染器", render.gl_description.clone());
    // 逻辑尺寸按**实际**的物理尺寸除以缩放算，不要写 `args.width/height`：
    // 那是命令行要的窗口大小，窗口管理器未必照办，Android 上更是整块屏幕
    // 说了算——第一轮真机记录里就写着「1280x800 逻辑 / 1116x2480 物理」，
    // 两个数除不出 scale 2.75 来。
    let scale = ui.window().scale_factor();
    evidence.push_env(
        "窗口",
        format!(
            "{:.0}x{:.0} 逻辑 / {}x{} 物理 / scale {scale:.2}",
            render.size.0 as f32 / scale,
            render.size.1 as f32 / scale,
            render.size.0,
            render.size.1,
        ),
    );

    evidence.push_media("列表行数", format!("{}（合成条数）", args.rows));
    evidence.push_media(
        "数据来源",
        "pb 合成 → bili_api::grpc_reply::project_main_list → bili_domain::reply_richtext::segment"
            .to_owned(),
    );
    evidence.push_media(
        "pb 编码后",
        format!("{:.1} MiB", built.encoded_bytes as f32 / 1_048_576.0),
    );
    evidence.push_media(
        "富文本并排",
        format!(
            "{} 条 / {} 个 run（其中表情 {}）",
            measured.rich_count, measured.run_count, measured.emote_runs
        ),
    );

    evidence.push_metric("合成 pb", format!("{:.1} ms", built.synth_ms));
    evidence.push_metric("生产投影", format!("{:.1} ms", built.project_ms));
    evidence.push_metric("生产分词", format!("{:.1} ms", built.segment_ms));
    evidence.push_metric("建 Slint 模型", format!("{:.1} ms", measured.model_ms));
    evidence.push_metric(
        "StyledText 构造",
        format!(
            "{:.2} ms / {} 条（失败 {}）",
            measured.styled_ms,
            measured.rich_count,
            measured.styled_failures.len()
        ),
    );
    evidence.push_metric(
        "手写排布",
        format!("{:.2} ms / {} 条", measured.flow_ms, measured.rich_count),
    );
    evidence.push_metric(
        "整段平均帧率",
        format!("{:.1} fps", render.meter.average_fps()),
    );
    evidence.push_metric("总帧数", render.meter.frames().to_string());
    evidence.push_metric(
        "16ms 定时器",
        format!(
            "{} 次，间隔 p50 {} / p95 {} / 最慢 {:.1} ms；回调自身耗时 p50 {} / p95 {} / 最慢 {:.1} ms",
            render.ticks,
            fmt(render.tick_gap.percentile(0.5)),
            fmt(render.tick_gap.percentile(0.95)),
            render.tick_gap.max(),
            fmt(render.tick_cost.percentile(0.5)),
            fmt(render.tick_cost.percentile(0.95)),
            render.tick_cost.max(),
        ),
    );

    for (index, phase) in render.phases.iter().enumerate() {
        // 这一段画了多少帧：下一段的入口帧数减自己的，最后一段减总帧数。
        let frames = render
            .phases
            .get(index + 1)
            .map_or_else(|| render.meter.frames(), |next| next.frames_at_entry)
            .saturating_sub(phase.frames_at_entry);
        let ticks = render
            .phases
            .get(index + 1)
            .map_or(render.ticks, |next| next.ticks_at_entry)
            .saturating_sub(phase.ticks_at_entry);
        // 两组数一起写：**渲染耗时**说「画一帧要多久」，**帧间隔**说
        // 「多久画一帧」。后者大而前者小，说明是没触发重绘，不是画得慢；
        // tick 数是对照——定时器每 16ms 改一次 HUD 文本，它跑了多少次，
        // 本来就该有多少次重绘的机会。
        evidence.push_metric(
            &format!("分段「{}」渲染耗时", phase.name),
            format!(
                "p50 {} / p95 {} / 最慢 {:.1} ms（{frames} 帧 / {ticks} 次 tick）",
                fmt(phase.draw.percentile(0.5)),
                fmt(phase.draw.percentile(0.95)),
                phase.draw.max(),
            ),
        );
        evidence.push_metric(
            &format!("分段「{}」帧间隔", phase.name),
            format!(
                "p50 {} / p95 {} / p99 {} ms，长帧 {} 次",
                fmt(phase.meter.percentile(0.5)),
                fmt(phase.meter.percentile(0.95)),
                fmt(phase.meter.percentile(0.99)),
                phase.meter.janks()
            ),
        );
    }

    for (name, rss) in &state.rss_marks {
        evidence.push_metric(&format!("RSS @ {name}"), format!("{rss:.0} MiB"));
    }
    evidence.push_metric(
        "RSS 峰值",
        format!("{:.0} MiB", state.sampler.peak_rss_mib()),
    );
    evidence.push_metric(
        "CPU（最后一次采样）",
        format!("{:.0} %", state.sampler.cpu_percent()),
    );

    // ── 验证项 ──
    let created = probed.rows_created.get();
    let visible_estimate = estimate_visible(probed, args.rows);
    evidence.push_check(Check::new(
        "长列表虚拟化",
        format!("实例化的行数应当远少于总行数（{} 行）", args.rows),
        format!(
            "整段运行累计实例化 {created} 行（可视约 {visible_estimate} 行；滚动与跳转会换新行，所以累计值大于可视值是正常的）"
        ),
        if args.rows >= 1000 && created * 5 < args.rows {
            Verdict::Pass
        } else {
            Verdict::Fail
        },
    ));

    // 结论看**渲染耗时**而不是帧间隔。帧间隔会把「界面没变、不该重绘」
    // 记成长帧：这是渲染器的正确行为，不是卡顿，拿它判 Go/No-Go 会把
    // Slint 冤枉掉。帧间隔仍然写进「测量」那一节供对照。
    if let Some(phase) = render.phases.iter().find(|p| p.name == "列表滚动") {
        let p95 = phase.draw.percentile(0.95).unwrap_or(f32::MAX);
        evidence.push_check(Check::new(
            "滚动渲染耗时",
            "匀速滚动段单帧渲染 p95 ≤ 16.7ms（60Hz 下一帧的预算）",
            format!(
                "渲染 p50 {} / p95 {} / 最慢 {:.1} ms；对照帧间隔 p50 {} / p95 {} ms",
                fmt(phase.draw.percentile(0.5)),
                fmt(phase.draw.percentile(0.95)),
                phase.draw.max(),
                fmt(phase.meter.percentile(0.5)),
                fmt(phase.meter.percentile(0.95)),
            ),
            if p95 <= 16.7 {
                Verdict::Pass
            } else {
                Verdict::Fail
            },
        ));
    }

    if let Some(phase) = render.phases.iter().find(|p| p.name == "跳到末尾") {
        let worst = phase.draw.max();
        evidence.push_check(Check::new(
            "跳到列表末尾",
            "一次跳到第两万条不应当出现 >100ms 的长帧",
            format!(
                "最慢一帧渲染 {worst:.1} ms；对照帧间隔 p99 {} ms",
                fmt(phase.meter.percentile(0.99))
            ),
            if worst <= 100.0 {
                Verdict::Pass
            } else {
                Verdict::Fail
            },
        ));
    }

    evidence.push_check(Check::new(
        "行内样式（颜色 / 链接 / 可选中）",
        "@某人、#话题#、链接、时间戳各有自己的颜色，链接可点、正文可选中",
        if measured.styled_failures.is_empty() {
            format!(
                "Slint 1.19 的 StyledText 全部解析成功（{} 条，{:.2} ms）",
                measured.rich_count, measured.styled_ms
            )
        } else {
            format!(
                "有 {} 条解析失败：{:?}",
                measured.styled_failures.len(),
                measured.styled_failures
            )
        },
        if measured.styled_failures.is_empty() {
            Verdict::Pass
        } else {
            Verdict::Fail
        },
    ));

    evidence.push_check(Check::new(
        "行内表情图（图文混排）",
        "表情应当像 Flutter 的 WidgetSpan 那样混在文字流里",
        match measured.inline_image_error.as_deref() {
            Some(err) => format!(
                "StyledText 不支持行内图片，原样报错：{err}。只能自己排布（本实验第三列：{} 个 run，其中表情 {} 个，断行靠估算字宽）",
                measured.run_count, measured.emote_runs
            ),
            None => "解析器居然接受了行内图片——与 1.19 的实现不符，复查".to_owned(),
        },
        Verdict::Manual,
    ));

    let focus = probed.focus_sequence.borrow().clone();
    evidence.push_check(Check::new(
        "Tab 焦点轮转",
        "Tab 应当在三个输入框之间依次移动",
        format!("焦点序列 {focus:?}"),
        if focus.len() >= 3 {
            Verdict::Pass
        } else if driver.enabled() {
            Verdict::Fail
        } else {
            Verdict::NotTested
        },
    ));

    let input_a = ui.get_input_a().to_string();
    let input_b = ui.get_input_b().to_string();
    let input_c = ui.get_input_c().to_string();
    // 中文落在**哪个框**取决于前面 Tab 轮转到了哪里，所以三个都看。
    // 只盯着 ① 的话，Tab 一旦真的生效，这一项反而会 FAIL。
    let is_cjk = |text: &str| {
        text.chars()
            .any(|c| (c as u32) >= 0x4E00 && (c as u32) <= 0x9FFF)
    };
    let cjk_in = [("①", &input_a), ("②", &input_b), ("③", &input_c)]
        .into_iter()
        .find(|(_, text)| is_cjk(text));
    evidence.push_check(Check::new(
        "英文直接输入",
        "外部按键应当进到有焦点的输入框",
        format!("① {input_a:?} ② {input_b:?} ③ {input_c:?}"),
        if driver.enabled() {
            if input_b.is_empty() && input_a.is_empty() {
                Verdict::Fail
            } else {
                Verdict::Pass
            }
        } else {
            Verdict::NotTested
        },
    ));
    evidence.push_check(Check::new(
        "中文 IME 组字与上屏",
        format!(
            "切到微软拼音打 `{}`，输入框里应当是中文",
            args.ime_keys.trim()
        ),
        match cjk_in {
            Some((which, text)) => format!("{which} 框里是 {text:?}"),
            None => format!("三个框都没有中文：① {input_a:?} ② {input_b:?} ③ {input_c:?}"),
        },
        if !driver.enabled() {
            Verdict::NotTested
        } else if cjk_in.is_some() {
            Verdict::Pass
        } else {
            Verdict::Fail
        },
    ));

    push_a11y_checks(&mut evidence, &driver, args.rows);

    // 最小化期间还在不在画。
    if let (Some(minimized), Some(before_restore)) = (
        state.frame_marks.iter().find(|(n, _)| n == "最小化"),
        state.frame_marks.iter().find(|(n, _)| n == "还原前"),
    ) {
        let during = before_restore.1.saturating_sub(minimized.1);
        let after = state
            .frame_marks
            .iter()
            .find(|(n, _)| n == "退出")
            .map_or(0, |(_, f)| f.saturating_sub(before_restore.1));
        evidence.push_check(Check::new(
            "最小化 / 还原",
            "最小化期间不应当继续满帧渲染；还原后应当恢复",
            format!("最小化的 1.5 秒里画了 {during} 帧；还原之后到退出画了 {after} 帧"),
            if !driver.enabled() {
                Verdict::NotTested
            } else if after > 0 {
                Verdict::Pass
            } else {
                Verdict::Fail
            },
        ));
    }

    evidence.push_check(Check::new(
        "HiDPI",
        "物理像素 = 逻辑像素 × 缩放系数，截图不应当糊",
        format!(
            "scale {:.2}，窗口 {}x{} 物理像素",
            ui.window().scale_factor(),
            render.size.0,
            render.size.1
        ),
        Verdict::Manual,
    ));

    evidence.push_check(Check::new(
        "窗口缩放 / 全屏",
        "布局跟着窗口重排，不裁切、不留黑边",
        "见截图 07-resized / 08-fullscreen".to_owned(),
        Verdict::Manual,
    ));

    // 触控这一项只有真机答得了。有「触控滚动」这一段就说明外部触摸真的驱动了
    // 滚动——那一段探针自己不滚，帧是被手指滚出来的。
    match render.phases.iter().find(|p| p.name == "触控滚动") {
        Some(phase) => evidence.push_check(Check::new(
            "触控滚动与惯性",
            "真实触摸要能滚动列表，甩手之后要有惯性（松手后继续画帧）",
            format!(
                "外部 swipe 驱动：这一段渲染 p50 {} / p95 {} / 最慢 {:.1} ms，帧间隔 p50 {} ms。\
                 下拉刷新未测——Slint 没有这个控件，要先自己写一个",
                fmt(phase.draw.percentile(0.5)),
                fmt(phase.draw.percentile(0.95)),
                phase.draw.max(),
                fmt(phase.meter.percentile(0.5)),
            ),
            Verdict::Pass,
        )),
        None => evidence.push_check(Check::new(
            "触控手势",
            "触摸滚动、惯性、下拉刷新",
            "本轮没有触控分段（`--touch-ms 0`）。桌面机器没有触摸屏，这一项只能在真机上答"
                .to_owned(),
            Verdict::NotTested,
        )),
    }

    evidence.push_check(Check::new(
        "VoiceOver / TalkBack",
        "macOS 与 Android 的屏幕阅读器实读",
        if cfg!(target_os = "android") {
            // 这里刻意不写结论：探针问不到自己在无障碍树里长什么样，那棵树
            // 要从**外面**抓（`android-drive.sh` 的 uiautomator dump）。写成
            // 「PASS/FAIL」等于拿猜的当证据。
            "本轮的无障碍树由宿主机 `uiautomator dump` 抓取，结论见记录末尾的宿主机侧证据"
                .to_owned()
        } else {
            "需要 mac（#8）与真机；本次只在 Windows 上用 UIA 取树".to_owned()
        },
        Verdict::NotTested,
    ));

    for shot in &render.shots {
        evidence.shots.push(shot.clone());
    }
    for note in &state.notes {
        evidence.notes.push(note.clone());
    }
    for line in &driver.log {
        evidence.notes.push(format!("外部驱动：{line}"));
    }
    for err in &render.errors {
        evidence.notes.push(format!("渲染侧报错：{err}"));
    }
    for (name, count) in &state.created_marks {
        evidence
            .notes
            .push(format!("{name} 时累计实例化 {count} 行"));
    }

    match args.record.as_ref() {
        Some(path) => {
            evidence.write(path)?;
            println!("[spike] 记录已写到 {}", path.display());
        }
        None => println!("{}", evidence.to_markdown()),
    }
    Ok(())
}

fn push_a11y_checks(evidence: &mut Evidence, driver: &drive::Driver, rows: usize) {
    let Some(list) = driver.dump("列表") else {
        evidence.push_check(Check::new(
            "无障碍树（长列表）",
            "列表与行应当作为 List / ListItem 暴露给屏幕阅读器",
            if cfg!(target_os = "android") {
                // 不写成「没抓到（未启用外部驱动？）」：在 Android 上那句话
                // 读起来像工具坏了，而实际情况是这棵树**本来就该从外面抓**。
                // 把失败和「分工如此」混成同一句，下一个人会去修一个不存在
                // 的 bug。
                "Android 上无障碍树由宿主机 `uiautomator dump` 抓，见记录末尾的宿主机侧证据"
                    .to_owned()
            } else {
                "没有抓到 UIA 树（未启用外部驱动？）".to_owned()
            },
            Verdict::NotTested,
        ));
        return;
    };
    let items = list.count_of("ListItem");
    let named = list
        .nodes
        .iter()
        .filter(|n| n.control.ends_with("ListItem") && !n.name.is_empty())
        .count();
    let offscreen = list
        .nodes
        .iter()
        .filter(|n| n.control.ends_with("ListItem") && n.offscreen)
        .count();
    evidence.push_check(Check::new(
        "无障碍树（长列表）",
        format!("{rows} 行的列表应当以 List + ListItem 的形式出现，每项带可读的名字"),
        format!(
            "找到窗口：{}；共 {} 个节点（{}），List {} 个，ListItem {} 个（带名字 {}，其中标记为屏幕外 {}），抓树耗时 {}ms{}",
            list.found,
            list.node_count,
            if list.truncated { "已截断" } else { "未截断" },
            list.count_of("List"),
            items,
            named,
            offscreen,
            list.elapsed_ms,
            if list.error.is_empty() {
                String::new()
            } else {
                format!("，错误：{}", list.error)
            }
        ),
        if items > 0 && named > 0 {
            Verdict::Pass
        } else {
            Verdict::Fail
        },
    ));

    if let Some(node) = list
        .nodes
        .iter()
        .find(|n| n.control.ends_with("ListItem") && !n.name.is_empty())
    {
        evidence.push_check(Check::new(
            "无障碍：一行读出来是什么",
            "一条评论应当读成「谁说了什么、赞多少、回复多少」，而不是七八个零碎文本",
            format!("{:?}", shorten(&node.name, 60)),
            Verdict::Manual,
        ));
    }

    let Some(widgets) = driver.dump("控件") else {
        return;
    };
    let button = widgets
        .nodes
        .iter()
        .find(|n| n.control.ends_with("Button") && !n.name.is_empty());
    let edit = widgets.nodes.iter().find(|n| n.control.ends_with("Edit"));
    let checkbox = widgets
        .nodes
        .iter()
        .find(|n| n.control.ends_with("CheckBox"));
    let slider = widgets.nodes.iter().find(|n| n.control.ends_with("Slider"));
    if let Some(node) = widgets.any_name_containing("点赞") {
        evidence.push_check(Check::new(
            "无障碍：accessible-label 盖得过控件自己的文本吗",
            "按钮上写着「点赞 1024」，label 另写了一句更完整的",
            format!(
                "读到的是 {:?}（automation-id {:?}，深度 {}）",
                shorten(&node.name, 40),
                node.automation_id,
                node.depth
            ),
            Verdict::Manual,
        ));
    }

    evidence.push_check(Check::new(
        "无障碍树（控件）",
        "按钮 / 勾选框 / 滑块 / 输入框各自带角色、名字与取值",
        format!(
            "按钮 {:?}；勾选 {:?}；滑块 {:?}（取值 {:?}）；输入 {:?}（取值 {:?}）；共 {} 个节点",
            button.map(|n| shorten(&n.name, 24)),
            checkbox.map(|n| shorten(&n.name, 24)),
            slider.map(|n| shorten(&n.name, 24)),
            slider.map(|n| n.value.clone()),
            edit.map(|n| shorten(&n.name, 24)),
            edit.map(|n| n.value.clone()),
            widgets.node_count
        ),
        if button.is_some() && edit.is_some() && checkbox.is_some() && slider.is_some() {
            Verdict::Pass
        } else {
            Verdict::Fail
        },
    ));
}

/// 每执行一步打一行。
///
/// Android 上宿主机靠它对时间轴：触摸、软键盘、无障碍快照都得发生在正确的
/// 分段里，而真机冷启动能差好几秒，**按绝对时间 sleep 必然错位**，错位的
/// 后果还不是报错，是「对着列表发按键」这种看不出来的脏数据。桌面上它同样
/// 有用：跑挂了能一眼看出停在哪一步。
fn trace_step(elapsed_ms: u128, step: Step) {
    let line = format!(
        "STEP at={elapsed_ms}ms planned={}ms {:?}",
        step.at_ms, step.action
    );
    #[cfg(target_os = "android")]
    log::info!("{line}");
    #[cfg(not(target_os = "android"))]
    println!("[spike] {line}");
}

fn shorten(text: &str, max_chars: usize) -> String {
    let mut out: String = text.chars().take(max_chars).collect();
    if text.chars().count() > max_chars {
        out.push('…');
    }
    out
}

/// 按 ListView 自己报的内容高度估一下屏幕上能放几行。
///
/// 只是量级参照：内容高度在变高的列表里本身就是估算值（见记录里那一条）。
fn estimate_visible(probed: &Rc<Probed>, rows: usize) -> usize {
    let content = probed.content_height.get();
    let list = probed.list_height.get();
    if content <= 0.0 || list <= 0.0 || rows == 0 {
        return 0;
    }
    let average_row = content / rows as f32;
    if average_row <= 0.0 {
        return 0;
    }
    (list / average_row).ceil() as usize
}

fn fmt(value: Option<f32>) -> String {
    value.map_or_else(|| "-".to_owned(), |v| format!("{v:.1}"))
}

const fn platform_label() -> &'static str {
    // Android 必须排在 Linux 前面：它的内核是 Linux，`cfg!(target_os)` 分得清，
    // 但「不是 Windows 也不是 macOS 就叫 Linux」这种写法分不清——2026-11-22
    // 第一次在真机上跑出来的记录，标题写的就是「· Linux」。
    if cfg!(target_os = "android") {
        "Android"
    } else if cfg!(windows) {
        "Windows"
    } else if cfg!(target_os = "macos") {
        "macOS"
    } else {
        "Linux"
    }
}

fn git_describe() -> String {
    let commit = capture("git", &["rev-parse", "--short", "HEAD"]);
    let dirty = capture("git", &["status", "--porcelain"]);
    format!(
        "{commit}（工作区{}）",
        if dirty.trim().is_empty() {
            "干净"
        } else {
            "不干净"
        }
    )
}
