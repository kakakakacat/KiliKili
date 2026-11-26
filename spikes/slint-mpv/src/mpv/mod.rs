//! `NativeMpvAdapter` 原型：把 libmpv 包成 ADR-003 的 [`PlayerPort`]。
//!
//! ADR-003 的「验证要求 5」是「同一套契约在 Flutter 适配器和原生适配器上
//! 行为一致」。这个文件就是原生那一侧的第一版，spike 结束后它要么长成
//! Phase 2 的正式适配器，要么连同结论一起被删掉。
//!
//! ## 代际是怎么做的（这是本文件唯一不平凡的地方）
//!
//! 契约要求 seek 之后不得采纳旧代际的事件。难点在于：**bump 代际的是
//! 调用方，而旧事件还在 mpv 的队列里**。如果事件泵一律用「当前代际」
//! 盖章，那些旧事件会被盖成新代际，闸门就白做了。
//!
//! 办法是用 mpv 自己的确认点分段：
//!
//! - 发出 Seek/Open 时，bump 闸门得到 `target`，但事件泵**继续用旧代际
//!   盖章**（于是旧快照到达时会被 `Stamped::accept` 丢掉）；
//! - 等到 mpv 报 `PLAYBACK_RESTART`（seek）或 `FILE_LOADED`（open）——
//!   这是后端真正完成了切换的事实——事件泵才切到 `target`，并发出
//!   `SeekCompleted`。
//!
//! 「发了 Seek 不等于 seek 完成了」这句话在 ADR 里是一行文字，在这里是
//! 这段状态机。

pub mod sys;

use std::ffi::{CString, c_char, c_int, c_void};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use bili_domain::player::{
    Lifecycle, MediaSource, PlaybackEvent, PlaybackState, PlayerCommand, PlayerEventSink,
    PlayerFailureKind, PlayerPort, PlayerSnapshot, TrackId,
};
use bili_domain::session::{Generation, GenerationGate, Stamped};

use sys::{MpvEvent, MpvEventProperty, MpvHandle, MpvLib, MpvRenderContext, cstr, event, format};

/// mpv handle 按官方文档是线程安全的（可从多个线程调用），所以这里允许它
/// 跨线程。`PlayerPort: Send` 要求的也正是这个。
#[derive(Debug, Clone, Copy)]
struct HandlePtr(*mut MpvHandle);
unsafe impl Send for HandlePtr {}
unsafe impl Sync for HandlePtr {}

#[derive(Debug, Clone)]
pub struct MpvConfig {
    /// `auto-safe` / `no` / `d3d11va` / `videotoolbox` …
    pub hwdec: String,
    pub user_agent: Option<String>,
    pub referer: Option<String>,
    /// mpv 自己的日志落盘，排查用。
    pub log_file: Option<String>,
    pub volume: i64,
    pub loop_file: bool,
    /// 额外的 mpv 选项，按顺序在 initialize 之前设置（#218 插帧 spike 用它
    /// 改 `video-sync`、`video-timing-offset`、`interpolation` 等）。
    pub extra: Vec<(String, String)>,
}

impl Default for MpvConfig {
    fn default() -> Self {
        Self {
            hwdec: "auto-safe".to_owned(),
            user_agent: None,
            referer: None,
            log_file: None,
            volume: 50,
            loop_file: false,
            extra: Vec::new(),
        }
    }
}

#[derive(Debug)]
struct GenState {
    /// 事件泵当前盖的章。
    emit: Generation,
    /// 后端确认切换之后要切到的章。
    target: Generation,
    awaiting_restart: bool,
    awaiting_load: bool,
}

pub struct MpvPlayer {
    lib: Arc<MpvLib>,
    handle: HandlePtr,
    gate: GenerationGate,
    gen_state: Arc<Mutex<GenState>>,
    stop: Arc<AtomicBool>,
    pump: Option<std::thread::JoinHandle<()>>,
}

impl std::fmt::Debug for MpvPlayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MpvPlayer").finish_non_exhaustive()
    }
}

impl MpvPlayer {
    pub fn new(
        lib: Arc<MpvLib>,
        config: &MpvConfig,
        gate: GenerationGate,
        sink: Box<dyn PlayerEventSink>,
    ) -> Result<Self> {
        let handle = unsafe { (lib.create)() };
        if handle.is_null() {
            bail!("mpv_create 返回空指针");
        }
        let handle = HandlePtr(handle);

        // 必须在 initialize 之前设：render API 要求 vo=libmpv。
        let mut options: Vec<(&str, String)> = vec![
            ("vo", "libmpv".to_owned()),
            ("hwdec", config.hwdec.clone()),
            ("terminal", "no".to_owned()),
            ("idle", "yes".to_owned()),
            // 关掉 OSD/字幕自动加载：spike 要量的是视频 + 弹幕层，
            // 多一层 mpv 自己画的文字会污染帧耗时。
            ("osd-level", "0".to_owned()),
            ("sub-auto", "no".to_owned()),
            ("volume", config.volume.to_string()),
        ];
        if config.loop_file {
            options.push(("loop-file", "inf".to_owned()));
        }
        if let Some(agent) = &config.user_agent {
            options.push(("user-agent", agent.clone()));
        }
        if let Some(referer) = &config.referer {
            options.push(("referrer", referer.clone()));
        }
        if let Some(path) = &config.log_file {
            options.push(("log-file", path.clone()));
            options.push(("msg-level", "all=v".to_owned()));
        }
        for (key, value) in &config.extra {
            options.push((key.as_str(), value.clone()));
        }
        for (key, value) in &options {
            let k = cstr(key)?;
            let v = cstr(value)?;
            let code = unsafe { (lib.set_option_string)(handle.0, k.as_ptr(), v.as_ptr()) };
            lib.check(code, &format!("设置 mpv 选项 {key}={value}"))?;
        }

        let code = unsafe { (lib.initialize)(handle.0) };
        lib.check(code, "mpv_initialize")?;

        for (id, name, fmt) in OBSERVED {
            let n = cstr(name)?;
            let code = unsafe { (lib.observe_property)(handle.0, *id, n.as_ptr(), *fmt) };
            lib.check(code, &format!("observe_property {name}"))?;
        }

        let generation = gate.current();
        let gen_state = Arc::new(Mutex::new(GenState {
            emit: generation,
            target: generation,
            awaiting_restart: false,
            awaiting_load: false,
        }));
        let stop = Arc::new(AtomicBool::new(false));

        let pump = {
            let lib = Arc::clone(&lib);
            let gen_state = Arc::clone(&gen_state);
            let stop = Arc::clone(&stop);
            let session = gate.session();
            std::thread::Builder::new()
                .name("mpv-events".to_owned())
                .spawn(move || {
                    let mut sink = sink;
                    pump_events(&lib, handle, session, &gen_state, &stop, sink.as_mut());
                })
                .context("起不来 mpv 事件线程")?
        };

        Ok(Self {
            lib,
            handle,
            gate,
            gen_state,
            stop,
            pump: Some(pump),
        })
    }

    /// 读一个 string 属性（`hwdec-current`、`video-codec` 之类）。
    #[must_use]
    pub fn property_string(&self, name: &str) -> Option<String> {
        let key = cstr(name).ok()?;
        let mut out: *mut c_char = std::ptr::null_mut();
        let code = unsafe {
            (self.lib.get_property)(
                self.handle.0,
                key.as_ptr(),
                format::STRING,
                (&raw mut out).cast::<c_void>(),
            )
        };
        if code < 0 || out.is_null() {
            return None;
        }
        let value = unsafe { std::ffi::CStr::from_ptr(out) }
            .to_string_lossy()
            .into_owned();
        unsafe { (self.lib.free)(out.cast::<c_void>()) };
        Some(value)
    }

    #[must_use]
    #[allow(dead_code)] // #218 的 interp 可执行文件不用
    pub fn property_f64(&self, name: &str) -> Option<f64> {
        let key = cstr(name).ok()?;
        let mut out: f64 = 0.0;
        let code = unsafe {
            (self.lib.get_property)(
                self.handle.0,
                key.as_ptr(),
                format::DOUBLE,
                (&raw mut out).cast::<c_void>(),
            )
        };
        (code >= 0).then_some(out)
    }

    #[must_use]
    #[allow(dead_code)] // #218 的 interp 改用 spawn_size_watcher
    pub fn property_i64(&self, name: &str) -> Option<i64> {
        let key = cstr(name).ok()?;
        let mut out: i64 = 0;
        let code = unsafe {
            (self.lib.get_property)(
                self.handle.0,
                key.as_ptr(),
                format::INT64,
                (&raw mut out).cast::<c_void>(),
            )
        };
        (code >= 0).then_some(out)
    }

    /// 后台线程每 200 ms 读一次 `dwidth` / `dheight`，渲染线程只读原子量。
    ///
    /// #218 在 Adreno 610 上实测：渲染线程每张新帧同步 `mpv_get_property`
    /// 两次，`hold` 模式就从 59 fps 掉到 51 fps——同步取属性要排队等 mpv 的
    /// core 线程，core 忙着解封装 / 解码时渲染线程跟着干等。
    ///
    /// 后台线程每 200 ms 读一次：`dwidth`、`dheight`、`container-fps` × 1000。
    #[allow(dead_code)] // 只有 #218 的 interp 用
    pub fn spawn_size_watcher(&self) -> Arc<SizeWatch> {
        let size = Arc::new((
            std::sync::atomic::AtomicI64::new(0),
            std::sync::atomic::AtomicI64::new(0),
            std::sync::atomic::AtomicI64::new(0),
        ));
        let lib = Arc::clone(&self.lib);
        let handle = self.handle;
        let stop = Arc::clone(&self.stop);
        let out = Arc::clone(&size);
        let _ = std::thread::Builder::new()
            .name("mpv-size".to_owned())
            .spawn(move || {
                // 整体搬进来；只写 handle.0 的话闭包会按字段捕获裸指针，不 Send。
                let handle = handle;
                let read = |name: &std::ffi::CStr| {
                    let mut v: i64 = 0;
                    let code = unsafe {
                        (lib.get_property)(
                            handle.0,
                            name.as_ptr(),
                            format::INT64,
                            (&raw mut v).cast::<c_void>(),
                        )
                    };
                    (code >= 0).then_some(v)
                };
                let read_f64 = |name: &std::ffi::CStr| {
                    let mut v: f64 = 0.0;
                    let code = unsafe {
                        (lib.get_property)(
                            handle.0,
                            name.as_ptr(),
                            format::DOUBLE,
                            (&raw mut v).cast::<c_void>(),
                        )
                    };
                    (code >= 0).then_some(v)
                };
                while !stop.load(Ordering::Acquire) {
                    if let (Some(w), Some(h)) = (read(c"dwidth"), read(c"dheight")) {
                        out.0.store(w, Ordering::Release);
                        out.1.store(h, Ordering::Release);
                    }
                    // #219：片源帧率（媒体时间），千分之一 fps；读不到记 0。
                    let fps = read_f64(c"container-fps").unwrap_or(0.0);
                    out.2.store((fps * 1000.0) as i64, Ordering::Release);
                    std::thread::sleep(Duration::from_millis(200));
                }
            });
        size
    }

    /// 在渲染线程上建 render context。必须在 GL 上下文 current 时调用。
    ///
    /// # Safety
    /// `loader` 必须能解析 GL 函数，且调用期间 GL 上下文为当前上下文。
    pub unsafe fn create_render(
        &self,
        loader: &dyn Fn(&std::ffi::CStr) -> *const c_void,
    ) -> Result<MpvRender> {
        unsafe { MpvRender::new(Arc::clone(&self.lib), self.handle, loader) }
    }

    /// mpv 的时钟（微秒），与 `mpv_render_frame_info::target_time` 同一时基。
    #[allow(dead_code)] // 只有 #218 的 interp 可执行文件用
    pub fn time_us(&self) -> i64 {
        unsafe { (self.lib.get_time_us)(self.handle.0) }
    }

    /// 直接写一个 mpv 属性。
    ///
    /// 只给探针用：`PlayerPort` 的命令集是**领域层**的词汇（ADR-003），不该为了
    /// 调一次 `hwdec` 就往里加一个「随便写属性」的口子。探针要的恰恰是那种
    /// 口子——它在问「mpv 这边到底怎么回事」，不是在实现播放语义。
    ///
    /// 桌面那条路（`src/main.rs`）不用它，所以在非 Android 目标上它是死代码——
    /// 这个模块被两个 target 各编一遍（见 `src/lib.rs` 的说明）。
    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    pub fn debug_set_property(&self, name: &str, value: &str) -> Result<()> {
        self.set_property_string(name, value)
    }

    /// 发命令。**一律走异步版本**，哪怕调用方并不需要结果。
    ///
    /// 这不是图省事，是绕开一个真实踩到的死锁：`mpv_command` 是同步的，
    /// 要等 mpv 核心处理完；而核心在某些时刻等着渲染线程画完一帧。本程序
    /// 的调用点（按键回调、16ms tick）就在渲染线程上，于是两边互等，整个
    /// 窗口冻死——手动按左方向键回退 seek 时复现，脚本化那次只是恰好没撞上。
    /// mpv 的文档对此有明确警告。
    ///
    /// 异步版把参数拷走就返回，结果以 `COMMAND_REPLY` 回到事件泵。
    fn command(&self, args: &[&str]) -> Result<()> {
        let owned: Vec<CString> = args.iter().map(|a| cstr(a)).collect::<Result<Vec<_>>>()?;
        let mut ptrs: Vec<*const c_char> = owned.iter().map(|c| c.as_ptr()).collect();
        ptrs.push(std::ptr::null());
        let code = unsafe { (self.lib.command_async)(self.handle.0, 0, ptrs.as_mut_ptr()) };
        self.lib.check(code, &format!("mpv 命令 {:?}", args[0]))
    }

    /// 写属性。同样走异步版本，理由同 [`Self::command`]。
    fn set_property_string(&self, name: &str, value: &str) -> Result<()> {
        let k = cstr(name)?;
        let v = cstr(value)?;
        // MPV_FORMAT_STRING 的 data 是 `char**`；异步版本在返回前就把值
        // 拷走，所以 `v` 活到调用结束即可。
        let mut value_ptr: *const c_char = v.as_ptr();
        let code = unsafe {
            (self.lib.set_property_async)(
                self.handle.0,
                0,
                k.as_ptr(),
                format::STRING,
                (&raw mut value_ptr).cast::<c_void>(),
            )
        };
        self.lib.check(code, &format!("设置 mpv 属性 {name}"))
    }
}

impl Drop for MpvPlayer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        // 事件泵阻塞在 wait_event 上，叫醒它，否则要等超时。
        unsafe { (self.lib.wakeup)(self.handle.0) };
        if let Some(pump) = self.pump.take() {
            let _ = pump.join();
        }
        unsafe { (self.lib.terminate_destroy)(self.handle.0) };
    }
}

impl PlayerPort for MpvPlayer {
    type Error = anyhow::Error;

    fn dispatch(&mut self, command: Stamped<PlayerCommand>) -> Result<()> {
        // 过期命令直接丢：调用方 bump 过闸门之后，在途的旧命令不该再生效。
        let Some(command) = command.accept(&self.gate) else {
            return Ok(());
        };
        match command {
            PlayerCommand::Open { source, headers } => {
                self.set_property_string("user-agent", &headers.user_agent)?;
                self.set_property_string("referrer", &headers.referer)?;
                let url = match &source {
                    MediaSource::Dash { video, audio } => {
                        // DASH 分轨：视频走 loadfile，音频作为外挂轨。
                        // 不用 edl:// —— 领域层刻意不编码 mpv 的语法
                        // （见 player.rs 的注释），拼 edl 等于把那条边界
                        // 又移回领域层。
                        //
                        // 不能 `set_property_string("audio-files", url)`：
                        // 它是路径列表，按平台分隔符拆——Unix 上是 `:`，
                        // `https://…` 会被拆碎，结果「有音频源但没有音轨」；
                        // Windows 的分隔符是 `;`，所以那边没暴露。
                        // `change-list … append` 只追加一项，不做拆分。
                        self.command(&["change-list", "audio-files", "clr", ""])?;
                        self.command(&["change-list", "audio-files", "append", audio.expose()])?;
                        video.expose().clone()
                    }
                    MediaSource::Segments(parts) => {
                        let Some(first) = parts.first() else {
                            bail!("Segments 为空");
                        };
                        first.expose().clone()
                    }
                };
                {
                    let mut state = self
                        .gen_state
                        .lock()
                        .map_err(|_| anyhow::anyhow!("gen_state 被毒化"))?;
                    state.target = self.gate.current();
                    state.awaiting_load = true;
                }
                self.command(&["loadfile", &url, "replace"])
            }
            PlayerCommand::Play => self.set_property_string("pause", "no"),
            PlayerCommand::Pause => self.set_property_string("pause", "yes"),
            PlayerCommand::Seek { position_ms } => {
                {
                    let mut state = self
                        .gen_state
                        .lock()
                        .map_err(|_| anyhow::anyhow!("gen_state 被毒化"))?;
                    state.target = self.gate.current();
                    state.awaiting_restart = true;
                }
                let secs = position_ms as f64 / 1000.0;
                self.command(&["seek", &format!("{secs:.3}"), "absolute"])
            }
            PlayerCommand::SetSpeed(speed) => {
                self.set_property_string("speed", &format!("{speed:.3}"))
            }
            PlayerCommand::SelectTracks { video, audio } => {
                if let Some(TrackId(id)) = video {
                    self.set_property_string("vid", &id.to_string())?;
                }
                if let Some(TrackId(id)) = audio {
                    self.set_property_string("aid", &id.to_string())?;
                }
                Ok(())
            }
            // 本 spike 的表面就是 Slint 的 GL framebuffer，attach/detach 由
            // render context 的生死代表，这里没有第二种表面可切。
            PlayerCommand::AttachSurface(_) | PlayerCommand::DetachSurface => Ok(()),
            PlayerCommand::SetLifecycle(lifecycle) => {
                // 后台只停画面、不停音频，对应基线的「后台播放」。
                let value = matches!(lifecycle, Lifecycle::Background);
                self.set_property_string("vid", if value { "no" } else { "auto" })
            }
            PlayerCommand::Close => self.command(&["stop"]),
        }
    }
}

/// 观察的属性：(userdata, 名字, 格式)。
const OBSERVED: &[(u64, &str, c_int)] = &[
    (1, "time-pos", format::DOUBLE),
    (2, "duration", format::DOUBLE),
    (3, "pause", format::FLAG),
    (4, "core-idle", format::FLAG),
    (5, "paused-for-cache", format::FLAG),
    (6, "eof-reached", format::FLAG),
    (7, "speed", format::DOUBLE),
    (8, "vid", format::INT64),
    (9, "aid", format::INT64),
];

#[derive(Debug, Default, Clone, Copy)]
struct Observed {
    position_ms: u64,
    duration_ms: Option<u64>,
    paused: bool,
    core_idle: bool,
    cache_pause: bool,
    eof: bool,
    speed: f32,
    loaded: bool,
    vid: Option<i64>,
    aid: Option<i64>,
}

impl Observed {
    fn snapshot(self) -> PlayerSnapshot {
        let state = if !self.loaded {
            PlaybackState::Opening
        } else if self.eof {
            PlaybackState::Ended
        } else if self.paused {
            PlaybackState::Paused
        } else if self.cache_pause || self.core_idle {
            // core-idle 为真而 pause 为假，说明后端在等数据或在 seek，
            // 这正是 Buffering。**不要**在这种状态下外推 position。
            PlaybackState::Buffering
        } else {
            PlaybackState::Playing
        };
        PlayerSnapshot {
            position_ms: self.position_ms,
            duration_ms: self.duration_ms,
            state,
            speed: if self.speed > 0.0 { self.speed } else { 1.0 },
            video_track: self.vid.map(TrackId),
            audio_track: self.aid.map(TrackId),
        }
    }
}

/// 事件泵。高频的 time-pos 在这里**合并**成最多每 40ms 一个快照，
/// 不逐帧过桥——ADR-003「适配器不得每帧一次调用」。
fn pump_events(
    lib: &MpvLib,
    handle: HandlePtr,
    session: bili_domain::session::SessionId,
    gen_state: &Mutex<GenState>,
    stop: &AtomicBool,
    sink: &mut dyn PlayerEventSink,
) {
    let mut observed = Observed::default();
    let mut dirty = false;
    let mut last_emit = Instant::now() - Duration::from_secs(1);

    let emit = |sink: &mut dyn PlayerEventSink, event: PlaybackEvent, generation: Generation| {
        sink.emit(Stamped::new(session, generation, event));
    };

    while !stop.load(Ordering::Acquire) {
        let raw = unsafe { (lib.wait_event)(handle.0, 0.02) };
        if raw.is_null() {
            continue;
        }
        let event: MpvEvent = unsafe { *raw };
        let generation = {
            let state = gen_state.lock();
            match state {
                Ok(state) => state.emit,
                Err(_) => break,
            }
        };

        match event.event_id {
            event::NONE => {}
            event::SHUTDOWN => {
                emit(sink, PlaybackEvent::Closed, generation);
                break;
            }
            event::START_FILE => {
                observed.loaded = false;
                dirty = true;
            }
            event::FILE_LOADED => {
                observed.loaded = true;
                observed.eof = false;
                let promoted = promote(gen_state, Trigger::Load);
                dirty = true;
                if let Some(promoted) = promoted {
                    emit(sink, PlaybackEvent::Snapshot(observed.snapshot()), promoted);
                    last_emit = Instant::now();
                    dirty = false;
                }
            }
            event::PLAYBACK_RESTART => {
                if let Some(promoted) = promote(gen_state, Trigger::Restart) {
                    // 这才是「seek 完成了」的事实。
                    emit(
                        sink,
                        PlaybackEvent::SeekCompleted {
                            position_ms: observed.position_ms,
                        },
                        promoted,
                    );
                    emit(sink, PlaybackEvent::Snapshot(observed.snapshot()), promoted);
                    last_emit = Instant::now();
                    dirty = false;
                }
            }
            event::END_FILE => {
                let reason = unsafe { end_file_reason(event.data) };
                match reason {
                    Some((4, code)) => {
                        emit(sink, PlaybackEvent::Failed(failure_kind(code)), generation);
                    }
                    Some((0, _)) => {
                        observed.eof = true;
                        emit(
                            sink,
                            PlaybackEvent::Snapshot(observed.snapshot()),
                            generation,
                        );
                    }
                    _ => {}
                }
            }
            event::COMMAND_REPLY | event::SET_PROPERTY_REPLY => {
                // 异步命令失败只有在这里才知道。**不**往 UI 发 Failed：
                // 命令本身失败（属性名打错之类）和播放失败不是一回事，
                // 混在一起会让「播放失败事件」那项失去意义。
                if event.error < 0 {
                    eprintln!("[spike] mpv 异步命令失败：{}", lib.error_text(event.error));
                }
            }
            event::PROPERTY_CHANGE => {
                apply_property(&mut observed, event.reply_userdata, event.data);
                dirty = true;
            }
            _ => {}
        }

        if dirty && last_emit.elapsed() >= Duration::from_millis(40) {
            emit(
                sink,
                PlaybackEvent::Snapshot(observed.snapshot()),
                generation,
            );
            last_emit = Instant::now();
            dirty = false;
        }
    }
}

enum Trigger {
    Load,
    Restart,
}

/// 后端确认切换之后，把事件泵的章切到 target。返回新章（没在等就返回 None）。
fn promote(gen_state: &Mutex<GenState>, trigger: Trigger) -> Option<Generation> {
    let mut state = gen_state.lock().ok()?;
    let waiting = match trigger {
        Trigger::Load => state.awaiting_load,
        Trigger::Restart => state.awaiting_restart,
    };
    if !waiting {
        return None;
    }
    match trigger {
        Trigger::Load => state.awaiting_load = false,
        Trigger::Restart => state.awaiting_restart = false,
    }
    state.emit = state.target;
    Some(state.emit)
}

fn apply_property(observed: &mut Observed, id: u64, data: *mut c_void) {
    if data.is_null() {
        return;
    }
    let property: MpvEventProperty = unsafe { *data.cast::<MpvEventProperty>() };
    if property.data.is_null() {
        // mpv 用 format=NONE 表示「该属性当前不可用」，例如没加载文件时的
        // duration。保留旧值即可，不要清零——清零会让 UI 闪一下。
        return;
    }
    match (id, property.format) {
        (1, format::DOUBLE) => {
            let secs = unsafe { *property.data.cast::<f64>() };
            if secs.is_finite() && secs >= 0.0 {
                observed.position_ms = (secs * 1000.0) as u64;
            }
        }
        (2, format::DOUBLE) => {
            let secs = unsafe { *property.data.cast::<f64>() };
            observed.duration_ms =
                (secs.is_finite() && secs > 0.0).then_some((secs * 1000.0) as u64);
        }
        (3, format::FLAG) => observed.paused = unsafe { *property.data.cast::<c_int>() } != 0,
        (4, format::FLAG) => {
            observed.core_idle = unsafe { *property.data.cast::<c_int>() } != 0;
        }
        (5, format::FLAG) => {
            observed.cache_pause = unsafe { *property.data.cast::<c_int>() } != 0;
        }
        (6, format::FLAG) => observed.eof = unsafe { *property.data.cast::<c_int>() } != 0,
        (7, format::DOUBLE) => {
            observed.speed = unsafe { *property.data.cast::<f64>() } as f32;
        }
        (8, format::INT64) => {
            observed.vid = Some(unsafe { *property.data.cast::<i64>() });
        }
        (9, format::INT64) => {
            observed.aid = Some(unsafe { *property.data.cast::<i64>() });
        }
        _ => {}
    }
}

/// `mpv_event_end_file` 的前两个字段：reason 与 error。
unsafe fn end_file_reason(data: *mut c_void) -> Option<(c_int, c_int)> {
    if data.is_null() {
        return None;
    }
    #[repr(C)]
    struct EndFileHead {
        reason: c_int,
        error: c_int,
    }
    let head = unsafe { &*data.cast::<EndFileHead>() };
    Some((head.reason, head.error))
}

/// mpv 错误码 → 契约里的失败类别。UI 要据此决定提示什么，不能靠日志文本。
const fn failure_kind(code: c_int) -> PlayerFailureKind {
    match code {
        -13 => PlayerFailureKind::Network, // LOADING_FAILED
        -17 | -18 => PlayerFailureKind::UnsupportedSource, // UNKNOWN_FORMAT / UNSUPPORTED
        -16 => PlayerFailureKind::Decode,  // NOTHING_TO_PLAY
        _ => PlayerFailureKind::Backend,
    }
}

/// GL 渲染侧。与 [`MpvPlayer`] 分开，因为它**只能**在渲染线程上用。
pub struct MpvRender {
    lib: Arc<MpvLib>,
    ctx: *mut MpvRenderContext,
}

impl std::fmt::Debug for MpvRender {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MpvRender")
    }
}

// 传给 mpv 的 get_proc_address 蹦床。Slint 只在渲染回调期间借给我们一个
// `&dyn Fn`，而 mpv 可能在 create **之后**继续解析函数，所以用线程局部
// 存当前有效的 loader，并在回调结束时清掉——宁可返回 null 让 mpv 报错，
// 也不要留一个悬垂指针。
thread_local! {
    static LOADER: std::cell::Cell<*const c_void> =
        const { std::cell::Cell::new(std::ptr::null()) };
}

type LoaderRef<'a> = &'a dyn Fn(&std::ffi::CStr) -> *const c_void;

pub struct LoaderGuard;

impl LoaderGuard {
    /// # Safety
    /// `holder` 必须在 guard 存活期间一直有效。
    unsafe fn new(holder: *const LoaderRef<'_>) -> Self {
        LOADER.with(|slot| slot.set(holder.cast::<c_void>()));
        Self
    }
}

impl Drop for LoaderGuard {
    fn drop(&mut self) {
        LOADER.with(|slot| slot.set(std::ptr::null()));
    }
}

unsafe extern "C" fn get_proc_address(_ctx: *mut c_void, name: *const c_char) -> *mut c_void {
    let holder = LOADER.with(std::cell::Cell::get);
    if holder.is_null() || name.is_null() {
        return std::ptr::null_mut();
    }
    let loader = unsafe { *holder.cast::<LoaderRef<'_>>() };
    let name = unsafe { std::ffi::CStr::from_ptr(name) };
    loader(name).cast_mut()
}

impl MpvRender {
    /// # Safety
    /// 必须在 GL 上下文 current 的线程上调用。
    unsafe fn new(
        lib: Arc<MpvLib>,
        handle: HandlePtr,
        loader: &dyn Fn(&std::ffi::CStr) -> *const c_void,
    ) -> Result<Self> {
        let holder: LoaderRef<'_> = loader;
        let _guard = unsafe { LoaderGuard::new(&raw const holder) };

        let api = cstr("opengl")?;
        let mut init = sys::MpvOpenglInitParams {
            get_proc_address: Some(get_proc_address),
            get_proc_address_ctx: std::ptr::null_mut(),
        };
        let mut params = [
            sys::MpvRenderParam {
                type_: sys::render_param::API_TYPE,
                data: api.as_ptr().cast::<c_void>().cast_mut(),
            },
            sys::MpvRenderParam {
                type_: sys::render_param::OPENGL_INIT_PARAMS,
                data: (&raw mut init).cast::<c_void>(),
            },
            sys::MpvRenderParam {
                type_: sys::render_param::INVALID,
                data: std::ptr::null_mut(),
            },
        ];
        let mut ctx: *mut MpvRenderContext = std::ptr::null_mut();
        let code =
            unsafe { (lib.render_context_create)(&raw mut ctx, handle.0, params.as_mut_ptr()) };
        lib.check(code, "mpv_render_context_create")?;
        Ok(Self { lib, ctx })
    }

    /// 有没有新帧等着画。
    ///
    /// **当前刻意没有用它来跳过渲染**：Slint 在背景透明时不清屏（视频能透
    /// 出来正是因为这一点），所以 mpv 那次全屏绘制同时充当了清屏。跳过它
    /// 的那一帧会留着上一帧的弹幕文字不擦，屏幕上就是一层层残影。要省这点
    /// GPU，得先给 UI 层自己的清屏，不在 spike 范围内。
    #[allow(dead_code)]
    #[must_use]
    pub fn has_new_frame(&self) -> bool {
        let flags = unsafe { (self.lib.render_context_update)(self.ctx) };
        flags & sys::RENDER_UPDATE_FRAME != 0
    }

    /// `mpv_render_context_update` 的原始返回值。#218 的插帧路径每个 vsync
    /// 调一次，用 [`sys::RENDER_UPDATE_FRAME`] 判断有没有帧待取。
    #[allow(dead_code)] // 只有 #218 的 interp 可执行文件用
    pub fn update_flags(&self) -> u64 {
        unsafe { (self.lib.render_context_update)(self.ctx) }
    }

    /// 下一张要画的帧的信息（`MPV_RENDER_PARAM_NEXT_FRAME_INFO`）。取不到时
    /// 返回 `None`（例如还没有帧排进队列）。
    #[allow(dead_code)] // 只有 #218 的 interp 可执行文件用
    pub fn next_frame_info(&self) -> Option<sys::MpvRenderFrameInfo> {
        let mut info = sys::MpvRenderFrameInfo::default();
        let param = sys::MpvRenderParam {
            type_: sys::render_param::NEXT_FRAME_INFO,
            data: (&raw mut info).cast::<c_void>(),
        };
        let code = unsafe { (self.lib.render_context_get_info)(self.ctx, param) };
        (code >= 0).then_some(info)
    }

    /// 把当前帧画进给定的 FBO。
    ///
    /// # Safety
    /// 必须在 GL 上下文 current 时调用，且 `fbo` 为可用的 draw framebuffer。
    pub unsafe fn render(
        &self,
        fbo: i32,
        width: i32,
        height: i32,
        flip_y: bool,
        loader: &dyn Fn(&std::ffi::CStr) -> *const c_void,
    ) -> Result<()> {
        let holder: LoaderRef<'_> = loader;
        let _guard = unsafe { LoaderGuard::new(&raw const holder) };

        let mut target = sys::MpvOpenglFbo {
            fbo,
            w: width,
            h: height,
            // 0 = 让 mpv 自己问 GL。
            internal_format: 0,
        };
        let mut flip: c_int = i32::from(flip_y);
        // **这一行值 30fps**：不给这个参数时 mpv 默认会阻塞到该帧的目标显示
        // 时刻才返回，于是整个 UI 线程被视频帧率拖住——第一次实测 30fps 的
        // 片子就把窗口锁在 31.6fps、p50 33ms，看上去像是弹幕层画不动，其实
        // 是 render 调用在睡觉。渲染节奏归窗口（vsync），不归片源。
        let mut block: c_int = 0;
        let mut params = [
            sys::MpvRenderParam {
                type_: sys::render_param::OPENGL_FBO,
                data: (&raw mut target).cast::<c_void>(),
            },
            sys::MpvRenderParam {
                type_: sys::render_param::FLIP_Y,
                data: (&raw mut flip).cast::<c_void>(),
            },
            sys::MpvRenderParam {
                type_: sys::render_param::BLOCK_FOR_TARGET_TIME,
                data: (&raw mut block).cast::<c_void>(),
            },
            sys::MpvRenderParam {
                type_: sys::render_param::INVALID,
                data: std::ptr::null_mut(),
            },
        ];
        let code = unsafe { (self.lib.render_context_render)(self.ctx, params.as_mut_ptr()) };
        self.lib.check(code, "mpv_render_context_render")
    }
}

impl Drop for MpvRender {
    fn drop(&mut self) {
        unsafe { (self.lib.render_context_free)(self.ctx) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffering_is_not_paused() {
        let observed = Observed {
            loaded: true,
            core_idle: true,
            ..Observed::default()
        };
        assert_eq!(observed.snapshot().state, PlaybackState::Buffering);
    }

    #[test]
    fn pause_wins_over_core_idle() {
        // 暂停时 core-idle 也是真的；先判 pause，否则整段暂停都被报成缓冲。
        let observed = Observed {
            loaded: true,
            core_idle: true,
            paused: true,
            ..Observed::default()
        };
        assert_eq!(observed.snapshot().state, PlaybackState::Paused);
    }

    #[test]
    fn nothing_is_opening_until_the_file_is_loaded() {
        let observed = Observed::default();
        assert_eq!(observed.snapshot().state, PlaybackState::Opening);
    }

    #[test]
    fn speed_falls_back_to_one() {
        let observed = Observed {
            loaded: true,
            ..Observed::default()
        };
        assert!((observed.snapshot().speed - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn the_stamp_only_advances_once_the_backend_confirms() {
        let gate = GenerationGate::new(bili_domain::session::SessionId(7));
        let old = gate.current();
        let state = Mutex::new(GenState {
            emit: old,
            target: old,
            awaiting_restart: false,
            awaiting_load: false,
        });

        // 调用方 seek：bump 闸门、记下目标代际，但盖章仍是旧的。
        let target = gate.bump();
        {
            let mut guard = state.lock().expect("锁");
            guard.target = target;
            guard.awaiting_restart = true;
        }
        assert_eq!(state.lock().expect("锁").emit, old);
        // 旧快照此刻到达 → 被闸门丢弃。
        let stale = Stamped::new(gate.session(), old, PlaybackEvent::Closed);
        assert!(stale.accept(&gate).is_none());

        // mpv 报 PLAYBACK_RESTART 之后才切章。
        assert_eq!(promote(&state, Trigger::Restart), Some(target));
        let fresh = Stamped::new(gate.session(), target, PlaybackEvent::Closed);
        assert!(fresh.accept(&gate).is_some());
        // 再来一次不会重复提升。
        assert_eq!(promote(&state, Trigger::Restart), None);
    }

    #[test]
    fn loading_failure_is_reported_as_network() {
        assert_eq!(failure_kind(-13), PlayerFailureKind::Network);
        assert_eq!(failure_kind(-17), PlayerFailureKind::UnsupportedSource);
        assert_eq!(failure_kind(-20), PlayerFailureKind::Backend);
    }
}

/// [`MpvPlayer::spawn_size_watcher`] 的输出：显示宽、高、`container-fps` × 1000。
pub type SizeWatch = (
    std::sync::atomic::AtomicI64,
    std::sync::atomic::AtomicI64,
    std::sync::atomic::AtomicI64,
);
