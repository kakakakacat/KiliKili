# 本 fork 与上游的分叉面

状态：**已实测**（由 git 历史计算，非估计）
日期：2026-11-10
修订：2026-11-11 —— 按 §6 的方法重算。实质改动 15 → **16**（新增
`lib/pages/setting/models/play_settings.dart`，`9d1d5c3c9` 默认开启自动播放），
仅重命名波及 951 → 950，新增文件 167 → **187**（弹幕解码 `9d1d5c3c9`）
修订：2026-11-11（弹幕引擎 issue #42、DmView 字幕、清晰度/音质选择、
字幕格式转换）—— 实质改动 16 → **23**，仅重命名波及 950 → **943**
（`lib/grpc/dm.dart`、`lib/grpc/grpc_req.dart`、
`lib/pages/danmaku/controller.dart`、`lib/pages/video/controller.dart`、
`lib/services/download/download_service.dart`、`lib/utils/video_utils.dart`、
`lib/utils/subtitle_utils.dart` 七个文件从「只有重命名」变成有逻辑改动），
新增文件 187 → **255**。本次数字含**尚未提交**的工作树改动
关联：RFC-0001 §10.1（分支与 upstream 同步）

RFC §10.1 规定了要固定 `UPSTREAM_COMMIT` 并记录 rebase/merge 策略，但没有
一份「本 fork 到底改了上游哪些文件」的清单。下一次同步上游时，没有这份
清单就只能靠冲突列表现场判断哪些冲突是自己的改动、哪些是误伤。本文补上。

## 1. 基准

```
UPSTREAM_TIP = 2dd24225b01a4d7b7aab6ac8eb3398c1bf1d0642
  "feat: update offline ugc season info"  2026-11-10
  由合并提交 9756ee5b0 "Merge branch 'bggRGjQaUbCoE:main' into main" 带入
```

**不要用 `docs/m0/BASELINE.md` 里的 `UPSTREAM_COMMIT`（`f52160af0`）来算分叉。**
那是最初的 fork 基线，此后已经合并过一次上游。拿它做 diff 会把上游自己的
后续改动也算成我们的改动——实测会多算出近千个文件。正确基准是**最近一次
上游合并带进来的 tip**，即该合并提交的第二个父节点。

## 2. 数量

| 类别 | 数量 |
| --- | ---: |
| 修改的上游既有文件 | 966 |
| &nbsp;&nbsp;其中：仅因包名重命名而改动 | 943 |
| &nbsp;&nbsp;其中：有实质改动 | **23** |
| 新增文件 | 255 |
| 删除文件 | 0 |

## 3. 包名重命名：分叉面的主体

`ba430476b chore: rename app package identifiers to KiliKili` 把每个 Dart
文件的 `package:PiliPlus/` 改成了 `package:kilikili/`，一次触及 992 个文件。
**分叉面里 950 个文件只有这一处差异。**

对同步的影响很具体：上游每改动一个 Dart 文件，我们这边大概率冲突，但
**冲突内容永远是那一行 import**。处理方式是取上游版本后重新套用重命名，
而不是逐个手工合并。值得为此准备一条命令，而不是每次人肉处理。

这也意味着：**冲突数量多不代表分叉深**。真正需要人判断的只有下面 16 个。

## 4. 有实质改动的上游文件

| 文件 | 改动来源 | 性质 |
| --- | --- | --- |
| `.gitattributes` | `f113d9136` | 补全跨平台换行符规则 |
| `.gitignore` | `6b54c9b01` `dd23f968c` | 忽略 Rust `target/` 等产物 |
| `analysis_options.yaml` | `ee41df52c` | 排除 FRB 生成的 Dart 代码 |
| `README.md` / `README.en.md` | `e0b32bfda` `c7dc17c1a` `f92a9890c` | 改写为 KiliKili 的说明与路线图 |
| `pubspec.yaml` / `pubspec.lock` | `ee41df52c` `57485ede8` | 引入 `flutter_rust_bridge` 与 `rust_builder` |
| `lib/main.dart` | `ee41df52c` | 启动时初始化 Rust Core |
| `lib/http/video.dart` | `f321aea73` `43827d42f` `9385cf06d` 及本次 | 视频详情与取流接入 Rust 路径（带开关与回退）；`getSubtitles` 的字幕转换同样接入 |
| `lib/http/search.dart` | `130b9f533` | 搜索建议接入 Rust 解析（带回退） |
| `lib/pages/setting/models/play_settings.dart` | `9d1d5c3c9` | 自动播放默认值改为开启（issue #16） |
| `lib/pages/danmaku/controller.dart` | 本次（issue #42） | 分段改走 Rust 弹幕引擎（带开关与运行中降级）；原 `handleDanmaku` 的循环原样移入新文件 `lib/pages/danmaku/engine.dart` |
| `lib/grpc/dm.dart` | 本次（issue #42、DmView 字幕） | 新增 `dmSegMobileRaw`（只取回字节交给 Rust）与 `dmViewSubtitles`（字幕解析走 Rust，带开关与回退） |
| `lib/pages/video/controller.dart` | 本次（DmView 字幕） | 取字幕改调 `DmGrpc.dmViewSubtitles` |
| `lib/services/download/download_service.dart` | 本次（DmView 字幕） | 同上 |
| `lib/utils/video_utils.dart` | 本次（清晰度/音质选择） | `findTargetVideo` / `findTargetAudio` 先走 Rust，失败回退原实现 |
| `lib/utils/subtitle_utils.dart` | 本次（字幕转换） | `json2Vtt` / `json2Srt` 的实现移到纯 dart 的 `lib/utils/subtitle_format.dart`，此处只保留委托 |
| `lib/grpc/grpc_req.dart` | 本次（issue #42） | 抽出 `_post`，新增 `requestRaw`；`request` 行为不变 |
| `test/utils/accounts/deleted_account_test.dart` | `83244c723` | 适配 CI 上打补丁后的 Flutter |
| `assets/linux/DEBIAN/control` | `5ecebd8f8` 及本次 | 依赖声明改由构建期生成（见 §5） |
| `.github/workflows/linux_x64.yml` | `d9b23ba4e` `45cfe832c` `6d35ee6e9` 及本次 | RPM 选包修复、产物体积闸门、依赖自动生成 |
| `.github/workflows/mac.yml` | `6d35ee6e9` | 产物体积闸门 |
| `.github/workflows/win_x64.yml` | `6d35ee6e9` | 产物体积闸门 |

`.github/workflows/build.yml` 与 `ios.yml` 也在分叉面里，但**只有包名重命名**，
没有逻辑改动。

### 同步时的注意点

- `lib/http/video.dart` 与 `lib/http/search.dart` 是**上游高频改动区**，
  同时又是 Rust 取数路径的接入点。上游在这两个文件里的改动必须同步到
  Rust 侧的规范化逻辑，否则会出现 Dart 与 Rust 结果不一致而 shadow mode
  才发现的问题（RFC §16.7 提过这个风险）。
- `pubspec.lock` 冲突时不要手工编辑，重新解析并核对 4 个 Git 依赖的
  revision 是否仍与 `docs/m0/BASELINE.md` §4.1 记录的一致。
- 三个 workflow 的改动都是**产物正确性**相关，不是风格调整，不要因为
  冲突就回退成上游版本。

## 5. 新增文件

255 个，不与上游冲突，按区域分布：

| 区域 | 文件数 | 内容 |
| --- | ---: | --- |
| `rust/` | 136 | Rust workspace：6 个 crate 与 fixture |
| `rust_builder/` | 46 | cargokit，把 Rust 接入 Flutter 各端构建 |
| `lib/services/` | 20 | Rust 开关、shadow mode、取数封装、规范化 |
| `lib/rust/` | 14 | FRB 生成的 Dart 绑定 |
| `tool/` | 13 | 构建准备、FRB 生成、README 统计、产物核对、fixture 生成 |
| `docs/` | 10 | RFC、ADR、M0 记录 |
| `lib/pages/` | 1 | 从控制器里搬出来的基线弹幕引擎（`danmaku/engine.dart`） |
| `lib/utils/` | 1 | 从 `SubtitleUtils` 里搬出来的基线字幕转换（`subtitle_format.dart`） |
| 其他 | 14 | workflow、测试、资源 |

## 6. 如何重新计算

这份清单会过时。重算方式：

```sh
# 1. 找到最近一次上游合并带进来的 tip
git log --merges --format='%h %p %s' | grep bggRGjQaUbCoE | head -1
#    第二个父节点就是 UPSTREAM_TIP

# 2. 修改的上游文件
git diff --name-only --diff-filter=M <UPSTREAM_TIP>..HEAD

# 3. 区分「仅重命名波及」与「实质改动」：
#    对每个文件，看 UPSTREAM_TIP..HEAD 之间触及它的非合并提交集合，
#    若只有重命名提交，则归入前者。
git log --no-merges --format='C %h %s' --name-only <UPSTREAM_TIP>..HEAD
```

第 3 步必须按**提交集合**判断，不能用「是否被重命名提交触及」来判断——
重命名提交同时也碰了 workflow 和 DEBIAN 文件，用后者会把真实改动误判成
重命名波及。
