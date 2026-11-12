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
修订：2026-11-11（typed 直连第三轮：取流 #79，已并入 #87）——
实质改动 142 → **150**，仅重命名波及 755 → **745**，新增文件
455 → **455**（新增与删除各有进出，净持平），删除文件 121 → **124**。
「修改」总数 907 → 895。
修订：2026-11-12（弹幕进度游标 issue #89 为本次）—— 实质改动
150 → **152**，仅重命名波及 745 → **742**，新增文件 455 → **459**，
删除文件 124 → **125**，「修改」总数 895 → **894**。本次数字含**尚未提交**
的工作树改动。
修订：2026-11-12（ADR-013 写操作，纯文档）—— 新增文件 459 → **460**
（`docs/architecture/adr/ADR-013-write-operations.md`），其余不变。
修订：2026-11-12（弹幕屏蔽规则的两个写操作，#66 的尾巴，为本次）——
实质改动 152 → **151**，新增文件 460 → **461**，删除文件 125 → **126**，
「修改」总数 894 → **893**，仅重命名波及不变（742）。**实质改动反而少了
一个**：`lib/http/danmaku_block.dart` 整个删掉，从「修改」转进了「删除」。
这是写操作迁移的常态——它和只读切片一样，迁完就把上游那个文件删了。

修订：2026-11-12（竖版视频封面改大卡片 + 底部叠加文案，issue #108，
为本次）—— 实质改动 151 → **165**，仅重命名波及 742 → **729**，新增文件
461 → **467**，「修改」总数 893 → **894**（`lib/common/style.dart` 从未改过
变成有改动），删除文件不变（126）。这是**第一次因为
界面改动而扩大分叉面**：以前每一轮都是迁业务逻辑，这一轮动的是上游的
视图层，性质不同，见 §4.4。本次数字含**尚未提交**的工作树改动。

这一版的差额**不全是本次改动**：上一版是在 #79 的工作树上算的，#90 合进
main 前的最后一个提交（CDN 判据跟着算法搬家）没算进去——它删掉四个 Dart
测试、新增 `bili-core/src/{cdn,playback}.rs` 与 `test/bridge/playback_bridge_test.dart`，
`test/utils/cdn_url_test.dart` 从「修改」转成「删除」。**本次自己只动了三个
数**：新增 +1（`bili-danmaku/src/cursor.rs`），以及
`lib/pages/danmaku/view.dart`、`lib/pages/video/introduction/local/controller.dart`
两个文件从「仅重命名波及」转入「有实质改动」。

取流这一片的分叉面与前几片不同：除了删上游模型，还**把上游文件里的算法
搬空了**。`lib/utils/video_utils.dart` 的 `getCdnUrl`（两条正则 + mcdn /
szbdyd 分支）与 `findTargetVideo` / `findTargetAudio` 的原实现整体移到
`bili-core`，上游文件里只剩转发。同步上游时这几处的冲突**不要照上游解**
——上游改的是实现，我们这边已经没有实现了，要改的是 Rust 侧
（见 RFC §2.3 与 ADR-011 补充十三）。

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
| 修改的上游既有文件 | 894 |
| &nbsp;&nbsp;其中：仅因包名重命名而改动 | 729 |
| &nbsp;&nbsp;其中：有实质改动 | **165** |
| 新增文件 | 467 |
| 删除文件 | **126** |

「修改」总数一路下降（935 → 909 → 897 → 893），不是改动变少，而是其中
一部分文件**被删除**了（迁移掉的模型），从 M 转到了 D。#108 把它从 893
顶回 894，是这轮唯一一次反向——界面改动不删文件，只会往上加。删除数从 57 一路
到 125，来源是 S10 的 PGC 两批、S11a 的直播三批与取流（#79）。

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

165 个。其中 **126 个来自业务层 typed 直连迁移**（§4.1），性质高度一致，
不必逐个记；**14 个来自竖版封面卡片改版**（§4.4），性质同样一致；剩下
25 个是逐个有来由的，列在 §4.3 的表里。

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
| 弹幕屏蔽规则的两个写操作 | #66 #50 / 本次 | `lib/pages/danmaku_block/controller.dart`、`lib/pages/video/widgets/header_control.dart`；**删掉 `lib/http/danmaku_block.dart`**。仓库头两条走 Rust 的写操作，形状见 ADR-013 |
| S11a 直播第三批（剩余六条只读） | #61 / #87 | `lib/http/live.dart`、`lib/pages/{live_emote,live_search,live_dm_block}/**`、`lib/pages/live_room/{controller.dart,contribution_rank/**}`、`lib/pages/member/{view.dart,widget/{medal_wall,user_info_card}.dart}`、`lib/pages/common/publish/common_rich_text_pub_page.dart`；留下单向适配 `lib/services/rust/live_rest_compat.dart` |

**同步时怎么处理这一类冲突**：上游在这些文件里的改动，多数落在我们已经
删掉的模型或函数上。判据是「这段逻辑现在归谁」——

- 改的是**协议理解**（字段名、取值含义、请求参数）→ 同步到 `rust/crates/
  bili-api` 的对应模块，并重新生成该切片的 golden 快照；
- 改的是**界面措辞或交互**→ 照常同步到 Dart 调用点；
- 改的是**已删除的模型文件**→ 丢弃上游该改动，但要确认它不属于前一类。

不要因为冲突就把上游版本整个取回：那会把已经删掉的 Dart 解析路径重新
引回来，而 Rust 侧的同名路径仍在，变成两份实现并存且只有一份被调用。

### 4.2 删除的上游文件

126 个：被 typed 结构取代的模型、迁空的 `lib/http` 文件，以及判据搬家后
随之作废的 Dart 测试。

| 区域 | 数量 | 内容 |
| --- | ---: | --- |
| `lib/models_new/` | 117 | live（39）、pgc（25）、reply（18）、video 的 ai_conclusion/note_list/play_info/video_shot（12）、emote（4）、match（4）、popular（4）、reply2reply（3）、blacklist（2）、music（2）、reply_interaction（2）、search（1）、sponsor_block（1） |
| `lib/models/` | 3 | `search/search_esports.dart`、`user/danmaku_block.dart`、`video/play/url.dart` |
| `lib/http/` | 3 | `black.dart`、`match.dart`（整个文件只有只读接口，迁完即空）、`danmaku_block.dart`（两个写操作迁完即空） |
| `test/` | 3 | `utils/{audio_quality_select,cdn_url,video_quality_select}_test.dart`——判据随 #79 搬进 `bili-core`，Dart 这边的用例跟着搬走 |

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
| `lib/grpc/grpc_req.dart` | issue #42 | 抽出 `_post`，新增 `requestRaw`；`request` 行为不变 |
| `lib/pages/danmaku/view.dart` | 本次（issue #89） | `videoPositionListen` 的 `latestAddedPosition` 换成 Rust 的进度游标（`bili_danmaku::cursor`）；该不该取桶、要不要先清屏都由 Rust 判定 |
| `lib/pages/video/introduction/local/controller.dart` | 本次（issue #89） | 单集列表循环播放改走 `PlPlayerController.seekTo`，不再直接调 `videoPlayerController.seek` |
| `test/utils/accounts/deleted_account_test.dart` | `83244c723` | 适配 CI 上打补丁后的 Flutter |
| `assets/linux/DEBIAN/control` | `5ecebd8f8` 及本次 | 依赖声明改由构建期生成（见 §5） |
| `.github/workflows/linux_x64.yml` | `d9b23ba4e` `45cfe832c` `6d35ee6e9` 及本次 | RPM 选包修复、产物体积闸门、依赖自动生成 |
| `.github/workflows/mac.yml` | `6d35ee6e9` | 产物体积闸门 |
| `.github/workflows/win_x64.yml` | `6d35ee6e9` | 产物体积闸门 |

`lib/plugin/pl_player/controller.dart` 不在这张表里，它已经由 §4.1 的 S3 一行
覆盖；issue #89 又给它加了 seek 代际 `_seekEpoch`，这里不重复计数。那条代际
带来一个**跨文件的约束**：所有跳转都必须走 `seekTo`，绕过去的跳转不会自增
代际，弹幕会把它当成回跳而整段不再显示。同步上游时若引入新的
`videoPlayerController.seek(...)` 调用点，要一并改掉。

`.github/workflows/build.yml` 与 `ios.yml` 也在分叉面里，但**只有包名重命名**，
没有逻辑改动。

### 4.4 竖版封面卡片改版触及的上游文件

issue #108 把所有**竖版**视频封面从「顶部封面图 + 底部独立文案区」改成
「整张封面 + 底部叠加文案」，遮罩配色由 `bili_core::cover_scrim` 按封面
取色算出。横版卡（搜索结果、收藏、历史、稍后再看…）**刻意没动**：那批
是列表密度优先的布局，改成大封面会让一屏能看的条目数明显下降，范围另议。

| 文件 | 性质 |
| --- | --- |
| `lib/common/widgets/video_card/video_card_v.dart` | 首页推荐卡，改用新的 `CoverScrimCard` |
| `lib/common/style.dart` | 新增 `coverCardRatio`（4:3）与 `coverCardRadius`（14）两个常量 |
| `lib/common/widgets/video_popup_menu.dart` | 新增 `onCover`：压在遮罩上时图标改白色 + 阴影 |
| `lib/common/skeleton/video_card_v.dart` | 骨架跟着变成整块封面，否则加载完会重排 |
| `lib/pages/{rcmd,live,live_follow,live_search,live_area_detail,search_panel/live,pgc,pgc_index,member_pgc,member_coin_arc,member_like_arc,member_home}/**` | 卡片本体改用 `CoverScrimCard`；对应 grid delegate 去掉给文字留的 `mainAxisExtent`（格子现在就是封面本身），视频/直播格子的 `childAspectRatio` 从 16:10 收到 4:3 |

卡片上的信息分三层，改哪一层决定了去哪里改：**底部遮罩**里的标题与辅助
信息，颜色由 `cover_scrim` 保证对比度，不要另外指定颜色；**封面左上角**的
UP 名与**右上角**的时长等角标落在封面上，不在那套保证里，所以一律自带
`black45` 底 + 白字；`CoverScrimCard` 自带的阴影与 14px 圆角是**有意偏离
全局 `cardTheme`** 的（那边 `shadowColor` 是透明），理由写在该类的
`elevation` 字段注释上。

**同步时怎么处理这一类冲突**：上游若改这些文件里的**信息**（多了一个角标、
换了一个字段），照常同步到新的卡片结构里；若改的是**布局**（padding、
Column 顺序、文字层级），丢弃——那一层我们已经整个换掉了。遮罩的取色与
对比度在 Rust，上游不会有对应改动。

这一类与 §4.1 的区别值得记一笔：§4.1 是把业务逻辑搬出宿主，分叉面是
「上游文件变空」；这里是**纯视图层改动**，上游文件还在、还是 Widget 树，
只是树形不同。所以它不适用 §4.1 的「协议改动同步到 bili-api」判据。

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

467 个，不与上游冲突，按区域分布：

| 区域 | 文件数 | 内容 |
| --- | ---: | --- |
| `rust/` | 298 | Rust workspace：6 个 crate、golden fixture 与快照 |
| `rust_builder/` | 46 | cargokit，把 Rust 接入 Flutter 各端构建 |
| `lib/services/` | 33 | Rust 开关、shadow mode、各切片的取数封装 |
| `test/` | 26 | parity 测试与桥接层集成测试 |
| `lib/rust/` | 24 | FRB 生成的 Dart 绑定 |
| `tool/` | 13 | 构建准备、FRB 生成、README 统计、产物核对、fixture 生成 |
| `docs/` | 14 | RFC、ADR、M0 记录、QA 清单 |
| `lib/common/` | 2 | 竖版封面大卡片与直播卡共用的文案行（issue #108） |
| `lib/pages/` | 1 | 从控制器里搬出来的基线弹幕引擎（`danmaku/engine.dart`） |
| `lib/utils/` | 1 | 从 `SubtitleUtils` 里搬出来的基线字幕转换（`subtitle_format.dart`） |
| 其他 | 9 | workflow、`AGENTS.md` / `CLAUDE.md`、README 统计图、`flutter_rust_bridge.yaml` |

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
