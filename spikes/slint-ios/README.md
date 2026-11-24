# slint-ios — SPIKE-004 的实验台（issue #11）

**进行中。** 一个跑在 iPhone 真机上的 Slint App，逐步回答 #11：
iOS 上 Slint + libmpv 能不能走、PiP 与后台音频怎么做、AVPlayer 作为备选的代价。

## 跑

```sh
cd spikes/slint-ios/ios
./link-frameworks.sh            # 把 media_kit 的 iOS libmpv 软链进来（要先 flutter pub get）
xcodegen generate               # brew install xcodegen
xcodebuild -project SlintIosSpike.xcodeproj -scheme SlintIosSpike -configuration Release \
  -destination 'id=<iPhone UDID>' -derivedDataPath ../target/xcode \
  DEVELOPMENT_TEAM=<你的团队 ID> -allowProvisioningUpdates build
xcrun devicectl device install app --device <UDID> ../target/xcode/Build/Products/Release-iphoneos/SlintIosSpike.app
xcrun devicectl device process launch --device <UDID> --console --terminate-existing \
  --environment-variables '{"SPIKE_VIDEO":"…","SPIKE_AUDIO":"…","SPIKE_UA":"…","SPIKE_REFERER":"https://www.bilibili.com"}' \
  dev.kilikili.spike.slintios
```

团队 ID 不写进仓库。免费 Personal Team 可以用，但**一台设备最多装 3 个**用它签的 App，
装满了会报「maximum number of installed apps using a free developer profile」。

## 已经撞出来的（详细结论最后写进 docs/m0/spikes/）

- **iOS 27 SDK 要求 UIScene 生命周期**：Info.plist 不声明 scene delegate，App 一连上 scene
  就被 UIKit 断言掉。Slint 1.19 自带 `SlintWindowSceneDelegate`，要在清单里写上它。
- **iOS 上 Slint 没有 OpenGL 渲染器**；winit 后端还**强制**链上 Skia（要 `-lc++`）。实际选中的是
  femtovg-wgpu（Metal），`GraphicsAPI::WGPU30`。
- **中文全是方块**：fontique 在 iOS 上看得到 361 个字体家族，但没有苹方，汉字的回退列表是空的。
  Hiragino 能救大部分字，但缺简体专用字。要么 App 自带简体字体，要么修 fontique 的 iOS 后端。
- **mpv 软件渲染不可行**：852x480 一帧 ~38ms，放在 UI 线程上界面掉到 5–7fps；挪走也要吃掉约一个核。
- **后台音频**：声明 `UIBackgroundModes: audio` 后，退到后台约 2–3 秒停顿后继续播放。
