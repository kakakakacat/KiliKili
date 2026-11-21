//! SPIKE-001 / SPIKE-002 的实验台：Slint 窗口 + libmpv 视频 + 真实弹幕层。
//!
//! 要回答的问题、以及为什么用这种形状，见 README.md 与
//! `docs/m0/spikes/`。这里只说运行期的结构：
//!
//! ```text
//!   [mpv 事件线程] --Stamped<PlaybackEvent>--> (mpsc) --> [UI 线程 16ms tick]
//!                                                            |
//!                                        bili-danmaku 取桶 --+--> Overlay 算轨迹
//!                                                            |
//!                                                            v
//!   [Slint 渲染回调] BeforeRendering: mpv 把视频画进同一个 FBO
//!                    然后 Slint 在它上面画弹幕与 HUD
//!                    AfterRendering: 记帧耗时 / 按需 glReadPixels 截图
//! ```
//!
//! **一帧之内不跨线程**：弹幕从 Rust 的桶里取出来直接变成 Slint 的 model，
//! 没有序列化、没有 FFI。这正是 Phase 2 相对 Phase 1 的核心收益，也是这个
//! spike 要量的东西。

mod fake;
mod gl;
mod metrics;
mod mpv;
mod overlay;
mod record;
mod scenario;
mod source;

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use bili_domain::player::{
    MediaHeaders, MediaSource, PlaybackEvent, PlaybackState, PlayerCommand, PlayerEventSink,
    PlayerPort, PlayerSnapshot,
};
use bili_domain::redact::Redacted;
use bili_domain::session::{GenerationGate, SessionId, Stamped};
use clap::Parser;
use slint::{ComponentHandle, SharedString};

use metrics::{FrameMeter, ProcSampler};
use overlay::{Overlay, OverlayOptions};
use record::{Check, Evidence, Verdict};
use scenario::{Action, Step};

slint::include_modules!();

/// Slint + libmpv 视频与弹幕覆盖的实验台（M0 SPIKE-001 / SPIKE-002）。
#[derive(Parser, Debug, Clone)]
#[command(version, about, long_about = None)]
struct Args {
    /// 视频 URL 或本地文件。不给就只跑弹幕层（拿到弹幕层自己的帧率基线）。
    #[arg(long)]
    video: Option<String>,

    /// DASH 的音频轨 URL。给了就走 `audio-files` 外挂音轨，对应真实取流形态。
    #[arg(long)]
    audio: Option<String>,

    /// libmpv 的路径。不给就按平台默认名找（见 mpv::sys::default_names）。
    #[arg(long, env = "KILIKILI_MPV_LIB")]
    mpv_lib: Option<String>,

    /// mpv 的 hwdec 设置：auto-safe / no / d3d11va / videotoolbox …
    #[arg(long, default_value = "auto-safe")]
    hwdec: String,

    #[arg(long)]
    user_agent: Option<String>,

    #[arg(long)]
    referer: Option<String>,

    /// 合成弹幕密度（条/秒）。0 表示只用 fixture 自带的那几条。
    #[arg(long, default_value_t = 30.0)]
    danmaku_rate: f32,

    /// 取哪些 golden fixture 的形状。
    #[arg(long, value_delimiter = ',', default_values_t = vec![
        "typical".to_owned(), "rich".to_owned()
    ])]
    fixtures: Vec<String>,

    /// 同屏弹幕上限。
    #[arg(long, default_value_t = 200)]
    max_active: usize,

    /// 开启同内容弹幕合并（基线的 mergeDanmaku）。
    #[arg(long)]
    merge: bool,

    /// 视频上下颠倒时加这个——不同平台/驱动给的 FBO 方向不一定一样。
    #[arg(long)]
    flip_y: bool,

    /// 跑脚本化流程并自动退出（实验记录就靠它可比）。
    #[arg(long)]
    scripted: bool,

    /// 脚本里 seek 的目标位置（毫秒）。
    #[arg(long, default_value_t = 30_000)]
    seek_to: u64,

    /// 写实验记录到这个 .md。
    #[arg(long)]
    record: Option<PathBuf>,

    /// 截图目录。
    #[arg(long)]
    shots: Option<PathBuf>,

    /// mpv 自己的日志落盘位置。
    #[arg(long)]
    mpv_log: Option<PathBuf>,

    #[arg(long, default_value_t = 1280)]
    width: u32,

    #[arg(long, default_value_t = 720)]
    height: u32,

    /// 循环播放。短样片跑不满整个脚本时要用它，否则后半段测的是「播完了」。
    #[arg(long)]
    loop_file: bool,

    /// 没有视频时假装的时长（毫秒）。
    #[arg(long, default_value_t = 600_000)]
    fake_duration: u64,
}

struct ChannelSink(Sender<Stamped<PlaybackEvent>>);

impl PlayerEventSink for ChannelSink {
    fn emit(&mut self, event: Stamped<PlaybackEvent>) {
        // 送不进去说明 UI 已经退了，丢掉即可。
        let _ = self.0.send(event);
    }
}

/// 两个后端共用的调度口子。
enum Backend {
    Mpv(Rc<RefCell<mpv::MpvPlayer>>),
    Fake(RefCell<fake::FakePlayer>),
}

impl Backend {
    fn dispatch(&self, command: Stamped<PlayerCommand>) -> Result<()> {
        match self {
            Self::Mpv(player) => player.borrow_mut().dispatch(command),
            Self::Fake(player) => player.borrow_mut().dispatch(command),
        }
    }
}

/// 渲染线程（= UI 线程）上的东西。只在 Slint 的渲染回调里碰。
struct RenderState {
    gl: Option<gl::Gl>,
    render: Option<mpv::MpvRender>,
    mpv: Option<Rc<RefCell<mpv::MpvPlayer>>>,
    flip_y: bool,
    meter: FrameMeter,
    gl_description: String,
    last_fbo: (i32, i32, i32),
    /// 排队，不是单槽：两次 Shot 落在同一 tick 时后一次会把前一次顶掉。
    pending_shots: std::collections::VecDeque<PathBuf>,
    shots: Vec<PathBuf>,
    errors: Vec<String>,
}

/// 16ms tick 上的东西。
struct TickState {
    overlay: Overlay,
    source: source::DanmakuSource,
    clock: f64,
    last_tick: Instant,
    last_bucket: Option<i32>,
    snapshot: Option<PlayerSnapshot>,
    stale_dropped: u64,
    events_accepted: u64,
    seek_completed: Vec<u64>,
    failures: Vec<String>,
    position_samples: BTreeMap<&'static str, u64>,
    sampler: ProcSampler,
    script: Vec<Step>,
    next_step: usize,
    /// 渲染侧是否已经就绪（GL 上下文 + mpv render context）。**在此之前
    /// 绝不能 loadfile**：render context 还没建起来时打开文件，mpv 照样
    /// 解码、照样走进度、照样出声，但画面永远是黑的——手动模式最初就是
    /// 这么黑了一整屏，而 HUD 显示一切正常。
    ready: bool,
    started: Instant,
    fullscreen: bool,
    resizes: Vec<(u32, u32)>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let command_line: Vec<String> = std::env::args().collect();

    let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../rust/crates/bili-api/tests/fixtures/danmaku_seg");
    let danmaku = source::load(
        &fixture_dir,
        &args.fixtures,
        args.danmaku_rate,
        // 弹幕铺满整个假定时长；真实视频更短也不影响，取不到的桶就是空的。
        i32::try_from(args.fake_duration).unwrap_or(i32::MAX),
        args.merge,
    )?;

    let ui = SpikeWindow::new().context("建不了 Slint 窗口")?;
    ui.window().set_size(slint::LogicalSize::new(
        args.width as f32,
        args.height as f32,
    ));

    let gate = GenerationGate::new(SessionId(1));
    let (tx, rx): (Sender<Stamped<PlaybackEvent>>, Receiver<_>) = channel();

    // libmpv 加载失败不是致命错误：弹幕层那一半结论照样能测出来，
    // 记录里会写明这次是 no-video 模式。
    let mut mpv_info: Vec<(String, String)> = Vec::new();
    let mut load_error: Option<String> = None;
    let backend = if args.video.is_some() {
        match mpv::sys::MpvLib::load(args.mpv_lib.as_deref()) {
            Ok(lib) => {
                let lib = Arc::new(lib);
                mpv_info.push(("libmpv".to_owned(), lib.path.clone()));
                mpv_info.push(("mpv client API".to_owned(), lib.api_version()));
                let config = mpv::MpvConfig {
                    hwdec: args.hwdec.clone(),
                    user_agent: args.user_agent.clone(),
                    referer: args.referer.clone(),
                    log_file: args
                        .mpv_log
                        .as_ref()
                        .map(|p| p.to_string_lossy().into_owned()),
                    volume: 50,
                    loop_file: args.loop_file,
                };
                let player = mpv::MpvPlayer::new(
                    Arc::clone(&lib),
                    &config,
                    gate.clone(),
                    Box::new(ChannelSink(tx.clone())),
                )?;
                Backend::Mpv(Rc::new(RefCell::new(player)))
            }
            Err(err) => {
                load_error = Some(format!("{err:#}"));
                eprintln!("[spike] 加载 libmpv 失败，退回 no-video 模式：\n{err:#}");
                Backend::Fake(RefCell::new(fake::FakePlayer::new(
                    gate.clone(),
                    args.fake_duration,
                    Box::new(ChannelSink(tx.clone())),
                )?))
            }
        }
    } else {
        Backend::Fake(RefCell::new(fake::FakePlayer::new(
            gate.clone(),
            args.fake_duration,
            Box::new(ChannelSink(tx.clone())),
        )?))
    };
    let backend = Rc::new(backend);
    let has_video = matches!(*backend, Backend::Mpv(_));
    ui.set_video_active(has_video);

    let render_state = Rc::new(RefCell::new(RenderState {
        gl: None,
        render: None,
        mpv: match &*backend {
            Backend::Mpv(player) => Some(Rc::clone(player)),
            Backend::Fake(_) => None,
        },
        flip_y: args.flip_y,
        meter: FrameMeter::new(),
        gl_description: "<未取>".to_owned(),
        last_fbo: (0, 0, 0),
        pending_shots: std::collections::VecDeque::new(),
        shots: Vec::new(),
        errors: Vec::new(),
    }));

    let tick_state = Rc::new(RefCell::new(TickState {
        overlay: Overlay::new(
            OverlayOptions {
                max_active: args.max_active,
                ..OverlayOptions::default()
            },
            args.width as f32,
            args.height as f32,
        ),
        source: danmaku,
        clock: 0.0,
        last_tick: Instant::now(),
        last_bucket: None,
        snapshot: None,
        stale_dropped: 0,
        events_accepted: 0,
        seek_completed: Vec::new(),
        failures: Vec::new(),
        position_samples: BTreeMap::new(),
        sampler: ProcSampler::new(),
        script: if args.scripted {
            scenario::default_script(args.seek_to)
        } else {
            Vec::new()
        },
        next_step: 0,
        ready: false,
        started: Instant::now(),
        fullscreen: false,
        resizes: Vec::new(),
    }));

    install_rendering_notifier(&ui, &render_state)?;
    install_keys(&ui, &backend, &gate, &render_state, &args);

    let model = Rc::new(slint::VecModel::<DanmakuItem>::default());
    ui.set_items(model.clone().into());
    ui.set_watermark(SharedString::from(if has_video {
        "KiliKili SPIKE-001 · 弹幕取自 golden fixture（密度为合成）"
    } else {
        "KiliKili SPIKE-001 · no-video 模式：只测弹幕层"
    }));

    let timer = slint::Timer::default();
    {
        let ui_weak = ui.as_weak();
        let tick_state = Rc::clone(&tick_state);
        let render_state = Rc::clone(&render_state);
        let backend = Rc::clone(&backend);
        let gate = gate.clone();
        let args_for_tick = args.clone();
        timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(16),
            move || {
                let Some(ui) = ui_weak.upgrade() else { return };
                if let Err(err) = tick(
                    &ui,
                    &model,
                    &tick_state,
                    &render_state,
                    &backend,
                    &gate,
                    &rx,
                    &args_for_tick,
                ) {
                    eprintln!("[spike] tick 出错：{err:#}");
                }
            },
        );
    }

    ui.run().context("Slint 事件循环退出异常")?;

    write_evidence(
        &args,
        &command_line.join(" "),
        &tick_state,
        &render_state,
        &mpv_info,
        load_error.as_deref(),
        has_video,
        &backend,
    )
}

fn open_and_play(backend: &Backend, gate: &GenerationGate, args: &Args) -> Result<()> {
    let generation = gate.bump();
    let source = match (&args.video, &args.audio) {
        (Some(video), Some(audio)) => MediaSource::Dash {
            video: Redacted::new(video.clone()),
            audio: Redacted::new(audio.clone()),
        },
        (Some(video), None) => MediaSource::Segments(vec![Redacted::new(video.clone())]),
        // no-video 模式下也要走一遍 Open，否则两个后端的状态机不一样，
        // 后面的对照就没有意义。
        (None, _) => MediaSource::Segments(vec![Redacted::new(String::new())]),
    };
    let headers = MediaHeaders {
        user_agent: args.user_agent.clone().unwrap_or_default(),
        referer: args.referer.clone().unwrap_or_default(),
    };
    backend.dispatch(Stamped::new(
        gate.session(),
        generation,
        PlayerCommand::Open { source, headers },
    ))?;
    backend.dispatch(Stamped::new(
        gate.session(),
        gate.current(),
        PlayerCommand::Play,
    ))
}

fn install_rendering_notifier(
    ui: &SpikeWindow,
    render_state: &Rc<RefCell<RenderState>>,
) -> Result<()> {
    let render_state = Rc::clone(render_state);
    ui.window()
        .set_rendering_notifier(move |state, api| {
            let slint::GraphicsAPI::NativeOpenGL { get_proc_address } = api else {
                let mut shared = render_state.borrow_mut();
                shared
                    .errors
                    .push("当前渲染后端不是 OpenGL，mpv 的 render API 用不了".to_owned());
                return;
            };
            let loader = |name: &std::ffi::CStr| get_proc_address(name);
            let mut shared = render_state.borrow_mut();

            match state {
                slint::RenderingState::RenderingSetup => {
                    match unsafe { gl::Gl::load(&loader) } {
                        Ok(gl) => {
                            shared.gl_description = gl.describe();
                            shared.gl = Some(gl);
                        }
                        Err(err) => shared.errors.push(format!("GL 函数加载失败：{err:#}")),
                    }
                    // mpv 的 render context 要在 GL 上下文已经建好之后再建。
                    if let Some(player) = shared.mpv.clone() {
                        let created = {
                            let player = player.borrow();
                            unsafe { player.create_render(&loader) }
                        };
                        match created {
                            Ok(render) => shared.render = Some(render),
                            Err(err) => shared
                                .errors
                                .push(format!("mpv render context 建不起来：{err:#}")),
                        }
                    }
                }
                slint::RenderingState::BeforeRendering => {
                    // 顺序就是层级：mpv 先把视频画进这个 FBO，Slint 接着在
                    // 同一个 FBO 上画弹幕和 HUD。
                    let Some(gl) = shared.gl.as_ref() else { return };
                    let fbo = gl.draw_framebuffer();
                    let (width, height) = shared.last_fbo_size();
                    if width > 0 && height > 0 {
                        shared.last_fbo = (fbo, width, height);
                        let flip = shared.flip_y;
                        if let Some(render) = shared.render.as_ref()
                            && let Err(err) =
                                unsafe { render.render(fbo, width, height, flip, &loader) }
                        {
                            let message = format!("mpv 渲染失败：{err:#}");
                            if !shared.errors.contains(&message) {
                                shared.errors.push(message);
                            }
                        }
                    }
                }
                slint::RenderingState::AfterRendering => {
                    shared.meter.tick(Instant::now());
                    if let Some(path) = shared.pending_shots.pop_front() {
                        let (_, width, height) = shared.last_fbo;
                        if let Some(gl) = shared.gl.as_ref() {
                            match gl.capture_png(width as u32, height as u32, &path) {
                                Ok(()) => shared.shots.push(path),
                                Err(err) => {
                                    shared.errors.push(format!("截图失败：{err:#}"));
                                }
                            }
                        }
                        // 截图是同步读回 + 压缩，整个循环被它堵住；这一下
                        // 不算进帧耗时，否则报告的 p99 量的是 PNG 压缩速度。
                        shared.meter.skip_next_interval();
                    }
                }
                slint::RenderingState::RenderingTeardown => {
                    // render context 必须在 GL 上下文还在时销毁。
                    shared.render = None;
                    shared.gl = None;
                }
                _ => {}
            }
        })
        .map_err(|err| anyhow::anyhow!("装不上渲染回调（需要 OpenGL 后端）：{err}"))
}

impl RenderState {
    /// 渲染侧是否可以开播了。没有 mpv（no-video 模式）时永远可以。
    fn is_ready(&self) -> bool {
        self.gl.is_some() && (self.mpv.is_none() || self.render.is_some())
    }

    /// 当前 framebuffer 的像素尺寸。Slint 没有直接给，用窗口物理尺寸。
    fn last_fbo_size(&self) -> (i32, i32) {
        (self.last_fbo.1, self.last_fbo.2)
    }
}

fn install_keys(
    ui: &SpikeWindow,
    backend: &Rc<Backend>,
    gate: &GenerationGate,
    render_state: &Rc<RefCell<RenderState>>,
    args: &Args,
) {
    let backend = Rc::clone(backend);
    let gate = gate.clone();
    let render_state = Rc::clone(render_state);
    let shots_dir = args.shots.clone();
    let ui_weak = ui.as_weak();
    let mut paused = false;
    let mut fullscreen = false;
    let mut shot_index = 0;
    ui.on_key(move |text| {
        let Some(ui) = ui_weak.upgrade() else { return };
        let Some(key) = text.chars().next() else {
            return;
        };
        let session = gate.session();
        let result = match key {
            ' ' => {
                paused = !paused;
                backend.dispatch(Stamped::new(
                    session,
                    gate.current(),
                    if paused {
                        PlayerCommand::Pause
                    } else {
                        PlayerCommand::Play
                    },
                ))
            }
            'f' | 'F' => {
                fullscreen = !fullscreen;
                ui.window().set_fullscreen(fullscreen);
                Ok(())
            }
            's' | 'S' => {
                let dir = shots_dir
                    .clone()
                    .unwrap_or_else(|| PathBuf::from("spike-shots"));
                shot_index += 1;
                render_state
                    .borrow_mut()
                    .pending_shots
                    .push_back(dir.join(format!("manual-{shot_index:02}.png")));
                Ok(())
            }
            'q' | 'Q' => {
                let _ = slint::quit_event_loop();
                Ok(())
            }
            // 方向键：Slint 把它们当成特定的私有区字符传过来。
            c if c == char::from(slint::platform::Key::LeftArrow) => {
                seek_relative(&backend, &gate, -5_000)
            }
            c if c == char::from(slint::platform::Key::RightArrow) => {
                seek_relative(&backend, &gate, 5_000)
            }
            _ => Ok(()),
        };
        if let Err(err) = result {
            eprintln!("[spike] 按键处理出错：{err:#}");
        }
    });
}

thread_local! {
    /// 相对 seek 要知道当前位置；按键回调拿不到 tick 的状态，
    /// 于是每 tick 存一份最后位置在这里。只有 UI 线程碰它。
    static LAST_POSITION: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

fn seek_relative(backend: &Backend, gate: &GenerationGate, delta_ms: i64) -> Result<()> {
    let current = LAST_POSITION.with(std::cell::Cell::get) as i64;
    let target = (current + delta_ms).max(0) as u64;
    let generation = gate.bump();
    backend.dispatch(Stamped::new(
        gate.session(),
        generation,
        PlayerCommand::Seek {
            position_ms: target,
        },
    ))
}

#[allow(clippy::too_many_arguments)]
fn tick(
    ui: &SpikeWindow,
    model: &Rc<slint::VecModel<DanmakuItem>>,
    tick_state: &Rc<RefCell<TickState>>,
    render_state: &Rc<RefCell<RenderState>>,
    backend: &Rc<Backend>,
    gate: &GenerationGate,
    rx: &Receiver<Stamped<PlaybackEvent>>,
    args: &Args,
) -> Result<()> {
    let mut state = tick_state.borrow_mut();

    // 0. 等渲染侧就绪再开播，并把脚本的零点对齐到这一刻——窗口创建与
    //    GL 初始化在不同机器上要花多久并不固定，按进程启动计时会让各机器
    //    的记录对不上。
    if !state.ready {
        if !render_state.borrow().is_ready() {
            return Ok(());
        }
        state.ready = true;
        state.started = Instant::now();
        state.last_tick = Instant::now();
        if !args.scripted {
            open_and_play(backend, gate, args)?;
        }
    }

    // 1. 收事件。过期的（代际对不上）在这里被丢掉并计数——这正是
    //    ADR-003 要的行为，计数本身就是证据。
    while let Ok(stamped) = rx.try_recv() {
        match stamped.accept(gate) {
            Some(event) => {
                state.events_accepted += 1;
                match event {
                    PlaybackEvent::Snapshot(snapshot) => state.snapshot = Some(snapshot),
                    PlaybackEvent::SeekCompleted { position_ms } => {
                        state.seek_completed.push(position_ms);
                        // seek 完成：清屏并重新对齐桶，否则跳转前的弹幕会
                        // 继续飘（ADR-003 验证项 1）。
                        state.overlay.reset();
                        state.last_bucket = None;
                    }
                    PlaybackEvent::Failed(kind) => {
                        state.failures.push(format!("{kind:?}"));
                    }
                    PlaybackEvent::SurfaceLost => {
                        state.failures.push("SurfaceLost".to_owned());
                    }
                    PlaybackEvent::Closed => {}
                }
            }
            None => state.stale_dropped += 1,
        }
    }

    // 2. 推进 overlay 的时钟。只有「后端说在播」才推进——暂停时整屏静止。
    let now = Instant::now();
    let delta = now.duration_since(state.last_tick).as_secs_f64();
    state.last_tick = now;
    let snapshot = state.snapshot;
    let playing = snapshot.is_some_and(|s| s.state == PlaybackState::Playing);
    if playing {
        let speed = f64::from(snapshot.map_or(1.0, |s| s.speed)).max(0.1);
        state.clock += delta * speed;
    }
    let position_ms = snapshot.map_or(0, |s| s.position_ms);
    LAST_POSITION.with(|cell| cell.set(position_ms));

    // 3. 取桶。ADR-010 的形状：Rust 持有分桶结果，渲染侧每 100ms 取一桶。
    let bucket_now = (position_ms / 100) as i32;
    let clock = state.clock;
    let from = match state.last_bucket {
        // 一次最多补 20 个桶（2 秒）。补太多说明刚 seek 或刚卡过，
        // 把它们一股脑倒上屏只会糊成一片。
        Some(last) if bucket_now > last => (last + 1).max(bucket_now - 20),
        // 位置往回跳而又没有 SeekCompleted：循环播放到头了。当成新的一轮，
        // 否则 last_bucket 会永远卡在上一轮的末尾，一条弹幕都喂不出来。
        Some(last) if bucket_now < last => {
            state.overlay.reset();
            bucket_now
        }
        Some(_) => bucket_now + 1,
        None => bucket_now,
    };
    // 先取完再喂：`session` 与 `overlay` 都挂在同一个 RefMut 上，
    // 借用检查器拆不开，中间克隆一次。
    let mut pending = Vec::new();
    for bucket in from..=bucket_now {
        if let Some(elems) = state.source.session.bucket(bucket * 100) {
            pending.push(elems.to_vec());
        }
    }
    for elems in pending {
        state.overlay.feed(clock, &elems);
    }
    state.last_bucket = Some(bucket_now);

    // 4. 交给 Slint 画。
    let size = ui.window().size();
    let scale = ui.window().scale_factor();
    let logical = (size.width as f32 / scale, size.height as f32 / scale);
    state.overlay.resize(logical.0, logical.1);
    let items: Vec<DanmakuItem> = state
        .overlay
        .advance(clock)
        .iter()
        .map(|item| DanmakuItem {
            text: SharedString::from(item.text.as_str()),
            x: item.x,
            y: item.y,
            size: item.font_px,
            color: slint::Color::from_rgb_u8(
                ((item.color >> 16) & 0xFF) as u8,
                ((item.color >> 8) & 0xFF) as u8,
                (item.color & 0xFF) as u8,
            ),
            alpha: item.alpha,
        })
        .collect();
    model.set_vec(items);

    // 5. HUD。
    state.sampler.sample();
    let stats = state.overlay.stats();
    let hud = {
        let render = render_state.borrow();
        format!(
            "fps {:>5.1} (avg {:>5.1})  帧耗时 p50 {:>5.2} p95 {:>5.2} p99 {:>5.2} ms\n\
             掉帧(>33ms) {:<5} 总帧 {:<6}\n\
             位置 {:>7.2}s / {:<7}  状态 {:?}\n\
             弹幕 同屏 {:<4} 累计 {:<6} 丢(上限) {:<5} 丢(无轨) {:<5} 跳过(高级) {}\n\
             CPU {:>5.1}%  RSS {:>6.1} MiB (峰值 {:.1})\n\
             FBO {}x{}  缩放 {:.2}  渲染器 {}",
            render.meter.recent_fps().unwrap_or(0.0),
            render.meter.average_fps(),
            render.meter.percentile(0.5).unwrap_or(0.0),
            render.meter.percentile(0.95).unwrap_or(0.0),
            render.meter.percentile(0.99).unwrap_or(0.0),
            render.meter.janks(),
            render.meter.frames(),
            position_ms as f64 / 1000.0,
            snapshot
                .and_then(|s| s.duration_ms)
                .map_or_else(|| "?".to_owned(), |d| format!("{:.1}s", d as f64 / 1000.0)),
            snapshot.map_or(PlaybackState::Idle, |s| s.state),
            stats.active,
            stats.spawned,
            stats.dropped_cap,
            stats.dropped_lane,
            stats.skipped_advanced,
            state.sampler.cpu_percent(),
            state.sampler.rss_mib(),
            state.sampler.peak_rss_mib(),
            render.last_fbo.1,
            render.last_fbo.2,
            scale,
            render.gl_description,
        )
    };
    ui.set_hud(SharedString::from(hud));

    // 6. 记录 FBO 尺寸给渲染回调用（它那边拿不到窗口）。
    {
        let mut render = render_state.borrow_mut();
        render.last_fbo = (render.last_fbo.0, size.width as i32, size.height as i32);
    }

    // 7. 脚本。
    run_script(ui, &mut state, render_state, backend, gate, args)?;
    ui.window().request_redraw();
    Ok(())
}

fn run_script(
    ui: &SpikeWindow,
    state: &mut TickState,
    render_state: &Rc<RefCell<RenderState>>,
    backend: &Rc<Backend>,
    gate: &GenerationGate,
    args: &Args,
) -> Result<()> {
    if state.script.is_empty() {
        return Ok(());
    }
    let elapsed = state.started.elapsed().as_millis() as u64;
    while state.next_step < state.script.len() && state.script[state.next_step].at_ms <= elapsed {
        let step = state.script[state.next_step];
        state.next_step += 1;
        let session = gate.session();
        match step.action {
            Action::Open => open_and_play(backend, gate, args)?,
            Action::Play => {
                backend.dispatch(Stamped::new(session, gate.current(), PlayerCommand::Play))?
            }
            Action::Pause => {
                backend.dispatch(Stamped::new(session, gate.current(), PlayerCommand::Pause))?
            }
            Action::Seek(position_ms) => {
                let generation = gate.bump();
                backend.dispatch(Stamped::new(
                    session,
                    generation,
                    PlayerCommand::Seek { position_ms },
                ))?;
            }
            Action::Fullscreen(on) => {
                state.fullscreen = on;
                ui.window().set_fullscreen(on);
            }
            Action::Resize(width, height) => {
                ui.window()
                    .set_size(slint::LogicalSize::new(width as f32, height as f32));
                state.resizes.push((width, height));
            }
            Action::Shot(name) => {
                let dir = args
                    .shots
                    .clone()
                    .unwrap_or_else(|| PathBuf::from("spike-shots"));
                render_state
                    .borrow_mut()
                    .pending_shots
                    .push_back(dir.join(format!("{name}.png")));
            }
            Action::SamplePosition(name) => {
                let position = state.snapshot.map_or(0, |s| s.position_ms);
                state.position_samples.insert(name, position);
            }
            Action::Quit => {
                let _ = slint::quit_event_loop();
            }
        }
        println!(
            "[spike] {:>6}ms  {:?}  ——  {}",
            elapsed, step.action, step.note
        );
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn write_evidence(
    args: &Args,
    command_line: &str,
    tick_state: &Rc<RefCell<TickState>>,
    render_state: &Rc<RefCell<RenderState>>,
    mpv_info: &[(String, String)],
    load_error: Option<&str>,
    has_video: bool,
    backend: &Rc<Backend>,
) -> Result<()> {
    let state = tick_state.borrow();
    let render = render_state.borrow();
    let mut evidence = Evidence {
        title: format!(
            "SPIKE-001 实验记录 · {} · {}",
            std::env::consts::OS,
            record::utc_now()
        ),
        command_line: command_line.to_owned(),
        ..Evidence::default()
    };

    let system = sysinfo::System::new_all().ok();
    evidence.push_env("平台", std::env::consts::OS);
    evidence.push_env(
        "系统",
        format!(
            "{} {}",
            sysinfo::System::name().unwrap_or_default(),
            sysinfo::System::os_version().unwrap_or_default()
        ),
    );
    evidence.push_env(
        "内核",
        sysinfo::System::kernel_version().unwrap_or_default(),
    );
    evidence.push_env(
        "CPU",
        system.as_ref().map_or_else(
            || "<取不到>".to_owned(),
            |system| {
                system
                    .cpus()
                    .first()
                    .map_or_else(String::new, |cpu| cpu.brand().trim().to_owned())
            },
        ),
    );
    evidence.push_env(
        "内存",
        system.as_ref().map_or_else(
            || "<取不到>".to_owned(),
            |system| {
                format!(
                    "{:.1} GiB",
                    system.total_memory() as f64 / 1024.0_f64.powi(3)
                )
            },
        ),
    );
    evidence.push_env("rustc", record::capture("rustc", &["-V"]));
    evidence.push_env(
        "commit",
        record::capture("git", &["rev-parse", "--short", "HEAD"]),
    );
    evidence.push_env(
        "工作区是否干净",
        if record::capture("git", &["status", "--porcelain"]).is_empty() {
            "是"
        } else {
            "否（记录里的 commit 不足以复现，见备注）"
        },
    );
    evidence.push_env("slint", env!("SLINT_VERSION_FOR_RECORD"));
    for (key, value) in mpv_info {
        evidence.push_env(key, value.clone());
    }

    evidence.push_media(
        "模式",
        if has_video {
            "视频 + 弹幕"
        } else {
            "仅弹幕（no-video）"
        },
    );
    evidence.push_media(
        "视频源",
        args.video.clone().unwrap_or_else(|| "<无>".to_owned()),
    );
    evidence.push_media(
        "音频源",
        args.audio
            .clone()
            .map_or_else(|| "<无，单轨>".to_owned(), |_| "DASH 外挂音轨".to_owned()),
    );
    evidence.push_media("hwdec 设置", args.hwdec.clone());
    evidence.push_media("OpenGL", render.gl_description.clone());
    evidence.push_media(
        "FBO 尺寸",
        format!("{}x{}", render.last_fbo.1, render.last_fbo.2),
    );
    evidence.push_media(
        "弹幕",
        format!(
            "形状取自 {} · 密度 {} 条/秒（{}）· 入桶 {} 条",
            state.source.shapes_from.join(" + "),
            args.danmaku_rate,
            if state.source.synthetic {
                "合成"
            } else {
                "fixture 原样"
            },
            state.source.total
        ),
    );

    let mut audio_codec_name: Option<String> = None;
    if let Backend::Mpv(player) = &**backend {
        let player = player.borrow();
        for (key, prop) in [
            ("hwdec 实际", "hwdec-current"),
            ("视频编码", "video-codec"),
            ("视频格式", "video-format"),
        ] {
            if let Some(value) = player.property_string(prop) {
                evidence.push_media(key, value);
            }
        }
        // 音轨**一定要出现在记录里**，哪怕是「没有」：第一次 Windows 实跑
        // 时样片本身没有音轨，而「音频源」那行照着命令行写着「DASH 外挂
        // 音轨」，看记录的人会以为分轨验过了。没验就得写没验。
        let audio_codec = player.property_string("audio-codec-name");
        evidence.push_media(
            "音频编码",
            audio_codec.clone().unwrap_or_else(|| "<无音轨>".to_owned()),
        );
        audio_codec_name = audio_codec;
        if let (Some(width), Some(height)) = (
            player.property_i64("video-params/w"),
            player.property_i64("video-params/h"),
        ) {
            evidence.push_media("视频分辨率", format!("{width}x{height}"));
        }
        if let Some(dropped) = player.property_i64("frame-drop-count") {
            evidence.push_metric("mpv 丢帧", dropped.to_string());
        }
        if let Some(fps) = player.property_f64("estimated-vf-fps") {
            evidence.push_metric("视频帧率", format!("{fps:.2}"));
        }
    }

    let meter = &render.meter;
    evidence.push_metric("运行时长", format!("{:.1}s", meter.elapsed().as_secs_f32()));
    evidence.push_metric("总帧数", meter.frames().to_string());
    evidence.push_metric("平均 fps", format!("{:.1}", meter.average_fps()));
    evidence.push_metric(
        "帧耗时 p50 / p95 / p99",
        format!(
            "{:.2} / {:.2} / {:.2} ms",
            meter.percentile(0.5).unwrap_or(0.0),
            meter.percentile(0.95).unwrap_or(0.0),
            meter.percentile(0.99).unwrap_or(0.0)
        ),
    );
    evidence.push_metric("掉帧（>33ms）", meter.janks().to_string());
    evidence.push_metric(
        "CPU / RSS（结束时）",
        format!(
            "{:.1}% / {:.1} MiB（峰值 {:.1} MiB）",
            state.sampler.cpu_percent(),
            state.sampler.rss_mib(),
            state.sampler.peak_rss_mib()
        ),
    );
    let stats = state.overlay.stats();
    evidence.push_metric(
        "弹幕 累计 / 丢弃",
        format!(
            "{} 条上屏，{} 条因同屏上限丢弃，{} 条因无空轨丢弃，{} 条高级弹幕跳过",
            stats.spawned, stats.dropped_cap, stats.dropped_lane, stats.skipped_advanced
        ),
    );
    evidence.push_metric(
        "事件",
        format!(
            "采纳 {}，因代际过期丢弃 {}",
            state.events_accepted, state.stale_dropped
        ),
    );

    // —— 验证项 ——
    let fps = meter.average_fps();
    evidence.push_check(Check::new(
        "帧率",
        "平均 ≥ 55 fps",
        format!("{fps:.1} fps"),
        if meter.frames() < 60 {
            Verdict::NotTested
        } else if fps >= 55.0 {
            Verdict::Pass
        } else {
            Verdict::Fail
        },
    ));

    let p95 = meter.percentile(0.95).unwrap_or(f32::MAX);
    evidence.push_check(Check::new(
        "帧耗时 p95",
        "≤ 20ms",
        format!("{p95:.2} ms"),
        if meter.frames() < 60 {
            Verdict::NotTested
        } else if p95 <= 20.0 {
            Verdict::Pass
        } else {
            Verdict::Fail
        },
    ));

    let (pause_a, pause_b) = (
        state.position_samples.get("pause-a").copied(),
        state.position_samples.get("pause-b").copied(),
    );
    evidence.push_check(match (pause_a, pause_b) {
        (Some(a), Some(b)) => Check::new(
            "暂停冻结位置",
            "暂停 1.4 秒后位置不变",
            format!("{a}ms → {b}ms"),
            if a == b { Verdict::Pass } else { Verdict::Fail },
        ),
        _ => Check::new(
            "暂停冻结位置",
            "暂停 1.4 秒后位置不变",
            "未采样",
            Verdict::NotTested,
        ),
    });

    evidence.push_check(if args.scripted {
        match state.seek_completed.last() {
            Some(position) => {
                let delta = position.abs_diff(args.seek_to);
                Check::new(
                    "Seek 完成是事实事件",
                    format!("收到 SeekCompleted 且位置接近 {}ms", args.seek_to),
                    format!(
                        "收到 {} 次，最后一次 {position}ms（差 {delta}ms）",
                        state.seek_completed.len()
                    ),
                    if delta <= 2_000 {
                        Verdict::Pass
                    } else {
                        Verdict::Fail
                    },
                )
            }
            None => Check::new(
                "Seek 完成是事实事件",
                "收到 SeekCompleted",
                "一次都没收到",
                Verdict::Fail,
            ),
        }
    } else {
        Check::new(
            "Seek 完成是事实事件",
            "收到 SeekCompleted",
            "手动模式未脚本化",
            Verdict::NotTested,
        )
    });

    evidence.push_check(Check::new(
        "旧代际事件被丢弃",
        "seek 期间旧快照不被采纳",
        format!("丢弃 {} 条", state.stale_dropped),
        if !args.scripted {
            Verdict::NotTested
        } else if state.stale_dropped > 0 {
            Verdict::Pass
        } else {
            // 没丢弃不一定是坏事（可能刚好没有在途事件），所以不判 FAIL。
            Verdict::Manual
        },
    ));

    evidence.push_check(if has_video {
        let hwdec = match &**backend {
            Backend::Mpv(player) => player
                .borrow()
                .property_string("hwdec-current")
                .unwrap_or_else(|| "<取不到>".to_owned()),
            Backend::Fake(_) => "<无>".to_owned(),
        };
        let ok = !matches!(hwdec.as_str(), "no" | "" | "<取不到>");
        Check::new(
            "硬件解码",
            "hwdec-current 不是 no",
            hwdec,
            if ok { Verdict::Pass } else { Verdict::Fail },
        )
    } else {
        Check::new(
            "硬件解码",
            "hwdec-current 不是 no",
            "no-video 模式",
            Verdict::Blocked,
        )
    });

    evidence.push_check(match (&args.audio, &audio_codec_name) {
        (Some(_), Some(codec)) => Check::new(
            "DASH 音视频分轨",
            "音频来自第二个源，且解得出编码",
            format!("{codec}（视频与音频各走一个 demuxer）"),
            Verdict::Pass,
        ),
        (Some(_), None) => Check::new(
            "DASH 音视频分轨",
            "音频来自第二个源，且解得出编码",
            "给了音频源但没有音轨——样片本身没有声音，这项没验到",
            Verdict::Blocked,
        ),
        (None, _) => Check::new(
            "DASH 音视频分轨",
            "音频来自第二个源",
            "本次是单源播放",
            Verdict::NotTested,
        ),
    });

    evidence.push_check(Check::new(
        "视频与弹幕的层级",
        "视频在下、弹幕在上、HUD 在最上",
        format!("见截图 {} 张", render.shots.len()),
        if render.shots.is_empty() {
            Verdict::NotTested
        } else {
            Verdict::Manual
        },
    ));

    evidence.push_check(Check::new(
        "HiDPI",
        "FBO 像素尺寸 = 逻辑尺寸 × 缩放",
        format!("{}x{}", render.last_fbo.1, render.last_fbo.2),
        Verdict::Manual,
    ));

    evidence.push_check(Check::new(
        "全屏",
        "能进能出且不崩",
        if args.scripted {
            "脚本切换过两次"
        } else {
            "未脚本化"
        },
        if args.scripted {
            Verdict::Manual
        } else {
            Verdict::NotTested
        },
    ));

    evidence.push_check(Check::new(
        "窗口缩放",
        "缩放后视频与弹幕都跟着变",
        format!("{:?}", state.resizes),
        if state.resizes.is_empty() {
            Verdict::NotTested
        } else {
            Verdict::Manual
        },
    ));

    evidence.push_check(Check::new(
        "播放失败事件",
        "没有 Failed / SurfaceLost",
        if state.failures.is_empty() {
            "无".to_owned()
        } else {
            state.failures.join(", ")
        },
        if state.failures.is_empty() {
            Verdict::Pass
        } else {
            Verdict::Fail
        },
    ));

    evidence.shots = render.shots.clone();
    for error in &render.errors {
        evidence.notes.push(format!("渲染侧错误：{error}"));
    }
    if let Some(err) = load_error {
        evidence
            .notes
            .push(format!("libmpv 没加载上，本次为 no-video 模式：{err}"));
    }
    evidence.notes.push(
        "弹幕密度是合成的（形状取自 golden fixture），不代表线上真实弹幕量；\
         高级弹幕（mode 7/9）未渲染。"
            .to_owned(),
    );

    let markdown = evidence.to_markdown();
    if let Some(path) = &args.record {
        evidence.write(path)?;
        println!("[spike] 实验记录已写入 {}", path.display());
    } else {
        println!("\n{markdown}");
    }
    Ok(())
}
