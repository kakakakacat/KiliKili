<div align="center">

# KiliKili

### A third-party Bilibili client with its core moved into Rust

Android · iPhone / iPad · macOS · Windows · Linux

[Download](https://github.com/kakakakacat/KiliKili/releases) · [简体中文](README.md) · [Feedback](https://github.com/kakakakacat/KiliKili/issues)

</div>

KiliKili is not a reskinned third-party client. **The parts that actually cost performance — fetching, stream selection, danmaku, frame interpolation — have moved out of Dart and into Rust**, so replacing the UI layer later won't mean writing all of it again.

---

## What makes it different

### 1. The core is rewritten in Rust

Not "calls a bit of Rust" — whole paths move across, one endpoint per slice ([#67](https://github.com/kakakakacat/KiliKili/issues/67)), and **the read-only endpoints are largely done**:

| Rust crate | What moved in |
| --- | --- |
| `bili-api` / `bili-grpc` / `bili-protocol` | Requests, WBI signing, protobuf parsing, risk-control parameters |
| `bili-core::media_select` | Candidate order and fallback chains for video/audio quality, CDN routing |
| `bili-danmaku` | Danmaku parsing, regex block rules, playback cursor |
| `player-interp` | The GPU frame-interpolation engine (below) |
| `bili-core::glass_droplet` | Spring physics for the navigation-bar droplet |

What that buys: **native code, no GC pauses**, and one implementation shared by all five platforms instead of five that drift apart. Even "how long to back off and how many times to retry when search hits rate limiting" lives in Rust ([#304](https://github.com/kakakakacat/KiliKili/issues/304)) rather than scattered through UI code.

> We have not run a controlled comparison against the official app, so no "x% faster" is claimed here. How we measure: [`docs/qa`](docs/qa).

### 2. GPU frame interpolation

Brings 24/30 fps sources up to 60 fps before they reach the screen. Not frame blending — **motion compensation**: block matching at low resolution to estimate the motion field, then bidirectional compensation against full-resolution textures.

Measured on real devices ([#218](https://github.com/kakakakacat/KiliKili/issues/218) spike, written up in [`docs/m0/spikes/2026-11-24-interp-218.md`](docs/m0/spikes/2026-11-24-interp-218.md)):

- **Low-end Android keeps up**: on an SM6225 / Adreno 610, 720p 30→60 interpolation holds a **steady 60 fps** with the same display intervals as the non-interpolated path, and no thermal throttling across 15 continuous minutes. 24→60 holds too.
- **~3 ms for motion estimation on an iPhone 16 Pro Max.**
- **One GLSL source for five platforms** (GL 3.3 / GLES 3.0), fragment shaders only — no compute shader, no separate Metal implementation.
- **Duplicate frames are detected**: anime is often 23.976 fps with 2:1 and 3:1 pulldown, where neighbouring frames are identical; those aren't mistaken for a still image and interpolated.
- Settings are Off / 45 FPS / 60 FPS / match display refresh rate, **off by default**; it steps down automatically on sustained frame drops or thermal throttling, and back up once things settle.

The engine lives in [`rust/crates/player-interp`](rust/crates/player-interp), with unit tests over the control plane. **It ships with the next-generation Slint client** ([#219](https://github.com/kakakakacat/KiliKili/issues/219) mobile, [#220](https://github.com/kakakakacat/KiliKili/issues/220) desktop); the current Flutter build does not have it yet.

### 3. Liquid Glass droplet

The selected-tab indicator in the bottom bar is a real drop of glass: it refracts and catches highlights, stretches like liquid when you switch tabs, overshoots and springs back, and swells while you hold a tab. It uses an Impeller shader and degrades gracefully when the device can't run it or "Reduce Motion" is on ([#266](https://github.com/kakakakacat/KiliKili/issues/266)).

### 4. A rebuilt "Mine" page

Account info and the dynamic/following/follower counts merge into one card, with a single tool capsule on top (incognito mode, theme switch, search, messages) and shortcut tiles plus a favourites card below — adapted for narrow screens, tablets, light/dark themes and large text ([#262](https://github.com/kakakakacat/KiliKili/issues/262)).

### 5. Everything unnecessary is cut

No ads, no splash screen, no shop or merch entries, and **no push SDK of any kind**. SponsorBlock is built in to skip sponsored segments, and anime openings and endings can be skipped too.

---

## Screenshots

| Android home | Android video |
| :---: | :---: |
| <img src="assets/screenshots/kilikili-android-home.png" width="260" alt="KiliKili Android home"> | <img src="assets/screenshots/kilikili-android-video.png" width="260" alt="KiliKili Android video page"> |

| iPhone home | iPhone video |
| :---: | :---: |
| <img src="assets/screenshots/kilikili-ios-home.png" width="260" alt="KiliKili iPhone home"> | <img src="assets/screenshots/kilikili-ios-video.png" width="260" alt="KiliKili iPhone video page"> |

## Coming next

- **Search inside a video**: jump straight to a moment using subtitles and danmaku ([#213](https://github.com/kakakakacat/KiliKili/issues/213)).
- **Smarter danmaku**: density control, automatic subtitle avoidance, spoiler-safe preview ([#214](https://github.com/kakakakacat/KiliKili/issues/214)).
- **Liquid Glass on more screens**: from the navigation bar to the whole control set, across five platforms with automatic fallback on low-end devices ([#224](https://github.com/kakakakacat/KiliKili/issues/224)–[#226](https://github.com/kakakakacat/KiliKili/issues/226)).
- **More flexible playback speed**: fine-grained steps, arbitrary values, and remembering your last choice ([#291](https://github.com/kakakakacat/KiliKili/issues/291)).
- **Animated avatars**: gif / ResAnimation avatars no longer flattened to a still image ([#292](https://github.com/kakakakacat/KiliKili/issues/292)).

## Download

Official builds are not out yet; watch [Releases](https://github.com/kakakakacat/KiliKili/releases). Problems and suggestions are welcome in [Issues](https://github.com/kakakakacat/KiliKili/issues).

---

KiliKili is based on [PiliPlus](https://github.com/bggRGjQaUbCoE/PiliPlus); thanks to [PiliPalaX](https://github.com/orz12/PiliPalaX), [PiliPala](https://github.com/guozhigq/pilipala) and all contributors. It is an unofficial client, not affiliated with Bilibili, released under [GPL-3.0](LICENSE).
