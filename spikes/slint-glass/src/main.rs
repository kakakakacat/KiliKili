//! 桌面入口。实验画面与两个对照开关都在 `src/lib.rs` 的 `run()` 里，
//! Android 那条路走的是同一个函数。
//!
//! 跑法（含 `GLASS_BLUR` / `GLASS_COUNT` 与量帧率的注意事项）见 README。
//! 选渲染器用 `SLINT_BACKEND=winit-femtovg` / `winit-skia`——后者要
//! `--features skia`，**没开 feature 就设环境变量的话 Slint 会静默退回默认
//! 渲染器**，看起来像「Skia 也不支持」，是个会把结论弄反的坑。
//!
//! 评估文档 §2.1 的判据已经做成真代码了，在 `src/wgpu_probe.rs`，
//! 用 `cargo run --features wgpu-probe` 跑。结论写在那个文件的头部。

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 默认那条路不碰 wgpu：先量「原生 backdrop-blur 差多少」，再决定要不要
    // 自己画。理由见 Cargo.toml 里 wgpu-probe feature 那段。
    #[cfg(feature = "wgpu-probe")]
    return slint_glass_spike::wgpu_probe::run_probe();

    #[cfg(not(feature = "wgpu-probe"))]
    Ok(slint_glass_spike::run()?)
}
