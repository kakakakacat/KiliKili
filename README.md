<div align="center">

# KiliKili

### 把核心搬进 Rust 的第三方哔哩哔哩客户端

Android · iPhone / iPad · macOS · Windows · Linux

[下载](https://github.com/kakakakacat/KiliKili/releases) · [English](README.en.md) · [反馈](https://github.com/kakakakacat/KiliKili/issues)

</div>

KiliKili 不是换一张皮的第三方客户端。**真正吃性能的部分——取数、选流、弹幕、插帧——都从 Dart 搬进了 Rust**，界面这一层以后换掉也不用重写一遍。

---

## 特点

### 一、核心用 Rust 重写

不是「调用了一点 Rust」，是把业务逻辑整条搬过去，一条接口一个切片地迁（[#67](https://github.com/kakakakacat/KiliKili/issues/67)），**只读接口已经基本迁完**：

| Rust crate | 搬进去的东西 |
| --- | --- |
| `bili-api` / `bili-grpc` / `bili-protocol` | 接口请求、WBI 签名、protobuf 解析、风控参数 |
| `bili-core::media_select` | 清晰度与音质的候选顺序、回退链、CDN 选路 |
| `bili-danmaku` | 弹幕解析、屏蔽规则的正则匹配、播放进度游标 |
| `player-interp` | GPU 视频插帧引擎（见下） |
| `bili-core::glass_droplet` | 底栏水滴的弹簧形变 |

这样做的直接好处：**原生代码、没有 GC 停顿**；五个平台跑的是同一份判断，不会各端各写一套、各错一套。连「搜索撞上风控时退避多久、重试几次」这种策略也在 Rust 里（[#304](https://github.com/kakakakacat/KiliKili/issues/304)），不是散在界面代码里。

> 我们没有和官方客户端做过受控对照测量，所以这里不报「快了百分之多少」。测量方法见 [`docs/qa`](docs/qa)。

### 二、GPU 视频插帧（补帧）

让 24 / 30 fps 的片源补到 60 fps 再上屏。不是帧混合，是**运动补偿**：低分辨率块匹配估计运动场，再用原分辨率纹理做双向补偿。

真机实测（[#218](https://github.com/kakakakacat/KiliKili/issues/218) 的 spike，记录在 [`docs/m0/spikes/2026-11-24-interp-218.md`](docs/m0/spikes/2026-11-24-interp-218.md)）：

- **低端安卓机也跑得动**：SM6225 / Adreno 610 上，720p 30→60 实时插帧**稳定 60 fps**，显示间隔和不插帧的直画路径一样，连续 15 分钟不降频。24→60 同样跑满。
- **iPhone 16 Pro Max 上运动估计约 3 ms**。
- **一份 GLSL 跑五个平台**（GL 3.3 / GLES 3.0），只用 fragment shader，不需要 compute shader，也不必单独写一份 Metal。
- **认得出重复帧**：番剧常见 23.976 fps 的一拍二、一拍三，相邻两帧完全相同，不会被当成静止画面去插值。
- 档位是 关闭 / 45 FPS / 60 FPS / 跟随屏幕刷新率，**默认关闭**；持续掉帧或机器发烫时自动降档，稳下来再升回去。

引擎代码在 [`rust/crates/player-interp`](rust/crates/player-interp)，控制面有单元测试覆盖。**它随下一代 Slint 客户端一起到来**（[#219](https://github.com/kakakakacat/KiliKili/issues/219) 移动端、[#220](https://github.com/kakakakacat/KiliKili/issues/220) 桌面端），当前的 Flutter 版还没有。

### 三、Liquid Glass 水滴

底部导航的选中指示是一滴真玻璃：有折射和高光，切换标签时像液体一样先被拉长、冲过目标再回弹，按住会鼓起。走 Impeller 的 shader 折射，设备跑不动或系统开了「减弱动态效果」时自动降级，不会卡（[#266](https://github.com/kakakakacat/KiliKili/issues/266)）。

### 四、重做的「我的」页

账户信息和动态 / 关注 / 粉丝统计合成一张卡片，顶部是一体式工具胶囊（无痕模式、主题切换、搜索、消息），下面是统一圆角的快捷入口和收藏卡片。窄屏、平板、深浅主题、大字体都适配过（[#262](https://github.com/kakakakacat/KiliKili/issues/262)）。

### 五、不必要的东西一律砍掉

没有广告，没有开屏，没有商城和会员购入口，**不集成任何推送 SDK**。内置 SponsorBlock 自动跳过恰饭片段，番剧可以跳过片头片尾。

---

## 界面

| Android 首页 | Android 播放页 |
| :---: | :---: |
| <img src="assets/screenshots/kilikili-android-home.png" width="260" alt="KiliKili Android 首页"> | <img src="assets/screenshots/kilikili-android-video.png" width="260" alt="KiliKili Android 播放页"> |

| iPhone 首页 | iPhone 播放页 |
| :---: | :---: |
| <img src="assets/screenshots/kilikili-ios-home.png" width="260" alt="KiliKili iPhone 首页"> | <img src="assets/screenshots/kilikili-ios-video.png" width="260" alt="KiliKili iPhone 播放页"> |

## 还在路上

- **视频内搜索**：按字幕和弹幕直接定位到要看的那一段（[#213](https://github.com/kakakakacat/KiliKili/issues/213)）。
- **智能弹幕**：密度调节、自动避开字幕、防剧透预览（[#214](https://github.com/kakakakacat/KiliKili/issues/214)）。
- **玻璃界面扩展到更多页面**：从导航栏扩到整套控件，并做五平台适配与低端机降级（[#224](https://github.com/kakakakacat/KiliKili/issues/224)–[#226](https://github.com/kakakakacat/KiliKili/issues/226)）。
- **更灵活的倍速**：细粒度调速、任意输入、记住上次的倍速（[#291](https://github.com/kakakakacat/KiliKili/issues/291)）。
- **动态头像**：gif / ResAnimation 头像不再被压成静态图（[#292](https://github.com/kakakakacat/KiliKili/issues/292)）。

## 下载

正式安装包尚未发布，请关注 [Releases](https://github.com/kakakakacat/KiliKili/releases)。遇到问题或有建议，欢迎到 [Issues](https://github.com/kakakakacat/KiliKili/issues) 反馈。

---

KiliKili 基于 [PiliPlus](https://github.com/bggRGjQaUbCoE/PiliPlus)，感谢 [PiliPalaX](https://github.com/orz12/PiliPalaX)、[PiliPala](https://github.com/guozhigq/pilipala) 及所有贡献者。本项目是非官方客户端，与哔哩哔哩无关，遵循 [GPL-3.0](LICENSE) 许可证。
