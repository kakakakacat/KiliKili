//! Liquid Glass 实验台（issue #130）的 crate 根，两个入口共用。
//!
//! - 桌面：`src/main.rs` 这个 bin，直接调 [`run`]；
//! - Android：下面的 `android_main`（android-activity 的入口只能在 cdylib 里
//!   导出），调**同一个** [`run`]。
//!
//! 两边跑同一个函数、同一个画面，是因为这个 spike 要回答的就是「同一份
//! `backdrop-blur` 在桌面和 Android 上表现差多少」。各跑一份不同的流程，
//! 两边的截图就没法并排看了。
//!
//! 实测状态见
//! `docs/m0/spikes/2026-11-24-glass-windows-notes.md`。

// material 还没有被接进来（评估文档 §6 的第三步才用），但它是这个 spike 里
// 最该被语法与类型检查看着的东西，所以先编进去，别让它在磁盘上烂掉。
#![allow(dead_code)]

mod material;

slint::include_modules!();

/// 桌面与 Android 共用的那一段。
pub fn run() -> Result<(), slint::PlatformError> {
    let app = GlassSpike::new()?;

    // 用环境变量做对照，不引 clap——这个 spike 还没到需要参数解析的时候，
    // 多一个依赖就多一截首次编译时间。
    //
    //   GLASS_BLUR=0   关掉 backdrop-blur
    //   GLASS_COUNT=n  同时摆几条玻璃（#130 质量表要的 1 / 5 / 10 档位）
    //
    // Android 上没有命令行，但 `std::env::set_var` 在 `android_main` 里照样
    // 管用，所以这两个开关两边通用。
    if std::env::var("GLASS_BLUR").as_deref() == Ok("0") {
        app.set_blur_sigma(0.0);
    }
    if let Ok(n) = std::env::var("GLASS_COUNT")
        && let Ok(n) = n.parse::<i32>()
    {
        // 窗口高度装不下太多：桌面 915×597 逻辑像素下只看得见 6 条，填 10
        // 的话多出来的 y 为负、被裁在窗口外，而帧率读数照常产出。
        // 2026-11-24 第一轮就是这么把「10 组件」那一档记错的，见 notes §4.1。
        // **跑之前先截图数一遍实际可见条数。**
        app.set_glass_count(n);
    }

    app.run()
}

/// Android 入口。桌面上这一段整个不编译。
///
/// 它和桌面唯一的区别是环境：没有命令行、没有 stdout、崩溃不留痕。所以这里
/// 做三件桌面不需要的事——接 logcat、装 panic 钩子、把「跑到哪了」打成可被
/// `adb logcat -s kk-glass` 脚本化取走的标记行。
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: slint::android::AndroidApp) {
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Info)
            .with_tag("kk-glass"),
    );
    // panic 默认只进 stderr，而 Android 上 stderr 没人看。不装这个钩子的话，
    // 崩溃表现为「进程没了、logcat 里什么都没有」，查起来全靠猜。
    // 这条是 slint-ui（#10）那边交过学费的，照搬。
    std::panic::set_hook(Box::new(|info| {
        log::error!("panic: {info}");
    }));

    // 外部数据目录优先：`adb pull` / `adb push` 取它不需要 run-as，设备没
    // root 也进得去。必须在 `slint::android::init` 之前取——那之后 `app`
    // 的所有权就交出去了。
    let app_external_dir = app.external_data_path().or_else(|| app.internal_data_path());
    log::info!("GLASS STEP out={app_external_dir:?}");

    // Android 上 Slint 走的是 **Skia**，不是桌面那条 FemtoVG——
    // `slint::platform::femtovg_renderer` 整个模块标着 `non-Android only`。
    // 也就是说这一轮在 Android 上验的是「Skia 的 backdrop-blur」，与桌面那轮
    // 不是同一个实现，**两边的结论不能互相代替**。
    log::info!("GLASS STEP init renderer=skia(android-activity)");

    // 把 `GLASS_BLUR` / `GLASS_COUNT` 从配置文件搬进环境变量。
    //
    // 为什么要这么绕：`adb shell am start` **传不了环境变量**，而桌面那两个
    // 开关读的就是环境变量。要么在 Android 上另写一套参数通道（两边逻辑分叉，
    // 对照时就要先确认「两边跑的是不是同一组参数」），要么像这里一样把文件
    // 翻译成环境变量——`run()` 那边一个字都不用改。
    //
    // 放外部数据目录是因为 adb 推得进去、app 自己也读得到，且不需要 root：
    //   adb shell mkdir -p /sdcard/Android/data/<pkg>/files
    //   echo BLUR=0 > glass.conf
    //   echo COUNT=5 >> glass.conf
    //   adb push glass.conf /sdcard/Android/data/<pkg>/files/
    // 没有这个文件就按默认跑（blur 开、一条玻璃），这是最常用的那一档。
    if let Some(dir) = app_external_dir.as_ref() {
        let conf = dir.join("glass.conf");
        match std::fs::read_to_string(&conf) {
            Ok(text) => {
                for line in text.lines() {
                    let line = line.trim();
                    let Some((k, v)) = line.split_once('=') else { continue };
                    let key = match k.trim() {
                        "BLUR" => "GLASS_BLUR",
                        "COUNT" => "GLASS_COUNT",
                        other => {
                            log::warn!("GLASS conf 里不认识的键: {other}");
                            continue;
                        }
                    };
                    log::info!("GLASS conf {key}={}", v.trim());
                    // SAFETY: 这里还在 android_main 的开头，Slint 尚未 init，
                    // 没有别的线程在读环境。
                    unsafe { std::env::set_var(key, v.trim()) };
                }
            }
            Err(err) => log::info!("GLASS conf 没读到（按默认跑）: {err}"),
        }
    }

    if let Err(err) = slint::android::init(app) {
        log::error!("GLASS FAILED slint::android::init: {err}");
        return;
    }

    log::info!("GLASS STEP run");
    match run() {
        Ok(()) => log::info!("GLASS DONE"),
        Err(err) => log::error!("GLASS FAILED {err}"),
    }
}
