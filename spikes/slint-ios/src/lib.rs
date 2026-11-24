//! SPIKE-004（issue #11）iOS 实验台。
//!
//! 入口是 [`slint_ios_main`]，由 `ios/main.c` 的 `main()` 调用。iOS 上
//! winit 的事件循环会接管 `UIApplicationMain`，**不会返回**。
//!
//! 参数走环境变量（`devicectl device process launch --environment-variables`）：
//! `SPIKE_VIDEO` / `SPIKE_AUDIO`（DASH 两路 URL）、`SPIKE_UA`、`SPIKE_REFERER`、
//! `SPIKE_HWDEC`（默认 `videotoolbox-copy`）。没有 `SPIKE_VIDEO` 就只跑第 1 步。

mod mpv;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

use slint::{ComponentHandle, Image, Model, Rgba8Pixel, SharedPixelBuffer, VecModel};

slint::include_modules!();

/// 打一行到 stdout。`xcrun devicectl device process launch --console` 能收到，
/// 宿主机靠这些行判定结果，不靠人看屏幕。
fn log(line: &str) {
    println!("[spike] {line}");
}

#[unsafe(no_mangle)]
pub extern "C" fn slint_ios_main() {
    if let Err(err) = run() {
        log(&format!("FATAL {err}"));
    }
}

fn env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

/// 进程累计 CPU 时间（用户 + 内核），秒。
fn cpu_seconds() -> f64 {
    // SAFETY: getrusage 只写我们给的结构体。
    unsafe {
        let mut usage: libc_rusage = std::mem::zeroed();
        getrusage(0, &mut usage);
        let t = |tv: Timeval| tv.sec as f64 + tv.usec as f64 / 1e6;
        t(usage.utime) + t(usage.stime)
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Timeval {
    sec: i64,
    usec: i32,
}
#[repr(C)]
struct libc_rusage {
    utime: Timeval,
    stime: Timeval,
    rest: [i64; 14],
}
unsafe extern "C" {
    fn getrusage(who: i32, usage: *mut libc_rusage) -> i32;
}

fn percentile(sorted: &[f32], p: f32) -> f32 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = ((sorted.len() - 1) as f32 * p).round() as usize;
    sorted[idx]
}

struct Video {
    mpv: mpv::Mpv,
    frames: u64,
    /// 本秒内每帧的软件渲染耗时（含 alpha 修正），ms。
    render_ms: Vec<f32>,
    size: (u32, u32),
}

fn start_video() -> Option<Video> {
    let video = env("SPIKE_VIDEO")?;
    let hwdec = env("SPIKE_HWDEC").unwrap_or_else(|| "videotoolbox-copy".into());
    let ua = env("SPIKE_UA").unwrap_or_default();
    let referer = env("SPIKE_REFERER").unwrap_or_default();
    // terminal + msg-level：让 mpv 自己的日志进 stderr，devicectl --console 收得到。
    let mut options = vec![
        ("vo", "libmpv"),
        ("hwdec", hwdec.as_str()),
        ("terminal", "yes"),
        ("msg-level", "all=info"),
    ];
    if !ua.is_empty() {
        options.push(("user-agent", ua.as_str()));
    }
    if !referer.is_empty() {
        options.push(("referrer", referer.as_str()));
    }
    let mpv = match mpv::Mpv::new_sw(&options) {
        Ok(mpv) => mpv,
        Err(err) => {
            log(&format!("mpv 起不来：{err}"));
            return None;
        }
    };
    if let Some(audio) = env("SPIKE_AUDIO") {
        // 同 slint-mpv 的教训：audio-files 是路径列表，Unix 上按 `:` 拆，
        // URL 必须用 change-list append 逐项加。
        if let Err(err) = mpv.command(&["change-list", "audio-files", "append", &audio]) {
            log(&format!("外挂音轨设置失败：{err}"));
        }
    }
    if let Err(err) = mpv.command(&["loadfile", &video, "replace"]) {
        log(&format!("loadfile 失败：{err}"));
        return None;
    }
    log(&format!("mpv started, hwdec={hwdec}"));
    Some(Video {
        mpv,
        frames: 0,
        render_ms: Vec::new(),
        size: (0, 0),
    })
}

/// Slint 用 fontique 找系统字体。直接问它：中文有没有字体可用。
fn diagnose_fonts() {
    let mut collection = fontique::Collection::new(fontique::CollectionOptions {
        shared: false,
        system_fonts: true,
    });
    let names: Vec<String> = collection.family_names().map(str::to_owned).collect();
    let cjk: Vec<&String> = names
        .iter()
        .filter(|n| n.contains("PingFang") || n.contains("Hiragino") || n.contains("Heiti") || n.contains("Song"))
        .collect();
    log(&format!("fonts: {} families; CJK-like: {cjk:?}", names.len()));
    log(&format!("fonts sample: {:?}", names.iter().take(12).collect::<Vec<_>>()));
    for name in ["PingFang SC", ".PingFang SC", "PingFangSC-Regular"] {
        log(&format!("family_by_name({name:?}) = {}", collection.family_by_name(name).is_some()));
    }
    let han = fontique::FallbackKey::new(fontique::Script::from_bytes(*b"Hani"), None);
    let ids: Vec<fontique::FamilyId> = collection.fallback_families(han).collect();
    let fallback: Vec<String> = ids
        .into_iter()
        .filter_map(|id| collection.family(id).map(|f| f.name().to_owned()))
        .collect();
    log(&format!("fallback(Hani) = {fallback:?}"));
}

fn run() -> Result<(), slint::PlatformError> {
    log("start");
    diagnose_fonts();
    let ui = AppWindow::new()?;

    // 帧计数放在渲染回调里：定时器跑了不代表画了。
    let frames = Rc::new(Cell::new(0u64));
    {
        let frames = Rc::clone(&frames);
        let mut announced = false;
        ui.window()
            .set_rendering_notifier(move |state, api| match state {
                slint::RenderingState::RenderingSetup if !announced => {
                    log(&format!("rendering setup: {api:?}"));
                    announced = true;
                }
                slint::RenderingState::AfterRendering => frames.set(frames.get() + 1),
                _ => {}
            })
            .map_err(|e| slint::PlatformError::Other(format!("{e:?}")))?;
    }

    let video = Rc::new(RefCell::new(start_video()));
    let bullets = Rc::new(VecModel::<Bullet>::default());
    ui.set_bullets(bullets.clone().into());

    let started = Instant::now();
    let timer = slint::Timer::default();
    {
        let ui = ui.as_weak();
        let frames = Rc::clone(&frames);
        let video = Rc::clone(&video);
        let bullets = Rc::clone(&bullets);
        let ticks = Cell::new(0u64);
        let last = Cell::new((Instant::now(), 0u64, 0u64, cpu_seconds()));
        timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(8),
            move || {
                let Some(ui) = ui.upgrade() else { return };
                ticks.set(ticks.get() + 1);
                let t = started.elapsed().as_secs_f32();
                ui.set_phase(t / 3.0);

                let mut video_ref = video.borrow_mut();
                if let Some(v) = video_ref.as_mut() {
                    for id in v.mpv.drain_events() {
                        match id {
                            mpv::EVENT_FILE_LOADED => log("event FILE_LOADED"),
                            mpv::EVENT_PLAYBACK_RESTART => log("event PLAYBACK_RESTART"),
                            mpv::EVENT_END_FILE => log("event END_FILE"),
                            _ => {}
                        }
                    }
                    if v.mpv.wants_frame() {
                        let w = v.mpv.property("dwidth").and_then(|s| s.parse().ok());
                        let h = v.mpv.property("dheight").and_then(|s| s.parse().ok());
                        if let (Some(w), Some(h)) = (w, h) {
                            let t0 = Instant::now();
                            let mut buffer = SharedPixelBuffer::<Rgba8Pixel>::new(w, h);
                            let bytes = buffer.make_mut_bytes();
                            match v.mpv.render_sw(w, h, bytes) {
                                Ok(()) => {
                                    // rgb0 的第 4 字节是未初始化的垃圾：当 alpha 用会
                                    // 让画面半透明甚至全透明，得逐像素改成 255。
                                    for px in bytes.chunks_exact_mut(4) {
                                        px[3] = 255;
                                    }
                                    ui.set_video(Image::from_rgba8(buffer));
                                    ui.set_has_video(true);
                                    v.frames += 1;
                                    v.size = (w, h);
                                    v.render_ms.push(t0.elapsed().as_secs_f32() * 1000.0);
                                }
                                Err(err) => log(&err),
                            }
                        }
                    }
                }

                // 合成弹幕：12 行，从右往左匀速走，只为看叠加层的帧率。
                let size = ui.window().size();
                let scale = ui.window().scale_factor();
                let width = size.width as f32 / scale;
                if bullets.row_count() == 0 {
                    for i in 0..12 {
                        bullets.push(Bullet {
                            text: format!("合成弹幕 {i} 前方高能 🎉").into(),
                            x: 0.0,
                            y: 140.0 + i as f32 * 34.0,
                        });
                    }
                }
                for i in 0..bullets.row_count() {
                    let speed = 120.0 + (i % 4) as f32 * 40.0;
                    let span = width + 320.0;
                    let x = width - ((t * speed + i as f32 * 97.0) % span);
                    let mut b = bullets.row_data(i).unwrap_or_default();
                    b.x = x;
                    bullets.set_row_data(i, b);
                }

                let (since, frames_then, video_then, cpu_then) = last.get();
                if since.elapsed() >= Duration::from_secs(1) {
                    let secs = since.elapsed().as_secs_f32();
                    let now_frames = frames.get();
                    let fps = (now_frames - frames_then) as f32 / secs;
                    let cpu_now = cpu_seconds();
                    let cpu = (cpu_now - cpu_then) / secs as f64 * 100.0;
                    let mut line = format!(
                        "ui {fps:.1} fps · CPU {cpu:.0}% · {}x{} 物理 scale {scale:.2}",
                        size.width, size.height
                    );
                    let mut video_frames = video_then;
                    if let Some(v) = video_ref.as_mut() {
                        video_frames = v.frames;
                        let vfps = (v.frames - video_then) as f32 / secs;
                        let mut ms = std::mem::take(&mut v.render_ms);
                        ms.sort_by(f32::total_cmp);
                        line.push_str(&format!(
                            " | video {vfps:.1} fps {}x{} · sw 渲染 p50 {:.1} / p95 {:.1} ms · hwdec {} · {} · pos {}",
                            v.size.0,
                            v.size.1,
                            percentile(&ms, 0.5),
                            percentile(&ms, 0.95),
                            v.mpv.property("hwdec-current").unwrap_or_default(),
                            v.mpv.property("video-codec").unwrap_or_default(),
                            v.mpv.property("time-pos").unwrap_or_default(),
                        ));
                    }
                    log(&line);
                    ui.set_hud(line.into());
                    last.set((Instant::now(), now_frames, video_frames, cpu_now));
                }
            },
        );
    }

    ui.run()
}
