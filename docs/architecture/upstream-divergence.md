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
修订：2026-11-12（短评五条 #60 + 想听/分组排序 #66，为本次）—— 实质改动
151 → **152**，仅重命名波及 742 → **739**，新增文件 461 → **465**，
删除文件 126 → **128**，「修改」总数 893 → **891**。
删掉 `lib/http/music.dart` 与 `lib/http/pgc.dart` 两个整文件；
`lib/http/follow.dart`、`lib/pages/follow_tag_sort/view.dart`、
`lib/pages/pgc_review/post/view.dart` 三个从「仅重命名」转入「有实质改动」。
修订：2026-11-12（头像主题色边框 #109）——实质改动 152 → **154**，
仅重命名波及 739 → **737**；修改总数仍为 891，新增和删除数不变。
`NetworkImgLayer` 与“我的”页从仅重命名文件转为有实质改动。
修订：2026-11-12（自动播放默认值 #16）——实质改动 154 → **155**，
仅重命名波及 737 → **736**；修改总数仍为 891，新增和删除数不变。
`storage_pref.dart` 从仅重命名转为有实质改动。
修订：2026-11-12（关注/粉丝列表，#66 收尾，为本次）—— 实质改动
154 → **157**，仅重命名波及 737 → **732**，新增文件 465 → **469**，
删除文件 128 → **130**，「修改」总数 891 → **889**。本次数字含**尚未提交**
的工作树改动。`lib/http/{follow,fan}.dart` 两个整文件删掉（从 M 转进 D，
其中 `follow.dart` 上一版刚进「有实质改动」，这一版就删干净了）；
`lib/http/dynamics.dart`、`lib/models/dynamics/up.dart`、
`lib/pages/fan/controller.dart`、`lib/pages/follow/child/child_controller.dart`
四个从「仅重命名」转入「有实质改动」。

修订：2026-11-12（竖版视频封面改大卡片 + 底部叠加文案，issue #108，
为本次）—— 实质改动 154 → **168**，仅重命名波及 737 → **724**，新增文件
465 → **471**，「修改」总数 891 → **892**，删除文件不变（128）。这是
**第一次因为界面改动而扩大分叉面**：以前每一轮都是迁业务逻辑，这一轮动的
是上游的视图层，性质不同，见 §4.4。

修订：2026-11-13（issue #108 横版视频及合集封面统一为大卡片）——
实质改动 168 → **217**，仅重命名波及 724 → **675**；修改的上游文件
仍为 892。新增文件按当前分支重算为 **472**，其中 `lib/common/` 为 3 个。
本轮新增的 49 个实质改动文件集中在搜索、收藏、历史、稍后再看、离线缓存、
动态、播放列表及其共用卡片，见 §4.4。

修订：2026-11-13（合入最新 main，包含关注/粉丝列表 #111）——按 §6 重算：
修改的上游文件 **890**，其中实质改动 **220**、仅重命名 **670**；新增
**476**，删除 **130**。这组数包含两条分支各自的改动，不把同一文件重复计数。

修订：2026-11-13（#108 封面角标等高）——新增一个布局回归测试，新增文件
**476 → 477**；修改的上游文件集合与其分类不变。

修订：2026-11-13（#122 导航玻璃胶囊、#124 封面顶部信息行）——按 §6 对当前
工作树重算：修改的上游文件仍为 **890**，其中实质改动 **220 → 224**、
仅重命名 **670 → 666**；新增文件 **477 → 478**，删除仍为 **130**。
新增实质改动的四个上游文件是 `floating_navigation_bar.dart`、
`pages/main/view.dart`、`models_new/{history,later}/list.dart`。

修订：2026-11-13（#122 Liquid Glass、#105 完整选中态）——继续修改上述导航文件，
并增加一个折射着色器；`pubspec.yaml` 已属实质改动文件，修改文件的分类与数量
不变，新增文件 **478 → 479**，删除文件不变。

修订：2026-11-13（#128 底部导航左右转场）——`lib/pages/main/controller.dart`
从仅重命名变成实质改动；修改的上游文件仍为 **890**，其中实质改动
**224 → 225**、仅重命名 **666 → 665**；新增和删除文件数不变。

修订：2026-11-13（合入自动播放默认值 #16）——按 §6 的分类，
`storage_pref.dart` 从仅重命名转为有实质改动：实质改动 **225 → 226**，
仅重命名 **665 → 664**；修改和删除文件总数不变。合入 ADR-012 #49 后，
新增文件 **479 → 480**（`docs/architecture/adr/ADR-012-grpc-transport.md`）。

前述 #89 修订的差额**不全是那次改动**：上一版是在 #79 的工作树上算的，#90 合进
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

修订：2026-11-13（#106 动态页空态操作）——修改上游的
`lib/common/widgets/loading_widget/http_error.dart`、`lib/pages/dynamics/view.dart`、
`lib/pages/dynamics_tab/view.dart`。按 §6 检查，这三个文件此前都只被重命名提交
`ba430476b` 触及，本次转为实质改动：实质改动 **226 → 229**、
仅重命名 **664 → 661**；修改总数、新增及删除数不变。
本次统一错误态胶囊内的图标与文案，匿名态导航到登录页，登录态刷新动态及 UP 列表。

修订：2026-11-13（#133 gRPC 公共前置）——按 §6 对当前 index 重算：
上游修改文件 **890**，其中实质改动 **230**、仅重命名 **660**；
新增 **510**、删除 **144**。生成器输入快照等另有 10 项重命名，沿用
现有统计口径，不计进 A/D；数字包含当前工作树改动。metadata 编码、帧与传输
迁到 Rust，旧字幕 JSON 桥接/降级及无消费者 metadata 生成文件删除；Device
仍被 dynamic/v2 间接消费，随 S5/S6 的最后消费者迁完再删。

修订：2026-11-13（#141 播放页控制层与标题展开动画）——按 §6 对当前工作树
重算：上游修改文件仍为 **890**，其中实质改动 **230 → 231**、仅重命名
**660 → 659**；新增 **510 → 511**，删除仍为 **144**。
`lib/common/widgets/expandable.dart` 从仅重命名转为有实质改动；
同一次改动还碰了 `lib/plugin/pl_player/view/view.dart`（控制层出现/隐藏
100 → 60ms）与 `lib/pages/video/introduction/ugc/view.dart`（标题旁加三角
指示），这两个已由 §4.1 的 S3 行覆盖，不重复计数。
顺带修正 §5：那张按区域分布的表自 #133 起就没更新，总数还停在 480。

修订：2026-11-13（#143 单击显隐控制层的空等）——按 §6 对当前工作树重算：
上游修改文件 **890 → 891**，其中实质改动 **231 → 232**、仅重命名仍为
**659**；新增 **511**、删除 **144** 不变。`lib/scripts/double_tap_gesture.patch`
此前与上游逐字相同（连包名重命名都没碰到它），本次首次改动，直接进入
「有实质改动」。`lib/plugin/pl_player/view/view.dart` 已由 §4.1 的 S3 行覆盖，
不重复计数。

修订：2026-11-13（S4 视频页写操作 #54，为本次）——按 §6 对当前工作树
重算：上游修改文件仍为 **891**，其中实质改动 **232 → 238**、仅重命名
**659 → 653**；新增 **511 → 521**，删除 **144 → 146**。

六个上游文件从仅重命名转为有实质改动：`lib/pages/article/controller.dart`、
`lib/pages/fav/pgc/controller.dart`、`lib/pages/member/controller.dart`、
`lib/pages/video/reply/widgets/reply_item_grpc.dart`、
`lib/pages/whisper_link_setting/controller.dart`、`lib/utils/request_utils.dart`
——它们都是改调用点，把 `VideoHttp` 的写操作换成 `RustVideoWrite`。另外
十一个被触及的上游文件（`lib/http/video.dart`、`lib/pages/video/**`、
`lib/plugin/pl_player/controller.dart` 等）已由 §4.1 的既有行覆盖，不重复计数。

删除的两个是 `lib/models_new/triple/{ugc,pgc}_triple.dart`——这是**第一次
删掉上游从未被我们改过的文件**：它们此前连包名重命名都没碰到（`fromJson`
里没有 import），既不在「修改」里也不在「仅重命名」里，直接从未触及转进
「删除」。所以本次修改总数一个没动，删除数 +2。

修订：2026-11-14（S5 #55 的动态红点 gRPC 路径）——`lib/grpc/dyn.dart`
从仅包名重命名转为实质改动，`lib/pages/main/controller.dart` 此前已因 #128
列入实质改动。按 §6 对合并后的工作树重算：上游修改文件仍为 **891**，
其中实质改动 **238 → 239**、仅重命名 **653 → 652**；新增文件
**521 → 526**，删除仍为 **146**。

修订：2026-11-14（S5 #55 继续迁点赞列表、点赞与置顶）——
`lib/pages/member_dynamics/controller.dart` 和
`lib/pages/common/dyn/like_list/{controller,view,widgets/item}.dart`
四个文件此前仅被包名重命名提交触及，本次转为实质改动。`lib/http/dynamics.dart`、
`lib/pages/article/controller.dart`、`lib/utils/request_utils.dart` 已在既有实质
改动集合里。按 §6 对本轮工作树核对：上游修改文件仍为 **891**，实质改动
**239 → 243**、仅重命名 **652 → 648**；新增 **526**、删除 **146** 不变。

修订：2026-11-15（S5 #55 预约功能迁 Rust）——动态发布页、预约创建页和动态附加卡片的
三个上游文件从仅重命名转为实质改动；`lib/http/dynamics.dart` 已计入既有实质改动。
移除两个预约响应模型。按 §6 的集合口径，本分支在上次统计基础上实质改动
**243 → 246**、仅重命名 **648 → 645**，修改总数仍为 **891**；新增仍为 **526**，
删除 **146 → 148**。

修订：2026-11-15（S5 #55 投票查看、提交与管理迁 Rust）——动态投票卡片、创建页、
选项编辑页和投票摘要模型四个上游文件转为实质改动；删除关注者投票列表模型。
按 §6 的集合口径，实质改动 **246 → 250**、仅重命名 **645 → 641**，修改总数仍为
**891**；新增仍为 **526**，删除 **148 → 149**。

修订：2026-11-16（#138 首页分类胶囊）——按 §6 检查，
`lib/pages/home/view.dart` 此前仅被重命名提交 `ba430476b` 触及，本次转为
实质改动。同步 main 后按 §6 重算：修改 **890**、实质改动 **253**、
仅重命名 **637**、新增 **527**、删除 **149**；另有 10 项重命名不计入 A/D。
相较同一 main 基线，本切片使实质改动 +1、仅重命名 −1，其余数量不变。
分类增加辅助图标和胶囊背景，选中态跟随现有 TabController，移除底部指示条。
同步上游时保留这层样式与选中态绑定，分类数据和页面切换逻辑仍按上游同步。

修订：2026-11-16（#136 音量竖向胶囊）——修改
`lib/plugin/pl_player/view/view.dart`，该文件此前已被 #79、#141、#143 实质修改，
本次不扩大上游修改文件集合，新增文件 **527 → 529**，并重算 §5 的区域分布。
新增独立展示组件
`lib/plugin/pl_player/widgets/volume_indicator.dart` 与
`test/plugin/pl_player/volume_indicator_test.dart`；手势与音量控制器不变。
同步上游时保留竖向填充展示，只合入控制行为相关的改动。

修订：2026-11-13（#140 短信验证码空白页）——修改上游的
`lib/http/login.dart`、`lib/models/login/captcha_config.dart`、
`lib/pages/login/controller.dart`、`lib/pages/login/geetest/geetest_webview_dialog.dart`；
按 §6 检查，四者此前均只有重命名提交 `ba430476b`，本次各转入实质改动。
新增 `geetest_html.dart` 与其脚本回归测试；原生 iOS 构建补丁保持适用。
本次相对父提交：实质改动 +4、仅重命名 -4、新增 +2，修改及删除总数不变。

修订：2026-11-16（#140 短信登录交互）——`lib/pages/login/view.dart` 此前
仅由包名重命名提交触及，本次转为实质改动；相对本分支上一提交，实质改动
+1、仅重命名 −1，修改总数不变。控制器与 WebView 弹窗已在 #140 修改集合中。
新增验证码输入组件、自适应弹窗外壳及对应 Widget 测试；同步上游时保留焦点、
自动提交防重与自适应约束，验证码请求及签名仍沿用现有实现。

修订：2026-11-16（S14 登录流程迁 Rust，issue #65）——`lib/http/login.dart`
整个删掉，连同它独有的模型 `lib/models_new/login_devices/{data,device}.dart`、
`lib/models/login/captcha_config.dart` 与对应测试
`test/pages/login/geetest/captcha_config_test.dart`。按 §6 分类：
`lib/pages/login_devices/{controller,view}.dart`、`lib/pages/setting/view.dart`
三个此前只被重命名提交 `ba430476b` 触及的文件转为实质改动；四个原在「修改」
里的文件转入「删除」——`login.dart` 与 `captcha_config.dart` 刚被 #140 改成
实质改动，`login_devices/data.dart` 与那个测试则一直是仅重命名；
`login_devices/device.dart` 此前完全未被触及，也直接进入「删除」。

修改 **891 → 887**、实质改动 **258 → 259**、仅重命名 **633 → 628**、
删除 **149 → 154**、新增 **537 → 563**（Rust 侧 5 个模块 + 1 个 golden 测试
+ 16 份 fixture 与快照，Dart 侧 FRB 绑定、取数封装与两个测试各若干）。
`lib/pages/login/controller.dart` 已在 #140 转为实质改动，本次不再重复计入。

这一片的分叉面性质与取流那次相同，但更彻底：**上游文件里不再剩转发**。
登录的签名、RSA 加密、设备号生成、三种状态的判别全部在
`rust/crates/bili-{protocol,domain,api}/src/login*.rs`，宿主侧只剩「把
cookie 存进账号体系」。同步上游时 `lib/http/login.dart` 的任何改动都
**不要照上游解**——那个文件在本 fork 已经不存在，要改的是 Rust 侧
（见 RFC §2.3 与 ADR-011 补充十三）。

修订：2026-11-16（S11b #62，含合入最新 main）——按 §6 对合并后的 index 实测：
修改上游文件 **882**，其中实质改动 **264**、仅重命名 **618**；
新增 **584**、删除 **163**，另有 **10** 项重命名不计入 A/D。
直播 token/历史预取/SC/长连接整体迁入 Rust，旧 TCP、弹幕和 SC 模型、
粉丝勋章兼容层与一次性 parity 删除；聊天、SC、勋章消费者直接使用 typed 数据。
同步上游时将协议/过滤/表情分段改动落在 Rust，宿主仅合布局与交互。
数字包含本次暂存改动，不含被工具自动改写后恢复的编辑器和 macOS 配置。

修订：2026-11-17（S6/S7 #56、#57，合入最新 main 后）——按 §6 对当前
工作树重算：修改上游文件 **826**，其中实质改动 **346**、仅重命名 **480**；
新增 **712**、删除 **271**，另有 **10** 项重命名不计入 A/D。
用户接口、关注分组、历史/稍后再看、空间各列表改为 typed Rust；旧模型与
通过后的一次性 parity 同 PR 删除。动态两条留给 #150，收藏课堂共享模型
仍有生产者，投稿卡片仅保留无解析的视图适配。数字不含工具改写的编辑器配置。

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
| 修改的上游既有文件 | **826** |
| &nbsp;&nbsp;其中：仅因包名重命名而改动 | **480** |
| &nbsp;&nbsp;其中：有实质改动 | **346** |
| 新增文件 | **712** |
| 删除文件 | **271** |

「修改」总数一路下降（935 → 909 → 897 → 891 → 887），部分文件是因迁移被
删除，从 M 转到了 D。#108 的界面改动一度把它顶回 892；合入 #111 的关注/
粉丝切片后是 890，#140 的登录修复使其回到 891，S14 登录切片再删掉四个后
是 887；S11b 后为 882，S6/S7 后为 826。删除数到 271，新增来源主要是
空间、个人信息、关注分组、历史与稍后再看的旧模型。

## 3. 包名重命名：分叉面的主体

`ba430476b chore: rename app package identifiers to KiliKili` 把每个 Dart
文件的 `package:PiliPlus/` 改成了 `package:kilikili/`，一次触及 992 个文件。
**当前分叉面里 480 个文件只有这一处差异。**

对同步的影响很具体：上游每改动一个 Dart 文件，我们这边大概率冲突，但
**冲突内容永远是那一行 import**。处理方式是取上游版本后重新套用重命名，
而不是逐个手工合并。值得为此准备一条命令，而不是每次人肉处理。

这也意味着：**冲突数量多不代表分叉深**。真正需要人判断的是下面 §4 的
那些文件，其中大部分又集中在 §4.1 的一类上。

## 4. 有实质改动的上游文件

346 个。业务层 typed 直连迁移集中列在 §4.1，竖版与横版封面卡片改版
见 §4.4，其余改动逐个列在 §4.3。

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
| 弹幕屏蔽规则的两个写操作 | #66 #50 / #100 | `lib/pages/danmaku_block/controller.dart`、`lib/pages/video/widgets/header_control.dart`；**删掉 `lib/http/danmaku_block.dart`**。仓库头两条走 Rust 的写操作，形状见 ADR-013 |
| 短评五条写操作 | #60 #50 / 本次 | `lib/pages/pgc_review/**`；**删掉 `lib/http/pgc.dart`**（迁完整个文件只剩这五条） |
| 想听 / 关注分组排序 | #66 #50 / #104 | `lib/pages/music/view.dart`、`lib/pages/follow_tag_sort/view.dart`、`lib/http/follow.dart`（瘦身）；**删掉 `lib/http/music.dart`** |
| S15 关注/粉丝列表（#66 收尾） | #66 / 本次 | `lib/pages/follow/child/child_controller.dart`、`lib/pages/fan/controller.dart`、`lib/http/dynamics.dart`（`followings` 改吃 Rust，同一条接口不留第二份实现）、`lib/models/dynamics/up.dart`；**删掉 `lib/http/{follow,fan}.dart`**；临时单向适配与 `FollowData` 后随 S6/S7 删除 |
| S4 视频页写操作（点赞/点踩/投币/三连/关注/删评论/追番/四条上报/不感兴趣） | #54 #50 / 本次 | `lib/http/video.dart`（瘦身到只剩 `replyAdd` 与四条只读）、`lib/common/widgets/video_popup_menu.dart`、`lib/pages/{article,blacklist,member,fan,whisper_link_setting,live_room,fav/pgc}/controller.dart`、`lib/pages/common/common_intro_controller.dart`、`lib/pages/dynamics/widgets/author_panel.dart`、`lib/pages/video/{controller.dart,introduction/{ugc,pgc}/controller.dart,reply/widgets/reply_item_grpc.dart}`、`lib/plugin/pl_player/controller.dart`、`lib/utils/request_utils.dart`；**删掉 `lib/models_new/triple/{ugc,pgc}_triple.dart`**。`replyAdd` 刻意留在 Dart，理由见 ADR-013 |
| S11a 直播第三批（剩余六条只读） | #61 / #87 | `lib/http/live.dart`、`lib/pages/{live_emote,live_search,live_dm_block}/**`、`lib/pages/live_room/{controller.dart,contribution_rank/**}`、`lib/pages/member/{view.dart,widget/{medal_wall,user_info_card}.dart}`、`lib/pages/common/publish/common_rich_text_pub_page.dart`；留下单向适配 `lib/services/rust/live_rest_compat.dart` |

| S6/S7 用户与空间 | #56 #57 / 本次 | `lib/http/member.dart`、`lib/pages/{member*,history*,later*,follow*,user*,privacy*}/**` 与共享卡片/多选层；**删掉 `lib/http/user.dart`、`lib/grpc/space.dart` 及独有模型**。`memberDynamic` / `dynSearch` 留给 #150；`SpaceArchiveItem` 与 `SpaceWebVideoModel` 仅作界面适配，收藏课堂模型随剩余生产者迁移删除 |

**同步时怎么处理这一类冲突**：上游在这些文件里的改动，多数落在我们已经
删掉的模型或函数上。判据是「这段逻辑现在归谁」——

- 改的是**协议理解**（字段名、取值含义、请求参数）→ 同步到 `rust/crates/
  bili-api` 的对应模块，并重新生成该切片的 golden 快照；
- 改的是**界面措辞或交互**→ 照常同步到 Dart 调用点；
- 改的是**已删除的模型文件**→ 丢弃上游该改动，但要确认它不属于前一类。

不要因为冲突就把上游版本整个取回：那会把已经删掉的 Dart 解析路径重新
引回来，而 Rust 侧的同名路径仍在，变成两份实现并存且只有一份被调用。

### 4.2 删除的上游文件

271 个，与 §2 同口径，按实际路径分类：

| 区域 | 数量 | 内容 |
| --- | ---: | --- |
| `lib/models_new/` | 232 | 空间 60、直播 47、PGC 25、评论 18、视频 13、member 10，及历史、稍后再看、账号与其他已迁模型；包含 triple 两个模型 |
| `lib/models/` | 9 | 旧用户资料/计数及 Hive 适配器、member 资料、登录配置、赛事、弹幕规则与取流模型 |
| `lib/http/` | 9 | `black`、`danmaku_block`、`fan`、`follow`、`login`、`match`、`music`、`pgc`、`user` |
| `lib/grpc/` | 15 | metadata 生成代码 14 个，以及迁空的 `space.dart` |
| `lib/tcp/` | 1 | 直播弹幕 TCP 宿主实现 |
| `lib/pages/` | 1 | 无消费者的 REST 图文卡片 |
| `test/` | 4 | 旧验证码配置测试与三项已下沉 Rust 的音画质/CDN 测试 |

`lib/models_new/live/live_feed_index/watched_show.dart` 仅剩动态响应生产者，
随 #150 迁移删除；其余已迁的直播模型不再保留。空间课堂模型仍被收藏课堂
生产者消费；无 `fromJson` 的投稿视图适配随共享视频卡片基类删除。

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
| `lib/common/widgets/image/network_img_layer.dart` | #109 | 头像图片与占位图统一绘制主题色圆边框 |
| `lib/pages/mine/view.dart` | #109 | 未登录头像改走共用头像组件，保持边框一致 |
| `lib/common/widgets/floating_navigation_bar.dart`、`lib/pages/main/view.dart` | #122 | 三种首页底栏共用玻璃背景、胶囊裁切、4px 内容内边距与边缘留白；安全区仅在胶囊外处理，分栏中心对齐选中背景，图标与文字组合居中，降低底色与高光的不透明度；保留导航与隐藏行为 |
| `lib/pages/main/controller.dart` | #128 | 底部导航在页面间按方向滑动 240 毫秒；侧栏仍直接切换 |
| `lib/scripts/double_tap_gesture.patch` | #143 | 给 Flutter SDK 的 `DoubleTapGestureRecognizer` 加一个 `doubleTapTimeout` 参数（默认仍为 `kDoubleTapTimeout`，只有播放器选择性调短）；**改了它就多一个只在打过补丁的 SDK 上才存在的符号**，见 AGENTS §5 |
| `lib/common/widgets/expandable.dart` | #141 | 展开/折叠时长 300 → 180ms，抽成共用常量 `kIntroExpandDuration`，并新增标题旁的三角指示 `ExpandIndicator` |
| `lib/http/video.dart` | `f321aea73` `43827d42f` `9385cf06d` 及 S3 | 视频详情与取流接入 Rust（**仍是 JSON 往返 + 开关**，见 #79）；S3 又从中删去了五条只读接口，见 §4.1 |
| `lib/http/search.dart` | `130b9f533`、S9 | 搜索建议接入 Rust；S9 之后整条搜索是 typed 直连、**无开关无回退**，见 §4.1 |
| `lib/pages/setting/models/play_settings.dart` | `9d1d5c3c9` | 自动播放默认值改为开启（issue #16） |
| `lib/utils/storage_pref.dart` | issue #16 | 未设置时默认开启自动播放，与设置页开关的默认值一致；已保存的选择不变 |
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

### 4.4 封面卡片改版触及的上游文件

issue #108 把视频封面从「顶部封面图 + 底部独立文案区」改成
「整张封面 + 底部叠加文案」，遮罩配色由 `bili_core::cover_scrim` 按封面
取色算出。搜索结果、收藏、历史、稍后再看等原先的横向图文行也使用
同一组件；格子改为按封面比例排布。离线缓存优先从本地封面文件取色，
断网时文案仍与实际显示的封面匹配。

| 文件 | 性质 |
| --- | --- |
| `lib/common/widgets/video_card/video_card_v.dart` | 首页推荐卡，改用新的 `CoverScrimCard` |
| `lib/common/style.dart` | 新增 `coverCardRatio`（4:3）与 `coverCardRadius`（14）两个常量 |
| `lib/common/widgets/video_popup_menu.dart` | 新增 `onCover`：压在遮罩上时图标改白色 + 阴影 |
| `lib/common/skeleton/video_card_v.dart` | 骨架跟着变成整块封面，否则加载完会重排 |
| `lib/pages/{rcmd,live,live_follow,live_search,live_area_detail,search_panel/live,pgc,pgc_index,member_pgc,member_coin_arc,member_like_arc,member_home}/**` | 卡片本体改用 `CoverScrimCard`；对应 grid delegate 去掉给文字留的 `mainAxisExtent`（格子现在就是封面本身），视频/直播格子的 `childAspectRatio` 从 16:10 收到 4:3 |
| `lib/common/widgets/video_card/video_card_h.dart`、`lib/utils/grid.dart` | 横版视频卡改用共用遮罩，并让搜索、热门、相关视频等格子按 4:3 排布 |
| `lib/pages/{search_panel,member_search,video/reply_search_item,history,history_search,later,later_search,fav_detail,fav_search,fav_sort,fav,member_video,member_video_web,member_favorite,member_season_series,member_cheese,music,subscription,subscription_detail,download}/**` | 视频、番剧、课程、收藏夹与离线缓存原有横向图文行改为整张封面卡；保留选择、进度、菜单与跳转入口 |
| `lib/pages/{dynamics,video/medialist}/**`、`lib/services/rust/rust_cover_scrim.dart` | 动态与播放列表共用遮罩；离线文件经同一 Rust 算法取色 |

卡片上的信息分三层，改哪一层决定了去哪里改：**底部遮罩**里的标题、UP 名
及辅助信息，颜色由 `cover_scrim` 保证对比度；UP 名的主题色药丸也由 Rust
根据当前主题色与封面合成结果选出底色、文字色；**封面顶部**的视频播放量、
弹幕数和时长共用紧贴边缘的单行深色渐变层（#124），与底部遮罩的 Rust
对比度保证分开。历史与稍后再看的徽标随之下移，避免压住信息行；直播围观数
仍用独立的 `black45` 药丸。`CoverScrimCard` 自带的阴影与 14px 圆角是**有意偏离
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

712 个，不与上游冲突，按区域分布：

| 区域 | 文件数 | 内容 |
| --- | ---: | --- |
| `rust/` | 480 | Rust workspace：7 个 crate、golden fixture 与快照 |
| `rust_builder/` | 46 | cargokit，把 Rust 接入 Flutter 各端构建 |
| `lib/services/` | 45 | 各切片的 typed 取数封装、会话寿命与传输适配 |
| `test/` | 39 | parity 测试与桥接层集成测试 |
| `lib/rust/` | 34 | FRB 生成的 Dart 绑定 |
| `tool/` | 23 | 构建准备、FRB 生成、proto 生成、README 统计、产物核对、fixture 生成 |
| `docs/` | 22 | RFC、ADR、M0 记录、QA 清单 |
| `shaders/` | 1 | Liquid Glass 背板折射着色器 |
| `lib/common/` | 3 | 封面大卡片、统计药丸与直播卡共用的文案行（issue #108） |
| `lib/pages/` | 7 | 基线弹幕引擎（`danmaku/engine.dart`）、验证码弹窗外壳、极验页面脚本、短信验证码输入框、SC 展示状态、id 多选控制器与空间预约卡片 |
| `lib/plugin/` | 1 | 播放器竖向音量指示组件（issue #136） |
| `lib/utils/` | 1 | 从 `SubtitleUtils` 里搬出来的基线字幕转换（`subtitle_format.dart`） |
| 其他 | 10 | workflow、`AGENTS.md` / `CLAUDE.md`、README 统计图、`flutter_rust_bridge.yaml` |

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
