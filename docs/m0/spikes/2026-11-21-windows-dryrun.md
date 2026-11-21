# SPIKE-001 实验记录 · windows · 2026-11-21 04:37:20 UTC

自动产出于 `slint-mpv-spike`，不要手工编辑：重跑一次会整份覆盖。

命令行：`E:\KiliKili\spikes\slint-mpv\target\release\slint-mpv-spike.exe --video ~/AppData/Local/Temp/claude/1d689316250c8d51/5c5cc440-5d51-45a8-929f-50b663161082/scratchpad/media/Big_Buck_Bunny_720_10s_5MB.mp4 --audio ~/AppData/Local/Temp/claude/1d689316250c8d51/5c5cc440-5d51-45a8-929f-50b663161082/scratchpad/media/Example.ogg --loop-file --flip-y --seek-to 5000 --danmaku-rate 30 --scripted --mpv-lib ~/AppData/Local/Temp/claude/1d689316250c8d51/5c5cc440-5d51-45a8-929f-50b663161082/scratchpad/libmpv/libmpv-2.dll --mpv-log ~/AppData/Local/Temp/claude/1d689316250c8d51/5c5cc440-5d51-45a8-929f-50b663161082/scratchpad/run-doc/mpv.log --shots ~/AppData/Local/Temp/claude/1d689316250c8d51/5c5cc440-5d51-45a8-929f-50b663161082/scratchpad/run-doc/shots --record E:/KiliKili/docs/m0/spikes/2026-11-21-windows-dryrun.md`

**自动项结论：PASS**（人工项见下表）

## 环境

| 项 | 值 |
|---|---|
| 平台 | windows |
| 系统 | Windows 11 (26200) |
| 内核 | 26200 |
| CPU | AMD Ryzen 5 3600 6-Core Processor |
| 内存 | 31.9 GiB |
| rustc | <取不到：rustc> |
| commit | 191af5f15 |
| 工作区是否干净 | 否（记录里的 commit 不足以复现，见备注） |
| slint | 1.19.0 |
| libmpv | ~/AppData/Local/Temp/claude/1d689316250c8d51/5c5cc440-5d51-45a8-929f-50b663161082/scratchpad/libmpv/libmpv-2.dll |
| mpv client API | 2.5 |

## 媒体与渲染

| 项 | 值 |
|---|---|
| 模式 | 视频 + 弹幕 |
| 视频源 | ~/AppData/Local/Temp/claude/1d689316250c8d51/5c5cc440-5d51-45a8-929f-50b663161082/scratchpad/media/Big_Buck_Bunny_720_10s_5MB.mp4 |
| 音频源 | DASH 外挂音轨 |
| hwdec 设置 | auto-safe |
| OpenGL | ATI Technologies Inc. \| AMD Radeon RX 7900 GRE \| OpenGL ES 3.2.0 26.9.1.260826  |
| FBO 尺寸 | 1920x1080 |
| 弹幕 | 形状取自 typical + rich · 密度 30 条/秒（合成）· 入桶 18182 条 |
| hwdec 实际 | d3d11va-copy |
| 视频格式 | h264 |
| 音频编码 | vorbis |
| 视频分辨率 | 1280x720 |

## 测量

| 项 | 值 |
|---|---|
| mpv 丢帧 | 2 |
| 视频帧率 | 30.00 |
| 运行时长 | 25.7s |
| 总帧数 | 1506 |
| 平均 fps | 58.6 |
| 帧耗时 p50 / p95 / p99 | 16.17 / 16.99 / 21.75 ms |
| 掉帧（>33ms） | 5 |
| CPU / RSS（结束时） | 43.8% / 244.0 MiB（峰值 281.9 MiB） |
| 弹幕 累计 / 丢弃 | 449 条上屏，0 条因同屏上限丢弃，6 条因无空轨丢弃，226 条高级弹幕跳过 |
| 事件 | 采纳 458，因代际过期丢弃 2 |

## 验证项

| 项 | 预期 | 实际 | 结论 |
|---|---|---|---|
| 帧率 | 平均 ≥ 55 fps | 58.6 fps | PASS |
| 帧耗时 p95 | ≤ 20ms | 16.99 ms | PASS |
| 暂停冻结位置 | 暂停 1.4 秒后位置不变 | 4866ms → 4866ms | PASS |
| Seek 完成是事实事件 | 收到 SeekCompleted 且位置接近 5000ms | 收到 1 次，最后一次 5000ms（差 0ms） | PASS |
| 旧代际事件被丢弃 | seek 期间旧快照不被采纳 | 丢弃 2 条 | PASS |
| 硬件解码 | hwdec-current 不是 no | d3d11va-copy | PASS |
| DASH 音视频分轨 | 音频来自第二个源，且解得出编码 | vorbis（视频与音频各走一个 demuxer） | PASS |
| 视频与弹幕的层级 | 视频在下、弹幕在上、HUD 在最上 | 见截图 6 张 | 人工确认 |
| HiDPI | FBO 像素尺寸 = 逻辑尺寸 × 缩放 | 1920x1080 | 人工确认 |
| 全屏 | 能进能出且不崩 | 脚本切换过两次 | 人工确认 |
| 窗口缩放 | 缩放后视频与弹幕都跟着变 | [(960, 540), (1280, 720)] | 人工确认 |
| 播放失败事件 | 没有 Failed / SurfaceLost | 无 | PASS |

## 截图

- `01-playing.png`
- `02-paused.png`
- `03-after-seek.png`
- `04-fullscreen.png`
- `05-resized.png`
- `06-final.png`

## 备注

- 弹幕密度是合成的（形状取自 golden fixture），不代表线上真实弹幕量；高级弹幕（mode 7/9）未渲染。

