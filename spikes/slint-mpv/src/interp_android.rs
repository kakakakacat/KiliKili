//! #218 插帧 spike 的 Android 入口（cargo feature `interp`）。
//!
//! 打法与 SPIKE-002 探针一样（cargo-apk + `android_main`），但不带探针的
//! PiP / 触摸 / 生命周期那一套：这里只回答「Adreno 610 上跑不跑得动」。
//!
//! 参数从应用外部数据目录的 `args.txt` 读（一行，与桌面命令行同格式），
//! 相对路径按该目录解析；记录写进 `--record` 指定的文件，同时逐行打到
//! logcat（tag `kk-interp`）。驱动见 `interp-android.sh`。

use std::ffi::c_void;

use anyhow::{Context, Result};
use clap::Parser;

use crate::interp::app::{self, Args};

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
    slint::android::init(app).map_err(|err| anyhow::anyhow!("slint android init：{err}"))?;
    app::run(args)
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
