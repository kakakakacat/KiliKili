# AGENTS.md

给在本仓库工作的 AI 编码助手（Claude Code / Codex / Cursor 等）与新来的人类
贡献者。**跟仓库走**：换一台机器、换一个工具，读完这份就能接上。

决策与理由在 `docs/`，状态在 GitHub Issues，本文件只放**跨会话不变的工作
约定**。两者冲突时以 `docs/` 为准。

---

## 1. 这个仓库在做什么

KiliKili 是 PiliPlus（Flutter/Dart）的分支，正在把业务层逐步迁到 Rust。
总方案见 `docs/architecture/RFC-0001-rust-slint-migration.md`，迁移形态见
`docs/architecture/adr/ADR-011-typed-ffi-direct.md`。

当前阶段：Flutter 仍是宿主，Rust 通过 flutter_rust_bridge（FRB）提供
typed 结构；按**切片**（slice）逐条接口迁移，一条接口一个 PR。

---

## 2. 架构铁律：宿主是可替换的

**Flutter/Dart 这一层以后会被换掉**（RFC 的 Stage 2 是 Slint，实际也可能是
别的跨平台框架），**Rust 这一层不会**。

因此判断一段逻辑该放哪边，标准**不是**「它算不算展示规则」，而是：

> 换掉宿主框架时，这段代码要不要重写一遍？要，就说明它放错地方了。

**必须下沉到 Rust**（哪怕它看起来像 UI 的事）：

- 排序规则、过滤规则、阈值判断；
- 档位/名称的回退链与查表（清晰度名、音质名…）；
- 候选 URL 的顺序、CDN 选路、重试与降级分支；
- 去重、合并、选流/选码算法；
- 任何「协议没规定、但两端必须一致」的决定。

**可以留在宿主**：

- 具体的字符串字面量与 i18n 文案（例：给 AI 配音的标题加「（AI）」后缀）；
- Widget / 布局 / 动画 / 手势；
- 平台 API 调用：本地存储、权限、通知、播放后端的具体 SDK。

推论，写代码时会反复用到：

- **FRB 的导出面保持语言中立。** 纯数据结构 + 明确的函数。不要把宿主语言的
  习惯（Dart 的 sealed 层级、extension、可变模型）泄进 Rust 的导出签名。
- **不要把逻辑搬进 `*_ext.dart`。** 迁一条路径时，顺手把它的算法从 Dart
  搬掉，而不是换个 Dart 文件接着放。
- **已经在 Rust 实现、却还藏在开关后面的**（典型：`bili-core::media_select`），
  迁到它那条路径时就让 Rust 成为**唯一**实现，删掉 Dart 原实现与开关
  （ADR-011 决策四）。
- `bili-core` 的 crate 文档写着「不得依赖 Flutter Widget 或 Slint 具体
  类型」——这条铁律就是它的来源，不是随口一句。

### 例外要写下来

确有留在宿主更划算的情况（例：一张十来行的静态枚举表，拔出来要动设置页、
`Pref` 默认值和一串 `Rxn<T>`）。**允许，但必须在代码注释里写明「为什么留」
和「随哪个切片删」**，并在对应 issue 里留一条。没写理由的留存，下一个人会
当成疏漏再迁一次。

---

## 3. 迁移切片的规矩（ADR-011）

- **typed 直连**：Rust 侧 `XxxResult { data, error_kind, error_detail,
  error_code }`，失败一律**返回值而非异常**；Dart 侧直接消费 FRB 类型，
  不再还原成 Dart 模型。
- **决策四：不留旧实现、不设降级开关。** 迁一条路径，就在**同一个 PR 内**
  删掉被取代的 Dart 模型与 `lib/http/*.dart` 里对应的函数。
- **一次性 parity 测试**：用尚未删除的 Dart 模型解析同一份 fixture，与 Rust
  输出逐字段比对，通过后随旧模型一并删除。parity 测试**刻意不 import
  `lib/`**，这样它在没打 Flutter SDK 补丁的机器上也跑得起来。
- **golden fixture** 取自真实响应并**脱敏**：CDN 地址、稿件 id、昵称、mid
  一律替换。
- 账号选择必须与 `lib/utils/accounts/api_type.dart` 的 `ApiType.apiTypeSet`
  一致（取流 `Accounts.video`、详情 `heartbeat`），迁移时最容易丢。
- 真机验证与回归统一留到 Phase 2，切片内不做。

---

## 4. 生成产物与 FRB

`lib/rust/frb_generated.dart`、`lib/rust/frb_generated.io.dart`、
`rust/crates/bridge_flutter/src/frb_generated.rs` 是**生成产物**，多分支并行
时必然冲突。

- **不要手工解冲突标记。** 取一侧做基底，`git add` 之后整体重跑
  `tool/frb-generate.sh`，让两边的导出一起生成。
- **改了 Rust 的导出签名就必须重跑 `tool/frb-generate.sh`**，否则
  `frb_generated.rs` 里还是旧调用，报出来的错会像是自己写错了参数个数。
- 共享的手写模块清单（`bridge_flutter/src/api/mod.rs`、`bili-domain/src/lib.rs`、
  `bili-api/src/lib.rs`）**只追加自己的行**，冲突时取并集、按字母序排回去。
- **FRB 不支持 `Option<Option<T>>`**，codegen 会直接 panic。要表达「成功但
  没有值」，用一个没有 `data` 字段、靠 `error_kind == null` 判成功的结果
  类型（例见 `LiveMedalNameResult`）。

---

## 5. 改完要跑什么

Rust（**三条都要**，只跑刚写的那个测试不够）：

```sh
cd rust
cargo fmt --check          # rustfmt 按字符宽度判行长，中文注释极易超
cargo clippy --workspace --all-targets
cargo test --workspace
```

Dart：

```sh
dart analyze lib
```

> `dart analyze` 在**没跑过 `tool/prepare-build.sh`** 的机器上恒有一批既有
> 报错（上百条），它们来自 `lib/scripts/*.patch` 要打进 Flutter SDK 的符号。
> 判断自己是否干净**不能看绝对数**，要把「有 error 的文件集合」与「本分支
> 改过的文件集合」求交集，交集为空即可。
>
> 同理，本机 `flutter test` 只能跑不碰 widget 层与 `Pref`/`storage` 的测试；
> 其余只能靠 CI（CI 会先跑 `prepare-build.sh`）。

---

## 6. 文档与代码同改

每完成一项改动都要检查并更新 `docs/` 下被它证伪的句子，**与代码进同一个
PR**，不要只留在代码和 Issues 里：

| 文件 | 什么时候要改 |
|---|---|
| `docs/architecture/upstream-divergence.md` | 碰了上游既有文件（它是实测值，不是估计） |
| `docs/m0/VALIDATION-CAPABILITY.md` | 任何 CI / 验证能力的增减 |
| `docs/architecture/RFC-0001-rust-slint-migration.md` | 架构、crate 结构、阶段范围变化 |
| `docs/architecture/adr/ADR-*.md` | 迁移形态、决策及其例外 |
| `docs/architecture/protocol-changelog.md` | 只在改**协议实现**（请求面/签名）时记一行，响应解析不算 |
| `docs/qa/RELEASE-VALIDATION.md` | 新增或删除需要人工验证的路径 |

状态仍以 Issues 为准，不在文档里维护副本。

---

## 7. 分支、PR 与 CI

- **合并前要仓库所有者明确许可。**
- **合并排队，一次只合一个**——生成产物决定了并行合必冲突。
- 必需检查叫 **`regression gate`**（`.github/workflows/ci.yml` 的 `gate` job
  用 `needs:` 聚合 rust / flutter / android 三项）。每个 PR 都会打一次
  Android APK，11–15 分钟，是全流程最慢的一项，按这个量级估时间。
- `mergeable=MERGEABLE` 且 `mergeStateStatus=CLEAN` 才能合；`BLOCKED` 通常
  只是 gate 还没上报。
- **不要 rebase 已开 PR 的分支**，用 `git merge origin/main` 跟上 main。
- **堆叠 PR**：`gh pr create --base <上一个分支>`，合并顺序必须从下往上。
  注意上一个分支被删时 GitHub 会把堆在它上面的 PR 一并关掉——先把 base
  改到 `main` 再合，或恢复那个 ref 后重开。
- 五端 app 构建（`build.yml` / `win_x64.yml` / `linux_x64.yml` …）保持
  `workflow_dispatch` 手动触发，**不要提议打开 `pull_request` 触发**；需要
  验证构建时手动 dispatch。

---

## 8. 本地工具链

跨机器可复用的要求（具体安装路径因机器而异，不写在这里）：

- Rust 工具链 + **MinGW-w64 的 MSVCRT 变体**（Windows）：rustup 的宿主工具链
  是 `x86_64-pc-windows-gnu`，链的是 msvcrt，混用 UCRT 会出 CRT 不一致。
- Flutter SDK（版本见 `pubspec.yaml`）。
- `flutter_rust_bridge_codegen` CLI，版本必须与
  `rust/crates/bridge_flutter/Cargo.toml` 和 `pubspec.yaml` 三者一致，
  `tool/frb-generate.sh` 会先校验再生成。
- PowerShell 7（`pwsh`）：`tool/prepare-build.sh` 第 4 步要用它跑
  `lib/scripts/patch.ps1`。

注意事项：

- `tool/prepare-build.sh` 会 **reset 全局 Flutter SDK** 并删 pub cache 里的
  cupertino_ui / material_ui，影响本机其他 Flutter 项目，别在无人值守时跑。
- `flutter pub get` **不要带 `PUB_HOSTED_URL` 镜像**：它会把 `pubspec.lock`
  里每条 `url: https://pub.dev` 改写成镜像地址并顺带升级若干包。要用镜像就
  只用在下载 SDK 上。
- 本机没有 fvm 时，issue 模板里写的 `.fvm/flutter_sdk/bin/dart` 按全局
  Flutter SDK 的 `dart` 来用。

---

## 9. 代码风格

- 注释与文档写**中文**，与现有代码保持一致。
- 注释解释**为什么**，不解释代码在做什么；尤其要写下「刻意没做 X」及其
  理由——那是下一个人最容易白走一遍的地方。
- 不要为了「更干净」顺手重构与本次改动无关的代码：上游（PiliPlus）还要
  持续同步，无谓的改动会变成每次同步的冲突点。
