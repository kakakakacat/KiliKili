//! 运行时与日志初始化。
//!
//! 移动端需显式处理 Tokio 初始化、后台任务与线程生命周期
//! （RFC-0001 §3）——不能依赖 `#[tokio::main]`，因为进程入口在
//! Dart/平台侧，不在 Rust。

use std::sync::OnceLock;

use flutter_rust_bridge::frb;
use tracing_subscriber::EnvFilter;

static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

/// 进程内共享的 Tokio 运行时。
///
/// 用 `OnceLock` 而非每次新建：移动端 Dart 侧可能多次调用初始化
/// （热重载、Activity 重建），重复建 runtime 会泄漏线程池。
///
/// 这是内部设施，**不跨 FFI 暴露**：Dart 侧拿到一个 runtime 句柄毫无用处，
/// 而 FRB 会把返回引用的函数降级成 `Future<void>`，产生一个误导性的 API。
///
/// # Panics
/// 运行时无法创建时 panic——此时进程无法工作，继续运行没有意义。
#[frb(ignore)]
#[allow(clippy::expect_used)]
pub fn runtime() -> &'static tokio::runtime::Runtime {
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("bili-core")
            .build()
            .expect("failed to build tokio runtime")
    })
}

/// 初始化日志与运行时。Dart 侧应在启动时调用一次，重复调用无副作用。
///
/// `max_level` 取 `off|error|warn|info|debug|trace`，无法解析时回退
/// 到 `info` 而非报错——日志配置不该成为启动失败的原因。
///
/// 注意：本函数**不接受**任何凭据参数。需要记录含凭据的内容时，
/// 调用方必须先经 [`bili_domain::redact`] 处理。
pub fn init_app(max_level: String) -> String {
    let filter = EnvFilter::try_new(&max_level)
        .or_else(|_| EnvFilter::try_new("info"))
        .unwrap_or_default();

    // 重复初始化会返回 Err，属预期情况，忽略即可。
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .with_ansi(false) // 日志常被转储到文件或 logcat，ANSI 转义是噪声
        .try_init();

    runtime();

    tracing::info!(level = %max_level, "bili core initialised");
    core_version()
}

/// Rust Core 版本，供 Dart 侧核对桥接两端是否同源。
#[must_use]
pub fn core_version() -> String {
    env!("CARGO_PKG_VERSION").to_owned()
}
