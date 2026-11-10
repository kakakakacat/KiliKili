# M0 基线 — KiliKili

状态：**构建已验证**（五端产物均在本机实际产出）
日期：2026-11-10
仓库：https://github.com/kakakakacat/KiliKili
上游：https://github.com/bggRGjQaUbCoE/PiliPlus

## 0. 锁定基线

```
UPSTREAM_COMMIT = f52160af0a026adfefaa4f9f99f0702b2cab0200
  bggRGjQaUbCoE/PiliPlus@main  2026-11-09T16:36:52+08:00
  "chore(grpc): upgrade proto (#3535)"
```

本 fork 领先上游 6 个提交，`git diff --stat` 证实**仅新增 docs，无一行代码改动**。故本次基线构建等价于构建未修改的上游原版。

## 1. 构建主机环境

| 项 | 值 |
|---|---|
| macOS | 27.3 (26C5035r)，arm64 |
| CPU / 内存 | Apple M4，10 核，32GB |
| Flutter | 3.47.9 · channel stable · `c197ca5199` |
| Engine / Dart | `89c39edfdf` / 3.13.7 · DevTools 2.60.0 |
| SDK 安装位置 | `/Users/dm/fvm/versions/3.47.9`（fvm 4.3.2 已关联本项目） |
| Xcode | 27.2 (27B52) · CocoaPods 1.17.0 |
| Android SDK | platform `android-37.0` · build-tools 37.0.0 · NDK 29.0.14206865 |
| JDK | OpenJDK 25.0.3（Android Studio JBR） |
| PowerShell | 7.6.9（**补丁脚本必需**，见 §3） |
| Gradle / Kotlin | 9.5.0 / 2.4.0 |

SDK 完整性：`flutter_macos_arm64_3.47.9-stable.zip` sha256 = `6545b85938745dc5070e9faf2adf70233c1811009d87fe6d27bfcda5a04b7698`，与官方 `releases_macos.json` 的 **arm64** 条目一致。
（注意：同版本的 x64 条目 sha256 为 `0dc38b40…a4fdcd`，两者不可混用。）

## 2. 构建结果

全部在本机实际执行并产出产物。

| 构建 | 结果 | 产物 | 体积 | 耗时 |
|---|---|---|---|---|
| `build apk --debug` | ✅ | `app-debug.apk` | 140.7 MB | 5m59s |
| `build apk --release --split-per-abi` | ✅ | `app-arm64-v8a-release.apk` | 24.1 MB | 2m37s（三 ABI 合计） |
| | | `app-armeabi-v7a-release.apk` | 24.1 MB | |
| | | `app-x86_64-release.apk` | 24.9 MB | |
| `build macos --release` | ✅ | `PiliPlus.app` | 136 MB | 2m35s |
| `build ios --release --no-codesign` | ✅ | `Runner.app` | 64 MB | Xcode 阶段 80.4s |

`flutter pub get` 成功，4 个 Git 依赖均按 `pubspec.lock` 的解析 SHA 拉取。

Windows / Linux 未在本机构建（无对应系统），依赖 CI；见 [VALIDATION-CAPABILITY.md](VALIDATION-CAPABILITY.md)。

## 3. 关键发现：本项目无法用原版 Flutter SDK 构建

首次用 stock SDK 构建**失败**，报错全部为 Flutter 框架私有类型未找到：`StandardBottomSheet`、`TabBarState`、`IndicatorPainter`、`DraggableScrollableSheetScrollPosition`、`BottomSheetState`、`TabBarLayoutCallback`。

原因：`lib/scripts/patch.ps1`（319 行 PowerShell）在构建前**直接修改 Flutter SDK 源码**。CI 中每个 workflow 均有 `Apply Patch` 步骤（`shell: pwsh`）。本地执行需补齐两个 CI 环境变量：

```sh
export FLUTTER_ROOT=/Users/dm/fvm/versions/3.47.9
export GITHUB_WORKSPACE=/Users/dm/KiliKili
pwsh -NoProfile -File lib/scripts/patch.ps1 <android|iOS|macOS|Linux|windows>
```

脚本修改**三处**位置，不止 Flutter SDK：

1. **Flutter SDK 仓库**（`$FLUTTER_ROOT`）：`git apply` 补丁、从上游 cherry-pick 指定 commit、revert 指定 commit。
2. **pub 缓存**：`material_ui` 1.10.0 与 `cupertino_ui` 1.1.6 的包目录。
3. **项目源码本身**（`$GITHUB_WORKSPACE`）：例如 `geetest_ios.patch` 会新建 `lib/pages/login/geetest/geetest_plugin.dart`（110 行），并改动 `pubspec.yaml`、`lib/` 下多个文件及 macOS 工程文件。

**第 3 点意味着运行构建会弄脏项目工作区。** CI 在一次性 checkout 中执行、从不提交这些改动；本地开发必须同样对待——构建后出现的 `lib/`、`pubspec.yaml`、`ios/`、`macos/` 改动是补丁脚本与工具链的产物，不是有意修改，**不得提交**。构建前后应比对 `git status` 并还原。

### 3.1 补丁数按平台不同（实测）

| 平台 | 补丁项数 |
|---|---|
| iOS | **37** |
| Android | **34** |
| macOS | **30** |

- iOS 独有：`bottom_sheet_ios_flutter`、`bottom_sheet_ios_piliplus`、`cupertino/bottom_sheet_ios_flutter`、`geetest_ios`、`material/bottom_sheet_ios_flutter_material`
- macOS 不含：`bottom_sheet_android`、`material/bottom_sheet_android`、`navigator`、`scroll_view`

### 3.2 覆盖范围（Android 34 项）

**框架层（25）**：`modal_barrier` `text_selection` `mouse_cursor` `image_anim` `layout_builder` `navigation_drawer` `popup_menu` `fab` `null_safety_for_selectable_region` `selectable_region` `editable_text` `text_field` `scroll_position` `scrollable` `scrollable_gesture` `draggable_scrollable_sheet` `scaffold` `text` `text_painter` `sliver` `refresh_indicator` `double_tap_gesture` `bottom_sheet_android` `scroll_view` `navigator`

**material 包（9）**：`modal_barrier_material` `navigation_drawer` `popup_menu` `fab` `text_field` `scaffold` `refresh_indicator` `tabs` `bottom_sheet_android`

### 3.2.1 pub cache 污染（全局、不可 git 复位）

补丁脚本修改的三处状态中，**只有两处能用 git 复位**：

| 位置 | 性质 | 复位方式 |
|---|---|---|
| Flutter SDK | git 仓库 | `git reset --hard` |
| 项目源码 | git 仓库 | `git restore` |
| **pub cache 的 `cupertino_ui` / `material_ui`** | **全局共享目录，非 git 仓库** | **只能删除后重新拉取** |

pub cache 的补丁**跨平台、跨会话累积残留**。实际踩到的故障：先执行 `patch.ps1 iOS`，之后复位 SDK 并执行 `patch.ps1 android`，SDK 已是 Android 状态而 pub cache 仍停在 iOS 状态——`cupertino_ui` 引用仅由 iOS 补丁提供的 `popGestureEnabled_`，编译报：

```
cupertino_ui-1.1.6/lib/src/route.dart:219:45: Error:
The getter 'popGestureEnabled_' isn't defined for the type 'PageRoute<T>'.
```

两点后果：

1. `~/.pub-cache` 是全局目录，该污染会**影响本机其他 Flutter 项目**。
2. CI 不会出现此问题（每个 job 均为全新环境），因此 **CI 绿不代表本机可构建**，反之亦然。

**切换目标平台必须使用 [`tool/prepare-build.sh`](../../tool/prepare-build.sh)**，它按正确顺序执行复位、清理 pub cache、重新拉取、打补丁四步。手工操作漏掉任一步都会产生难以定位的编译错误。

### 3.3 影响

- **锁定 Flutter 版本**不只是锁 3.47.9，而是锁 **3.47.9 + 对应平台补丁集**。升级 Flutter 时每个补丁都可能冲突。
- **本地五端构建必须串行**：补丁集按平台不同且共用同一份 SDK，切换平台需 `git reset --hard` 复位后重新打补丁。CI 各 job 有独立 runner 与独立 SDK，反而可并行——这是本机相对 CI 的实际劣势。
- **对 Phase 2 影响重大**：这些补丁代表上游 Flutter 满足不了、作者改框架才实现的交互行为（底部弹窗手势、TabBar 指示器、可拖拽面板、文本选择、滚动边界与手势识别）。换 Slint 后不会自动存在，需全部重新实现。RFC 风险 R9 低估了此项。详见 issue #11。

## 4. 源码审计结论

| 领域 | 证据 | 含义 |
|---|---|---|
| SDK | `pubspec.yaml`：Flutter 3.47.9，Dart ≥3.13.0 | 基线构建需精确匹配 |
| 视频 | `media_kit` 1.1.11；`media_kit_video` 1.2.5；平台 Git override | fork 行为需在替换 Player Adapter 前审计（见 §5） |
| 状态 | `lib/utils/storage.dart` 使用 `hive_ce` | Rust SQLite 导入前需 Dart exporter |
| 存储 box | `userInfo` `localCache` `setting` `historyWord` `video` `watchProgress`，可选 `reply`，另有 `lib/utils/accounts.dart` 的 `account` | 敏感账号凭据须与普通设置分离 |
| 设置导出 | `GStorage.exportAllSettings()` 仅导出 `setting` 与 `video` | 现有导出**不是**全量数据迁移 |
| HTTP | `lib/http/init.dart`：Dio、账号管理、HTTP/2 与 cookie | 协议/认证边界需在 Rust 迁移前审计 |
| 下载 | `lib/services/download/`，元数据 `entry.json` / `index.json` | 保留既有下载；不可假设已有 FFmpeg mux |

### 4.1 Git 依赖解析 SHA（来自 pubspec.lock）

| 依赖 | 来源 ref | 解析 SHA |
|---|---|---|
| media_kit + Android/iOS/Windows video libs | `My-Responsitories/media-kit@native` | `73771ec38176be2d984a3049c28177bce23b54a0` |
| media_kit_video | `bggRGjQaUbCoE/media-kit@native` | `85f0a69ea66b100882b27a3375c72e5155375343` |
| audio_service | `bggRGjQaUbCoE/audio_service@dev` | `9f91eb8d6aa0905661ab3914ebdd3c9fb5c37cc2` |
| canvas_danmaku | `bggRGjQaUbCoE/canvas_danmaku@main` | `435063d093bdc0dde70616a2c30552411c9e13e0` |

`pubspec.yaml` 引用分支，`pubspec.lock` 提供具体 revision。基准对比完成前不要随意升级依赖。

## 5. 播放链路审计（RUST-002，详见 issue #2）

1. **media-kit fork 是结构性裁剪**：`player.dart` 全文仅 `typedef Player = NativePlayer;`，`src/player/web/` 已删除。自有 API `Player.create()`、`setMediaHeader()`。Phase 1 Adapter 要对齐的是 fork 私有契约而非上游公开契约。
2. **DASH 分轨用 mpv `edl://` 拼接**成单一伪 URL，非双播放器同步。AVPlayer/Media3 无等价物 → 换后端须重做轨道合成路径（ADR-004 关键输入）。
3. **请求头在播放器层注入**：`setMediaHeader(userAgent: BrowserUa.pc, referer: HttpString.baseUrl)`。ADR-003 候选接口目前缺此项。
4. **错误恢复靠匹配 mpv/ffmpeg 日志字符串**，无稳定错误码可映射。Player Port 需定义自有错误分类枚举并建 fixture 回归。
5. **弹幕时钟已符合单一时钟原则**：`lib/pages/danmaku/view.dart` 消费播放器 position 流（100ms 分桶），无独立计时器。但事件流**完全没有 `session_id`/`generation`**，过期事件靠 `_playerCount == 0` 零散兜底——这是 ADR-003 要补的缺口。

## 6. 已记录的构建期警告

- **media-kit 系插件未适配 Swift Package Manager**（`media_kit_libs_ios_video`、`media_kit_native_event_loop`、`media_kit_video`）。Flutter 提示将来版本会变为错误。这些正是定制 fork 中的包，上游不会适配，需自行处理。
- macOS：`connectivity_plus.o` 的 ld 地址歧义警告；`Flutter Assemble` 的 Run Script 未声明 output 导致每次全量重跑（上游已知问题，拖慢增量构建）。
- Android：`AndroidHelper.java` 使用已过时 API 与未经检查的操作。
- `flutter doctor` 两项非阻塞警告：部分 Android 许可证未接受；iOS 27.2 模拟器未安装。

### 6.1 构建会改写受版本控制的文件（需注意）

`flutter build ios` 在本机执行后修改了两个已跟踪文件，**本次提交已将其还原**，但行为本身须记录：

- `ios/Podfile` 与 `macos/Podfile`：`platform` 由 **14.0 自动改写为 15.0**。这抬高了项目最低部署版本，是实质性变更而非格式调整。
- `ios/Podfile.lock`（净减 155 行）、`macos/Podfile.lock`（净减 94 行）：多数插件已迁移至 Swift Package Manager，不再经由 CocoaPods 解析。
- `ios/Runner.xcodeproj`、`macos/Runner.xcodeproj`、xcscheme、xcworkspace：写入 `FlutterGeneratedPluginSwiftPackage` 的 package 引用。
- `macos/Flutter/Flutter-Debug.xcconfig`、`Flutter-Release.xcconfig`。

叠加 §3 所述补丁脚本对项目源码的改动，一次完整的本机五端构建会弄脏 15 个以上受版本控制的文件。

含义：本机构建与 CI 构建会产生不同的工程文件状态。任何构建流程文档都应说明这两个文件在构建后需检查，避免把工具链的自动改写当作有意变更合入。最低 iOS 版本若要从 14.0 提升到 15.0，应是一次显式决策并记录，而非由构建命令代为决定。

## 7. 可用验证设备（doctor 实测）

| 平台 | 系统版本 | 架构 | 类型 |
|---|---|---|---|
| Android | 14 (API 34) | android-arm64 | 实机 |
| iOS | 27.2 (24B5089g) | ios | 实机 |
| macOS | 27.3 (26C5035r) | darwin-arm64 | 本机 |

（设备序列号 / UDID 属标识信息，不记入公开文档；本机 `flutter devices` 可查。）

## 8. 待完成项

- [ ] 真机安装与冒烟：首页→详情→播放→弹幕、登录、搜索、断网恢复
- [ ] iOS 签名：`DEVELOPMENT_TEAM` 为空、bundle id 仍为占位 `com.example.piliplus`，真机安装需配置
- [ ] 枚举自定义 Hive TypeAdapter 与账号敏感字段（仅用脱敏 fixture）
- [ ] 验证既有下载格式、重组与后台执行行为
- [ ] 测量基线启动、播放、CPU、RSS 与弹幕指标（须固定本机采样）
- [ ] Windows / Linux CI 构建验证

## 9. 初始决策

1. 保留 Phase-1 Flutter Player Adapter；ADR-003/004 前不重写原生播放后端。
2. 以播放器实际位置为唯一事实时钟；Rust 拥有命令编排与派生快照，而非独立时间线。
3. 采用 Dart 侧 Hive 导出 + Rust SQLite 导入，验证通过前保留原数据。
4. M0 Slint Spike 为 Phase-2 Go/No-Go 门槛；Phase-1 Flutter + Rust Core 保持独立可发布。
5. **新增**：Flutter SDK 补丁是构建的必要前置步骤，任何构建文档与 CI 变更都不得遗漏。
