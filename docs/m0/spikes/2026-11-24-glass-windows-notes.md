# Liquid Glass 第一轮实测：Windows / FemtoVG（issue #130）

**这份是人写的记录，不是实验台 `--record` 产出的**——`spikes/slint-glass`
这一轮还没有接 `spike-harness`（理由见它的 README）。纸面部分见
[2026-11-24-glass-wgpu-eval.md](2026-11-24-glass-wgpu-eval.md)，本文只记
**跑出来的东西**。

## 环境

| 项 | 值 |
|---|---|
| 机器 | Windows 11 Pro 10.0.26300 / AMD RX 7900 GRE |
| 显示 | 144 Hz，缩放 1.5（窗口 1372×896 物理 ≈ 915×597 逻辑） |
| 工具链 | rustc / cargo 1.100.0，`x86_64-pc-windows-gnu`，MinGW-W64 gcc 16.2.0（msvcrt） |
| Slint | 1.19.1，`SLINT_BACKEND=winit-femtovg` |
| 构建 | dev profile，依赖 `opt-level = 2`（见 Cargo.toml） |
| commit | 工作区不干净，理由见 [README](README.md)「记录里的 commit 为什么总是『工作区不干净』」 |

## 1. 构建开销（实测，此前只有估计）

| 动作 | 实测 |
|---|---|
| `cargo generate-lockfile` | **46.3 s**（主要是刷 crates.io index），锁定 561 包 |
| 首次 `cargo build` | **6 min 01 s**，**零 error 零 warning** |
| 改 `src/main.rs` 后增量 | **12.2 s** |
| 改 `ui/glass_spike.slint` + `src/main.rs` 后增量 | **12.5 s** |

两条值得记下来：

- **骨架一次编过，`.slint` 没返工。** 写它的时候一行都没编译过（机器被
  占着），本来预期 `Timer` / `Math.mod` / `for i in 14` 这几处会报语法错，
  实际全中。
- **增量 12 秒**是这个 spike 后续迭代的真实节奏。首次那 6 分钟是一次性的，
  估成本时别拿它当单位。

## 2. `backdrop-blur` 在 FemtoVG 上**生效**——文档是过时的

**这是本轮最值钱的一条，它判掉了待跑清单的第 1 项。**

纸面阶段记过三处说法打架：官方 Rectangle 参考页说
「Currently only supported by the Skia renderer」，CHANGELOG 说
「Skia and FemtoVG only」，FemtoVG 的 wgpu 源码里又有 `BackdropBlur` 的
约束说明。

实测：`SLINT_BACKEND=winit-femtovg` 下，胶囊后面的背景条带**明显糊开**，
胶囊外的同一批条带是锐利硬边；摆多条胶囊时每条各自独立模糊自己底下那块，
没有互相把对方的模糊结果再糊一遍。

**结论：CHANGELOG 是对的，参考页滞后。** 直接后果是
**Skia 那笔未知的大账不用付**——`skia-bindings` 在这台机器上从来没真正
构建过（见 #229），而现在证明了第一步根本不需要它。

同时确认了另外两件事：

- 胶囊形状、圆角、白色描边、渐变、外阴影都按 `GlassNavigationSurface` 的
  参数渲染出来了；
- 中文在 FemtoVG 上正常显示（不是 iOS 那种方块，参照 #206）。

## 3. 帧率这一项**没测出来**，而且必须记成「测不出」而不是「不贵」

跑了 1 / 5 / 10 条玻璃 × 开关 blur 共六组，每组 12 秒，
`SLINT_DEBUG_PERFORMANCE=refresh_full_speed,console`：

| 组件数 | blur | 后 8 次均值 | 最低 |
|---:|:---:|---:|---:|
| 1 | 开 | 144.1 | 144 |
| 1 | 关 | 144.0 | 143 |
| 5 | 开 | 144.1 | 144 |
| 5 | 关 | 144.0 | 142 |
| 10 | 开 | 144.1 | 144 |
| 10 | 关 | 144.0 | 142 |

（`count=10 / blur=0` 那次样本只有 13 个、最低读数 0，是 `Stop-Process`
杀进程时的收尾噪声，不是掉帧。）

**六组全是 144——那是显示器刷新率，不是渲染能力。** `refresh_full_speed`
并没有绕过 vsync，所以这台 GPU 上 `backdrop-blur` 的开销被完全掩盖。

**这不是「backdrop-blur 不贵」的证据，是「这个方法量不出它」。** 官方对它
的警告（「每次重绘都要重读并过滤矩形底下每个像素」）一个字都没有被证伪。
真要量它，#226 的基准得另想办法：绕过 vsync、把负载放到 4K 全屏大面积，
或者直接上压得动的设备（低端 Android 才是那个设备）。

## 4. 两个把实验本身弄脏的毛病，都要写下来

### 4.1 `GLASS_COUNT=10` 实际只渲染了 6 条

截图核对时发现的：窗口高度只装得下 6 条胶囊（间距 92px），多出来的 4 条
`y` 为负、被裁在窗口外。**所以 #130 质量表要的「10 组件」档位这一轮根本
没测到**，上表第三行的标签是错的，真实场景是 6 条。

这正是 #10 那条教训的形状——**自动产出的数字自洽，不等于实验是对的**。
那一轮「虚拟化极好 + 滚动极卡」出自同一个 bug（列表根本没滚动），只有截图
戳破了它；这一轮同样是截图戳破的。修法：下一轮要么把胶囊缩小、要么开大
窗口，并在记录里核对实际可见条数，别只信环境变量。

### 4.2 截图脚本的 DPI 坑

第一次截图抓到的是旁边的桌面，不是窗口。原因：本机缩放 1.5，PowerShell
进程默认不是 DPI-aware，`GetWindowRect` 返回物理像素而
`Graphics.CopyFromScreen` 走虚拟化坐标，两边差 1.5 倍。

修法是在任何窗口操作**之前**调一次 `SetProcessDPIAware()`，然后
`SetWindowPos` 把窗口挪到 (0,0) 并置顶再截。置顶那一步不是洁癖——被遮挡
的窗口在某些驱动上会降帧，会把 §3 的读数弄脏。

这条对 #226 和任何要在这台机器上截 GUI 的 spike 都适用，所以记在这里而
不是只留在脚本里。

## 5. 这一轮没做的

- **没有和 Flutter 版并排对照。** 那是评估文档 §6 第 1 步的完整形态，要跑
  起 Flutter app（编 Rust + Dart，开销和这个 spike 不是一个量级），而且
  首页导航栏要登录态。本轮只证明了「FemtoVG 糊得出来」，**没有证明「糊得
  像」**。
- **没开 Skia**（`--features skia` 没跑）。§2 的结论让它从「必须跑」降级为
  「想交叉验证时再跑」。
- **没开 wgpu**（`--features wgpu-probe` 没跑）。评估文档 §2.1 那个判据
  （`AfterRendering` 里拿不拿得到可写 `TextureView`）还没验。
- **没测 Android。** 那是待跑清单里优先级最高的一项，决定五端能不能齐。
  入口（`src/lib.rs` 的 `android_main`）与驱动脚本（`android-drive.sh`）
  当天稍后写好了，但**一次都没构建、没装过机**。

  摸前置条件时确认了两件会影响那一轮开销的事：

  1. **Android 上没有 FemtoVG**，`backend-android-activity-06` 自带的是
     Skia。所以**桌面这一轮的结论代替不了 Android**——两边是两个实现。
  2. **Skia 在 android target 上是下载的预编译二进制**，不是源码构建：
     slint-ui 的 `target/aarch64-linux-android/release/build/skia-bindings/`
     下有 `skia-binaries-…-aarch64-linux-android-ganesh-gl-jpegd-jpege-pdf-vulkan.tar.gz`
     与 `libskia.a`。桌面上躲掉的那笔账在 Android 躲不掉，但它可控。

  真机（`fa392823`）、`cargo-apk`、`aarch64-linux-android` target 都就位。
- 截图不进仓库（见 [README](README.md)「截图去哪了」），用 §1 的命令随时
  重跑得出来。
