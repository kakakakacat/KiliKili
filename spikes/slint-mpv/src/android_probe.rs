//! SPIKE-002（issue #9）的 Android 探针。
//!
//! 第一轮（PR #185）只回答一个**前提**问题：Android 上 Slint 的渲染回调给不给
//! OpenGL 上下文、libmpv 能不能把视频画进同一个 framebuffer。答案是「给」「能」，
//! 记录在 `docs/m0/spikes/SPIKE-002-android.md`。
//!
//! 这一轮补 issue 范围里**剩下的全部**。每一项都要变成「系统那边做一个动作、
//! 探针这边记下一个事实」，否则只是换个说法再猜一遍：
//!
//! | #9 的项 | 探针这边的观察点 | 系统那边的动作 |
//! |---|---|---|
//! | surface 生命周期 | `RenderingSetup`/`Teardown` 的次数与配对、render context 能不能**重建** | HOME → 回前台 |
//! | 旋转 | 窗口尺寸变化、`android_main` 有没有重跑（= activity 重建）、转完还出不出帧 | `settings put system user_rotation` |
//! | 后台音频 | **独立线程**每秒读 `time-pos` / `audio-pts`：UI 停了它还在 | HOME 之后继续读 |
//! | PiP 可行性 | JNI 调 `enterPictureInPictureMode`，再 poll `isInPictureInPictureMode` | `dumpsys activity` 对照 |
//! | 触摸分发 | 全屏 `TouchArea` 的 pointer-event 与按钮的 `clicked` 分得开 | `input tap` / `input swipe` |
//!
//! 两条贯穿全文的设计：
//!
//! 1. **状态线程独立于 UI 事件循环。** 后台音频这一项的全部意义就在于「画面
//!    停了、声音没停」，而 Slint 的 timer 跑在事件循环上——用它去证明后台还在
//!    播，等于让嫌疑人自己作证。所以每秒一条的 `STATUS` 由单独的线程打。
//! 2. **驱动靠 logcat，不靠人眼。** 结论行都带固定前缀（`EV` / `STATUS` /
//!    `TOUCH` / `BUTTONS` / `SURFACE` / `SIZE` / `PIP` / `SHOT` / `CMD`），
//!    `android-drive.sh` 按前缀取，人只负责看截图确认画面。

use std::cell::RefCell;
use std::ffi::c_void;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow};
use bili_domain::player::{
    Lifecycle, MediaHeaders, MediaSource, PlaybackEvent, PlaybackState, PlayerCommand,
    PlayerEventSink, PlayerPort,
};
use bili_domain::redact::Redacted;
use bili_domain::session::{GenerationGate, SessionId, Stamped};
use slint::ComponentHandle;

use crate::{gl, mpv};

slint::include_modules!();

/// `android_main` 跑了第几次。
///
/// activity 被系统重建时它会**再跑一次**（进程不变，`static` 不清零）——
/// 「旋转到底有没有重建 activity」这一问，这是最硬的证据：NativeActivity 下
/// 没有自己的 Java 代码，`onCreate` 那种日志压根不存在。
static MAIN_RUNS: AtomicU32 = AtomicU32::new(0);

/// #210 补测：外部数据目录里有 `source.txt`（`key=value` 每行一个：video、audio、
/// ua、referer，可选 video2、audio2）就改放网络流，不用 probe.mp4。文件里是带签名的
/// 取流地址，只由驱动脚本临时推进来，不进仓库。
#[derive(Default)]
struct NetSource {
    video: String,
    audio: String,
    ua: String,
    referer: String,
    video2: String,
    audio2: String,
}
static NET: OnceLock<NetSource> = OnceLock::new();

fn load_net_source(path: &std::path::Path) -> Option<NetSource> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut src = NetSource::default();
    for line in text.lines() {
        let Some((k, v)) = line.split_once('=') else { continue };
        let v = v.trim().to_owned();
        match k.trim() {
            "video" => src.video = v,
            "audio" => src.audio = v,
            "ua" => src.ua = v,
            "referer" => src.referer = v,
            "video2" => src.video2 = v,
            "audio2" => src.audio2 = v,
            _ => {}
        }
    }
    (!src.video.is_empty()).then_some(src)
}

/// 每秒一条 STATUS 用：UI 合成帧数、mpv 渲染耗时（本秒最大 / 累计）。
static UI_FRAMES: AtomicU64 = AtomicU64::new(0);
static RENDER_MAX_US: AtomicU64 = AtomicU64::new(0);
static RENDER_SUM_US: AtomicU64 = AtomicU64::new(0);
static RENDER_N: AtomicU64 = AtomicU64::new(0);
static LAST_STATUS: Mutex<Option<(Instant, u64, f64)>> = Mutex::new(None);

/// (CPU 累计秒, VmRSS MiB)，读 /proc/self。
fn proc_usage() -> (f64, f64) {
    let cpu = std::fs::read_to_string("/proc/self/stat")
        .ok()
        .and_then(|s| {
            // comm 里可能有空格，从最后一个 ')' 之后数：utime 是第 14 列、stime 第 15 列。
            let rest = &s[s.rfind(')')? + 2..];
            let f: Vec<&str> = rest.split_whitespace().collect();
            let ticks = f.get(11)?.parse::<f64>().ok()? + f.get(12)?.parse::<f64>().ok()?;
            Some(ticks / 100.0)
        })
        .unwrap_or(0.0);
    let rss = std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            let line = s.lines().find(|l| l.starts_with("VmRSS:"))?;
            line.split_whitespace().nth(1)?.parse::<f64>().ok()
        })
        .map_or(0.0, |kb| kb / 1024.0);
    (cpu, rss)
}

/// 探针观察到的事实。一行一条，顺序即发生顺序。
#[derive(Default)]
struct Findings {
    lines: Vec<String>,
}

impl Findings {
    fn say(&mut self, line: impl Into<String>) {
        let line = line.into();
        log::info!("{line}");
        self.lines.push(line);
    }

    /// 同一条错误可能每帧都来一次，HUD 会被刷爆，所以去重。
    fn say_once(&mut self, line: impl Into<String>) {
        let line = line.into();
        if !self.lines.iter().any(|l| *l == line) {
            self.say(line);
        }
    }

    /// HUD 只放最近这几行：手机屏幕就那么高，早期的行滚上去也没人看得见，
    /// 而每帧重新布局一整段长文本是要算进帧耗时的。完整的在 logcat 里。
    fn hud(&self) -> String {
        const KEEP: usize = 12;
        let start = self.lines.len().saturating_sub(KEEP);
        self.lines[start..].join("\n")
    }
}

/// JavaVM 与 Activity 的裸指针。
///
/// `activity` 是 android-activity 给的**全局引用，所有权不在我们这儿**：只能
/// 拿它当 `JObject` 用，不能包成 `Global`/`Auto`（那会在 drop 时去删别人的
/// 引用）。两个指针在进程生命周期内有效，所以允许跨线程带着走。
#[derive(Clone, Copy)]
struct JavaHandles {
    vm: *mut c_void,
    activity: *mut c_void,
}

// SAFETY: JavaVM 指针本来就是设计成跨线程用的（每个线程自己 attach，这一步在
// `with_env` 里做）；activity 是全局引用，同样不绑线程。
unsafe impl Send for JavaHandles {}
unsafe impl Sync for JavaHandles {}

impl JavaHandles {
    fn is_usable(self) -> bool {
        !self.vm.is_null() && !self.activity.is_null()
    }
}

/// 在当前线程上 attach 到 JVM，拿到 `Env` 与 Activity 对象。
///
/// 每次调用都 attach 一遍听起来浪费，但 jni 那边「已经 attach 过」只是查一次
/// TLS，不发 JNI 调用；而探针这边的调用频率是每秒一次这个量级。
fn with_env<T>(
    handles: JavaHandles,
    f: impl FnOnce(&mut jni::Env<'_>, &jni::objects::JObject<'_>) -> Result<T>,
) -> Result<T> {
    anyhow::ensure!(handles.is_usable(), "没有 JavaVM / Activity 句柄");
    // SAFETY: 指针来自 android-activity，在 AndroidApp 活着期间有效。
    let vm = unsafe { jni::JavaVM::from_raw(handles.vm.cast()) };
    vm.attach_current_thread(|env| {
        // SAFETY: 见 JavaHandles 的说明——只借用，不接管所有权。
        let activity = unsafe { jni::objects::JObject::from_raw(env, handles.activity.cast()) };
        f(env, &activity)
    })
}

/// PiP 的三个 JNI 调用。
///
/// NDK 没有画中画的 C API——`enterPictureInPictureMode` 是 `Activity` 的方法，
/// 只能从 Java 那一侧要。这本身就是本项要验的一部分：**纯 Rust /
/// NativeActivity 的形态下，PiP 够不够得着**。
mod pip {
    use anyhow::Result;
    use jni::objects::JObject;
    use jni::{JValue, jni_sig, jni_str};

    use super::{JavaHandles, with_env};

    /// `Activity.isInPictureInPictureMode()`
    pub fn is_active(handles: JavaHandles) -> Result<bool> {
        with_env(handles, |env, activity| {
            let value = env.call_method(
                activity,
                jni_str!("isInPictureInPictureMode"),
                jni_sig!(() -> boolean),
                &[],
            )?;
            Ok(value.z()?)
        })
    }

    /// 进入 PiP。
    ///
    /// 默认走**带参数**的那个重载（API 26）：真实播放器要按视频宽高比给小窗
    /// 定形，否则系统按默认比例裁。`aspect` 为 `None` 时走无参版本（API 24），
    /// 用来确认老接口也在。
    pub fn enter(handles: JavaHandles, aspect: Option<(i32, i32)>) -> Result<bool> {
        with_env(handles, |env, activity| {
            let Some((num, den)) = aspect else {
                env.call_method(
                    activity,
                    jni_str!("enterPictureInPictureMode"),
                    jni_sig!(() -> void),
                    &[],
                )?;
                return Ok(true);
            };
            let params = build_params(env, Some((num, den)), None)?;
            let value = env.call_method(
                activity,
                jni_str!("enterPictureInPictureMode"),
                jni_sig!((params: android.app.PictureInPictureParams) -> boolean),
                &[JValue::Object(&params)],
            )?;
            Ok(value.z()?)
        })
    }

    /// `Activity.setPictureInPictureParams`，用来打开 API 31 的**自动进入**：
    /// 从底部上滑回桌面时直接变小窗，是视频应用的标配手势。
    pub fn set_auto_enter(handles: JavaHandles, aspect: (i32, i32), on: bool) -> Result<()> {
        with_env(handles, |env, activity| {
            let params = build_params(env, Some(aspect), Some(on))?;
            env.call_method(
                activity,
                jni_str!("setPictureInPictureParams"),
                jni_sig!((params: android.app.PictureInPictureParams) -> void),
                &[JValue::Object(&params)],
            )?;
            Ok(())
        })
    }

    /// `new PictureInPictureParams.Builder().setAspectRatio(..).setAutoEnterEnabled(..).build()`
    fn build_params<'local>(
        env: &mut jni::Env<'local>,
        aspect: Option<(i32, i32)>,
        auto_enter: Option<bool>,
    ) -> Result<JObject<'local>> {
        let builder_class =
            env.find_class(jni_str!("android/app/PictureInPictureParams$Builder"))?;
        let mut builder = env.new_object(&builder_class, jni_sig!(() -> void), &[])?;

        if let Some((num, den)) = aspect {
            let rational_class = env.find_class(jni_str!("android/util/Rational"))?;
            let rational = env.new_object(
                &rational_class,
                jni_sig!((numerator: jint, denominator: jint) -> void),
                &[JValue::Int(num), JValue::Int(den)],
            )?;
            builder = env
                .call_method(
                    &builder,
                    jni_str!("setAspectRatio"),
                    jni_sig!((ratio: android.util.Rational) -> android.app.PictureInPictureParams::Builder),
                    &[JValue::Object(&rational)],
                )?
                .l()?;
        }

        if let Some(on) = auto_enter {
            builder = env
                .call_method(
                    &builder,
                    jni_str!("setAutoEnterEnabled"),
                    jni_sig!((enabled: boolean) -> android.app.PictureInPictureParams::Builder),
                    &[JValue::Bool(on)],
                )?
                .l()?;
        }

        let params = env
            .call_method(
                &builder,
                jni_str!("build"),
                jni_sig!(() -> android.app.PictureInPictureParams),
                &[],
            )?
            .l()?;
        anyhow::ensure!(
            !params.is_null(),
            "PictureInPictureParams.build() 返回 null"
        );
        Ok(params)
    }
}

/// UI 线程之外也要碰的东西。
///
/// 为什么不是 `Rc<RefCell<..>>`：状态线程要读 mpv 的属性、要下发
/// `SetLifecycle`，它跟 UI 线程是两条线。`MpvPlayer` 本身是 `Send` 的
/// （mpv 的 handle 按官方文档线程安全），包一层 `Mutex` 就够。
struct Watch {
    player: Option<Arc<Mutex<mpv::MpvPlayer>>>,
    java: JavaHandles,
    /// `reopen` 命令要用：播放被 VO 之死带走之后，重开一次看看能不能救回来。
    media: Option<PathBuf>,
    audio: Option<PathBuf>,
    /// 驱动脚本往这个文件写一行命令，状态线程 4Hz 轮询它。
    ///
    /// 为什么是文件而不是 socket / intent：NativeActivity 没有自己的 Java 层
    /// 可以收 intent，而 `adb shell` 本来就写得进应用的外部数据目录
    /// （probe.mp4 就是这么推进去的），不要权限，也不用再加一个依赖。
    cmd_path: Option<PathBuf>,
    /// 状态线程写、UI 线程取走贴到 HUD 上。
    notes: Mutex<Vec<String>>,
    /// 请求回读一帧，值即文件名。真正的回读只能在渲染回调里做。
    shot: Mutex<Option<String>>,
    /// 同上，但读的是 mpv 画完、Slint 还没画的那一刻。
    shot_pre: Mutex<Option<String>>,
    /// surface 没了就自动把 `vid` 关掉（真实播放器的后台策略）。
    /// 默认**关**：先看原样的行为，再看加了策略的行为，否则分不清是谁的功劳。
    auto_lifecycle: AtomicBool,
    gate: GenerationGate,
    quit: AtomicBool,
}

impl Watch {
    fn note(&self, line: impl Into<String>) {
        let line = line.into();
        log::info!("{line}");
        if let Ok(mut notes) = self.notes.lock() {
            notes.push(line);
        }
    }

    fn drain_notes(&self) -> Vec<String> {
        self.notes
            .lock()
            .map(|mut notes| std::mem::take(&mut *notes))
            .unwrap_or_default()
    }

    fn request_shot(&self, name: impl Into<String>) {
        if let Ok(mut slot) = self.shot.lock() {
            *slot = Some(name.into());
        }
    }

    fn take_shot_request(&self) -> Option<String> {
        self.shot.lock().ok().and_then(|mut slot| slot.take())
    }

    fn take_shot_pre_request(&self) -> Option<String> {
        self.shot_pre.lock().ok().and_then(|mut slot| slot.take())
    }

    /// 每秒一条。**这条线跟 UI 事件循环无关**——后台音频就靠它作证。
    fn log_status(&self) {
        let Some(player) = self.player.as_ref() else {
            return;
        };
        let Ok(player) = player.lock() else {
            return;
        };
        let position = player.property_f64("time-pos").unwrap_or(f64::NAN);
        // 声音真的在出，看的是 `audio-pts` 走不走，而不是 `pause` 的值：
        // 后台时如果 VO 没了把核心卡住，`pause` 照样是 no，pts 却不动。
        let audio = player.property_f64("audio-pts").unwrap_or(f64::NAN);
        let idle = player
            .property_string("core-idle")
            .unwrap_or_else(|| "?".to_owned());
        let paused = player
            .property_string("pause")
            .unwrap_or_else(|| "?".to_owned());
        let hwdec = player
            .property_string("hwdec-current")
            .unwrap_or_else(|| "?".to_owned());
        let vid = player
            .property_string("vid")
            .unwrap_or_else(|| "?".to_owned());
        let drops = player.property_i64("frame-drop-count").unwrap_or(-1);
        let avsync = player.property_f64("avsync").unwrap_or(f64::NAN);
        let speed = player.property_f64("speed").unwrap_or(f64::NAN);
        let vfps = player.property_f64("estimated-display-fps").unwrap_or(f64::NAN);
        let codec = player.property_string("video-codec").unwrap_or_default();
        let (w, h) = (
            player.property_i64("width").unwrap_or(0),
            player.property_i64("height").unwrap_or(0),
        );
        let (cpu_now, rss) = proc_usage();
        let frames_now = UI_FRAMES.load(Ordering::Relaxed);
        let (ui_fps, cpu) = match LAST_STATUS.lock().ok().and_then(|mut g| g.replace((Instant::now(), frames_now, cpu_now))) {
            Some((at, frames, cpu_then)) => {
                let secs = at.elapsed().as_secs_f64().max(1e-3);
                ((frames_now - frames) as f64 / secs, (cpu_now - cpu_then) / secs * 100.0)
            }
            None => (0.0, 0.0),
        };
        let rmax = RENDER_MAX_US.swap(0, Ordering::Relaxed) as f64 / 1000.0;
        let rsum = RENDER_SUM_US.swap(0, Ordering::Relaxed) as f64 / 1000.0;
        let rn = RENDER_N.swap(0, Ordering::Relaxed).max(1) as f64;
        log::info!(
            "STATUS t={position:.3} audio-pts={audio:.3} idle={idle} pause={paused} \
             hwdec={hwdec} vid={vid} drop={drops} avsync={avsync:.4} speed={speed} \
             src={w}x{h} codec={codec} ui_fps={ui_fps:.1} vo_fps={vfps:.1} \
             render_avg={:.2}ms render_max={rmax:.2}ms cpu={cpu:.0}% rss={rss:.0}MiB",
            rsum / rn
        );
    }

    /// 取一条命令并把文件清空。清空是为了幂等：轮询 4Hz，不清的话一条命令会
    /// 被执行好几次。
    fn take_command(&self) -> Option<String> {
        let path = self.cmd_path.as_ref()?;
        let text = std::fs::read_to_string(path).ok()?;
        let line = text.trim().to_owned();
        if line.is_empty() {
            return None;
        }
        let _ = std::fs::write(path, b"");
        Some(line)
    }

    fn run_command(&self, line: &str) {
        let mut parts = line.split_whitespace();
        let Some(verb) = parts.next() else { return };
        let rest: Vec<&str> = parts.collect();
        let result = match verb {
            // 驱动脚本的阶段标记。日志里有它，事后对时间轴不用靠猜。
            "note" => {
                log::info!("CMD note {}", rest.join(" "));
                Ok(())
            }
            "pip" => self.cmd_pip(&rest),
            "pip-auto" => self.cmd_pip_auto(&rest),
            "bg" => self.cmd_lifecycle(Lifecycle::Background),
            "fg" => self.cmd_lifecycle(Lifecycle::Foreground),
            "auto-lifecycle" => {
                let on = rest.first().is_some_and(|v| *v == "on");
                self.auto_lifecycle.store(on, Ordering::Release);
                self.note(format!("auto-lifecycle = {on}"));
                Ok(())
            }
            "reopen" => self.cmd_reopen(),
            // 切清晰度：从当前位置开 source.txt 里的 video2 / audio2。整条重载，不是无缝。
            "switch" => self.cmd_switch(),
            "recover" => self.cmd_recover(),
            // 直接写 mpv 属性。排查「视频为什么没回来」时要能单独动
            // `hwdec` / `vid`，不然只能靠重编一次 APK 换一个常量。
            "prop" => self.cmd_prop(&rest),
            "shot" => {
                let name = rest.first().copied().unwrap_or("shot").to_owned();
                self.request_shot(name);
                Ok(())
            }
            // 一条命令要两张图：mpv 刚画完一张、整帧合成完一张。
            "shot-pair" => {
                let name = rest.first().copied().unwrap_or("pair").to_owned();
                if let Ok(mut slot) = self.shot_pre.lock() {
                    *slot = Some(format!("{name}-pre"));
                }
                self.request_shot(format!("{name}-post"));
                Ok(())
            }
            "quit" => {
                self.note("收到 quit，退出进程");
                self.quit.store(true, Ordering::Release);
                // 直接退：探针不需要优雅关闭，而事件循环在后台时不一定转得动，
                // 走 quit_event_loop 可能根本没人处理。
                std::process::exit(0);
            }
            other => Err(anyhow!("不认识的命令 {other}")),
        };
        match result {
            Ok(()) => log::info!("CMD done {line}"),
            Err(err) => self.note(format!("CMD 失败 {line}: {err:#}")),
        }
    }

    /// surface 重建之后把画面救回来的**完整配方**：记住进度 → 重新 loadfile
    /// → seek 回去。
    ///
    /// 为什么要这么重：实测在 `hwdec=mediacodec` 下，render context 重建之后
    /// 光把视频轨选回来（`vid`）不出画面，把 `hwdec` 来回写一遍也不出，只有
    /// 整个 loadfile 才行（软解则没有这个问题）。代价是进度会回到 0，所以得
    /// 自己记一下再 seek 回去——这一步就是 Phase 2 的 Android 适配器要做的事。
    fn cmd_recover(&self) -> Result<()> {
        let Some(player) = self.player.as_ref() else {
            return Err(anyhow!("没有 player"));
        };
        let position_ms = {
            let player = player.lock().map_err(|_| anyhow!("player 锁被毒化"))?;
            player
                .property_f64("time-pos")
                .filter(|value| value.is_finite() && *value > 0.0)
                .map(|value| (value * 1000.0) as u64)
        };
        self.cmd_reopen()?;
        let Some(position_ms) = position_ms else {
            self.note("recover: 读不到进度，只重开不 seek");
            return Ok(());
        };
        // loadfile 是异步的，seek 得等文件真的装上。500ms 是实测够用的值；
        // 正式适配器应当等 FILE_LOADED 事件，而不是睡一觉。
        std::thread::sleep(Duration::from_millis(500));
        let mut player = player.lock().map_err(|_| anyhow!("player 锁被毒化"))?;
        player.dispatch(Stamped::new(
            self.gate.session(),
            self.gate.current(),
            PlayerCommand::Seek { position_ms },
        ))?;
        self.note(format!("recover: 重开并 seek 回 {position_ms}ms"));
        Ok(())
    }

    fn cmd_prop(&self, rest: &[&str]) -> Result<()> {
        let (Some(name), Some(value)) = (rest.first(), rest.get(1)) else {
            return Err(anyhow!("prop 要两个参数"));
        };
        let Some(player) = self.player.as_ref() else {
            return Err(anyhow!("没有 player"));
        };
        player
            .lock()
            .map_err(|_| anyhow!("player 锁被毒化"))?
            .debug_set_property(name, value)?;
        self.note(format!("prop {name}={value} 已下发"));
        Ok(())
    }

    fn cmd_switch(&self) -> Result<()> {
        let net = NET.get().ok_or_else(|| anyhow!("没有 source.txt"))?;
        if net.video2.is_empty() {
            return Err(anyhow!("source.txt 里没有 video2"));
        }
        let player = self.player.as_ref().ok_or_else(|| anyhow!("没有 player"))?;
        let pos = player
            .lock()
            .map_err(|_| anyhow!("player 锁被毒化"))?
            .property_f64("time-pos")
            .unwrap_or(0.0);
        player
            .lock()
            .map_err(|_| anyhow!("player 锁被毒化"))?
            .debug_set_property("start", &format!("{pos:.3}"))?;
        let audio = (!net.audio2.is_empty()).then(|| PathBuf::from(&net.audio2));
        open(player, &self.gate, &PathBuf::from(&net.video2), audio.as_ref())?;
        self.note(format!("switch 已下发，从 {pos:.3}s 续播"));
        Ok(())
    }

    /// 重开一次。播放被「VO 死了」带走之后，这是唯一的救法——Phase 2 的适配器
    /// 要么避免那种死法，要么就得有这一步。
    fn cmd_reopen(&self) -> Result<()> {
        let (Some(player), Some(media)) = (self.player.as_ref(), self.media.as_ref()) else {
            return Err(anyhow!("没有 player / 媒体"));
        };
        open(player, &self.gate, media, self.audio.as_ref())?;
        self.note("已重新下发 Open + Play");
        Ok(())
    }

    fn cmd_pip(&self, rest: &[&str]) -> Result<()> {
        let aspect = if rest.first().copied() == Some("noparams") {
            None
        } else {
            Some((16, 9))
        };
        // **故意从状态线程调**：Activity 的方法通常要求 UI 线程，而这里没有
        // 自己的 Java UI 线程可用（NativeActivity 的 Java 侧是系统的）。能不能
        // 从别的线程调得通，本身就是 Phase 2 要知道的事。按钮那条路走 UI
        // 线程，两边对照。
        let entered = pip::enter(self.java, aspect)?;
        self.note(format!(
            "PIP enter(aspect={aspect:?}, 来自状态线程) = {entered}"
        ));
        Ok(())
    }

    fn cmd_pip_auto(&self, rest: &[&str]) -> Result<()> {
        let on = rest.first().copied() != Some("off");
        pip::set_auto_enter(self.java, (16, 9), on)?;
        self.note(format!("PIP setAutoEnterEnabled({on}) 成功"));
        Ok(())
    }

    /// activity 被按到后台**或者**进了 PiP，两者都会走 `Main(Pause)`。
    ///
    /// 这里是 Phase 2 最容易写错的那一处：**PiP 也是 Pause**，但小窗里视频必须
    /// 还在。只靠 Pause 判「进后台」的话，一进小窗画面就没了。所以先问一句
    /// `isInPictureInPictureMode`。
    fn on_pause(&self) {
        if !self.auto_lifecycle.load(Ordering::Acquire) {
            return;
        }
        match pip::is_active(self.java) {
            Ok(true) => {
                self.note("Pause 到了，但在 PiP 里：不关视频轨");
                return;
            }
            Ok(false) => {}
            Err(err) => self.note(format!("读 isInPictureInPictureMode 失败: {err:#}")),
        }
        if let Err(err) = self.cmd_lifecycle(Lifecycle::Background) {
            self.note(format!("Pause 时的 SetLifecycle 失败: {err:#}"));
        }
    }

    /// 等 `vid` 真的变成想要的值。
    ///
    /// `SetLifecycle` 走的是 `mpv_set_property_async`（同步版本会跟渲染线程
    /// 互等，见 mpv/mod.rs 的说明），所以下发完**不代表已经生效**。而这件事
    /// 的全部意义就在于「赶在 render context 销毁之前」，所以必须等一下。
    /// 等不到也只是记一条，不卡死：200ms 是实测够用的上限。
    fn wait_for_vid(&self, want: &str) -> bool {
        let Some(player) = self.player.as_ref() else {
            return false;
        };
        for _ in 0..20 {
            if let Ok(player) = player.lock()
                && player.property_string("vid").as_deref() == Some(want)
            {
                return true;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        false
    }

    fn cmd_lifecycle(&self, lifecycle: Lifecycle) -> Result<()> {
        let Some(player) = self.player.as_ref() else {
            return Err(anyhow!("没有 player"));
        };
        let mut player = player.lock().map_err(|_| anyhow!("player 锁被毒化"))?;
        player.dispatch(Stamped::new(
            self.gate.session(),
            self.gate.current(),
            PlayerCommand::SetLifecycle(lifecycle),
        ))?;
        self.note(format!("已下发 SetLifecycle({lifecycle:?})"));
        Ok(())
    }
}

/// 渲染回调与 tick 之间共享的东西（**只在 UI 线程上碰**）。
struct Shared {
    findings: Findings,
    gl: Option<gl::Gl>,
    render: Option<mpv::MpvRender>,
    player: Option<Arc<Mutex<mpv::MpvPlayer>>>,
    /// 回调里拿到的是不是 OpenGL——第一轮的主结论，这一轮继续盯着：
    /// surface 重建之后它必须还成立。
    native_opengl: bool,
    frames: u64,
    first_frame_at: Option<Instant>,
    fbo_size: (i32, i32),
    opened: bool,
    /// 上一次画进去的那个 FBO 的 id。
    last_fbo: Option<i32>,
    /// `RenderingSetup` / `RenderingTeardown` 各发生了几次。配不上对就是泄漏。
    setups: u32,
    teardowns: u32,
    /// 下发命令要用**建 player 时那一把**闸门，另起一把只是碰巧代际相同。
    gate: GenerationGate,
    /// 回读落点的目录。
    shot_dir: Option<PathBuf>,
    /// 等着回读的那一帧叫什么名字。
    pending_shot: Option<String>,
    /// 同上，但在 **mpv 刚画完、Slint 还没画**的那一刻回读。
    ///
    /// 两张图一比，「视频是在哪一层没的」就不用猜：pre 有、post 没有 = Slint
    /// 这一层把它盖掉了；两张都没有 = mpv 压根没画进来。
    pending_shot_pre: Option<String>,
    /// 尺寸变化后隔几帧再回读：转屏那一下 Slint 还没重新布局完，立刻读回来的
    /// 是一张半旧半新的图，看着像渲染坏了。
    shot_after: Option<(u64, String)>,
    watch: Arc<Watch>,
}

impl Shared {
    fn say_once_not_opengl(&mut self) {
        self.findings.say_once(
            "渲染回调给的不是 NativeOpenGL：skia 这次没走 OpenGL surface，\
             mpv 的 render API 用不了（#9 要翻到 SurfaceView/纹理合成那条路）",
        );
    }
}

/// 把 JavaVM 交给 FFmpeg。
///
/// `av_jni_set_java_vm` 在 libmpv.so 里（libavcodec 静态链进去的），
/// 这里再 dlopen 一次拿同一个句柄——dlopen 同名库返回的是同一份映射，
/// 不会把 libmpv 装两遍。
fn register_java_vm(vm: *mut c_void) -> Result<()> {
    if vm.is_null() {
        anyhow::bail!("android-activity 没给出 JavaVM 指针");
    }
    type SetJavaVm = unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32;
    let lib = unsafe { libloading::Library::new("libmpv.so") }?;
    let set: libloading::Symbol<'_, SetJavaVm> = unsafe { lib.get(b"av_jni_set_java_vm\0") }?;
    let code = unsafe { set(vm, std::ptr::null_mut()) };
    anyhow::ensure!(code == 0, "av_jni_set_java_vm 返回 {code}");
    // 句柄故意泄掉：注册进去的 JavaVM 指针在整个进程生命周期里都要有效。
    std::mem::forget(lib);
    Ok(())
}

/// 事件只用来证明 mpv 真的在动，不做状态机——那是 SPIKE-001 的事。
///
/// **只记状态变化**：Snapshot 每 30ms 一条，照单全收会把 logcat 的环形
/// 缓冲冲掉——第一轮就是这么把「渲染回调给到 NativeOpenGL」那几行挤没的，
/// 证据没了比没打还糟。
#[derive(Default)]
struct LogSink {
    last_state: Option<PlaybackState>,
}

impl PlayerEventSink for LogSink {
    fn emit(&mut self, event: Stamped<PlaybackEvent>) {
        match &event.value {
            PlaybackEvent::Snapshot(snapshot) => {
                if self.last_state != Some(snapshot.state) {
                    self.last_state = Some(snapshot.state);
                    log::info!(
                        "mpv 状态: {:?} position={}ms duration={:?} video={:?}",
                        snapshot.state,
                        snapshot.position_ms,
                        snapshot.duration_ms,
                        snapshot.video_track
                    );
                }
            }
            other => log::info!("mpv 事件: {other:?}"),
        }
    }
}

#[unsafe(no_mangle)]
fn android_main(app: slint::android::AndroidApp) {
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Info)
            .with_tag("kk-spike"),
    );
    let run = MAIN_RUNS.fetch_add(1, Ordering::AcqRel) + 1;
    log::info!(
        "SPIKE-002 探针启动（issue #9）：android_main 第 {run} 次，pid={}",
        std::process::id()
    );
    if let Err(err) = run_probe(app) {
        // 失败本身也是结论，别让它只变成一个闪退。
        log::error!("探针失败: {err:#}");
    }
}

fn run_probe(app: slint::android::AndroidApp) -> Result<()> {
    // 媒体文件按 android-activity 给的外部数据目录找，不写死 /sdcard 路径：
    // 应用自己的 files 目录不需要任何存储权限，adb push 也够得着。
    let data_dir = app.external_data_path();
    let net = data_dir
        .as_ref()
        .and_then(|dir| load_net_source(&dir.join("source.txt")));
    let net_media = net.as_ref().map(|n| (PathBuf::from(&n.video), (!n.audio.is_empty()).then(|| PathBuf::from(&n.audio))));
    if let Some(net) = net {
        let _ = NET.set(net);
    }
    let media = match &net_media {
        Some((video, _)) => Some(video.clone()),
        None => data_dir
            .as_ref()
            .map(|dir| dir.join("probe.mp4"))
            .filter(|path| path.exists()),
    };
    // 外挂音轨。probe.mp4 本身**没有音频**（mpv 日志里 `audio=eof`），拿它测
    // 后台音频等于拿默片去证明音响好不好。有这个文件就按 DASH 分轨开，
    // 顺带把 `MediaSource::Dash` 这条真实取流路径一起走一遍。
    // 造法见 make-probe-audio.py。
    let audio = match net_media {
        Some((_, audio)) => audio,
        None => data_dir
            .as_ref()
            .map(|dir| dir.join("probe-audio.wav"))
            .filter(|path| path.exists()),
    };

    // 这两个句柄必须在 `slint::android::init*` **之前**取：init 会把 app 搬走。
    let java = JavaHandles {
        vm: app.vm_as_ptr(),
        activity: app.activity_as_ptr(),
    };

    // 把 JavaVM 交给 FFmpeg，否则 mediacodec 的**零拷贝**那条路根本起不来：
    // 第一轮实跑的 mpv 日志里是 `No Java virtual machine has been registered`
    // → `Both surface and native_window are NULL` → 悄悄退到 `mediacodec-copy`，
    // 每帧多一趟 GPU→CPU→GPU，正是 #9 标粗要避免的那个拷贝。
    // 退化是静默的：只有 mpv 自己的日志里有，PlayerPort 那一侧什么都看不到。
    let jvm_registered = register_java_vm(java.vm);

    // player 与 watch 要在装监听器**之前**建好：后台策略挂在 `Main(Pause)` 上，
    // 而 Pause 可能在窗口刚起来时就到（亮屏解锁那一下），晚一步就漏掉了。
    let gate = GenerationGate::new(SessionId(1));
    let player = build_player(&gate, media.as_ref(), data_dir.as_ref());

    let watch = Arc::new(Watch {
        player: player.clone(),
        java,
        media: media.clone(),
        audio: audio.clone(),
        cmd_path: data_dir.as_ref().map(|dir| dir.join("cmd.txt")),
        notes: Mutex::new(Vec::new()),
        shot: Mutex::new(None),
        shot_pre: Mutex::new(None),
        auto_lifecycle: AtomicBool::new(false),
        gate: gate.clone(),
        quit: AtomicBool::new(false),
    });
    // 命令文件可能留着上一轮的内容，先清掉。
    if let Some(path) = watch.cmd_path.as_ref() {
        let _ = std::fs::write(path, b"");
    }

    // 事件监听器要在 init 时装上。android-activity 的 `MainEvent` 是
    // **surface 生命周期这一项的第一手证据**：InitWindow / TerminateWindow /
    // Pause / Resume / ConfigChanged 都从这儿过，比从渲染回调倒推可靠。
    //
    // 它同时是后台策略的触发点：`Pause` 比 `TerminateWindow` 早到，而
    // 「关掉视频轨」这件事**必须赶在 render context 被销毁之前**生效，
    // 否则 mpv 的 VO 一死就把整个文件卸掉（第一轮实测：EOF code 6
    // `video output initialization failed`，连音频一起停）。
    let event_notes: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    {
        let event_notes = Arc::clone(&event_notes);
        let watch_for_events = Arc::clone(&watch);
        slint::android::init_with_event_listener(app, move |event| {
            let text = short_event(&format!("{event:?}"));
            // Wake / Timeout 每几毫秒一条，留着只会把结论冲掉。
            if text.contains("Wake") || text.contains("Timeout") {
                return;
            }
            log::info!("EV {text}");
            if text.starts_with("Main(Pause") {
                watch_for_events.on_pause();
            }
            if let Ok(mut notes) = event_notes.lock() {
                notes.push(text);
            }
        })
        .map_err(|err| anyhow!("slint android 初始化失败: {err}"))?;
    }

    let ui = SpikeWindow::new()?;

    let shared = Rc::new(RefCell::new(Shared {
        findings: Findings::default(),
        gl: None,
        render: None,
        player,
        native_opengl: false,
        frames: 0,
        first_frame_at: None,
        fbo_size: (0, 0),
        opened: false,
        last_fbo: None,
        setups: 0,
        teardowns: 0,
        gate: gate.clone(),
        shot_dir: data_dir.clone(),
        pending_shot: None,
        pending_shot_pre: None,
        shot_after: None,
        watch: Arc::clone(&watch),
    }));

    {
        let mut state = shared.borrow_mut();
        state.findings.say(format!(
            "android_main 第 {} 次",
            MAIN_RUNS.load(Ordering::Acquire)
        ));
        state.findings.say(match &jvm_registered {
            Ok(()) => "已把 JavaVM 注册给 FFmpeg（av_jni_set_java_vm）".to_owned(),
            Err(err) => format!("JavaVM 没注册上，硬解会退到 -copy: {err}"),
        });
        state.findings.say(if java.is_usable() {
            "拿到 JavaVM + Activity 句柄（PiP 要用）".to_owned()
        } else {
            "没有 Activity 句柄，PiP 这一项记 BLOCKED".to_owned()
        });
        state.findings.say(match &audio {
            Some(path) => format!("外挂音轨: {}", path.display()),
            None => "没有 probe-audio.wav：probe.mp4 本身没有音频，后台音频这一项会测不出东西"
                .to_owned(),
        });
        match &media {
            Some(path) => state.findings.say(format!("媒体: {}", path.display())),
            None => state
                .findings
                .say("没有找到 probe.mp4：只验 GL 上下文这一半，mpv 那一半记 BLOCKED"),
        }
    }

    install_notifier(&ui, &shared)?;
    install_input(&ui, &shared);

    // 几条静态弹幕：层级要靠截图证明——弹幕必须压在视频之上。
    // 这里刻意不接 bili-danmaku 的真实流，那是 SPIKE-001 已经验过的事，
    // 本次多接一层只会让失败原因变多。
    ui.set_items(sample_items());
    ui.set_video_active(media.is_some());
    ui.set_hud_visible(true);
    ui.set_controls_visible(true);
    ui.set_watermark("SPIKE-002 · #9".into());

    spawn_watchdog(Arc::clone(&watch));

    let tick = slint::Timer::default();
    {
        let shared = Rc::clone(&shared);
        let ui_weak = ui.as_weak();
        let media = media.clone();
        let audio = audio.clone();
        let event_notes = Arc::clone(&event_notes);
        // 16ms 不是为了刷 HUD，是为了**驱动渲染**：Slint 按需重绘，UI 不变
        // 就不出帧，而 mpv 只在 BeforeRendering 里画——第一轮 200ms 的 tick
        // 把整条管线压到了 5fps，看起来像是 mpv 不出帧。
        tick.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(16),
            move || {
                let Some(ui) = ui_weak.upgrade() else { return };
                let mut state = shared.borrow_mut();

                // FBO 尺寸每次 tick 都更新：Android 上没有命令行传进来的宽高，
                // 而窗口尺寸在旋转、进 PiP、系统栏变化时都会改。
                let size = ui.window().size();
                let (width, height) = (size.width as i32, size.height as i32);
                if (width, height) != state.fbo_size {
                    let before = state.fbo_size;
                    state.fbo_size = (width, height);
                    let shape = if width > height { "横" } else { "竖" };
                    state.findings.say(format!(
                        "SIZE {}x{} -> {width}x{height}（{shape}屏）",
                        before.0, before.1
                    ));
                    report_buttons(&ui, &mut state);
                    // 尺寸变了就留一张证据图，但要等布局跟上（见字段说明）。
                    let frames = state.frames;
                    state.shot_after = Some((frames + 20, format!("size-{width}x{height}")));
                }

                // 状态线程与事件监听器攒下的行，贴到 HUD 上。
                let notes = state.watch.drain_notes();
                for line in notes {
                    state.findings.say(line);
                }
                for line in drain(&event_notes) {
                    state.findings.say(format!("EV {line}"));
                }

                if let Some(name) = state.watch.take_shot_request() {
                    state.pending_shot = Some(name);
                }
                if let Some(name) = state.watch.take_shot_pre_request() {
                    state.pending_shot_pre = Some(name);
                }
                if let Some((at, name)) = state.shot_after.clone()
                    && state.frames >= at
                {
                    state.shot_after = None;
                    state.pending_shot = Some(name);
                }

                // GL 与 render context 都就绪之后才开播：顺序反了的话
                // mpv 会把第一帧画到一个还不存在的 FBO 上。
                if !state.opened
                    && state.gl.is_some()
                    && (state.player.is_none() || state.render.is_some())
                    && let (Some(player), Some(path)) = (state.player.clone(), media.as_ref())
                {
                    state.opened = true;
                    let gate = state.gate.clone();
                    match open(&player, &gate, path, audio.as_ref()) {
                        Ok(()) => state.findings.say("已下发 Open + Play"),
                        Err(err) => state.findings.say(format!("开播失败: {err:#}")),
                    }
                }

                // 跑够一段时间就把 mpv 自己选中的硬解档位打出来。
                if state.frames == 60
                    && let Some(player) = state.player.clone()
                    && let Ok(player) = player.lock()
                {
                    let hwdec = player
                        .property_string("hwdec-current")
                        .unwrap_or_else(|| "<读不到>".to_owned());
                    let codec = player
                        .property_string("video-codec")
                        .unwrap_or_else(|| "<读不到>".to_owned());
                    let size = format!(
                        "{}x{}",
                        player.property_i64("dwidth").unwrap_or(0),
                        player.property_i64("dheight").unwrap_or(0)
                    );
                    state
                        .findings
                        .say(format!("hwdec-current={hwdec} codec={codec} 尺寸={size}"));
                }

                // 文本必须真的变，否则 Slint 判不脏、不出帧，mpv 也就不画。
                let hud = format!(
                    "frame={} setup={} teardown={}\n{}",
                    state.frames,
                    state.setups,
                    state.teardowns,
                    state.findings.hud()
                );
                ui.set_hud(hud.into());
            },
        );
    }

    ui.run()?;
    watch.quit.store(true, Ordering::Release);
    Ok(())
}

/// `MainEvent` 的 Debug 里带着整个 `AndroidApp`：`Resume` 一条就是 1.5KB，
/// 几条就能把 logcat 的环形缓冲冲掉（第一轮已经栽过一次，见本文件开头）。
/// 结论只需要到变体名这一层，所以在第一个 `{` 处截断（`Main(Start)` 这种
/// 没有花括号的原样留着，`Main(Resume { .. })` 截成 `Main(Resume`）。
fn short_event(text: &str) -> String {
    let end = text.find('{').unwrap_or(text.len());
    text[..end].trim_end().trim_end_matches(' ').to_owned()
}

fn drain(notes: &Arc<Mutex<Vec<String>>>) -> Vec<String> {
    notes
        .lock()
        .map(|mut notes| std::mem::take(&mut *notes))
        .unwrap_or_default()
}

/// 按钮中心的**物理**像素坐标：`adb shell input tap` 要的就是这个。
///
/// 逻辑像素 × scale factor。转屏、进 PiP 之后都要重报一次，否则驱动脚本会按
/// 旧坐标去点一个已经不在那儿的按钮，然后得出「触摸没响应」的错结论。
fn report_buttons(ui: &SpikeWindow, state: &mut Shared) {
    let scale = ui.window().scale_factor();
    let to_physical = |value: f32| (value * scale).round() as i32;
    state.findings.say(format!(
        "BUTTONS scale={scale:.2} pip={},{} shot={},{}",
        to_physical(ui.get_pip_cx()),
        to_physical(ui.get_pip_cy()),
        to_physical(ui.get_shot_cx()),
        to_physical(ui.get_shot_cy()),
    ));
}

fn build_player(
    gate: &GenerationGate,
    media: Option<&PathBuf>,
    data_dir: Option<&PathBuf>,
) -> Option<Arc<Mutex<mpv::MpvPlayer>>> {
    media?;
    match mpv::sys::MpvLib::load(None) {
        Ok(lib) => {
            log::info!("libmpv.so 已加载");
            // mpv 自己的日志落到应用的外部数据目录，adb pull 得出来。
            // `Failed(Backend)` 这类事件只说「后端失败了」，不说为什么——
            // 第一轮实跑就撞上一次，原因只在 mpv 的日志里。
            let log_file = data_dir.map(|dir| dir.join("mpv.log").to_string_lossy().into_owned());
            let config = mpv::MpvConfig {
                log_file,
                // 显式要 mediacodec（零拷贝那条）。第一轮用 auto-safe
                // 实测落在 `mediacodec-copy` 上——auto-safe 不含非 copy 的
                // mediacodec，而 #9 标粗要避免的正是每帧那趟拷贝。
                // 真实选中的档位以 `hwdec-current` 为准，不以这里要了什么为准。
                hwdec: "mediacodec".to_owned(),
                loop_file: true,
                ..Default::default()
            };
            match mpv::MpvPlayer::new(
                Arc::new(lib),
                &config,
                gate.clone(),
                Box::new(LogSink::default()),
            ) {
                Ok(player) => {
                    log::info!("mpv 实例已创建（vo=libmpv）");
                    Some(Arc::new(Mutex::new(player)))
                }
                Err(err) => {
                    log::error!("mpv 创建失败: {err:#}");
                    None
                }
            }
        }
        Err(err) => {
            log::error!("加载 libmpv.so 失败: {err:#}");
            None
        }
    }
}

fn spawn_watchdog(watch: Arc<Watch>) {
    let spawned = std::thread::Builder::new()
        .name("kk-watch".to_owned())
        .spawn(move || {
            let mut last_pip = false;
            let mut ticks: u64 = 0;
            loop {
                std::thread::sleep(Duration::from_millis(250));
                if watch.quit.load(Ordering::Acquire) {
                    break;
                }
                ticks += 1;

                if let Some(command) = watch.take_command() {
                    watch.run_command(&command);
                }

                // PiP 的进出不走任何回调（NativeActivity 收不到
                // onPictureInPictureModeChanged），只能轮询。
                if watch.java.is_usable()
                    && let Ok(active) = pip::is_active(watch.java)
                    && active != last_pip
                {
                    last_pip = active;
                    watch.note(format!("PIP isInPictureInPictureMode={active}"));
                }

                if ticks % 4 == 0 {
                    watch.log_status();
                }
            }
        });
    if let Err(err) = spawned {
        log::error!("状态线程起不来: {err}");
    }
}

fn open(
    player: &Arc<Mutex<mpv::MpvPlayer>>,
    gate: &GenerationGate,
    path: &PathBuf,
    audio: Option<&PathBuf>,
) -> Result<()> {
    let generation = gate.current();
    let source = match audio {
        Some(audio) => MediaSource::Dash {
            video: Redacted::new(path.to_string_lossy().into_owned()),
            audio: Redacted::new(audio.to_string_lossy().into_owned()),
        },
        None => MediaSource::Segments(vec![Redacted::new(path.to_string_lossy().into_owned())]),
    };
    let mut player = player.lock().map_err(|_| anyhow!("player 锁被毒化"))?;
    player.dispatch(Stamped::new(
        gate.session(),
        generation,
        PlayerCommand::Open {
            source,
            headers: MediaHeaders {
                user_agent: NET.get().map(|n| n.ua.clone()).unwrap_or_default(),
                referer: NET.get().map(|n| n.referer.clone()).unwrap_or_default(),
            },
        },
    ))?;
    player.dispatch(Stamped::new(
        gate.session(),
        generation,
        PlayerCommand::Play,
    ))?;
    Ok(())
}

/// 触摸分发这一项。
///
/// 要分得开两件事才算答完：
/// - 点在**按钮**上 → 按钮的 `clicked`（命中测试把事件给了最上面那个元素）；
/// - 点在**别处** → 铺满窗口的 TouchArea 的 pointer-event。
///
/// 视频不在元素树里（它是 BeforeRendering 时画进同一个 FBO 的），所以
/// 「视频会不会把触摸吃掉」这一问的答案，就是这两条都收得到。
fn install_input(ui: &SpikeWindow, shared: &Rc<RefCell<Shared>>) {
    {
        let shared = Rc::clone(shared);
        let moves = std::cell::Cell::new(0u32);
        ui.on_touch(move |kind, x, y| {
            // 一次滑动的 move 能来上百条，全打会把 logcat 冲掉；只留前几条，
            // 抬手时再把总数报出来（手势分发看的就是这个数不是 0）。
            if kind == "move" {
                let count = moves.get() + 1;
                moves.set(count);
                if count > 3 {
                    return;
                }
            }
            if kind == "up" {
                let count = moves.replace(0);
                shared.borrow_mut().findings.say(format!(
                    "TOUCH up x={x:.0} y={y:.0} move={count} 目标=画面层"
                ));
                return;
            }
            shared
                .borrow_mut()
                .findings
                .say(format!("TOUCH {kind} x={x:.0} y={y:.0} 目标=画面层"));
        });
    }
    {
        let shared = Rc::clone(shared);
        ui.on_pip_pressed(move || {
            let mut state = shared.borrow_mut();
            state.findings.say("TOUCH click 目标=PiP 按钮");
            let java = state.watch.java;
            // 按钮这条路是**在 UI 线程上**调的，与命令通道那条（状态线程）
            // 形成对照：真实播放器的 PiP 按钮就在 UI 线程上。
            match pip::enter(java, Some((16, 9))) {
                Ok(entered) => state
                    .findings
                    .say(format!("PIP enter(来自 UI 线程) = {entered}")),
                Err(err) => state.findings.say(format!("PIP enter 失败: {err:#}")),
            }
        });
    }
    {
        let shared = Rc::clone(shared);
        ui.on_shot_pressed(move || {
            let mut state = shared.borrow_mut();
            state.findings.say("TOUCH click 目标=SHOT 按钮");
            state.pending_shot = Some("manual".to_owned());
        });
    }
}

/// 探针的核心：这个回调拿到的 `GraphicsAPI` 是什么，以及 surface 重建之后它
/// 还成不成立。
fn install_notifier(ui: &SpikeWindow, shared: &Rc<RefCell<Shared>>) -> Result<()> {
    let shared = Rc::clone(shared);
    ui.window()
        .set_rendering_notifier(move |state, api| {
            let slint::GraphicsAPI::NativeOpenGL { get_proc_address } = api else {
                let mut inner = shared.borrow_mut();
                // 这就是 #9 的「走不通」分支：拿不到 GL 上下文，mpv 的
                // render API 无从下手，答案翻到 SurfaceView／纹理合成。
                inner.say_once_not_opengl();
                return;
            };
            let loader = |name: &std::ffi::CStr| get_proc_address(name);
            let mut inner = shared.borrow_mut();
            inner.native_opengl = true;

            match state {
                slint::RenderingState::RenderingSetup => {
                    inner.setups += 1;
                    let nth = inner.setups;
                    inner.findings.say(format!(
                        "SURFACE setup #{nth}：给到 GraphicsAPI::NativeOpenGL"
                    ));
                    match unsafe { gl::Gl::load(&loader) } {
                        Ok(gl) => {
                            inner.findings.say(format!("GL: {}", gl.describe()));
                            inner.gl = Some(gl);
                        }
                        Err(err) => inner.findings.say(format!("GL 函数加载失败: {err:#}")),
                    }
                    if let Some(player) = inner.player.clone() {
                        let created = {
                            let player = player.lock().expect("player 锁被毒化");
                            unsafe { player.create_render(&loader) }
                        };
                        match created {
                            Ok(render) => {
                                inner.findings.say(format!(
                                    "SURFACE mpv render context 已建立（第 {nth} 次）"
                                ));
                                inner.render = Some(render);
                            }
                            Err(err) => inner
                                .findings
                                .say(format!("SURFACE mpv render context 建不起来: {err:#}")),
                        }
                    }
                    // 回到前台：把视频轨要回来（只有开了 auto-lifecycle 才管）。
                    if nth > 1 && inner.watch.auto_lifecycle.load(Ordering::Acquire) {
                        let watch = Arc::clone(&inner.watch);
                        if let Err(err) = watch.cmd_lifecycle(Lifecycle::Foreground) {
                            inner
                                .findings
                                .say(format!("回前台的 SetLifecycle 失败: {err:#}"));
                        }
                    }
                    // 重建之后照一张：画面有没有真的回来，看图比看日志硬。
                    if nth > 1 {
                        let frames = inner.frames;
                        inner.shot_after = Some((frames + 20, format!("resume-{nth}")));
                    }
                }
                slint::RenderingState::BeforeRendering => {
                    let Some(gl) = inner.gl.as_ref() else { return };
                    let fbo = gl.draw_framebuffer();
                    let (width, height) = inner.fbo_size;
                    if width <= 0 || height <= 0 {
                        return;
                    }
                    // 每换一张 surface 都报一次：FBO 的 id 重建之后可能变，
                    // 而「画进哪个 FBO」是这条路的全部前提。
                    if inner.frames == 0 || inner.last_fbo != Some(fbo) {
                        inner.last_fbo = Some(fbo);
                        inner
                            .findings
                            .say(format!("draw FBO = {fbo}，尺寸 {width}x{height}"));
                    }
                    let render_started = Instant::now();
                    if let Some(render) = inner.render.as_ref() {
                        // flip_y：Android 这边实测要翻（第一轮的结论），桌面不用。
                        // 方向不照抄平台。
                        match unsafe { render.render(fbo, width, height, true, &loader) } {
                            Ok(()) => {
                                let us = render_started.elapsed().as_micros() as u64;
                                RENDER_MAX_US.fetch_max(us, Ordering::Relaxed);
                                RENDER_SUM_US.fetch_add(us, Ordering::Relaxed);
                                RENDER_N.fetch_add(1, Ordering::Relaxed);
                                if inner.first_frame_at.is_none() {
                                    inner.first_frame_at = Some(Instant::now());
                                    inner.findings.say("mpv 已往这个 FBO 画了第一帧");
                                }
                                if let Some(name) = inner.pending_shot_pre.take() {
                                    capture(&mut inner, &name);
                                }
                            }
                            Err(err) => {
                                let message = format!("mpv 渲染失败: {err:#}");
                                inner.findings.say_once(message);
                            }
                        }
                    }
                }
                slint::RenderingState::AfterRendering => {
                    inner.frames += 1;
                    UI_FRAMES.fetch_add(1, Ordering::Relaxed);
                    // 合成完之后把整帧读回来存成 PNG。
                    //
                    // 为什么不靠 `adb exec-out screencap`：那条路量的是**系统
                    // 合成之后**的结果，中间还隔着 SurfaceFlinger；读回 FBO
                    // 量的是「我们这一层到底画成了什么」。两者不一致时，问题
                    // 出在哪一层一眼就能分开——PiP 那一项正要用这个区分：小窗
                    // 里看到的是系统缩放后的结果，FBO 里是原始合成。
                    if let Some(name) = inner.pending_shot.take() {
                        capture(&mut inner, &name);
                    }
                }
                slint::RenderingState::RenderingTeardown => {
                    inner.teardowns += 1;
                    let nth = inner.teardowns;
                    // 画面没了就把视频轨关掉、只留音频——这正是 ADR-003 里
                    // `SetLifecycle(Background)` 要表达的策略。真正的下发在
                    // `Main(Pause)` 那一步（更早），**这里只确认它落地了**，
                    // 而且必须赶在下面销毁 render context 之前：VO 一死，
                    // 还挂着视频轨的文件会被整个卸掉，音频跟着停。
                    if inner.watch.auto_lifecycle.load(Ordering::Acquire) {
                        let watch = Arc::clone(&inner.watch);
                        let landed = watch.wait_for_vid("no");
                        inner.findings.say(format!(
                            "SURFACE teardown 前 vid=no {}",
                            if landed {
                                "已生效"
                            } else {
                                "没等到（后台会断）"
                            }
                        ));
                    }
                    // render context 必须在 GL 上下文还活着时销毁。
                    inner.render = None;
                    inner.gl = None;
                    inner.findings.say(format!(
                        "SURFACE teardown #{nth}：GL 与 render context 已销毁"
                    ));
                }
                _ => {}
            }
        })
        .map_err(|err| anyhow!("装不上渲染回调: {err}"))?;

    Ok(())
}

/// 把当前 FBO 读回成 PNG。调用点必须在渲染回调里（那时 GL 上下文是当前的）。
fn capture(state: &mut Shared, name: &str) {
    let Some(dir) = state.shot_dir.clone() else {
        return;
    };
    let (width, height) = state.fbo_size;
    let path = dir.join(format!("shot-{name}.png"));
    let Some(gl) = state.gl.as_ref() else { return };
    match gl.capture_png(width as u32, height as u32, &path) {
        Ok(()) => state.findings.say(format!("SHOT {}", path.display())),
        Err(err) => state.findings.say(format!("SHOT 失败: {err:#}")),
    }
}

fn sample_items() -> slint::ModelRc<DanmakuItem> {
    let rows: Vec<DanmakuItem> = (0..8)
        .map(|i| DanmakuItem {
            text: slint::SharedString::from(format!("SPIKE-002 弹幕层 {i}")),
            x: 40.0 + (i as f32) * 10.0,
            y: 60.0 + (i as f32) * 56.0,
            size: 28.0,
            color: slint::Color::from_rgb_u8(255, 255, 255),
            alpha: 1.0,
        })
        .collect();
    slint::ModelRc::new(slint::VecModel::from(rows))
}
