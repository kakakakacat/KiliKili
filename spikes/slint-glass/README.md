# slint-glass — Liquid Glass 的实验台骨架（issue #130）

> **状态：Windows/FemtoVG 与 Android/Skia 各跑通一轮，wgpu 判据也跑了。**
>
> 实测记录两份：
> [Windows / FemtoVG](../../docs/m0/spikes/2026-11-24-glass-windows-notes.md)、
> [Android / Skia](../../docs/m0/spikes/2026-11-25-glass-android-notes.md)。
>
> 跑到的：
>
> - **`backdrop-blur` 在 FemtoVG 与 Skia 上都生效**（官方 Rectangle 参考页
>   那句「only supported by the Skia renderer」是过时的）；Android 侧判据是
>   量化的，边缘能量掉 42.8%。
> - **帧代价量出来了**：一条导航栏 ≈0.64 ms/帧、十条 ≈1.66 ms/帧（骁龙
>   8 Gen 2）。**靠的是换方法不是换设备**——帧率两端都被 vsync 钉死，
>   改读 Adreno 的 `gpubusy` 才绕开。
> - **wgpu 关口 2**：`GraphicsAPI::WGPU30` 没有渲染目标，候选 A 出局；
>   自绘纹理导入 Slint 这条通，`glass.wgsl` 第一次过 naga 零报错。
>
> **没跑的，而且其中一条足够挡住后续**：
>
> - **和 Flutter 版并排对照还是空的。** 两轮都只证明了「糊得出来」，
>   **至今没有证明「糊得像」**。
> - 低端机、iOS / macOS、功耗与温度、滚动残影，全都没碰。
>   已测的 Android 是 2023 年的旗舰，**不代表低端机**，而低端机才是风险所在。
>
> 第一轮撞出的两个毛病（`GLASS_COUNT=10` 实际只渲染 6 条、截图脚本的 DPI
> 偏移）记在第一轮 notes §4；前者 2026-11-25 已修，修法见第二轮 notes §4。

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

几个环境变量用来做对照，都在 `src/lib.rs` 的 `apply_env_overrides()` 里读：

```sh
GLASS_BLUR=0   ./target/debug/slint-glass-spike   # backdrop-blur 的 sigma，0 等于关掉
GLASS_COUNT=5  ./target/debug/slint-glass-spike   # 同时摆几条玻璃
GLASS_TICK=4   ./target/debug/slint-glass-spike   # 背景 Timer 周期（ms），见下
```

**`GLASS_COUNT` 曾经有个坑，已修**：原来固定 92px 间距、80px 高，窗口只
装得下 6 条，填 10 的话多出来的 4 条被裁在窗口外，而帧率读数照样产出——
第一轮就是这么把「10 组件」那一档记错的。现在按 count 反推间距与胶囊高度，
N 条就真的是 N 条，**而且画面左上角印出了实际的 `blur-sigma / count /
tick / slot / h / 窗口尺寸`，截图自带档位**。那条教训的正解是后者：
档位只活在环境变量里、画面上看不出来，才是记录和实际对不上的根因。

**`GLASS_TICK` 是为了拆一个撞车**：默认 16ms = 62.5Hz，和 60Hz 刷新率只差
2.5，于是「帧率 61」分不清是撞到 vsync 还是撞到这个 Timer——两种解释导向
相反的结论。调到 4ms 再读一次才分得开。

量帧率用 Slint 自带的：

```sh
SLINT_DEBUG_PERFORMANCE=refresh_full_speed,console ./target/debug/slint-glass-spike
```

输出走 **stderr** 不是 stdout。注意它**绕不过 vsync**：这台机器上六组
对照全是 144 fps（= 刷新率），量不出 `backdrop-blur` 的代价，见第一轮
notes §3。真正量到代价用的是另一条路（Adreno 的 GPU 占用计数），只在
Android 上可行，见第二轮 notes §3.3——**那一节同时记了这个方法在 vsync
锁死时会被 DVFS 骗，用它之前先读**。

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
ui/glass_spike.slint   会动的背景 + backdrop-blur 胶囊，参数对着 Flutter 抄
src/lib.rs             桌面与 Android 共用的 run()；android_main 也在这里
src/main.rs            桌面入口，按 feature 分派到普通跑法或 wgpu 探针
src/material.rs        GlassMaterial 参数与五个预设
src/wgpu_probe.rs      wgpu 关口 2 的判据（--features wgpu-probe 才编）
shaders/glass.wgsl     liquid_glass.frag 的逐行 WGSL 移植
android-drive.sh       真机的装/配/起/截图/收证据
```

`material.rs` 与 `glass.wgsl` 原本只是被编进去做检查，**2026-11-25 之后
它们真的跑起来了**：`wgpu_probe.rs` 用 `GlassUniforms` 把
`GlassMaterial::FLUTTER_BASELINE` 打成 uniform，跑 `glass.wgsl` 画进一张
自己的纹理，再交给 Slint 显示。那也是这份 WGSL 第一次被 naga 看过。

## wgpu 探针（关口 2 的判据）

```sh
cargo run --features wgpu-probe
```

默认**不开**，Cargo.toml 里写了理由，最要紧的一条是：评估文档 §6 的顺序
是先量差距、再决定 wgpu 要不要上，默认开着会诱导人跳过第一步，顺带先付
一次几百个 crate 的全量编译。

2026-11-25 跑过一次，结论写在 `src/wgpu_probe.rs` 的文件头与第一轮
notes §6。两句话版本：

1. **在 Slint 这一帧上直接加画 —— 不行。** 三个渲染状态拿到的都只有
   `WGPU30{instance,device,queue}`，没有渲染目标，wgpu 又没有 GL 那种隐式
   当前 FBO，发不出 render pass。
2. **自己出一张纹理交给 Slint —— 行。** 一次过。

**但探针里的背板是合成的，不是 Slint 这一帧的真实背景。**「怎么拿到真实
背板」才是关口 2 的本体，这一轮没动它。

一个会卡住人的点：只开 `unstable-wgpu-30` 启动直接失败
（`no renderer was selected`），必须同时带一个 wgpu 能力的渲染器。这里选
`renderer-femtovg-wgpu` 而不是 Skia——躲开那笔账，而且和默认那条路同属
femtovg，画面差异不会混进渲染器差异。

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
少了它直接 panic）、`ANDROID_NDK_ROOT`。

```sh
export JAVA_HOME="D:/as/jbr"
export ANDROID_HOME="$LOCALAPPDATA/Android/Sdk"
export ANDROID_NDK_ROOT="$ANDROID_HOME/ndk/30.0.16248370"
cargo apk build --release --lib   # 实测首次 2 min 48 s，增量 4 s

./android-drive.sh                                      # 默认档：blur 开、一条玻璃
./android-drive.sh --blur 0                             # 对照档
./android-drive.sh --count 10 --perf --fps 120 --tick 4 # 顶到 120Hz 量帧率
```

**首次构建比担心的便宜得多**：skia 在 android target 上是**下载的预编译
包**（`libskia.a` 22.5 MiB），不是源码构建。

`--release` 不是洁癖：Cargo.toml 只给 release 配了 keystore，而且帧率与
GPU 占用的数字要代表 Phase 2 的真实形态。

**`--fps 120` 和 `--tick 4` 要一起用。** 单给 `--fps` 没用：app 自己只出
~60 帧的话，SurfaceFlinger 按内容帧率把面板投回 60——**面板是跟着 app 走
的**。adb 侧三条路（`settings`、`service call SurfaceFlinger 1035`、
`cmd display set-user-preferred-display-mode`）全部压不住，详见第二轮
notes §3.2。

`android-drive.sh` 管安装、配置、启动、截图、收 logcat，证据落在
`target/android-evidence/`（不进仓库）。它靠 logcat 的 `GLASS STEP` 行对
时间轴，不按固定秒数 sleep——真机冷启动能差好几秒，对不上的后果不是报错，
是「截到一张黑图而记录照样产出」。

**`adb shell am start` 传不了环境变量**，所以桌面那几个开关在真机上改从
`$FILES/glass.conf` 读（`BLUR= / COUNT= / TICK= / PERF= / FPS=`），由
`android_main` 翻译成同样的环境变量——`run()` 那边一个字都不用改，两边跑的
是同一组参数。

还有两条只在真机上成立的：

- **`dumpsys gfxinfo` 对 NativeActivity 无效**（`Total frames rendered: 0`），
  它只统计 HWUI 管线。要帧时间用 `dumpsys SurfaceFlinger --latency`，
  要 GPU 成本用 `/sys/class/kgsl/kgsl-3d0/gpubusy`（免 root、读一次清零）。
- **面板刷新率必须和截图同时取**，脚本每轮落在 `active-mode.txt`。
  61 fps 在 60Hz 面板上是「满了」，在 120Hz 面板上是「只跑到一半」。

## 这一轮刻意没做的

- **没接 `spike-harness`。** 第一步要的是「人看两张图」，不是帧耗时分位数。
  等第三步真要量 wgpu 管线开销时再接，那时候记录格式必须和另外两个 spike
  一致，否则跨实验没法逐行对照。
- **没有和 Flutter 版并排对照。** 这是最要紧的那个空缺：两轮都只证明了
  「糊得出来」，**没有证明「糊得像」**。它要跑起 Flutter app，而且首页
  导航栏要登录态。
- **没碰真实背板的捕获。** wgpu 探针里那张是合成的，而「怎么拿到当前帧的
  干净背景」才是关口 2 的本体。
- **没有多尺度模糊 / Fresnel / 动态光照。** P0 表里这些 Flutter 版也没有，
  先量差距再谈。
- **没测低端机、iOS、macOS，没测功耗与温度，没测滚动残影。**
