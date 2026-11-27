<div align="center">
    <h1>KiliKili</h1>

[中文](README.md) | English

<p>A third-party BiliBili client with its core rewritten in Rust</p>

<img src="assets/screenshots/kilikili-android-home.png" width="24%" alt="Android home" />
<img src="assets/screenshots/kilikili-android-video.png" width="24%" alt="Android video" />
<img src="assets/screenshots/kilikili-ios-home.png" width="24%" alt="iOS home" />
<img src="assets/screenshots/kilikili-ios-video.png" width="24%" alt="iOS video" />

</div>

<br/>

## Platforms

- [x] Android
- [x] iOS / iPad
- [x] macOS
- [x] Windows
- [x] Linux

## Highlights

| | |
| --- | --- |
| **Core in Rust** | Fetching, stream selection, danmaku and rate-limit backoff all live in Rust. Native code, no GC pauses, one implementation for all five platforms |
| **GPU frame interpolation** | Motion compensation, not frame blending. 720p 30→60 holds a steady 60 fps on an Adreno 610; one GLSL source for five platforms (ships with the Slint client) |
| **Liquid Glass droplet** | The selected-tab indicator is refracting glass — it stretches, overshoots and springs back, and swells when held |
| **Rebuilt "Mine" page** | Account card + a single tool capsule + uniform rounded shortcuts |
| **Nothing extra** | No ads, no splash screen, no shop, no push SDK; SponsorBlock built in, anime openings and endings skippable |

## Rust migration

| crate | What it covers | Status |
| --- | --- | --- |
| `bili-api` / `bili-grpc` / `bili-protocol` | Requests, WBI signing, protobuf, risk-control parameters | Read-only endpoints largely done ([#67](https://github.com/kakakakacat/KiliKili/issues/67)) |
| `bili-core::media_select` | Quality fallback chains, CDN routing | Done |
| `bili-danmaku` | Parsing, regex block rules, playback cursor | Done |
| `player-interp` | GPU interpolation engine (block matching + bidirectional compensation) | Engine done, pending Slint integration |
| `bili-core::glass_droplet` | Spring physics for the droplet | Done |

> No controlled comparison against the official app has been run, so no performance figures are claimed. Interpolation measurements: [`docs/m0/spikes/2026-11-24-interp-218.md`](docs/m0/spikes/2026-11-24-interp-218.md).

## Coming next

| Feature | Issue |
| --- | --- |
| GPU frame interpolation on five platforms | [#211](https://github.com/kakakakacat/KiliKili/issues/211) [#219](https://github.com/kakakakacat/KiliKili/issues/219) [#220](https://github.com/kakakakacat/KiliKili/issues/220) |
| Search inside a video (subtitles and danmaku) | [#213](https://github.com/kakakakacat/KiliKili/issues/213) |
| Smarter danmaku (density, subtitle avoidance, spoiler-safe) | [#214](https://github.com/kakakakacat/KiliKili/issues/214) |
| Liquid Glass across the whole control set | [#224](https://github.com/kakakakacat/KiliKili/issues/224)–[#226](https://github.com/kakakakacat/KiliKili/issues/226) |
| More flexible playback speed | [#291](https://github.com/kakakakacat/KiliKili/issues/291) |
| Animated avatars | [#292](https://github.com/kakakakacat/KiliKili/issues/292) |

## Download

[Releases](https://github.com/kakakakacat/KiliKili/releases) (no official builds yet) · [Feedback](https://github.com/kakakakacat/KiliKili/issues)

## Thanks

Based on [PiliPlus](https://github.com/bggRGjQaUbCoE/PiliPlus); thanks to [PiliPalaX](https://github.com/orz12/PiliPalaX), [PiliPala](https://github.com/guozhigq/pilipala) and all contributors. Unofficial client, released under [GPL-3.0](LICENSE).
