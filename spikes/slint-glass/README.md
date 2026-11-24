# slint-glass — Liquid Glass 的实验台骨架（issue #130）

> **状态：Windows / FemtoVG 跑通了一轮，其余都没碰。**
>
> 2026-11-24 首次构建 6 min 01 s、零 error 零 warning，一次编过；增量
> 12 秒。实测记录见
> [2026-11-24-glass-windows-notes.md](../../docs/m0/spikes/2026-11-24-glass-windows-notes.md)。
>
> 这**不等于它是交付**。跑到的只有一件事：**`backdrop-blur` 在 FemtoVG 上
> 确实生效**（官方 Rectangle 参考页那句「only supported by the Skia
> renderer」是过时的）。没跑的有：和 Flutter 版并排对照、Skia、wgpu、
> Android——也就是说它证明了「糊得出来」，**没有证明「糊得像」**。
>
> **Android 那一套（`src/lib.rs` 的 `android_main`、Cargo.toml 的
> `[package.metadata.android]`、`android-drive.sh`）已经写好，但一次都没有
> 构建、没有装过机。** 同一条规矩：跑通之前别拿它的结构当结论。
>
> 另外第一轮就撞出两个把实验弄脏的毛病（`GLASS_COUNT=10` 实际只渲染 6 条、
> 截图脚本的 DPI 偏移），都记在 notes §4。**拿这里的数字下结论之前先读
> 那一节。**

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
cargo build                    # 首次约 6 min，之后增量约 12 s
./target/debug/slint-glass-spike
```

两个环境变量用来做对照，都在 `src/main.rs` 里读：

```sh
GLASS_BLUR=0  ./target/debug/slint-glass-spike   # 关掉 backdrop-blur
GLASS_COUNT=5 ./target/debug/slint-glass-spike   # 同时摆几条玻璃
```

**`GLASS_COUNT` 有个坑**：窗口高度只装得下 6 条（间距 92px），填 10 的话
多出来的 4 条 `y` 为负、被裁在窗口外，而帧率读数照样产出——第一轮就是这么
把「10 组件」那一档记错的。**跑之前先截图数一遍实际可见条数。**

量帧率用 Slint 自带的：

```sh
SLINT_DEBUG_PERFORMANCE=refresh_full_speed,console ./target/debug/slint-glass-spike
```

输出走 **stderr** 不是 stdout。注意它**绕不过 vsync**：这台机器上六组
对照全是 144 fps（= 刷新率），量不出 `backdrop-blur` 的代价，见 notes §3。

**FemtoVG 这轮已经糊了，所以 Skia 不用开。** 官方的 Rectangle 参考页说
`backdrop-blur`「Currently only supported by the Skia renderer」，CHANGELOG
却写「Skia and FemtoVG only」——2026-11-24 实测**CHANGELOG 对，参考页滞后**
（notes §2）。下面这段留着，是给想交叉验证、或者将来 Android 那轮要用
Skia 的人看的：

想交叉验证时再开：

```sh
cargo build --features skia
SLINT_BACKEND=winit-skia ./target/debug/slint-glass-spike
```

**Skia 这笔账在这台机器上是未知的**：`slint-ui` / `slint-mpv` /
`slint-ios` 的 lockfile 里都有 `skia-bindings`，但
`target/release/build/skia-bindings/*/` 下只有 build script 自己，没有 out
产物、没有下载下来的 skia-binaries、也没有静态库——**Skia 从来没有在这台
机器上真正构建过**。下得到预编译二进制是几分钟，下不到就得在 windows-gnu
上从源码编 Skia（python3 + ninja + clang，几十分钟起步）。

另外注意：**没开 `skia` feature 就设 `SLINT_BACKEND=winit-skia`，Slint 会
静默退回默认渲染器**，看起来像「Skia 也不支持」——这个坑会把结论弄反。

用 debug 而不是 `--release`：Cargo.toml 里给依赖单独设了 `opt-level = 2`，
所以 Slint 与 femtovg 仍然是优化过的（不然滚动卡，分不清是 backdrop-blur
贵还是 debug 慢），而自己的代码改一行重编是秒级的——实测 12 秒。等到第三步
真要报性能数字时再换 `--release`。

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

## Android

**先读这条，它决定了结论怎么记**：Android 上**没有 FemtoVG**——
`slint::platform::femtovg_renderer` 整个模块标着 `non-Android only`，
`backend-android-activity-06` 自带的是 **Skia**。所以：

- 桌面那轮验的是「FemtoVG 的 `backdrop-blur`」，Android 这轮验的是
  「Skia 的 `backdrop-blur`」，**两边的结论不能互相代替**；
- 桌面上躲掉的那笔 Skia 账，这里躲不掉。但它可控：slint-ui 的
  `target/aarch64-linux-android/release/build/skia-bindings/` 下有
  `skia-binaries-…-aarch64-linux-android-….tar.gz` 和 `libskia.a`，
  **是下载的预编译二进制，不是源码构建**。本 crate 的 target 不共享那份
  缓存，会重新下一次。

前置：`aarch64-linux-android` target（1.100.0 下已装）、`cargo-apk`、
`JAVA_HOME`（本机在 `D:/as/jbr`，slint 的 android 后端 build.rs 要 javac，
少了它直接 panic）。

```sh
export JAVA_HOME="D:/as/jbr"
cargo apk build --lib        # 别在别人用着机器的时候跑：要下 skia 再交叉编译几百个 crate
./android-drive.sh           # 默认档：blur 开、一条玻璃
./android-drive.sh --blur 0  # 对照档
```

`android-drive.sh` 管安装、配置、启动、截图、收 logcat，证据落在
`target/android-evidence/`（不进仓库）。它靠 logcat 的 `GLASS STEP` 行对
时间轴，不按固定秒数 sleep——真机冷启动能差好几秒，对不上的后果不是报错，
是「截到一张黑图而记录照样产出」。

**`adb shell am start` 传不了环境变量**，所以桌面那两个开关在真机上改从
`$FILES/glass.conf` 读（`BLUR=` / `COUNT=` 两行），由 `android_main` 翻译成
同样的环境变量——`run()` 那边一个字都不用改，两边跑的是同一组参数。

## 这一轮刻意没做的

- **没接 `spike-harness`。** 第一步要的是「人看两张图」，不是帧耗时分位数。
  等第三步真要量 wgpu 管线开销时再接，那时候记录格式必须和另外两个 spike
  一致，否则跨实验没法逐行对照。
- **Android 没有构建过、没有装过机。** 入口和驱动脚本都写好了（见上一节），
  但 `cargo apk build` 一次都没跑——那一步要占机器十几二十分钟。
- **没有多尺度模糊 / Fresnel / 动态光照。** P0 表里这些 Flutter 版也没有，
  先量差距再谈。
