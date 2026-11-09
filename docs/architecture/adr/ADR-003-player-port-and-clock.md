# ADR-003: Player Port, Ownership, and Danmaku Clock

Status: **Proposed — requires implementation review and runtime evidence**  
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

## Candidate interface (illustrative, not committed ABI)

```rust
pub struct PlaybackSnapshot {
    pub session_id: u64,
    pub generation: u64,
    pub position_ms: u64,
    pub duration_ms: Option<u64>,
    pub paused: bool,
    pub buffering: bool,
    pub speed: f32,
}

pub enum PlayerCommand {
    Open { session_id: u64, generation: u64 },
    Play,
    Pause,
    Seek { position_ms: u64, generation: u64 },
    SetSpeed(f32),
    Close,
}
```

The final interface must additionally model DASH video/audio streams, request headers, track selection, buffering/error events, stream cancellation, and surfaces; do not freeze the ABI before auditing the media-kit fork.

## Validation requirements

1. Seek forward/back while danmaku is active; no stale danmaku should appear after a generation switch.
2. Pause, resume, speed 0.5x–2x, network buffering and quality switch maintain correct clock and UI state.
3. Background/foreground and route disposal correctly cancel listeners; no old session overwrites new video.
4. Player event traffic is bounded through batching/coalescing; compare wall time and memory with baseline.
5. Verify identical behavior in a Flutter adapter and a prototype native adapter before adopting the latter.

## Consequences

The Rust Core receives a unified player state model but must accommodate platform-specific capabilities (PiP, background audio, hardware decoding and overlay). Platform-specific video rendering stays out of Domain. No claim is made that libmpv is production-ready on all platforms; ADR-004 will decide after tests.

## Fallback

Continue shipping Flutter + Rust Core with the current media-kit backend if native playback or Slint compositor tests fail.
