# ADR-004: 播放器后端选择——五端统一 libmpv，原生播放器只留退路

Status: **Accepted（2026-11-24 仓库所有者批准）**。依据是四个平台的真机实测；
§6 列出的测量在批准时仍缺，**批准不以它们为前提**，补测跟踪在 issue #210——
若补测发现某个平台有硬伤，按决策四启用该平台的退路，并回来修订本 ADR。
Date: 2026-11-24
Related: RFC-0001 §7.2 / §7.3 / §16.2、ADR-003、issue #12、
SPIKE-001 #8、SPIKE-002 #9、SPIKE-004 #11、RUST-002 #2

## 背景

RFC-0001 把 Phase 2 的播放器写成「libmpv 优先；移动端 AVPlayer / Media3 备选」，
并要求 ADR-004 **以实测而不是假设**来裁决（§16.2）。当时的担心有三条：

1. 移动端的 libmpv 能不能和 Slint 合成在同一个画面里，能不能避免每帧 CPU 拷贝；
2. PiP、后台音频这类移动端能力，在 libmpv 上是不是要从头造；
3. 如果移动端最后要换原生播放器，B 站的分轨 DASH 怎么喂给它们。

M0 的四个 spike 已经在真机上把这三条都量过了。本 ADR 把结论定下来。

## 实测依据（一张表）

| 平台 | 合成方式 | 实测 | hwdec | 移动端能力 | 记录 |
|---|---|---|---|---|---|
| Windows 11 / RX 7900 GRE | Slint GL 上下文，同一 FBO | 58.6 fps，p95 17.0ms | `d3d11va-copy`（**有拷贝**） | — | [dryrun](../../m0/spikes/2026-11-21-windows-dryrun.md) |
| macOS 27.3 / M4 | 同上（GL 4.1 on Metal） | 58.0 fps，p95 19.98ms | `videotoolbox` 零拷贝 | — | [SPIKE-001](../../m0/spikes/2026-11-23-slint-mpv-macos-notes.md) |
| Android 14 / Adreno 740 | 同上（GLES 3.2） | 满帧，触摸分发正常 | `mediacodec` 零拷贝（要两个前提） | 后台音频、PiP、旋转 PASS | [SPIKE-002](../../m0/spikes/SPIKE-002-android.md) |
| iOS 27.3 / A18 Pro | mpv GLES → IOSurface → Metal → Slint（Skia on wgpu） | 60 fps，每帧 1.3ms | `videotoolbox` 零拷贝 | 后台音频、PiP PASS（含人工确认） | [SPIKE-004](../../m0/spikes/SPIKE-004-ios.md) |
| Linux | 无 GPU 验证环境，best-effort | — | — | — | [VALIDATION-CAPABILITY §3](../../m0/VALIDATION-CAPABILITY.md) |

原生播放器的对照：

| 备选 | 实测 | 真正的代价 |
|---|---|---|
| Android Media3 | 约 300 行 Kotlin 就跑通，后台、回前台、PiP 都由框架处理 | **换的是宿主形态**：Slint 1.19 的 Android 后端以 `NativeActivity` 为入口，没有 Java 视图树可放 SurfaceView（SPIKE-002「Media3 回退对照」） |
| iOS AVPlayer | 直接喂 m4s 要 46 秒起播；由 m4s 自带的 sidx 生成字节范围 HLS 后 **0.85 秒**，CPU 比 mpv 低 4 个点，PiP 可行 | 设 Referer 只能用私有键 `AVURLAssetHTTPHeaderFieldsKey`；HEVC/AV1/DV 未测；与其他四端的行为对齐要长期维护（SPIKE-004 §5） |

## 决策一：Phase 2 五端统一用 libmpv

`NativeMpvAdapter` 是 Phase 2 唯一要实现的 ADR-003 Player Port 后端，覆盖
Windows / macOS / Linux / Android / iOS。

理由，按分量排：

1. **能力已经验证，不再是「能不能」。** 上表四个平台的合成、硬解、移动端
   PiP 与后台音频都有了可工作的做法。担心 1、2 被实测否定了。
2. **一个后端，一套行为。** 弹幕时钟依赖的位置上报、seek 完成事件、倍速、
   错误分类（ADR-003）只需对齐一次。换成「桌面 mpv + 移动端原生」，就要
   对齐三套，而且 AVPlayer 与 Media3 的位置上报、缓冲、seek 语义各不相同。
3. **分轨的现成解法。** 基线用 mpv 的 `edl://` 拼接音视频分轨（RUST-002 #2、
   `docs/m0/BASELINE.md`），Player Port 的 `Open` 已按此设计；原生播放器
   没有对应的东西。
4. **包体不增加。** 现有 App 已经在五端带着 libmpv（Windows `libmpv-2.dll`
   38MB、Android `libmpv.so` 15MB、macOS `Mpv.framework` 7MB、iOS 整套 framework，
   见 VALIDATION-CAPABILITY §2），选 mpv 不会让包体在今天的基础上变大。
5. **铁律一致。** AGENTS.md §2：宿主可替换、Rust 不替换。mpv 由 Rust 直接驱动，
   换宿主框架时播放层不用重写；Media3 那条路反而要求一层 Kotlin 宿主。

## 决策二：合成路径按平台分两类，`flip_y` 按平台给默认值

- **Windows / macOS / Linux / Android**：在 Slint 渲染回调拿到的 GL 上下文里，
  把 mpv 画进同一个 FBO（`BeforeRendering`）。`flip_y = true`。
- **iOS**：Slint 在 iOS 上没有 GL 渲染器，mpv 0.39 在 iOS 上只有 GLES 渲染接口。
  mpv 画进 IOSurface 支撑的 CVPixelBuffer，同一块内存包成 Metal 纹理，经
  wgpu-hal 交给 Slint（`slint::Image::try_from(wgpu::Texture)`，feature
  `unstable-wgpu-30`）。`flip_y = false`。
  - 纹理必须是**真正的** `Rgba8Unorm`：IOSurface 上建 RGBA8 纹理，再加一个
    B↔R 互换的 swizzle 视图。不能把 BGRA 谎报成 RGBA，否则 Skia 渲染器会
    在 Slint 内部 abort（SPIKE-004 §2.3）。

SPIKE-001 notes 里「三端都要翻转、适配器不必分支」那句对 iOS 不成立。适配器
**必须**按平台给默认值，而不是写死一个常量。

## 决策三：平台胶水是必做项，不是优化项

下面每一条都是实测里不做就出错、而且多数是**静默**出错的事。它们属于
`NativeMpvAdapter` 的平台层（按 AGENTS.md §2 属于「平台 API 调用」，留在宿主一侧
也可以，但必须有）：

| 平台 | 必做 | 不做的后果 | 出处 |
|---|---|---|---|
| 全部 | `MPV_RENDER_PARAM_BLOCK_FOR_TARGET_TIME = 0` | UI 线程被锁在视频帧率上 | Windows dryrun 坑 3 |
| 全部 | 同步 `mpv_command` 不从渲染线程发 | 互等死锁、窗口冻死 | Windows dryrun 坑 1 |
| 全部 | render context 在 `loadfile` 之前建好 | 有声无画，HUD 一切正常 | Windows dryrun 坑 2 |
| 全部 | 外挂音轨用 `change-list audio-files append`，不用 `set audio-files` | Unix 上 URL 被 `:` 拆碎，悄悄没声音 | SPIKE-001 §3.1 |
| Android | 把 JavaVM 交给 FFmpeg + 显式 `hwdec=mediacodec` | 静默退到 `-copy` | SPIKE-002 坑 1 |
| Android | 声明 configChanges | 转屏重建 activity，进度归零 | SPIKE-002「旋转」 |
| Android | 表面重建后走恢复配方（记进度 → 重新 Open → seek） | 回前台黑屏 | SPIKE-002 坑 5 |
| Android / iOS | 退后台（且未进 PiP）前 `vid=no`，回前台 `vid=auto` | Android：后台还能不能出声取决于 mpv 碰巧的行为（VO 死掉后丢视频轨；片源没有音轨时整条播放被卸掉）；iOS：VT 会话失效（-12903），mpv **永久**退回软解、卡 3–4 秒 | SPIKE-002「后台音频」、SPIKE-004 §4 |
| Android / iOS | PiP 期间不当作「进了后台」 | 小窗里没画面 | SPIKE-002 坑 6、SPIKE-004 §3 |
| iOS | PiP 用 AVSampleBufferDisplayLayer + `contentSource:` 控制器，喂同一张 CVPixelBuffer | 没有 PiP | SPIKE-004 §3 |
| iOS | 后台且未进 PiP 时不提交 GLES 命令 | 文档规定会被系统杀（iOS 27 实测没杀，不能依赖） | SPIKE-004 §3.2 |

## 决策四：原生播放器不预先实现，只记下退路和代价

- **Android Media3**：只在 Android 主路出现无法绕过的问题时才启用。启用意味着
  换宿主形态（Slint 渲染进应用自己的 Surface，或 UI 改用 Android 原生），这需要
  单独一份 ADR，不是换个播放后端那么简单。
- **iOS AVPlayer**：是 Apple 真正移除 OpenGL ES 时的退路（mpv 0.39 在 iOS 上只有
  GLES 渲染接口；Apple 在 iOS 12 就宣布废弃了 GLES）。退路的成本已经量过：
  - 画面进 Slint 与 mpv 共用同一个「CVPixelBuffer → Slint」导入，弹幕叠加与 PiP
    的做法不变；
  - 分轨用「sidx → 字节范围 fMP4 HLS」，不需要转码，也不需要本地代理进程；
  - 若真启用，**播放列表的生成算法放进 `bili-core`**（sidx 解析、分片表、master
    的拼法都是「协议没规定、两端必须一致」的东西，AGENTS.md §2），宿主只负责把
    字符串交给 `AVAssetResourceLoader`。

也就是说，Player Port 不需要为「可能换后端」预留任何东西，ADR-003 的契约已经
足够；真要换的那一天，代价是上面写的这些，而不是未知数。

## 决策五：已知风险，接受并盯住

- **OpenGL 在 Apple 平台是废弃 API。** macOS 走的是 GL 4.1 on Metal，p95 19.98ms
  贴着 20ms 预算线；iOS 走 GLES on Metal。两边今天都能工作。盯的信号：mpv 上游
  出现 Metal / Vulkan（MoltenVK）的 render API，或者 Apple 在新 SDK 里真正拿掉 GL。
  前者出现就迁，后者出现就启用决策四的 iOS 退路。
- **Windows 是 `d3d11va-copy`。** 每帧多一次 GPU→CPU→GPU 拷贝，480p 下没造成问题，
  4K/HDR 下未知。零拷贝要走 D3D11 与 GL 的互操作（WGL_NV_DX_interop），没做。
- **错误恢复靠匹配 mpv / ffmpeg 的日志字符串**（RUST-002 #2）。Player Port 已定义
  自有错误分类（ADR-003），每一条字符串匹配都要有 fixture 回归，否则 mpv 升级会
  静默打断重连、软解回退这些逻辑。

## 6. 批准时仍缺的测量（补测见 #210）

issue #12 要求的测量项里，下面这些**还没有任何平台的数据**：

| 项 | 为什么不能跳过 | 怎么补 |
|---|---|---|
| HEVC / AV1 / 杜比视界的硬解 | `video_decode_type.dart` 的四种编码里，480p 未登录只拿到 `avc1`；HEVC 是 B 站主力 | 登录态取流，四端各跑一次，看 `hwdec-current` |
| 1080p / 4K 下的帧耗时与 CPU | macOS p95 已经贴线；Windows 有拷贝 | 同上 |
| Seek、倍速、切清晰度 | 弹幕时钟依赖它们（ADR-003），只在 Windows / macOS 的自动项里量过 seek | 在 Android、iOS 实验台补上这三个动作 |
| 内存 / RSS | 移动端被系统杀的主要原因 | 实验台已有 CPU，补 RSS |
| 音画同步 | 只在 macOS 人工听过 | 移动端各听一次，或者用 `avsync` 属性自动判定 |

**这些缺口不改变决策一的方向**：它们问的是「mpv 在高码率、新编码下够不够快」，
而原生播放器在这些项上同样没量过。若补测时某个平台出现硬伤，按决策四启用该
平台的退路，并回来修订本 ADR。

## 后果

- RFC-0001 §7.3 的表、§13 第 4 问（播放器策略）、§16.2、§16.8 的「移动端 AVPlayer/Media3 候选」一行据此更新。
- Phase 2 的播放器工作量 = 一个 `NativeMpvAdapter` + 决策三那张表里的平台胶水，
  不包括原生播放器适配器。
- ADR-003 不需要改：契约里已经有 surface attach/detach、前后台生命周期与
  分类错误，决策三的每一条都能映射到这些事件上。
