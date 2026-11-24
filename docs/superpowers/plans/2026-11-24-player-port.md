# Player Port 实施计划

> 执行方式：当前会话按 superpowers:executing-plans 逐项实施；测试驱动。

**目标：** 为 #4 接入 Flutter Player Port，复用 Rust 会话并留下可复跑证据。
**架构：** Rust 状态机与 typed FRB；Dart media-kit 命令执行和独立 mpv 事件 client。
**技术栈：** Rust、FRB 2.13.0、Dart/Flutter、media-kit/libmpv。

- [x] 1. 在 `rust/crates/bili-core/tests/media_kit_port.rs` 先写时序失败测试，
  `cargo test -p bili-core --test media_kit_port` 确认缺实现；在
  `src/media_kit_port.rs` 实现开始/确认/快照合并，复用 PlayerSession。
  覆盖旧确认、连发 seek、未开始的命令、关闭、错误及资源编码。
- [x] 2. 在 `bridge_flutter/src/api/player_port.rs` 增加 typed 命令、原生观察批次、
  快照与失败结果，以及显式 create/close；导出不含 Dart 或 mpv 指针。
  在 `test/bridge/player_port_bridge_test.dart` 跨真实动态库验证完整命令/事件时序。
  修改导出后运行 `tool/frb-generate.sh`，检查生成物与 init 文档块。
- [x] 3. 新建 `lib/plugin/pl_player/player_port/`：独立 `mpv_observer.dart`
  非阻塞轮询自有 client；`flutter_media_kit_adapter.dart` 串行执行已盖章命令，
  每 40ms 一批事实进入 Rust。先写可控观察传输的 Dart 测试，验证关闭、超时、
  过期延续与确认前无快照；再接入真实 media-kit 调用。
- [x] 4. 修改 `lib/plugin/pl_player/controller.dart`，移除 EDL 和日志分类旧实现，
  替换主要状态监听与控制方法。检查 `lib/pages/danmaku/view.dart` 的 seekEpoch
  消费与初始化竞态，给平台生命周期调用点增加端口通知。
- [x] 5. 添加可选真实 libmpv 回归（本地生成视频，不依赖线上账号/CDN），跑
  open/seek/pause/speed/replace/close，记录证据和未验项。
- [x] 6. 更新 `docs/architecture/adr/ADR-003-player-port-and-clock.md`、
  `docs/m0/VALIDATION-CAPABILITY.md`、`docs/qa/RELEASE-VALIDATION.md` 和
  `docs/architecture/upstream-divergence.md`；不关闭 #4。
- [x] 7. 运行 `cargo fmt --check`、`cargo clippy --workspace --all-targets`、
  `cargo test --workspace`，Dart analyze 求 error 文件与改动文件交集，运行不触及
  Pref/Widget 的测试与真实桥测试。复核 diff 与生成代码后提交；不合并。


执行结果：Rust workspace 974 通过、2 个既有忽略，fmt/clippy 通过；真实 FFI
6 项与真实 media-kit/libmpv 2 项通过；相邻 Dart 会话/弹幕 6 项通过。
`dart analyze lib` 无 error，既有 warning/info 保留。真机五项验收矩阵缺项
已写入 ADR-003 与运行记录，因此本 PR 不关闭 #4。


继续验收（用户要求当场推进）：

- [x] 真实 HTTP 缓冲与不同分辨率切换；发现并修复可恢复 error 日志误判。
- [x] 三轮 raw media-kit / Player Port 独立进程基线，保留回调/wall time/RSS。
- [x] 独立 Flutter 图形宿主：复用生产适配器、纹理、Rust/Canvas 真实弹幕。
- [x] Android 真机前后台、PiP、缓冲、销毁重建及画面检查。
- [x] 原生 MpvPlayer 同素材、同 Seek/倍速/分辨率/受控缓冲契约对照。
- [x] iOS profile 真机完整重跑：播放/弹幕/缓冲、前后台/PiP、销毁重建通过；复核小窗及 PASS 画面，旧回写 0。
- [x] 增量复核：修复 Android 驱动重复读取旧 PASS 的运行边界问题。
