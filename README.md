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

| 平台 | 最低系统版本 |
| --- | --- |
| Android | 7.0（API 24） |
| iOS / iPadOS | 15.0 |
| macOS | 12 Monterey |
| Windows | 10（64 位） |
| Linux | glibc 2.38+（实验性，未在真机上测试过） |

除 Android 外的安装包都没有开发者签名，装之前要绕过系统拦截，办法写在
[Release 页](https://github.com/kakakakacat/KiliKili/releases/latest)上。

## 特点

| | |
| --- | --- |
| **Rust 重构底层** | 性能提升 |
| **GPU 补帧** | 24/30 fps 补到 60，低端机也跑满（[实测](docs/m0/spikes/2026-11-24-interp-218.md)） |
| **Liquid Glass 水滴** | 底栏选中指示，切换时拉伸回弹 |
| **重做「我的」页** | 账户卡片 + 一体工具胶囊 |
| **没有多余东西** | 无广告、无开屏、无商城、无推送 SDK |

## 即将到来

| | |
| --- | --- |
| GPU 视频插帧 | [#211](https://github.com/kakakakacat/KiliKili/issues/211) |
| 视频内搜索 | [#213](https://github.com/kakakakacat/KiliKili/issues/213) |
| 智能弹幕 | [#214](https://github.com/kakakakacat/KiliKili/issues/214) |
| 玻璃界面扩展 | [#224](https://github.com/kakakakacat/KiliKili/issues/224) |
| 更灵活的倍速 | [#291](https://github.com/kakakakacat/KiliKili/issues/291) |
| 动态头像 | [#292](https://github.com/kakakakacat/KiliKili/issues/292) |
| 酷炫皮肤 | [#316](https://github.com/kakakakacat/KiliKili/issues/316) |

## 下载

[最新版本](https://github.com/kakakakacat/KiliKili/releases/latest) · [全部版本](https://github.com/kakakakacat/KiliKili/releases) · [反馈](https://github.com/kakakakacat/KiliKili/issues)

## 致谢

基于 [PiliPlus](https://github.com/bggRGjQaUbCoE/PiliPlus)，感谢 [PiliPalaX](https://github.com/orz12/PiliPalaX)、[PiliPala](https://github.com/guozhigq/pilipala) 及所有贡献者。非官方客户端，遵循 [GPL-3.0](LICENSE)。
