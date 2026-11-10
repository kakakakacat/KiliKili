# ADR-003: Player Port, Ownership, and Danmaku Clock

Status: **Contract implemented — adapter review and runtime evidence pending**
Date: 2026-11-10  
Related: RFC-0001 v0.2, ADR-004

## Context

Phase 1 keeps Flutter `media_kit` while moving business logic to Rust. Phase 2 proposes a native player integrated with Slint. If Dart and Rust both maintain play/pause/position independently, danmaku may drift and seek or buffering may produce inconsistent results.

## Proposed decision

- **Player backend is authoritative** for actual position, duration, pause/buffer state, playback speed, active tracks and seek completion.
- **Rust Player Orchestrator** owns playback intentions, quality selection, navigation-session generation, business progress persistence decisions, and failure recovery policy.
- **UI** only dispatches user intent and consumes derived playback snapshots. No second timer should be used as the authoritative video clock.
- Phase 1: `FlutterMediaKitAdapter` translates media-kit events into the same Player Port events. Phase 2: `NativeMpvAdapter` must implement the identical contract.
- Every asynchronous command and playback event includes `session_id` and monotonic `generation`/sequence; stale events after seek, video swap and widget destruction must be ignored.
- Danmaku scheduler uses backend-reported presentation position; seek resets scheduled items, pause freezes position, speed changes recompute trajectories.

## Contract in code

`rust/crates/bili-domain/src/player.rs` defines the backend-independent contract.
`PlayerPort::dispatch` receives `Stamped<PlayerCommand>`; a backend emits
`Stamped<PlaybackEvent>` through `PlayerEventSink`. The stamp contains
`SessionId` and monotonic `Generation`. Before opening another resource,
seeking, or destroying a player, the caller bumps its `GenerationGate` and
drops events that fail `Stamped::accept`. This is a Rust API, not a frozen FFI
ABI; adapters can map it to Dart streams or native channels.

`Open` carries either separate DASH video/audio URLs or ordered legacy
segments, plus `User-Agent` and `Referer`. The media-kit adapter may encode
those resources as `edl://`, while another backend may use its own track
composition. URLs and headers are redacted in `Debug` output. Commands also
cover play, pause, seek, speed, video/audio track selection, surface attach
and detach, foreground/background lifecycle, and close. The surface ID is
opaque: its native handle stays inside the platform adapter.

`PlayerSnapshot` contains the backend-reported position, duration, playback
state, speed and active tracks. The backend also emits explicit seek-complete,
surface-lost, closed and categorized failure events. Sending a seek command
does not imply that a seek completed. Buffering and pause do not advance
position in Rust. Position events should be coalesced before crossing FFI;
the adapter must not make one call per frame or per danmaku item.

## Validation requirements

1. Seek forward/back while danmaku is active; no stale danmaku should appear after a generation switch.
2. Pause, resume, speed 0.5x–2x, network buffering and quality switch maintain correct clock and UI state.
3. Background/foreground and route disposal correctly cancel listeners; no old session overwrites new video.
4. Player event traffic is bounded through batching/coalescing; compare wall time and memory with baseline.
5. Verify identical behavior in a Flutter adapter and a prototype native adapter before adopting the latter.

The pure contract tests in `rust/crates/bili-domain/tests/player_port_contract.rs`
cover resource/header representation, redaction and stale event filtering.
They do not satisfy the five runtime requirements above. The Flutter adapter
and native prototype must be measured on devices before this ADR is accepted
or the player backend is changed.

## Consequences

The Rust Core receives a unified player state model but must accommodate platform-specific capabilities (PiP, background audio, hardware decoding and overlay). Platform-specific video rendering stays out of Domain. No claim is made that libmpv is production-ready on all platforms; ADR-004 will decide after tests.

## Fallback

Continue shipping Flutter + Rust Core with the current media-kit backend if native playback or Slint compositor tests fail.
