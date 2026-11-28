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

动了 `tool/proto-generator`（加/改 proto 消息表时一定会动）再补一条。它是
独立的 cargo 工程，上面那条 `cargo fmt --check` **管不到它**，而 CI 的
`proto.yml` 会查——#176、#198 两次都是合进 main 之后才红：

```sh
cd rust
cargo fmt --manifest-path ../tool/proto-generator/Cargo.toml --check
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
>
> **交集判据有一个例外：自己给 `lib/scripts/*.patch` 新加符号的时候。**
> 那个符号只存在于打过补丁的 SDK，未跑 `prepare-build.sh` 的机器上会在
> **自己刚改过的文件里**报 `undefined_*`，交集必然非空，但并不说明改错了。
> 当前已知的这类符号：`DoubleTapGestureRecognizer.doubleTapTimeout`
> （`double_tap_gesture.patch`，见 #143）、`DoubleTapGestureRecognizer.checkUp`。
> 遇到时先用 `git apply --check` 确认补丁能干净打上（只读，不动 SDK），
> 真正的编译验证交给 CI。

### 性能与功耗测试不属于「改完要跑什么」

**帧耗时、GPU 占用、内存、启动耗时、温度、功耗这一类测量，只在专门做性能
优化的时候做，其他任何时候都不要自己加进来。** 即使改动看起来和渲染、
播放器或列表有关，也不要顺手去量。

理由是成本不对称：这类测量要占用真机、要打 profile 包、要反复安装与长时间
采样，花的是人的设备和时间，而**绝大多数改动并不需要一个数字才能判断对错**。
功能是否正确、观感是否符合设计，看一眼就知道；性能基线该不该动，是另一件
独立的事，有它自己的时机。

碰到确实可能影响性能的改动，正确做法是**把风险写进 issue 或 PR 正文**
（哪个场景、可能往哪个方向劣化、需要什么设备才能验），标成未验证，
交给性能优化那一轮统一测，而不是当场自己开一轮。

需要人工确认的性能场景与采样方法本身，记在
[docs/qa/RELEASE-VALIDATION.md](docs/qa/RELEASE-VALIDATION.md) §4，
那是给那一轮用的，不是给每个 PR 用的。

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

- **动手之前先有 issue，并挂上 milestone。** 不做「顺手改的、没有单子」的
  改动；PR 正文引用那条 issue。业务层迁移挂 `M2: Rust 业务迁移`。

  这条不是流程洁癖，是补一次真实的窟窿：2026-11-12 复盘时发现
  `M2` 显示「开 0 关 2」，看起来早已收工，而 EPIC #67 连同 16 个切片、
  3 份 ADR、取流 #79 等 **27 个 issue 一个都没挂 milestone**——只看
  milestone 的话整个仓库像是停在 M0/M1。活干了，进度没有任何地方
  反映得出来（见 issue #102）。

- **关 issue 前对着它自己写的「范围」逐项核一遍。** 别凭「这一片做得
  差不多了」就按 COMPLETED 关——#66 就是这么关错的：范围里列的
  `music.dart` / `follow.dart` / `fan.dart` 当时都还在 Dart。

- **PR 正文里要自动关 issue，关键词必须写英文。** GitHub 只认
  `close` / `closes` / `closed` / `fix` / `fixes` / `fixed` /
  `resolve` / `resolves` / `resolved` 后面跟 `#N`。本仓库的注释与 PR 正文
  一律写中文（§9），于是最顺手的「关闭 #58」**不会关**——它只在 issue 里
  留一条 cross-reference，看起来像是关联上了。

  失败是静默的：#170 就是这样，PR 绿着合了，#58 还开着，最后手动
  `gh issue close` 补的。代价不只是多一步——milestone 的「开 N 关 M」
  会跟着失真，那正是 #102 记的那个窟窿。

  写法：正文照旧用中文，只把关键词那一句留成英文，例如
  `Closes #58`，后面再用中文说明。合并后**顺手确认一眼 issue 真的关了**，
  别假定关键词生效（见 issue #171）。

- **合并前要仓库所有者明确许可。**
- **合并排队，一次只合一个**——生成产物决定了并行合必冲突。
- **PR 上的回归检查已经恢复**（issue #114）：`ci.yml` 的 `pull_request:`
  触发于 2026-11-27 恢复，ruleset「main regression gate」于 2026-11-28 改回
  `active`，`regression gate` 重新是合并到 main 的必需检查。

  **当初是两处一起关的，恢复却必须分两步。** 必需检查一旦生效而
  `regression gate` 报不上来，PR 会全部卡死在 `BLOCKED` 永久合不进去；所以
  先恢复触发，在真实 PR 上看到它报过绿（#309 / #310），再开 ruleset。

  **出事时唯一的出路是把 ruleset 关回去**：它的 `bypass_actors` 是空的，
  仓库所有者也没有豁免。典型触发场景是仓库 Actions 又被停（见下一条）——
  那时 gate 永远报不上来，所有 PR 都合不进去：

  ```sh
  gh api -X PUT repos/<owner>/<repo>/rulesets/27013798 -f enforcement=disabled
  ```

- **合并前仍然自己在本机验一遍**（至少 `cargo fmt --check` / `cargo clippy` /
  `cargo test`，动了 Dart 再跑 `flutter test`）。PR 上有必需检查不等于能省掉
  这一步：Android 那项 11–15 分钟，低级错误在本机挡住比来回推分支快得多。
  要在开 PR 前先跑一次完整回归：`gh workflow run "Regression CI" --ref <分支>`。
- 回归触发的地方：PR、合并进 main 的 push、每日 02:17 定时、手动 dispatch。
  合完仍然顺手看一眼 main 上那次运行——PR 绿只说明合并前那一版是绿的。
- **那次运行根本没出现时，先怀疑 Actions 被停了，而不是触发条件。**
  2026-11-26 遇到过一次：`gh workflow run` 返回
  `HTTP 422: Actions has been disabled for this repository.`，而仓库设置页与
  `repos/.../actions/permissions` 都显示 `enabled: true`、十个工作流全
  `active`，逐个调 `PUT .../actions/workflows/<id>/enable` 也全部返回 OK。

  **成因是「本仓库的 workflow 总开关」没开，而这个开关 API 看不见也按不到。**
  本仓库是 PiliPlus 的 fork，fork 的工作流有一道独立的闸门，开关在**仓库的
  Actions 标签页**上（页面顶部的横幅与按钮），它与 Settings → Actions 里那组
  `Allow all actions / Disable actions` 单选**不是同一个东西**：后者是
  `actions/permissions` 反映的那个，前者不是。所以会出现「API 说开着、派发说
  关着」。

  **判据：`actions/permissions` 说 `enabled: true` 但派发 422 时，不要再在
  API 和仓库设置页里找了，直接去 Actions 标签页点那个开关。** 2026-11-26 那次
  在 API 这边白试了一轮才问出来。

  GitHub 不在别处提示这件事，表现就是 workflow 安静地不跑：那次连着三次合并
  进 main 都没触发回归也没人发现，是合完去看那次运行才暴露的。

- **先分清是账号级还是仓库级，再去猜成因。** 一条命令就够——看同账号别的
  仓库最近有没有运行：

  ```sh
  gh repo list <owner> --limit 200 --json name -q '.[].name'     | xargs -I{} sh -c 'gh api repos/<owner>/{}/actions/runs?per_page=1         -q "\"{} \" + (.workflow_runs[0].created_at // \"无\")"'
  ```

  别的仓库还在跑 → 仓库级，与账单、额度无关，别去动账号设置。全停 → 才轮到
  账号级那几种可能。**这一步要在动手之前做**：#279 那次是反过来的，看到
  产物 4.87 GB、而 Free 账号额度是 500 MB，就认定是存储超额导致账号级停用，
  据此删光了全部产物——事后一量才发现 public 仓库的 Actions 用量本来免费、
  那个额度主要约束 private 仓库与 Packages，而且同账号别的仓库当天都在正常
  跑。**「数字看着超标」不等于「这个数字受那条限制约束」。**

- **别把要长期留的包当产物传。** public 仓库的 Release 附件不计入 Actions
  存储，产物那份不是。带 `tag` 的发布构建已经把包发到 Release，所以五端
  workflow 里的 `upload-artifact` 都加了 `if: <tag> == ''`；验证包一律
  `retention-days: 2`（仓库默认已从 90 天改成 7 天）。量一眼用的是：

  ```sh
  gh api "repos/<owner>/<repo>/actions/artifacts?per_page=100" -q '.total_count'
  gh api repos/<owner>/<repo>/actions/permissions/artifact-and-log-retention
  ```

  缓存（`actions/cache/usage`）另算、不计费，GitHub 按 LRU 自动淘汰到 10 GB，
  不用管。
- **定时回归红在 `ios` / `macos` 时，先看是不是没抢到 runner。** 托管 macOS
  runner 容量紧张时 job 会在 15 分钟整处被取消——`steps` 为空、日志拉不到、
  运行注解写着 `The job was not acquired by Runner of type hosted even after
  multiple attempts`。那跟你的改动无关，重跑即可。2026-11-24 起 `gate` 不再
  把平台打包的 `abandoned` / `cancelled` 判红（issue #181），所以**反过来也要
  注意：这四项绿着不等于它们真的编出了包**，扫一眼运行页上有没有「Platform
  package never started / cancelled」那两条 warning。rust / flutter / proto /
  android 不在放宽范围内。

  **写这类判断时别照着运行页抄状态词。** 没抢到 runner 的 job，运行页与
  Jobs API 显示 `conclusion: cancelled`，而 `needs.<job>.result` 给的是
  **`abandoned`**——#223 的第一版就是照着页面写了 `cancelled`，合完照旧红。
  要看 `gate` 日志里那几个 `*_RESULT:` 的实际取值。
- 必需检查叫 **`regression gate`**（`.github/workflows/ci.yml` 的 `gate` job
  用 `needs:` 聚合 rust / flutter / proto / android 四项）。它只在上面那几种触发下
  运行。恢复 PR 触发后，每个 PR 会打一次 Android APK，11–15 分钟，是全流程
  最慢的一项，按这个量级估时间。
- （PR 检查恢复后适用）`mergeable=MERGEABLE` 且 `mergeStateStatus=CLEAN`
  才能合；`BLOCKED` 通常只是 gate 还没上报。
- **不要 rebase 已开 PR 的分支**，用 `git merge origin/main` 跟上 main。
- **堆叠 PR**：`gh pr create --base <上一个分支>`，合并顺序必须从下往上。
  注意上一个分支被删时 GitHub 会把堆在它上面的 PR 一并关掉——先把 base
  改到 `main` 再合，或恢复那个 ref 后重开。
- 五端 app 构建（`build.yml` / `win_x64.yml` / `linux_x64.yml` …）保持
  `workflow_dispatch` 手动触发，**不要提议打开 `pull_request` 触发**；需要
  验证构建时手动 dispatch。dispatch 时把不需要的平台显式置 false，
  `tag` 留空——填了 `tag` 会直接建一个 GitHub Release：

  ```sh
  gh workflow run build.yml --ref <分支> \
      -f build_android=true -f build_ios=false -f build_mac=false \
      -f build_win_x64=false -f build_linux_x64=false -f tag=""
  ```

- **取 APK 产物不要用 `gh run download`。** `build.yml` 的上传步骤用了
  `archive: false`，于是：产物名就是**文件名本身**（形如
  `KiliKili_android_<版本>-<短 sha>+<versionCode>_arm64-v8a.apk`），
  `-n Android_arm64-v8a` 这种按平台猜的名字找不到；而且 APK 本身就是 zip，
  `gh run download` 会把它**当压缩包解开**，把七百来个条目摊进目标目录，
  还会因 `res/` 下有大小写同名的条目（`2f.xml` 与 `2F.xml`）在 Windows 上
  中途失败：`error extracting "res/2f.xml": openat 2f.xml: file exists`。
  正确做法是按 artifact id 取原始字节：

  ```sh
  gh api repos/<owner>/<repo>/actions/artifacts -q ...   # 或 .../runs/<id>/artifacts
  gh api repos/<owner>/<repo>/actions/artifacts/<id>/zip > x.apk
  ```

  核对下载大小与 `size_in_bytes` 一致即可确认拿到的是 APK 原文件，
  不是再套一层的压缩包。

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
- **pub cache 的位置由宿主系统决定，与要打给哪个平台无关**：Windows 上是
  `%LOCALAPPDATA%\Pub\Cache`（**不是** `~/.pub-cache`），其余平台是
  `~/.pub-cache`，`PUB_CACHE` 环境变量优先。`lib/scripts/pub-cache.ps1` 是
  这条判断的唯一出处，`patch.ps1` 与 `prepare-build.sh` 都调它——别再在任何
  一侧写死路径（#175）。
- `flutter pub get` **不要带 `PUB_HOSTED_URL` 镜像**：它会把 `pubspec.lock`
  里每条 `url: https://pub.dev` 改写成镜像地址并顺带升级若干包。要用镜像就
  只用在下载 SDK 上。
- 本机没有 fvm 时，issue 模板里写的 `.fvm/flutter_sdk/bin/dart` 按全局
  Flutter SDK 的 `dart` 来用。
- Android 本地签名读 Git 忽略的 `android/key.properties` 与其中指定的 JKS。
  CI 的验证包使用固定的验证密钥，应用 ID 带 `.dev`；带 `tag` 的发布包使用
  **另一把**发布密钥，应用 ID 不带后缀。两种密钥各自的后续版本都须沿用
  原证书，否则不能原地升级。材料与配置见
  [`docs/release/SIGNING.md`](docs/release/SIGNING.md) §6。
- #146 的签名工作流在密钥或公开指纹缺失时直接失败，不再回落到 runner
  临时生成的 debug 签名。**配置好 Actions secrets 与 vars 之前不要触发
  Android 构建或带 `tag` 发布**；旧 CI APK 的签名身份无法继承。
- **发布签名的约定在 [`docs/release/SIGNING.md`](docs/release/SIGNING.md)**，
  五端各一套身份，不共用：Android 的 JKS 只用于 Android，Linux 用 GPG，
  Apple 与 Windows 各按自己的证书流程来。跨会话不变的几条：

  - **私钥、证书、口令不进仓库**；进仓库的只有公钥与指纹
    （`assets/linux/signing-key.asc`），CI 每次构建都拿它比对——这是
    「下一版还是同一个发布者」唯一的锚点，不一致就直接红。
  - **CI 的 GPG 子钥必须无口令。** rpmsign / debsigs / appimagetool 各自去调
    gpg，没有统一的口令传递口子，有口令时它们要么卡在 pinentry 上，要么
    静默产出未签名的包。
  - **可复用工作流默认拿不到调用方的 secrets**：`build.yml` 调四个平台
    workflow 时必须带 `secrets: inherit`，少这一行的话 Secrets 配好了签名
    也照样跳过，而且一声不吭（#146 的形状）。
  - Android 的签名材料缺失时构建失败；其他平台未配 secret 时仍可产出
    **未签名**的包，日志里会写明，不能作为正式发布分发。

---

## 9. 代码风格

- 注释与文档写**中文**，与现有代码保持一致。
- 注释解释**为什么**，不解释代码在做什么；尤其要写下「刻意没做 X」及其
  理由——那是下一个人最容易白走一遍的地方。
- 不要为了「更干净」顺手重构与本次改动无关的代码：上游（PiliPlus）还要
  持续同步，无谓的改动会变成每次同步的冲突点。
