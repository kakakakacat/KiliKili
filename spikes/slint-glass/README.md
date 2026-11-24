# slint-glass — Liquid Glass 的实验台骨架（issue #130）

> **这份骨架一行都没有编译过。**
>
> 不是疏忽，是刻意：写它的那一轮机器被占着。而
> [docs/m0/spikes/README.md](../../docs/m0/spikes/README.md) 自己写过
> 「**一份没跑过的 harness 不是交付，是一堆待调试的代码**」——所以在
> `cargo build` 跑通并把这一段删掉之前，**不要拿它的任何结构当结论**，
> 更不要照着它写第二个组件。
>
> 它存在的价值只有一个：把
> [2026-11-24-glass-wgpu-eval.md](../../docs/m0/spikes/2026-11-24-glass-wgpu-eval.md)
> 的纸面结论落到具体的接线点上，让下一个人开机之后从「编译报错」开始，
> 而不是从「Slint 的玻璃能力到底有哪些」开始。

## 它要回答什么

**只回答一个问题**，就是评估文档 §6 的第 1 步：

> Slint 1.19 原生的 `Rectangle::backdrop-blur`，照着 Flutter 版
> `GlassNavigationSurface` 的参数摆出来，和首页那条导航栏差多少？

这个问题值得排在最前面，是因为 Slint 1.19.0（2026-11-16）新增的
`backdrop-blur` 比 #130 立单（2026-11-13 的参考方案）**晚了三天**——整单是
在「Slint 没有背板能力」的前提下设计的，而那个前提已经不成立。详见评估
文档 §0。

对照 #130 评论 §2 已确认的事实：Flutter 版实际只做了「胶囊 SDF 边缘折射 +
单点高光」。所以如果 `backdrop-blur` 的观感差距只在这两件事上，那 wgpu 那
条路要补的东西，比 #130 原本设想的少一个数量级。

## 怎么跑（开机之后的第一步）

```sh
cd spikes/slint-glass
cargo build --release          # 预期：会报错。先让它编过，再谈别的

SLINT_BACKEND=winit-skia    ./target/release/slint-glass-spike
SLINT_BACKEND=winit-femtovg ./target/release/slint-glass-spike
```

**两个后端都要跑，这不是多此一举**：官方的 Rectangle 参考页说
`backdrop-blur`「Currently only supported by the Skia renderer」，CHANGELOG
却写「Skia and FemtoVG only」，而 FemtoVG 的 wgpu 后端源码里确实有
`BackdropBlur` 的约束说明。三处对不上，只能跑一次定。两次截图一糊一不糊，
就是答案。

跑完和首页导航栏并排截图，结论写进
`docs/m0/spikes/2026-11-24-glass-<平台>-notes.md`。**这一步的结论要人看，
不是程序判的**——#10 的教训（「记录里每个数都自洽，不等于实验是对的」）在
这里尤其适用，因为要比的就是观感。

## 结构

```
ui/glass_spike.slint   会动的背景 + 一条 backdrop-blur 胶囊，参数对着 Flutter 抄
src/main.rs            开窗。第二步（wgpu 判据）的路标写在文件末尾的注释里
src/material.rs        GlassMaterial 参数与五个预设，第三步才接进来
shaders/glass.wgsl     liquid_glass.frag 的逐行 WGSL 移植，第三步才接进来
```

后两个**这一轮不参与运行**，`material.rs` 只是被编进去做语法与类型检查。
它们俩是第三步的料，提前写出来是因为移植账（评估文档 §5）本来就是纸面
工作，不占机器。

## 为什么默认不开 wgpu

`wgpu-probe` feature 默认关着，Cargo.toml 里写了理由，这里只重复最要紧的
一条：**评估文档 §6 的顺序是先量差距、再决定 wgpu 要不要上**，默认开着会
诱导人跳过第一步，顺带先付一次几百个 crate 的全量编译。

## 为什么又是一个独立 workspace、为什么不进 CI

和 `slint-mpv` / `slint-ui` 同样的理由，Cargo.toml 顶部写了。不进 CI 这条
比前两个更硬：#130 要的是帧时间、掉帧、显存，托管 runner 上只有软件光栅器，
那种数字写进 ADR 比没有更坏（评估文档 §4）。

## 这一轮刻意没做的

- **没接 `spike-harness`。** 第一步要的是「人看两张图」，不是帧耗时分位数。
  等第三步真要量 wgpu 管线开销时再接，那时候记录格式必须和另外两个 spike
  一致，否则跨实验没法逐行对照。
- **没有 Android 入口。** Android 那一轮是评估文档 §6 的第 2 步，而且要先
  决定用不用 cargo-apk。到时候照 `slint-ui/Cargo.toml` 补，别提前抄。
- **没有多尺度模糊 / Fresnel / 动态光照。** P0 表里这些 Flutter 版也没有，
  先量差距再谈。
