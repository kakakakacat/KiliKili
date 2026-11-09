# M0 Source Baseline — KiliKili

Status: **SOURCE AUDIT IN PROGRESS** (not a build verification)  
Audit date: 2026-11-10  
Repository: https://github.com/kakakakacat/KiliKili  
Reviewed tree: main, after RFC v0.2 commit `8753b3dddb7a86cf9fd95c7ab629a54fe4b1e2bf` (recorded as audit starting reference; refresh commit before reproducible build).  
Upstream: https://github.com/bggRGjQaUbCoE/PiliPlus

## Verified source observations

| Area | Evidence | Implication |
|---|---|---|
| SDK | `pubspec.yaml`: Flutter 3.47.9, Dart >=3.13.0 | Match SDK precisely during baseline build |
| Video | `media_kit` 1.1.11; `media_kit_video` 1.2.5; platform-specific Git overrides | Audit actual fork behavior before replacing Player Adapter |
| State | `hive_ce` in `lib/utils/storage.dart` | Dart exporter required before Rust SQLite import |
| Storage boxes | `userInfo`, `localCache`, `setting`, `historyWord`, `video`, `watchProgress`, optional `reply`, plus `account` in `lib/utils/accounts.dart` | Separate sensitive account credentials from ordinary settings |
| Settings | `GStorage.exportAllSettings()` exports only `setting` and `video` | Existing export is **not** a full-data migration |
| HTTP | `lib/http/init.dart`: Dio, account manager, HTTP/2 and cookie integration | Protocol/auth boundary must be audited before Rust migration |
| Downloads | `lib/services/download/download_service.dart` and `download_manager.dart`; metadata `entry.json` / `index.json` | Preserve existing downloads; do not assume FFmpeg mux already exists |
| Platforms | `.github/workflows/build.yml` has Android/iOS/macOS/Windows/Linux jobs | Source supports build workflows; actual builds **not yet executed** |

### Pinned Git dependency SHAs (from pubspec.lock)

| Dependency | Source ref | Resolved SHA |
|---|---|---|
| media_kit + Android/iOS/Windows video libraries | `My-Responsitories/media-kit@native` | `73771ec38176be2d984a3049c28177bce23b54a0` |
| media_kit_video | `bggRGjQaUbCoE/media-kit@native` | `85f0a69ea66b100882b27a3375c72e5155375343` |
| audio_service | `bggRGjQaUbCoE/audio_service@dev` | `9f91eb8d6aa0905661ab3914ebdd3c9fb5c37cc2` |
| canvas_danmaku | `bggRGjQaUbCoE/canvas_danmaku@main` | `435063d093bdc0dde70616a2c30552411c9e13e0` |

Note: `pubspec.yaml` references branches; `pubspec.lock` provides the concrete resolved revision. Do not casually run dependency upgrades before benchmark comparisons.

## Open baseline gates

- [ ] Record exact upstream/base SHA and current fork HEAD using a local Git clone.
- [ ] Build and launch unmodified app on macOS; retain build logs and runtime screenshots.
- [ ] Build Android and iOS artifacts in CI; test on real devices when available.
- [ ] Inspect the actual PlayerController and media_kit fork code; document event timestamps/seek/buffer semantics.
- [ ] Enumerate custom Hive TypeAdapters and account-sensitive fields, with only **redacted** test fixtures.
- [ ] Verify existing download formats, reassembly and background execution behavior.
- [ ] Measure baseline app start, playback, CPU, RSS, and danmaku metrics.

## Environment limitation

This audit used GitHub repository file reads. The execution environment cannot reach github.com with local Git, and Rust/Flutter toolchains are not installed here. No compilation, playback, real-device testing, Slint integration or performance benchmark is claimed.

## Initial decisions

1. Keep the Phase-1 Flutter Player Adapter; do not rewrite the native media backend before ADR-003/004.
2. Make actual player position the only clock of truth; Rust will own command orchestration and derived snapshots, not an independent timeline.
3. Use Dart-side Hive export followed by Rust SQLite import, preserving original data until verified.
4. M0 Slint Spike is a Phase-2 Go/No-Go gate; Phase-1 Flutter + Rust Core remains independently shippable.
