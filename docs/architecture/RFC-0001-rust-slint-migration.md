# PiliPlus Rust + Slint 跨平台重构：技术方案与目标规划

> **状态**：RFC v0.2 / Review 意见已吸收，待 ADR 验证  
> **版本**：v0.2  
> **日期**：2026-11-10  
> **原始项目**：https://github.com/bggRGjQaUbCoE/PiliPlus  
> **目标平台**：iOS、Android、macOS、Windows、Linux；**不支持 Web**  
> **核心原则**：先 Rust 化可复用业务核心，再 Slint 替换 UI，最后集中做性能与稳定性；每个阶段保持可运行、可回退。

---

## 0. 执行摘要 / Review 结论

本项目不是将 Dart 代码机械翻译成 Rust，而是基于 PiliPlus 功能基线重构跨平台客户端。

- **阶段 1：Flutter + Rust Core**。保持 Flutter UI 和现有播放器，逐模块迁移业务逻辑至独立 Rust Workspace，通过 `flutter_rust_bridge` 通信。
- **阶段 2：Slint + Rust Core**。另建 Slint 客户端，复用既有 Core，按页面和平台迁移，最终移除 Flutter 运行时。视频播放采用平台适配层，不能假设 Slint 能直接嵌入所有视频表面。
- **阶段 3：性能优化 + Bug 修复**。在阶段 1/2 持续保证质量的基础上，集中优化视频、弹幕、图片、滚动、启动时间、CPU、内存和耗电。

**M0 阻断门槛**：阶段 1 启动时限时验证 Slint 视频合成、弹幕覆盖、长列表、富文本、IME 和无障碍。未通过仅阻断 Phase 2 Slint 完整替换；Phase 1 Flutter + Rust Core 仍可独立发布。

## 1. 背景和现状

PiliPlus 是 Flutter 第三方 Bilibili 客户端。当前 `pubspec.yaml` 包含 `media_kit`、`dio`、`protobuf`、`hive_ce`、`get` 等依赖，且对 `media_kit` 等采用 Git override；因此播放器路径不能仅依据上游标准插件行为推断。首个交付应锁定 upstream commit 和依赖版本，并绘制真实依赖图。

### 1.1 目标

1. 以 Rust 共享业务核心覆盖五个平台，UI 和平台细节不侵入 Core。
2. **保留功能和数据兼容性**，迁移期间保持可用的 Flutter 版本。
3. 第二阶段采用 Slint 声明式 UI，形成一套跨平台设计和组件体系。
4. 平台硬解、系统音频/画中画、窗口能力通过可替换适配器实现，不追求所有依赖都为纯 Rust。
5. 建立可重复的功能、兼容性、性能测试与 CI 构建链路。

### 1.2 非目标

- 不做 Web/WASM、不实现网页服务端。
- 不自研 H.264/HEVC/AV1 解码器，不以纯 Rust 取代平台硬件解码。
- 不在第一阶段重写完整 UI、动画和播放渲染。
- 不以 Rust 代码行占比作为成功指标。
- 不承诺规避 iOS 侧载签名有效期、第三方 API 授权或平台审核要求。

### 1.3 边界与假设

- 优先保留 PiliPlus 功能而非界面完全一致；对每项差异记录验收。
- 开发基线：macOS Apple Silicon 开发主机；Android 实机、iPhone、Windows/Linux 验证环境需在里程碑前明确。
- 因上游经常更新，迁移以**固定 commit**为基线，后续通过显式同步窗口合并必要修复。
- GPL-3.0 相关义务须纳入发布前法律/许可证检查，复用代码和资源时保留来源声明。

## 2. 目标架构

```text
Stage 1                               Stage 2/3
┌────────────────────┐                ┌────────────────────┐
│ Flutter UI          │                │ Slint UI            │
│ 现有 Flutter Player │                │ 页面/组件/主题      │
└──────────┬─────────┘                └──────────┬─────────┘
           │ flutter_rust_bridge                 │ Rust callbacks / models
           ▼                                     ▼
┌─────────────────────────────────────────────────────────┐
│ Rust Application / Domain                               │
│ UseCases | Repositories | PlayerController | Events     │
├─────────────────────────────────────────────────────────┤
│ API / Auth / Models / Danmaku / Download / Cache / DB   │
├─────────────────────────────────────────────────────────┤
│ Ports: Http | Storage | Clock | Player | SecureStore    │
└────────────────────────────┬────────────────────────────┘
                             │ platform adapters
              ┌──────────────┼────────────────┐
              ▼              ▼                ▼
         iOS/Android     macOS/Win/Linux   Test adapters
         AVPlayer/Media3      libmpv        fake/in-memory
```

### 2.1 分层职责

| 层 | 内容 | 禁止依赖 |
|---|---|---|
| `domain` | 业务实体、规则、错误、UseCase、Repository Trait | Flutter/Slint、特定平台 SDK |
| `data` | HTTP、序列化、API DTO、会话、数据库、缓存 | UI 组件 |
| `application` | 页面所需聚合查询、任务取消、并发和事件流 | Flutter Widget/Slint 具体类型 |
| `bridge_flutter` | Rust API 转 Dart、事件订阅、生命周期管理 | 具体 Bilibili 协议细节 |
| `ui_slint` | Slint 页面、主题、输入事件、可访问性 | API 底层细节 |
| `platform-*` | 视频、Keychain/Keystore、文件、分享、通知、窗口 | 业务页面结构 |

### 2.2 设计约束

- Core 对外暴露**粗粒度异步 API**，避免每帧/每条弹幕跨 FFI；高频数据经批处理或原生共享结构传递。
- 事件需携带 `request_id`/`generation`，路由切换、Seek、搜索输入改变时支持取消和丢弃过期结果。
- Core 的错误统一映射为机器可识别的错误码、用户提示与诊断字段；日志中不得包含 Cookie/Token。
- 网络和存储的所有平台差异封装为 trait 或平台 Adapter。
- 用户数据迁移必须可回滚，避免一次性删除旧数据库或缓存。

## 3. 推荐技术栈

| 关注点 | 阶段 1 | 阶段 2/3 | 决策说明 |
|---|---|---|---|
| UI | Flutter 原界面 | Slint | 迁移中同时保留两种 Shell |
| Dart/Rust 桥接 | flutter_rust_bridge v2（固定版本） | 移除 | 仅属于 Stage 1 |
| Rust runtime | Tokio | Tokio | 移动端需处理初始化、后台任务和线程生命周期 |
| HTTP | reqwest + rustls（需审查 API 要求） | 不变 | 定义超时、重试、代理、压缩和 Cookie 策略 |
| 序列化 | serde、serde_json、prost、quick-xml | 不变 | Protobuf schema 版本化 |
| DB | SQLite（rusqlite/sqlx 二选一） | 不变 | 建议先用 rusqlite 封装同步 I/O 工作线程 |
| Secret | iOS Keychain / Android Keystore 适配层 | 不变 | 非敏感设置可在 SQLite/配置文件 |
| 缓存 | 文件存储 + SQLite 索引 + LRU | 不变 | 明确配额和清理策略 |
| 视频 | 保留定制 media-kit/libmpv | 优先验证五端 libmpv；原生 AVPlayer/Media3 作为备选 | ADR-004 以 DASH/硬解/PiP/后台播放实测决定 |
| 弹幕 | Rust 解析/调度，Flutter 呈现 | Slint/GPU Overlay | 跨 FFI 采用批处理 |
| 图片 | 保持 Flutter 图片链路 | image crate + 系统/GPU 解码评估 | 避免全尺寸解码和重复缓存 |
| 诊断 | tracing、统一错误码 | tracing + 崩溃/性能采样 | 自建服务为可选项 |
| CI | cargo fmt/clippy/test + Flutter build | Slint 五平台构建与测试 | 构建成功不等于真机通过 |

## 4. Rust Workspace 初始结构

```text
PiliPlus-Rust/
├── upstream/                  # 可选：固定基线或 Git submodule
├── app-flutter/               # 阶段1壳，保持可发布
├── app-slint/                 # 阶段2独立壳，不在Stage1过早全量开发
├── rust/
│   ├── Cargo.toml
│   └── crates/
│       ├── bili-domain/       # 纯业务实体与 Traits
│       ├── bili-api/          # REST/协议、DTO、网络访问
│       ├── bili-auth/         # 登录状态、Cookie/凭证接口
│       ├── bili-core/         # UseCase、Application Service
│       ├── bili-danmaku/      # 解码、索引、过滤、调度
│       ├── bili-storage/      # DB、下载索引、缓存
│       ├── bili-download/     # 下载任务、续传和资源管理
│       ├── bili-player/       # 播放控制抽象和事件
│       ├── bili-platform/     # 文件、安全存储、系统能力traits
│       ├── bridge-flutter/    # FRB 对外接口
│       ├── player-mpv/        # 桌面适配
│       ├── player-apple/      # AVPlayer FFI 适配
│       └── player-android/    # JNI/Media3 适配
├── tests/fixtures/            # 脱敏的网络协议样本
├── benchmarks/
├── docs/adr/                  # Architecture Decision Records
└── .github/workflows/
```

注：阶段 1 无需一次性创建所有 crates；按功能边界逐个加入。拆分的原则是**可独立测试和可替换**，而不是目录越多越好。

## 5. 第一阶段：Flutter → Rust Core（P0）

### 5.1 交付目标

- Flutter UI 与原有播放器行为保持一致；业务 API 和数据处理逐步由 Rust 接管。
- Core 不依赖 Flutter 类型；基于同一组测试可以在不启动 Flutter 的情况下运行。
- Rust 模块功能开关允许回退到 Dart 实现，避免出现不可定位的大范围回归。
- 提前执行播放器/Slint 技术 Spike，为阶段 2 设置 Go/No-Go 门槛。

### 5.2 任务拆解（推荐顺序）

| ID | 工作包 | 交付物 | 必过验收 |
|---|---|---|---|
| P1-00 | 基线审计 | upstream commit、页面/功能矩阵、依赖图、数据目录清单 | 可复现原版构建和基础流程 |
| P1-01 | Workspace、Player Port、日志、CI、FRB | 播放时钟、事件、状态契约 | P1-04/P1-05 前批准 Player Port，桥接测试通过 |
| P1-02 | Domain / DTO / API | 统一实体、API Client、fixture 测试 | API 对照数据一致、异常可恢复 |
| P1-03 | 搜索/首页/详情 | Flutter 页面通过 Rust UseCase 加载 | UI 行为和加载状态无明显退化 |
| P1-04 | 播放资源解析 | 视频清晰度、DASH 音视频 URL、签名/请求头处理 | 现有播放器可成功使用 Rust 返回的数据 |
| P1-05 | 弹幕引擎 | Protobuf/XML 解码、过滤、时间线、调度 | 同样 fixture 的过滤/排序结果与基线一致 |
| P1-06 | 认证/用户数据 | Dart Hive 导出、Rust SQLite 导入、会话、历史、收藏 | 中间格式与升级/回滚/幂等校验通过 |
| P1-07 | 存储/缓存/下载 | DASH 分轨合并、URLSession/WorkManager 后台适配、配额 | 下载中断恢复、后台任务和回滚通过 |
| P1-08 | 收尾/替换 Dart Service | 文档、跨平台测试、兼容性报告 | 核心业务通过 Rust 运行且 Flutter 版本稳定 |

### 5.3 对齐旧行为的策略

- `golden fixtures`：对 API 响应脱敏并固定，Rust 与 Dart 使用同一输入数据校验规范化输出。
- `shadow mode`：早期仅对非敏感只读功能同时执行 Dart/Rust 解析并比对，结果仍用 Dart 显示；禁止对写操作重复执行。
- `feature flags`：按功能、按平台切换实现；发生故障可降级回 Dart。
- `upgrade tests`：复制旧版用户数据，验证首次启动、数据库迁移、登录状态、下载任务和缓存兼容性。
- 重要外部 API 的变化通过协议适配层修复，避免污染 Domain。

### 5.4 阶段 1 定义完成（DoD）

1. 按已审计的范围完成核心业务模块 Rust 化；剩余 Dart 仅承担 UI、播放渲染、平台桥接及有记录的遗留功能。
2. 核心 Rust 单测、集成测试、Flutter 回归通过；无阻断性崩溃/数据损坏。
3. 重要流程：首页→详情→播放→弹幕；登录→收藏/历史；搜索；网络断连恢复。
4. 核心 Rust Crates 独立测试；Dart Widget 不存在于 Core。
5. 迁移数据及升级/回滚路径已通过演练。
6. 至少完成 macOS + iOS/Android 的最小播放集成测试；Windows/Linux 构建和后续实测计划明确。
7. 完成下述阶段 2 视频 Spike 并形成书面决策。

## 6. M0 限时执行：Slint 视频与 UI 可行性 Spike（仅阻断 Phase 2）

> 这不是提前开展阶段 2 全量 UI 开发，而是低成本排除最关键的技术风险。

制作独立 Slint 窗口验证视频、弹幕、控制与全屏；同时验证长列表虚拟化、富文本/表情、IME、辅助功能和移动端生命周期。M0 留存测试证据，M4 评审 Phase 2 的 Go/No-Go。

**分别验证：**

- macOS：Slint 与 libmpv 视频输出如何同窗合成、是否支持硬解、弹幕层级与 HiDPI。
- Android：优先 Slint + libmpv 视频表面/触摸分发；Media3 SurfaceView/TextureView 作为备选，避免每帧 CPU RGBA 拷贝。
- iOS：优先验证 Slint + libmpv 合成、旋转、安全区、后台音频/画中画；AVPlayerLayer 为备选。
- Windows/Linux：窗口句柄、GPU 后端、视频表面、打包依赖；不能因为 macOS 通过就视为完成。

**决策门槛**：若硬解、弹幕覆盖、全屏/输入、富文本、IME、长列表或无障碍不达标，则 Phase 2 No-Go；Phase 1 Flutter + Rust Core 保持独立可发布，并评估替代方案。

## 7. 第二阶段：Slint UI 替换 Flutter（P1）

### 7.1 架构目标

- 建立 `app-slint`，直接调用 `bili-core` / `bili-player`。
- 页面样式和 UI 状态均在 Slint Shell；网络请求、会话管理、业务数据和播放状态仍在 Rust Core。
- 使用单一设计系统：颜色、字号、间距、列表项、对话框、触摸目标、深色模式、响应式窗口。
- 五个平台共享页面结构，但允许触屏和桌面交互规则差异（鼠标悬停、快捷键、手势、窗口行为）。

### 7.2 推荐迁移顺序

1. 设计系统 / 导航 / 设置、关于、错误态、加载态。
2. 首页、分类、搜索、视频列表/卡片，建立虚拟滚动和图片缓存。
3. 视频详情、评论、个人中心、收藏、历史。
4. 视频播放器、弹幕叠加、全屏、倍速、清晰度、播放进度。
5. Android/iOS 触控、键盘、IME、安全区域、系统返回键、后台恢复和画中画。
6. 桌面快捷键、窗口管理、托盘（可选）、通知和深色模式。
7. 最后迁移直播等复杂功能，视 PiliPlus 基线的正式功能清单排序。

### 7.3 五端播放器策略

| 平台 | 目标播放后端 | 关键问题 |
|---|---|---|
| iOS | libmpv 优先；AVPlayer 备选 | DASH 音视频分轨、必要请求头、硬解、PiP、合成覆盖 |
| Android | libmpv 优先；Media3 备选 | DASH/分轨、Surface 生命周期、HDR、后台音频、PiP |
| macOS | libmpv（候选） | Video surface、硬解、全屏、HiDPI、沙盒签名 |
| Windows | libmpv（候选） | GPU 视频输出、DPI、DLL 分发、驱动兼容 |
| Linux | libmpv（候选） | Wayland/X11、视频输出、系统库兼容 |

**注意**：Bilibili 视频可能提供分离音视频 DASH 资源；AVPlayer/Media3 是否能使用相同清单/封装，需用真实、合法可访问的样本验证。必要时对不同平台使用适配后的 Manifest/资源描述，不能仅用单 URL 接口硬编码。

### 7.4 Slint 阶段完成标准

- 五端能够启动并运行核心路径；每个平台完成实际设备/系统测试。
- 与第一阶段相比，用户数据可以升级迁移；不要求完全共用旧 Flutter 运行时存储目录。
- 视频播放、弹幕覆盖、触摸/鼠标操作、音频焦点和前后台恢复通过专项验收。
- 安装包及版本信息齐全；正式 Slint 客户端不再依赖 Flutter 运行时。
- 无重大可访问性、输入法或多 DPI 回归。

## 8. 第三阶段：性能优化与 Bug 修复（P2）

> 性能与 Bug 修复从阶段 1 就开始实施；第三阶段仅表示集中专项优化。

### 8.1 必须有基线的指标

| 指标 | 场景 | 衡量方法 | 建议门槛 |
|---|---|---|---|
| 冷启动 | 设备重启/杀进程后至可交互 | 多次采样 p50/p95 | 不劣于基线；专项优化后再设目标 |
| 内存 | 空闲首页、长列表、1080p 视频 | RSS/PSS + GPU 指标 | 无持续增长/泄漏，比较峰值 |
| CPU | 1080p + 固定弹幕密度 | 多次稳态采样 | 不劣于基线，明确环境 |
| 帧耗时 | 首页快速滚动、视频弹幕 | p95/p99 帧耗时和丢帧 | 针对目标刷新率设预算 |
| Seek 延迟 | 同一视频、固定网络 | 点击至有效帧时间 | 不劣于基线 |
| 网络 | 弱网/断线/恢复 | 超时、重试、首帧 | 不出现无限重试/重复请求风暴 |
| 电量 | 固定亮度、音量、网络、播放时长 | 能耗工具/设备统计 | 优化前后 A/B |
| 安装体积 | Release 包 | 各平台实际产物 | 每次增量可解释 |

### 8.2 优化顺序

1. 正确性、卡死和崩溃优先；依赖数据和实测，而非理论加速比例。
2. 避免大对象跨 FFI 重复序列化；使用批处理和粗粒度接口。
3. 视频硬解与零拷贝/低拷贝路径；减少 UI 与播放器抢占 GPU。
4. 弹幕：预索引、轨道缓存、文本测量缓存、批量渲染和同步时钟。
5. 图片：按展示尺寸解码、分页预取、内存和磁盘分级缓存。
6. 列表：虚拟化、稳定 item key、减少重复布局与无意义状态通知。
7. Rust 端 profiling：`cargo flamegraph`（支持平台）、Instruments、Android Perfetto、Windows WPA 等。

## 9. CI、测试与发布矩阵

### 9.1 CI 分层

- 每个 PR：`cargo fmt --check`、`cargo clippy`、`cargo test`、协议 fixture、基础静态检查。
- 阶段 1：Flutter 单测、Widget 测试、至少 macOS/Android/iOS 构建检查。
- 阶段 2：macOS、Windows、Linux 构建；iOS/Android 构建与模拟器冒烟（能力允许时）。
- 每夜：跨平台集成测试、性能基准样本、依赖许可证报告。
- 发布前：真机播放与后台行为、安装包签名、断网恢复、升级路径和用户数据回归。

### 9.2 测试等级

- **Unit**：API Model/弹幕/缓存策略/时间线/错误映射，无网络。
- **Contract**：脱敏 HTTP fixtures；固定预期；覆盖协议变化。
- **Integration**：沙盒存储、数据库升级、播放器模拟器、真实 API 的受控测试。
- **E2E**：账号、首页、搜索、详情、播放、Seek、全屏、弹幕、评论与数据升级。
- **Performance**：固定机器、网络、视频样本、分辨率、弹幕密度，重复采样。

### 9.3 平台覆盖注意

> 完整的实测能力矩阵、硬件探测证据与排除记录见 [docs/m0/VALIDATION-CAPABILITY.md](../m0/VALIDATION-CAPABILITY.md)。

编译打包全部放 GitHub Actions（上游已有五端 workflow 可继承），本机只做 CI 证明不了的事。

| 平台 | 编译 | 真机 GUI | 硬解 | 结论 |
|---|---|---|---|---|
| macOS | Actions + 本机 | ✅ 本机 | ✅ videotoolbox | 完整验收 |
| iOS | Actions | ✅ 真机 | ✅ videotoolbox | 完整验收（签名待配置） |
| Android | Actions | ✅ 真机 | ✅ mediacodec | 完整验收 |
| Windows | Actions | ✅ 家中设备 | ✅ d3d11va | 完整验收 |
| Linux | Actions | ❌ | ❌ | **best-effort** |

- **CI 证明不了核心风险**：硬解、Slint + libmpv 合成、PiP、后台音频、IME、无障碍、渲染性能。CI runner 为虚拟化环境，`hwdec` 必然回落软解。性能基线（§8.1）必须固定同一台机器采样。
- **Linux 降级为 best-effort**：仅保证 Actions 编译通过 + Docker 多发行版 glibc/动态库兼容性矩阵 + Rust Core headless 单测；放弃 vaapi 硬解、Wayland/X11 合成、渲染性能与音频验收。Docker 是编译与兼容性闸门，不是验收手段——macOS 宿主下无 GPU 直通与显示输出。
- **已评估并排除**：家用 iStoreOS 软路由（x86_64、具备 `/dev/dri`），因 Celeron J1900 为 Bay Trail Gen7，仅支持 H.264 硬解，不支持 HEVC/AV1/DV，且驱动栈（i965）与现代 Linux 桌面（iHD）不一致，验证结论不可外推。
- **成本**：私有仓库 macOS runner 为 10 倍分钟倍率，需公开仓库、限制触发时机或使用 self-hosted runner。
- **Rust 进入后 CI 需补**：各端 target、NDK 配置与 `~/.cargo`、`target/` 缓存，归入 `RUST-003`。
- iOS 侧载免费签名 7 天限制独立于客户端技术架构；此项目不承诺绕过限制。

## 10. 工程管理与上线策略

### 10.1 分支与 upstream 同步

- `main`：稳定可构建。
- `feature/rust-*`：按模块迁移；小 PR。
- `spike/slint-player-*`：技术探索，未经 Review 不并入正式功能。
- `release/*`：测试发布；upstream 同步须配合回归。
- 设定 `UPSTREAM_COMMIT` 并记录 rebase/merge 策略，避免基础代码不断变化。

### 10.2 ADR 决策记录

ADR 路径：docs/architecture/adr/。每篇记录候选、测试证据、决策、后果、回退与复审条件；无证据时标记 Proposed。

| ADR | 决策范围 | 截止时点 |
|---|---|---|
| ADR-001 | Domain/Application/Data 对应 crates | M1 |
| ADR-002 | Flutter/Slint/Tokio 线程与生命周期 | M1 |
| ADR-003 | Player Port、状态归属、弹幕时钟 | P1-04/P1-05 前 |
| ADR-004 | libmpv 与 AVPlayer/Media3 比较、视频合成 | M0 提案，M4 准入前批准 |
| ADR-005 | Hive Dart 导出 → Rust SQLite 导入与回滚 | P1-06 前 |
| ADR-006 | DASH 分轨 mux、URLSession/WorkManager 后台下载 | P1-07 前 |
| ADR-007 | Slint 视频/列表/富文本/IME/无障碍及失败回退 | M0/M4 |
| ADR-008 | WBI、协议版本化与上游同步 | M1 |
| ADR-009 | GPL、播放器依赖和分发合规 | 发布前 |

### 10.3 发布与合规

- PiliPlus 使用 GPL-3.0；若派生代码用于分发，遵守许可证、版权和对应源码义务。
- Slint 可选择 GPLv3 等官方许可路径；需结合项目发布方式确认。
- 审查视频和 API 的使用条款、账号凭据处理、日志脱敏、用户内容下载授权及各应用商店规则。
- 不嵌入硬编码账户凭据、设备私钥或不可公开的 API 秘钥。

## 11. 风险登记簿

| ID | 风险 | 概率/影响 | 缓解策略 | 阶段阻断？ |
|---|---|---|---|---|
| R1 | Slint 与原生视频表面难以合成 | 高/高 | M0 限时视频 Spike | **是（Phase 2）** |
| R2 | iOS/Android DASH、清晰度、请求头差异 | 中/高 | 单独播放器契约与真机样本 | **是** |
| R3 | 原 Flutter 的定制 media-kit 行为未摸清 | 高/中 | 固定 commit 和依赖审计 | 是（阶段1） |
| R4 | 登录/数据库迁移损坏数据 | 中/高 | 复制数据演练、双读、回滚 | **是** |
| R5 | FFI 高频调用拖慢性能 | 中/中 | 粗粒度接口、事件批处理 | 否 |
| R6 | 上游 Bilibili API 变化频繁 | 高/中 | 版本化 Protocol Adapter 和 fixtures | 否 |
| R7 | 五平台发布/签名/依赖问题 | 中/高 | 早期 CI、逐平台验收 | 是（发布） |
| R8 | GPL/平台分发条款不满足 | 中/高 | 发布前许可证与合规 Review | 是（发布） |
| R9 | Slint 列表虚拟化、富文本、IME、无障碍及移动端成熟度 | 高/高 | M0 Spike，Flutter + Rust Core 为回退 | **是（Phase 2）** |
| R10 | 过早追求纯 Rust 导致过度重写 | 高/中 | 以可验证价值为前提，硬解保留原生 | 否 |

## 12. 里程碑与交付物

> M0 是限时 Spike；Phase 1 发布和 Phase 2 准入分别判断，禁止将 CI 编译等同于真机验收。

| 里程碑 | 主要成果 | Go / No-Go |
|---|---|---|
| M0：基线与风险 Spike | 固定上游 commit、审计 Hive/media-kit；macOS+Android 的 Slint 视频/弹幕/长列表/IME 验证 | 产出测试报告；未通过阻断 Phase 2 而非 Phase 1 |
| M1：Rust Core 基础 | Workspace、FRB、Player Port、协议 crate、CI、线程模型 | 播放器状态所有权与时钟契约批准 |
| M2：Rust 业务迁移 | API、签名、首页、详情、DASH、弹幕 | 行为/fixture 与 Flutter 基线一致 |
| M3：Flutter + Rust Core 稳定发布 | Hive 迁移、认证、缓存、下载、恢复与回滚 | **正式可独立发布** |
| M4：Slint 首批平台 | ADR-004/007 审批；macOS+Android 播放/弹幕/UI | 实机通过，否则维持 M3 |
| M5：其余平台逐项交付 | iOS、Windows、Linux 分别验证与发布 | 各平台独立 Go/No-Go |
| M6：优化及正式版本 | CPU/内存/流畅度/崩溃/耗电对照 | 性能、稳定性与合规检查通过 |

## 13. Review 需确定的问题

1. **项目关系**：Fork PiliPlus 继续公开协作，还是独立新仓库并保留来源与许可证？
2. **第一阶段范围**：直播、下载、动态、字幕、番剧是否都要求进入 M3，还是允许按功能矩阵后移？
3. **数据库策略**：沿用原版持久化格式，还是采取一次性导入 Rust SQLite + 回滚？
4. **播放器策略**：接受移动端保留 AVPlayer/Media3 的原生桥接，还是为五端统一 mpv 承担额外集成成本？
5. **Slint 失败回退**：播放器 Spike 不达标时，允许更换 UI/合成技术方案吗？
6. **支持的 OS 最低版本**：iOS、Android、macOS、Windows、Linux 发行版和架构范围要明确。
7. **交付发布**：仅 GitHub Releases 侧载，还是考虑 App Store / TestFlight / 各商店？
8. ~~**测试资源**~~：**已确定**。macOS/iOS/Android/Windows 四端均具备真机验证条件；Linux 无 GUI/GPU 环境，已决定降级为 best-effort。详见 [VALIDATION-CAPABILITY.md](../m0/VALIDATION-CAPABILITY.md)。
9. **团队/Agent 实施方式**：每个 PR 的最大范围、Review 人、自动测试及完成判断。

## 14. 第一批 Issue

已全部建为 GitHub Issues，不在本文维护副本（避免状态双写漂移）：
<https://github.com/kakakakacat/KiliKili/issues>

- **Milestones** 对应 §12 的 M0–M6
- **Labels**：`phase:P1`/`phase:P2`、`area:*`、`type:spike`/`type:blocker`/`type:chore`
- 本文与 ADR 保留**决策与证据**；**状态**一律以 Issues 为准

## 15. 参考资料（2026-11-10 检索）

- PiliPlus 官方仓库：https://github.com/bggRGjQaUbCoE/PiliPlus
- PiliPlus 当前依赖配置：https://github.com/bggRGjQaUbCoE/PiliPlus/blob/main/pubspec.yaml
- flutter_rust_bridge：https://pub.dev/packages/flutter_rust_bridge
- Slint 框架及平台：https://slint.dev/faqs
- Slint 移动端开发：https://docs.slint.dev/latest/docs/slint/guide/platforms/mobile/general/
- Slint Android：https://docs.slint.dev/latest/docs/slint/guide/platforms/mobile/android/
- Slint 许可证：https://github.com/slint-ui/slint/blob/master/LICENSE.md
- mpv：https://github.com/mpv-player/mpv
- Media3：https://developer.android.com/media/media3

---

**Review 建议**：原则上批准三阶段路线，但阶段 2 的完整 Slint 替换以视频合成 Spike 通过为前置条件；第一阶段优先交付一个可以继续使用的 Flutter + Rust 客户端，而不是过早追求所有代码全部 Rust 化。

## 16. v0.2 Review 意见与架构约束

### 16.1 Player Port 和单一播放时钟（P0）
- 阶段 1 的真实播放状态由 Dart media-kit/libmpv 拥有，阶段 2 由 Native libmpv Adapter 拥有。包括 position、duration、paused、buffering、speed、音轨以及 Seek 完成事实。
- Rust Player Orchestrator 负责播放意图、清晰度选择、业务状态、错误恢复策略、进度持久化以及对 UI 发布状态快照；不自行累计时间模拟播放器时钟。
- 播放器事件附加 sequence/generation。Flutter/Slint 消费 Rust 快照，Seek/暂停/倍速/缓冲时弹幕以真实播放器时钟重新定位。跨 FFI 使用合并/限频快照，避免每帧调用。
- P1-04 与 P1-05 开始前完成 ADR-003、Player Port 资源/音轨/事件/渲染表面/生命周期接口评审。

### 16.2 ADR-004：播放器后端选择（P0）
- libmpv 为五端统一的首选验证方案，因现有 media-kit 已与 mpv 相关；不能预设它在移动端具备完整 PiP、后台音频、硬解和 Slint 表面合成能力。
- AVPlayer 不直接支持常规分轨 DASH manifest，若用作 iOS fallback 必须评估 HLS 转换/本地代理的功能、性能和维护代价；Android Media3 作为另一个 fallback。
- 使用相同合法样本测量 DASH 分轨、Seek、倍速、请求头、清晰度、HDR/硬解、音画同步、弹幕叠加、CPU/GPU/内存、PiP、后台音频、包体与构建复杂度。记录设备、系统、版本与日志；据此决定。

### 16.3 Hive 数据兼容和回滚（P0）
- 不要求 Rust 直接读取 hive_ce 二进制。先由 Dart Exporter 导出版本化 JSON/SQLite 中间格式，带 schema_version、记录数量和校验信息；Rust Importer 用 SQLite 事务导入。
- 迁移前备份，迁移成功验收前不删除旧数据；必须验证重复执行幂等、失败中断、用户收藏/历史完整性和账号安全。敏感凭据优先走系统安全存储，不导出明文。
- ADR-005 必须在 P1-06 前批准。

### 16.4 DASH 下载与后台运行（P0）
- Rust 负责下载状态机、任务队列和持久化，分轨音视频下载后需 mux，评估 FFmpeg/替代实现的包体、性能与许可。
- iOS 长时间后台传输依赖 URLSession 等系统机制，Android 采用 WorkManager 或适用的前台任务策略；应用内 Tokio 不等于后台保活。
- 断点续传、任务恢复、磁盘配额、失败清理、跨进程恢复需要独立测试。ADR-006 在 P1-07 前通过。

### 16.5 Slint UI 可行性与线程模型（P0）
- M0 限时 Spike 除视频外必须包含：大列表虚拟化、评论/动态富文本与表情、中文 IME/焦点、辅助功能 VoiceOver/TalkBack、触控、HiDPI、前后台与窗口生命周期。
- Slint UI 只在其事件循环线程被更新；Tokio 处理网络/IO/调度，使用 slint::invoke_from_event_loop 或等价消息队列切换，处理弱引用、任务取消与移动端挂起/恢复。
- 任一关键风险无法接受则 Phase 2 No-Go，M3 Flutter + Rust Core 保持正式可发布且持续维护。

### 16.6 分层与 crate 映射（P1）
- domain：bili-domain 中的实体、规则和 Ports traits。
- application：bili-core 中 UseCase、播放器意图编排、事件/快照。
- data：bili-api、bili-auth、bili-protocol、bili-storage 的 HTTP/认证/协议/DB Adapters。
- infrastructure：player-mpv、platform-download、secure-storage 等平台 Adapters。
- presentation：bridge-flutter 与 app-slint；不得直接依赖具体协议实现。

### 16.7 协议变更、上游同步和平台分级（P1）
- WBI 签名、必要请求头、设备信息格式等易变协议放在单独 bili-protocol crate，使用版本化 fixtures、变更记录和独立回归测试；遵守第三方服务规则。
- 固定 UPSTREAM_COMMIT，按同步窗口审计上游 API/签名/播放器修复。先改 Rust Protocol/Adapter 与 fixture，再验证 Flutter 旧实现差异；避免只同步 Dart 修复遗漏 Rust。
- Slint 首批验收目标为 **macOS + Android + Windows**（三端均具备真机与硬解验证条件），iOS 紧随其后，Linux 单独降级为 best-effort。编译成功不代表视频/IME/硬解等真实功能可用。依据见 §9.3 与 [VALIDATION-CAPABILITY.md](../m0/VALIDATION-CAPABILITY.md)。

### 16.8 Review 逐项处理结果
| Review 问题 | 结论 |
|---|---|
| Spike 阻断与 M4 排期冲突 | M0 限时 Spike，M4 审批/复核 |
| 播放器所有权断层 | 真实时间归播放器，意图与快照归 Rust |
| 移动端 AVPlayer/Media3 候选 | libmpv 优先实测，ADR-004 裁决 |
| Hive 隐含迁移成本 | Dart 导出 + Rust 导入，保留回滚 |
| DASH 下载/后台任务 | mux + URLSession/WorkManager |
| Slint 长列表/富文本/IME/无障碍 | Phase 2 阻断风险，M3 可发布回退 |
| 线程与运行时生命周期 | Slint 事件循环单线程，Tokio 后台 |
| Crate 与分层不一致 | 增加明确映射 |
| 签名与风控协议易变 | bili-protocol 版本化 |
| 上游变更映射缺失 | UPSTREAM_COMMIT + Rust fixtures 回归 |
| 弹幕时钟不明 | 使用单一播放器事实时钟 |
| 五端同时卡验收 | macOS/Android 先行，其余单独准入 |
