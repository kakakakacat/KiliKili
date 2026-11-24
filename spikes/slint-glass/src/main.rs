//! Liquid Glass 实验台（issue #130）的入口。
//!
//! **这份没有编译过。** 见 README 顶部那段。
//!
//! 它现在只做一件事：把 `ui/glass_spike.slint` 开起来，让人把 Slint 原生
//! `backdrop-blur` 的观感和 Flutter 版首页导航栏并排比一次（评估文档
//! §6 第 1 步）。**刻意没有接 wgpu**，理由见 Cargo.toml 里 `wgpu-probe`
//! feature 的说明。
//!
//! 和 `slint-ui` / `slint-mpv` 不一样，这里**没有接 `spike-harness`**：
//! 这一步要的是「人看两张图，差多少」，不是帧耗时分位数。等 §6 第 3 步真的
//! 要量 wgpu 管线的开销时再接——那时候记录格式必须和另外两个 spike 一致，
//! 否则跨实验没法逐行对照。

// material 这一轮还没有被接进来（第三步才用），但它是这个 spike 里唯一
// 可以在编译之前当参考的东西，所以先编进去——让它随每次 `cargo build` 被
// 语法和类型检查一遍，比留在磁盘上烂掉强。
#![allow(dead_code)]

mod material;

slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    // 渲染器用环境变量选，不在代码里写死：这个 spike 的第一个待答问题就是
    // 「`backdrop-blur` 到底是 Skia 独有还是 FemtoVG 也有」（评估文档
    // §0 记的那处文档与 CHANGELOG 打架），而回答它的办法就是同一个二进制
    // 换着后端各跑一次。
    //
    //   SLINT_BACKEND=winit-skia    ./glass-spike
    //   SLINT_BACKEND=winit-femtovg ./glass-spike
    //
    // 两次截图一模一样（都糊）或者一糊一不糊，就是答案。注意 Cargo.toml 里
    // 必须同时开着两个 renderer feature，否则选不到的那个会静默退回默认，
    // 看起来像是「支持」。
    let app = GlassSpike::new()?;
    app.run()
}

// ---------------------------------------------------------------------------
// 下面这段不是代码，是给第二步留的路标。写成注释而不是 `todo!()`，因为
// 一个编译得过但一跑就 panic 的壳，比一个明摆着没写的坑更容易被误当成
// 「已经接好了」。
//
// 第二步（评估文档 §2.1 的判据，十几行就能试出来）：
//
//   app.window().set_rendering_notifier(|state, api| {
//       if let (RenderingState::AfterRendering, GraphicsAPI::WGPU30 { device, queue, .. })
//           = (state, api)
//       {
//           // 这里就是关口：拿得到 device 和 queue，但**拿不到这一帧的
//           // 渲染目标**。没有 TextureView 就发不出 render pass，玻璃画
//           // 不到屏幕上。
//           //
//           // 证据（slint v1.19.1 源码）：
//           //   internal/renderers/femtovg/lib.rs — AfterRendering 在
//           //     present_surface() 之前触发，场景命令已提交，时机是对的；
//           //   GraphicsAPI::WGPU30 只有 { instance, device, queue } 三个
//           //     字段，没有 surface texture 也没有它的 view。
//           //
//           // 所以这一步要证的不是「怎么画」，而是「到底拿不拿得到目标」。
//           // 拿不到 —— 候选 A 出局，直接去评估候选 C（自定义 platform +
//           // FemtoVGWGPURenderer::render_to_texture_view），并把「Android
//           // 上没有 C」写进结论。
//       }
//   })?;
//
// 第三步才轮到 shaders/glass.wgsl 和 src/material.rs。在第一步的差距量出来
// 之前，它们两个都不该被接进来。
// ---------------------------------------------------------------------------
