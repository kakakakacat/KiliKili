# KiliKili 0.1.0 发布说明

> 状态：**已发布**（2026-11-28）。tag `v0.1.0` → `1f84c3cdf`，产物版本 `0.1.0+6088`，
> 构建 [run 46109080291](https://github.com/kakakakacat/KiliKili/actions/runs/46109080291)，
> Release 页 <https://github.com/kakakakacat/KiliKili/releases/tag/v0.1.0>。
> 跟踪 issue #242，「已知问题」一节由 #311 收口。
> 本文「给用户看的部分」（下面两行注释之间）就是 Release 正文，改了要同步改 Release。
>
> 验收情况见 #242：Android 与 Windows 已按 [`docs/qa/RELEASE-VALIDATION.md`](../qa/RELEASE-VALIDATION.md)
> 跑过主链路，其余未跑完的用例由所有者决定按通过处置（处置记录在 #242）。
> #311 第三节里还有三项待定内容（三个 UX 小瑕疵、是否写明验收覆盖面、
> 关注列表 M16），所有者决定**本版先不写进来**。

---

<!-- ===== 给用户看的部分（贴进 Release 正文）从这里开始 ===== -->

## KiliKili 0.1.0

KiliKili 的第一个公开版本。它基于 [PiliPlus](https://github.com/bggRGjQaUbCoE/PiliPlus)，
保留它的功能，并把网络请求、数据解析、弹幕处理等核心逻辑逐步换成 Rust 实现。
界面仍是 Flutter。

**这是非官方客户端，与哔哩哔哩（bilibili）没有任何关系。**

### 支持的平台

| 平台 | 安装包 | 最低系统版本 | 状态 |
|---|---|---|---|
| Android | `KiliKili_android_…_arm64-v8a.apk`（大多数手机）<br>`…_armeabi-v7a.apk`（老 32 位手机）<br>`…_x86_64.apk`（模拟器 / x86 设备） | Android 7.0（API 24） | 已签名 |
| iOS / iPadOS | `KiliKili_ios_….ipa` | iOS 15.0 | **未签名**，需自行侧载 |
| macOS | `KiliKili_macos_….dmg` | macOS 12 Monterey | **未签名、未公证** |
| Windows | `KiliKili_windows_…_x64_setup.exe`（安装版）<br>`KiliKili_windows_…_x64_portable.zip`（便携版） | Windows 10（64 位） | **未签名** |
| Linux | `.deb` / `.rpm` / `.AppImage` / `.tar.gz`（x86_64） | 见下方说明 | **实验性：未在真机上测试过** |

**Linux 是实验性的。** 安装包能构建出来，依赖也按程序实际链接的库自动生成，但我们没有
Linux 桌面环境，**没有在任何一台真实的 Linux 机器上启动过它**。能用、不能用都欢迎反馈。
安装包在 Ubuntu 24.04 上构建，需要同代或更新的发行版（glibc 2.38 及以上，例如
Ubuntu 24.04、Debian 13、Fedora 39）。

### 安装，以及怎么绕过系统拦截

除 Android 外，这一版的安装包**都没有开发者签名**（证书需要付费，0.1 先不买）。系统会
因此拦一下，这是预期的，不代表文件有问题。请只从本页（GitHub Releases）下载。

#### Android

1. 下载与手机匹配的 APK（不确定就选 `arm64-v8a`）。
2. 打开时如果提示「禁止安装未知应用」，按提示允许当前浏览器或文件管理器安装应用。
3. 如果 Google Play 保护机制提示「未知开发者」，点「更多详情」→「仍然安装」。

APK 使用 KiliKili 固定的发布密钥签名，以后的版本可以直接覆盖升级。

#### iOS / iPadOS

IPA 没有签名，不能直接安装，要用你自己的 Apple ID 侧载：

1. 在电脑上安装 [AltStore](https://altstore.io/)、[SideStore](https://sidestore.io/) 或
   [Sideloadly](https://sideloadly.io/) 之一，用它们把 IPA 装到设备上（会要求登录你的 Apple ID，
   签名在你自己的账号下完成）。
2. 打开前到「设置 → 通用 → VPN 与设备管理」，信任你的 Apple ID 对应的开发者。
3. iOS 16 及以上还要打开「设置 → 隐私与安全性 → 开发者模式」，按提示重启。

免费 Apple ID 签出来的应用 **7 天后失效**，需要用同一工具重新签名（AltStore / SideStore
可以自动续签）；一个免费账号同时最多装 3 个侧载应用。

#### macOS

1. 打开 dmg，把 KiliKili 拖进「应用程序」。
2. 第一次打开会提示「无法验证开发者」或「Apple 无法检查其是否包含恶意软件」：
   - 先关掉提示，到「系统设置 → 隐私与安全性」，在页面下方找到 KiliKili 的提示，点
     「仍要打开」，再输入密码确认；
   - 如果提示的是「KiliKili 已损坏，无法打开」，在「终端」里执行下面这行再打开：

     ```sh
     xattr -dr com.apple.quarantine /Applications/KiliKili.app
     ```

   这只会去掉下载时系统加的「隔离」标记，之后正常双击即可。

#### Windows

- **安装版**：运行 `setup.exe` 时如果出现蓝色的「Windows 已保护你的电脑」，点「更多信息」→
  「仍要运行」。
- **便携版**：解压前对 zip 点右键 →「属性」，勾选底部的「解除锁定」→「确定」，再解压；
  否则解压出来的每个文件都会带下载标记，启动时反复提示。

#### Linux（实验性）

```sh
sudo apt install ./KiliKili_linux_*_amd64.deb      # Debian / Ubuntu
sudo dnf install ./KiliKili_linux_*_amd64.rpm      # Fedora 等
chmod +x KiliKili_linux_*_amd64.AppImage && ./KiliKili_linux_*_amd64.AppImage
```

### 已知问题

- **Linux 版没有在真机上测试过**（见上）。安装包能构建、依赖也按程序实际链接的库生成，
  但我们没有 Linux 桌面环境，能用、不能用都欢迎反馈。
- **Linux 的 deb 包少一条依赖声明**：程序里的 `libdartjni.so` 需要 `libjvm.so`，deb 的
  `Depends` 里没有它。只要用不到这部分功能就没有影响。
- **默认主题色从蓝色改成了经典粉**。从旧版本升级上来、而且从没自己选过主题色的用户，
  打开后会发现界面变粉了，在设置里可以改回去。
- **番剧的片头片尾跳过会被 SponsorBlock 的分类开关连带关掉**：在设置里关掉 SponsorBlock
  的某些分类后，番剧的自动跳过也会一起失效（[#278](https://github.com/kakakakacat/KiliKili/issues/278)）。
- **自己发的短评在应用内删不掉**：发出去的短评不会出现在「我的评论」里，要删只能去
  官方客户端。上游版本也是这样。

问题反馈：<https://github.com/kakakakacat/KiliKili/issues>

### 许可证与来源

KiliKili 以 [GNU GPL v3.0](https://github.com/kakakakacat/KiliKili/blob/main/LICENSE) 发布，
完整源码在 <https://github.com/kakakakacat/KiliKili>（本版本对应的提交见 Release 页的 tag）。

本项目是 [PiliPlus](https://github.com/bggRGjQaUbCoE/PiliPlus) 的分支；PiliPlus 又源自
[PiliPalaX](https://github.com/orz12/PiliPalaX) 与 [PiliPala](https://github.com/guozhigq/pilipala)。
感谢这些项目的作者和所有贡献者。

KiliKili 是非官方客户端，与哔哩哔哩（上海宽娱数码科技有限公司）无关；视频、弹幕等内容的
版权归原作者与平台所有。本软件按「现状」提供，不附带任何担保（GPL v3.0 第 15、16 条）。

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
手工维护），Android 的版本名再追加 `-<commit 前 9 位>`。所以产物文件名形如
`KiliKili_android_0.1.0-xxxxxxxxx+NNNNN_arm64-v8a.apk`、`KiliKili_linux_0.1.0+NNNNN_amd64.deb`，
上面的表格因此只写前缀。

### 发版步骤

发布门槛与验收范围见 issue #242 第二节。签名材料与 secrets 见 [`SIGNING.md`](SIGNING.md)；
本版只有 Android（以及 Linux 的 rpm GPG 签名，如果 `LINUX_GPG_PRIVATE_KEY` 在）是签名的。

0.1.0 实际是这么发出去的，下一版照抄即可：

1. **先把 tag 打/移到要发的提交上。** 五个平台工作流里的
   `softprops/action-gh-release` 都没设 `target_commitish`，tag 不存在时它会把 tag
   建在 main 上。移动已存在的 tag 要 `git push --force origin refs/tags/<tag>`；
   仓库的三条 ruleset 都是分支级的，没有 tag 规则，这一步不会被挡。
2. **先建带正文的草稿 Release**，而不是发完再贴正文：

   ```sh
   gh release create v0.1.0 --draft --verify-tag --title v0.1.0 \
     --notes-file <本文「给用户看的部分」>
   ```

   `action-gh-release@v3` 在 `getReleaseByTag` 返回 404 时会回落到列表扫描，**能找到
   草稿**；更新时保留草稿标记与已有 body（工作流没传 `body`），上传完附件再
   `finalizeRelease` 把它转成公开。于是 Release 从公开的第一秒起就带着安装指引，
   不会有一段「有包没说明」的空窗。
   **别用 `gh api -X PATCH` 只改 `target_commitish`**：那会把草稿的 `tag_name` 换成
   `untagged-…` 占位值，等于脱离 tag，构建时会另建一个 Release。要改就连 `tag_name`
   一起 PATCH 回去。
3. **带 tag 触发构建**：`gh workflow run build.yml --ref <发布分支> -f tag=v0.1.0`
   （五端全开）。带 `tag` 的 Android 包走发布密钥、应用 ID 不带 `.dev`。
4. **产物抽查**：从 Release 页下回来，用 `tool/inspect-artifact.py` 核五端的 Rust 动态库
   与 deb 依赖，`sha256sum -c SHA256SUMS`，`gpg --verify` 四个 `.asc`，Android 用
   `apksigner verify --print-certs` 核证书身份。构建绿 ≠ 产物对（见 2026-11-10 复盘）。
