# 验证能力矩阵 — 五端各能验到什么程度

状态：**已实测**（硬件探测结果，非推断）
日期：2026-11-10
修订：2026-11-10 — §2 / §3.1 原先把「每个 PR 自动编译」与 Docker 多发行版矩阵
写成既有保障，与仓库里的 workflow 配置不符，已按实际配置更正；§2.1 补入五端产物核查的
实测结论（Android 与 macOS 为首次构建）；§5 勾掉四条已完成项
关联：RFC-0001 §9.3 / §16.7，ADR-004

本文回答一个 RFC 里此前含糊带过的问题：五个目标平台，**哪些能做真机验收、哪些只能做编译验证、哪些根本没覆盖**。结论直接影响 M4/M5 的准入范围，以及 ADR-004 能拿到多少实测数据。

## 1. 结论矩阵

| 平台 | 编译 | 真机 GUI | 硬解 | 结论 |
|---|---|---|---|---|
| macOS | Actions + 本机 | ✅ 本机 M4 | ✅ videotoolbox | **完整验收** |
| iOS | Actions | ✅ 真机 | ✅ videotoolbox | **完整验收**（签名见 §4） |
| Android | Actions | ✅ 真机 | ✅ mediacodec | **完整验收** |
| Windows | Actions | ✅ 家中设备 | ✅ d3d11va | **完整验收** |
| Linux | Actions | ❌ | ❌ | **best-effort，见 §3** |

对应 RFC §16.7 的修订：Slint 首批验收目标从「macOS + Android」扩大为 **macOS + Android + Windows**，iOS 紧随，Linux 单独降级。

## 2. 构建与验证的分工

编译打包全部放 GitHub Actions，本机只做 CI 证明不了的事。上游已配好五端 workflow，可直接继承：

| workflow | runner | 触发 | 产物 |
|---|---|---|---|
| `build.yml` (android) | `ubuntu-latest` | 手动 | APK，`--split-per-abi`；并按输入调用下面四个 |
| `ios.yml` | `macos-26` | 手动 / 被 `build.yml` 调用 | `--release --no-codesign` |
| `mac.yml` | `macos-26` | 手动 / 被 `build.yml` 调用 | .app |
| `win_x64.yml` | `windows-latest` | 手动 / 被 `build.yml` 调用 | innosetup 安装包 |
| `linux_x64.yml` | `ubuntu-latest` | 手动 / 被 `build.yml` 调用 | 需 `libmpv-dev` `libgtk-3-dev` `webkit2gtk-4.1` |

### 2.1 五端构建不在 push / PR 上自动运行

核对于 2026-11-10。上表五个 workflow 均只有 `workflow_dispatch`（后四个另有
`workflow_call`，由 `build.yml` 按平台开关调用）。`build.yml` 的 `pull_request`
触发自上游 `3ae3955f5` 起即为注释状态，本 fork 保持原样——这是明确保留的现状，
不是遗漏：macOS runner 的 10 倍倍率见下「成本约束」，且少改上游 workflow 可以
减少同步冲突。

push / PR 上自动运行的只有两个，都**不产出任何平台产物**：

| workflow | 触发路径 | 内容 |
|---|---|---|
| `flutter-test.yml` | `lib/**` `test/**` `pubspec.*` `tool/prepare-build.sh` | `ubuntu-latest`；`tool/prepare-build.sh Linux` 打补丁后跑 `flutter test` |
| `rust.yml` | `rust/**` | `cargo fmt` / `clippy` / `test`，外加五个 target 的交叉编译 |

**含义：一个只改 Dart 的 PR，在合并前没有任何环节验证过它还能编译出 app。**
需要这个保证时，必须在合并前手动 dispatch 对应 workflow 并把结论记进 PR。
自 `ee41df52c`（cargokit 接入 app 构建）之后尤其要注意：五个 app workflow 里
没有任何 rust/cargo 设置步骤，Rust 工具链完全依赖 runner 自带环境满足
`rust/rust-toolchain.toml` 钉的 1.97.0 与七个 target，Android 侧还要
`android.ndkVersion` 对得上。这条路径只能靠手动 dispatch 验证。

#### 已验证：五端产物均含 Rust 库（2026-11-10）

**构建日志不是证据。** `flutter build` 会吞掉插件的编译输出，Windows 那次
成功构建的日志里 `cargo` / `rustc` **零出现**。如果 cargokit 没被触发，构建
照样成功，产物只是少一个 Rust 动态库，`RustCore.isReady` 在用户机上恒为
false，四条 Rust 路径全部静默回退 Dart。**这种失败不会让任何 status check
变红**，只能查产物。

五端各手动 dispatch 一次，逐个解开产物核对：

| 平台 | run | 产物 | Rust 库路径 | 字节 |
|---|---|---|---|---|
| Windows | [42813991420](https://github.com/kakakakacat/KiliKili/actions/runs/42813991420) | portable zip | `bridge_flutter.dll` | 6,110,720 |
| Android | [42819477559](https://github.com/kakakakacat/KiliKili/actions/runs/42819477559) | arm64-v8a APK | `lib/arm64-v8a/libbridge_flutter.so` | 5,178,952 |
| iOS | [42802714845](https://github.com/kakakakacat/KiliKili/actions/runs/42802714845) | ipa | `Payload/Runner.app/Frameworks/bridge_flutter.framework/bridge_flutter` | 12,396,752 |
| macOS | [42819477559](https://github.com/kakakakacat/KiliKili/actions/runs/42819477559) | dmg | `KiliKili.app/Contents/Frameworks/bridge_flutter.framework/Versions/A/bridge_flutter` | 25,453,584 |
| Linux | [42822338947](https://github.com/kakakakacat/KiliKili/actions/runs/42822338947) | rpm | `/opt/KiliKili/lib/libbridge_flutter.so` | 见 RPM 文件列表 |

五个二进制内均含 `bili_api`、`rustc`、`frb_` 字符串，确为本项目 crate 编出。
**Android 与 macOS 是本仓库首次构建**，此前这两个 workflow 一次都没跑过。

播放器依赖同样在位：Windows `libmpv-2.dll` 37,981,184 字节；Android
`libmpv.so` 14,869,024；iOS `Mpv.framework` 与整套 ffmpeg framework；macOS
`Mpv.framework` 7,445,680。Linux 不随包分发 libmpv，由 rpmbuild 自动生成的
`Requires: libmpv.so.2()(64bit)` 声明为系统依赖（deb 的依赖声明有缺口，见 issue #36）。

Windows 安装包与 portable 同源：`windows/packaging/exe/inno_setup.iss` 的
`[Files]` 递归整个 Release 目录。这回答了 §5 第 3 条里「libmpv DLL 的安装包
分发方式需在 Windows 首次构建时确认」——无需额外处理。

##### 核对方法

每种产物的打开方式不同，记在这里以免下次重新摸索：

| 产物 | 方法 |
|---|---|
| apk / ipa | 本身就是 zip，直接解 |
| portable zip | 直接解 |
| dmg | 7-Zip 可解（本例 LZFSE 压缩）。Windows 上建符号链接会失败，实体文件在 `Versions/A/` 下，不影响核对 |
| rpm | header 未压缩，可直接解析 `BASENAMES`(1117) / `DIRNAMES`(1118) / `REQUIRENAME`(1049) 拿到文件列表与依赖，无需 rpm 工具 |

##### 这次核对发现的两个缺陷

- **Linux RPM 发布的是 debuginfo 子包**，11,289 字节，不含应用。构建全程绿。
  已修（#34），并给产物加了体积闸门（#35）。
- **deb 的 `Depends` 少声明 webkit2gtk 与 libsoup3**，干净系统上可能起不来。
  体积闸门拦不住这一类——包的体积是对的，错的是元数据。见 issue #36。

**仍未验证**：产物里有 Rust 库 ≠ 运行时加载成功。`RustCore.isReady` 在真机上
是否为 true、四条 Rust 路径在登录态下行为是否与 Dart 一致，都还没验（issue #25）。
本节只证明「编出来了、装进去了」。

**CI 能证明的**（前提是手动 dispatch 了对应 workflow）：编译通过、依赖完整、单测与协议 fixture 通过。
**CI 证明不了的**（全部是本项目的核心风险）：硬解、Slint + libmpv 同窗合成、PiP、后台音频、音频焦点、中文 IME、VoiceOver/TalkBack、渲染性能。CI runner 是虚拟化的，`hwdec` 必然回落软解。

性能基线（RFC §8.1 的八项指标）**必须固定在同一台机器采样**，CI runner 的性能抖动使其数据不可比。

### 成本约束
私有仓库下 macOS runner 为 **10 倍分钟倍率**，2000 分钟免费额度实际只够 200 分钟 macOS 构建。Rust 五端交叉编译会显著拉长构建时间。三个选项：仓库公开、macOS/iOS job 限定 tag 或手动触发、或将开发机注册为 self-hosted runner。

### Rust 进入后 CI 需补
现有 workflow 仅含 Flutter。需增加 Rust toolchain、各端 target（`aarch64-linux-android`、`aarch64-apple-ios`、`x86_64-pc-windows-msvc` 等）、NDK 配置，以及 `~/.cargo` 与 `target/` 缓存——无缓存时每次全量编译 Rust 会使构建时间失控。归入 `RUST-003`，与 workspace 同期完成。

## 3. Linux：明确降级为 best-effort

### 3.1 保留的保障

区分**已落地**与**计划中**，下表核对于 2026-11-10。

| 保障 | 状态 | 实际情况 |
|---|---|---|
| PR 上跑 Dart 单测 | ✅ 已落地 | `flutter-test.yml`，`ubuntu-latest`，命中 `lib/**` / `test/**` 等路径时触发。**只跑测试，不产出 Linux 产物** |
| PR 上跑 Rust 单测与交叉编译 | ✅ 已落地 | `rust.yml`，命中 `rust/**` 时触发；`bili-domain` / `bili-api` / `bili-protocol` / `bili-core` 的单测与 fixture 对照在 `ubuntu-latest` 上直接运行（非容器内） |
| Linux 产物构建 | ⚠️ 手动 | `linux_x64.yml` 只有 `workflow_dispatch` / `workflow_call`，见 §2.1 |
| Docker 多发行版矩阵（`ubuntu:22.04` / `ubuntu:24.04` / `debian:12` / `fedora:41`）验证 **glibc 版本与动态库符号兼容性** | ❌ 计划，尚未落地 | 仓库中没有对应 workflow 或 compose 文件。这是 Flutter Linux 最常见的发布事故（构建机能跑、用户机 glibc 过旧起不来），在该矩阵建起来之前，Linux 的兼容性保障实际为零 |

`bili-danmaku` 等 crate 尚未建立，上表只列已存在的。

### 3.2 放弃的保障
vaapi/vdpau 硬解、Wayland 与 X11 合成行为、渲染性能与帧耗时、音频（PulseAudio/PipeWire）、HiDPI 缩放。

### 3.3 Docker 的定位
Docker 是 **Linux 编译与兼容性闸门，不是 Linux 验收手段**。开发宿主为 macOS 时，Docker 运行在 Linux VM 中，无 GPU 直通、无显示输出；即便加 Xvfb + VNC，所得也仅为「窗口能创建、进程未崩溃」的冒烟级结论。
（注：Linux 宿主可用 `--device /dev/dri` 透传 GPU 并实测 vaapi；本项目宿主为 macOS，此路不通。）

本节描述的是该矩阵建成后的定位；按 §3.1，它目前仍是计划。

### 3.4 已评估并排除的硬件：家用软路由

实测探测结果（2026-11-10，SSH 直连）：

```
iStoreOS 25.12.5 / kernel 6.12.94 / x86_64
CPU   Intel Celeron J1900 @ 1.99GHz
GPU   Bay Trail "VALLEYVIEW" (PCI 0f31), display version 7.00
      i915 已加载，/dev/dri/{card0,renderD128} 存在
内存  3.8GB（可用 2.8GB）
磁盘  /mnt/sda4 12.1G，可用 5.7G
Docker 27.3.1，已承载 6 个生产容器
```

具备 `/dev/dri` 与充足磁盘，但 **J1900 为 2013 年 Bay Trail，Gen7 核显**，硬解能力与本项目需求严重错配：

| 编码 | Bay Trail 硬解 | 项目需求 |
|---|---|---|
| H.264 `avc1` | ✅ | 在用，退场中 |
| HEVC `hev1`/`hvc1` | ❌ | **主力** |
| AV1 `av01` | ❌ | **推进中** |
| Dolby Vision `dvh1` | ❌ | 高码率内容 |

对照 `lib/models/common/video/video_decode_type.dart` 枚举的四种格式，该硬件仅覆盖其中最不关键的一种。Intel HEVC 硬解自 Broadwell/Skylake 起，AV1 自 Gen12 起。驱动栈亦不同：Bay Trail 需老的 `i965-va-driver`，而现代 Linux 桌面使用 `intel-media-driver`（iHD），后者不支持 Bay Trail。

**排除理由**：在此机器上验出「vaapi 通路 OK」**不能**外推至真实 Linux 用户——他们的硬件走 HEVC/AV1 与完全不同的驱动路径。一个仅覆盖 25% 编码、驱动栈又不一致的验证，提供的信心高于其实际保障，属于有害验证。此外该设备承载家庭网络生产服务，不适合施加编译或长时间视频负载。

## 4. 遗留缺口

- **iOS 签名**：`ios/Runner.xcodeproj` 中 `DEVELOPMENT_TEAM = ""`，bundle id 仍为占位 `com.example.piliplus`。CI 与本机均只能 `--no-codesign` 构建；真机安装需配置 Apple Developer Team。（macOS 为 `CODE_SIGN_IDENTITY = "-"` ad-hoc 签名，不受影响。）
- **iOS 模拟器 runtime**：本机 `simctl list runtimes` 为空，模拟器冒烟测试需额外下载 runtime。
- **Linux GPU/合成**：如后续确有必要，比 Docker 可靠得多的路径是在 Windows 设备上安装双系统，或使用支持 GPU 分区直通的 VM。当前判断为投入产出比不足，暂不投入。

## 5. 跨机协作约定（macOS ↔ Windows）

开发在两台机器间通过 git 同步，需提前规避：

1. ~~**`.gitattributes` 需补全**~~ ✅ 已完成（`f113d9136`，现 49 行）。原问题：跨 macOS/Windows 换行符不统一会产生大量假 diff，并使 `cargo fmt --check` 与 `dart format` 在两端判定不一致导致 CI 红。
2. ~~**工具链版本两端钉死**~~ ✅ 已完成。Flutter 由 `.fvmrc` 锁 3.47.9；`rust/rust-toolchain.toml` 钉 1.97.0 + rustfmt/clippy + 七个 target；`rust/Cargo.lock` 已提交。
3. **Windows 侧已知障碍**（本机开发用，CI 不受影响）：需开启开发者模式（Flutter symlink 支持）；Rust `target/` 嵌套深，易触发 260 字符路径限制，仓库应置于短路径并开启长路径支持。~~libmpv DLL 的安装包分发方式~~ ✅ 已确认随包分发，见 §2.1。
4. ~~**`.gitignore` 补 Rust `target/`**~~ ✅ 已完成（`.gitignore:157` `/rust/target/`）。仍需遵守：**仅用 git 同步**，不要将仓库置于 iCloud/OneDrive/Dropbox；`build/`、`target/`、`.dart_tool/`、`Pods/` 等产物不得跨机同步。
