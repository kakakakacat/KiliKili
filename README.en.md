<div align="center">

# KiliKili

**A faster, more stable third-party Bilibili client**

Android · iPhone / iPad · macOS · Windows · Linux

[简体中文](README.md) · [English](README.en.md) · [Download](https://github.com/kakakakacat/KiliKili/releases) · [Feedback](https://github.com/kakakakacat/KiliKili/issues)

</div>

KiliKili is based on [PiliPlus](https://github.com/bggRGjQaUbCoE/PiliPlus). It keeps every feature and rebuilds the core in Rust to make the app faster and more stable.

<p align="center">
  <img src="assets/screenshots/kilikili-ios-home.png" height="400" alt="iOS home">
  <img src="assets/screenshots/kilikili-ios-video.png" height="400" alt="iOS video page">
  <img src="assets/screenshots/kilikili-android-home.png" height="400" alt="Android home">
  <img src="assets/screenshots/kilikili-android-video.png" height="400" alt="Android video page">
</p>

## How it differs from the official Bilibili app

- **The core logic is rewritten in Rust.** Fetching, stream selection, sorting and filtering are moving out of Dart into Rust one endpoint at a time ([#67](https://github.com/kakakakacat/KiliKili/issues/67)); the read-only endpoints are largely done. Native code, no GC pauses, and all five platforms run the same implementation instead of five separate ones. *We have not run a controlled comparison against the official app, so no percentages are claimed here; see [docs/qa](docs/qa) for how we measure.*

- **Liquid Glass UI, with the iOS-style droplet.** The selected-tab indicator in the bottom bar is a real drop of glass: it refracts and catches highlights, stretches like liquid when you switch tabs, overshoots and springs back, and swells while you hold a tab. It uses an Impeller shader and degrades gracefully when the device can't run it or "Reduce Motion" is on.

- **The "Mine" page was rebuilt.** Account info and the dynamic/following/follower counts merge into one card, with a single tool capsule on top (incognito mode, theme switch, search, messages) and shortcut tiles plus a favourites card below — adapted for narrow screens, tablets, light/dark themes and large text ([#262](https://github.com/kakakakacat/KiliKili/issues/262)).

- **Everything unnecessary is cut.** No ads, no splash screen, no shop or merch entries, no push SDK of any kind, and nothing that tries to keep you in the app. SponsorBlock is built in to skip sponsored segments, and anime openings and endings can be skipped too.

## Coming next

- **GPU frame interpolation**: smoother playback for low-frame-rate videos; you can turn it off anytime ([#211](https://github.com/kakakakacat/KiliKili/issues/211), [#219](https://github.com/kakakakacat/KiliKili/issues/219), [#220](https://github.com/kakakakacat/KiliKili/issues/220)).
- **Search inside a video**: jump straight to a moment using subtitles and danmaku ([#213](https://github.com/kakakakacat/KiliKili/issues/213)).
- **Smarter danmaku**: density control, automatic subtitle avoidance, spoiler-safe preview ([#214](https://github.com/kakakakacat/KiliKili/issues/214)).
- **Liquid Glass on more screens**: from the navigation bar to the whole control set, across five platforms with automatic fallback on low-end devices ([#224](https://github.com/kakakakacat/KiliKili/issues/224)–[#226](https://github.com/kakakakacat/KiliKili/issues/226)).
- **More flexible playback speed**: fine-grained steps, arbitrary values, and remembering your last choice ([#291](https://github.com/kakakakacat/KiliKili/issues/291)).
- **Animated avatars**: gif / ResAnimation avatars no longer flattened to a still image ([#292](https://github.com/kakakakacat/KiliKili/issues/292)).

Official builds are not out yet; watch [Releases](https://github.com/kakakakacat/KiliKili/releases).

## Long-term maintenance

We will keep updating KiliKili. If you run into a problem, tell us in [Issues](https://github.com/kakakakacat/KiliKili/issues).

Thanks to [PiliPlus](https://github.com/bggRGjQaUbCoE/PiliPlus), [PiliPalaX](https://github.com/orz12/PiliPalaX), [PiliPala](https://github.com/guozhigq/pilipala), and all contributors. KiliKili follows [GPL-3.0](LICENSE) and is an unofficial client.
