# Player Port 无窗口运行记录（#4）

日期：2026-11-24。环境：macOS 27.3（26C5047e）/ Mac16,1 / Apple M4，
Flutter 3.47.9、锁定 media-kit
`73771ec38176be2d984a3049c28177bce23b54a0`、本机 libmpv 0.42.0。
Rust 使用仓库 `rust/rust-toolchain.toml` 的 1.100.0。

执行 `tool/player-port-runtime-test.sh`。本地 ffmpeg 生成 320×180 / 25fps、
15 秒 H.264 + AAC 测试媒体，拆成视频与音频两个 HTTP 资源；`vo=null`、
`ao=null`、`hwdec=no`。因此没有画面、可听声音或硬解通过的结论。

真实 media-kit/libmpv 的两个用例通过：

- Open → 播放 → 暂停，间隔 250ms 的位置采样不推进；
- Seek 到 5000ms，再回 1000ms，收到后端确认，位置误差小于 150ms；
- 倍速 2× 与 0.5× 均以实际属性回复确认；
- 重新 Open 后资源代际变化，位置回到新资源起点；
- 两路 DASH 资源均收到预期 User-Agent/Referer，实际视频/音频轨非空；
- SelectTracks 由后端轨道属性确认；
- 销毁后 Rust 会话数回到测试前基线。

首次无窗口时序运行约 1 秒产生 22 个快照批次；这是快速自动化步骤中的采样数，
不是稳定播放性能基准。完整脚本含库清理等待，两个真实播放器用例约 15 秒结束。
真实 FRB 的确定性回归另覆盖旧采样、旧确认、关闭时在途 Open、确认超时永久
停用、网络失败允许重建以及新资源接管初始化。

未执行：Flutter 生产界面、真实弹幕、网络缓冲、实际清晰度切换、移动端生命周期、
PiP、视频表面、baseline wall time/RSS 对照，以及与原生适配器逐项同场景比较。
五项验收的边界以 ADR-003 本轮矩阵为准；本记录不宣称 #4 可以关单。

完整本地检查：Rust fmt/clippy 通过，workspace 974 通过、2 个既有忽略；
真实 FRB 6 项与真实播放器 2 项通过，相邻 Dart 弹幕/会话 6 项通过。
`dart analyze lib` 无 error，改动文件未新增分析诊断；既有 warning/info 保留。
