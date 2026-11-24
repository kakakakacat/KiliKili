//! 桌面入口。实验画面与两个对照开关都在 `src/lib.rs` 的 `run()` 里，
//! Android 那条路走的是同一个函数。
//!
//! 跑法（含 `GLASS_BLUR` / `GLASS_COUNT` 与量帧率的注意事项）见 README。
//! 选渲染器用 `SLINT_BACKEND=winit-femtovg` / `winit-skia`——后者要
//! `--features skia`，**没开 feature 就设环境变量的话 Slint 会静默退回默认
//! 渲染器**，看起来像「Skia 也不支持」，是个会把结论弄反的坑。
//!
//! 第二步（评估文档 §2.1 的判据，十几行就能试出来）写成注释留在下面，没写成
//! `todo!()`：一个编译得过但一跑就 panic 的壳，比一个明摆着没写的坑更容易被
//! 误当成「已经接好了」。
//!
//! ```text
//! app.window().set_rendering_notifier(|state, api| {
//!     if let (RenderingState::AfterRendering, GraphicsAPI::WGPU30 { device, queue, .. })
//!         = (state, api)
//!     {
//!         // 这里就是关口：拿得到 device 和 queue，但**拿不到这一帧的渲染
//!         // 目标**。没有 TextureView 就发不出 render pass。
//!         //
//!         // 证据（slint v1.19.1 源码）：
//!         //   internal/renderers/femtovg/lib.rs — AfterRendering 在
//!         //     present_surface() 之前触发，场景命令已提交，时机是对的；
//!         //   GraphicsAPI::WGPU30 只有 { instance, device, queue } 三个字段，
//!         //     没有 surface texture 也没有它的 view。
//!         //
//!         // 所以这一步要证的不是「怎么画」，而是「到底拿不拿得到目标」。
//!         // 拿不到 —— 候选 A 出局，直接去评估候选 C（自定义 platform +
//!         // FemtoVGWGPURenderer::render_to_texture_view），并把「Android 上
//!         // 没有 C」写进结论。
//!     }
//! })?;
//! ```

fn main() -> Result<(), slint::PlatformError> {
    slint_glass_spike::run()
}
