# M0 Spike 实验记录

[SPIKE-PLAN.md](../SPIKE-PLAN.md) 规定了要跑哪些实验、按什么模板留证据；
这个目录放**跑出来的记录本身**。

记录由 spike 的实验台自动产出（`--record <文件>`），**不要手工编辑**：
重跑一次会整份覆盖。目前有两个产出方——`spikes/slint-mpv`（播放，
SPIKE-001/002）与 `spikes/slint-ui`（界面，SPIKE-003），记录格式同出一份
[`spikes/spike-harness`](../../../spikes/spike-harness)，所以能逐行对照。
要补充人看出来的结论，写在本目录下另一份同名 `-notes.md`，或者直接写进
对应的 issue。

**手写的有几份例外**，它们不是 `--record` 产出的，可以也必须手工编辑：
`2026-11-22-flutter-patches-slint-eval.md` 与
`2026-11-24-glass-wgpu-eval.md`（读 diff / 读 Slint 文档与源码写出来的
评估，根本没有程序可跑）、`2026-11-24-glass-windows-notes.md`、
`2026-11-25-glass-android-notes.md`、
`2026-11-25-glass-gl-overlay-notes.md` 与
`2026-11-25-glass-phase2-notes.md`（#130 的
实验台还没接 `spike-harness`，结论要人看截图判）、下面这份 Android 的、iOS 的 `SPIKE-004-ios.md`、#210 的补测记录
（理由同 Android：入口是 App，证据来自 `devicectl --console` 与真机截图），以及 #218 的
插帧记录（汇总实验室模式、macOS、Android 与 iOS 真机几类运行）。

**Android 那份是手写的例外**：那边没有命令行，入口是 `android_main`，
证据来自 logcat、mpv 自己的日志与帧缓冲回读，不是 `--record` 产出的。
跑法本身仍然是脚本化的（`spikes/slint-mpv/android-drive.sh` 驱动真机，
`spikes/media3-probe/drive.sh` 跑对照组），只是结论要人来写。

| 文件 | 平台 | 结论 |
|---|---|---|
| [2026-11-21-windows-dryrun.md](2026-11-21-windows-dryrun.md) | Windows 11 / RX 7900 GRE | 自动项全 PASS。**这是实验台自检，不是 SPIKE-001 的交付** |
| [SPIKE-001-macos.md](SPIKE-001-macos.md)（＋[notes](2026-11-23-slint-mpv-macos-notes.md)） | macOS 27.3 / Apple M4 | **#8 PASS**：真实 DASH 480p，58 fps、p95 19.98ms，`videotoolbox` 零拷贝硬解，人工项（层级/方向/HiDPI/音画同步）全过。要 `--flip-y`——三个平台一致，适配器不必分支。顺带修掉两个实验台 bug：外挂音轨在 Unix 上被 `:` 拆碎、记录里写进带签名的 URL |
| [SPIKE-002-android.md](SPIKE-002-android.md) | Android 14 / Adreno 740 真机 | **#9 全部范围已答，PASS**：GL 上下文、触摸、旋转、surface 生命周期、后台音频、PiP，外加 Media3 回退对照。带一条代价：表面重建后零拷贝硬解要靠完整重载才接得回来 |
| [2026-11-22-slint-ui-windows.md](2026-11-22-slint-ui-windows.md)（＋[notes](2026-11-22-slint-ui-windows-notes.md)） | Windows 11 / RX 7900 GRE | **SPIKE-003（#10）前四项，自动项全 PASS**：两万行滚动单帧 0.9ms（p95）、Tab/IME 正常、UIA 读得出 List/ListItem。一条确定的额外工作量：`StyledText` 不支持行内图片，图文混排要自己排版，而 Slint 不给文本测量 API。**#10 未完**，见 notes §6 |
| [2026-11-22-slint-ui-android.md](2026-11-22-slint-ui-android.md)（＋[notes](2026-11-22-slint-ui-android-notes.md)） | Android 14 / Adreno 740 真机 | **SPIKE-003（#10）在真机上的同一轮**：长列表过（滚动 p95 10.5ms，但余量只剩 37%）、**触控惯性过**（手指驱动 122fps，`Flickable` 自带）。最贵的结论是**无障碍树是空的**——Slint 的 Android 后端根本没接，TalkBack 一个节点都读不到 |
| [2026-11-23-slint-ui-macos.md](2026-11-23-slint-ui-macos.md)（＋[notes](2026-11-23-slint-ui-macos-notes.md)） | macOS 27.3 / Apple M4 | **SPIKE-003（#10）的 macOS 一轮＋旁白实读**：列表与富文本同 Windows（滚动 p95 0.6ms）。旁白控件全 PASS；**长列表只读得到第一屏**——虚拟化对无障碍不透明，旁白报「1/7」、翻页滚不动。另撞出一条 macOS 独有的：固定宽度的子元素会把窗口最大宽度钉死。新写了 mac 外部驱动，Tab / 英文 / 简体拼音组字 / 最小化全 PASS |
| [SPIKE-004-ios.md](SPIKE-004-ios.md) | iOS 27.3 / iPhone 16 Pro Max 真机 | **#11 PASS**：mpv 经 GLES 画进 IOSurface、零拷贝进 Slint（Skia on Metal），60 fps、每帧 1.3ms、`videotoolbox` 非 copy；PiP 自动进入、小窗持续出帧；后台音频要在退后台前 `vid=no`，否则 VT 会话失效、永久退回软解。**iOS 上不翻转**（与三端相反）。AVPlayer 对照：m4s 自带 sidx，生成字节范围 HLS 即可 0.85 秒起播，不需要代理或转码 |
| [2026-11-24-adr004-measurements.md](2026-11-24-adr004-measurements.md) | macOS 27.3 / M4；iOS 27.3 / iPhone 16 Pro Max；Android 14 / Adreno 610 | **ADR-004 §6 补测（#210）**：HEVC、杜比视界两端硬解；AV1 在 iOS 上是软解。iOS seek / 倍速 / 切清晰度都通，音画差 ≤ 12ms。**发现 iOS 零拷贝偏绿**（mpv 的 nv12 interop 把 V 读成 U），修法 `hwdec-image-format=bgra`。**Android 低端机 4K 能力表说支持、实际 configure 失败，静默落到软解**，要按能力选清晰度。macOS p95 贴线是显示环境的读数。Windows 未测 |
| [2026-11-24-interp-218.md](2026-11-24-interp-218.md) | macOS 27.3 / M4；Android 14 / Adreno 610；iOS 27.3 / iPhone 16 Pro Max | **#218 插帧 spike**：mpv 的每张源帧都能识别并带上时间戳（`target_time` 实为纳秒）；mpv 最多提前交一张帧，双向插帧要把画面推迟一个源帧间隔并用 `audio-delay` 补偿。只用 fragment shader 的块匹配 + 运动补偿在 **Adreno 610 上 720p 30→60 稳定 60 fps**（运动估计 4.9 ms，连续 15 分钟不降频），iOS 同一份 GLSL 跑满（运动估计约 3 ms），BBB 真值对比 SSIM 0.958 对线性混合 0.906。三个会把低端机拖到 51 fps 的坑：渲染线程同步取 mpv 属性、`glReadPixels` 回读（改用 transform feedback）、着色器里的 uniform 循环与整数除法 |
| [2026-11-22-flutter-patches-slint-eval.md](2026-11-22-flutter-patches-slint-eval.md) | 不涉及（读 diff 与 Slint 源码） | **SPIKE-003（#10）后半边：34 个 Flutter 补丁逐项评估**。去重后是 26 个独立问题：原生支持 3 / 不适用 4 / 需自写 12 / 做不到 7。做不到的归成四件事，**跨元素文本选择**与**嵌套滚动的手势消歧**是 Phase 2 No-Go 候选 |
| [2026-11-24-glass-wgpu-eval.md](2026-11-24-glass-wgpu-eval.md) | 不涉及（读 Slint 1.19.1 的文档与源码） | **#130 的 Liquid Glass 预研，纸面部分**。最硬的一条：**立单前提已变**——Slint 1.19.0（立单后三天）给 `Rectangle` 加了 `backdrop-blur`，官方描述就是「for translucent glass effects」，#130 「必须自建 wgpu 管线才能拿到背板」的隐含假设不再成立。关口 1（wgpu 接口）已答，首轮不必 fork；关口 2 的三条候选里 **A 基本可判掉**（`AfterRendering` 拿不到渲染目标），**C 在 Android 上不存在**。没有任何一条同时满足「当前帧干净背景 + 自定义 shader + Android 可用」 |
| [2026-11-24-glass-windows-notes.md](2026-11-24-glass-windows-notes.md) | Windows 11 / RX 7900 GRE | **#130 第一轮实测（Slint 1.19.1 / FemtoVG）**：骨架一次编过（首次 6 min 01 s、零 warning，增量 12 s）。**`backdrop-blur` 在 FemtoVG 上生效**——官方 Rectangle 参考页那句「only supported by the Skia renderer」是过时的，CHANGELOG 对，于是 Skia 那笔没人付过的账不用付。**帧率这项是「测不出」不是「不贵」**：1/5/10 组件 × 开关 blur 六组全 144 fps（= 刷新率），`refresh_full_speed` 绕不过 vsync。两个弄脏实验的毛病：`GLASS_COUNT=10` 实际只渲染 6 条（已修）、截图脚本的 DPI 偏移。**2026-11-25 加写 §6：wgpu 关口 2 的判据跑了**——`GraphicsAPI::WGPU30` 只给 `instance/device/queue`、**没有渲染目标**，候选 A 出局有了运行时证据；而**自绘纹理导入 Slint 这条通**，`glass.wgsl` 第一次过 naga 零报错。仍未解决的是「怎么拿到真实背板」——探针里那张是合成的 |
| [2026-11-25-glass-android-notes.md](2026-11-25-glass-android-notes.md) | Redmi K70（SM8550 / Adreno 740）/ Android 14 | **#130 第二轮实测（Slint 1.19.1 / Skia）**：`backdrop-blur` 在 Skia 上同样生效，判据是量化的（边缘能量掉 42.8%，且度量与背景相位无关）。**帧代价第一次量出来了，靠的是换方法不是换设备**——帧率两端都被 vsync 钉死（桌面 144、手机 61），改读 Adreno 的 `gpubusy`（免 root、读一次清零）才绕开：**一条导航栏 ≈0.64 ms/帧，十条 ≈1.66 ms/帧**，拐点在 40 条。一条反例值得记住：**在 vsync 锁死的设备上用占用率算成本会被 DVFS 骗**，60Hz 下量的那组非单调、外推到 120Hz 被实测证伪。另有三条工具教训：`gfxinfo` 对 NativeActivity 无效、`am start` 传不了环境变量、面板刷新率必须和截图同时取。**已测的是 2023 旗舰，不代表低端机** |
| [2026-11-25-glass-gl-overlay-notes.md](2026-11-25-glass-gl-overlay-notes.md) | Windows 11 / RX 7900 GRE ＋ Android 14 / Adreno 740 真机 | **#130 第三轮：关口 2 的本体过了**。上一轮把「`AfterRendering` 覆盖」整条判为出局，**那一步推广错了**——出局的只是 wgpu 变体（`WGPU30` 没有渲染目标），而 `NativeOpenGL` 带 `get_proc_address`、GL 有隐式当前帧缓冲，**桌面 femtovg 与 Android 的 Skia 给 notifier 的都是它**。于是 `glCopyTexSubImage2D` 拷本帧帧缓冲、再用自己的 GLSL 画玻璃这条路跑通了，两个后端各验一次，`draw_fbo=0 read_fbo=0`、零 GL 错误。**P0 五项全部跑在真实背板上**。代价拆开了：原生 `backdrop-blur` +0.65ms、自绘折射/Fresnel/高光 +0.27ms、自绘模糊 +1.71ms，显存 +1.87MB 与理论的 1.91MB 对得上。顺带查出**Apple 两端大概率不通**（Skia 走 Metal-via-wgpu，推断未验，排 #225）。取舍写进 ADR-016 |
| [2026-11-25-glass-phase2-notes.md](2026-11-25-glass-phase2-notes.md) | Windows 11 / RX 7900 GRE ＋ Android 14 / Adreno 740 真机 | **#130 第四轮：Phase 2 的四项能力做完，ADR-016 决策四有了答案**。决策四（原生模糊 + 自绘折射）**可用，但只有一种摆法可用**：原生那条必须只开 `backdrop-blur`，渐变与描边整个归自绘，否则折射会把那道 1px 白描边拖出第二道（判据是过圆头的一行亮度剖面，双峰相距 7 物理px）。附带一条会影响组件库的：**这个组合与多尺度模糊互斥**——原生只有一档 σ，从边到心是常数（残余对比度 18.0/18.6/18.6），而自绘三级是 15.1/9.1/4.9。七档代价：多尺度再加两级只要 +0.19ms、融合 ≈0，**自绘单级模糊第三次量到 1.70ms**。另修掉两个 Phase 1 的静默错误（背板边距漏算折射、GL 状态存档放在了拷贝之后）|

## 为什么先有一份 Windows 的

[SPIKE-PLAN](../SPIKE-PLAN.md) 的顺序是 macOS 先行、Windows 排在 macOS +
Android 门槛之后，这个顺序没有变。先在 Windows 上跑一次，是因为实验台本身
需要被验证：**一份没跑过的 harness 不是交付，是一堆待调试的代码**。拿它在
手边的机器上跑通一遍，mac 那次就只需要回答平台差异，而不是从头排查
「render context 建不起来」这类与平台无关的问题。

这一次也确实暴露了五个与平台无关的坑，都已经修在代码里。**前两个是会写进
Phase 2 适配器的教训，不是 spike 自己的毛病**：

1. **同步的 `mpv_command` 不能从渲染线程发。** 它要等 mpv 核心处理完，
   而核心在某些时刻等着渲染线程画完一帧，两边互等，窗口整个冻死。手动按
   左方向键回退 seek 时稳定复现，脚本化那次恰好没撞上——**这类 bug 不会
   在自动化里暴露，只会在用户手里暴露**。改用 `mpv_command_async` /
   `mpv_set_property_async` 之后，连按六次 seek/暂停三轮都没再冻。
2. **render context 必须在 `loadfile` 之前建好。** 否则 mpv 照样解码、
   照样走进度、照样出声，画面却永远是黑的，HUD 上一切正常。手动模式最初
   就在开窗之前打开文件，黑了一整屏才查出来。现在开播统一等渲染侧就绪，
   脚本的零点也对齐到那一刻（各机器窗口初始化耗时不同，按进程启动计时的
   记录没法互相对照）。
3. `mpv_render_context_render` **默认会阻塞到该帧的目标显示时刻**。不关掉它，
   整个 UI 线程被片源帧率拖住：30fps 的样片把窗口锁在 31.6fps、p50 33ms，
   看上去像是弹幕层画不动。关掉之后同一台机器 58fps、p50 16ms。
4. 截图（`glReadPixels` + PNG）是在事件循环里同步做的。4K 下 33MB，debug
   构建曾经把循环堵了 **8.5 秒**，脚本后半段的五个步骤挤在同一 tick 里执行，
   p99 也被它一个人顶到 2.4 秒。现在截图后会跳过一个帧间隔。
5. 轨道的 y 原先按每条弹幕自己的字号算，fixture 里 25px 与 64px 混排时
   5 号轨道的大字会跑到 10 号轨道的位置上，屏幕上就是两条字叠在一起。
   改成统一行高。

Windows 这一跑同时附带回答了 [VALIDATION-CAPABILITY](../VALIDATION-CAPABILITY.md)
里「Windows 真机 GUI ✅」那一格在 Slint 形态下还成不成立：成立。

## 记录里的 commit 为什么总是「工作区不干净」

记录是在它自己被提交**之前**跑出来的，所以那一栏必然写着上一个 commit 加
「否」。这不是疏漏：要让它写出自己所在的 commit，就得先提交再重跑、再提交，
来回两次。真正需要精确复现时，看记录文件自己的 `git log` 即可。

## 截图去哪了

记录里列的 PNG **不提交**：一次实验六张 4K 截图近 3MB，而它们随时可以用记录
开头那行命令原样重跑出来。需要留存时自己归档，别往仓库里塞。

**这条规矩有一个代价，SPIKE-003 撞上了**：截图不提交，于是「数都对、画面
不对」这种 bug 只能靠跑的人当场看一眼。#10 第一轮的滚动结论整段是假的
（列表根本没动），而记录里每一个数都自洽——详见
[2026-11-22 的 notes](2026-11-22-slint-ui-windows-notes.md) §0。**跑完务必
自己翻一遍截图，别只读记录。**

**`assets/2026-11-22-slint-ui-android-a11y.xml` 也是例外，理由同上**：3.6 KB
的一棵无障碍树，是「Slint 在 Android 上什么都没暴露给 TalkBack」这条结论的
全部证据，而重跑它要一台连着线的真机。

**`assets/` 下的 Android 那几张是刻意的例外。** 这条规矩成立的前提是「随时
能重跑出来」——Android 这些不满足：要一台连着线的真机、要人把锁屏解开，
而它们恰恰是 #9 这个 Go/No-Go 门槛上唯一能证明「画面到底长什么样」的东西。
破例的代价压到最小：每张都缩过，五张加起来 1.4MB。

| 图 | 它证明什么 | 不留就没法说清的事 |
|---|---|---|
| `-fbo-readback` | 视频真的出现在 UI 之下 | 第一轮的决定性结论 |
| `-landscape` | 转屏之后画面与布局都跟上了 | 「旋转 PASS」不能只靠一行尺寸日志 |
| `-pip` | 小窗里视频还在画 | 日志只能说「进了 PiP」，说不了「小窗里有画面」 |
| `-resume-recover` | 回前台后画面没了、跑完恢复配方又回来 | 整份记录里最贵的那个发现，一句话说不清 |
| `-media3-layers` | 回退路线的三层（视频 / GL 覆盖层 / 普通 View）叠得住 | 「回退可行」否则只是一句断言 |
| `2026-11-23-macos-no-flip` / `-flip-y` | macOS 上 FBO 也是倒的，加 `--flip-y` 之后才对（#8） | 「三端都要翻转」这条结论；重跑要那台 M4，理由与 Android 相同。两张合计 0.4MB |
| `2026-11-24-ios-*`（四张） | iOS 零拷贝画面、`flip_y=1` 的倒像、PiP 小窗、AVPlayer + 弹幕叠加（#11） | 「iOS 上不翻转」与「PiP 小窗里有画面」；重跑要一台签过名的 iPhone，理由与 Android 相同。PiP 那张只裁了小窗，系统界面里有账号名。四张合计 0.26MB |
