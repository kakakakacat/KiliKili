# Liquid Glass 第三轮：关口 2 本体过了——GL 覆盖拿到了本帧真实背板（issue #130）

**这份是人写的记录**，和
[第一轮（Windows / FemtoVG）](2026-11-24-glass-windows-notes.md)、
[第二轮（Android / Skia）](2026-11-25-glass-android-notes.md) 同一个 spike。
纸面部分见[评估文档](2026-11-24-glass-wgpu-eval.md)。

**本轮结论一句话：#130 关口 2 的本体解决了，P0 五项全部跑在真实背板上，
两个后端各验一次。** 代价也量出来了。但同时查出一条会卡住五端的事，见 §6。

## 0. 上一轮把一条还通着的路判死了

第二轮的评估文档 §3 摆了四条路，结论是「没有任何一条同时满足『当前帧干净
背景 + 自定义 shader + Android 可用』」，于是候选 A（AfterRendering 覆盖）
整条出局。

**那一步推广错了。** 出局的只是 **wgpu 这个变体**：

| | notifier 拿到什么 | 能不能发 render pass |
|---|---|---|
| `GraphicsAPI::WGPU30` | `{instance, device, queue}` | **不能**，没有渲染目标 |
| `GraphicsAPI::NativeOpenGL` | `{get_proc_address}` | **能**，GL 有隐式当前帧缓冲 |

而 `NativeOpenGL` 正是桌面 femtovg（`opengl.rs:315`）和 **Android 上的
Skia**（`opengl_surface.rs:85`）给出的那个变体。第二轮的探针只问了 wgpu
那一侧，没有回头问 GL。

读源码时还有两条佐证，都指向「上游本来就预期有人在这里画东西」：

- femtovg 的 `lib.rs:316` 先 `flush_to_output` + `submit_commands`，**然后**
  才在 328 行发 `AfterRendering`，`present_surface` 在其后；Skia 在
  `lib.rs:992` 的 `ctx.flush(None)` 收尾，notifier 在 804 行其后。
  **回调跑的时候本帧已经画进帧缓冲、还没上屏。**
- `femtovg/opengl.rs:212` 的上游注释：「`screenshot()` 读的是当前绑定的帧
  缓冲，而 **`AfterRendering` notifier 可能在本帧最后一次 flush 之后重新
  绑定它**」。

## 1. 判据：两个后端都过

代码在 `spikes/slint-glass/src/gl_probe.rs`，`--features gl-probe`。

```
Windows / FemtoVG：
PROBE GL setup version=3.2 embedded=true renderer=AMD Radeon RX 7900 GRE glsl=OpenGL ES GLSL ES 3.20
PROBE GL AfterRendering draw_fbo=0 read_fbo=0 viewport=[0, 0, 1350, 840] scale=1.5
PROBE GL 结论 背板拷贝 成功——拿到的是本帧真实帧缓冲 (glGetError=0x0, 735x207，含 45px 边距)
PROBE GL 结论 自定义 shader 覆盖绘制 成功 (glGetError=0x0)

Android / Skia（Redmi K70，Adreno 740）：
PROBE GL setup version=3.2 embedded=true renderer=Adreno (TM) 740 glsl=OpenGL ES GLSL ES 3.20
PROBE GL AfterRendering draw_fbo=0 read_fbo=0 viewport=[0, 0, 1116, 2480] scale=2.75
PROBE GL 结论 背板拷贝 成功——拿到的是本帧真实帧缓冲 (glGetError=0x0, 1116x380，含 83px 边距)
PROBE GL 结论 自定义 shader 覆盖绘制 成功 (glGetError=0x0)
```

`draw_fbo=0 read_fbo=0`——绑的是窗口默认帧缓冲。于是
`glCopyTexSubImage2D` 读到的就是 Slint 本帧画完、还没 present 的内容，
**不是上一帧**（候选 B 的硬伤），**也不是合成的**（第二轮 wgpu 探针的硬伤）。

## 2. 捕获范围：3σ 边距，这就是关口 2 问的那件事

第一版只拷胶囊自己那块矩形，模糊到边缘时没有矩形外的内容可采，clamp 出一道
假硬边。改成**多拷 3σ**（σ=10、scale=2.75 时是 83 物理像素），和 Slint 原生
`backdrop-blur`「最远采到该值三倍」是同一个口径。

shader 侧随之要知道这圈边距在哪：`u_backdrop_origin` / `u_backdrop_size`。
漏掉这一步的表现是整张背板偏移一圈边距那么多，**看起来像「折射强度调过头」**。

边距带出一条真实约束：**两块玻璃靠得比 3σ 还近，就会互相采到对方**。
Demo 里两条胶囊之间因此留了 40px 空当（`ui/glass_spike.slint` 的 `gl-gap`）。
这条换到真实应用同样成立，#130 正文把「靠近时的玻璃融合」排在 P1，
现在它先以「已知物理约束」的形式存在，不用等到组件库那一轮再撞。

## 3. P0 五项，全部跑在真实背板上

| P0 | 谁做的 | 本轮状态 |
|---|---|---|
| 实时背景模糊 | 自绘，1/4 降采样 + 两趟可分离高斯 | ✅ |
| 半透明材质 | 自绘（渐变 0.24/0.10/0.18 + 白描边 0.36） | ✅ |
| 局部折射 | 自绘，`liquid_glass.frag` 逐行移植 | ✅ |
| Fresnel 边缘反射 | 自绘，**Flutter 版没有这项** | ✅ |
| Specular 高光 | 自绘，glint 0.18 / 带宽 4 | ✅ |

一条**移植账上的坑**，差点把对照弄错：Flutter 的玻璃不只是那 28 行 shader，
外面还套着一层 `DecoratedBox`（渐变 + 白描边）。只移植 shader 那一半的话，
画出来是一圈几乎看不见的折射边，**并排看会误判成「自绘这条不行」**。
两层都移植之后才可比。

两档材质都留着，缺一不可（`src/material.rs`）：

- `FLUTTER_BASELINE`——与 Flutter 的 shader 路径**同式**，用来判「像不像」。
  注意它的 `blur_radius` 必须按 0 跑：Flutter 那边 `ImageFilter.shader` 与
  `ImageFilter.blur(10)` 是**二选一**，10 是回退档的值，不是 shader 档的值。
  不清零的话这个「对照档」会比 Flutter 多一层模糊，等于拿一个并不存在的
  参照物去比。
- `P0_TARGET`——在它之上补模糊与 Fresnel，也就是本单 P0 要的全套。

截图（`shots/snap-*.png`，不进仓库，见 §7 复跑）：同一画面上下两条玻璃，
下面是 GL 自绘、上面是原生 `backdrop-blur`，背景是同一条移动彩带。
**观感像不像由人看**，这一条不由程序判（#130 已去掉与 Flutter 并排对照
这项产出，以仓库所有者目视为准）。

## 4. 代价：量出来了，而且和上一轮独立对上了

Redmi K70 / Adreno 740，面板钉在 120Hz（每帧预算 8.33ms），采样 12 秒，
方法同第二轮 §3.3（`gpubusy`，绕开 vsync）。脚本：`./android-measure.sh`。

| 档 | GPU 忙 | 折算每帧 | Graphics |
|---|---:|---:|---:|
| 地板（连原生模糊都关） | 24.2% | 2.02 ms | 56.4 MB |
| 原生 `backdrop-blur` | 32.0% | 2.67 ms | 61.8 MB |
| + GL 覆盖（无模糊：折射/Fresnel/高光/材质层） | 35.2% | 2.94 ms | 62.5 MB |
| + GL 覆盖（P0 全套） | 55.8% | 4.65 ms | 63.7 MB |

拆出来：

- **原生 `backdrop-blur` +0.65 ms**。第二轮独立量到的是 **+0.64 ms**，
  两次方法相同、档位不同，数字对上了——**这等于把那套测法交叉验证了一次**。
- **自绘折射 + Fresnel + 高光 + 材质层 +0.27 ms**。便宜，因为它是一趟
  全屏三角形、只采一次纹理。
- **自绘模糊 +1.71 ms**，是原生那笔的 **2.6 倍**。贵在全分辨率拷一张
  带边距的背板（1116×380）再降采样，而原生那条在渲染器内部做，省掉这一拷。

显存同样对得上账：GL 覆盖比只有原生时多 **1.87 MB**，而按尺寸算
背板 1116×380×4B = 1.70 MB、两张 1/4 模糊纹理 279×95×4B×2 = 0.21 MB，
合计 1.91 MB。**这不是巧合，是这条路径只有这几张纹理**——
全程没有 CPU 回读，每帧就一次 GPU 内拷贝（关口 4 问的「量化必要的复制」）。

桌面侧**没有可比的帧时间**：帧率被 vsync 钉死（第一轮 §3 已记），
而 Windows 上没有 `gpubusy` 那种免 root 的占用计数。这一条仍然空着。

## 5. 顺带修好的两件工具事，都在这台机器上反复咬人

### 5.1 不再截屏，改成程序自己出图

`GLASS_SNAPSHOT=path` 让程序用 Slint 的 `take_snapshot()` 自己存 PNG。
**截屏这条路在这台机器上不可靠**，三种失败方式都撞过：

1. 主屏跑着全屏游戏时，GDI 的 `CopyFromScreen` 对**另一块屏**也返回全黑；
2. 锁屏时窗口不渲染，截出来是黑图（第二轮那张 wgpu 探针截图就是这么丢的）；
3. 缩放 1.5 下 `GetWindowRect` 与 `CopyFromScreen` 差 1.5 倍（第一轮栽过）。

**自己出图把三条一次绕开**，而且实测 `take_snapshot` 的结果里**包含
AfterRendering 画上去的 GL 覆盖层**——这点本来不是想当然的，快照走的是
离屏渲染目标，和上屏那条不是同一个。

### 5.2 窗口不再抢焦点

`GLASS_WINDOW_POS=x,y` 走 `BackendSelector::with_winit_window_attributes_hook`，
在**窗口创建之前**就定下位置并 `with_active(false)`。事后用 `SetWindowPos`
挪是没用的，焦点那一下已经发生了。

这条是被用户当场逮到的：改用自出图之后我直接起 exe，绕过了挪窗口的脚本，
窗口又开回主屏、把正在打游戏的人踢了出来。**修在程序里，不是修在脚本里**
——只要有人直接跑 exe，脚本里的规矩就不作数。

### 5.3 pwsh 的 `$env:` 会跨调用残留

连着跑几轮时，上一轮的 `GLASS_*` 会**静默带进下一轮**：实测跑完
`MATERIAL=baseline` 再跑 `COUNT=10`，那一轮印出来的材质仍然是 baseline。
和第一轮「`GLASS_COUNT=10` 只画 6 条」同一个形状——记录写的和实际跑的
对不上，而两边都「看起来正常」。`shot.ps1` 现在每轮先清掉所有 `GLASS_*`。
画面上印着实际档位是第二道保险。

## 6. 查出一条会卡住五端的事：**Apple 平台上这条路大概率不通**

Slint 1.19.1 的 Skia 渲染器里，**Metal 是经由 wgpu 的**
（`i-slint-renderer-skia-1.19.1/lib.rs:248` 的 `metal_wgpu_surface_factory`，
`default_metal()` 的文档写着「always render using Metal, **through WGPU**」）。
那个 crate 目录下根本没有 `metal_surface.rs`，只有
`opengl_surface.rs` / `software_surface.rs` / `wgpu_29_surface.rs` /
`wgpu_30_surface.rs`。

而 wgpu 那个变体给 notifier 的是 `WGPU30{instance,device,queue}`，**没有
渲染目标**——正是 §0 里判掉的那条。

所以：

| 平台 | 渲染器 | notifier 变体 | GL 覆盖 |
|---|---|---|---|
| Windows | FemtoVG / GL | `NativeOpenGL` | ✅ 实测 |
| Android | Skia / ganesh-gl | `NativeOpenGL` | ✅ 实测 |
| Linux | FemtoVG / GL | 同桌面，**未验** | 大概率可用 |
| macOS | Skia → **Metal via wgpu** | `WGPU30` | ❌ **推断不通**，未验 |
| iOS | 同上 | `WGPU30` | ❌ **推断不通**，未验 |

**这是推断，不是实测**：macOS 上如果改用 femtovg 的 GL 后端（GL 在 macOS
已废弃但仍可用）也许仍然走得通，iOS 上则要看 GLES 还在不在。这件事排在
#225（五平台适配）最前面，**它决定玻璃是五端一致还是要分两套**。

## 7. 怎么复跑

```sh
cd spikes/slint-glass

# 桌面：开在空闲的那块屏、不抢焦点、程序自己出图
pwsh -NoProfile -Command "& .\shot.ps1 -Out shots/snap-p0.png -EnvVars @{GLASS_GL='1'; GLASS_COUNT='1'}"
pwsh -NoProfile -Command "& .\shot.ps1 -Out shots/snap-baseline.png -EnvVars @{GLASS_GL='1'; GLASS_GL_MATERIAL='baseline'}"

# Android
export JAVA_HOME="D:/as/jbr"
export ANDROID_HOME="$LOCALAPPDATA/Android/Sdk"
export ANDROID_NDK_ROOT="$ANDROID_HOME/ndk/30.0.16248370"
cargo apk build --release --lib --features gl-probe
./android-drive.sh --gl 1 --count 1 --out target/android-evidence-gl
./android-measure.sh            # 四档 GPU 占用与显存，约 2 分钟
```

构建开销（实测，续前两轮的账）：桌面增量 38 s–1 min 29 s（加了
`unstable-winit-030` 那次要重编 slint 自己）；Android `cargo apk build`
增量 **1 min 16 s**，没有新下任何预编译包。

## 8. 这一轮没做的

- **低端 Android 仍然没碰。** 这台是 2023 年的旗舰，§4 的数字不代表低端机，
  而低端机才是 #130 的风险所在。缺设备，转 #225。
- **功耗与温度没测。** 12 秒窗口太短，热降频来不及发生。
- **滚动残影没看。** 背景是匀速横移的彩带，不是真实列表滚动。
- **「原生模糊 + 自绘折射」这个组合没试。** §4 的数字摆明了它值得试：
  原生模糊 0.65 ms 比自绘模糊 1.71 ms 便宜 2.6 倍，而折射那部分只要
  0.27 ms。但两者叠在同一块表面上时，自绘那层采到的背板里已经含有原生
  那层画的渐变与描边，**折射会把描边一起抹开**——这是要看画面才能判的事，
  本轮没跑。ADR-016 把它列为首选的下一步。
- **桌面侧的帧时间**，见 §4 末尾。
