<div align="center">
    <h1>KiliKili</h1>

中文 | [English](README.en.md)

<p>使用 Rust 重写核心的 BiliBili 第三方客户端</p>

<img src="assets/screenshots/kilikili-android-home.png" width="24%" alt="Android 首页" />
<img src="assets/screenshots/kilikili-android-video.png" width="24%" alt="Android 播放页" />
<img src="assets/screenshots/kilikili-ios-home.png" width="24%" alt="iOS 首页" />
<img src="assets/screenshots/kilikili-ios-video.png" width="24%" alt="iOS 播放页" />

</div>

<br/>

## 适配平台

- [x] Android
- [x] iOS / iPad
- [x] macOS
- [x] Windows
- [x] Linux

## 特点

| | |
| --- | --- |
| **核心在 Rust** | 取数、选流、弹幕、风控退避都在 Rust。原生代码、无 GC 停顿，五端同一份实现 |
| **GPU 补帧** | 运动补偿而非帧混合。720p 30→60 在 Adreno 610 上稳定 60 fps，一份 GLSL 跑五端（随 Slint 客户端到来） |
| **Liquid Glass 水滴** | 底栏选中指示是会折射的玻璃，切换时拉伸、冲过、回弹，按压鼓起 |
| **重做的「我的」页** | 账户卡片 + 一体工具胶囊 + 统一圆角快捷入口 |
| **没有多余的东西** | 无广告、无开屏、无商城、无推送 SDK；内置 SponsorBlock，可跳过番剧片头片尾 |

## Rust 迁移

| crate | 内容 | 状态 |
| --- | --- | --- |
| `bili-api` / `bili-grpc` / `bili-protocol` | 请求、WBI 签名、protobuf、风控参数 | 只读接口基本迁完（[#67](https://github.com/kakakakacat/KiliKili/issues/67)） |
| `bili-core::media_select` | 清晰度/音质回退链、CDN 选路 | 已迁 |
| `bili-danmaku` | 弹幕解析、屏蔽正则、进度游标 | 已迁 |
| `player-interp` | GPU 插帧引擎（块匹配 + 双向补偿） | 引擎完成，待接入 Slint |
| `bili-core::glass_droplet` | 水滴的弹簧形变 | 已迁 |

> 未与官方客户端做过受控对照测量，故不列性能数字。插帧实测见 [`docs/m0/spikes/2026-11-24-interp-218.md`](docs/m0/spikes/2026-11-24-interp-218.md)。

## 即将到来

| 功能 | issue |
| --- | --- |
| 五平台 GPU 视频插帧 | [#211](https://github.com/kakakakacat/KiliKili/issues/211) [#219](https://github.com/kakakakacat/KiliKili/issues/219) [#220](https://github.com/kakakakacat/KiliKili/issues/220) |
| 视频内搜索（按字幕和弹幕定位） | [#213](https://github.com/kakakakacat/KiliKili/issues/213) |
| 智能弹幕（密度、字幕避让、防剧透） | [#214](https://github.com/kakakakacat/KiliKili/issues/214) |
| 玻璃界面扩展到整套控件 | [#224](https://github.com/kakakakacat/KiliKili/issues/224)–[#226](https://github.com/kakakakacat/KiliKili/issues/226) |
| 更灵活的倍速 | [#291](https://github.com/kakakakacat/KiliKili/issues/291) |
| 动态头像 | [#292](https://github.com/kakakakacat/KiliKili/issues/292) |

## 下载

[Releases](https://github.com/kakakakacat/KiliKili/releases)（正式安装包尚未发布） · [反馈](https://github.com/kakakakacat/KiliKili/issues)

## 致谢

基于 [PiliPlus](https://github.com/bggRGjQaUbCoE/PiliPlus)，感谢 [PiliPalaX](https://github.com/orz12/PiliPalaX)、[PiliPala](https://github.com/guozhigq/pilipala) 及所有贡献者。非官方客户端，遵循 [GPL-3.0](LICENSE)。
