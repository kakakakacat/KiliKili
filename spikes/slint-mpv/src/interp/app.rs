//! #218 插帧 spike 的应用层：参数、播放循环、实验室模式、记录。
//!
//! 桌面入口是 `src/bin/interp.rs`，Android 入口是 `src/interp_android.rs`
//! （cargo feature `interp`）；两边都只负责拿到 [`Args`] 再调 [`run`]。
//!
//! 两种用法：
//!
//! - **播放**（`--video`）：Slint 窗口 + libmpv。mpv 把每张新源帧画进我们自己的
//!   纹理环，按 `target_time` 排程，按 `--mode` 输出（直画 / 不插帧 / 混合 /
//!   运动补偿）。跑完写一份记录：源帧节奏、前瞻余量、选帧结果、GPU 分项耗时。
//! - **实验室**（`--lab <目录>`）：不开 mpv，读一串 PNG（60fps 的真值序列），
//!   用第 k-1、k+1 帧插出第 k 帧，写出 PNG，交给 ffmpeg 与真值比 PSNR / SSIM。
//!   画质靠它定量，不靠人眼。
//!
//! 与 `src/main.rs`（SPIKE-001）共用 `mpv` 模块与 Slint 界面，不共用弹幕层：
//! 这里只量视频这一层。

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use bili_domain::player::{
    MediaHeaders, MediaSource, PlaybackEvent, PlayerCommand, PlayerEventSink, PlayerPort,
};
use bili_domain::redact::Redacted;
use bili_domain::session::{GenerationGate, SessionId, Stamped};
use clap::Parser;
use slint::ComponentHandle;

use player_interp::{Event, Interpolator, Plan, Presented, Setting, Thermal};

use super::sched::{Pick, Push, Ring};
use super::{Gpu, Mode, Params, Stage};
use crate::mpv;

slint::include_modules!();

#[derive(Parser, Debug, Clone)]
#[command(about = "#218 插帧 spike：mpv 源帧 → 纹理环 → 运动补偿 → Slint")]
pub struct Args {
    /// 视频文件或 URL。
    #[arg(long)]
    video: Option<String>,

    /// 外挂音轨（DASH）。
    #[arg(long)]
    audio: Option<String>,

    #[arg(long, env = "KILIKILI_MPV_LIB")]
    mpv_lib: Option<String>,

    #[arg(long, default_value = "auto-safe")]
    hwdec: String,

    #[arg(long, value_enum, default_value = "mc")]
    mode: CliMode,

    /// mpv 的 `video-sync`。`audio` 是默认值；`display-resample` 配合
    /// `--mpv-interp` 是 mpv 自带插帧（帧混合）的对照。
    #[arg(long, default_value = "audio")]
    video_sync: String,

    /// 打开 mpv 自带的 `interpolation`（只在 `--mode direct` 下有意义）。
    #[arg(long)]
    mpv_interp: bool,

    /// mpv 的 `video-timing-offset`（秒）：新帧最多提前多久交给我们。
    /// 这就是前瞻余量的来源，插帧要它至少覆盖一个源帧间隔。
    #[arg(long, default_value_t = 0.05)]
    timing_offset: f64,

    /// 输出时刻相对「现在」再往后推多少个显示周期（这一帧真正上屏的时刻）。
    #[arg(long, default_value_t = 1.0)]
    present_frames: f64,

    /// 前瞻不够时的退路：输出整体推迟这么多毫秒，同时用 `audio-delay`
    /// 把音频推迟同样多。
    #[arg(long, default_value_t = 0.0)]
    delay_ms: f64,

    /// 跳过已判定的重复帧（一拍二 / 一拍三）。
    #[arg(long)]
    skip_dups: bool,

    /// 环的槽数。
    #[arg(long, default_value_t = 8)]
    ring: usize,

    #[arg(long, default_value_t = Params::default().radius)]
    radius: i32,

    #[arg(long, default_value_t = Params::default().lambda)]
    lambda: f32,

    /// SAD 取样步长（1 = 全部 64 点，2 = 棋盘格 16 点）。
    #[arg(long, default_value_t = Params::default().step)]
    step: i32,

    /// 亚像素细化层数：0 关，1 半像素，2 四分之一像素。
    #[arg(long, default_value_t = Params::default().subpel)]
    subpel: i32,

    /// 失配区域回退：0 最近源帧，1 线性混合。
    #[arg(long, default_value_t = Params::default().fallback)]
    fallback: i32,

    /// 失配回退的软阈值（下、上）。
    #[arg(long, default_value_t = Params::default().occ_lo)]
    occ_lo: f32,
    #[arg(long, default_value_t = Params::default().occ_hi)]
    occ_hi: f32,

    /// 关掉全帧统计的回读（排查它本身的开销；重复帧检测随之失效）。
    #[arg(long)]
    no_readback: bool,

    /// 关掉 GPU timer query（排查它本身的开销）。
    #[arg(long)]
    no_timers: bool,

    /// 输出运动场的可视化而不是画面。
    #[arg(long)]
    debug_field: bool,

    #[arg(long)]
    flip_y: bool,

    #[arg(long)]
    loop_file: bool,

    /// 播放倍速。
    #[arg(long, default_value_t = 1.0)]
    speed: f64,

    /// 跑这么多秒后自动退出并写记录。
    #[arg(long)]
    duration: Option<f64>,

    /// 第几秒时 seek（验证 flush），可给多次。
    #[arg(long)]
    seek_at: Vec<f64>,

    /// seek 的目标（秒）。
    #[arg(long, default_value_t = 5.0)]
    seek_to: f64,

    /// 第几秒截图（存进 `--shots`），可给多次。
    #[arg(long)]
    shot_at: Vec<f64>,

    #[arg(long)]
    shots: Option<PathBuf>,

    /// 实验记录（Markdown）。
    #[arg(long)]
    record: Option<PathBuf>,

    /// 每个 vsync 一行的 CSV：时间、选帧结果、相位。
    #[arg(long)]
    frames_csv: Option<PathBuf>,

    #[arg(long)]
    mpv_log: Option<PathBuf>,

    #[arg(long, default_value_t = 1280)]
    width: u32,

    #[arg(long, default_value_t = 720)]
    height: u32,

    /// 实验室模式：PNG 序列所在目录（f0001.png …，60fps 真值）。
    #[arg(long)]
    lab: Option<PathBuf>,

    /// 实验室模式的输出目录。
    #[arg(long)]
    lab_out: Option<PathBuf>,

    /// 实验室模式：第一个 PNG 的编号（ffmpeg 从 1 开始）。
    #[arg(long, default_value_t = 1)]
    lab_first: u32,

    /// #219：走正式的 `player_interp::Interpolator`，按这个设置档位插帧。
    /// 给了它，`--mode` / `--delay-ms` / `--skip-dups` 不再生效（档位、延迟、
    /// 跳重复帧都由 Interpolator 决定）。
    #[arg(long, value_enum)]
    setting: Option<CliSetting>,

    /// 不测了，直接当屏幕是这个刷新率（桌面用：macOS ProMotion 上 Slint 的
    /// 回调不锁 vsync，测出来的间隔中位数是 13 ms 左右，见 #219 记录）。
    /// 平台钩子报了刷新率时以平台为准。
    #[arg(long)]
    display_hz: Option<f32>,

    /// 「跟随屏幕刷新率」的上限（默认 60，实测通过前不放开）。
    #[arg(long, default_value_t = player_interp::policy::FOLLOW_CAP_UNVERIFIED)]
    follow_cap: f32,

    /// 定时注入生命周期 / 温控事件，`秒:事件`，可给多次。事件：seek（同时真的
    /// seek 到 `--seek-to`）、bg、fg、pip、pipexit、source、thermal-nominal、
    /// thermal-moderate、thermal-severe、speed=倍速（同时真的改 mpv 的
    /// `speed`）。用来在真机上走一遍状态机。
    #[arg(long)]
    event_at: Vec<String>,

    /// mpv 的 `glsl-shaders`（例如 Anime4K 的一串，`:` 分隔，Android 上给设备
    /// 上的路径）。
    #[arg(long)]
    glsl_shaders: Option<String>,

    /// `--glsl-shaders` 里是放大类着色器：源帧槽改成视频放进窗口后的尺寸，
    /// 让它们在插帧路径下照样生效（`player_interp::engine::slot_size`）。
    #[arg(long)]
    upscale_shaders: bool,

    /// 放大时槽的像素上限（默认 1920×1080）。
    #[arg(long, default_value_t = 1920 * 1080)]
    slot_max_pixels: i64,

    /// 桌面：窗口左上角放在这个全局坐标（逻辑像素，主屏左上角为原点，
    /// `x,y`），并且**不激活、不抢焦点**。没给时读环境变量
    /// `SPIKE_WINDOW_POS`。多显示器时用来把测试窗口固定到某一块屏上。
    #[arg(long)]
    window_pos: Option<String>,

    /// 原始参数串，写进记录。由入口填。
    #[arg(skip)]
    pub raw: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum CliSetting {
    Off,
    #[value(name = "45")]
    Fps45,
    #[value(name = "60")]
    Fps60,
    Follow,
}

impl From<CliSetting> for Setting {
    fn from(s: CliSetting) -> Self {
        match s {
            CliSetting::Off => Self::Off,
            CliSetting::Fps45 => Self::Fps45,
            CliSetting::Fps60 => Self::Fps60,
            CliSetting::Follow => Self::FollowDisplay,
        }
    }
}

/// 平台钩子（#219「刷新率」「回退」）：Android 在 `interp_android.rs` 里实现，
/// 桌面上全是空的（按 vsync 间隔测刷新率、没有温控读数）。
pub trait Platform {
    /// 系统报告的当前刷新率。
    fn refresh_hz(&self) -> Option<f32> {
        None
    }
    /// 系统温控状态。
    fn thermal(&self) -> Option<Thermal> {
        None
    }
    /// 向系统请求内容帧率（`None` = 撤销请求）。返回给记录看的说明。
    fn request_frame_rate(&mut self, _fps: Option<f32>) -> Option<String> {
        None
    }
}

/// 桌面：什么都不做。
#[cfg_attr(target_os = "android", allow(dead_code))]
pub struct NoPlatform;
impl Platform for NoPlatform {}

/// [`Mode`] 的命令行形态（`Mode` 本身在 gpu.rs，那里不能依赖 clap）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum CliMode {
    Direct,
    Hold,
    Blend,
    Mc,
}

impl From<CliMode> for Mode {
    fn from(mode: CliMode) -> Self {
        match mode {
            CliMode::Direct => Self::Direct,
            CliMode::Hold => Self::Hold,
            CliMode::Blend => Self::Blend,
            CliMode::Mc => Self::Mc,
        }
    }
}

impl Args {
    fn mode(&self) -> Mode {
        self.mode.into()
    }

    fn params(&self) -> Params {
        Params {
            radius: self.radius,
            lambda: self.lambda,
            debug_field: self.debug_field,
            step: self.step,
            subpel: self.subpel,
            fallback: self.fallback,
            occ_lo: self.occ_lo,
            occ_hi: self.occ_hi,
            ..Params::default()
        }
    }
}

struct NullSink;
impl PlayerEventSink for NullSink {
    fn emit(&mut self, _event: Stamped<PlaybackEvent>) {}
}

/// 一段样本的分位数。
#[derive(Default)]
struct Samples(Vec<f64>);

impl Samples {
    fn push(&mut self, v: f64) {
        self.0.push(v);
    }
    fn q(&self, q: f64) -> Option<f64> {
        if self.0.is_empty() {
            return None;
        }
        let mut v = self.0.clone();
        v.sort_by(f64::total_cmp);
        let idx = ((v.len() - 1) as f64 * q).round() as usize;
        Some(v[idx])
    }
    fn mean(&self) -> Option<f64> {
        (!self.0.is_empty()).then(|| self.0.iter().sum::<f64>() / self.0.len() as f64)
    }
    /// 小数值用四位小数。
    fn summary4(&self) -> String {
        match (self.q(0.05), self.q(0.25), self.q(0.5), self.q(0.95)) {
            (Some(p5), Some(p25), Some(p50), Some(p95)) => {
                format!(
                    "n={} · p5 {p5:.4} · p25 {p25:.4} · p50 {p50:.4} · p95 {p95:.4}",
                    self.0.len()
                )
            }
            _ => "（无样本）".to_owned(),
        }
    }

    fn summary(&self, unit: &str) -> String {
        match (
            self.q(0.05),
            self.q(0.5),
            self.q(0.95),
            self.q(0.99),
            self.0.iter().copied().reduce(f64::max),
        ) {
            (Some(p5), Some(p50), Some(p95), Some(p99), Some(max)) => format!(
                "n={} · p5 {p5:.2} · p50 {p50:.2} · p95 {p95:.2} · p99 {p99:.2} · max {max:.2} {unit}",
                self.0.len()
            ),
            _ => "（无样本）".to_owned(),
        }
    }
}

#[derive(Default)]
struct Stats {
    /// 收到的新源帧。
    src_frames: u64,
    /// 相邻源帧 target_time 的间隔（毫秒）。
    src_interval: Samples,
    /// 收到时 target_time - now（毫秒）：前瞻余量。
    lead: Samples,
    /// 收到时环里「比 now 晚」的帧数（含这一张）：真正可用的前瞻帧数。
    ahead_frames: Samples,
    flags_seen: BTreeMap<&'static str, u64>,
    flushes: u64,
    picks: BTreeMap<&'static str, u64>,
    /// 显示周期（毫秒），即相邻两次 BeforeRendering 的间隔。
    vsync: Samples,
    /// 插帧代码在 BeforeRendering 里的 CPU 墙钟（毫秒）。
    cpu_interp: Samples,
    /// BeforeRendering → AfterRendering 的 CPU 墙钟（毫秒）：含 Slint 自己画 UI。
    cpu_frame: Samples,
    /// 插帧段细分（CPU 墙钟，毫秒）：收帧 / 运动估计 / 输出 / 回读。
    cpu_parts: BTreeMap<&'static str, Samples>,
    gpu: BTreeMap<Stage, Samples>,
    pair_best: Samples,
    pair_zero: Samples,
    dups: u64,
    cuts: u64,
    pairs_measured: u64,
    errors: Vec<String>,
    shots: Vec<PathBuf>,
    /// 实测的 target_time 单位。
    target_unit: &'static str,
    /// #219：Interpolator 每秒一行的状态。
    status_log: Vec<String>,
    /// #219：audio-delay 变化、刷新率请求、注入的事件。
    engine_log: Vec<String>,
    /// 用 `--event-at` 注入过温控，之后不再被平台读数覆盖。
    thermal_override: bool,
}

struct Live {
    args: Args,
    params: Params,
    player: Rc<RefCell<mpv::MpvPlayer>>,
    render: Option<mpv::MpvRender>,
    gpu: Option<Gpu>,
    ring: Ring,
    stats: Stats,
    last_vsync: Option<Instant>,
    vsync_ema_us: f64,
    started: Option<Instant>,
    opened: bool,
    csv: Option<std::io::BufWriter<std::fs::File>>,
    renderer: String,
    pending_shot: Option<PathBuf>,
    video_size: (i32, i32),
    size_watch: Arc<mpv::SizeWatch>,
    harness_gl: Option<spike_harness::gl::Gl>,
    frame_started: Option<Instant>,
    /// `--setting` 给了才有。
    engine: Option<Interpolator>,
    platform: Box<dyn Platform>,
    clock: Instant,
}

/// 跑一次，返回 Markdown 记录（同时按 `--record` 落盘）。
#[cfg_attr(target_os = "android", allow(dead_code))]
pub fn run(args: Args) -> Result<String> {
    run_with(args, Box::new(NoPlatform))
}

/// 带平台钩子跑一次。
pub fn run_with(args: Args, platform: Box<dyn Platform>) -> Result<String> {
    if args.lab.is_some() {
        return run_lab(&args);
    }
    let Some(video) = args.video.clone() else {
        bail!("要么 --video，要么 --lab");
    };

    let lib = Arc::new(mpv::sys::MpvLib::load(args.mpv_lib.as_deref())?);
    let mut extra = vec![
        ("video-sync".to_owned(), args.video_sync.clone()),
        (
            "video-timing-offset".to_owned(),
            format!("{}", args.timing_offset),
        ),
        ("speed".to_owned(), format!("{}", args.speed)),
    ];
    if let Some(shaders) = &args.glsl_shaders {
        extra.push(("glsl-shaders".to_owned(), shaders.clone()));
    }
    if args.mpv_interp {
        extra.push(("interpolation".to_owned(), "yes".to_owned()));
        extra.push(("tscale".to_owned(), "oversample".to_owned()));
    }
    if args.delay_ms != 0.0 && args.setting.is_none() {
        extra.push((
            "audio-delay".to_owned(),
            format!("{}", args.delay_ms / 1000.0),
        ));
    }
    let config = mpv::MpvConfig {
        hwdec: args.hwdec.clone(),
        log_file: args
            .mpv_log
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned()),
        loop_file: args.loop_file,
        extra,
        ..Default::default()
    };
    let gate = GenerationGate::new(SessionId(1));
    let player = Rc::new(RefCell::new(mpv::MpvPlayer::new(
        Arc::clone(&lib),
        &config,
        gate.clone(),
        Box::new(NullSink),
    )?));

    place_desktop_window(args.window_pos.as_deref())?;
    let ui = SpikeWindow::new().context("建不了 Slint 窗口")?;
    ui.window().set_size(slint::LogicalSize::new(
        args.width as f32,
        args.height as f32,
    ));
    ui.set_video_active(true);
    ui.set_watermark(format!("#218 插帧 spike · mode={:?}", args.mode).into());

    let csv = match &args.frames_csv {
        Some(path) => {
            let mut file = std::io::BufWriter::new(std::fs::File::create(path)?);
            std::io::Write::write_all(&mut file, b"t_ms,kind,a_seq,b_seq,p,vsync_ms\n")?;
            Some(file)
        }
        None => None,
    };
    let live = Rc::new(RefCell::new(Live {
        params: args.params(),
        ring: Ring::new(args.ring.max(3)),
        args: args.clone(),
        player: Rc::clone(&player),
        render: None,
        gpu: None,
        stats: Stats::default(),
        last_vsync: None,
        vsync_ema_us: 16_667.0,
        started: None,
        opened: false,
        csv,
        renderer: String::new(),
        pending_shot: None,
        video_size: (0, 0),
        size_watch: player.borrow().spawn_size_watcher(),
        harness_gl: None,
        frame_started: None,
        engine: args.setting.map(|setting| {
            let mut engine = Interpolator::new();
            engine.set_setting(setting.into());
            engine.controller_mut().set_follow_cap(args.follow_cap);
            engine.set_speed(args.speed);
            engine
        }),
        platform,
        clock: Instant::now(),
    }));

    install_notifier(&ui, &live)?;

    // 渲染侧就绪后才 loadfile（ADR-004：render context 要先于 loadfile）。
    let timer = slint::Timer::default();
    {
        let live = Rc::clone(&live);
        let ui_weak = ui.as_weak();
        let gate = gate.clone();
        let mut next_seek = 0usize;
        let mut next_shot = 0usize;
        let mut last_hud = Instant::now();
        let mut last_status = Instant::now();
        let mut requested: Option<Option<f32>> = None;
        let mut events: Vec<(f64, String)> = args
            .event_at
            .iter()
            .filter_map(|e| {
                let (t, name) = e.split_once(':')?;
                Some((t.parse().ok()?, name.to_owned()))
            })
            .collect();
        events.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut next_event = 0usize;
        timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(50),
            move || {
                let Some(ui) = ui_weak.upgrade() else { return };
                let mut s = live.borrow_mut();
                if !s.opened && s.render.is_some() {
                    s.opened = true;
                    s.started = Some(Instant::now());
                    if let Err(err) = open(&s.player, &gate, &video, s.args.audio.as_deref()) {
                        s.stats.errors.push(format!("打开失败：{err:#}"));
                    }
                }
                let Some(started) = s.started else { return };
                let t = started.elapsed().as_secs_f64();
                s.engine_tick(t, &mut requested, &mut last_status);
                while let Some((at, name)) = events.get(next_event).cloned()
                    && t >= at
                {
                    next_event += 1;
                    s.inject(t, &name);
                }
                if let Some(at) = s.args.seek_at.get(next_seek).copied()
                    && t >= at
                {
                    next_seek += 1;
                    let to = format!("{}", s.args.seek_to);
                    let result = s.player.borrow().debug_set_property("time-pos", &to);
                    if let Err(err) = result {
                        s.stats.errors.push(format!("seek 失败：{err:#}"));
                    }
                }
                if let Some(at) = s.args.shot_at.get(next_shot).copied()
                    && t >= at
                {
                    next_shot += 1;
                    if let Some(dir) = s.args.shots.clone() {
                        let _ = std::fs::create_dir_all(&dir);
                        s.pending_shot = Some(
                            dir.join(format!("{:?}-{at:05.1}s.png", s.args.mode).to_lowercase()),
                        );
                    }
                }
                if last_hud.elapsed() > Duration::from_millis(250) {
                    last_hud = Instant::now();
                    ui.set_hud(hud(&s).into());
                }
                if let Some(limit) = s.args.duration
                    && t >= limit
                {
                    let _ = slint::quit_event_loop();
                }
            },
        );
    }

    ui.run()?;
    drop(timer);

    let mut s = live.borrow_mut();
    let report = report(&s);
    if let Some(path) = &s.args.record {
        std::fs::write(path, &report).with_context(|| format!("写不了 {}", path.display()))?;
    }
    if let Some(csv) = s.csv.as_mut() {
        std::io::Write::flush(csv)?;
    }
    Ok(report)
}

fn open(
    player: &Rc<RefCell<mpv::MpvPlayer>>,
    gate: &GenerationGate,
    video: &str,
    audio: Option<&str>,
) -> Result<()> {
    let source = match audio {
        Some(audio) => MediaSource::Dash {
            video: Redacted::new(video.to_owned()),
            audio: Redacted::new(audio.to_owned()),
        },
        None => MediaSource::Segments(vec![Redacted::new(video.to_owned())]),
    };
    let mut player = player.borrow_mut();
    player.dispatch(Stamped::new(
        gate.session(),
        gate.bump(),
        PlayerCommand::Open {
            source,
            headers: MediaHeaders {
                user_agent: String::new(),
                referer: String::new(),
            },
        },
    ))?;
    player.dispatch(Stamped::new(
        gate.session(),
        gate.current(),
        PlayerCommand::Play,
    ))
}

fn install_notifier(ui: &SpikeWindow, live: &Rc<RefCell<Live>>) -> Result<()> {
    let live = Rc::clone(live);
    let ui_weak = ui.as_weak();
    ui.window()
        .set_rendering_notifier(move |state, api| {
            let slint::GraphicsAPI::NativeOpenGL { get_proc_address } = api else {
                return;
            };
            let loader = |name: &std::ffi::CStr| get_proc_address(name);
            let mut s = live.borrow_mut();
            match state {
                slint::RenderingState::RenderingSetup => {
                    if let Some(engine) = s.engine.as_mut() {
                        let attached = unsafe { engine.attach_gl(&loader) };
                        match attached {
                            Ok(()) => {
                                s.renderer = engine.gpu().map(Gpu::renderer).unwrap_or_default();
                            }
                            Err(err) => s.stats.errors.push(format!("Interpolator GPU：{err}")),
                        }
                    } else {
                        match unsafe { Gpu::new(&loader, s.ring.capacity()) } {
                            Ok(mut gpu) => {
                                if s.args.no_timers {
                                    gpu.disable_timers();
                                }
                                if s.args.no_readback {
                                    gpu.disable_readback();
                                }
                                s.renderer = gpu.renderer();
                                s.gpu = Some(gpu);
                            }
                            Err(err) => s.stats.errors.push(format!("GPU 初始化失败：{err:#}")),
                        }
                    }
                    s.harness_gl = unsafe { spike_harness::gl::Gl::load(&loader) }.ok();
                    let created = unsafe { s.player.borrow().create_render(&loader) };
                    match created {
                        Ok(render) => s.render = Some(render),
                        Err(err) => s.stats.errors.push(format!("mpv render context：{err:#}")),
                    }
                }
                slint::RenderingState::BeforeRendering => {
                    let Some(window) = ui_weak.upgrade() else {
                        return;
                    };
                    let size = window.window().size();
                    let target = (size.width as i32, size.height as i32);
                    let started = Instant::now();
                    s.frame_started = Some(started);
                    let result = s.vsync(&loader, target);
                    s.stats
                        .cpu_interp
                        .push(started.elapsed().as_secs_f64() * 1000.0);
                    if let Err(err) = result {
                        let message = format!("{err:#}");
                        if !s.stats.errors.contains(&message) {
                            s.stats.errors.push(message);
                        }
                    }
                }
                slint::RenderingState::AfterRendering => {
                    if let Some(started) = s.frame_started.take() {
                        s.stats
                            .cpu_frame
                            .push(started.elapsed().as_secs_f64() * 1000.0);
                    }
                    if let Some(path) = s.pending_shot.take() {
                        let size = ui_weak.upgrade().map(|w| w.window().size());
                        if let (Some(gl), Some(size)) = (s.harness_gl.as_ref(), size) {
                            match gl.capture_png(size.width, size.height, &path) {
                                Ok(()) => s.stats.shots.push(path),
                                Err(err) => s.stats.errors.push(format!("截图失败：{err:#}")),
                            }
                        }
                        // 截图那一帧的间隔不算。
                        s.last_vsync = None;
                    }
                    // 持续重绘：输出节奏跟显示刷新走，不跟 UI 有没有变化走。
                    if let Some(window) = ui_weak.upgrade() {
                        window.window().request_redraw();
                    }
                }
                slint::RenderingState::RenderingTeardown => {
                    s.render = None;
                    s.gpu = None;
                    if let Some(engine) = s.engine.as_mut() {
                        engine.detach_gl();
                    }
                }
                _ => {}
            }
        })
        .map_err(|err| anyhow::anyhow!("装不上渲染回调：{err}"))
}

impl Live {
    fn vsync(
        &mut self,
        loader: &dyn Fn(&std::ffi::CStr) -> *const std::ffi::c_void,
        target: (i32, i32),
    ) -> Result<()> {
        let wall = Instant::now();
        if let Some(last) = self.last_vsync {
            let us = wall.duration_since(last).as_secs_f64() * 1e6;
            self.stats.vsync.push(us / 1000.0);
            // 只拿正常间隔更新估计，卡顿那一下不算。
            if us < self.vsync_ema_us * 1.5 {
                self.vsync_ema_us = self.vsync_ema_us * 0.95 + us * 0.05;
            }
        }
        self.last_vsync = Some(wall);
        let Some(render) = self.render.as_ref() else {
            return Ok(());
        };
        let slint_fbo = self
            .harness_gl
            .as_ref()
            .map_or(0, spike_harness::gl::Gl::draw_framebuffer);

        if self.engine.is_some() {
            return self.vsync_engine(loader, target, slint_fbo, wall);
        }

        if self.args.mode() == Mode::Direct {
            return unsafe {
                render.render(slint_fbo, target.0, target.1, self.args.flip_y, loader)
            };
        }
        let Some(gpu) = self.gpu.as_mut() else {
            return Ok(());
        };
        let now = self.player.borrow().time_us();

        // 1. 收帧：mpv 每交出一张新帧，就画进环里的一个槽。
        let t_intake = Instant::now();
        for _ in 0..8 {
            let flags = render.update_flags();
            if flags & mpv::sys::RENDER_UPDATE_FRAME == 0 {
                break;
            }
            let info = render.next_frame_info().unwrap_or_default();
            for (bit, name) in [
                (mpv::sys::frame_info::PRESENT, "PRESENT"),
                (mpv::sys::frame_info::REDRAW, "REDRAW"),
                (mpv::sys::frame_info::REPEAT, "REPEAT"),
                (mpv::sys::frame_info::BLOCK_VSYNC, "BLOCK_VSYNC"),
            ] {
                if info.flags & bit != 0 {
                    *self.stats.flags_seen.entry(name).or_default() += 1;
                }
            }
            if info.target_time == 0 {
                *self.stats.flags_seen.entry("target_time=0").or_default() += 1;
            }
            let redraw = info.flags & mpv::sys::frame_info::REDRAW != 0 || info.target_time == 0;

            {
                let w = self.size_watch.0.load(std::sync::atomic::Ordering::Acquire) as i32;
                let h = self.size_watch.1.load(std::sync::atomic::Ordering::Acquire) as i32;
                if w > 0 && h > 0 {
                    self.video_size = (w, h);
                }
            }
            let (w, h) = self.video_size;
            if w == 0 || h == 0 {
                // 还不知道尺寸：画一下把帧消费掉，免得 mpv 一直等。
                unsafe { render.render(slint_fbo, target.0, target.1, self.args.flip_y, loader)? };
                break;
            }
            if gpu.ensure_size(w, h)? {
                self.ring.clear();
            }
            // redraw（暂停中改了参数之类）覆盖最新那张，不进时间轴。
            let slot = match (redraw, self.ring.newest()) {
                (true, Some(newest)) => newest.slot,
                _ => self.ring.slot_for_next(),
            };
            let fbo = gpu.slot_fbo(slot);
            let flip = self.args.flip_y;
            // 不给 mpv 这一下套 GPU 计时：mpv 自己也用 TIME_ELAPSED 查询，嵌套会让
            // 它的 begin_query 报 INVALID_OPERATION（第一轮实测这里读出全 0）。
            unsafe { render.render(fbo, w, h, flip, loader) }?;
            gpu.prepare_slot(slot);
            if std::env::var_os("KK_DEBUG_HASH").is_some() {
                eprintln!(
                    "HASH t={} now={now} flags={:#x} slot={slot} sum={}",
                    info.target_time,
                    info.flags,
                    gpu.luma_checksum(slot)
                );
            }
            if redraw && self.ring.newest().is_some() {
                continue;
            }
            // render.h 说 target_time 与 mpv_get_time_us 同为微秒；mpv 0.42 实测是
            // **纳秒**（内部时钟 0.37 起改成 ns，这个字段没跟着换算）。按量级判：
            // 同一时刻的两个读数，若 target/1000 离 now 更近，就是纳秒。
            let t = if info.target_time == 0 {
                now
            } else if (info.target_time / 1000 - now).abs() < (info.target_time - now).abs() {
                self.stats.target_unit = "ns（与 render.h 注释不符）";
                info.target_time / 1000
            } else {
                self.stats.target_unit = "µs（与 render.h 一致）";
                info.target_time
            };
            let prev = self.ring.newest();
            let (_, pushed) = self.ring.push(t, slot);
            self.stats.src_frames += 1;
            if pushed == Push::Flushed {
                self.stats.flushes += 1;
            } else if let Some(prev) = prev {
                self.stats
                    .src_interval
                    .push((t - prev.t_us) as f64 / 1000.0);
            }
            self.stats.lead.push((t - now) as f64 / 1000.0);
        }

        self.stats
            .cpu_parts
            .entry("1 收帧")
            .or_default()
            .push(t_intake.elapsed().as_secs_f64() * 1000.0);

        // 2. 选帧：这一帧真正上屏大约在 now + present_frames 个显示周期之后。
        let t_out = now + (self.vsync_ema_us * self.args.present_frames) as i64
            - (self.args.delay_ms * 1000.0) as i64;
        let ahead = self.ring.count_after(now);
        self.stats.ahead_frames.push(ahead as f64);
        let pick = self.ring.pick(t_out, self.args.skip_dups);
        let fbo = Gpu::framebuffer(slint_fbo);
        let (kind, a_seq, b_seq, p) = match pick {
            Pick::Empty => ("empty", 0, 0, 0.0),
            Pick::Pair { a, b, p } => {
                if self.args.mode() == Mode::Mc {
                    let t = Instant::now();
                    gpu.ensure_field(&a, &b, &self.params);
                    self.stats
                        .cpu_parts
                        .entry("2 运动估计")
                        .or_default()
                        .push(t.elapsed().as_secs_f64() * 1000.0);
                }
                let t = Instant::now();
                gpu.present(fbo, target, &a, &b, p, self.args.mode(), &self.params);
                self.stats
                    .cpu_parts
                    .entry("3 输出")
                    .or_default()
                    .push(t.elapsed().as_secs_f64() * 1000.0);
                ("pair", a.seq, b.seq, p)
            }
            Pick::Starved { a } => {
                gpu.present(fbo, target, &a, &a, 0.0, Mode::Hold, &self.params);
                ("starved", a.seq, a.seq, 0.0)
            }
            Pick::Early { a } => {
                gpu.present(fbo, target, &a, &a, 0.0, Mode::Hold, &self.params);
                ("early", a.seq, a.seq, 0.0)
            }
        };
        *self.stats.picks.entry(kind).or_default() += 1;
        if let (Some(csv), Some(started)) = (self.csv.as_mut(), self.started) {
            let _ = writeln!(
                StringWriter(csv),
                "{:.3},{kind},{a_seq},{b_seq},{p:.4},{:.3}",
                started.elapsed().as_secs_f64() * 1000.0,
                self.vsync_ema_us / 1000.0
            );
        }

        // 3. 收 GPU 计时与回读。
        let t = Instant::now();
        let (timings, pairs) = gpu.poll();
        self.stats
            .cpu_parts
            .entry("4 回读")
            .or_default()
            .push(t.elapsed().as_secs_f64() * 1000.0);
        for (stage, ms) in timings {
            self.stats.gpu.entry(stage).or_default().push(f64::from(ms));
        }
        for stats in pairs {
            if std::env::var_os("KK_DEBUG_HASH").is_some() {
                eprintln!(
                    "PAIR {:?} best={:.4} changed={:.3}",
                    stats.pair, stats.best, stats.changed
                );
            }
            self.stats.pairs_measured += 1;
            self.stats.pair_best.push(f64::from(stats.best));
            self.stats.pair_zero.push(f64::from(stats.changed));
            if stats.changed < self.params.dup_thresh {
                self.stats.dups += 1;
                self.ring.mark_dup(stats.pair.1);
            }
            if stats.best > self.params.cut_thresh {
                self.stats.cuts += 1;
            }
        }
        Ok(())
    }
}

impl Live {
    /// #219：走正式 `Interpolator` 的一帧。与正式接入的写法相同（见
    /// `player_interp::engine` 的模块文档），外加记录。
    fn vsync_engine(
        &mut self,
        loader: &dyn Fn(&std::ffi::CStr) -> *const std::ffi::c_void,
        target: (i32, i32),
        slint_fbo: i32,
        wall: Instant,
    ) -> Result<()> {
        let Some(render) = self.render.as_ref() else {
            return Ok(());
        };
        let Some(engine) = self.engine.as_mut() else {
            return Ok(());
        };
        let started = Instant::now();
        let wall_us = wall.duration_since(self.clock).as_micros() as i64;
        let now = self.player.borrow().time_us();
        let flip = self.args.flip_y;
        let plan = engine.begin_vsync(wall_us);
        {
            let w = self.size_watch.0.load(std::sync::atomic::Ordering::Acquire) as i32;
            let h = self.size_watch.1.load(std::sync::atomic::Ordering::Acquire) as i32;
            if w > 0 && h > 0 {
                self.video_size = (w, h);
            }
            let fps = self.size_watch.2.load(std::sync::atomic::Ordering::Acquire);
            engine.set_source_fps_hint((fps > 0).then_some(fps as f32 / 1000.0));
        }
        let kind = match plan {
            Plan::Direct => {
                if render.update_flags() & mpv::sys::RENDER_UPDATE_FRAME != 0 {
                    let info = render.next_frame_info().unwrap_or_default();
                    if info.flags & mpv::sys::frame_info::REDRAW == 0 {
                        engine.observe_direct(info.target_time, now);
                        self.stats.src_frames += 1;
                    }
                }
                unsafe { render.render(slint_fbo, target.0, target.1, flip, loader)? };
                "direct"
            }
            Plan::Interp => {
                for _ in 0..8 {
                    if render.update_flags() & mpv::sys::RENDER_UPDATE_FRAME == 0 {
                        break;
                    }
                    let info = render.next_frame_info().unwrap_or_default();
                    let (w, h) = self.video_size;
                    if w == 0 || h == 0 {
                        unsafe { render.render(slint_fbo, target.0, target.1, flip, loader)? };
                        break;
                    }
                    let redraw = info.flags & mpv::sys::frame_info::REDRAW != 0;
                    let size = player_interp::engine::slot_size(
                        (w, h),
                        target,
                        self.args.upscale_shaders,
                        self.args.slot_max_pixels,
                    );
                    let slot = engine.source_target(size, redraw)?;
                    unsafe { render.render(slot.fbo, slot.width, slot.height, flip, loader)? };
                    engine.commit_source(slot, info.target_time, now);
                    if !redraw {
                        self.stats.src_frames += 1;
                    }
                }
                match engine.present(slint_fbo, target, now) {
                    Presented::Frame => "interp",
                    Presented::Nothing => {
                        unsafe { render.render(slint_fbo, target.0, target.1, flip, loader)? };
                        "interp-empty"
                    }
                }
            }
        };
        *self.stats.picks.entry(kind).or_default() += 1;
        let cost = started.elapsed();
        engine.end_vsync(cost.as_micros() as i64);
        if let Some(delay_us) = engine.take_av_delay_change() {
            // 异步写（debug_set_property 走 mpv_set_property_async），渲染线程上可以发。
            let secs = format!("{}", delay_us as f64 / 1e6);
            if let Err(err) = self
                .player
                .borrow()
                .debug_set_property("audio-delay", &secs)
            {
                self.stats.errors.push(format!("audio-delay：{err:#}"));
            }
            let t = self.started.map_or(0.0, |s| s.elapsed().as_secs_f64());
            self.stats
                .engine_log
                .push(format!("{t:6.2}s audio-delay → {} ms", delay_us / 1000));
        }
        Ok(())
    }

    /// 计时器里调：平台读数喂给 Interpolator、按档位请求帧率、每秒记一行状态。
    fn engine_tick(
        &mut self,
        t: f64,
        requested: &mut Option<Option<f32>>,
        last_status: &mut Instant,
    ) {
        let Some(engine) = self.engine.as_mut() else {
            return;
        };
        engine.set_display_refresh(self.platform.refresh_hz().or(self.args.display_hz));
        if let Some(thermal) = self.platform.thermal()
            && !self.stats.thermal_override
        {
            engine.set_thermal(thermal);
        }
        let want = engine.controller().setting().preferred_display_rate();
        if *requested != Some(want) {
            *requested = Some(want);
            if let Some(note) = self.platform.request_frame_rate(want) {
                self.stats.engine_log.push(format!("{t:6.2}s {note}"));
            }
        }
        if last_status.elapsed() >= Duration::from_secs(1) {
            *last_status = Instant::now();
            let st = engine.status();
            let line = format!(
                "{t:6.2}s {} · 屏幕 {} · 片源 {} · {:?} · 温控 {:?}（平台 {:?}）· 延迟 {} ms · 帧对 {} 重复 {}",
                status_line(&st),
                st.display_hz
                    .map_or("?".to_owned(), |v| format!("{v:.1} Hz")),
                st.source_fps
                    .map_or("?".to_owned(), |v| format!("{v:.2} fps")),
                st.decision,
                st.thermal,
                self.platform.thermal(),
                st.av_delay_us / 1000,
                st.pairs_measured,
                st.dup_pairs,
            );
            self.stats.status_log.push(line);
        }
    }

    fn inject(&mut self, t: f64, name: &str) {
        let Some(engine) = self.engine.as_mut() else {
            return;
        };
        if let Some(speed) = name.strip_prefix("speed=") {
            match speed.parse::<f64>() {
                Ok(speed) => {
                    if let Err(err) = self
                        .player
                        .borrow()
                        .debug_set_property("speed", &format!("{speed}"))
                    {
                        self.stats.errors.push(format!("改倍速失败：{err:#}"));
                    }
                    engine.set_speed(speed);
                }
                Err(_) => self.stats.errors.push(format!("倍速写错了：{name}")),
            }
            self.stats.engine_log.push(format!("{t:6.2}s 注入 {name}"));
            return;
        }
        let event = match name {
            "seek" => {
                let to = format!("{}", self.args.seek_to);
                if let Err(err) = self.player.borrow().debug_set_property("time-pos", &to) {
                    self.stats.errors.push(format!("seek 失败：{err:#}"));
                }
                Some(Event::Seek)
            }
            "bg" => Some(Event::Background),
            "fg" => Some(Event::Foreground),
            "pip" => Some(Event::PipEnter),
            "pipexit" => Some(Event::PipExit),
            "source" => Some(Event::SourceChanged),
            _ => None,
        };
        let thermal = match name {
            "thermal-nominal" => Some(Thermal::Nominal),
            "thermal-moderate" => Some(Thermal::Moderate),
            "thermal-severe" => Some(Thermal::Severe),
            _ => None,
        };
        if let Some(event) = event {
            engine.on_event(event);
        }
        if let Some(thermal) = thermal {
            // 注入之后不再让平台读数覆盖它。
            self.stats.thermal_override = true;
            engine.set_thermal(thermal);
        }
        self.stats.engine_log.push(format!("{t:6.2}s 注入 {name}"));
    }
}

/// 「目标 60 FPS · 当前 30 FPS」。界面文案按 AGENTS.md §2 留在宿主。
fn status_line(st: &player_interp::Status) -> String {
    let target = st
        .requested_fps
        .map_or("关闭".to_owned(), |v| format!("目标 {v:.0} FPS"));
    let current = st
        .output_fps
        .map_or("当前 ?".to_owned(), |v| format!("当前 {v:.0} FPS"));
    format!("{target} · {current}")
}

/// `writeln!` 需要 `fmt::Write`，CSV 是 `io::Write`。
struct StringWriter<'a, W: std::io::Write>(&'a mut W);
impl<W: std::io::Write> std::fmt::Write for StringWriter<'_, W> {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        self.0.write_all(s.as_bytes()).map_err(|_| std::fmt::Error)
    }
}

fn hud(s: &Live) -> String {
    if let Some(engine) = &s.engine {
        let st = engine.status();
        return format!(
            "{}\n{:?} · 屏幕 {} · 片源 {} · 延迟 {} ms\n{}",
            status_line(&st),
            st.decision,
            st.display_hz
                .map_or("?".to_owned(), |v| format!("{v:.1} Hz")),
            st.source_fps
                .map_or("?".to_owned(), |v| format!("{v:.2} fps")),
            st.av_delay_us / 1000,
            s.renderer,
        );
    }
    let picks: Vec<String> = s
        .stats
        .picks
        .iter()
        .map(|(k, v)| format!("{k} {v}"))
        .collect();
    let gpu: Vec<String> = s
        .stats
        .gpu
        .iter()
        .map(|(k, v)| format!("{k:?} {:.2}ms", v.q(0.5).unwrap_or(0.0)))
        .collect();
    format!(
        "mode {:?} · 显示 {:.1} Hz · 源帧 {} · 前瞻 p50 {:.1} ms\n{}\n{}\n{}",
        s.args.mode,
        1e6 / s.vsync_ema_us,
        s.stats.src_frames,
        s.stats.lead.q(0.5).unwrap_or(0.0),
        picks.join(" · "),
        gpu.join(" · "),
        s.renderer,
    )
}

fn report(s: &Live) -> String {
    let mut out = String::new();
    let a = &s.args;
    let player = s.player.borrow();
    let prop = |name: &str| {
        player
            .property_string(name)
            .unwrap_or_else(|| "<无>".to_owned())
    };
    let _ = writeln!(out, "## interp 运行记录\n");
    let _ = writeln!(out, "- 参数：`{}`", a.raw);
    let _ = writeln!(out, "- GL：{}", s.renderer);
    let _ = writeln!(
        out,
        "- mpv：hwdec-current={} · video-codec={} · container-fps={} · estimated-vf-fps={} · 尺寸 {}×{}",
        prop("hwdec-current"),
        prop("video-codec"),
        prop("container-fps"),
        prop("estimated-vf-fps"),
        s.video_size.0,
        s.video_size.1
    );
    let _ = writeln!(
        out,
        "- mpv 自报：frame-drop-count={} · vo-delayed-frame-count={} · avsync={} · display-fps={}",
        prop("frame-drop-count"),
        prop("vo-delayed-frame-count"),
        prop("avsync"),
        prop("display-fps")
    );
    let _ = writeln!(
        out,
        "- 模式 {:?} · video-sync={} · timing-offset={}s · delay={}ms · skip-dups={} · ring={} · R={} · λ={}",
        a.mode, a.video_sync, a.timing_offset, a.delay_ms, a.skip_dups, a.ring, a.radius, a.lambda
    );
    let _ = writeln!(
        out,
        "- GPU 计时：{}",
        if s.gpu.as_ref().is_some_and(Gpu::timers_enabled) {
            "有"
        } else {
            "无（缺 timer query）"
        }
    );
    let _ = writeln!(out, "\n| 项 | 值 |\n|---|---|");
    let st = &s.stats;
    let _ = writeln!(
        out,
        "| target_time 单位 | {} |",
        if st.target_unit.is_empty() {
            "<未判定>"
        } else {
            st.target_unit
        }
    );
    let _ = writeln!(
        out,
        "| 新源帧 | {}（flush {} 次） |",
        st.src_frames, st.flushes
    );
    let _ = writeln!(out, "| 源帧间隔 | {} |", st.src_interval.summary("ms"));
    let _ = writeln!(
        out,
        "| 前瞻余量（target − now） | {} |",
        st.lead.summary("ms")
    );
    let _ = writeln!(
        out,
        "| 环里晚于 now 的帧数 | {} |",
        st.ahead_frames.summary("张")
    );
    let _ = writeln!(out, "| 显示间隔 | {} |", st.vsync.summary("ms"));
    let _ = writeln!(out, "| CPU：插帧段 | {} |", st.cpu_interp.summary("ms"));
    let _ = writeln!(
        out,
        "| CPU：整帧（含 Slint） | {} |",
        st.cpu_frame.summary("ms")
    );
    for (name, v) in &st.cpu_parts {
        let _ = writeln!(out, "| CPU：{name} | {} |", v.summary("ms"));
    }
    let flags: Vec<String> = st
        .flags_seen
        .iter()
        .map(|(k, v)| format!("{k} {v}"))
        .collect();
    let _ = writeln!(out, "| frame_info flags | {} |", flags.join(" · "));
    let total: u64 = st.picks.values().sum();
    let picks: Vec<String> = st
        .picks
        .iter()
        .map(|(k, v)| format!("{k} {v}（{:.1}%）", *v as f64 * 100.0 / total.max(1) as f64))
        .collect();
    let _ = writeln!(out, "| 选帧 | {} |", picks.join(" · "));
    for (stage, samples) in &st.gpu {
        let _ = writeln!(out, "| GPU {stage:?} | {} |", samples.summary("ms"));
    }
    let _ = writeln!(out, "| 帧对：最优失配 | {} |", st.pair_best.summary4());
    let _ = writeln!(out, "| 帧对：变化块占比 | {} |", st.pair_zero.summary4());
    let _ = writeln!(
        out,
        "| 帧对判定 | 测了 {} 对 · 重复 {} · 转场 {} · 平均失配 {:.4} |",
        st.pairs_measured,
        st.dups,
        st.cuts,
        st.pair_best.mean().unwrap_or(0.0)
    );
    if !st.status_log.is_empty() || !st.engine_log.is_empty() {
        let _ = writeln!(out, "\n### Interpolator（--setting {:?}）\n", a.setting);
        let _ = writeln!(out, "```");
        for line in &st.engine_log {
            let _ = writeln!(out, "{line}");
        }
        for line in &st.status_log {
            let _ = writeln!(out, "{line}");
        }
        let _ = writeln!(out, "```");
    }
    if !st.shots.is_empty() {
        let shots: Vec<String> = st.shots.iter().map(|p| p.display().to_string()).collect();
        let _ = writeln!(out, "| 截图 | {} |", shots.join(" · "));
    }
    if !st.errors.is_empty() {
        let _ = writeln!(out, "\n错误：\n");
        for e in &st.errors {
            let _ = writeln!(out, "- {e}");
        }
    }
    out
}

// ───────────────────────────── 实验室模式 ─────────────────────────────

fn read_png(path: &Path) -> Result<(u32, u32, Vec<u8>)> {
    let decoder = png::Decoder::new(std::io::BufReader::new(
        std::fs::File::open(path).with_context(|| format!("打不开 {}", path.display()))?,
    ));
    let mut reader = decoder.read_info()?;
    let mut buf = vec![0; reader.output_buffer_size().context("PNG 太大")?];
    let info = reader.next_frame(&mut buf)?;
    let (w, h) = (info.width, info.height);
    let rgba = match info.color_type {
        png::ColorType::Rgba => buf[..(w * h * 4) as usize].to_vec(),
        png::ColorType::Rgb => buf[..(w * h * 3) as usize]
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|c| [c[0], c[1], c[2], 255])
            .collect(),
        other => bail!("{}：只认 RGB / RGBA，这张是 {other:?}", path.display()),
    };
    Ok((w, h, rgba))
}

fn write_png(path: &Path, w: u32, h: u32, rgba: &[u8]) -> Result<()> {
    let file = std::io::BufWriter::new(std::fs::File::create(path)?);
    let mut encoder = png::Encoder::new(file, w, h);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(rgba)?;
    Ok(())
}

fn run_lab(args: &Args) -> Result<String> {
    let dir = args.lab.clone().expect("调用方已判断");
    let out = args.lab_out.clone().unwrap_or_else(|| dir.join("out"));
    std::fs::create_dir_all(&out)?;
    let mut frames = Vec::new();
    for i in args.lab_first.. {
        let path = dir.join(format!("f{i:04}.png"));
        if !path.exists() {
            break;
        }
        frames.push(path);
    }
    if frames.len() < 3 {
        bail!("{} 里至少要 3 张 f%04d.png", dir.display());
    }

    place_desktop_window(args.window_pos.as_deref())?;
    let ui = SpikeWindow::new()?;
    ui.window().set_size(slint::PhysicalSize::new(64, 64));
    let result: Rc<RefCell<Option<Result<String>>>> = Rc::new(RefCell::new(None));
    {
        let result = Rc::clone(&result);
        let args = args.clone();
        let mut done = false;
        ui.window()
            .set_rendering_notifier(move |state, api| {
                if done || !matches!(state, slint::RenderingState::BeforeRendering) {
                    return;
                }
                done = true;
                let slint::GraphicsAPI::NativeOpenGL { get_proc_address } = api else {
                    return;
                };
                let loader = |name: &std::ffi::CStr| get_proc_address(name);
                *result.borrow_mut() = Some(lab_pass(&loader, &args, &frames, &out));
                let _ = slint::quit_event_loop();
            })
            .map_err(|err| anyhow::anyhow!("{err}"))?;
    }
    ui.run()?;
    let report = result.borrow_mut().take().context("渲染回调没跑起来")??;
    if let Some(path) = &args.record {
        std::fs::write(path, &report)?;
    }
    Ok(report)
}

/// 对每个奇数下标 k：用 k-1、k+1 插出 k，分别按 blend 与 mc 写 PNG。
fn lab_pass(
    loader: &dyn Fn(&std::ffi::CStr) -> *const std::ffi::c_void,
    args: &Args,
    frames: &[PathBuf],
    out: &Path,
) -> Result<String> {
    let (w, h, _) = read_png(&frames[0])?;
    let (wi, hi) = (w as i32, h as i32);
    let mut gpu = unsafe { Gpu::new(loader, 3)? };
    gpu.ensure_size(wi, hi)?;
    let target = gpu.offscreen(wi, hi)?;
    let params = args.params();
    let mut ring = Ring::new(3);
    let mut motion_ms = Samples::default();
    let mut lab_timings = Vec::new();
    let mut lab_pairs: Vec<String> = Vec::new();
    let mut written = 0;
    let mut k = 1;
    while k + 1 < frames.len() {
        ring.clear();
        for (idx, t) in [(k - 1, 0i64), (k + 1, 2)] {
            let (_, _, rgba) = read_png(&frames[idx])?;
            let slot = ring.slot_for_next();
            gpu.upload(slot, &flip_rows(&rgba, w, h));
            gpu.prepare_slot(slot);
            ring.push(t, slot);
        }
        let Pick::Pair { a, b, p } = ring.pick(1, false) else {
            bail!("实验室选帧失败")
        };
        gpu.finish();
        let started = Instant::now();
        gpu.ensure_field(&a, &b, &params);
        gpu.finish();
        motion_ms.push(started.elapsed().as_secs_f64() * 1000.0);
        let (timings, pairs) = gpu.poll();
        lab_timings.extend(timings);
        for st in pairs {
            lab_pairs.push(format!("{:.4}/{:.3}", st.best, st.changed));
        }
        let stem = frames[k]
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("f")
            .to_owned();
        for mode in [Mode::Hold, Mode::Blend, Mode::Mc] {
            gpu.present(Some(target), (wi, hi), &a, &b, p, mode, &params);
            let rgba = flip_rows(&gpu.read_rgba(target, wi, hi), w, h);
            let name = format!("{}-{stem}.png", format!("{mode:?}").to_lowercase());
            write_png(&out.join(name), w, h, &rgba)?;
        }
        written += 1;
        k += 2;
    }
    let (timings, _) = gpu.poll();
    lab_timings.extend(timings);
    let mut by_stage: BTreeMap<Stage, Samples> = BTreeMap::new();
    for (stage, ms) in lab_timings {
        by_stage.entry(stage).or_default().push(f64::from(ms));
    }
    let gpu_motion = by_stage.remove(&Stage::Motion).unwrap_or_default();
    let breakdown: Vec<String> = by_stage
        .iter()
        .filter(|(stage, _)| {
            matches!(
                stage,
                Stage::MeCost
                    | Stage::MeArgmin
                    | Stage::MeSmooth
                    | Stage::MeReduce
                    | Stage::Luma
                    | Stage::Output
            )
        })
        .map(|(stage, v)| format!("{stage:?} p50 {:.2}", v.q(0.5).unwrap_or(0.0)))
        .collect();
    Ok(format!(
        "## interp 实验室\n\n- GL：{}\n- 尺寸 {w}×{h} · 运动场 {:?} · R={} · λ={}\n- 插出 {written} 张（blend 与 mc 各一份）→ {}\n- 运动估计（含 finish 的墙钟）：{}\n- 运动估计（GPU timer）：{}\n- 分项（GPU timer，ms）：{}\n- 帧对（失配/变化块占比）：{}\n",
        gpu.renderer(),
        gpu.field_size(),
        params.radius,
        params.lambda,
        out.display(),
        motion_ms.summary("ms"),
        gpu_motion.summary("ms"),
        breakdown.join(" · "),
        lab_pairs.join(" "),
    ))
}

/// PNG 从上往下，GL 纹理从下往上；进出各翻一次，输出与输入同向。
fn flip_rows(rgba: &[u8], w: u32, h: u32) -> Vec<u8> {
    let stride = (w * 4) as usize;
    rgba.chunks_exact(stride)
        .rev()
        .take(h as usize)
        .flatten()
        .copied()
        .collect()
}

/// 见 `--window-pos`。必须在建第一个 Slint 窗口之前调用。
#[cfg(not(target_os = "android"))]
fn place_desktop_window(arg: Option<&str>) -> Result<()> {
    use slint::winit_030::winit;
    let spec = arg
        .map(str::to_owned)
        .or_else(|| std::env::var("SPIKE_WINDOW_POS").ok());
    let Some(spec) = spec else {
        return Ok(());
    };
    let (x, y) = spec
        .split_once(',')
        .and_then(|(x, y)| Some((x.trim().parse::<f64>().ok()?, y.trim().parse::<f64>().ok()?)))
        .with_context(|| format!("--window-pos 要写成 x,y：{spec}"))?;
    #[allow(unused_mut)]
    let mut builder =
        winit::event_loop::EventLoop::<slint::winit_030::SlintEvent>::with_user_event();
    #[cfg(target_os = "macos")]
    {
        use winit::platform::macos::EventLoopBuilderExtMacOS;
        builder.with_activate_ignoring_other_apps(false);
    }
    slint::BackendSelector::new()
        .with_winit_event_loop_builder(builder)
        .with_winit_window_attributes_hook(move |attrs| {
            attrs
                .with_position(winit::dpi::LogicalPosition::new(x, y))
                .with_active(false)
        })
        .select()
        .context("选不了 winit 后端")?;
    Ok(())
}

#[cfg(target_os = "android")]
fn place_desktop_window(_arg: Option<&str>) -> Result<()> {
    Ok(())
}
