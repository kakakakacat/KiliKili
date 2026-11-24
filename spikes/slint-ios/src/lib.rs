//! SPIKE-004（issue #11）iOS 实验台。
//!
//! 入口是 [`slint_ios_main`]，由 `ios/main.c` 的 `main()` 调用。iOS 上
//! winit 的事件循环会接管 `UIApplicationMain`，**不会返回**。
//!
//! 参数走环境变量（`devicectl device process launch --environment-variables`）：
//! - `SPIKE_VIDEO` / `SPIKE_AUDIO`（DASH 两路 URL）、`SPIKE_UA`、`SPIKE_REFERER`；
//!   没有 `SPIKE_VIDEO` 就只跑第 1 步（Slint 能不能起来）。
//! - `SPIKE_MODE`：`gl`（默认，mpv 经 GLES 画进 IOSurface，零拷贝进 Slint）、
//!   `sw`（第 2 步的软件渲染基线）、`avplayer`（对照组，不用 mpv）。
//! - `SPIKE_HWDEC`：默认 gl 模式 `videotoolbox`、sw 模式 `videotoolbox-copy`。
//! - `SPIKE_FLIP`（默认 0，见下面 `flip` 字段的注释）、`SPIKE_GLFINISH`（默认 0，用 glFlush）、
//!   `SPIKE_BG_RENDER`（默认 0：退到后台且没进 PiP 时不再画）、
//!   `SPIKE_PIP`（默认 1；0 = 不装 PiP，用来测纯后台音频）、
//!   `SPIKE_BG_VIDNO`（默认 1：退到后台且没进 PiP 时关掉视频轨，回前台再开）。
//! - `SPIKE_INTERP`（#218）：`mc` / `blend` / `hold`，mpv 的新帧先画进纹理环，再按
//!   `target_time` 排程、插帧后画进零拷贝目标（只在 gl 模式下有效）。配套：
//!   `SPIKE_INTERP_DELAY_MS`（默认 50，同时设 mpv 的 `audio-delay`）、
//!   `SPIKE_INTERP_R`（默认 4）、`SPIKE_INTERP_PRESENT_MS`（默认 8，输出时刻相对
//!   now 的提前量）、`SPIKE_INTERP_FINISH=1`（运动估计前后 glFinish，量 GPU 耗时）。
//! - `SPIKE_MPV_OPTS`：`k=v,k=v`，追加 mpv 选项（调色彩/格式问题时做对照）。
//! - `SPIKE_SCRIPT`：定时动作，`秒=动作;秒=动作…`（秒从启动算）。动作是一条 mpv 命令
//!   （空格分词，如 `seek 30 absolute`、`set speed 2`），或 `quality`：换到
//!   `SPIKE_VIDEO2` / `SPIKE_AUDIO2` 这一对流并从当前位置续播（切清晰度）。

mod interp;
mod mpv;
mod platform;
mod zerocopy;

use std::cell::{Cell, RefCell};
use std::ffi::{CStr, CString, c_int, c_uint, c_void};
use std::rc::Rc;
use std::time::{Duration, Instant};

use slint::wgpu_30::wgpu;
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

/// 进程峰值常驻内存，MiB（Darwin 的 ru_maxrss 单位是字节）。
fn peak_rss_mib() -> f64 {
    // SAFETY: 同 cpu_seconds。
    unsafe {
        let mut usage: libc_rusage = std::mem::zeroed();
        getrusage(0, &mut usage);
        usage.rest[0] as f64 / 1048576.0
    }
}

/// 解析 `SPIKE_SCRIPT`，按时间排好。
fn parse_script(text: &str) -> Vec<(f32, String)> {
    let mut steps: Vec<(f32, String)> = text
        .split(';')
        .filter_map(|step| {
            let (t, action) = step.split_once('=')?;
            Some((t.trim().parse().ok()?, action.trim().to_string()))
        })
        .collect();
    steps.sort_by(|a, b| a.0.total_cmp(&b.0));
    steps
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

/// AVAudioSession 的类别、输出路由、系统音量。「mpv 说 AO 开了但听不到」时，
/// 先确认声音被送到了哪里。
fn audio_session_summary() -> String {
    use objc2::runtime::AnyObject;
    use objc2::{class, msg_send};
    fn ns(obj: *mut AnyObject) -> String {
        if obj.is_null() {
            return "<nil>".into();
        }
        // SAFETY: obj 是 NSObject，description/UTF8String 总是可用。
        unsafe {
            let desc: *mut AnyObject = msg_send![obj, description];
            let utf8: *const std::ffi::c_char = msg_send![desc, UTF8String];
            std::ffi::CStr::from_ptr(utf8)
                .to_string_lossy()
                .into_owned()
        }
    }
    // SAFETY: AVAudioSession 是系统单例，这几个属性都是只读查询。
    unsafe {
        let session: *mut AnyObject = msg_send![class!(AVAudioSession), sharedInstance];
        let category: *mut AnyObject = msg_send![session, category];
        let mode: *mut AnyObject = msg_send![session, mode];
        let volume: f32 = msg_send![session, outputVolume];
        let route: *mut AnyObject = msg_send![session, currentRoute];
        let outputs: *mut AnyObject = msg_send![route, outputs];
        let count: usize = msg_send![outputs, count];
        let mut ports = Vec::new();
        for i in 0..count {
            let port: *mut AnyObject = msg_send![outputs, objectAtIndex: i];
            let kind: *mut AnyObject = msg_send![port, portType];
            let name: *mut AnyObject = msg_send![port, portName];
            ports.push(format!("{}({})", ns(kind), ns(name)));
        }
        format!(
            "session category={} mode={} 系统音量={volume:.2} 输出={ports:?}",
            ns(category),
            ns(mode)
        )
    }
}

fn percentile(sorted: &[f32], p: f32) -> f32 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = ((sorted.len() - 1) as f32 * p).round() as usize;
    sorted[idx]
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Mode {
    Sw,
    Gl,
    AvPlayer,
}

/// 一张零拷贝目标：mpv 画进 `fbo`，Slint 采样 `image`，PiP 吃 `pixbuf`——三者是同一块 IOSurface。
struct Target {
    fbo: u32,
    pixbuf: *mut c_void,
    image: Image,
}

/// 轮转的目标张数。只有一张时 mpv 写的同时 Slint/PiP 可能正在读，会撕裂；
/// 三张足够让「正在显示的」和「正在写的」错开。
const TARGETS: usize = 3;

struct Video {
    mode: Mode,
    mpv: Option<mpv::Mpv>,
    frames: u64,
    /// 本秒内每帧的「出一帧」耗时，ms：sw 是软件渲染 + alpha 修正；gl 是
    /// mpv 渲染 + glFlush（UI 线程实际被占用的时间）；avplayer 是导入纹理。
    render_ms: Vec<f32>,
    size: (u32, u32),
    targets: Vec<Target>,
    next: usize,
    flip: bool,
    finish: bool,
    bg_render: bool,
    skipped_bg: u64,
    bg_vidno: bool,
    vid_off: bool,
    interp: Option<Interp>,
}

/// #218 插帧状态。GPU 侧是 slint-mpv 那份原样复用的代码（见 src/interp.rs）。
struct Interp {
    mode: interp::gpu::Mode,
    params: interp::gpu::Params,
    gpu: Option<interp::gpu::Gpu>,
    ring: interp::sched::Ring,
    delay_us: i64,
    present_us: i64,
    measure: bool,
    video_size: (u32, u32),
    since_size: u32,
    /// 每秒清零的统计。
    picks: [u64; 4],
    me_ms: Vec<f32>,
    src_frames: u64,
}

impl Interp {
    fn from_env() -> Option<Self> {
        let mode = match env("SPIKE_INTERP")?.as_str() {
            "mc" => interp::gpu::Mode::Mc,
            "blend" => interp::gpu::Mode::Blend,
            "hold" => interp::gpu::Mode::Hold,
            _ => return None,
        };
        let num =
            |name: &str, default: i64| env(name).and_then(|v| v.parse().ok()).unwrap_or(default);
        let params = interp::gpu::Params {
            radius: num("SPIKE_INTERP_R", 4) as i32,
            ..interp::gpu::Params::default()
        };
        Some(Self {
            mode,
            params,
            gpu: None,
            ring: interp::sched::Ring::new(8),
            delay_us: num("SPIKE_INTERP_DELAY_MS", 50) * 1000,
            present_us: num("SPIKE_INTERP_PRESENT_MS", 8) * 1000,
            measure: env("SPIKE_INTERP_FINISH").is_some_and(|v| v == "1"),
            video_size: (0, 0),
            since_size: 0,
            picks: [0; 4],
            me_ms: Vec::new(),
            src_frames: 0,
        })
    }

    fn summary(&mut self) -> String {
        let [pair, starved, early, empty] = std::mem::take(&mut self.picks);
        let mut me = std::mem::take(&mut self.me_ms);
        me.sort_by(f32::total_cmp);
        let src = std::mem::take(&mut self.src_frames);
        format!(
            " · interp {:?} 源帧 {src} · pair {pair} starved {starved} early {early} empty {empty}{}",
            self.mode,
            if me.is_empty() {
                String::new()
            } else {
                format!(
                    " · ME(finish) p50 {:.2} / p95 {:.2} ms",
                    percentile(&me, 0.5),
                    percentile(&me, 0.95)
                )
            },
        )
    }
}

fn start_video(mode: Mode) -> Option<Video> {
    let video = env("SPIKE_VIDEO")?;
    let ua = env("SPIKE_UA").unwrap_or_default();
    let referer = env("SPIKE_REFERER").unwrap_or_default();
    let mut v = Video {
        mode,
        mpv: None,
        frames: 0,
        render_ms: Vec::new(),
        size: (0, 0),
        targets: Vec::new(),
        next: 0,
        // **iOS 上不翻转**，和 Windows / Android / macOS 三端相反（那三端都要
        // flip_y）：那边是 femtovg 在 GL 里采样 GL 纹理，GL 的纹理原点在左下；
        // 这里 mpv 画进 IOSurface，Slint 经 Metal 采样，Metal 的原点在左上，
        // 两次「左下」约定抵消了。真机截图对照过：flip=1 时字幕倒立在画面顶部。
        flip: env("SPIKE_FLIP").is_some_and(|v| v == "1"),
        finish: env("SPIKE_GLFINISH").is_some_and(|v| v == "1"),
        bg_render: env("SPIKE_BG_RENDER").is_some_and(|v| v == "1"),
        skipped_bg: 0,
        bg_vidno: env("SPIKE_BG_VIDNO").is_none_or(|v| v != "0"),
        vid_off: false,
        interp: if mode == Mode::Gl {
            Interp::from_env()
        } else {
            None
        },
    };
    // `~/` 开头的路径按 App 容器展开（视频用 devicectl copy 推进 Documents）。
    let video = match video.strip_prefix("~/") {
        Some(rest) => format!("{}/{rest}", std::env::var("HOME").unwrap_or_default()),
        None => video,
    };
    if mode == Mode::AvPlayer {
        let c = |s: &str| CString::new(s).unwrap_or_default();
        let (vc, ac, uc, rc) = (
            c(&video),
            c(&env("SPIKE_AUDIO").unwrap_or_default()),
            c(&ua),
            c(&referer),
        );
        // SAFETY: 四个 C 字符串在调用期间有效，bridge.m 会自行复制。
        unsafe { platform::spike_avp_start(vc.as_ptr(), ac.as_ptr(), uc.as_ptr(), rc.as_ptr()) };
        return Some(v);
    }
    let default_hwdec = if mode == Mode::Gl {
        "videotoolbox"
    } else {
        "videotoolbox-copy"
    };
    let hwdec = env("SPIKE_HWDEC").unwrap_or_else(|| default_hwdec.into());
    // terminal + msg-level：让 mpv 自己的日志进 stderr，devicectl --console 收得到。
    let mut options = vec![
        ("vo", "libmpv"),
        ("hwdec", hwdec.as_str()),
        ("terminal", "yes"),
        ("msg-level", "all=info"),
    ];
    if mode == Mode::Gl {
        // VideoToolbox 默认吐 nv12，mpv 的 iOS 零拷贝 interop（CVOpenGLESTextureCache）
        // 采样第二个平面时 V 读成了 U：肤色发绿、蓝色发紫（2026-11-24 真机对照 AVPlayer
        // 与软解确认）。让 VT 直接出 BGRA：单平面、仍是零拷贝，颜色与 AVPlayer 一致。
        options.push(("hwdec-image-format", "bgra"));
    }
    // SPIKE_MPV_OPTS=k=v,k=v：临时加 mpv 选项做对照（如 fbo-format、cscale）。
    let extra = env("SPIKE_MPV_OPTS").unwrap_or_default();
    options.extend(extra.split(',').filter_map(|kv| kv.split_once('=')));
    // 插帧把画面推迟 delay，音频跟着推迟同样多（#218 的 macOS / Android 结论）。
    let audio_delay = v
        .interp
        .as_ref()
        .map(|i| format!("{}", i.delay_us as f64 / 1e6));
    if let Some(delay) = audio_delay.as_deref() {
        options.push(("audio-delay", delay));
    }
    if !ua.is_empty() {
        options.push(("user-agent", ua.as_str()));
    }
    if !referer.is_empty() {
        options.push(("referrer", referer.as_str()));
    }
    let api = if mode == Mode::Gl {
        // SAFETY: 只建上下文并设为当前。
        let r = unsafe { platform::spike_gl_init() };
        if r != 0 {
            log(&format!("EAGLContext 起不来：{r}"));
            return None;
        }
        mpv::RenderApi::OpenGl(platform::spike_gl_proc)
    } else {
        mpv::RenderApi::Sw
    };
    let mpv = match mpv::Mpv::new(&options, api) {
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
    log(&format!("mpv started, mode={mode:?} hwdec={hwdec}"));
    v.mpv = Some(mpv);
    Some(v)
}

impl Video {
    fn ensure_targets(
        &mut self,
        w: u32,
        h: u32,
        importer: &zerocopy::Importer,
    ) -> Result<(), String> {
        if self.size == (w, h) && self.targets.len() == TARGETS {
            return Ok(());
        }
        self.targets.clear();
        for _ in 0..TARGETS {
            let mut fbo: c_uint = 0;
            // SAFETY: EAGLContext 已是当前；返回的目标永不释放（spike 进程级生命周期）。
            let target = unsafe { platform::spike_target_create(w as c_int, h as c_int, &mut fbo) };
            if target.is_null() {
                return Err("spike_target_create 失败".into());
            }
            // SAFETY: target 有效。
            let pixbuf = unsafe { platform::spike_target_pixbuf(target) };
            let image = importer.import(pixbuf)?;
            self.targets.push(Target { fbo, pixbuf, image });
        }
        self.size = (w, h);
        log(&format!("zero-copy targets ready: {TARGETS} × {w}x{h}"));
        Ok(())
    }

    /// 出一帧，返回要交给 Slint 的图和要喂给 PiP 的 CVPixelBuffer。
    fn frame(
        &mut self,
        importer: Option<&zerocopy::Importer>,
        may_render: bool,
    ) -> Option<(Image, *mut c_void)> {
        match self.mode {
            Mode::Sw => {
                let mpv = self.mpv.as_ref()?;
                if !mpv.wants_frame() {
                    return None;
                }
                let w = mpv.property("dwidth")?.parse().ok()?;
                let h = mpv.property("dheight")?.parse().ok()?;
                let t0 = Instant::now();
                let mut buffer = SharedPixelBuffer::<Rgba8Pixel>::new(w, h);
                let bytes = buffer.make_mut_bytes();
                if let Err(err) = mpv.render_sw(w, h, bytes) {
                    log(&err);
                    return None;
                }
                // rgb0 的第 4 字节是未初始化的垃圾：当 alpha 用会
                // 让画面半透明甚至全透明，得逐像素改成 255。
                for px in bytes.as_chunks_mut::<4>().0 {
                    px[3] = 255;
                }
                self.render_ms.push(t0.elapsed().as_secs_f32() * 1000.0);
                self.size = (w, h);
                Some((Image::from_rgba8(buffer), std::ptr::null_mut()))
            }
            Mode::Gl if self.interp.is_some() => self.frame_interp(importer?, may_render),
            Mode::Gl => {
                let importer = importer?;
                let mpv = self.mpv.as_ref()?;
                if !mpv.wants_frame() {
                    return None;
                }
                // 退到后台时 iOS 不许 GLES 提交命令（违者被系统以
                // gpus_ReturnNotPermittedKillClient 杀掉）。这里先按「后台不画」
                // 处理，PiP 期间是否允许画由 SPIKE_BG_RENDER / PiP 状态决定，见 README。
                if !may_render {
                    self.skipped_bg += 1;
                    return None;
                }
                let w: u32 = mpv.property("dwidth")?.parse().ok()?;
                let h: u32 = mpv.property("dheight")?.parse().ok()?;
                // SAFETY: 主线程，同一个 EAGLContext。
                unsafe { platform::spike_gl_make_current() };
                if let Err(err) = self.ensure_targets(w, h, importer) {
                    log(&err);
                    return None;
                }
                let mpv = self.mpv.as_ref()?;
                let target = &self.targets[self.next % TARGETS];
                let t0 = Instant::now();
                if let Err(err) = mpv.render_gl(target.fbo, w, h, self.flip) {
                    log(&err);
                    return None;
                }
                // glFlush 足够让同一 GPU 上的 Metal 看到 IOSurface 的新内容
                // （驱动按 IOSurface 做跨 API 同步）；SPIKE_GLFINISH=1 用来对照。
                // SAFETY: 上下文是当前的。
                unsafe { platform::spike_gl_flush(self.finish.into()) };
                self.render_ms.push(t0.elapsed().as_secs_f32() * 1000.0);
                let out = (target.image.clone(), target.pixbuf);
                self.next += 1;
                Some(out)
            }
            Mode::AvPlayer => {
                let importer = importer?;
                // SAFETY: 返回 +1 的 CVPixelBufferRef 或 NULL。
                let pixbuf = unsafe { platform::spike_avp_copy_frame() };
                if pixbuf.is_null() {
                    return None;
                }
                let t0 = Instant::now();
                let image = importer.import(pixbuf);
                self.render_ms.push(t0.elapsed().as_secs_f32() * 1000.0);
                let size = image.as_ref().map(|i| i.size()).unwrap_or_default();
                self.size = (size.width, size.height);
                // PiP 要在 release 之前喂：显示层会自己 retain。
                // SAFETY: pixbuf 有效。
                unsafe { platform::spike_pip_enqueue(pixbuf) };
                // spike_mtl_texture 已经 retain 了 pixbuf，这里放掉自己那份。
                // SAFETY: 配对 copy 的 +1。
                unsafe { platform::spike_cf_release(pixbuf) };
                match image {
                    Ok(image) => Some((image, std::ptr::null_mut())),
                    Err(err) => {
                        log(&err);
                        None
                    }
                }
            }
        }
    }

    /// #218：mpv 的新帧画进纹理环；每个 tick 选一对源帧、插一张画进零拷贝目标。
    fn frame_interp(
        &mut self,
        importer: &zerocopy::Importer,
        may_render: bool,
    ) -> Option<(Image, *mut c_void)> {
        use interp::gpu::{Gpu, Mode as IMode};
        use interp::sched::Pick;
        if !may_render {
            self.skipped_bg += 1;
            return None;
        }
        // SAFETY: 主线程，同一个 EAGLContext。
        unsafe { platform::spike_gl_make_current() };
        let flip = self.flip;
        let t0 = Instant::now();
        {
            let it = self.interp.as_mut()?;
            if it.gpu.is_none() {
                let loader = |name: &CStr| -> *const c_void {
                    // SAFETY: 名字是 NUL 结尾的 C 串。
                    unsafe { platform::spike_gl_proc(std::ptr::null_mut(), name.as_ptr()) }
                        .cast_const()
                };
                // SAFETY: EAGLContext 是当前的。
                match unsafe { Gpu::new(&loader, it.ring.capacity()) } {
                    Ok(gpu) => {
                        log(&format!("interp GPU：{}", gpu.renderer()));
                        it.gpu = Some(gpu);
                    }
                    Err(err) => {
                        log(&format!("interp GPU 初始化失败：{err:#}"));
                        self.interp = None;
                        return None;
                    }
                }
            }
        }
        let mpv = self.mpv.as_ref()?;
        let it = self.interp.as_mut()?;
        let gpu = it.gpu.as_mut()?;
        let now = mpv.time_us();

        // 1. 收帧。尺寸每 30 张新帧才同步问一次 mpv（Android 上每帧同步取属性
        //    会让渲染线程等 mpv 的 core 线程，掉 8 fps）。
        for _ in 0..8 {
            if !mpv.wants_frame() {
                break;
            }
            let info = mpv.next_frame_info();
            if it.video_size.0 == 0 || it.since_size >= 30 {
                it.since_size = 0;
                if let (Some(w), Some(h)) = (
                    mpv.property("dwidth").and_then(|v| v.parse().ok()),
                    mpv.property("dheight").and_then(|v| v.parse().ok()),
                ) {
                    it.video_size = (w, h);
                }
            }
            it.since_size += 1;
            let (w, h) = it.video_size;
            if w == 0 {
                break;
            }
            match gpu.ensure_size(w as i32, h as i32) {
                Ok(true) => it.ring.clear(),
                Ok(false) => {}
                Err(err) => {
                    log(&format!("interp ensure_size：{err:#}"));
                    return None;
                }
            }
            let redraw = info.flags & mpv::FRAME_INFO_REDRAW != 0 || info.target_time == 0;
            let slot = match (redraw, it.ring.newest()) {
                (true, Some(newest)) => newest.slot,
                _ => it.ring.slot_for_next(),
            };
            if let Err(err) = mpv.render_gl(gpu.slot_fbo(slot) as u32, w, h, flip) {
                log(&err);
                return None;
            }
            gpu.prepare_slot(slot);
            if redraw && it.ring.newest().is_some() {
                continue;
            }
            // target_time 的单位按量级判（mpv 0.42 实测是 ns）。
            let t = if info.target_time == 0 {
                now
            } else if (info.target_time / 1000 - now).abs() < (info.target_time - now).abs() {
                info.target_time / 1000
            } else {
                info.target_time
            };
            it.ring.push(t, slot);
            it.src_frames += 1;
        }

        // 2. 选帧并输出到下一张零拷贝目标。
        let (w, h) = it.video_size;
        if w == 0 {
            return None;
        }
        let t_out = now + it.present_us - it.delay_us;
        let pick = it.ring.pick(t_out, false);
        let (a, b, p, mode) = match pick {
            Pick::Pair { a, b, p } => {
                it.picks[0] += 1;
                (a, b, p, it.mode)
            }
            Pick::Starved { a } => {
                it.picks[1] += 1;
                (a, a, 0.0, IMode::Hold)
            }
            Pick::Early { a } => {
                it.picks[2] += 1;
                (a, a, 0.0, IMode::Hold)
            }
            Pick::Empty => {
                it.picks[3] += 1;
                return None;
            }
        };
        if mode == IMode::Mc {
            if it.measure {
                gpu.finish();
            }
            let t = Instant::now();
            gpu.ensure_field(&a, &b, &it.params);
            if it.measure {
                gpu.finish();
                // 同一对的后续输出直接复用运动场，耗时≈0，不算进分布。
                let ms = t.elapsed().as_secs_f32() * 1000.0;
                if ms > 0.05 {
                    it.me_ms.push(ms);
                }
            }
        }
        let _ = gpu.poll();
        // 目标与源帧同尺寸（视频原始尺寸），Slint 负责缩放。
        if let Err(err) = self.ensure_targets(w, h, importer) {
            log(&err);
            return None;
        }
        let it = self.interp.as_mut()?;
        let gpu = it.gpu.as_mut()?;
        let target = &self.targets[self.next % TARGETS];
        gpu.present(
            Gpu::framebuffer(target.fbo as i32),
            (w as i32, h as i32),
            &a,
            &b,
            p,
            mode,
            &it.params,
        );
        // SAFETY: 上下文是当前的。
        unsafe { platform::spike_gl_flush(self.finish.into()) };
        self.render_ms.push(t0.elapsed().as_secs_f32() * 1000.0);
        self.size = (w, h);
        let out = (target.image.clone(), target.pixbuf);
        self.next += 1;
        Some(out)
    }

    /// 退到后台（且没进 PiP）时 iOS 会让 VideoToolbox 会话失效：mpv 收到
    /// `invalid session (-12903)`，一次性退回软解，**回到前台也不会再切回
    /// 硬解**，而且切换那一下卡 2–3 秒（之前「退后台停 2–3 秒」的根因）。
    /// 所以在进后台前就关掉视频轨，只解音频；回前台再打开，VT 会话重建。
    fn on_background(&mut self, background: bool) {
        if !self.bg_vidno || self.mode == Mode::AvPlayer || background == self.vid_off {
            return;
        }
        if let Some(mpv) = &self.mpv {
            let r = mpv.set_property("vid", if background { "no" } else { "auto" });
            log(&format!(
                "vid → {} ({r:?})",
                if background { "no" } else { "auto" }
            ));
            self.vid_off = background;
        }
    }

    fn set_playing(&self, playing: bool) {
        match self.mode {
            // SAFETY: 主线程调用。
            Mode::AvPlayer => unsafe { platform::spike_avp_set_playing(playing.into()) },
            _ => {
                if let Some(mpv) = &self.mpv {
                    let _ = mpv.set_property("pause", if playing { "no" } else { "yes" });
                }
            }
        }
    }

    /// 执行 `SPIKE_SCRIPT` 里的一步。
    fn run_action(&self, action: &str) {
        let Some(mpv) = &self.mpv else { return };
        let pos = mpv.property("time-pos").unwrap_or_default();
        let result = if action == "quality" {
            match (env("SPIKE_VIDEO2"), env("SPIKE_AUDIO2")) {
                (Some(video), audio) => (|| {
                    mpv.set_property("start", &pos)?;
                    // 同 start_video：URL 里有 `:`，只能 clr 再 append。
                    mpv.command(&["change-list", "audio-files", "clr", ""])?;
                    if let Some(audio) = audio {
                        mpv.command(&["change-list", "audio-files", "append", &audio])?;
                    }
                    mpv.command(&["loadfile", &video, "replace"])
                })(),
                (None, _) => Err("没有 SPIKE_VIDEO2".into()),
            }
        } else {
            let args: Vec<&str> = action.split_whitespace().collect();
            mpv.command(&args)
        };
        log(&format!("script「{action}」@pos {pos} → {result:?}"));
    }

    /// (位置, 时长, 暂停)
    fn timeline(&self) -> (f64, f64, bool) {
        match self.mode {
            Mode::AvPlayer => {
                let (mut pos, mut dur, mut status) = (0.0, 0.0, 0);
                // SAFETY: 三个出参都指向栈上变量。
                unsafe { platform::spike_avp_info(&mut pos, &mut dur, &mut status) };
                (pos, if dur.is_finite() { dur } else { 0.0 }, false)
            }
            _ => {
                let Some(mpv) = &self.mpv else {
                    return (0.0, 0.0, false);
                };
                let f = |n: &str| mpv.property(n).and_then(|s| s.parse().ok()).unwrap_or(0.0);
                (
                    f("time-pos"),
                    f("duration"),
                    mpv.property("pause").as_deref() == Some("yes"),
                )
            }
        }
    }

    fn describe(&self) -> String {
        match self.mode {
            Mode::AvPlayer => {
                // SAFETY: 返回静态缓冲区。
                let status = unsafe { CStr::from_ptr(platform::spike_avp_status()) };
                // SAFETY: 主线程读两个播放器的位置。
                let offset = unsafe { platform::spike_avp_av_offset() };
                format!(
                    "avplayer: {} · 音轨-视频 {:+.3}s",
                    status.to_string_lossy(),
                    offset
                )
            }
            _ => {
                let Some(mpv) = &self.mpv else {
                    return String::new();
                };
                let p = |n: &str| mpv.property(n).unwrap_or_default();
                format!(
                    "hwdec {} · {} · speed {} · avsync {} · drop {}/{}",
                    p("hwdec-current"),
                    p("video-codec"),
                    p("speed"),
                    p("avsync"),
                    p("frame-drop-count"),
                    p("decoder-frame-drop-count"),
                )
            }
        }
    }
}

thread_local! {
    /// PiP 窗口上的播放/暂停按钮：回调在主线程，交给定时器去执行。
    static PENDING_PLAY: Cell<Option<bool>> = const { Cell::new(None) };
}

extern "C" fn on_pip_set_playing(playing: c_int) {
    PENDING_PLAY.with(|p| p.set(Some(playing != 0)));
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
        .filter(|n| {
            n.contains("PingFang")
                || n.contains("Hiragino")
                || n.contains("Heiti")
                || n.contains("Song")
        })
        .collect();
    log(&format!(
        "fonts: {} families; CJK-like: {cjk:?}",
        names.len()
    ));
    log(&format!(
        "fonts sample: {:?}",
        names.iter().take(12).collect::<Vec<_>>()
    ));
    for name in ["PingFang SC", ".PingFang SC", "PingFangSC-Regular"] {
        log(&format!(
            "family_by_name({name:?}) = {}",
            collection.family_by_name(name).is_some()
        ));
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
    let mode = match env("SPIKE_MODE").as_deref() {
        Some("sw") => Mode::Sw,
        Some("avplayer") => Mode::AvPlayer,
        _ => Mode::Gl,
    };
    let ui = AppWindow::new()?;
    ui.set_mode(format!("{mode:?}").into());

    // 帧计数放在渲染回调里：定时器跑了不代表画了。
    let frames = Rc::new(Cell::new(0u64));
    // Slint 的 wgpu 设备，RenderingSetup 时才拿得到；零拷贝导入要用它。
    let device: Rc<RefCell<Option<wgpu::Device>>> = Rc::default();
    {
        let frames = Rc::clone(&frames);
        let device = Rc::clone(&device);
        ui.window()
            .set_rendering_notifier(move |state, api| match state {
                slint::RenderingState::RenderingSetup => {
                    log(&format!("rendering setup: {api:?}"));
                    if let slint::GraphicsAPI::WGPU30 { device: d, .. } = api {
                        *device.borrow_mut() = Some(d.clone());
                    }
                }
                slint::RenderingState::AfterRendering => frames.set(frames.get() + 1),
                _ => {}
            })
            .map_err(|e| slint::PlatformError::Other(format!("{e:?}")))?;
    }

    let video = Rc::new(RefCell::new(start_video(mode)));
    let importer: Rc<RefCell<Option<zerocopy::Importer>>> = Rc::default();
    let bullets = Rc::new(VecModel::<Bullet>::default());
    ui.set_bullets(bullets.clone().into());
    let pip_ready = Rc::new(Cell::new(false));
    let pip_wanted = env("SPIKE_PIP").is_none_or(|v| v != "0");
    {
        let pip_ready = Rc::clone(&pip_ready);
        ui.on_toggle_pip(move || {
            if pip_ready.get() {
                // SAFETY: 主线程。
                unsafe { platform::spike_pip_toggle() };
            }
        });
    }

    let script = RefCell::new(parse_script(&env("SPIKE_SCRIPT").unwrap_or_default()));
    let started = Instant::now();
    let timer = slint::Timer::default();
    {
        let ui = ui.as_weak();
        let frames = Rc::clone(&frames);
        let video = Rc::clone(&video);
        let bullets = Rc::clone(&bullets);
        let ticks = Cell::new(0u64);
        let last = Cell::new((Instant::now(), 0u64, 0u64, cpu_seconds()));
        let app_state = Cell::new(0);
        timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(8),
            move || {
                let Some(ui) = ui.upgrade() else { return };
                ticks.set(ticks.get() + 1);
                let t = started.elapsed().as_secs_f32();
                ui.set_phase(t / 3.0);

                if importer.borrow().is_none()
                    && let Some(d) = device.borrow().clone()
                {
                    let made = zerocopy::Importer::new(d);
                    log(&format!("zero-copy importer: {}", if made.is_some() { "Metal OK" } else { "不是 Metal 后端" }));
                    *importer.borrow_mut() = made;
                }
                // PiP 要挂在窗口的视图树上，等第一帧画出来以后再装。
                if !pip_ready.get() && frames.get() > 5 && mode != Mode::Sw && pip_wanted {
                    // SAFETY: 主线程；回调是 'static 的 extern "C" fn。
                    let r = unsafe { platform::spike_pip_setup(on_pip_set_playing) };
                    log(&format!("pip setup: {r}"));
                    pip_ready.set(r == 0);
                }

                // SAFETY: 主线程读 UIApplication 状态。
                let state = unsafe { platform::spike_app_state() };
                let pip_active = pip_ready.get() && unsafe { platform::spike_pip_active() } != 0;
                if state != app_state.replace(state) {
                    log(&format!("app state → {state}（0 前台 1 过渡 2 后台）· pip_active={pip_active}"));
                }

                let mut video_ref = video.borrow_mut();
                if let Some(v) = video_ref.as_mut() {
                    if let Some(playing) = PENDING_PLAY.with(|p| p.take()) {
                        v.set_playing(playing);
                    }
                    let due = script.borrow().first().is_some_and(|(at, _)| t >= *at);
                    if due {
                        let (_, action) = script.borrow_mut().remove(0);
                        v.run_action(&action);
                    }
                    if let Some(mpv) = &v.mpv {
                        for id in mpv.drain_events() {
                            match id {
                                mpv::EVENT_FILE_LOADED => log("event FILE_LOADED"),
                                mpv::EVENT_PLAYBACK_RESTART => log(&format!(
                                    "event PLAYBACK_RESTART @t {t:.2} pos {}",
                                    mpv.property("time-pos").unwrap_or_default()
                                )),
                                mpv::EVENT_END_FILE => log("event END_FILE"),
                                _ => {}
                            }
                        }
                    }
                    // 状态 1（过渡）也算：上滑回桌面时先到 1，此刻关视频轨能赶在 VT 失效前。
                    // 但 PiP 是在过渡期间才启动的，所以 PiP 可用时等到 2 再判断。
                    let leaving = if pip_ready.get() { state == 2 } else { state != 0 };
                    v.on_background(leaving && !pip_active);
                    let may_render = state == 0 || pip_active || v.bg_render;
                    if let Some((image, pixbuf)) = v.frame(importer.borrow().as_ref(), may_render) {
                        ui.set_video(image);
                        ui.set_has_video(true);
                        v.frames += 1;
                        if !pixbuf.is_null() && pip_ready.get() {
                            // SAFETY: pixbuf 属于常驻的零拷贝目标。
                            unsafe { platform::spike_pip_enqueue(pixbuf) };
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
                        "[{mode:?}] ui {fps:.1} fps · CPU {cpu:.0}% · RSS峰 {:.0} MiB · {}x{} scale {scale:.2} · app {state} · pip {}",
                        peak_rss_mib(),
                        size.width,
                        size.height,
                        if !pip_ready.get() {
                            "-".to_string()
                        } else {
                            // SAFETY: 主线程。
                            unsafe {
                                format!(
                                    "possible={} active={} layer={}",
                                    platform::spike_pip_possible(),
                                    pip_active as u8,
                                    platform::spike_pip_layer_status()
                                )
                            }
                        }
                    );
                    let mut video_frames = video_then;
                    if let Some(v) = video_ref.as_mut() {
                        video_frames = v.frames;
                        let vfps = (v.frames - video_then) as f32 / secs;
                        let mut ms = std::mem::take(&mut v.render_ms);
                        ms.sort_by(f32::total_cmp);
                        let (pos, dur, paused) = v.timeline();
                        if pip_ready.get() {
                            // SAFETY: 主线程。
                            unsafe { platform::spike_pip_timeline(pos, dur, paused.into()) };
                        }
                        line.push_str(&format!(
                            " | video {vfps:.1} fps {}x{} · 出帧 p50 {:.2} / p95 {:.2} ms · {} · pos {pos:.1}/{dur:.0}{}{}",
                            v.size.0,
                            v.size.1,
                            percentile(&ms, 0.5),
                            percentile(&ms, 0.95),
                            v.describe(),
                            if paused { " 暂停" } else { "" },
                            if v.skipped_bg > 0 { format!(" · 后台跳过 {} 帧", v.skipped_bg) } else { String::new() },
                        ));
                    }
                    if let Some(it) = video_ref.as_mut().and_then(|v| v.interp.as_mut()) {
                        line.push_str(&it.summary());
                    }
                    log(&line);
                    if ticks.get() % 600 < 130 {
                        log(&audio_session_summary());
                    }
                    ui.set_hud(line.into());
                    last.set((Instant::now(), now_frames, video_frames, cpu_now));
                }
            },
        );
    }

    ui.run()
}
