# ADR-003: Player Port, Ownership, and Danmaku Clock

Status: **Contract implemented — native-adapter runtime evidence collected on Windows and Android (2026-11-21); macOS pending**
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

`rust/crates/bili-core/src/player_session.rs` applies this rule in the
application layer: open, seek, track changes and close invalidate older
events. It stores only the last backend snapshot and does not derive position
from wall time or assume that a command succeeded.

## Validation requirements

1. Seek forward/back while danmaku is active; no stale danmaku should appear after a generation switch.
2. Pause, resume, speed 0.5x–2x, network buffering and quality switch maintain correct clock and UI state.
3. Background/foreground and route disposal correctly cancel listeners; no old session overwrites new video.
4. Player event traffic is bounded through batching/coalescing; compare wall time and memory with baseline.
5. Verify identical behavior in a Flutter adapter and a prototype native adapter before adopting the latter.

The pure contract tests in `rust/crates/bili-domain/tests/player_port_contract.rs`
and `rust/crates/bili-core/tests/player_session.rs` cover resource/header
representation, redaction, stale event filtering and backend clock ownership.
They do not satisfy the five runtime requirements above. The Flutter adapter
and native prototype must be measured on devices before this ADR is accepted
or the player backend is changed.

## Consequences

The Rust Core receives a unified player state model but must accommodate platform-specific capabilities (PiP, background audio, hardware decoding and overlay). Platform-specific video rendering stays out of Domain. No claim is made that libmpv is production-ready on all platforms; ADR-004 will decide after tests.

## Fallback

Continue shipping Flutter + Rust Core with the current media-kit backend if native playback or Slint compositor tests fail.

## 原生适配器的首批实测（2026-11-21）

`spikes/slint-mpv/src/mpv/mod.rs` 是 `NativeMpvAdapter` 的原型：libmpv 包成
同一个 `PlayerPort`，事件经 `PlayerEventSink` 过 channel 回 UI 线程。
在 Windows 上按脚本跑了一次（记录见
[docs/m0/spikes/2026-11-21-windows-dryrun.md](../../m0/spikes/2026-11-21-windows-dryrun.md)），
上面「Validation requirements」的第 1、2、4 条拿到了第一批真实数据：

- **暂停不推进位置**：暂停 1.4 秒后两次采样都是 4866ms。
- **Seek 完成是事实事件**：收到一次 `SeekCompleted`，位置与目标相差 0ms；
  seek 期间有 2 条旧代际事件被 `Stamped::accept` 丢掉，没有进 UI。
- **事件被合并而非逐帧过桥**：`time-pos` 在适配器里合并成最多每 40ms 一个
  快照，25.7 秒的运行共 458 条事件（渲染了 1506 帧）。

实现上有一处值得写进 ADR：**代际不能在事件泵里用「当前值」盖章**。
调用方 bump 闸门的那一刻，旧事件还在 mpv 的队列里；用当前代际盖章会把它们
一并盖成新代际，闸门就白做了。原型的做法是拿 mpv 自己的确认点分段——
发出 Seek/Open 后事件泵**继续用旧代际盖章**，直到 `PLAYBACK_RESTART`
（seek）或 `FILE_LOADED`（open）到达才切到新代际并发出 `SeekCompleted`。
「发了 Seek 不等于 seek 完成了」这句话在实现里就是这段状态机，任何后端的
适配器都要给出等价的确认点。

另外两条给未来适配器的硬约束，是实测撞出来的，不是推演：

- **命令不能用同步 API 从渲染线程发出。** `mpv_command` 等核心处理完，
  核心又会等渲染线程画完一帧；UI 线程同时承担两者时必然有互等的时机，
  表现为整个窗口冻死。原型改用 `mpv_command_async` /
  `mpv_set_property_async`。注意这和「命令必须是异步的」在契约层已经一致：
  `PlayerPort::dispatch` 返回的本来就只是「已受理」，不是「已完成」。
- **渲染表面必须先于 `Open` 就绪。** 渲染上下文还没建好就 `loadfile`，
  mpv 会照常解码、走进度、出声，但画面永远是黑的，且不报任何错误。
  契约里的 `AttachSurface` 因此不是可选步骤：适配器要么等表面就绪再接受
  `Open`，要么在表面就绪时自行重开。

## 第 3 条（前后台与表面销毁）：Android 真机实测，2026-11-21

SPIKE-002 第二轮在小米 13 Ultra / Android 14 上把这条验完了，记录与证据在
[SPIKE-002-android](../../m0/spikes/SPIKE-002-android.md)。契约本身不用改，
但多了三条**对适配器的硬约束**，都是实测撞出来的：

- **`SetLifecycle(Background)` 必须赶在表面销毁之前生效。** Android 上
  `Pause` 比 `TerminateWindow` 早到，而 `PlayerPort::dispatch` 是异步受理，
  下发不等于落地。原型的做法是在销毁渲染上下文之前确认 `vid` 已经变成 `no`。
  不这么做的后果不是「画面多画了几帧」：render context 一没，mpv 的 VO 跟着
  死，**整个文件会被卸掉**（`EOF code: 6`，连音频一起停）——在没有音频轨的
  素材上百分之百复现。
- **`Pause` 不等于「进了后台」。** 进画中画同样走 `Pause`。适配器要能区分，
  否则一进小窗就把视频轨自己关掉了。契约这边的含义是：`Lifecycle` 的切换点
  由平台适配器判断，领域层给的只是两种状态。
- **表面重建之后，零拷贝硬解这条路要靠一次完整重载才接得回来。** Android +
  `hwdec=mediacodec` 下，新的渲染上下文建好、`vid` 重选、`hwdec` 重写都不出
  画面（软解则没事），只有重新 `Open` 才行。所以 `AttachSurface` 在这类后端上
  不只是「接一个新表面」，而是**要带着「记住位置 → 重开 → seek 回去」这套
  恢复动作**。这条与上面「渲染表面必须先于 `Open` 就绪」是同一件事的两面。

第 5 条（两种适配器行为一致）仍未验：要等 Flutter 侧也跑同一组脚本。
