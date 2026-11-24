<div align="center">

# KiliKili

**Five platforms · A third-party Bilibili client with its core rebuilt in Rust**

[简体中文](README.md) · [English](README.en.md) · [Download](https://github.com/kakakakacat/KiliKili/releases) · [Feedback](https://github.com/kakakakacat/KiliKili/issues)

</div>

KiliKili is forked from [PiliPlus](https://github.com/bggRGjQaUbCoE/PiliPlus) and inherits its full feature set and user experience.

## What we're doing

On top of PiliPlus, we are **rebuilding the data and logic layers in Rust** to improve performance and fix long-standing problems wherever we can. The UI layer stays as it is for now.

Later on, we plan to rewrite the UI layer with a Rust UI framework or each platform's native code, so every platform gets the best possible experience.

## Screenshots

| | Home (glass capsule navigation) | Video page |
| :---: | :---: | :---: |
| **iOS** | <img src="assets/screenshots/kilikili-ios-home.png" height="460" alt="iOS home feed with the glass capsule navigation bar"> | <img src="assets/screenshots/kilikili-ios-video.png" height="460" alt="iOS video page with danmaku and details"> |
| **Android** | <img src="assets/screenshots/kilikili-android-home.png" height="460" alt="Android home feed with the glass capsule navigation bar"> | <img src="assets/screenshots/kilikili-android-video.png" height="460" alt="Android video page with danmaku, subtitles and details"> |

<sub>Real-device screenshots of the current development build (Nov 2026): iPhone 16 Pro Max · Android test device.</sub>

## Coming next

### Cross-platform Liquid Glass

The home navigation bar is already a glass capsule (above); on Flutter it currently does capsule-shaped edge refraction and a highlight. Next, we are turning the material into a **shared glass engine written in Rust + wgpu/WGSL**, so all five platforms use the same material parameters and shaders instead of each platform's built-in glass widgets:

- real-time background blur, translucent material, edge refraction, Fresnel reflection and specular highlights;
- deformation on press and drag, and merging when components move close together;
- high / balanced / low quality tiers chosen by device capability, with automatic fallback on low-end devices so text always stays readable.

Track progress in [#130](https://github.com/kakakakacat/KiliKili/issues/130).

### Video frame interpolation

We are adding **GPU frame interpolation** for on-demand video, lifting 24/30 fps sources to higher frame rates for smoother playback on high-refresh displays:

- options **Off / 45 FPS / 60 FPS / Match display refresh rate**, off by default;
- validated first on Android and iOS at 720p, 30 → 60 fps, then extended to Windows, macOS and Linux;
- danmaku and controls are composited after interpolation so they stay sharp; audio and video stay in sync after seeking, speed changes and quality switches; it falls back to the source frame rate on scene cuts or when the device gets hot.

Track progress in [#211](https://github.com/kakakakacat/KiliKili/issues/211).

> Both are still in the research stage; final quality and supported devices depend on real-device testing.

## Platforms

| Platform | Status |
| :--- | :---: |
| Android | ✅ |
| iPhone / iPad | ✅ |
| macOS | ✅ |
| Windows | ✅ |
| Linux | ✅ |

Platform support comes from PiliPlus. Official KiliKili builds are not yet available; watch [Releases](https://github.com/kakakakacat/KiliKili/releases).

## Project activity

These charts are generated from the `main` branch and update after every push.

![Recent commits](assets/stats/commits.svg)

![Source-code changes](assets/stats/code-changes.svg)

<sub>Statistics start at the fork baseline. Line counts include source code only, excluding generated files and upstream syncs. GitHub Actions keeps the charts updated.</sub>

## Long-term maintenance

**We commit to updating KiliKili for the long term.** We will keep following upstream fixes, API changes and platform compatibility issues. If something feels off or you hit a bug, tell us in [Issues](https://github.com/kakakakacat/KiliKili/issues).

> Want to contribute? Working conventions are in [AGENTS.md](AGENTS.md); design documents live in [`docs/`](docs/).

Thanks to [PiliPlus](https://github.com/bggRGjQaUbCoE/PiliPlus), [PiliPalaX](https://github.com/orz12/PiliPalaX), [PiliPala](https://github.com/guozhigq/pilipala), and all contributors. KiliKili follows [GPL-3.0](LICENSE) and is an independently maintained, unofficial client.
