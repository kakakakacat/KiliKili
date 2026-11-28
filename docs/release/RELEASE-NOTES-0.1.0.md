# KiliKili 0.1.0 发布说明

> 状态：**待发布**，跟踪 issue #242，「已知问题」一节由 #311 收口。
> 发版时把本文「给用户看的部分」原样贴进 GitHub Release 正文。
>
> 验收情况见 #242：Android 与 Windows 已按 [`docs/qa/RELEASE-VALIDATION.md`](../qa/RELEASE-VALIDATION.md)
> 跑过主链路，其余未跑完的用例由所有者决定按通过处置（处置记录在 #242）。
> #311 第三节里还有三项待定内容（三个 UX 小瑕疵、是否写明验收覆盖面、
> 关注列表 M16），所有者决定**本版先不写进来**。

---

<!-- ===== 给用户看的部分（贴进 Release 正文）从这里开始 ===== -->

## KiliKili 0.1.0

第一个公开版本。非官方哔哩哔哩客户端，基于 [PiliPlus](https://github.com/bggRGjQaUbCoE/PiliPlus)，
核心逐步换成 Rust 实现；[GPL-3.0](https://github.com/kakakakacat/KiliKili/blob/main/LICENSE)，
与哔哩哔哩无关。**支持的系统版本见 [README](https://github.com/kakakakacat/KiliKili#适配平台)。**

### 安装

除 Android 外都没有开发者签名，系统会拦一下，这是预期的。请只从本页下载。

**Android** —— 下载 `KiliKili_android_0.1.0_arm64-v8a.apk`（老 32 位机型选 `armeabi-v7a`，
模拟器选 `x86_64`）直接安装。提示「禁止安装未知应用」就按提示允许来源；Play 保护提示
「未知开发者」点「更多详情 → 仍然安装」。

**iOS / iPadOS** —— IPA 未签名，用 [AltStore](https://altstore.io/)、[SideStore](https://sidestore.io/)
或 [Sideloadly](https://sideloadly.io/) 以自己的 Apple ID 签名安装；装完到「设置 → 通用 →
VPN 与设备管理」信任该开发者，iOS 16 及以上还要打开「设置 → 隐私与安全性 → 开发者模式」。
免费账号签出来的 7 天失效，用同一工具重签即可。

**macOS** —— 打开 dmg，把 KiliKili 拖进「应用程序」。首次打开提示「无法验证开发者」时，
到「系统设置 → 隐私与安全性」点「仍要打开」；若提示「已损坏」，在终端执行：

```sh
xattr -dr com.apple.quarantine /Applications/KiliKili.app
```

**Windows** —— 安装版运行 `setup.exe`，出现蓝色「Windows 已保护你的电脑」点「更多信息 →
仍要运行」；便携版解压即用。

**Linux** —— **实验性：没有在任何一台真实 Linux 机器上启动过。** 提供 `.deb` / `.rpm` /
`.AppImage` / `.tar.gz`，需要 glibc 2.38 及以上（Ubuntu 24.04、Debian 13、Fedora 39 或更新）。

问题反馈：<https://github.com/kakakakacat/KiliKili/issues>

<!-- ===== 给用户看的部分到这里结束 ===== -->

---

## 附：给维护者

### 最低系统版本是怎么定的

按仓库所有者的决定（2026-11-26）「跟上游 PiliPlus 一致」。上游与我们用同一个 Flutter
（`pubspec.yaml` 的 `flutter: 3.47.9`），所以实际下限由 Flutter 引擎与工程配置共同决定：

| 平台 | 工程里写的 | 实际下限 | 依据 |
|---|---|---|---|
| Android | `minSdk = flutter.minSdkVersion` | **API 24** | Flutter 3.47.9 `FlutterExtension.kt` 的默认值；上游同 |
| iOS | `IPHONEOS_DEPLOYMENT_TARGET = 14.0`、Podfile `14.0` | **15.0** | Flutter 3.47.9 的 `Flutter.framework` 是 `minos 15.0`（`vtool -show-build`），14.x 上装不上 |
| macOS | `MACOSX_DEPLOYMENT_TARGET = 10.14`、Podfile `10.14` | **12.0** | `FlutterMacOS.framework` 是 `minos 12.0`（arm64 与 x86_64 相同） |
| Windows | — | **Windows 10 x64** | Flutter 桌面的支持范围；只构建 x64 |
| Linux | — | glibc ≥ 2.38 | `linux_x64.yml` 跑在 `ubuntu-latest`（24.04）；候选 deb 实测 `Depends: libc6 >= 2.38` |

iOS / macOS 工程文件里的 14.0 / 10.14 是上游留下的旧值，与上游一致所以没改；它们不影响
产物能装的最低系统（引擎二进制本身就要求更高）。以后升 Flutter 时按同样的方法重查一遍。

### 版本号

`pubspec.yaml` 只决定**版本名** `0.1.0`。CI 构建时 `lib/scripts/build.ps1` 会改写它：
`+N` 换成 `git rev-list --count HEAD`（提交数，天然递增，所以 Android 的 `versionCode` 不用
手工维护），Android 的版本名再追加 `-<commit 前 9 位>`。

**这些都只进包里，不进文件名**（#318）：`build.ps1` 另写一个 `file_version`，就是
`pubspec.yaml` 里的版本名本身，五个平台工作流的产物文件名统一用它——
`KiliKili_android_0.1.0_arm64-v8a.apk`、`KiliKili_linux_0.1.0_amd64.deb`。
deb / rpm 包**内部**的 `Version:` 仍带 `+提交数`，包管理器要它单调递增。

### 发版步骤

见 issue #242 第二节。签名材料与 secrets 见 [`SIGNING.md`](SIGNING.md)；本版只有 Android
（以及 Linux 的 rpm GPG 签名，如果 `LINUX_GPG_PRIVATE_KEY` 在）是签名的。

### 已知问题（0.1.0）

所有者决定正文只写安装说明（#318），所以这一节留在文档里、不进 Release 正文；
长期收口在 #311。

- **Linux 版没有在真机上测试过**。安装包能构建、依赖也按程序实际链接的库生成，
  但我们没有 Linux 桌面环境，能用、不能用都欢迎反馈。
- **Linux 的 deb 包少一条依赖声明**：程序里的 `libdartjni.so` 需要 `libjvm.so`，deb 的
  `Depends` 里没有它。只要用不到这部分功能就没有影响。
- **默认主题色从蓝色改成了经典粉**。从旧版本升级上来、而且从没自己选过主题色的用户，
  打开后会发现界面变粉了，在设置里可以改回去。
- **番剧的片头片尾跳过会被 SponsorBlock 的分类开关连带关掉**：在设置里关掉 SponsorBlock
  的某些分类后，番剧的自动跳过也会一起失效（[#278](https://github.com/kakakakacat/KiliKili/issues/278)）。
- **自己发的短评在应用内删不掉**：发出去的短评不会出现在「我的评论」里，要删只能去
  官方客户端。上游版本也是这样。

### 许可证与来源

KiliKili 以 [GNU GPL v3.0](https://github.com/kakakakacat/KiliKili/blob/main/LICENSE) 发布，
完整源码在 <https://github.com/kakakakacat/KiliKili>（本版本对应的提交见 Release 页的 tag）。

本项目是 [PiliPlus](https://github.com/bggRGjQaUbCoE/PiliPlus) 的分支；PiliPlus 又源自
[PiliPalaX](https://github.com/orz12/PiliPalaX) 与 [PiliPala](https://github.com/guozhigq/pilipala)。
感谢这些项目的作者和所有贡献者。

KiliKili 是非官方客户端，与哔哩哔哩（上海宽娱数码科技有限公司）无关；视频、弹幕等内容的
版权归原作者与平台所有。本软件按「现状」提供，不附带任何担保（GPL v3.0 第 15、16 条）。
