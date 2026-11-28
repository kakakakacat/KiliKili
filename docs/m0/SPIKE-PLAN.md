# M0 Slint + Playback Spike: executable acceptance plan

Status: **HARNESS READY; macOS RUN PENDING**（2026-11-21 起）
Start reference: RFC-0001 v0.2

实验台在 `spikes/slint-mpv`（自成 workspace，`rust.yml` 不碰它），用法见它的
README。跑出来的记录放 [spikes/](spikes/README.md)，已有一份 Windows 自检记录；
macOS 那条（issue #8）仍然 **NOT TESTED**，因为写它的那台开发机不是 mac，
而这项验证按 [VALIDATION-CAPABILITY](VALIDATION-CAPABILITY.md) §1 只能在本机 M4 上做。

Windows 那一跑不改变下面的顺序（macOS 仍然先行），它回答的是「实验台本身能不能
用」。顺带暴露的三个与平台无关的坑见 [spikes/README.md](spikes/README.md)。

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

- [~] M0-SPIKE-01: macOS libmpv + Slint video + danmaku overlay
      （实验台与记录位就绪；macOS 实跑未做，见 [spikes/SPIKE-001-macos.md](spikes/SPIKE-001-macos.md)）
- [ ] M0-SPIKE-02: Android surface/lifecycle + libmpv versus Media3 fallback
- [ ] M0-SPIKE-03: Slint list virtualization, rich content, IME
- [x] M0-SPIKE-04: iOS libmpv/AVPlayer feasibility and PiP/background-audio comparison
      （见 [spikes/SPIKE-004-ios.md](spikes/SPIKE-004-ios.md)）
- [~] M0-SPIKE-05: collect real baseline CPU/RSS/video frame-time samples
      （采集手段就绪：帧耗时分位数 / 掉帧 / CPU / RSS 随每次实验自动入记录；
      目前只有 Windows 一组样本）
- [x] M0-SPIKE-06: author ADR-004 with measurements, not assumptions
      （[ADR-004](../architecture/adr/ADR-004-player-backend.md) 2026-11-24 批准；§6 补测见 #210）
