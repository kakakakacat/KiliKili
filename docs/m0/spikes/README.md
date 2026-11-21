# M0 Spike 实验记录

[SPIKE-PLAN.md](../SPIKE-PLAN.md) 规定了要跑哪些实验、按什么模板留证据；
这个目录放**跑出来的记录本身**。

记录由 `spikes/slint-mpv` 自动产出（`--record <文件>`），**不要手工编辑**：
重跑一次会整份覆盖。要补充人看出来的结论，写在本目录下另一份同名
`-notes.md`，或者直接写进对应的 issue。

**Android 那份是手写的例外**：那边没有命令行，入口是 `android_main`，
证据来自 logcat、mpv 自己的日志与帧缓冲回读，不是 `--record` 产出的。

| 文件 | 平台 | 结论 |
|---|---|---|
| [2026-11-21-windows-dryrun.md](2026-11-21-windows-dryrun.md) | Windows 11 / RX 7900 GRE | 自动项全 PASS。**这是实验台自检，不是 SPIKE-001 的交付** |
| [SPIKE-001-macos.md](SPIKE-001-macos.md) | macOS | **未跑**，等一台 mac（issue #8） |
| [SPIKE-002-android.md](SPIKE-002-android.md) | Android 14 / Adreno 740 真机 | 决定性问题 **PASS**：Slint 给得出 GL 上下文，mpv 画得进同一个 FBO（issue #9） |

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

**`assets/` 下的 Android 那一张是刻意的例外。** 这条规矩成立的前提是「随时
能重跑出来」——Android 这张不满足：要一台连着线的真机、要人把锁屏解开，
而它恰恰是 #9 这个 Go/No-Go 门槛唯一能证明「视频真的出现在 UI 之下」的东西。
破例的代价压到最小：只裁视频那一带、缩到 720 宽、300KB。
