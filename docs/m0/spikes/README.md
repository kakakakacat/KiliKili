# M0 Spike 实验记录

[SPIKE-PLAN.md](../SPIKE-PLAN.md) 规定了要跑哪些实验、按什么模板留证据；
这个目录放**跑出来的记录本身**。

记录由 spike 的实验台自动产出（`--record <文件>`），**不要手工编辑**：
重跑一次会整份覆盖。目前有两个产出方——`spikes/slint-mpv`（播放，
SPIKE-001/002）与 `spikes/slint-ui`（界面，SPIKE-003），记录格式同出一份
[`spikes/spike-harness`](../../../spikes/spike-harness)，所以能逐行对照。
要补充人看出来的结论，写在本目录下另一份同名 `-notes.md`，或者直接写进
对应的 issue。

**Android 那份是手写的例外**：那边没有命令行，入口是 `android_main`，
证据来自 logcat、mpv 自己的日志与帧缓冲回读，不是 `--record` 产出的。
跑法本身仍然是脚本化的（`spikes/slint-mpv/android-drive.sh` 驱动真机，
`spikes/media3-probe/drive.sh` 跑对照组），只是结论要人来写。

| 文件 | 平台 | 结论 |
|---|---|---|
| [2026-11-21-windows-dryrun.md](2026-11-21-windows-dryrun.md) | Windows 11 / RX 7900 GRE | 自动项全 PASS。**这是实验台自检，不是 SPIKE-001 的交付** |
| [SPIKE-001-macos.md](SPIKE-001-macos.md) | macOS | **未跑**，等一台 mac（issue #8） |
| [SPIKE-002-android.md](SPIKE-002-android.md) | Android 14 / Adreno 740 真机 | **#9 全部范围已答，PASS**：GL 上下文、触摸、旋转、surface 生命周期、后台音频、PiP，外加 Media3 回退对照。带一条代价：表面重建后零拷贝硬解要靠完整重载才接得回来 |
| [2026-11-22-slint-ui-windows.md](2026-11-22-slint-ui-windows.md)（＋[notes](2026-11-22-slint-ui-windows-notes.md)） | Windows 11 / RX 7900 GRE | **SPIKE-003（#10）前四项，自动项全 PASS**：两万行滚动单帧 0.9ms（p95）、Tab/IME 正常、UIA 读得出 List/ListItem。一条确定的额外工作量：`StyledText` 不支持行内图片，图文混排要自己排版，而 Slint 不给文本测量 API。**#10 未完**，见 notes §6 |

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
