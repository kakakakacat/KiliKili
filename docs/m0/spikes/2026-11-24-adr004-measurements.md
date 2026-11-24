# ADR-004 §6 补测：编码 / 分辨率 / 操作 / 内存 / 音画同步（#210）

日期 2026-11-24。手写记录（理由同 SPIKE-004：iOS 入口是 App；macOS 部分的逐次
`--record` 产出含签名取流地址的上下文，只留在本机，这里摘数字）。

- **macOS**：Apple M4，macOS 27.3，`spikes/slint-mpv` 脚本化一轮
  （`--scripted --seek-to 30000 --danmaku-rate 30 --flip-y`）。
- **iOS**：iPhone 16 Pro Max，iOS 27.3，`spikes/slint-ios` gl 模式（零拷贝），每组 40 秒，
  取第 4 秒以后的每秒 HUD 行。
- **Android**：一台**低端**设备（Snapdragon 680 / SM6225、Adreno 610、Android 14、屏幕 640×960），
  `spikes/slint-mpv` 的 Android 探针（`hwdec=mediacodec`，零拷贝），每组 30 秒，取第 5 秒以后的
  每秒 STATUS 行。SPIKE-002 用过的 Adreno 740 旗舰机这次不在手边。
- 片源：登录态取流。普通片源一条（H.264 / HEVC / AV1 各有 1080p，H.264 / HEVC 有 4K）；
  杜比视界另用一条带 `dvh1.05.07` 的 4K 片源（同一条也有 4K AV1）。
  记录里只写清晰度与 codec 串；登录态与带签名地址没有离开本机临时目录。
- **Windows 没有测**：这台机器上没有对应硬件，留在 #210。

## 0. 结论

1. **iOS 上 mpv 的零拷贝硬解有一个颜色 bug，已找到绕法。** VideoToolbox 默认输出
   nv12，mpv 0.39 的 iOS GLES interop（CVOpenGLESTextureCache）采样色度平面时
   **V 读成了 U**：肤色发绿、蓝色背景发紫。这个现象由仓库所有者在真机上肉眼发现；
   我们对照 AVPlayer 与软解，确认问题只出在零拷贝 interop（§3）。
   **修法：`hwdec-image-format=bgra`**，让 VT 直接出 BGRA，单平面、仍零拷贝，颜色与
   AVPlayer 一致，出帧耗时和内存反而更低。SPIKE-004 当时的截图也是偏色的，当时没人看出来。
2. **HEVC、杜比视界（P5）在两端都是硬解。** AV1 在 macOS（M4）是硬解；**在 iOS 上是软解**：
   media_kit 带的 ffmpeg 没走 VT 的 AV1，A18 Pro 的硬件没用上。软解 4K AV1 仍然跑满
   24 fps，CPU 约一个核。
3. **seek / 倍速 / 切清晰度在 iOS 上都通**：seek 0.3–0.8 s 恢复出帧，切清晰度约 1 s 并从原位置
   续播，2 倍速实测位置走速 ≈ 2×。音画差（mpv `avsync`）稳态 ≤ 12 ms。
4. **内存**：iOS 峰值 RSS 310–490 MiB（4K 含 3 张零拷贝目标），macOS 450–670 MiB。
5. **Android 低端机的 4K 会静默落到软解。** 解码器能力表写着最大 4096×2176，实际 `configure`
   4K 就失败（`IllegalArgumentException`），mpv 退到软解：UI 10–19 fps、CPU 260–520%，完全不可看。
   1080p 的 H.264 / HEVC 是 `mediacodec` 硬解，UI 58–59 fps；AV1 没有硬件解码器，1080p 软解仍能
   58 fps（CPU 约 190%）。seek / 倍速 / 切清晰度在 Android 上同样都通。
6. **macOS 帧耗时 p95 那条线跟显示环境有关，跟编码无关**：同一条 1080p H.264，上午
   19.87 ms，下午 28.48 ms；下午六组（含 H.264 基线）全在 28 ms 上下，p50 13.5 ms。
   20 ms 这道线实际量的是窗口在哪块屏、按什么刷新率合成（§1.2）。

## 1. macOS（M4）

| 片源 | codec | 硬解 | 平均 fps | 帧耗时 p50 / p95 | CPU | 峰值 RSS | seek / 暂停 / 分轨 |
|---|---|---|---|---|---|---|---|
| 1080p H.264（上午） | avc1.640032 | videotoolbox | 57.5 | 16.72 / 19.87 | 22% | 562 MiB | PASS |
| 1080p H.264（下午重跑） | avc1.640032 | videotoolbox | 58.6 | 13.46 / 28.48 | 33% | 560 MiB | PASS |
| 1080p HEVC | hvc1.1.6.L150.90 | videotoolbox | 58.6 | 13.54 / 27.88 | 34% | 561 MiB | PASS |
| 1080p AV1 | av01.0.08M.08 | videotoolbox | 58.6 | 13.45 / 28.48 | 36% | 449 MiB | PASS |
| 4K H.264 | avc1.640033 | videotoolbox | 56.4 | 13.92 / 28.49 | 38% | 663 MiB | PASS |
| 4K HEVC | hvc1.1.6.L153.90 | videotoolbox | 55.9 | 13.80 / 28.92 | 35% | 671 MiB | PASS |
| 4K 杜比视界 | dvh1.05.07 | videotoolbox | 55.3 | 14.02 / 28.33 | 31% | 520 MiB | PASS |

（帧耗时是 UI 帧间隔；CPU / RSS 是实验台结束时的读数。）

### 1.1 自动项

六组都是：`hwdec-current = videotoolbox`、收到 SeekCompleted 且落点误差 0 ms、暂停后位置
不动、音频走第二个 demuxer。唯一不过的是 p95 ≤ 20 ms。

### 1.2 p95 为什么不算编码的锅

p50 从 16.7 ms 变成 13.5 ms，说明下午窗口的合成节奏变了（这台机器同时接着内建 ProMotion
屏和一块 60 Hz 外接屏）。p95 ≈ 两个 p50，是「偶尔错过一次合成」的形状。H.264 基线在同一时段
同样 28 ms，所以不同编码之间的对比仍然成立；但 SPIKE-001 那条「p95 19.98 ms 贴线」也该按
同样的方式看待：**它量的是显示环境，不是 mpv**。ADR-004 决策五据此改写。

### 1.3 跑测本身踩到的两件事

- 带签名的地址约 2 小时过期。过期后 mpv 什么都放不出来，但 UI 照样 60 fps，记录会显示
  「帧率 PASS、位置 0 ms、hwdec 取不到」，看起来像能用。第一轮因此作废重跑。
- 屏幕锁定时窗口不重绘，帧率掉到 0.2 fps。

## 2. iOS（iPhone 16 Pro Max，零拷贝 + `hwdec-image-format=bgra`）

| 片源 | codec | 解码 | 视频 fps | 出帧 p95 最大 | CPU 均值 | 峰值 RSS | \|avsync\| 最大 |
|---|---|---|---|---|---|---|---|
| 1080p H.264 | avc1.640032 | videotoolbox → bgra | 25.0 | 2.10 ms | 65% | 426 MiB | 0.000 s |
| 1080p HEVC＋操作 | hvc1.1.6.L150.90 → 720p | videotoolbox → bgra | — | 12.03 ms | 87% | 428 MiB | 0.002 s |
| 1080p AV1 | av01.0.08M.08 | **软解** yuv420p | 25.0 | 3.34 ms | 86% | 493 MiB | 0.000 s |
| 4K H.264 | avc1.640033 | videotoolbox → bgra | 25.0 | 1.31 ms | 51% | 445 MiB | 0.000 s |
| 4K HEVC | hvc1.1.6.L153.90 | videotoolbox → bgra | 25.0（稳态） | 12.19 ms | 62% | 477 MiB | 0.012 s |
| 4K AV1 | av01.0.12M.08 | **软解** yuv420p | 24.0 | 8.09 ms | 102% | 482 MiB | 0.000 s |
| 4K 杜比视界 | dvh1.05.07 | videotoolbox → bgra | 24.0 | 1.62 ms | 42% | 311 MiB | 0.000 s |

- UI 全程 55–60 fps（封顶 60，见 SPIKE-004 §7）。CPU 是整个进程，含 12 行合成弹幕与 HUD。
- 「出帧」= mpv 渲染进 IOSurface + glFlush 占 UI 线程的时间。两个 12 ms 的峰都出现在起播或换流
  那一秒，稳态都在 2 ms 以内。
- 4K HEVC 前 3 秒有两次缓冲停顿（网络），之后稳定 25 fps。
- 杜比视界 P5 不需要任何设置：VT 本来就输出 BGRA，所以它是这一轮里唯一「颜色一直是对的」的片源。

### 2.1 操作时间线（1080p HEVC，`SPIKE_SCRIPT`）

| t（秒） | 动作 | 结果 |
|---|---|---|
| 10.0 | `seek 60 absolute`（当时 pos 9.0） | 10.31 s 收到 PLAYBACK_RESTART，pos 60.04 |
| 16.0 | `set speed 2` | 8 秒内 pos 65.5 → 81.2（≈ 2×），音画差不变 |
| 24.0 | `set speed 1` | — |
| 28.0 | `quality`：换到 720p HEVC（视频＋音频两路都换，从当前位置续播） | 29.00 s 恢复出帧，pos 85.56（原位置 85.52）；零拷贝目标按新尺寸重建 |
| 36.0 | `seek 10 absolute` | 36.84 s 恢复，pos 10.04 |

切清晰度在这里的做法是 `start=<当前位置>` 加 `loadfile … replace`，是整条重载，不是无缝切换。
无缝切换要 mpv 的 `edl` 或 DASH 自适应，不在这一轮的范围里。

## 2.5 Android（SM6225 / Adreno 610，`hwdec=mediacodec`）

| 片源 | 解码 | UI fps | mpv 渲染 均值 / 最大 | CPU 均值 | 峰值 RSS | \|avsync\| 最大 | 丢帧 |
|---|---|---|---|---|---|---|---|
| 1080p H.264 | mediacodec | 58.8 | 3.9 / 39 ms | 119% | 439 MiB | 0.000 s | 1 |
| 1080p HEVC＋操作 | mediacodec（720p 后同） | 58.0 | 3.9 / 103 ms | 102% | 310 MiB | 0.003 s | 15（倍速时） |
| 1080p AV1 | **软解**（没有硬件解码器） | 58.0 | 5.5 / 27 ms | 188% | 432 MiB | 0.000 s | 3 |
| 4K H.264 | **软解**（configure 失败） | 19.1 | 14.0 / 82 ms | 262% | 611 MiB | 0.012 s | 23 → 208 |
| 4K HEVC | **软解**（同上） | 16.2 | 20.0 / 110 ms | 393% | 733 MiB | 0.000 s | 52 → 281 |
| 4K 杜比视界 | **软解**（同上） | 10.8 | 70.1 / 210 ms | 521% | 861 MiB | 5.978 s | 56 → 426 |

- CPU 是整个进程，100% = 一个核（这台 8 核）。「mpv 渲染」是 `BeforeRendering` 里 mpv 往
  Slint 的 FBO 画一帧的耗时；最大值都出在起播或换流那一秒。
- 4K 失败的原文：`h264_mediacodec: Failed to configure codec c2.qti.avc.decoder ... MediaCodec failed
  to start` → `Using software decoding`。`/vendor/etc/media_codecs*.xml` 里 `c2.qti.avc.decoder` 与
  `c2.qti.hevc.decoder` 的 size 上限都写着 4096×2176，**只看尺寸会判断成支持**。
- 杜比视界在这台机器上没有专门的解码器，按 HEVC 软解；画面颜色没有判定（不可看，没意义）。

操作时间线（1080p HEVC，驱动脚本经 `cmd.txt` 下发）：

| 动作 | 结果 |
|---|---|
| `prop time-pos 60`（pos 7.7） | 下一秒 pos 60.84、音频在走（< 1 s） |
| `prop speed 2` | 每秒走 2.0 s，音画差 0 |
| `prop speed 1` | 换回时一次性丢 15 帧，之后不再丢 |
| `switch`：换到 720p HEVC，从 86.64 s 续播 | 1 s 内恢复，pos 86.96，仍是 mediacodec |
| `prop time-pos 10` | < 1 s 恢复 |

## 3. 偏绿：定位过程

同一帧（pos 11.1 s）在真机上截图对照：

| 路径 | VO 格式 | 颜色 |
|---|---|---|
| AVPlayer（对照） | — | 正常 |
| mpv `hwdec=videotoolbox`（默认） | `videotoolbox[nv12]` | **肤色绿、背景紫** |
| mpv `fbo-format=rgba8` | `videotoolbox[nv12]` | 同上，与中间 FBO 无关 |
| mpv `hwdec=videotoolbox-copy` | `nv12`（拷回内存再上传） | 正常 |
| mpv `hwdec=no` | `yuv420p` | 正常 |
| mpv `hwdec-image-format=bgra` | `videotoolbox[bgra]` | **正常** |

- 「绿＋紫」的方向说明 Cr 取到了 Cb 的值：暖色肤色（Cb 低）变成 Cr 也低，也就是绿；冷色背景
  （Cb 高）变成 Cr 也高，也就是紫。
- mpv 自己判断的 GL 环境是对的（`GL_VERSION='OpenGL ES 3.0 Metal - 105'`、Detected GLES 3.0），
  软解和 copy 两条路都用 mpv 自建的纹理，颜色正常；只有 CVOpenGLESTextureCache 给第二个平面
  建的纹理不对。根因在 mpv 的 `hwdec_ios` 与 iOS 的 GLES-on-Metal 之间，没有往下挖。
- 1080p H.264 下几种做法的代价：

  | 做法 | 出帧 p95 | CPU | 峰值 RSS |
  |---|---|---|---|
  | nv12 零拷贝（颜色错） | 1.3–1.5 ms | 53–61% | 255 MiB |
  | **bgra 零拷贝** | 1.1–1.3 ms | 64–73% | 208 MiB |
  | videotoolbox-copy | 2.4–3.0 ms | 61–89% | 294 MiB |
  | 软解 | 2.0–2.8 ms | 69–72% | 345 MiB |

  （每种只跑 18 秒，CPU 波动大，只能看量级。）

## 4. 对 ADR-004 的影响

- 方向不变：没有哪个平台出现需要启用退路的硬伤。Android 4K 的问题看起来与播放器选型无关：
  Media3 用的也是系统的 `c2.qti.*` 解码器（这台机器上 Media3 跑 4K **没有实测**）；该做的是选清晰度。
- 决策三加一条 Android 必做项：按解码器的**尺寸＋帧率**能力
  （`VideoCapabilities.areSizeAndRateSupported`）给清晰度上限；mediacodec 起不来时降一档重开，
  不接受静默软解。
- 决策三加一条 iOS 必做项：`hwdec-image-format=bgra`。不做的后果是**静默**偏色，HUD 一切正常。
- 决策五改写 macOS p95 那条，并加一条风险：iOS 上 AV1 是软解。
- 还没测的：Windows 全部；Android 旗舰机（Adreno 740）上的 4K / AV1；iOS 的电量与发热；AVPlayer 在 HEVC / AV1 / DV 上的表现
  （决策四的退路是否覆盖这些编码）。
