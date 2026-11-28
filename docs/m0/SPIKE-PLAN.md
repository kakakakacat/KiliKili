# M0 Slint + Playback Spike: executable acceptance plan

Status: **五条 spike 全部实跑完毕，结论见下方 backlog；Phase 2 的 Go/No-Go 还欠一份 ADR-007**（2026-11-28 核对）
Start reference: RFC-0001 v0.2

实验台有三个，都自成 workspace、`rust.yml` 都不碰：`spikes/slint-mpv`（播放，
SPIKE-001/002）、`spikes/slint-ui`（界面，SPIKE-003）、`spikes/slint-glass`
（Liquid Glass，#130）。用法见各自的 README，跑出来的记录放
[spikes/](spikes/README.md)。

**门槛实验已经跑完**：macOS（#8）、Android（#9）、界面（#10）、iOS（#11）四条
全部 PASS，ADR-004 据此批准。2026-11-21 那份 Windows 记录是**实验台自检，不是
SPIKE-001 的交付**——它回答的是「实验台本身能不能用」，让后面 macOS 那一跑只需
回答平台差异；顺带暴露的五个与平台无关的坑见 [spikes/README.md](spikes/README.md)。

**所以现在欠的不是实验，是文书**：四轮结论还没收成一份可批准的 ADR-007，Phase 2
的 Go/No-Go 目前只散在各份 notes 里。

## Scope and order

1. **macOS first**: minimal Slint window with embedded libmpv video and true overlay danmaku; run audio-video DASH, pause, seek, fullscreen and window resize.
2. **Android second**: media surface layering, gestures, lifecycle/rotation, hardware acceleration, background audio, PiP feasibility.
3. **UI viability in parallel**: large lazy list scroll, rich comments with emoji/image/text, Chinese/English IME composition and focus. Screen reader accessibility is **out of project scope** (2026-11-28 owner decision, RFC-0001 §1.2) and is no longer part of this spike or of any gate below.
4. **iOS risk review early**: libmpv build and native surface/PiP/background audio integration evidence; complete real-device gate before shipping iOS Slint build.
5. **Windows/Linux**: build integration and window/compositor checks after macOS+Android gates; GUI validation cannot be replaced by CI build.

## Test artifact template

Record per experiment:
- Platform, OS version, CPU/GPU, device model and physical/emulated status
- Repository commit, dependency lockfile, compiler/toolchain versions
- Media type (DASH split video/audio, codec, duration, resolution, request headers redacted)
- Video renderer, hardware decoder indicator, layer hierarchy, FPS, frame time, CPU, RSS, graphics allocations
- Expected result, actual result, crash/trace logs, minimal reproduction and link to artifact
- Result: PASS / FAIL / BLOCKED / NOT TESTED

## Explicit Go / No-Go

Phase 2 Slint migration is **NO-GO** if video compositing/overlay, mobile lifecycle, large lists, essential rich text or IME cannot meet acceptable functional quality. Phase 1 Flutter + Rust Core remains a valid stable release.

## Initial backlog

- [x] M0-SPIKE-01: macOS libmpv + Slint video + danmaku overlay
      （2026-11-23 在本机 M4 实跑，**#8 PASS**：58 fps、p95 19.98ms、`videotoolbox`
      零拷贝，层级/方向/HiDPI/音画同步的人工项全过。见
      [spikes/SPIKE-001-macos.md](spikes/SPIKE-001-macos.md) ＋
      [notes](spikes/2026-11-23-slint-mpv-macos-notes.md)）
- [x] M0-SPIKE-02: Android surface/lifecycle + libmpv versus Media3 fallback
      （Android 14 / Adreno 740 真机，**#9 PASS**：GL 上下文、触摸、旋转、surface
      生命周期、后台音频、PiP，外加 Media3 回退对照。代价一条：表面重建后零拷贝
      硬解要靠完整重载才接得回来。见 [spikes/SPIKE-002-android.md](spikes/SPIKE-002-android.md)）
- [x] M0-SPIKE-03: Slint list virtualization, rich content, IME
      （**#10 PASS**，跑了 Windows、Android 真机与 macOS 三轮：两万行滚动 p95
      0.9ms / 10.5ms / 0.6ms，Tab 焦点与中文组字全过。确定的工作量有一条：
      `StyledText` 不支持行内图片，图文混排要自己排版，而 Slint 不给文本测量
      API——连同 34 个 Flutter 补丁的逐项评估记在
      [补丁评估](spikes/2026-11-22-flutter-patches-slint-eval.md) §4）
- [x] M0-SPIKE-04: iOS libmpv/AVPlayer feasibility and PiP/background-audio comparison
      （见 [spikes/SPIKE-004-ios.md](spikes/SPIKE-004-ios.md)）
- [x] M0-SPIKE-05: collect real baseline CPU/RSS/video frame-time samples
      （帧耗时分位数 / 掉帧 / CPU / RSS 随每次实验自动入记录，现已有 Windows、
      macOS、Android、iOS 四组真机样本；Linux 无真机，按
      [VALIDATION-CAPABILITY](VALIDATION-CAPABILITY.md) §3 降级为 best-effort。
      注意 RFC §8.1：跨机器的数不可比，要设门槛得固定同一台机器重采）
- [x] M0-SPIKE-06: author ADR-004 with measurements, not assumptions
      （[ADR-004](../architecture/adr/ADR-004-player-backend.md) 2026-11-24 批准；§6 补测见 #210）
