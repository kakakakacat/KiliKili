# Player Port 运行记录（#4）

日期：2026-11-24。环境：macOS 27.3（26C5047e）/ Mac16,1 / Apple M4，
Flutter 3.47.9、Rust 1.100.0、锁定 media-kit
`73771ec38176be2d984a3049c28177bce23b54a0`、桌面 libmpv 0.42.0。

## 真实后端与缓冲

`tool/player-port-runtime-test.sh` 通过真实 FRB、media-kit 与 libmpv 运行。
ffmpeg 生成 320×180 / 25fps / 15 秒 H.264 + AAC，以及 640×360 / 30 秒
H.264 + AAC；HTTP 资源支持 Range，请求头与独立音视频轨实际验证。
`vo=null`、`ao=null`、`hwdec=no`，不宣称可听声音或硬解通过。

3 个播放器用例通过（加上真实桥 6 项，共 9 项）：

- Open → 播放 → 暂停，250ms 后位置不变；
- 前跳 5000ms、回跳 1000ms，后端确认后误差小于 150ms；
- 2× / 0.5× 以实际属性回复确认；
- 重新 Open 的资源代际正确，位置回到新资源起点；
- 两路 DASH 资源收到预期 User-Agent/Referer，实际音视频轨非空，选轨获确认；
- 320→640 实际分辨率变化，Open(start: 5s) 后时钟从该位置继续；
- 显式开启 mpv 网络缓存，限制为 1MiB，播放推进后阻塞 HTTP 输出，确实进入
  Buffering；500ms 内位置变化小于 100ms，解除阻塞后恢复 Playing 并继续推进；
- 关闭后 Rust 会话数回到基线。

本轮还复现并修正了日志误判：低缓存 Seek 的可恢复 H.264 缺参考帧日志不能
直接触发致命失败。Rust 保留已知错误分类，未知日志交宿主记录，真正终止用
`END_FILE` 错误码判定。新增回归先失败、修正后通过，仍验证 -14 解码终止会失败。

## Flutter 画面与真实弹幕

`tool/player_port_probe/prepare.py` 生成独立包名的最小宿主，直接引用生产适配器、
FRB、`SimpleVideo` 与 canvas_danmaku；弹幕来自真实脱敏 `typical.raw.pb`，经 Rust
引擎与进度游标取桶。复跑方式见 [探针说明](../../tool/player_port_probe/README.md)。

macOS 窗口脚本通过：看到视频上覆盖「前方高能」「顶部弹幕」；暂停 1.4 秒
时位置与弹幕冻结；前跳 10 秒清屏，检查渲染器滚动/静态列表无旧弹幕；回跳、
2×/0.5×、320→640 切换、移除纹理后销毁与重建通过。最终 `PASS`，旧订阅回写
0 次，真实弹幕累计上屏 6 条，游标清屏 5 次。此为适配器与表面验证，
没有替代账号、心跳、完整视频详情页及产品 PiP 服务的回归。

## 同素材基线

`tool/player-port-benchmark.sh` 分别启动 raw media-kit / Player Port 的独立
Flutter test 进程，各 3 轮，交替顺序。同素材、软件解码、关闭音视频输出，
Rust 库在两种模式都初始化，播放窗口 8 秒，随后暂停 1.4 秒、Seek、2×。

| 项 | raw media-kit | Player Port |
|---|---:|---:|
| 8 秒位置/快照回调 | 每轮 200 | 每轮 200 |
| 回调率 | 约 24.99/s | 约 24.99/s |
| 平均 RSS（三轮均值） | 212.76 MiB | 218.89 MiB |
| 场景 wall time（三轮均值） | 9411.7ms | 9483.0ms |

Player Port 增加约 6.13MiB（2.9%）平均 RSS。Open 等实际确认约 43–44ms，
Seek 约 26–30ms；raw Future 只保证命令送达，因此约 71ms 的总差值包含确认
成本，不能据此判定后端变慢。该测量不含 UI 渲染，也不是生产负载或帧耗时结论。
这里只统计快照回调数，不把它当成全部 FFI 调用数。

## 原生适配器对照

新增 `tool/player-port-native-runtime-test.sh`，直接复用 Slint 原型的
`MpvPlayer`，与 Flutter 使用同样的媒体、暂停 1.4 秒、前后 Seek 目标及 150ms
误差断言、2×/0.5×、320→640 资源替换、5 秒恢复点、同样 1MiB 缓存/
16KiB 限速 Range 传输的阻塞/冻结/恢复，以及销毁。真实运行通过，
受控缓冲用例采纳 106 条事件、丢弃 4 条旧代际事件。原生后端暂停时不重复发未变化快照，
因此第二次暂停采样读取真实 mpv 属性，不等待一个本就不会出现的事件。

另运行原有 Slint 窗口脚本（同一 libmpv、320×180 素材、软解、fixture 密度 0）：
25.6 秒，暂停采样 4840→4840ms，Seek 目标 3000ms、实际 3000ms；
采纳 464、丢弃 1 条旧事件。截图可见视频/真实弹幕/HUD，脚本完成全屏和缩放。
平均 56.8fps，p95 **21.10ms，超过该 spike 的 20ms 门槛**；关闭硬解的本次
设置也不满足其硬解门槛。此轮不是原生性能 Go 的证据；两宿主的窗口大小与
渲染过程不同，不能拿各自 RSS 直接判优劣。

## 移动设备

Android：DN_ENG41 / Android 14 / arm64 / 640×960，独立 debug APK，
使用插件打包的 libmpv。与 macOS 相同的真实弹幕、Seek/倍速/分辨率脚本通过。
新增可见缓冲验证：断流期间 UI 为 Buffering，位置和弹幕停住 2 秒，恢复继续。
真实系统前后台、PiP 进入/播放/退出、纹理移除/销毁/重建通过，旧回写 0。
小窗截图已检查，视频正常显示；探针诊断控件在 PiP 隐藏，避免挤占视频。
本轮 Android 只改 Dart 验收步骤时，复用此前同 checkout 已验的 Rust 二进制，
最终包内 SHA-256 与原包一致：
`19297b9d2e1ad00a4ffff44e58d488f8bacabe3a2ed48da4640fb7f307a50c00`。

Android 驱动明确重启独立测试包，并按 PID/本轮启动时间过滤日志；FAIL 优先。
复核发现原脚本会读取同进程旧 PASS，已经修正并重新执行，不清除设备全局日志。

iOS：iPhone 16 Pro Max / iOS 27.3。独立包构建并安装到真机；使用 profile
避开无调试器时 debug FlutterEngine 的启动限制。复用产品 `PipPlugin.swift`。
首次安装遇到免费签名三应用上限，经用户明确许可卸载 Slint Spike 后解决；
两个既有正式用途应用保留。插件 framework 的漏签在生成包上单独补签，
不改产品签名流程。实际设备已通过真实弹幕、前后 Seek、暂停 1.4s、0.5×/2×、
320→640 和可见缓冲冻结/恢复的脚本断言；在前后台往返时被首启的无线数据
系统提示挡住，未收到 resumed，不能宣称该项/PiP 通过。等待用户处理设备提示。

构建一度因磁盘空间耗尽失败，清理本次可再生成中间产物后完成；没有运行会
reset 全局 Flutter SDK 的 prepare-build，也没有改 pub cache。完整复跑步骤在
探针 README，设备 JSON/截图保留在本地 build/，不把个人设备桌面截图上传仓库。

仍需补齐：完整产品页面与可听声音/硬解、iOS 剩余往返与产品 PiP 服务，以及两种
适配器在网络缓冲、移动生命周期、PiP 等**完整**矩阵上的一致性。#4 继续开着。

本轮完整本地检查：Rust workspace 975 通过、2 个既有忽略，fmt/clippy 通过；
真实 FRB+播放器 9 项通过，原生真实缓冲对照通过，针对性 Dart 分析无诊断。
原生新增测试的 rustfmt/clippy 通过；原生整个独立工程的 fmt 还会指出既有
`android_probe.rs` 的排版差异，本次没有顺手改该文件。
