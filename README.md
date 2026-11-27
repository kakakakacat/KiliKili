<div align="center">

# KiliKili

**更快、更稳的哔哩哔哩第三方客户端**

Android · iPhone / iPad · macOS · Windows · Linux

[简体中文](README.md) · [English](README.en.md) · [下载](https://github.com/kakakakacat/KiliKili/releases) · [反馈](https://github.com/kakakakacat/KiliKili/issues)

</div>

KiliKili 基于 [PiliPlus](https://github.com/bggRGjQaUbCoE/PiliPlus)，保留它的全部功能，并用 Rust 重写核心，让它更快、更稳。

<p align="center">
  <img src="assets/screenshots/kilikili-ios-home.png" height="400" alt="iOS 首页">
  <img src="assets/screenshots/kilikili-ios-video.png" height="400" alt="iOS 播放页">
  <img src="assets/screenshots/kilikili-android-home.png" height="400" alt="Android 首页">
  <img src="assets/screenshots/kilikili-android-video.png" height="400" alt="Android 播放页">
</p>

## 和原版哔哩哔哩比，哪里不一样

- **底层逻辑用 Rust 重写。** 取数、选流、排序与过滤这些核心逻辑正从 Dart 一条接口一条地搬进 Rust（[#67](https://github.com/kakakakacat/KiliKili/issues/67)），只读接口已基本迁完。原生代码、没有 GC 停顿，五个平台跑的是同一份实现，不会各端各写一套、各错一套。*我们还没有和官方客户端做受控对照测量，所以这里不报百分比；基准方法见 [docs/qa](docs/qa)。*

- **Liquid Glass 玻璃界面，iOS 那种水滴特效。** 底部导航的选中指示是一滴真玻璃：有折射和高光，切换标签时像液体一样先被拉长、冲过目标再回弹，按住会鼓起。走 Impeller 的 shader 折射，设备不支持或系统开了「减弱动态效果」时自动降级，不会卡。

- **「我的」页面重做过。** 账户信息和动态/关注/粉丝统计合成一张卡片，顶部是一体式工具胶囊（无痕模式、主题切换、搜索、消息），下面是统一圆角的快捷入口和收藏卡片，窄屏、平板、深浅主题、大字体都适配（[#262](https://github.com/kakakakacat/KiliKili/issues/262)）。

- **不必要的东西一律砍掉。** 没有广告，没有开屏，没有商城和会员购入口，不集成任何推送 SDK，不做「用完即走」之外的挽留。还内置 SponsorBlock 自动跳过恰饭片段，番剧可以跳过片头片尾。

## 即将到来

- **GPU 视频插帧**：让低帧率视频播放更顺滑，可随时关闭（[#211](https://github.com/kakakakacat/KiliKili/issues/211)、[#219](https://github.com/kakakakacat/KiliKili/issues/219)、[#220](https://github.com/kakakakacat/KiliKili/issues/220)）。
- **视频内搜索**：按字幕和弹幕直接定位到要看的那一段（[#213](https://github.com/kakakakacat/KiliKili/issues/213)）。
- **智能弹幕**：密度调节、自动避开字幕、防剧透预览（[#214](https://github.com/kakakakacat/KiliKili/issues/214)）。
- **玻璃界面扩展到更多页面**：从导航栏扩到整套控件，并做五平台适配与低端机降级（[#224](https://github.com/kakakakacat/KiliKili/issues/224)–[#226](https://github.com/kakakakacat/KiliKili/issues/226)）。
- **更灵活的倍速**：细粒度调速、任意输入、记住上次的倍速（[#291](https://github.com/kakakakacat/KiliKili/issues/291)）。
- **动态头像**：gif / ResAnimation 头像不再被压成静态图（[#292](https://github.com/kakakakacat/KiliKili/issues/292)）。

正式安装包尚未发布，请关注 [Releases](https://github.com/kakakakacat/KiliKili/releases)。

## 长期维护

我们会长期更新 KiliKili。遇到问题欢迎在 [Issues](https://github.com/kakakakacat/KiliKili/issues) 反馈。

感谢 [PiliPlus](https://github.com/bggRGjQaUbCoE/PiliPlus)、[PiliPalaX](https://github.com/orz12/PiliPalaX)、[PiliPala](https://github.com/guozhigq/pilipala) 和所有贡献者。本项目遵循 [GPL-3.0](LICENSE)，是非官方客户端。
