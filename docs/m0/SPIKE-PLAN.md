# M0 Slint + Playback Spike: executable acceptance plan

Status: **PLANNED, NOT RUN**  
Start reference: RFC-0001 v0.2

## Scope and order

1. **macOS first**: minimal Slint window with embedded libmpv video and true overlay danmaku; run audio-video DASH, pause, seek, fullscreen and window resize.
2. **Android second**: media surface layering, gestures, lifecycle/rotation, hardware acceleration, background audio, PiP feasibility.
3. **UI viability in parallel**: large lazy list scroll, rich comments with emoji/image/text, Chinese/English IME composition, focus and screen reader accessibility.
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

Phase 2 Slint migration is **NO-GO** if video compositing/overlay, mobile lifecycle, large lists, essential rich text, IME or accessibility cannot meet acceptable functional quality. Phase 1 Flutter + Rust Core remains a valid stable release.

## Initial backlog

- [ ] M0-SPIKE-01: macOS libmpv + Slint video + danmaku overlay
- [ ] M0-SPIKE-02: Android surface/lifecycle + libmpv versus Media3 fallback
- [ ] M0-SPIKE-03: Slint list virtualization, rich content, IME, accessibility
- [ ] M0-SPIKE-04: iOS libmpv/AVPlayer feasibility and PiP/background-audio comparison
- [ ] M0-SPIKE-05: collect real baseline CPU/RSS/video frame-time samples
- [ ] M0-SPIKE-06: author ADR-004 with measurements, not assumptions
