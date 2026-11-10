# 验证能力矩阵 — 五端各能验到什么程度

状态：**已实测**（硬件探测结果，非推断）
日期：2026-11-10
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

| workflow | runner | 产物 |
|---|---|---|
| `build.yml` (android) | `ubuntu-latest` | APK，`--split-per-abi` |
| `ios.yml` | `macos-26` | `--release --no-codesign` |
| `mac.yml` | `macos-26` | .app |
| `win_x64.yml` | `windows-latest` | innosetup 安装包 |
| `linux_x64.yml` | `ubuntu-latest` | 需 `libmpv-dev` `libgtk-3-dev` `webkit2gtk-4.1` |

**CI 能证明的**：编译通过、依赖完整、单测与协议 fixture 通过。
**CI 证明不了的**（全部是本项目的核心风险）：硬解、Slint + libmpv 同窗合成、PiP、后台音频、音频焦点、中文 IME、VoiceOver/TalkBack、渲染性能。CI runner 是虚拟化的，`hwdec` 必然回落软解。

性能基线（RFC §8.1 的八项指标）**必须固定在同一台机器采样**，CI runner 的性能抖动使其数据不可比。

### 成本约束
私有仓库下 macOS runner 为 **10 倍分钟倍率**，2000 分钟免费额度实际只够 200 分钟 macOS 构建。Rust 五端交叉编译会显著拉长构建时间。三个选项：仓库公开、macOS/iOS job 限定 tag 或手动触发、或将开发机注册为 self-hosted runner。

### Rust 进入后 CI 需补
现有 workflow 仅含 Flutter。需增加 Rust toolchain、各端 target（`aarch64-linux-android`、`aarch64-apple-ios`、`x86_64-pc-windows-msvc` 等）、NDK 配置，以及 `~/.cargo` 与 `target/` 缓存——无缓存时每次全量编译 Rust 会使构建时间失控。归入 `RUST-003`，与 workspace 同期完成。

## 3. Linux：明确降级为 best-effort

### 3.1 保留的保障
- Actions `ubuntu-latest` 保证每个 PR 编译通过
- Docker 多发行版矩阵（`ubuntu:22.04` / `ubuntu:24.04` / `debian:12` / `fedora:41`）验证 **glibc 版本与动态库符号兼容性**——Flutter Linux 最常见的发布事故是构建机能跑、用户机 glibc 过旧起不来
- Rust Core 无 UI 依赖的 crate（`bili-domain`、`bili-api`、`bili-danmaku` 等）在容器内跑单测与 fixture 对照

### 3.2 放弃的保障
vaapi/vdpau 硬解、Wayland 与 X11 合成行为、渲染性能与帧耗时、音频（PulseAudio/PipeWire）、HiDPI 缩放。

### 3.3 Docker 的定位
Docker 是 **Linux 编译与兼容性闸门，不是 Linux 验收手段**。开发宿主为 macOS 时，Docker 运行在 Linux VM 中，无 GPU 直通、无显示输出；即便加 Xvfb + VNC，所得也仅为「窗口能创建、进程未崩溃」的冒烟级结论。
（注：Linux 宿主可用 `--device /dev/dri` 透传 GPU 并实测 vaapi；本项目宿主为 macOS，此路不通。）

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

1. **`.gitattributes` 需补全**。当前仅 `*.patch text eol=lf`。跨 macOS/Windows 时换行符不统一会产生大量假 diff，并使 `cargo fmt --check` 与 `dart format` 在两端判定不一致导致 CI 红。
2. **工具链版本两端钉死**。Flutter 已由 `.fvmrc` 锁定 3.47.9；Rust 需在 workspace 根添加 `rust-toolchain.toml`（channel + components + targets）。`Cargo.lock` 必须提交（应用非库）。
3. **Windows 侧三个已知障碍**：需开启开发者模式（Flutter symlink 支持）；Rust `target/` 嵌套深，易触发 260 字符路径限制，仓库应置于短路径并开启长路径支持；libmpv DLL 的安装包分发方式需在 Windows 首次构建时确认，不留至发布前。
4. **仅用 git 同步**，不要将仓库置于 iCloud/OneDrive/Dropbox。`build/`、`target/`、`.dart_tool/`、`Pods/` 等产物不得跨机同步。现有 `.gitignore` 为 Flutter 标准模板，**Rust `target/` 需在建 workspace 时补入**。
