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

    /// 原始参数串，写进记录。由入口填。
    #[arg(skip)]
    pub raw: String,
}

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
    size_watch: Arc<(std::sync::atomic::AtomicI64, std::sync::atomic::AtomicI64)>,
    harness_gl: Option<spike_harness::gl::Gl>,
    frame_started: Option<Instant>,
}

/// 跑一次，返回 Markdown 记录（同时按 `--record` 落盘）。
pub fn run(args: Args) -> Result<String> {
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
    if args.mpv_interp {
        extra.push(("interpolation".to_owned(), "yes".to_owned()));
        extra.push(("tscale".to_owned(), "oversample".to_owned()));
    }
    if args.delay_ms != 0.0 {
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

/// `writeln!` 需要 `fmt::Write`，CSV 是 `io::Write`。
struct StringWriter<'a, W: std::io::Write>(&'a mut W);
impl<W: std::io::Write> std::fmt::Write for StringWriter<'_, W> {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        self.0.write_all(s.as_bytes()).map_err(|_| std::fmt::Error)
    }
}

fn hud(s: &Live) -> String {
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
