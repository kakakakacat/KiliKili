# Liquid Glass 第二轮实测：Android / Skia（issue #130）

**这份是人写的记录，不是实验台 `--record` 产出的**，和
[第一轮（Windows / FemtoVG）](2026-11-24-glass-windows-notes.md) 同一个
spike。纸面部分见 [评估文档](2026-11-24-glass-wgpu-eval.md)。

**本轮最值钱的一条：backdrop-blur 的帧代价第一次被量出来了**，而且不是靠
帧率——帧率这条路在两端都走死了，详见 §3。

## 环境

| 项 | 值 |
|---|---|
| 设备 | Redmi K70（`2304FPN6DC`），Android 14 |
| SoC / GPU | SM8550（骁龙 8 Gen 2，`kalama`）/ Adreno 740，OpenGL ES 3.2 |
| 屏 | 1116×2480 物理，density 440（2.75×）→ **逻辑 406×902**；支持 60/90/120Hz |
| 渲染 | Slint 1.19.1，`backend-android-activity-06` → **Skia（ganesh-gl）** |
| 构建 | release profile，NDK r30 `30.0.16248370`，JDK `D:/as/jbr` |
| min_sdk | 31（为了 `ANativeWindow_setFrameRate`，见 §3.2） |

**Android 上没有 FemtoVG**：`slint::platform::femtovg_renderer` 整个模块标着
`non-Android only`。所以第一轮验的是 FemtoVG 的实现，这一轮验的是 Skia 的，
**两边的结论不互相代替**——好在两边都支持（§2）。

## 1. 构建开销（实测，此前只有估计）

| 动作 | 实测 |
|---|---|
| 首次 `cargo apk build --release --lib` | **2 min 48 s**，零 error 零 warning |
| 改 `.slint` + `src/lib.rs` 后增量 | **4.1 s** |
| 改 `Cargo.toml` 加依赖后 | 1 min 44 s |
| 产物 | APK 7.3 MB |

**纸面上担心的那笔 Skia 账没有发生。** `skia-bindings` 在 android target 上
拿到的是**下载的预编译包**
（`skia-binaries-…-aarch64-linux-android-ganesh-gl-jpegd-jpege-pdf-vulkan.tar.gz`，
解出来 `libskia.a` 22.5 MiB），不是在本机从源码构建。评估文档里「下不到就要
几十分钟起步」那条分支没走到。

## 2. `backdrop-blur` 在 Skia / Android 上**生效**，而且这次是量化的

第一轮只能说「看着糊了」。这一轮换了个能写进报告的判据：

> **横向梯度能量**：背景是竖直彩条，锐利时相邻像素在条边界上跳变大，
> 模糊后跳变被摊平。取同一段 x 范围，比胶囊内与胶囊外。

这个度量**与条带相位无关**（横移不改变窗口内的边数与幅度），所以两次
独立运行的截图可以直接比——实测也印证了：两张图的背景读数都是 1.959，
小数点后三位一致。

| 区域 | blur 关 | blur 开 |
|---|---:|---:|
| 胶囊外（背景参照） | 1.959 | 1.959 |
| 胶囊内（同一批行） | 2.082 | **1.190** |

**边缘能量掉 42.8%。** blur 关的时候胶囊内（2.082）≈ 背景（1.959），
说明玻璃本身只是半透明叠了一层、条带照穿——对照组是干净的。

sigma 也确实在起作用，不是被钳住的一个固定值（count=10，第一条胶囊内）：

| sigma | 10 | 30 | 60 | 120 | 240 |
|---|---:|---:|---:|---:|---:|
| 边缘能量 | 1.189 | 0.861 | 0.617 | 0.532 | 0.522 |

120 之后饱和，是因为背板已经被摊平到接近均值，再加 sigma 也没有更多信息
可丢——不是实现的上限。

## 3. 帧代价：**量出来了**，但不是用帧率量的

### 3.1 帧率这条路为什么走死

60Hz 下跑 1/5/10 条 × 开关 blur 六组，`refresh_full_speed,overlay`，
**六组读数全是 61**。真实呈现时间戳（`dumpsys SurfaceFlinger --latency`）：

- SurfaceFlinger 报的刷新周期 **16.67 ms（60.0 Hz）**；
- 126 帧，间隔中位 **16.56 ms**，**p90 = max = 16.57 ms**，一帧没掉。

这里有个会把人绕进去的巧合：`Timer { interval: 16ms }` 等于 **62.5 Hz**，
和 60 Hz 只差 2.5。于是「61」有两种解释——撞到 vsync，还是撞到我自己的
Timer——**两种解释导向相反的结论**。分开它们的办法是把 tick 调小（现在
`GLASS_TICK` 可调，画面上也印出来了），调到 4ms 再读一次。

### 3.2 面板是**跟着 app 走**的，不是反过来

这台手机支持 120Hz，但 app 一起来，系统两三秒后就把面板投回 60。
adb 侧三条路**全部压不住**：

| 做法 | 结果 |
|---|---|
| `settings put system peak_refresh_rate 120` + `min_refresh_rate 120` | 投票进去了（`DisplayModeDirector` 历史里能看到 `render: (120.0 120.0)`），面板仍然掉回 60 |
| `service call SurfaceFlinger 1035 i32 1` | `Operation not permitted`（要 system 权限） |
| `cmd display set-user-preferred-display-mode 1116 2480 120` | 设进去了（`get-` 能读回），面板仍然掉回 60 |
| `settings put system is_smart_fps 0`（MIUI 智能刷新率） | 更糟，直接钉死 60 |

原因是**app 自己只出 ~60 帧**（Timer 16ms），SurfaceFlinger 按内容帧率把
面板投到 60。把两件事一起做之后面板才稳住 120：

1. `GLASS_TICK=4`，让 app 真的能产出 120 帧以上；
2. app 自己用 `ANativeWindow_setFrameRate(120, Default, Always)` 报期望值
   （`src/lib.rs`，每 2 秒重报一次——surface 刚开始还不存在，而且系统会
   在几秒后重新仲裁）。

`ChangeFrameRateStrategy::Always` 不能换成 `OnlyIfSeamless`：60→120 在这块屏
上不算无缝，用 OnlyIfSeamless 会被静默忽略，**而且返回 `Ok(())`**，看起来
就像这个 API 不管用。

做完这两件事：10 条玻璃、sigma=10，**FPS 121，面板 120.00 Hz**。

### 3.3 GPU 占用——绕开 vsync 的那条路

`/sys/class/kgsl/kgsl-3d0/gpubusy` 在这台设备上**免 root 可读**，给的是
「忙/总」计数且**读一次清零**，所以采样窗口里的占用率算得出来，和帧率撞不撞
vsync 无关。12 秒窗口，面板钉在 120Hz，跑两轮：

| 档位 | GPU 忙 | 折算每帧 GPU 时间（8.33 ms 预算） |
|---|---:|---:|
| 1 条，blur 关 | 23.9% | 1.99 ms |
| 1 条，sigma=10 | 31.6% | 2.63 ms（**blur +0.64 ms**） |
| 10 条，blur 关 | 43.3% | 3.61 ms |
| 10 条，sigma=10 | 63.3% | 5.27 ms（**blur +1.66 ms**） |
| 10 条，sigma=60 | 71.5% | 5.96 ms |
| 10 条，sigma=240 | 80.8% | 6.73 ms |

两轮之间的波动都在 1 个百分点以内。

**换算成结论：首页那一条导航栏的原生 backdrop-blur，在这台设备上约
0.64 ms/帧。** 这是 #130 到目前为止唯一一个可以写进 ADR 的代价数字。

### 3.4 60Hz 下量的那一组是**错的参照**，必须说清楚

先在 60Hz 下量过一轮，读数是这样的（三轮，波动同样 <1 个百分点）：

| 档位 | 1 条关 | 1 条 s10 | 10 条关 | 10 条 s10 | 10 条 s60 | 10 条 s240 |
|---|---:|---:|---:|---:|---:|---:|
| GPU 忙 | 12.3% | 16.2% | 21.8% | 60.0% | **52.6%** | 71.3% |

两处露馅：

1. **非单调**——`sigma=60` 比 `sigma=10` 还便宜。到 120Hz 那组就单调了。
2. 按这组的 60% 外推每帧约 10 ms，那么 120Hz（预算 8.3 ms）必然掉帧；
   **实测 121 fps，外推当场被证伪。**

原因是 **DVFS**：60Hz 时 GPU 每帧之间大段空闲、降频运行，占用率里混进了
「时钟低」这个因素，不能当工作量读。`gpuclk` 在这台设备上读不了
（permission denied），所以时钟这一项至今是个没关掉的不确定性——
**120Hz 那组同样有，只是负载更满、残留更小。**

记下来不是为了自嘲：#226 的基准如果在 vsync 锁死的设备上用占用率算成本，
会原样踩这个坑。

### 3.5 拐点在哪

| 档位 | FPS（面板 120Hz） |
|---|---:|
| 10 条，sigma=10 | 121 |
| 10 条，sigma=240 | 121 |
| 20 条，sigma=240 | 121 |
| **40 条，sigma=240** | **77** |

**#130 质量表要的 1 / 5 / 10 组件档位，在这台设备上全都有富余。**

但这是 2023 年的旗舰，见 §5 第一条。

## 4. 第一轮那个 `GLASS_COUNT=10` 只画 6 条的 bug，修了

修法不是把窗口调大，是**按 count 反推间距与胶囊高度**
（`ui/glass_spike.slint` 的 `slot` / `capsule-height`），这样「摆 N 条」在
任何窗口尺寸下都真的是 N 条。Android 逻辑高度 902px，固定 92px 间距同样
装不下 10 条，不改的话这一轮会原样重演。

同时把**实际参数印在画面上**（`blur-sigma / count / tick / slot / h / 窗口`）。
这才是 notes §4.1 那条教训的正解：上一轮记错档位，根本原因是档位只存在于
环境变量和脚本参数里，截图上看不出来，于是「记录写的」和「画面跑的」
没有任何地方对得上。现在截图自带档位。

实测 count=10 的那张截图上：`slot=77px h=65px 窗口=406×902`，数得出 10 条。

## 5. 这一轮没做的 / 还不能说的

- **这是 2023 年的旗舰机**（骁龙 8 Gen 2 / Adreno 740）。§3 的所有数字
  **不代表低端机**，而低端机恰恰是 #130 的风险所在。第一轮记录里写的
  「低端 Android 才是那个压得动的设备」这句话**还没有被验证**，只是这台
  机器也没能证伪它。
- **没有和 Flutter 版并排对照。** 仍然只证明了「糊得出来」，没有证明
  「糊得像」。那要跑起 Flutter app 且首页导航栏要登录态。
- **没测功耗与温度。** 12 秒的采样窗口太短，热降频根本来不及发生，
  而玻璃效果真正的风险之一就是持续开着之后掉频。
- **count 档位不是面积梯度。** 现在的布局里胶囊高度随 count 反比缩小，
  10 条的总模糊面积和 5 条相差不大。#130 质量表要是想用「组件数」当负载
  轴，得先把这件事定义清楚，否则 1/5/10 三档量的几乎是同一个负载。
- **桌面侧的 wgpu 探针截图没取到**：跑到那一步时这台机器已经锁屏
  （`GetForegroundWindow()` 返回 0），GDI 和 PIL 的 ImageGrab 都抓不到活动
  桌面。文字判据够定论了（见
  [第一轮记录 §6](2026-11-24-glass-windows-notes.md)），截图留给下一次
  有人在机器前的时候。
- 截图不进仓库（见 spike README），用下面的命令随时重跑得出来。

## 6. 工具上的三条，给后面的 spike 省时间

### 6.1 `dumpsys gfxinfo` 对 NativeActivity 无效

`adb shell dumpsys gfxinfo <pkg>` 返回 `Total frames rendered: 0`，直方图
全零。它只统计 Android 的 HWUI 管线，而 NativeActivity 自己拿 GL 画，
它看不见。**这条对任何 Slint / 原生 GL 的 Android spike 都成立**，别在它
身上浪费时间，直接上 `SurfaceFlinger --latency` 或 `gpubusy`。

### 6.2 `am start` 传不了环境变量

桌面那两个对照开关读的是环境变量，Android 上没有命令行。解法是让
`android_main` 从外部数据目录的 `glass.conf` 读 `BLUR/COUNT/PERF/FPS/TICK`
再 `set_var`，**`run()` 一个字不用改**——两端跑同一组参数，不分叉成两套
参数通道。

### 6.3 面板刷新率必须**和截图同时取**

帧率读数的分母就是它：61 fps 在 60Hz 面板上是「满了」，在 120Hz 面板上是
「只跑到一半」。而系统会在 app 起来几秒后重新仲裁，启动时那次读数不作数。
驱动脚本现在每轮都把 `activeMode` 落到 `active-mode.txt`。

## 7. 怎么重跑

```sh
export JAVA_HOME="D:/as/jbr"
export ANDROID_HOME="$LOCALAPPDATA/Android/Sdk"
export ANDROID_NDK_ROOT="$ANDROID_HOME/ndk/30.0.16248370"
cargo apk build --release --lib

./android-drive.sh                                      # 默认档
./android-drive.sh --blur 0                             # 对照档
./android-drive.sh --count 10 --perf --fps 120 --tick 4 # 顶到 120Hz 量帧率
```

手机要解锁并保持亮屏（锁屏时 activity 不可见，Slint 不渲染，截出来是黑图
但脚本照样产出记录）。

**跑完记得还原刷新率设置**：本轮动过
`system/peak_refresh_rate`、`system/min_refresh_rate`、`system/is_smart_fps`
与 user-preferred display mode，收工时已全部还原成原值
（null / null / 1 / null）。
