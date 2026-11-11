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
修订：2026-11-11（业务层 typed 直连：S13 #64、S3 #53、S9 #59、S1 #51、
S15 #66 已合并）—— 实质改动 23 → **90**，仅重命名波及 943 → **845**，
新增文件 255 → **376**，**删除文件 0 → 57**。分叉面的性质从这一轮起变了：
以前是「在上游文件里插开关」，现在是**删上游文件**（见 §4.1 与 §4.2）
修订：2026-11-11（typed 直连第二轮：S10 两批 #73 #83、S11a 第一批 #76
已合并，S11a 第二批为 #86）—— 实质改动 90 → **130**，仅重命名波及
845 → **779**，新增文件 376 → **428**，删除文件 57 → **105**。
「修改」总数随之从 935 降到 909：又有一批上游模型从 M 转成了 D
修订：2026-11-11（S11a 第三批，剩余六条只读，为本次）—— 实质改动
130 → **142**，仅重命名波及 779 → **755**，新增文件 428 → **455**，
删除文件 105 → **121**。`lib/models_new/live/` 累计删到 39 个，超过
pgc 成为删得最多的一块
关联：RFC-0001 §10.1（分支与 upstream 同步）、ADR-011（typed 直连）

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
| 修改的上游既有文件 | 897 |
| &nbsp;&nbsp;其中：仅因包名重命名而改动 | 755 |
| &nbsp;&nbsp;其中：有实质改动 | **142** |
| 新增文件 | 455 |
| 删除文件 | **121** |

「修改」总数持续下降（935 → 909 → 897），不是改动变少，而是其中一部分
文件**被删除**了（迁移掉的模型），从 M 转到了 D。删除数从 57 一路到
121，来源是 S10 的 PGC 两批与 S11a 的直播三批。

## 3. 包名重命名：分叉面的主体

`ba430476b chore: rename app package identifiers to KiliKili` 把每个 Dart
文件的 `package:PiliPlus/` 改成了 `package:kilikili/`，一次触及 992 个文件。
**分叉面里 950 个文件只有这一处差异。**

对同步的影响很具体：上游每改动一个 Dart 文件，我们这边大概率冲突，但
**冲突内容永远是那一行 import**。处理方式是取上游版本后重新套用重命名，
而不是逐个手工合并。值得为此准备一条命令，而不是每次人肉处理。

这也意味着：**冲突数量多不代表分叉深**。真正需要人判断的是下面 §4 的
那些文件，其中大部分又集中在 §4.1 的一类上。

## 4. 有实质改动的上游文件

142 个。其中 **119 个来自业务层 typed 直连迁移**（§4.1），性质高度一致，
不必逐个记；剩下 23 个是逐个有来由的，列在 §4.3 的表里。

### 4.1 typed 直连迁移触及的上游文件

ADR-011 决策四定下「迁一条路径就连同 Dart 原实现一起删」之后，分叉面的
形状变了：**不再是在上游文件里插一个 `rust.*` 开关，而是把上游的 `fromJson`
模型与 `lib/http/*.dart` 里的函数删掉，调用点改吃 FRB 的 typed 结构。**

| 切片 | issue / PR | 触及的上游文件 |
| --- | --- | --- |
| S13 SponsorBlock | #64 / #68 | `lib/http/sponsor_block.dart`、`lib/pages/sponsor_block/*` |
| S3 视频页只读 | #53 / #70 #71 | `lib/http/video.dart`、`lib/pages/hot`、`popular_*`、`rank/zone`、`video/{ai_conclusion,note,related,introduction}`、`plugin/pl_player/*`、`lib/models/model_hot_video_item.dart` |
| S9 搜索 | #59 / #72 | `lib/http/search.dart`、`lib/pages/search*`、`lib/pages/search_panel/**`、`lib/models/search/result.dart`、`lib/utils/recommend_filter.dart` |
| S1 评论 REST 只读 | #51 / #75 | `lib/http/reply.dart`、`lib/utils/reply_utils.dart`、`lib/pages/emote/*`、`lib/pages/dynamics/widgets/author_panel.dart`、`lib/pages/common/publish/common_rich_text_pub_page.dart` |
| S15 杂项 | #66 / #77 | `lib/http/{music,danmaku_block}.dart`、`lib/pages/{music,match_info,blacklist,danmaku_block}/**`、`lib/pages/search_panel/all/*`（赛事卡片）、`lib/models/user/danmaku_rule.dart` |
| S10 PGC 第一批 | #60 / #73 | `lib/http/pgc.dart`、`lib/pages/pgc_*`、`lib/pages/fav/pgc/**`、`lib/models/common/pgc_review_type.dart` |
| S10 PGC 第二批（番剧/课堂详情） | #60 / #83 | `lib/http/pgc.dart`、`lib/pages/video/introduction/pgc/**`、`lib/pages/episode_panel/**`；留下单向适配 `lib/services/rust/pgc_detail_compat.dart` |
| S11a 直播第一批（只读七条） | #61 / #76 | `lib/http/live.dart`、`lib/pages/{live,live_area,live_area_detail,live_follow,live_room}/**`、`lib/pages/dynamics/widgets/live_*`、`lib/services/audio_handler.dart` |
| S11a 直播第二批（取流） | #61 / #86 | `lib/http/live.dart`、`lib/pages/live_room/controller.dart`、`lib/utils/video_utils.dart`（`getLiveCdnUrl` 改吃 FRB 的 `LiveCodec`）；留下单向适配 `lib/services/rust/live_playurl_compat.dart` |
| S11a 直播第三批（剩余六条只读） | #61 / 本次 | `lib/http/live.dart`、`lib/pages/{live_emote,live_search,live_dm_block}/**`、`lib/pages/live_room/{controller.dart,contribution_rank/**}`、`lib/pages/member/{view.dart,widget/{medal_wall,user_info_card}.dart}`、`lib/pages/common/publish/common_rich_text_pub_page.dart`；留下单向适配 `lib/services/rust/live_rest_compat.dart` |

**同步时怎么处理这一类冲突**：上游在这些文件里的改动，多数落在我们已经
删掉的模型或函数上。判据是「这段逻辑现在归谁」——

- 改的是**协议理解**（字段名、取值含义、请求参数）→ 同步到 `rust/crates/
  bili-api` 的对应模块，并重新生成该切片的 golden 快照；
- 改的是**界面措辞或交互**→ 照常同步到 Dart 调用点；
- 改的是**已删除的模型文件**→ 丢弃上游该改动，但要确认它不属于前一类。

不要因为冲突就把上游版本整个取回：那会把已经删掉的 Dart 解析路径重新
引回来，而 Rust 侧的同名路径仍在，变成两份实现并存且只有一份被调用。

### 4.2 删除的上游文件

121 个，全部是被 typed 结构取代的模型与两个只剩空壳的 `lib/http` 文件：

| 区域 | 数量 | 内容 |
| --- | ---: | --- |
| `lib/models_new/` | 117 | live（39）、pgc（25）、reply（18）、video 的 ai_conclusion/note_list/play_info/video_shot（12）、emote（4）、match（4）、popular（4）、reply2reply（3）、blacklist（2）、music（2）、reply_interaction（2）、search（1）、sponsor_block（1） |
| `lib/models/` | 2 | `search/search_esports.dart`、`user/danmaku_block.dart` |
| `lib/http/` | 2 | `black.dart`、`match.dart`（整个文件只有只读接口，迁完即空） |

`lib/models_new/live/` 那 39 个分三批：第一批 16 个（分区、feed、关注、
H5 房间信息）、第二批 7 个（`live_room_play_info/` 整个目录）、第三批
16 个（表情包、搜索、屏蔽、贡献榜、勋章墙）。

**该目录下仍活着的五个文件各有理由**，不是遗漏：`live_danmaku/` 与
`live_dm_info/` 归 S11b（#62）；`live_superchat/` 的 `SuperChatItem` 第二
个生产者是弹幕长连接，整条留给 S11b；`live_medal_wall/uinfo_medal.dart`
是四个生产者的共享勋章模型，两个在 S11b；
`live_feed_index/watched_show.dart` 还有动态与用户信息两个生产者。

上游若改动这些文件，同步时按 §4.1 的判据处理；**不要恢复文件本身**。

### 4.3 其余有实质改动的上游文件

| 文件 | 改动来源 | 性质 |
| --- | --- | --- |
| `.gitattributes` | `f113d9136` | 补全跨平台换行符规则 |
| `.gitignore` | `6b54c9b01` `dd23f968c` | 忽略 Rust `target/` 等产物 |
| `analysis_options.yaml` | `ee41df52c` | 排除 FRB 生成的 Dart 代码 |
| `README.md` / `README.en.md` | `e0b32bfda` `c7dc17c1a` `f92a9890c` | 改写为 KiliKili 的说明与路线图 |
| `pubspec.yaml` / `pubspec.lock` | `ee41df52c` `57485ede8` | 引入 `flutter_rust_bridge` 与 `rust_builder` |
| `lib/main.dart` | `ee41df52c` | 启动时初始化 Rust Core |
| `lib/http/video.dart` | `f321aea73` `43827d42f` `9385cf06d` 及 S3 | 视频详情与取流接入 Rust（**仍是 JSON 往返 + 开关**，见 #79）；S3 又从中删去了五条只读接口，见 §4.1 |
| `lib/http/search.dart` | `130b9f533`、S9 | 搜索建议接入 Rust；S9 之后整条搜索是 typed 直连、**无开关无回退**，见 §4.1 |
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
  Rust 侧：取流那条还是 JSON 往返，错配会被 shadow 比对发现（RFC §16.7）；
  **搜索与其余 typed 直连的路径没有 shadow 这层网**——对照只剩冻结的
  golden 快照，所以同步协议改动时必须同时更新 fixture 并重新生成快照，
  否则差异要到真机上才暴露。
- `pubspec.lock` 冲突时不要手工编辑，重新解析并核对 4 个 Git 依赖的
  revision 是否仍与 `docs/m0/BASELINE.md` §4.1 记录的一致。
- 三个 workflow 的改动都是**产物正确性**相关，不是风格调整，不要因为
  冲突就回退成上游版本。

## 5. 新增文件

376 个，不与上游冲突，按区域分布：

| 区域 | 文件数 | 内容 |
| --- | ---: | --- |
| `rust/` | 226 | Rust workspace：6 个 crate、golden fixture 与快照 |
| `rust_builder/` | 46 | cargokit，把 Rust 接入 Flutter 各端构建 |
| `lib/services/` | 29 | Rust 开关、shadow mode、各切片的取数封装 |
| `test/` | 20 | parity 测试与桥接层集成测试 |
| `lib/rust/` | 20 | FRB 生成的 Dart 绑定 |
| `tool/` | 13 | 构建准备、FRB 生成、README 统计、产物核对、fixture 生成 |
| `docs/` | 13 | RFC、ADR、M0 记录、QA 清单 |
| `lib/pages/` | 1 | 从控制器里搬出来的基线弹幕引擎（`danmaku/engine.dart`） |
| `lib/utils/` | 1 | 从 `SubtitleUtils` 里搬出来的基线字幕转换（`subtitle_format.dart`） |
| 其他 | 7 | workflow、资源 |

`rust/` 的增量主要是 fixture 与快照：每个切片都要带「脱敏后的真实响应 +
冻结的解析结果」两份，这是 typed 直连唯一的长期对照物（ADR-011 决策五）。

## 6. 如何重新计算

这份清单会过时。重算方式：

```sh
# 1. 找到最近一次上游合并带进来的 tip
git log --merges --format='%h %p %s' | grep bggRGjQaUbCoE | head -1
#    第二个父节点就是 UPSTREAM_TIP

# 2. 修改 / 新增 / 删除的上游文件
git diff --name-only --diff-filter=M <UPSTREAM_TIP>..HEAD
git diff --name-only --diff-filter=A <UPSTREAM_TIP>..HEAD
git diff --name-only --diff-filter=D <UPSTREAM_TIP>..HEAD   # 迁移删掉的模型

# 3. 区分「仅重命名波及」与「实质改动」：
#    对每个文件，看 UPSTREAM_TIP..HEAD 之间触及它的非合并提交集合，
#    若只有重命名提交，则归入前者。
git log --no-merges --format='C %h %s' --name-only <UPSTREAM_TIP>..HEAD
```

第 3 步必须按**提交集合**判断，不能用「是否被重命名提交触及」来判断——
重命名提交同时也碰了 workflow 和 DEBIAN 文件，用后者会把真实改动误判成
重命名波及。
