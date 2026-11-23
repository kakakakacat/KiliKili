# SPIKE-001 实验记录 · macos · 2026-11-23 08:49:36 UTC

自动产出于 `slint-mpv-spike`，不要手工编辑：重跑一次会整份覆盖。

命令行：`./target/release/slint-mpv-spike --video https://xy58x222x25x142xy.mcdn.bilivideo.cn:8082/v1/resource/upgcxcode/17/52/43787355217/43787355217-1-30032.m4s?<已脱敏> --audio https://xy183x234x98x153xy.mcdn.bilivideo.cn:8082/v1/resource/upgcxcode/17/52/43787355217/43787355217-1-30280.m4s?<已脱敏> --user-agent Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0 Safari/537.36 --referer https://www.bilibili.com --hwdec videotoolbox --flip-y --scripted --seek-to 30000 --danmaku-rate 30 --shots /private/tmp/claude-501/-Users-dm-KiliKili/02306e06-3383-487c-aa3b-cd251f002fe5/scratchpad/shots8b --record ../../docs/m0/spikes/SPIKE-001-macos.md`

**自动项结论：PASS**（人工项见下表）

## 环境

| 项 | 值 |
|---|---|
| 平台 | macos |
| 系统 | Darwin 27.3 |
| 内核 | 27.3.0 |
| CPU | Apple M4 |
| 内存 | 32.0 GiB |
| rustc | rustc 1.100.0 (0ef1bc386 2026-11-09) |
| commit | 101417a68 |
| 工作区是否干净 | 否（记录里的 commit 不足以复现，见备注） |
| slint | 1.19.0 |
| libmpv | /opt/homebrew/lib/libmpv.2.dylib |
| mpv client API | 2.6 |

## 媒体与渲染

| 项 | 值 |
|---|---|
| 模式 | 视频 + 弹幕 |
| 视频源 | https://xy58x222x25x142xy.mcdn.bilivideo.cn:8082/v1/resource/upgcxcode/17/52/43787355217/43787355217-1-30032.m4s?<已脱敏> |
| 音频源 | DASH 外挂音轨 |
| hwdec 设置 | videotoolbox |
| OpenGL | Apple \| Apple M4 \| 4.1 Metal - 91.7 |
| FBO 尺寸 | 2560x1440 |
| 弹幕 | 形状取自 typical + rich · 密度 30 条/秒（合成）· 入桶 18182 条 |
| hwdec 实际 | videotoolbox |
| 视频编码 | H.264 / AVC / MPEG-4 AVC / MPEG-4 part 10 |
| 视频格式 | h264 |
| 音频编码 | aac |
| 视频分辨率 | 852x480 |

## 测量

| 项 | 值 |
|---|---|
| mpv 丢帧 | 0 |
| 视频帧率 | 25.00 |
| 运行时长 | 25.5s |
| 总帧数 | 1483 |
| 平均 fps | 58.0 |
| 帧耗时 p50 / p95 / p99 | 16.72 / 19.98 / 23.59 ms |
| 掉帧（>33ms） | 6 |
| CPU / RSS（结束时） | 36.1% / 619.5 MiB（峰值 619.5 MiB） |
| 弹幕 累计 / 丢弃 | 437 条上屏，0 条因同屏上限丢弃，6 条因无空轨丢弃，221 条高级弹幕跳过 |
| 事件 | 采纳 487，因代际过期丢弃 1 |

## 验证项

| 项 | 预期 | 实际 | 结论 |
|---|---|---|---|
| 帧率 | 平均 ≥ 55 fps | 58.0 fps | PASS |
| 帧耗时 p95 | ≤ 20ms | 19.98 ms | PASS |
| 暂停冻结位置 | 暂停 1.4 秒后位置不变 | 4440ms → 4440ms | PASS |
| Seek 完成是事实事件 | 收到 SeekCompleted 且位置接近 30000ms | 收到 1 次，最后一次 30000ms（差 0ms） | PASS |
| 旧代际事件被丢弃 | seek 期间旧快照不被采纳 | 丢弃 1 条 | PASS |
| 硬件解码 | hwdec-current 不是 no | videotoolbox | PASS |
| DASH 音视频分轨 | 音频来自第二个源，且解得出编码 | aac（视频与音频各走一个 demuxer） | PASS |
| 视频与弹幕的层级 | 视频在下、弹幕在上、HUD 在最上 | 见截图 6 张 | 人工确认 |
| HiDPI | FBO 像素尺寸 = 逻辑尺寸 × 缩放 | 2560x1440 | 人工确认 |
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

