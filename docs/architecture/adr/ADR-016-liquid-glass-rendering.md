# ADR-016: Liquid Glass 的渲染路径、材质单一来源与质量分档

Status: **Proposed（待仓库所有者确认）**
Date: 2026-11-25
Related: issue #130（Phase 1）、#224（组件库）、#225（五平台适配）、#226（基准）、
RFC-0001 Stage 2、AGENTS.md §2（宿主可替换）
证据：[第一轮 Windows/FemtoVG](../../m0/spikes/2026-11-24-glass-windows-notes.md)、
[第二轮 Android/Skia](../../m0/spikes/2026-11-25-glass-android-notes.md)、
[第三轮 GL 覆盖](../../m0/spikes/2026-11-25-glass-gl-overlay-notes.md)、
[纸面评估](../../m0/spikes/2026-11-24-glass-wgpu-eval.md)

## 背景

#130 立单于 2026-11-13，前提是「Slint 没有背板能力，所以玻璃必须自建
wgpu 管线」。**那个前提在立单三天后就不成立了**：Slint 1.19.0（2026-11-16）
给 `Rectangle` 加了 `backdrop-blur`。三轮 spike 之后，可选项与代价都有了
实测，这份 ADR 把取舍定下来。

一句话现状：**原生 `backdrop-blur` 解决了「模糊」，但解决不了折射与高光；
要折射与高光就必须拿到本帧真实背板，而拿背板只有一条路跑通了。**

## 决策一：玻璃走 GL 覆盖（`AfterRendering` + `NativeOpenGL`），不自建 wgpu 管线

在 Slint 的 `AfterRendering` 回调里，用 `glCopyTexSubImage2D` 把本帧已画完、
尚未 present 的帧缓冲拷进自己的纹理，再用自己的 GLSL 在其上绘制玻璃。

**为什么不是 wgpu**：`GraphicsAPI::WGPU30` 只有 `{instance, device, queue}`，
没有本帧的渲染目标，发不出 render pass（运行时证据见 `src/wgpu_probe.rs`）。
GL 没有这个问题——它有隐式的当前帧缓冲。

**为什么不是「上一帧回灌」**：滚动必拖影。
**为什么不是 fork Slint 或自定义 platform**：前者成本不成比例，后者的
`femtovg_renderer` 标着 `non-Android only`，Android 直接出局。

**证据**：Windows/FemtoVG 与 Android/Skia 两个后端各跑通一次，
`draw_fbo=0 read_fbo=0`，拷贝与绘制全程 `glGetError=0`。上游在
`femtovg/opengl.rs:212` 的注释里明确预期 notifier 会重新绑定帧缓冲，
说明这是支持的用法而非钻空子。

**已知的边界（见决策六）**：Apple 平台上 Slint 1.19 的 Skia 走的是
Metal-via-wgpu，这条路**大概率不通**。

## 决策二：背板捕获范围 = 玻璃矩形外扩 3σ，全程 GPU 内，不回读 CPU

拷贝区域按 `3 × blur_radius × scale` 外扩，与 Slint 原生 `backdrop-blur`
「最远采到该值三倍」同口径；shader 侧用 `u_backdrop_origin` /
`u_backdrop_size` 换算 uv。

每帧的复制**只有一次**（`glCopyTexSubImage2D`），没有任何 CPU 回读或
重新上传。显存实测与按尺寸算出来的一致（1.87 MB 实测 / 1.91 MB 理论），
可以反过来确认「只有这几张纹理」。这回答 #130 关口 4。

**随之成立的一条约束**：两块玻璃相距小于 3σ 时会互相采到对方。这不是
实现缺陷，是这条路径的物理性质。组件库（#224）排布玻璃时必须知道它，
#130 P1 的「靠近时的玻璃融合」正是它的正面用法。

## 决策三：材质参数表是单一来源，放在 Rust，宿主只负责接入

`GlassMaterial`（现在在 `spikes/slint-glass/src/material.rs`）是玻璃观感的
唯一定义处：模糊半径、折射强度与带宽、高光强度与带宽、Fresnel 指数与强度、
表面渐变三档 alpha、描边 alpha 与宽度、圆角。shader 与宿主控件都只是它的
消费者，**不得各自硬编码**。

这条直接来自 AGENTS.md §2：换掉宿主框架时这张表不该重写。它也堵掉
#130 评论 §4 记的那个分歧风险——参数同时存在于 `shaders/liquid_glass.frag`
和 Rust 两处必然漂移。

**两档必须同时保留**：

- `FLUTTER_BASELINE`：与 Flutter 的 shader 路径逐项同式，**`blur_radius`
  按 0 使用**（Flutter 那边 shader 与 blur 是二选一，10 是回退档的值）。
  它是「像不像」的唯一参照物。
- `P0_TARGET`：在其上补模糊与 Fresnel，即本单 P0 的目标形态。

只留后者的话，画面上的差别里分不清哪些是新能力、哪些是移植没对上。

**搬家时机**：这张表随 #224 迁进 `rust/crates/` 下的正式 crate，届时
`material.rs` 顶部那条「例外要写下来」的注释一并删除。在此之前它待在
spike 里，是因为不该把未验证的架构决策先写进产品代码。

## 决策四：模糊优先用原生 `backdrop-blur`，自绘模糊只在必须与折射叠加时开

实测代价（Adreno 740，120Hz，每帧预算 8.33 ms）：

| 项 | 每帧 GPU |
|---|---:|
| 原生 `backdrop-blur` | **+0.65 ms** |
| 自绘折射 + Fresnel + 高光 + 材质层 | **+0.27 ms** |
| 自绘模糊（1/4 降采样 + 两趟可分离高斯） | **+1.71 ms** |

自绘模糊比原生贵 **2.6 倍**，贵在全分辨率拷一张带边距的背板再降采样，
而原生那条在渲染器内部做，省掉这一拷。

因此默认形态是**原生负责模糊、自绘负责折射与高光**，合计约 0.92 ms，
而不是全自绘的 2.36 ms。

**但这个组合本轮没有验过**，它有一个必须先看画面才能判的问题：自绘那层
采到的背板里已经含有原生那层画的渐变与描边，**折射会把描边一起抹开**。
这是 #130 Phase 2 的第一项工作，也是本 ADR 唯一一条「决策已定、实现待验」
的条目。验不过就退回全自绘，代价表上面已经有了。

## 决策五：质量分档按**整条表面**升降，不逐效果开关

| 档 | 构成 | 每帧 GPU（实测/推算） |
|---|---|---:|
| High | 原生模糊 + 自绘折射/Fresnel/高光 | ~0.92 ms（推算，依赖决策四待验项） |
| Balanced | 仅原生 `backdrop-blur` | 0.65 ms（实测） |
| Low | 静态/缓存模糊，关闭自绘层 | 待 #225 定 |

能力判定在材质层**做一次**，整条表面一起降级。这条沿用 Flutter 侧的现成
先例（`ImageFilter.isShaderFilterSupported` 为假时整体退到
`ImageFilter.blur(10)`，见 #122 的验收记录），它的好处是降级后的观感是
设计过的一档，而不是若干开关的任意组合。

降档规则与低端机判定由 Rust 统一管理，宿主不参与——同决策三的理由。

## 决策六：本阶段按**三端**承诺，Apple 平台单独立项

Slint 1.19.1 的 Skia 渲染器**没有独立的 Metal surface**：
`lib.rs:248` 的 `metal_wgpu_surface_factory` 把 Metal 走成 wgpu，
而 wgpu 变体没有渲染目标（决策一）。

| 平台 | notifier 变体 | GL 覆盖 | 依据 |
|---|---|---|---|
| Windows | `NativeOpenGL` | ✅ | 实测 |
| Android | `NativeOpenGL` | ✅ | 实测 |
| Linux | `NativeOpenGL` | 大概率 | 同桌面路径，**未验** |
| macOS | `WGPU30` | ❌ 推断不通 | 源码，**未验** |
| iOS | `WGPU30` | ❌ 推断不通 | 源码，**未验** |

**不承诺五端一致**。Apple 两端要么改用 GL 后端（macOS 上 GL 已废弃但仍
可用；iOS 要看 GLES），要么只给原生 `backdrop-blur`（Balanced 档），
要么另想办法。这件事排在 #225 最前面，**它决定玻璃是一套还是两套**。

## 不做什么

- **不 fork Slint。** 三轮下来没有一处需要它。
- **不往 Slint 渲染器里注入 WGSL。** 做不到，已有运行时证据；玻璃只能是
  独立一层。
- **不在 CI 里验收玻璃。** 托管 runner 只有软件光栅器，那种帧时间与显存
  写进文档比没有更坏（见 `docs/m0/VALIDATION-CAPABILITY.md`）。本 ADR 的
  每个数字都来自真机。
- **不据此动 Android 最低版本。** spike 的 `min_sdk = 31` 是探针自己的
  取值（要 `ANativeWindow_setFrameRate`），不是对 app 的主张。

## 未决与风险

1. **决策四的组合未验**（折射会不会抹开原生那层的描边）。挡着 Phase 2。
2. **Apple 两端未验**，见决策六。挡着五端一致性，转 #225。
3. **低端机未验。** 现有数字出自 2023 年旗舰，#130 的风险恰恰在低端机。
   缺设备，转 #225。
4. **功耗与温度未测**，采样窗口太短，热降频来不及发生。转 #226。
5. **桌面侧没有帧时间**：帧率被 vsync 钉死，Windows 上又没有 Adreno
   `gpubusy` 那种免 root 的占用计数。转 #226。
6. **滚动残影未看**：现有背景是匀速横移的彩带，不是真实列表滚动。
7. **`gpuclk` 读不到**（permission denied），DVFS 对占用率的影响无法彻底
   排除。120Hz 满负载下残留较小，但这条不确定性至今没关掉——
   复用这套测法之前先读第二轮记录 §3.4。
