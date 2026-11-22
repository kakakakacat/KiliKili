//! SPIKE-003（issue #10）的 crate 根：模块挂在这里，两个入口共用。
//!
//! - 桌面：`src/main.rs` 这个 bin，`Args::parse()` 之后调 [`app::run`]；
//! - Android：下面的 `android_main`（android-activity 的入口只能在 cdylib 里
//!   导出），自己拼一份 `Args` 再调**同一个** [`app::run`]。
//!
//! 为什么不像 `slint-mpv` 那样给 Android 另写一个探针：SPIKE-002 要回答的是
//! 「surface 生命周期、PiP、后台音频」这类**只有 Android 才有**的问题，另写
//! 一份是合理的。SPIKE-003 要回答的是「同一套界面在三个平台上表现差多少」，
//! 两边各跑一份不同的流程，记录就没法逐行对照——那正是 `spike-harness` 存在
//! 的理由。代价是 `app.rs` 从 `main.rs` 改了名并抽出 `run()`，Windows 那一轮
//! 的数字必须重跑一遍确认没动，见 `docs/m0/spikes/` 里的记录。

pub mod app;

mod drive;
mod flow;
mod rows;
mod script;

slint::include_modules!();

/// Android 入口。桌面上这一段整个不编译。
///
/// 这里**不解析命令行**——Android 上没有命令行可解析。参数写死成一份与
/// Windows 那一轮等价的配置，再走 `Args::parse_from`，这样「记录里写的那行
/// 命令」和「实际跑的参数」仍然是同一个来源，不会各写一份然后慢慢对不上。
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: slint::android::AndroidApp) {
    use clap::Parser;

    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Info)
            .with_tag("kk-spike"),
    );
    // panic 默认只进 stderr，而 Android 上 stderr 没人看。不装这个钩子的话，
    // 崩溃表现为「进程没了、logcat 里什么都没有」，查起来全靠猜。
    std::panic::set_hook(Box::new(|info| {
        log::error!("panic: {info}");
    }));

    // 记录与截图只能写进应用自己的目录。外部目录（/sdcard/Android/data/...）
    // 优先，因为 `adb pull` 取它不需要 run-as，设备没 root 也拿得走。
    let out = app
        .external_data_path()
        .or_else(|| app.internal_data_path())
        .unwrap_or_else(|| std::path::PathBuf::from("/data/local/tmp"));
    let record = out.join("record.md");
    let shots = out.join("shots");
    log::info!("SPIKE out={} ", out.display());

    if let Err(err) = slint::android::init(app) {
        log::error!("slint::android::init 失败: {err}");
        return;
    }

    let argv = vec![
        "slint-ui-spike".to_owned(),
        "--scripted".to_owned(),
        "--rows".to_owned(),
        "20000".to_owned(),
        // 外部驱动在**宿主机**上（adb），不在进程里。Windows 那一轮是
        // 进程自己起 PowerShell，Android 上做不到，所以按键 / IME / 无障碍
        // 三项由 `android-drive.sh` 从外面做，记录里按 NOT TESTED 留白，
        // 再由那个脚本补。
        "--no-driver".to_owned(),
        // 列表场景末尾留 8 秒空窗给真实触摸：Windows 上没有触摸屏，
        // 「惯性」这一项是 Android 才答得了的。
        "--touch-ms".to_owned(),
        "8000".to_owned(),
        "--record".to_owned(),
        record.display().to_string(),
        "--shots".to_owned(),
        shots.display().to_string(),
    ];

    match app::run(app::Args::parse_from(&argv), argv.clone()) {
        Ok(()) => log::info!("SPIKE DONE record={}", record.display()),
        Err(err) => log::error!("SPIKE FAILED {err:#}"),
    }
}
