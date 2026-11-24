# slint-ios — SPIKE-004 的实验台（issue #11）

一个跑在 iPhone 真机上的 Slint App，回答 #11：iOS 上 Slint + libmpv 能不能走、
PiP 与后台音频怎么做、AVPlayer 作为备选的代价。**结论在
[docs/m0/spikes/SPIKE-004-ios.md](../../docs/m0/spikes/SPIKE-004-ios.md)**，这里只放跑法与代码地图。

## 跑

```sh
# 一次性：仓库根目录 flutter pub get（取 media_kit 的 iOS libmpv）、brew install xcodegen
DEVELOPMENT_TEAM=<你的团队 ID> SPIKE_DEVICE=<iPhone UDID> \
  spikes/slint-ios/build-run.sh <env.json> [秒数] [日志文件]
```

`env.json` 是给 `devicectl --environment-variables` 的 JSON：

| 变量 | 含义 |
|---|---|
| `SPIKE_VIDEO` / `SPIKE_AUDIO` | DASH 两路地址（playurl 的 `baseUrl`）；不给就只跑「Slint 能不能起来」 |
| `SPIKE_UA` / `SPIKE_REFERER` | 请求头，B 站 CDN 要 |
| `SPIKE_MODE` | `gl`（默认，零拷贝）/ `sw`（软件渲染基线）/ `avplayer`（对照组） |
| `SPIKE_AVP` | avplayer 模式的喂法：`hls`（默认）/ `two`（两个播放器）/ `composition` |
| `SPIKE_HWDEC` / `SPIKE_FLIP` / `SPIKE_GLFINISH` | 对照用：`videotoolbox-copy`、`1`（会倒）、`1` |
| `SPIKE_PIP` / `SPIKE_BG_VIDNO` / `SPIKE_BG_RENDER` | 后台实验：`0` 不装 PiP、`0` 不关视频轨、`1` 后台也画 |

**取流地址带签名与客户端 IP、约 2 小时过期**：`env.json` 放临时目录，别进仓库。
从桌面图标直接点开 App 是没有视频的（没有环境变量）。

切后台不用人：`xcrun devicectl device process launch --device <UDID> com.apple.Preferences`。

团队 ID 不写进仓库。免费 Personal Team 可以用，但**一台设备最多装 3 个**用它签的 App，
装满了会报「maximum number of installed apps using a free developer profile」。

## 代码地图

| 文件 | 内容 |
|---|---|
| `src/lib.rs` | 三种模式、每秒一行 HUD/日志、前后台与 PiP 状态机 |
| `src/zerocopy.rs` | CVPixelBuffer → MTLTexture → `wgpu::Texture` → `slint::Image` |
| `src/mpv.rs` | libmpv 最小绑定（sw 与 OpenGL 两种 render API） |
| `ios/bridge.m` | 平台胶水：EAGL + IOSurface 目标、PiP、AVPlayer（含 DASH→HLS） |
| `ios/project.yml` | xcodegen 工程描述（踩过的 Info.plist 坑都在注释里） |

## 环境里踩过的坑

- **iOS 27 SDK 要求 UIScene 生命周期**：Info.plist 不声明 scene delegate，App 一连上 scene
  就被 UIKit 断言掉。Slint 1.19 自带 `SlintWindowSceneDelegate`，要在清单里写上它。
- **iOS 上 Slint 没有 OpenGL 渲染器**；winit 后端还**强制**链上 Skia（要 `-lc++`）。
  不开 `unstable-wgpu-30` 时选中 femtovg-wgpu，开了以后选中 Skia（都是 Metal）。
- **中文全是方块**（#206）：fontique 在 iOS 上看不到苹方，汉字回退列表是空的；
  UI 暂用 Hiragino，缺简体专用字。
- 嵌入 media_kit 的 framework 要 `STRIP_BITCODE_FROM_COPIED_FILES: NO`。
- 免费 Personal Team 一台设备最多 3 个 App。
