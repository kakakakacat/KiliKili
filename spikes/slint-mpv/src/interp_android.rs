//! #218 插帧 spike 的 Android 入口（cargo feature `interp`）。
//!
//! 打法与 SPIKE-002 探针一样（cargo-apk + `android_main`），但不带探针的
//! PiP / 触摸 / 生命周期那一套：这里只回答「Adreno 610 上跑不跑得动」。
//!
//! 参数从应用外部数据目录的 `args.txt` 读（一行，与桌面命令行同格式），
//! 相对路径按该目录解析；记录写进 `--record` 指定的文件，同时逐行打到
//! logcat（tag `kk-interp`）。驱动见 `interp-android.sh`。

use std::ffi::c_void;
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

use anyhow::{Context, Result};
use clap::Parser;
use player_interp::Thermal;

use crate::interp::app::{self, Args, Platform};

#[unsafe(no_mangle)]
fn android_main(app: slint::android::AndroidApp) {
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Info)
            .with_tag("kk-interp"),
    );
    match run(app) {
        Ok(report) => {
            for line in report.lines() {
                log::info!("REPORT {line}");
            }
            log::info!("DONE");
        }
        Err(err) => log::error!("FAILED {err:#}"),
    }
}

fn run(app: slint::android::AndroidApp) -> Result<String> {
    let dir = app.external_data_path().context("拿不到外部数据目录")?;
    std::env::set_current_dir(&dir)?;
    let line =
        std::fs::read_to_string(dir.join("args.txt")).context("外部数据目录里没有 args.txt")?;
    let line = line.trim().to_owned();
    log::info!("ARGS {line}");
    let mut args = Args::try_parse_from(
        std::iter::once("interp".to_owned()).chain(line.split_whitespace().map(str::to_owned)),
    )
    .map_err(|err| anyhow::anyhow!("{err}"))?;
    args.raw = line;

    // mediacodec 零拷贝要先把 JavaVM 交给 FFmpeg（SPIKE-002 的结论，见
    // android_probe.rs 的 register_java_vm）。必须在 init 之前取指针。
    if let Err(err) = register_java_vm(app.vm_as_ptr()) {
        log::warn!("av_jni_set_java_vm 失败，mediacodec 会退到 copy：{err:#}");
    }
    let platform = AndroidPlatform::new(app.clone());
    slint::android::init(app).map_err(|err| anyhow::anyhow!("slint android init：{err}"))?;
    app::run_with(args, Box::new(platform))
}

/// #219 的 Android 平台胶水。三个 NDK 函数都是 API 30 才有，而探针的
/// minSdk 是 24，所以运行期 dlsym，缺了就当没有（退回按 vsync 间隔测刷新率、
/// 没有温控读数）。
///
/// 正式客户端里这一层属于 `NativeMpvAdapter` 的平台层（ADR-004 决策三），
/// 不进 `player-interp`。
struct AndroidPlatform {
    app: slint::android::AndroidApp,
    lib: Option<libloading::Library>,
    thermal_mgr: Option<usize>,
    /// 刷新率回调写进来的周期（纳秒），0 = 还没收到。
    period_ns: Arc<AtomicI64>,
    registered: bool,
}

type SetFrameRate = unsafe extern "C" fn(*mut c_void, f32, i8) -> i32;
type GetChoreographer = unsafe extern "C" fn() -> *mut c_void;
type RefreshCallback = unsafe extern "C" fn(i64, *mut c_void);
type RegisterRefresh = unsafe extern "C" fn(*mut c_void, RefreshCallback, *mut c_void);
type AcquireThermal = unsafe extern "C" fn() -> *mut c_void;
type ThermalStatus = unsafe extern "C" fn(*mut c_void) -> i32;

/// `ANATIVEWINDOW_FRAME_RATE_COMPATIBILITY_FIXED_SOURCE`：内容就是这个帧率，
/// 系统挑一个整数倍的模式（45 → 90 Hz）。视频应当用它而不是 DEFAULT。
const FIXED_SOURCE: i8 = 1;

unsafe extern "C" fn on_refresh(period_ns: i64, data: *mut c_void) {
    let cell = unsafe { &*data.cast::<AtomicI64>() };
    cell.store(period_ns, Ordering::Release);
}

impl AndroidPlatform {
    fn new(app: slint::android::AndroidApp) -> Self {
        let lib = unsafe { libloading::Library::new("libandroid.so") }.ok();
        let thermal_mgr = lib.as_ref().and_then(|lib| {
            let acquire =
                unsafe { lib.get::<AcquireThermal>(b"AThermal_acquireManager\0") }.ok()?;
            let mgr = unsafe { acquire() };
            (!mgr.is_null()).then_some(mgr as usize)
        });
        Self {
            app,
            lib,
            thermal_mgr,
            period_ns: Arc::new(AtomicI64::new(0)),
            registered: false,
        }
    }

    /// 刷新率回调要挂在有 looper 的线程上：`android_main` 线程就是（Slint
    /// 的事件循环在这条线程上 poll 它），而计时器回调正跑在这条线程上，
    /// 所以第一次被问到刷新率时再注册。
    fn ensure_refresh_callback(&mut self) {
        if self.registered {
            return;
        }
        self.registered = true;
        let Some(lib) = self.lib.as_ref() else { return };
        let (Ok(get), Ok(register)) = (
            unsafe { lib.get::<GetChoreographer>(b"AChoreographer_getInstance\0") },
            unsafe { lib.get::<RegisterRefresh>(b"AChoreographer_registerRefreshRateCallback\0") },
        ) else {
            log::warn!("没有 AChoreographer_registerRefreshRateCallback（API < 30）");
            return;
        };
        let choreographer = unsafe { get() };
        if choreographer.is_null() {
            log::warn!("AChoreographer_getInstance 为空（当前线程没有 looper）");
            return;
        }
        // 回调数据活到进程结束：Arc 泄漏一份引用。
        let data = Arc::into_raw(Arc::clone(&self.period_ns))
            .cast_mut()
            .cast::<c_void>();
        unsafe { register(choreographer, on_refresh, data) };
        log::info!("已注册刷新率回调");
    }
}

impl Platform for AndroidPlatform {
    fn refresh_hz(&self) -> Option<f32> {
        let ns = self.period_ns.load(Ordering::Acquire);
        (ns > 0).then(|| (1e9 / ns as f64) as f32)
    }

    fn thermal(&self) -> Option<Thermal> {
        let lib = self.lib.as_ref()?;
        let mgr = self.thermal_mgr?;
        let status =
            unsafe { lib.get::<ThermalStatus>(b"AThermal_getCurrentThermalStatus\0") }.ok()?;
        // ATHERMAL_STATUS_*：0 NONE、1 LIGHT、2 MODERATE、3 SEVERE、4 CRITICAL、
        // 5 EMERGENCY、6 SHUTDOWN；负数是出错。
        Some(match unsafe { status(mgr as *mut c_void) } {
            i32::MIN..=0 => Thermal::Nominal,
            1 => Thermal::Light,
            2 => Thermal::Moderate,
            3 => Thermal::Severe,
            _ => Thermal::Critical,
        })
    }

    fn request_frame_rate(&mut self, fps: Option<f32>) -> Option<String> {
        self.ensure_refresh_callback();
        let lib = self.lib.as_ref()?;
        let Ok(set) = (unsafe { lib.get::<SetFrameRate>(b"ANativeWindow_setFrameRate\0") }) else {
            return Some("没有 ANativeWindow_setFrameRate（API < 30）".to_owned());
        };
        let Some(window) = self.app.native_window() else {
            return Some("还没有 native window，帧率请求未发".to_owned());
        };
        // 0 = 撤销请求，交还给系统默认策略。
        let rate = fps.unwrap_or(0.0);
        let code = unsafe { set(window.ptr().as_ptr().cast(), rate, FIXED_SOURCE) };
        Some(format!(
            "ANativeWindow_setFrameRate({rate}, FIXED_SOURCE) = {code}"
        ))
    }
}

fn register_java_vm(vm: *mut c_void) -> Result<()> {
    anyhow::ensure!(!vm.is_null(), "android-activity 没给出 JavaVM 指针");
    type SetJavaVm = unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32;
    let lib = unsafe { libloading::Library::new("libmpv.so") }?;
    let set: libloading::Symbol<'_, SetJavaVm> = unsafe { lib.get(b"av_jni_set_java_vm\0") }?;
    let code = unsafe { set(vm, std::ptr::null_mut()) };
    anyhow::ensure!(code == 0, "av_jni_set_java_vm 返回 {code}");
    std::mem::forget(lib);
    Ok(())
}
