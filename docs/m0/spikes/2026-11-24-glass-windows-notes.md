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

**2026-11-25 补：量到了，但用的不是帧率，也不是在这台机器上。**
Android 那一轮读 Adreno 的 GPU 占用计数（`gpubusy`，免 root 可读、读一次
清零），绕开了 vsync 这道墙——**一条导航栏的 backdrop-blur 约 0.64 ms/帧**。
见 [第二轮记录 §3.3](2026-11-25-glass-android-notes.md)。
那边还记了一条反例：在 vsync 锁死的设备上，**用占用率算成本会被 DVFS
骗**，60Hz 下量出来的那组数字外推到 120Hz 当场被实测证伪（§3.4）。
#226 的基准要用这个方法的话，先读那一段。

## 4. 两个把实验本身弄脏的毛病，都要写下来

### 4.1 `GLASS_COUNT=10` 实际只渲染了 6 条

截图核对时发现的：窗口高度只装得下 6 条胶囊（间距 92px），多出来的 4 条
`y` 为负、被裁在窗口外。**所以 #130 质量表要的「10 组件」档位这一轮根本
没测到**，上表第三行的标签是错的，真实场景是 6 条。

这正是 #10 那条教训的形状——**自动产出的数字自洽，不等于实验是对的**。
那一轮「虚拟化极好 + 滚动极卡」出自同一个 bug（列表根本没滚动），只有截图
戳破了它；这一轮同样是截图戳破的。

**2026-11-25 已修。** 修法不是「把胶囊缩小或开大窗口」那种一次性调参，
而是按 count 反推间距与胶囊高度，并**把实际参数印在画面上**——根因是档位
只活在环境变量里、截图上看不出来，所以「记录写的」和「画面跑的」没有任何
地方对得上。详见
[第二轮记录 §4](2026-11-25-glass-android-notes.md)。

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
- ~~**没开 wgpu**（`--features wgpu-probe` 没跑）。~~
  **2026-11-25 跑了，见下面 §6。**
- ~~**没测 Android。**~~ **2026-11-25 跑了**，另成一份：
  [第二轮实测：Android / Skia](2026-11-25-glass-android-notes.md)。
  那一轮把帧代价真的量出来了（本轮量不出来的那一项）。
- 截图不进仓库（见 [README](README.md)「截图去哪了」），用 §1 的命令随时
  重跑得出来。

## 6. wgpu 关口 2 的判据：候选 A 出局，自绘纹理这条通（2026-11-25）

`cargo run --features wgpu-probe`，代码在 `spikes/slint-glass/src/wgpu_probe.rs`。
两个问题分开答：

### 6.1 能不能在 Slint 这一帧上直接加画？——**不能**

探针把每个渲染状态实际拿到的东西原样打出来：

```
PROBE state=RenderingSetup   api=WGPU30{instance,device,queue}（没有渲染目标）
PROBE state=BeforeRendering  api=WGPU30{instance,device,queue}（没有渲染目标）
PROBE state=AfterRendering   api=WGPU30{instance,device,queue}（没有渲染目标）
```

和源码一致（i-slint-core 1.19.1 `api.rs:306`：`GraphicsAPI::WGPU30` 只有
`instance / device / queue` 三个字段）。wgpu 不像 GL 有隐式的当前 FBO，
**拿不到 `TextureView` 就发不出 render pass**。

**评估文档 §2 判候选 A「largely out」的那个推断，现在有运行时证据了。**

### 6.2 自己出一张纹理交给 Slint 显示？——**通**

```
PROBE 结论2 自绘纹理导入 Slint：成功
```

`Rgba8Unorm` + `TEXTURE_BINDING | RENDER_ATTACHMENT`，走
`slint::Image::try_from(wgpu::Texture)`，一次过，没碰到
`TextureImportError` 的任何一种。

顺带：**`shaders/glass.wgsl` 第一次过 naga，零报错**。那份 28 行 GLSL 的
逐行移植在此之前一个编译器都没看过。

### 6.3 这里有个坑：feature 得带上渲染器

只开 `unstable-wgpu-30` 启动直接失败：

```
Winit backend failed to find a suitable renderer: unstable-wgpu-30 was
enabled but no renderer was selected.
```

要配 `renderer-skia*` / `renderer-femtovg-wgpu` / `renderer-vello` 之一。
这里选了 `renderer-femtovg-wgpu`：**躲开 Skia 那笔账**，而且和默认那条路
同属 femtovg，画面差异不会混进渲染器差异。

### 6.4 **没有**解决的那一关

探针里的背板纹理是**合成的**（自己画的条带），不是 Slint 这一帧的真实背景。
**「怎么拿到真实背板」仍然是关口 2 的本体，这一轮没动它。** 探针证明的是
管线通、WGSL 跑得起来、纹理导得进去——三件必要条件，不是充分条件。

截图没取到：跑到这一步时机器已经锁屏，`GetForegroundWindow()` 返回 0，
GDI 与 PIL 的 ImageGrab 都抓不到活动桌面。
