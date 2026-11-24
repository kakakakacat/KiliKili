# Liquid Glass 的 Slint 集成预研：立单前提已变，以及关口 1/2 的纸面答案（issue #130）

**这份不是实验台产出的记录，是读 Slint 1.19.1 的文档与源码、对照本仓库现状
写出来的评估，没有跑任何程序，也没有编译过任何代码。** 它和
[2026-11-22-flutter-patches-slint-eval.md](2026-11-22-flutter-patches-slint-eval.md)
同类，属于 [README](README.md) 里列的「手写例外」。

它回答 #130「首要技术关口」里**不需要 GPU 就能答**的那一部分，并把需要
GPU 才能答的部分收敛成一张待跑清单。**本文不下 Go/No-Go 结论**——按 #130
自己的要求，未验证的集成不能当成架构决策。

## 证据等级

| 标记 | 含义 |
|---|---|
| **文档** | Slint 1.19.1 的 rustdoc / 官方文档站 / CHANGELOG |
| **源码** | 读 `slint-ui/slint` 在 tag `v1.19.1` 上的源码得出，给到文件:行 |
| **仓库** | 本仓库已有的实测记录或代码 |
| **推断** | 以上都没有，按 Slint 的渲染模型推的。**这类不能单独作为 No-Go 依据，也不能单独作为动工依据** |

---

## 0. 先说最重要的一件事：#130 的技术前提在立单之后变了

**Slint 1.19.0 给 `Rectangle` 加了 `backdrop-blur`，官方描述就是
「for translucent glass effects」**（**文档**，CHANGELOG「Slint language」
一节）。

时间线值得记一笔：

| 日期 | 事件 |
|---|---|
| 2026-11-13 | #130 参考的《Slint + Rust + WGSL 跨平台 Liquid Glass 技术方案》 |
| **2026-11-16** | **Slint 1.19.0 发布，`backdrop-blur` 随之落地** |
| 2026-11-24 | 本文 |

也就是说，**#130 整单是在「Slint 没有背板能力」的前提下设计的**，而这个
前提在它立单三天后就不成立了。单子里那句「不依赖各平台原生玻璃组件」仍然
成立——`backdrop-blur` 是 Slint 自己实现的，不是系统玻璃控件——但「必须
自建 wgpu 管线才能拿到背板」这个隐含假设需要重新审。

`backdrop-blur` 的实况（**文档**，Rectangle 参考页）：

- 类型是 `length`，值是**高斯模糊的标准差**，`<= 0` 关闭。
- 覆盖矩形区域、**跟随 border radius**，对应 CSS 的 `backdrop-filter: blur()`。
- 与 CSS 不同，它**还会采到矩形外面的内容**，最远到 `backdrop-blur` 值的
  三倍——这正好是 #130 关口 2 里问的「捕获范围」。
- `opacity < 1` 或带 `cache-rendering-hint` 的元素内部，只模糊该元素的内容；
  矩形自身的 `opacity` 不限制模糊范围，只是把模糊结果和矩形一起淡化。
  **自采规则 Slint 替我们定义好了。**
- 官方点名的代价：「Blurring is expensive: it re-reads and filters every
  pixel beneath the rectangle each time the area is redrawn.」——这条直接
  对上 #126 的首页 feed 滚动卡顿，不是新风险，是同一个风险换了实现。

**有一处文档与 CHANGELOG 打架，必须跑一次才能定**：CHANGELOG 写
「Skia and FemtoVG only」，Rectangle 参考页却写「Currently only supported by
the Skia renderer; other renderers draw the rectangle without the blur」。
而 FemtoVG 的 wgpu 后端源码里确实有 `BackdropBlur` 的约束说明
（**源码**，`internal/renderers/femtovg/wgpu.rs`：`BackdropBlur` 需要 view
所在纹理带 `COPY_SRC`，否则背板不糊）。三处证据指向「FemtoVG 也有，参考页
滞后」，但这是**推断**，别当定论。

### 0.1 这对 #130 意味着什么

**不是「wgpu 路线作废」**，是**基线变了**：

- #130 的 P0 表里，「实时背景模糊、半透明材质」这两项 Slint 原生就有。
- 剩下的 P0——**局部折射、Fresnel 边缘反射、Specular 高光**——`backdrop-blur`
  给不了，仍然需要自己画。
- 对照 #130 评论 §2 那条已确认的事实：**Flutter 版实际只做了「胶囊 SDF
  边缘折射 + 单点高光」**，没有多尺度模糊、没有 Fresnel。那么把
  `backdrop-blur` 和现行 Flutter 版放一起比，差的其实只有**折射与 glint
  两件事**。

由此得到本文最该被采纳的一条建议（展开见 §7）：

> **Phase 1 的第一个 Demo 应该是「Slint 原生 `backdrop-blur` 能做到多像」，
> 而不是「wgpu 管线能不能搭起来」。** 前者一天能出结论，而且它决定了后者
> 到底要补多少东西——有可能只需要在原生模糊之上叠一层折射/高光，而不是
> 重做整条背景捕获 + 多尺度模糊管线。

---

## 1. 关口 1 已答：Slint 1.19 自带 wgpu 互操作，首轮不必 fork

#130 的第一个关口是「明确所选 Slint 版本、渲染后端及其可用接口；Slint 现有
渲染器不能被假定可直接注入任意 WGSL」。这一条可以结案。

**版本不用选**：`spikes/slint-mpv` 与 `spikes/slint-ui` 已经钉在 slint 1.19
（**仓库**），而 1.19.1 就是当前最新版，wgpu 互操作在这个版本里是现成的。

**接口长这样**（**文档**，`slint::wgpu_30`）：

| 要素 | 实况 |
|---|---|
| feature | `unstable-wgpu-29`（wgpu ^29.0.4）/ `unstable-wgpu-30`（wgpu ^30）。官方要求用 `version = "~1.19"` 波浪号约束，因为这类 feature 会随 wgpu 大版本整体换名 |
| 选后端 | `BackendSelector::new().require_wgpu_30(cfg).select()?` |
| 拿同一个 device/queue | `WGPUConfiguration::Manual { instance, adapter, device, queue }`，或 `WGPUConfiguration::Automatic(WGPUSettings)` 配 `Window::set_rendering_notifier()`，在回调里取 `GraphicsAPI::WGPU30 { instance, device, queue }` |
| 画自己的内容 | `set_rendering_notifier()` 的 underlay / overlay 两个时机；或把自己渲染好的 `wgpu::Texture` 经 `slint::Image::try_from()` 塞进场景当 `Image` 用 |
| 回调时机 | `RenderingState` 只有四个：`RenderingSetup` / `BeforeRendering` / `AfterRendering` / `RenderingTeardown` |
| 纹理导入约束 | `TextureImportError` 只有两种：格式**只能**是 `Rgba8Unorm` 或 `Rgba8UnormSrgb`；usage **必须**同时含 `TEXTURE_BINDING` 和 `RENDER_ATTACHMENT` |

对 #130 的三点直接影响：

1. **「首轮不以 fork Slint 为前提」成立**：Device/Queue 共享不用自己想办法。
2. **「往 Slint 的渲染器里注入 WGSL」确实不行**，单子里那句谨慎是对的。
   能做的是「在 Slint 之外用同一个 device/queue 画，再合成」。玻璃因此只能是
   **独立一层**，不是 Slint 控件的一个属性——这正是 `backdrop-blur` 相比
   之下最大的优势：它**是**控件的属性，跟着布局走。
3. **颜色空间的口子已被 API 收窄**：导入纹理只认 `Rgba8Unorm` /
   `Rgba8UnormSrgb`。#130「合成需明确颜色空间」在导入路径上只剩「选 linear
   还是 sRGB」一个决定。

### 1.1 wgpu 的后端选择不用我们管

`WGPUSettings` 走 wgpu 自己的 adapter 选择。#130 里「Apple 用 Metal、
Android/Linux 用 Vulkan、Windows 用 D3D12」那张候选表是 wgpu 的默认行为，
不是我们要实现的东西（**文档**）。该项从「待设计」降级为「待实测确认」。

---

## 2. 如果仍要走 wgpu：关口 2 的三条候选，其中一条已经可以判掉

#130 的第二个关口——「获取玻璃组件后方的真实 UI 背景纹理……避免把玻璃自身
再次采入背景」——在 wgpu 这条路上**没有被官方 API 直接解决**。
`RenderingState` 只有 Before / After 两个时机，**没有「场景画到一半」的
钩子**（**文档**）。

### 2.1 候选 A：AfterRendering 覆盖层 —— 读源码之后基本可以判掉

设想是：Slint 画完整个场景 → `AfterRendering` 回调里拿当前渲染目标当背景 →
画玻璃。读完源码，这条在 wgpu 下**缺一样东西：渲染目标本身**。

证据（**源码**，`internal/renderers/femtovg/lib.rs`）：

```rust
// …场景的绘制命令已经提交…
let commands = canvas.borrow_mut().flush_to_output(surface.render_output());
self.graphics_backend.submit_commands(commands);
// …
if let Some(callback) = self.rendering_notifier.borrow_mut().as_mut() {
    self.with_graphics_api(|api| callback.notify(RenderingState::AfterRendering, &api))?;
}
self.graphics_backend.present_surface(surface)?;
```

时机是对的——命令已提交、**还没 present**，surface 里就是这一帧的画面。
问题在于回调收到的 `GraphicsAPI::WGPU30` 只有 `{ instance, device, queue }`
三个字段（**文档**，`GraphicsAPI` 的定义），**没有 surface texture，也没有
它的 view**。wgpu 不是 OpenGL 那样的全局状态机，没有目标 view 就发不出
render pass。

- 这解释了为什么官方说「可以用 notifier 画 underlay/overlay」却只给了
  `Image::try_from` 的示例：**overlay 那条路是给 `NativeOpenGL` 变体的**
  （GL 下当前 FBO 就是目标，不需要额外传）。
- **推断**，不是定论：也许存在别的拿法（比如 `WGPUConfiguration::Manual`
  下自己持有 surface）。但 surface 是 Slint 的 winit 后端创建的，Manual 只
  交出 instance/adapter/device/queue，**推断它也拿不到**。
- **判据**：只要在 `AfterRendering` 里拿不到可写的 `TextureView`，A 出局。
  这一项是整张待跑清单里最便宜的，十几行就能试出来。

### 2.2 候选 B：Image 回灌 —— 官方示例的形状，但差一帧

把玻璃渲染成 `wgpu::Texture`，经 `Image::try_from()` 当 Slint 的 `Image`
放进场景。这就是 `slint::wgpu_30` 文档里那个示例。

- **代价**：**背景仍然没有来源**。喂给玻璃的只能是上一帧的结果，滚动时
  必然拖影——正是 #130 质量表 Low 档点名要查的「滞后、残影」。
- **判据**：滚动基准里看残影能不能接受；不能接受就只能当 Low 档实现。

### 2.3 候选 C：自定义 platform + 场景渲进纹理 —— 唯一干净，但最贵

`FemtoVGWGPURenderer` 的文档原话：实现自定义 Slint platform、**希望把场景
渲染进一张 wgpu texture** 时用它（**文档**）。公开方法是
`render_to_texture_view(&self, texture_view, width, height, format)`
（**源码**，`femtovg/wgpu.rs`）。于是：场景进纹理 → 这张纹理就是干净的背景
（玻璃不在里面）→ 我们做折射/高光/合成 → 输出到 surface。

- **同时解掉自采**：玻璃根本不参与场景渲染。
- **代价一**：要实现 Slint 的 platform abstraction（`Platform` /
  `WindowAdapter` / 事件循环接入）。这不是调 API，是接管窗口系统。
- **代价二**：`slint::platform::femtovg_renderer` 整个模块标着
  **`non-Android only`**（**文档**），所以这条路在 Android 上**不存在**。
- **代价三**：接管 platform 之后，`spikes/slint-ui` 已经验过的无障碍树、
  IME、惯性滚动（见 [Windows](2026-11-22-slint-ui-windows.md) /
  [macOS](2026-11-23-slint-ui-macos.md) 两份记录，**仓库**）要重新确认还在
  不在——它们是 winit 后端给的，不是 Slint 核心给的。
- **判据**：如果 C 是唯一能过关口 2 的路径，#130 的工作量要按「写一个 Slint
  后端」重估，而不是按「写一个渲染器」。

### 2.4 A/B/C 与 `backdrop-blur` 的关系

把 §0 的发现放进来之后，四条路的位置是这样的：

| 路径 | 背景从哪来 | 折射/Fresnel/高光 | Android | 工作量 |
|---|---|---|---|---|
| **`backdrop-blur`** | Slint 自己捕获，规则已定义 | **做不了** | Skia 路径，**大概率可用** | 近乎为零 |
| 候选 A | 拿不到渲染目标（§2.1） | 可以 | — | 小，但**大概率不成立** |
| 候选 B | 只有上一帧 | 可以 | 存疑（§3） | 中 |
| 候选 C | 干净，当前帧 | 可以 | **不存在** | 大（等于写后端） |

**没有任何一条同时满足「当前帧干净背景 + 自定义 shader + Android 可用」。**
这是本文最硬的一条结论，而且它是纸面就能得出的——不需要 GPU。

---

## 3. 平台矩阵：wgpu 在 Android 上存疑，而 `backdrop-blur` 不存疑

#130 把「五平台逐项适配」放在 Phase 4。**这个排法有问题**：Android 上能不能
走 wgpu，是会推翻技术路线的问题，不是适配细节。

已经确定的（**文档**）：

- `slint::platform::femtovg_renderer` 整个模块 **`non-Android only`**，
  `FemtoVGWGPURenderer` 就在里面，所以**它在 Android 上不存在**。
- `renderer-skia-opengl` 明确**不支持 iOS**，iOS 要用 `renderer-skia`
  （Metal）。与 [SPIKE-004-ios.md](SPIKE-004-ios.md) 实测的「Skia on Metal」
  对得上（**仓库**）。
- Android 后端是单独的 `backend-android-activity-06`，不走 winit。
- `unstable-wgpu-30` 是跨后端的 API 暴露 feature，不等于每个后端都会选
  wgpu 渲染路径。

没查到的：**没有任何文档说 Android 后端支持 wgpu 渲染**。这是「没找到」，
不是「确认不支持」。

对照本仓库已有的事实（**仓库**）：#218 / #219 的插帧在 Android（Adreno 610）
与 iOS 上跑的是 **GLSL + GL**，不是 wgpu——`player-interp` 依赖 glow，注释
写明「与 Slint 的 femtovg 渲染器同一个大版本」。**仓库里已经验证过的移动端
GPU 路径是 GL。**

反过来，`backdrop-blur` 的支持面是 **Skia**（**文档**），而 Android 与 iOS
用的都是 Skia。**它很可能是五端唯一齐的那条路**，这一点与 wgpu 的处境正好
相反，也是 §7 把它排在第一位的理由。

---

## 4. CI 代跑不了，这条已经有先例

`spikes/slint-ui` 与 `spikes/slint-mpv` 都不进 CI，理由写在
[VALIDATION-CAPABILITY](../VALIDATION-CAPABILITY.md)：要 GPU 合成、要显示
输出，runner 都没有（**仓库**）。Liquid Glass 落在同一条边界里，而且更严格：

- 托管 Linux runner 是纯 CPU 虚机，Vulkan 只有 lavapipe 这类软件实现；
  Windows runner 没有 GPU；macOS runner 上能否拿到 Metal 设备，历史上反复
  出过问题。
- 就算凑出软件栈「能出图」，#130 要的是**帧时间、掉帧、显存、功耗**。软件
  光栅器上的这些数字不能写进 ADR，比没有更坏。

**结论：#130 的 Phase 1 验收无法由 CI 产出，只能在开发机与真机上跑。**
这条应当写进 #130，免得下一轮又有人提议「挂到 CI 上」。

---

## 5. GLSL → WGSL 的逐项移植账

`shaders/liquid_glass.frag` 这 28 行是目前玻璃观感的事实规格（#130 评论已
确认）。移植到 WGSL 时逐项对照如下，**算法一行不改**，只记语言与环境差异。
配套的 WGSL 在 `spikes/slint-glass/shaders/glass.wgsl`。

（这张表在走 `backdrop-blur` 路线时同样有用：折射与 glint 那几行要叠在原生
模糊之上，常数还是这些。）

| 项 | GLSL（Impeller） | WGSL（wgpu） | 说明 |
|---|---|---|---|
| 片元坐标 | `FlutterFragCoord().xy` | `@builtin(position)` 的 `xy` | Impeller 的宏是为了抹平后端差异；wgpu 里 `position` 已经是左上原点的像素坐标 |
| 尺寸/背板绑定 | `uniform`，由 `ImageFilter` **自动**绑定 | 自己建 bind group：uniform buffer + `texture_2d<f32>` + `sampler` | **移植里唯一的真工作量**：Flutter 替我们做的背板绑定，在 wgpu 侧就是 §2 那件事 |
| 胶囊 SDF | `q = abs(p-center) - vec2(max(center.x-radius,0),0)`，`length(max(q,0)) + min(max(q.x,q.y),0) - radius` | 同式 | 无差异 |
| 法线 | `normalize(vec2(sign(dx)*max(abs(dx)-(cx-r),0), dy) + vec2(0.0001))` | 同式 | `+0.0001` 是防零向量。WGSL 的 `normalize` 对零向量同样未定义，必须保留 |
| 边缘带宽 | `rim = 1 - smoothstep(0,15,-distance)` | 同式 | 15 是**逻辑像素**。Flutter 的 shader 跑在逻辑像素坐标系，wgpu 的 `@builtin(position)` 是**物理像素**，HiDPI 上必须除以 scale factor，否则 2x 屏上边缘带只有一半宽 |
| 折射偏移 | `uv = (p + normal*rim*7.0)/u_size` | 同式 | 7 同样是逻辑像素 |
| Y 翻转 | `#ifdef IMPELLER_TARGET_OPENGLES` 时 `uv.y = 1-uv.y` | **不需要** | wgpu 纹理坐标统一左上原点。但这不代表翻转问题不会再出现：背景纹理怎么来的决定了它的朝向，[SPIKE-001](SPIKE-001-macos.md) / [SPIKE-004](SPIKE-004-ios.md) 记过 mpv 那条路上三端要 `--flip-y`、iOS 不要 |
| 采样 | `texture(u_backdrop, clamp(uv,0,1))` | `textureSample(t, s, clamp(uv, vec2(0), vec2(1)))` | `clamp` 保留；sampler 的 address mode 也设成 clamp-to-edge，双保险 |
| 高光 | `glint = (1-smoothstep(0,4,-distance))*0.18`，`mix(backdrop.rgb, vec3(1), glint)` | 同式 | 4 也是逻辑像素 |
| 输出 alpha | `backdrop.a` | 同 | 候选 C 下要重新确认（场景纹理的 alpha 未必是 1） |
| 颜色空间 | Impeller 自己管 | 导入纹理只能是 `Rgba8Unorm` / `Rgba8UnormSrgb` | §1 的约束。`mix` 到白色在 sRGB 与 linear 下**观感不同**，必须测一次再定 |

**两条是会咬人的，不是形式差异**：逻辑像素 vs 物理像素（15 / 7 / 4 三个
常数全都受影响），以及颜色空间下 `mix` 的观感差异。

另外，#130 评论里那份 `GlassNavigationSurface` 的材质层参数（表面渐变
0.24/0.10/0.18、白边 0.36、选中态、外阴影 22/(0,7)）**不在这 28 行里**，
它们是 Flutter 的 Widget 层画的，对应
`lib/common/widgets/floating_navigation_bar.dart`。这部分在 Slint 里能直接
用 `Rectangle` 的渐变、边框和阴影表达，**不需要 shader**。已按 #130
「参数不散落硬编码」的要求收进 `spikes/slint-glass/src/material.rs`。

---

## 6. 建议的 Phase 1 改法

#130 现在的 Phase 1 是「独立渲染 Demo，完成三项集成关口与 P0 效果，在至少
两个 GPU 后端上验证并形成 ADR」。按本文的发现，建议改成三步，**顺序很重要**：

1. **先量差距，别先搭管线。** 用 Slint 原生 `Rectangle` + `backdrop-blur`
   + 渐变/边框/阴影，照着 `GlassNavigationSurface` 的参数摆一个导航栏，和
   Flutter 版并排截图。**如果差距只在折射与 glint**，后面两步的范围会小
   一个数量级。
2. **再判 Android。** `backdrop-blur` 在 Android（Skia）上出不出效果、帧
   预算多少。这一项决定五端能不能齐，排在任何桌面 wgpu 实验之前。
3. **最后才判 wgpu 要不要上。** 先做 §2.1 那个十几行的判据（能不能在
   `AfterRendering` 里拿到可写 `TextureView`），A 出局就直接评估 C 的代价，
   并把「Android 没有 C」写进结论。

照这个顺序，**Go/No-Go 有可能在搭出 wgpu 管线之前就能下**——那正是 #130
自己写的「避免在集成关口通过前扩张组件库」的精神。

---

## 7. 必须跑才能答的（待跑清单）

按上面的顺序，优先级从高到低：

1. `backdrop-blur` 在 Skia 与 FemtoVG 上到底谁支持（§0 的文档打架）。
2. 原生 `backdrop-blur` 导航栏 vs Flutter 版的并排观感差距（§6 第 1 步）。
3. Android 上 `backdrop-blur` 的效果与帧预算（§6 第 2 步）。
4. `AfterRendering` 里能否拿到可写的 `TextureView`（§2.1 的判据）。
5. Android 上 Slint 能不能以 wgpu 渲染（§3）。
6. 逻辑像素 / 物理像素换算后，15 / 7 / 4 在 2x 屏上的观感是否与 Flutter 版
   一致（§5）。
7. `Rgba8Unorm` 与 `Rgba8UnormSrgb` 下 glint 的观感差异，定一个（§5）。
8. 滚动时的背景更新延迟与残影，以及 `backdrop-blur` 那条「每次重绘都要
   重读并过滤底下每个像素」的实际代价（对照 #126）。

**前三项不出结论之前，不要开始写 wgpu 管线，也不要开始写组件库。**

## 8. 本轮刻意没做的

- **没有写 ADR。** 关口 2 没有实测答案之前，写出来的只能是候选清单，而
  #130 要的是取舍结论。本文就是那份候选清单。
- **没有编译过 `spikes/slint-glass`**，一行都没有。它是骨架，不是交付，
  见它自己的 README。
- **没有碰 Flutter 侧的 `shaders/liquid_glass.frag`。** 在 Slint 路线确定
  之前，两套实现的「单一来源」问题（#130 评论 §4）还不到收口的时候。
- **没有去查 Slint 有没有计划把 backdrop 滤镜做成可扩展的。** 如果有，那
  才是比 A/B/C 都好的第四条路（玻璃仍然是控件属性，又能换 shader）。这件事
  值得问上游，但属于下一轮。
