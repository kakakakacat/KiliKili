# 验证能力矩阵 — 五端各能验到什么程度

状态：**已实测**（硬件探测结果，非推断）
日期：2026-11-10
修订：2026-11-10 — §2 / §3.1 原先把「每个 PR 自动编译」与 Docker 多发行版矩阵
写成既有保障，与仓库里的 workflow 配置不符，已按实际配置更正；§2.1 补入五端产物核查的
实测结论（Android 与 macOS 为首次构建）；§5 勾掉四条已完成项
修订：2026-11-11 — `rust.yml` 的交叉编译矩阵拆分：PR 只跑 x86_64 的 Linux/Windows，
Apple 与 Android 改为手动 dispatch（理由见 §2.1）；「成本约束」按仓库已公开的事实重写；
「Rust 进入后 CI 需补」已完成，改为记录现状
修订：2026-11-11 — `flutter-test.yml` 新增 bridge job：编出原生库、打开开关，
在 CI 里真正跨 FFI 跑一遍（§3.1 的 Dart 单测那一行改写）。在此之前桥接层从未被
执行过一次；`bili-danmaku` 已建立，§3.1 末尾那段随之更正
修订：2026-11-12 — **PR 上的回归检查暂停**（issue #114）：`ci.yml` 的
`pull_request:` 触发注释掉、ruleset「main regression gate」改为 `disabled`；
§2.2 按此重写，下文凡写「每个 PR」的地方当前只在 push/定时/手动运行时成立
修订：2026-11-11 — `ci.yml` 成为统一 PR/main 入口：每次运行 Rust、Dart 基线、
真实 FFI 和 Android 产物检查；定时/按需调用五端打包工作流。下文 §2.1 保留
原先手动流程的历史说明；当前触发与发布门槛以 [QA-001](../qa/RELEASE-VALIDATION.md)
和 `ci.yml` 为准。
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
| `flutter-test.yml` | `lib/**` `test/**` `pubspec.*` `tool/prepare-build.sh` `rust/crates/bili-api/tests/fixtures/**` | `ubuntu-latest`；`tool/prepare-build.sh Linux` 打补丁后跑 `flutter test` |
| `rust.yml` | `rust/**` | `cargo fmt` / `clippy` / `test`，外加 **x86_64 Linux 与 Windows** 两个 target 的交叉编译 |

`flutter-test.yml` 把 fixture 目录也列为触发路径，是因为 Dart 侧有测试读 `rust/`
下的 golden（核对它们确实出自基线）；只改 fixture 时若不触发，唯一检查它们的
job 就跑不到。

##### Apple 与 Android 的交叉编译为何不在 PR 上

2026-11-11 起，`aarch64-apple-darwin` / `aarch64-apple-ios` / `aarch64-linux-android`
三个 target 移到 `rust.yml` 的 `cross-build-dispatch` job，只在手动 dispatch 时运行。

直接起因：PR #41 上 `aarch64-apple-darwin` 连续两次在 **15 分钟**后失败，错误是
"The job was not acquired by Runner of type hosted even after multiple attempts"
——GitHub 的 macOS arm64 hosted runner 容量不足，job 根本没有启动。它是那个 PR
唯一的红灯，而关于代码的信息量为零。这种红灯重复几次就会训练出「红了也先合」的
习惯，比没有这个检查更糟。

判断依据是**交叉编译能证明什么**：它只回答「代码里有没有写死某个平台」，而
x86_64 的 Linux + Windows 已经能回答这个问题，且一分钟内结束。真正的平台级断裂
（链接器、NDK、cargokit 把 Rust 接进 app 构建）本来就不是 `cargo build --target`
能发现的，要靠五个 app workflow——而它们一直是手动 dispatch。

`rust/rust-toolchain.toml` 仍钉着七个 target，手动跑与本地跑的覆盖面不变。
**建议 dispatch 的时机**：改动触及 FFI/桥接层、升级 FRB 或工具链、发版前。

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
`Requires: libmpv.so.2()(64bit)` 声明为系统依赖（deb 侧当时有缺口，已修，见下）。

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
  体积闸门拦不住这一类——包的体积是对的，错的是元数据。已修（issue #36 / PR #38），
  并在 #39 里改为构建期用 `dpkg-shlibdeps` 从 ELF 生成，不再手工维护；缺关键库则
  构建失败。**未在干净系统上实际安装验证**，结论来自 ELF 与包元数据的静态比对
  （本机无 Linux GUI 环境，见 §3）。

**仍未验证**：产物里有 Rust 库 ≠ 运行时加载成功。`RustCore.isReady` 在真机上
是否为 true、四条 Rust 路径在登录态下行为是否与 Dart 一致，都还没验（issue #25）。
本节只证明「编出来了、装进去了」。

**CI 能证明的**（前提是手动 dispatch 了对应 workflow）：编译通过、依赖完整、单测与协议 fixture 通过。
**CI 证明不了的**（全部是本项目的核心风险）：硬解、Slint + libmpv 同窗合成、PiP、后台音频、音频焦点、中文 IME、VoiceOver/TalkBack、渲染性能。CI runner 是虚拟化的，`hwdec` 必然回落软解。

性能基线（RFC §8.1 的八项指标）**必须固定在同一台机器采样**，CI runner 的性能抖动使其数据不可比。

### 成本与可用性约束

本节原先按私有仓库写（macOS runner 10 倍分钟倍率、2000 分钟免费额度只够 200
分钟 macOS 构建）。**仓库现已公开**，Actions 分钟数不再计费，当初列出的三个
选项里「仓库公开」已经发生。

剩下的约束不是钱，是**可用性**：GitHub 的 macOS arm64 hosted runner 容量紧张，
排不到时 job 会在 15 分钟后以 "not acquired by Runner" 失败。已据此把 Apple 与
Android 的 Rust 交叉编译改为手动 dispatch（§2.1）；五个 app workflow 本就是手动。
若将来需要稳定的 macOS CI，剩下的路是把开发机注册为 self-hosted runner。

### Rust CI 现状（原「进入后需补」，已完成）

`RUST-003`（issue #3，已关闭）落地了 `rust.yml`：工具链由 `rust-toolchain.toml`
决定（不在 workflow 里重复指定，避免两处版本漂移）、`Swatinem/rust-cache` 缓存
`~/.cargo` 与 `target/`、Android 侧配 NDK r29 的 clang 作为链接器（`rustup target`
只给标准库，不给链接器与 C 编译器，而 FRB 依赖链含 C 代码）。NDK 刻意不用
`local-cache`，否则恢复出的 NDK 残缺、故障只在第二次之后出现。

### 2.2 回归门（`ci.yml` 的 `regression gate`）

> **当前状态（2026-11-12 起，issue #114）：PR 上不跑。** 重构期一条接口一个
> PR，每个 PR 等一次完整回归（最慢的 Android 打包 11–15 分钟）的成本超过了
> 收益，所有者决定暂停。两处同时改：`ci.yml` 的 `pull_request:` 触发注释掉、
> ruleset「main regression gate」（id 27013798）`enforcement` 改成 `disabled`。
>
> **为什么必须两处一起改**：ruleset 要的 `regression gate` 只由 `ci.yml` 产出。
> 只注释触发而留着 ruleset，检查永不上报，PR 全部永久 `BLOCKED`；而个人账号
> 仓库给 ruleset 加 bypass 会 422，没有退路。反之只停 ruleset 则白等。
>
> 门本身、`gate` job 和下面这些判断**都没有删**：`push: main`、每日 02:17 定时、
> `workflow_dispatch` 仍然跑完整回归。恢复 = 取消那行注释 + `enforcement` 改回
> `active`，issue #114 在恢复前保持 open。期间要在合并前验某个分支：
> `gh workflow run "Regression CI" --ref <分支>`。
>
> 代价：坏改动的发现点从合并前后移到合并后的 push 运行或次日定时运行。

2026-11-11 加。仓库 ruleset「main regression gate」把一个名为
`regression gate` 的状态检查设成合并到 main 的必需项。产出这个检查的是
`.github/workflows/ci.yml`（工作流名 **Regression CI**）里的 `gate` job——
**job 的 `name:` 就是 ruleset 认的那个检查名**，改掉它 ruleset 就对不上，
所有 PR 会永久停在 `BLOCKED`。这是本机制唯一会静默失效的方式。

门本身**只聚合、不跑测试**：`needs: [rust, flutter, proto, android, ios, macos,
windows, linux]`，再逐个核对 `needs.*.result`。真正的回归在被聚合的工作流
里，在门里重跑一遍只会让同一份测试跑两次。`rust.yml` 与 `flutter-test.yml`
为此改成纯 `workflow_call`，只由 ci.yml 调用，不再自己监听事件。

四处判断值得记住：

- **`if: always()` 不能去掉。** 默认语义下任一 `needs` 失败就会把 `gate`
  置为 skipped，必需检查于是永远不上报——失败的 PR 会卡在 pending，比通过的
  PR 更难看出原因。
- **被聚合的两条可复用工作流刻意不加 `paths` 过滤。** 必需检查一旦在某些 PR
  上缺席就是永久 pending，只改 fixture 或 workflow 的 PR 会卡死。ci.yml 里
  那条注释说的就是这件事。
- **rust / flutter / proto / android 四项强制 `success`。** ios / macos /
  windows / linux 带 `if: github.event_name == 'schedule' ||
  inputs.full_platforms`，平时是 skipped；门只在 `full_platforms` 为真时才
  核对这四项。
- **图表 PR 靠创建事件跑回归门**（暂停期间不适用：没有必需检查要等，
  auto-merge 直接合）。`readme-stats.yml` 用 `GITHUB_TOKEN`
  推送 `chore/readme-stats` 时，防递归机制不会触发 `push` 工作流；PR 创建
  事件会触发 `ci.yml`。仓库须允许 Actions 创建 PR；但即使 fork PR 审批策略
  已设为 `first_time_contributors_new_to_github`，#96 的机器人创建事件仍以
  `action_required` 结束，auto-merge 等不到 `regression gate`。此时须人工批准
  该运行，或由仓库账号推送新提交触发 PR 同步事件。#82 的成功运行由仓库账号
  触发，不能证明机器人创建事件免审批。

**等 CI 的时间尺度**（恢复 PR 触发后适用）：每个 PR 都会打一次 Android APK（`Android package and
native library`，`timeout-minutes: 60`），是全流程最慢的一项，实测约 15
分钟。`gh pr checks <n> --watch` 会超过 600 秒，要放后台跑。判断一个 PR 能
不能合看 `mergeable=MERGEABLE` 且 `mergeStateStatus=CLEAN`；只要
`regression gate` 还没上报就是 `BLOCKED`，那通常只代表 Android 那项还没跑完。

## 3. Linux：明确降级为 best-effort

### 3.1 保留的保障

区分**已落地**与**计划中**，下表核对于 2026-11-10。

| 保障 | 状态 | 实际情况 |
|---|---|---|
| PR 上跑 Dart 单测 | ✅ 已落地 | `flutter-test.yml` 的 `test` job，`ubuntu-latest`，命中 `lib/**` / `test/**` / `rust/**` 等路径时触发。**只跑测试，不产出 Linux 产物**。该 job 不加载原生库，`RustCore.isReady` 恒为 false，因此它验的是**开关关闭**下的 Dart 原路径、纯算法单测，以及 golden 的裁判（「这份期望值确实是基线会得出的结果」） |
| PR 上真正跨 FFI 跑一遍 | ✅ 已落地（2026-11-11） | 同一 workflow 的 `bridge` job：`cargo build -p bridge_flutter --release` 之后打开开关跑 `test/bridge`。在它之前，FRB 的类型映射、弹幕会话的生命周期、失败降级在 CI 里一次也没被执行过——Rust 单测证明引擎算得对，这个 job 证明那个结果能原样回到 Dart。原生库没加载起来时**直接失败而不是跳过**（理由见 `test/bridge/bridge_env.dart`：这组测试最可能的退化就是悄悄变成 0 个测试） |
| PR 上跑 Rust 单测与交叉编译 | ✅ 已落地（交叉编译部分收窄） | `rust.yml`，命中 `rust/**` 时触发；`bili-domain` / `bili-api` / `bili-protocol` / `bili-core` 的单测与 fixture 对照在 `ubuntu-latest` 上直接运行（非容器内）。交叉编译在 PR 上只剩 x86_64 Linux/Windows，Apple 与 Android 改为手动，见 §2.1 |
| Linux 产物构建 | ⚠️ 手动 | `linux_x64.yml` 只有 `workflow_dispatch` / `workflow_call`，见 §2.1 |
| Docker 多发行版矩阵（`ubuntu:22.04` / `ubuntu:24.04` / `debian:12` / `fedora:41`）验证 **glibc 版本与动态库符号兼容性** | ❌ 计划，尚未落地 | 仓库中没有对应 workflow 或 compose 文件。这是 Flutter Linux 最常见的发布事故（构建机能跑、用户机 glibc 过旧起不来），在该矩阵建起来之前，Linux 的兼容性保障实际为零 |

上表只列已存在的 crate。弹幕的**解码**落在 `bili-api`、实体在 `bili-domain`，`bili-danmaku`（2026-11-11 建立）只装引擎——标记自己、合并、过滤、分桶（RFC §4 的注里记了这条分界线）。它们的单测与 golden 对照包含在上面第二行里，跨 FFI 的那一半在 `bridge` 那行。

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


### gRPC 描述符与传输验证（#133，2026-11-13）

`proto.yml` 在 Ubuntu 独立运行生成器格式检查、描述符校验测试和
`tool/proto-generate.sh --check`，其结果由 regression gate 强制聚合。
不增加五端构建依赖，不恢复 PR 触发或 ruleset。描述符编译使用 compile_fds，
没有 protoc 外部二进制要求；普通 app 构建只编译签入的 prost 生成代码。

Rust 测试覆盖帧阈值/压缩、截断、缺失状态头、嵌套业务码、metadata 五头、
真实 reqwest 字节响应与读写超时重试差异。Flutter bridge 测试新增 metadata、
分段字节、typed 字幕、损坏响应错误值与嵌套业务码，替换旧的字幕解析开关测试。
离线 Dart/Rust 解帧 parity 不 import lib/，对照同一真实脱敏帧的载荷。
这些能力不替代登录态、系统代理、证书与设备上的人工验证。


本机 macOS 的 Rust 1.97 release 动态库在默认 `strip=debuginfo` 后出现
`mis-aligned LINKEDIT string pool`，dyld 在进入任何业务测试前拒绝加载。
同份代码用 `CARGO_PROFILE_RELEASE_STRIP=none cargo build -p bridge_flutter --release`
可加载，故本轮真实 FFI 验证使用该临时产物。仓库 release profile 未改动；
此记录只限定本机 dylib 验证，不推断五端 app 打包结果。工具链升级另见 #134。

搜索发现的 bridge 回归（#120）经本地 HTTP 服务核对 App 请求路径、
签名字段和登录凭据，并跨 FFI 检查推荐词及业务 `-400` 错误值。
Rust 取数测试另用独立计算的签名快照锁定匿名、空凭据和登录参数集。
这不能替代设备上 App 主机连通性与真实账号的人工验证。
