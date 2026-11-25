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

// 默认那条路不用 material（它只在 wgpu 探针里真正跑起来），但它是这个 spike
// 里最该被语法与类型检查看着的东西，所以无论开不开 feature 都编进去。
#![allow(dead_code)]

#[cfg(feature = "gl-probe")]
pub mod gl_probe;
mod material;
#[cfg(feature = "wgpu-probe")]
pub mod wgpu_probe;

slint::include_modules!();

/// 桌面与 Android 共用的那一段。
pub fn run() -> Result<(), slint::PlatformError> {
    place_window_without_stealing_focus()?;

    let app = GlassSpike::new()?;
    apply_env_overrides(&app);

    // GL 覆盖探针。桌面和 Android 同一条路径——这正是它比 wgpu 那条强的
    // 地方：两端给 notifier 的都是 `NativeOpenGL`，代码一份。
    //
    // 默认开（编进来了就说明是专门为它跑的这一轮），`GLASS_GL=0` 关掉当
    // 对照组。对照组必须存在：没有它就分不清画面上那条玻璃是 GL 画的还是
    // 原生 backdrop-blur 画的。
    #[cfg(feature = "gl-probe")]
    {
        let on = std::env::var("GLASS_GL").map(|v| v != "0").unwrap_or(true);
        app.set_gl_overlay_visible(on);
        if on {
            gl_probe::install(&app);
        }
    }

    // 到点自己出一张图，然后退出。见 Cargo.toml 里 image 依赖处的理由。
    //
    //   GLASS_SNAPSHOT=shots/x.png   出图路径；不设就正常跑，一切照旧
    //   GLASS_SNAPSHOT_MS=3000       等多久再截（默认 3s，让背景动画跑起来）
    //
    // 必须留足等待时间：Slint 要先把窗口建起来、渲染notifier 要先收到
    // RenderingSetup 建好 GL 资源，第一帧之前截等于截了个空窗口。
    let _snapshot_timer = std::env::var("GLASS_SNAPSHOT").ok().map(|path| {
        let delay = std::env::var("GLASS_SNAPSHOT_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(3000);
        let weak = app.as_weak();
        let timer = slint::Timer::default();
        timer.start(
            slint::TimerMode::SingleShot,
            std::time::Duration::from_millis(delay),
            move || {
                if let Some(app) = weak.upgrade() {
                    save_snapshot(&app, &path);
                }
                let _ = slint::quit_event_loop();
            },
        );
        timer
    });

    app.run()
}

/// 按 `GLASS_WINDOW_POS=x,y`（物理像素）把窗口开在指定位置，并且**不激活**。
///
/// 这不是便利功能，是这台开发机的硬约束：主屏上有人在打游戏，而 Slint 的
/// 窗口默认开在主屏、默认抢焦点——全屏游戏会被踢出来。事后用 SetWindowPos
/// 挪是没用的，焦点那一下已经发生了，所以必须在**窗口创建之前**就定下来。
///
/// `with_active(false)` 同时解决了另一件事：不激活的窗口不会把前台程序打断，
/// 于是人和实验台可以同时用这台机器——这正是 #130 这种要反复起 GUI 的
/// spike 最缺的。
///
/// 不设这个环境变量时整段跳过，行为和以前完全一样。
#[cfg(not(target_os = "android"))]
fn place_window_without_stealing_focus() -> Result<(), slint::PlatformError> {
    let Ok(spec) = std::env::var("GLASS_WINDOW_POS") else {
        return Ok(());
    };
    let Some((x, y)) = spec
        .split_once(',')
        .and_then(|(a, b)| Some((a.trim().parse::<i32>().ok()?, b.trim().parse::<i32>().ok()?)))
    else {
        println!("GLASS_WINDOW_POS 格式应为 `x,y`（物理像素），收到的是 {spec:?}，忽略");
        return Ok(());
    };

    slint::BackendSelector::new()
        .with_winit_window_attributes_hook(move |attrs| {
            attrs
                .with_position(slint::winit_030::winit::dpi::PhysicalPosition::new(x, y))
                // 这一行是关键。
                .with_active(false)
        })
        .select()
}

/// Android 上没有多屏也没有窗口位置，整段不编译。
#[cfg(target_os = "android")]
fn place_window_without_stealing_focus() -> Result<(), slint::PlatformError> {
    Ok(())
}

/// 把当前窗口存成 PNG。
///
/// 一个**必须验的点**：`take_snapshot` 走的是「渲染到离屏纹理」那条路，
/// 和上屏那条不是同一个渲染目标。GL 覆盖探针是在 `AfterRendering` 里往
/// 当前帧缓冲上画的，所以「截出来的图里有没有 GL 玻璃」不是想当然——
/// 上游 femtovg 的 `render_snapshot` 里专门有段注释说 notifier 可能重新
/// 绑定帧缓冲，说明两者确实会互相影响。截完看一眼图再下结论。
fn save_snapshot(app: &GlassSpike, path: &str) {
    use slint::ComponentHandle;
    match app.window().take_snapshot() {
        Ok(buf) => {
            let (w, h) = (buf.width(), buf.height());
            match image::RgbaImage::from_raw(w, h, buf.as_bytes().to_vec()) {
                Some(img) => match img.save(path) {
                    Ok(()) => println!("SNAPSHOT 已写 {path} ({w}x{h})"),
                    Err(err) => println!("SNAPSHOT 写文件失败：{err}"),
                },
                None => println!("SNAPSHOT 像素缓冲尺寸对不上：{w}x{h}"),
            }
        }
        Err(err) => println!("SNAPSHOT take_snapshot 失败：{err}"),
    }
}

/// 把环境变量里的档位套到窗口上。
///
/// 单独拆出来是因为 wgpu 探针走的是另一条启动路径（要先 `BackendSelector`
/// 再建窗口），但**必须吃同一组开关**——两条路分别解析参数的话，对照时
/// 就得先确认「两边理解的 count=10 是不是同一回事」。
pub fn apply_env_overrides(app: &GlassSpike) {
    // 用环境变量做对照，不引 clap——这个 spike 还没到需要参数解析的时候，
    // 多一个依赖就多一截首次编译时间。
    //
    //   GLASS_BLUR=n   backdrop-blur 的高斯标准差，单位逻辑 px；0 等于关掉
    //   GLASS_COUNT=n  同时摆几条玻璃（#130 质量表要的 1 / 5 / 10 档位）
    //
    // Android 上没有命令行，但 `std::env::set_var` 在 `android_main` 里照样
    // 管用，所以这两个开关两边通用。
    if let Ok(v) = std::env::var("GLASS_BLUR")
        && let Ok(sigma) = v.parse::<f32>()
    {
        app.set_blur_sigma(sigma);
    }
    if let Ok(n) = std::env::var("GLASS_COUNT")
        && let Ok(n) = n.parse::<i32>()
    {
        // 2026-11-24 第一轮这里埋过一个坑：固定 92px 间距下，桌面窗口只装得
        // 下 6 条，`GLASS_COUNT=10` 多出来的 4 条 y 为负、裁在窗外，而帧率
        // 读数照常产出，于是记录里「10 组件」那一档的标签是错的（notes §4.1）。
        //
        // 现在 .slint 那边按 count 反推间距与胶囊高度，N 条就真的是 N 条；
        // 画面左上角也印出了实际的 slot / h / 窗口尺寸，截图自带档位。
        app.set_glass_count(n);
    }
    // 融合演示里两个形状的间距，逻辑像素。不设就按动画来回走。
    //
    // 它存在的理由是**可复跑**：动画相位取决于窗口起来之后 Timer 跑了多久，
    // 而启动耗时每次不同，拿动画中的某一帧当证据，那个数字是碰运气来的。
    if let Ok(v) = std::env::var("GLASS_MERGE_GAP")
        && let Ok(gap) = v.parse::<f32>()
    {
        app.set_merge_gap_override(gap);
    }
    // 背景 Timer 的周期。默认 16ms 等于 62.5Hz，和 60Hz 刷新率只差 2.5——
    // 帧率读数 61 时分不清是谁在限速。要判它就把这个调小（如 4ms）再读一次。
    if let Ok(n) = std::env::var("GLASS_TICK")
        && let Ok(n) = n.parse::<i32>()
        && n > 0
    {
        app.set_tick_ms(n);
    }
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
    let app_external_dir = app
        .external_data_path()
        .or_else(|| app.internal_data_path());
    log::info!("GLASS STEP out={app_external_dir:?}");

    // `slint::android::init` 会拿走 app 的所有权，但后面还要用它取 native
    // window 去要帧率。AndroidApp 是 Clone 的（内部是 Arc），克隆一份留着。
    let app_for_frame_rate = app.clone();

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
                    let Some((k, v)) = line.split_once('=') else {
                        continue;
                    };
                    let key = match k.trim() {
                        "BLUR" => "GLASS_BLUR",
                        "COUNT" => "GLASS_COUNT",
                        // 帧率。桌面那轮用的是 `console`，把数字打到 stdout；
                        // Android 上没有 stdout，而 Slint 的 debug_log 走的是
                        // eprintln、**不进 logcat**，所以这里只能用 `overlay`
                        // ——它把帧率画在窗口里，截图直接取得到。
                        //   PERF=refresh_full_speed,overlay
                        "PERF" => "SLINT_DEBUG_PERFORMANCE",
                        // 想要的帧率，见下面 set_frame_rate 那段。
                        "FPS" => "GLASS_FPS",
                        "TICK" => "GLASS_TICK",
                        // GL 覆盖探针的开关，见 run() 里那段。Android 上
                        // 这是唯一的传参通道（am start 传不了环境变量）。
                        "GL" => "GLASS_GL",
                        // 哪一档材质：phase2（默认，全套）/ p0 / baseline
                        // （与 Flutter 同式，用来判「像不像」）。
                        // 见 gl_probe::install。
                        "MATERIAL" => "GLASS_GL_MATERIAL",
                        // 自绘玻璃怎么摆：slot / over-full / over-blur / merge。
                        // over-* 两档是 ADR-016 决策四要验的那个组合。
                        "LAYOUT" => "GLASS_GL_LAYOUT",
                        // 质量档：high / balanced / low（ADR-016 决策五）。
                        "QUALITY" => "GLASS_GL_QUALITY",
                        // 只关自绘模糊，留折射与高光。
                        "GLBLUR" => "GLASS_GL_BLUR",
                        // 融合演示的间距，逻辑像素。见 apply_env_overrides。
                        "MERGEGAP" => "GLASS_MERGE_GAP",
                        // 光源方向钉死在这个角度（度），同时停掉自转。
                        "LIGHT" => "GLASS_GL_LIGHT",
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

    // 向系统声明想要的帧率。
    //
    // 背景：这台手机面板支持 60/90/120，用户选的是 120，但 app 一起来系统
    // 就把它投回 60，于是「跑满 vsync」只能跑到 60——帧率读数全是 61，
    // backdrop-blur 的代价被 16.6ms 的预算完全吃掉，什么都量不出来。
    // adb 侧三条路都压不住（见 Cargo.toml 里 ndk 依赖处的说明），
    // 正规做法是 app 自己用 ANativeWindow_setFrameRate 报一个期望值。
    //
    // 为什么要反复报而不是只报一次：
    // 1. 刚进 android_main 时 surface 还没创建，`native_window()` 是 None；
    // 2. 系统的刷新率仲裁会在 app 起来几秒后重新投票，报一次会被盖掉。
    // 所以挂一个 2 秒的重复 Timer，一直报到进程结束。Timer 必须持有到
    // `run()` 返回，drop 掉就停了——这个绑定不是多余的。
    let _frame_rate_timer = std::env::var("GLASS_FPS")
        .ok()
        .and_then(|v| v.parse::<f32>().ok())
        .map(|fps| {
            let timer = slint::Timer::default();
            timer.start(
                slint::TimerMode::Repeated,
                std::time::Duration::from_secs(2),
                move || {
                    let Some(window) = app_for_frame_rate.native_window() else {
                        log::info!("GLASS fps: native window 还没有，下一拍再报");
                        return;
                    };
                    // Always 而不是 OnlyIfSeamless：60→120 在这块屏上不一定
                    // 算无缝切换，用 OnlyIfSeamless 的话系统会直接忽略请求，
                    // 而且**不报错**——看起来就像这个 API 不管用。
                    let result = window.set_frame_rate_with_change_strategy(
                        fps,
                        ndk::native_window::FrameRateCompatibility::Default,
                        ndk::native_window::ChangeFrameRateStrategy::Always,
                    );
                    log::info!("GLASS fps 请求 {fps} -> {result:?}");
                },
            );
            timer
        });

    log::info!("GLASS STEP run");
    match run() {
        Ok(()) => log::info!("GLASS DONE"),
        Err(err) => log::error!("GLASS FAILED {err}"),
    }
}
