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
工作树重算：修改上游文件 **824**，其中实质改动 **344**、仅重命名 **480**；
新增 **712**、删除 **271**，另有 **10** 项重命名不计入 A/D。
用户接口、关注分组、历史/稍后再看、空间各列表改为 typed Rust；旧模型与
通过后的一次性 parity 同 PR 删除。动态两条留给 #150，收藏课堂共享模型
仍有生产者，投稿卡片仅保留无解析的视图适配。数字不含工具改写的编辑器配置。

修订：2026-11-18（material_ui 1.11.1 + 上游补丁更新 #168）——按 §6 实测：
修改上游文件 **824 → 834**，其中实质改动 **344 → 354**、仅重命名仍 **480**；
新增 **713**、删除 **271** 不变。新进「修改」桶的 10 个全部来自 cherry-pick
上游 `915276c87 update patches`（`lib/scripts/material/` 下 8 个 patch +
`dyn_tab_bar.dart`）。**这 10 个是上游自己的内容，不是本 fork 的分歧**——
§6 以「最近一次上游合并的 tip」为基准，cherry-pick 进来的上游改动会被记成
我们的改动，下次真正 merge 上游后它们会自动归零。
同日 cherry-pick 的另三个上游 commit（`f61cdab35` `3bfb84e37` `217fb54f5`）
未改变任何数字——那几个文件早已在「修改」桶里。

修订：2026-11-18（S8 收藏夹迁 Rust，issue #58，为本次）——按 §6 对当前
index 实测：修改上游文件 **834 → 815**（19 个从 M 转进 D），其中实质改动
**354 → 378**、仅重命名 **480 → 437**；新增 **713 → 742**，删除
**271 → 302**。
`lib/http/fav.dart` 整个删掉，连同 `lib/models_new/fav/` 全部 20 个、
`lib/models_new/sub/{sub,sub_detail}/` 四个、
`lib/models_new/space/{space_fav,space_cheese}/` 六个模型。
`lib/models_new/fav/fav_detail/cnt_info.dart` **不是删除而是搬家**
（移到 `lib/models_new/media_list/`）：播放列表那一片还在用它，而它原来的
目录整个没了。
`lib/services/rust/sub_compat.dart` 也删了——它的注释里写着「收藏夹那片迁完
`favSeasonList` 的那个 PR 应当连同本文件一起删」，这一刻到了。
新进「有实质改动」桶的 22 个上游文件集中在收藏夹各页（详情、搜索、排序、
创建、面板、四个页签）、订阅与合集详情、个人空间收藏页、追番列表与「我的」。
数字不含工作树里本就改着的三个本机文件（编辑器配置与 macOS 构建配置）。

修订：2026-11-18（登出成功后返回首页，issue #164，为本次）——按 §6 对当前
工作树实测：**四个数一个都没变**。改的 `lib/pages/setting/view.dart` 早在
S14 登录切片（`34a34982b`）就进了「有实质改动」桶，本次只是在同一个文件里
多改几行，既不新增文件也不删文件。修改上游文件仍 **815**，其中实质改动
**378**、仅重命名 **437**；新增 **742**、删除 **302**。
记这一条是为了留下「查过了」这个事实——否则下一个人会以为漏了。

修订：2026-11-18（登录页胶囊按钮，issue #162，为本次）——按 §6 对当前
工作树实测：只动一个数，新增 **742 → 743**
（`lib/common/widgets/capsule_tab.dart`）。改的 `lib/pages/login/view.dart`
早在 #140 就进了「有实质改动」桶，修改上游文件仍 **815**，其中实质改动
**378**、仅重命名 **437**，删除 **302** 不变。

胶囊 Tab 没有就地复用首页那份（#138 写在 `lib/pages/home/view.dart` 里）：
那是上游文件，把它抽成公共组件会让这次界面改动顺带改到一块无关的上游代码，
每次同步都要多解一处冲突。新组件放在 `lib/common/`（本 fork 自己的目录，
不参与上游同步），首页那份等有单子统一时再并过来。这是 §4.4 那类
「界面改动扩大分叉面」里成本最低的一种：**只增不改**。

修订：2026-11-19（S2 评论过滤规则下沉 Rust，issue #52，为本次）——按 §6 对
当前分支实测：修改上游文件仍 **812**，其中实质改动 **378 → 380**、
仅重命名 **434 → 432**，新增 **743 → 750**，删除 **302** 不变。
转入「有实质改动」的两个是 `lib/grpc/reply.dart`（四条过滤规则搬走，
只剩取数与应用判定结果）与 `lib/common/constants.dart`（`goodsUrlPrefix`
随规则搬到 `bili_domain::reply_filter`，上游那一份删掉）。

这一片的分叉面与取流那次同类：**把上游文件里的算法搬空了**，不是插开关。
同步上游时 `lib/grpc/reply.dart` 里涉及过滤判定的冲突**不要照上游解**——
上游改的是实现，我们这边已经没有实现了，要改的是 Rust 侧
（见 §4.1 末尾那三条判据与 ADR-011 补充十三）。

注意本片**只搬了规则**：`ReplyGrpc` 的五条 gRPC 取数、`reply/v1` 的
2 万行 pb 子树与 27 个直接吃 pb 的 UI 文件都还在，属于 #52 的续单。
在那之前 `lib/services/rust/rust_reply_filter.dart` 里有一段
`ReplyInfo` → 规则输入的投影，按 AGENTS.md §2 例外写明了随哪一片删除。

修订：2026-11-19（S2 续单：评论 gRPC 取数与 pb 子树，issue #176，为本次）
——按 §6 对当前分支实测：修改上游文件 **812 → 811**，其中实质改动
**380 → 398**、仅重命名 **432 → 413**，新增 **750 → 758**，删除
**302 → 305**。

**19 个文件从「仅重命名波及」转进「有实质改动」**，是本仓库单次最多的一
批：`lib/pages/{common/reply,video/reply,video/reply_reply,video/reply_new,
video/reply_search_item,main_reply,my_reply,common/dyn,setting/models}` 下
的评论相关文件、`lib/common/widgets/context_menu/reply_menu_helper.dart`
与 `lib/utils/config_archive.dart`。原因是上一片（#52）只搬了规则，这一片
把**取数、消息解码与实体形状**一起换掉了，吃 `ReplyInfo` 的每个文件都要
改。

`lib/grpc/reply.dart` 从「有实质改动」转进「删除」——与取流、收藏那几次
同一个形状：上游文件里的东西搬空之后整个删掉，而不是留一个空壳。另外
删掉 `lib/grpc/bilibili/main/community/reply/v1.pb{,enum}.dart`
（`v1.pbjson.dart` 被 git 判成重命名到 `tool/proto-generator/inputs/`，
因此不计入删除；它作为生成器的输入快照留着，见 ADR-012 决策四 4.3）。

同步上游时这一片的冲突判据与 #52 那条相同，范围扩大到整个评论区：
`lib/grpc/reply.dart` 已经不存在，上游对它的改动**不要照搬**；吃评论
实体的那 28 个 UI 文件里，涉及字段形状的冲突要改的是
`bili_domain::reply_grpc` 与 `bili_api::grpc_reply`，不是 Dart 侧
（§4.1 末尾那三条判据与 ADR-011 补充十三）。

一处**没有**扩大分叉面的地方：共享的 pb 子树
（`dagw/component/avatar`、`pagination`、`account/service`、`vas/garb`、
`app/dynamic/v2`）都还有别的 importer，逐个 `grep -rl` 确认后原样保留。

修订：2026-11-20（S2 续单：正文分词与线程化视图合成，issue #179，为本次）
——按 §6 对当前分支实测：修改上游文件 **811** 不变，其中实质改动 **398**、
仅重命名 **413** 也都不变；新增 **758 → 766**，删除 **305** 不变。

本次碰的四个上游文件（`lib/pages/video/reply/widgets/reply_item_grpc.dart`、
`lib/pages/video/reply_reply/{tree,controller}.dart`、
`lib/pages/setting/models/reply_settings.dart`）在 #176 那一轮**已经**转进
「有实质改动」，所以分类一个没动——这正是连着迁同一片的常态：第一刀把
文件从上游那一侧切下来，后面几刀只是在我们自己这一侧继续削。

新增的 8 个文件全在本 fork 自己的目录里（`rust/crates/bili-domain/` 的两个
模块与两份 golden、两份 fixture，`test/services/rust/` 的两份一次性对照），
不扩大与上游的接触面。

分叉面的**性质**变了一点，值得记一笔：前两片（#52 规则、#176 取数）搬走的
是「上游文件里本来就独立的一段」，这一片搬的是**嵌在 widget 里的算法**——
`_buildMessage` 原本与 `InlineSpan` 的拼装缠在一起，`ReplyTree.build` 原本
与折叠态缠在一起。同步上游时这两处的冲突判据因此更细：上游改的若是**画法**
（颜色、间距、用哪个 widget），照上游解；若是**判定**（哪一段算链接、占位
父评论挂在哪），要改的是 `bili_domain::reply_richtext` /
`bili_domain::reply_thread`，Dart 侧已经没有那段实现了。
修订：2026-11-21（五端发布包签名，#121，为本次）—— 实质改动
354 → **356**，仅重命名波及 480 → **478**，「修改」总数仍为 834；新增文件
713 → **715**（`docs/release/SIGNING.md`、`tool/verify-release-signatures.sh`），
删除文件不变。`.github/workflows/build.yml` 与 `ios.yml` 从「仅包名重命名」
转入有实质改动——前者给四个可复用工作流补 `secrets: inherit`，后者写下
「iOS 侧载没有仓库侧发布身份」这个结论。`linux_x64.yml` / `mac.yml` /
`win_x64.yml` 本来就在实质改动里，这次只是加签名链路，计数不变。

修订：2026-11-22（S12 消息中心与私信设置 #63，为本次）——**重算时发现
§2 那张表已经过时**：它停在 2026-11-18（#58 之后），此后合并的 S2 续单
（#176、#179）、#184、#190 等都没有回填。本次按 §6 的方法整表重算。

合并前的 HEAD（`ceabd1672`）实际是：修改 **811**（仅重命名波及 411 /
有实质改动 400），新增 **840**，删除 **305**——与表里写的 834 / 715 / 271
相差甚远，差值全部来自那几次漏填。

加上本次改动后：修改 811 → **800**，仅重命名波及 411 → **383**，
有实质改动 400 → **417**，新增 840 → **841**，删除 305 → **334**。
本次数字含**尚未提交**的工作树改动。

「修改」又少 11 个、「删除」多 29 个，是迁移的常态：`lib/models_new/msg/**`
与 `lib/models_new/{single_unread,msgfeed_unread}/` 共 29 个上游模型整个删掉，
从 M 转进了 D。`lib/http/msg.dart` **没有**跟着删——它还剩三条不属于消息域
的接口（两条 multipart 上传与一条动态删除），所以仍留在「有实质改动」里。

修订：2026-11-22（S12 gRPC 续单 #63，私信那一半，为本次）——
修改 800 → **799**，仅重命名波及 383 → **371**，有实质改动 417 → **428**，
新增 841 → **875**，删除 334 → **344**。本次数字含**尚未提交**的工作树改动。

修订：2026-11-24（Android CI 签名 #146）——`build.yml` 早在 #121 就属于
「有实质改动」，`android/app/build.gradle.kts` 也早在 #184 属于该类，
本次继续改它们不会让修改类目增加；`ci.yml` 是本 fork 新增的文件。
新增 `tool/android-signing.py` 与签名解析回归测试；本轮按工作树重新统计，
新增文件 **916**，其余数量不变。实测发现 SDK 37 的 `apksigner` 使用按签名
方案命名的输出，校验脚本兼容新旧格式；两个构建入口均运行回归测试。
按 §6 的上游 tip 与当前工作树核对。

新增那 34 个里绝大多数不是手写代码：IM 的 proto 生成产物、三份描述符快照
（`tool/proto-generator/inputs/bilibili/{app/im,im}/**`——Dart 子树删掉之后
生成器要靠它们才跑得起来），以及 golden 的冻结快照。

删除的 10 个是 `lib/grpc/im.dart` 与 `lib/grpc/bilibili/{app/im,im}/**`
三棵 pb 子树，**16.6k 行生成的 Dart**。这是单次迁移删掉的最大一块，但它
在「删除文件数」上只体现为 10——按文件数看不出这一片的分量，按行数才看
得出来。

修订：2026-11-24（Player Port Flutter 适配 #4，已同步 main 的 #146 / #210）——
按 §6 对当前提交整表实测：修改 **787**，其中实质改动 **441**、仅重命名
**346**；新增 **967**、删除 **381**。旧表遗漏的后续切片与 spike 一并重算。
本轮修改播放器控制器、暂停按钮、视频详情、自动跳过和片段预览，位置/状态
消费者统一到 Rust 批准的快照；新增三个 Dart 平台适配模块、Rust 确认状态机、
FRB 导出、两份集成测试与一个运行脚本。同步上游时，平台 API 的变化映射到
media-kit 适配器，时序、错误与资源决策修改 Rust，不恢复旧的 Dart 算法。

修订：2026-11-24（#4 继续实跑）——同一上游 tip 对暂存工作树重算：
修改 **787**（实质 **441**、仅重命名 **346**），新增 **979**、删除 **381**。
新增 12 个文件用于基线、原生契约对照和独立移动/桌面验收宿主；未增加产品
上游文件的分叉面。原生 spike 的 lockfile 同步 main 已引入的 reqwest 功能依赖，
没有升级现有包版本。
修订：2026-11-25（应用图标 #208 与启动图 #14 为本次）——按 §6 的方法对
**main 当前状态**整表重算：修改 **851**（实质 **509**、仅重命名 **342**），
新增 **1009**、删除 **369**。

与上一条比，删除从 381 降到 369 **不是把文件找回来了**——上一条算的是
#4 分支的暂存工作树，里面含尚未合并的删除；这一条只算 main。两者基准不同，
不要当成同一条曲线上的相邻两点。

本轮分叉面多出一个**此前没有的类别：平台打包资源**。在此之前这份文档几乎
全是 Dart 代码与 Rust 新增，图标和启动图一直与上游逐字节相同；#208 换掉五端
启动器图标、#14 续上 `logo_2` 与启动图之后，**62 个上游资源文件**转入「修改」，
另有 3 个新增（`assets/app_icon/`）。清单见 §4.5。这类文件的同步策略和代码
不同：上游再改图标时不要手工合并，重跑生成器即可。

修订：2026-11-25（Android 打开 Impeller #230 为本次）——按 §6 的方法对
分支重算：修改 **851** 不变（实质 **509 → 510**、仅重命名 **342 → 341**），
新增 **1009**、删除 **369** 均不变。只动了一个文件
`android/app/src/main/AndroidManifest.xml`，它从「仅因包名重命名而改动」
转入「有实质改动」——这是**第一个因渲染后端而进入分叉面的 Android 构建
配置文件**，此前这个文件与上游只差包名。同步注意点见 §4.3 对应行。

修订：2026-11-25（登录页整页胶囊化 #236 为本次）——按 §6 重算：
**851 / 341 / 510，新增 1009、删除 369，一个数都没变**。记一笔是为了留下
「查过了」这个事实：`lib/common/widgets/capsule_tab.dart` 删除、
`lib/common/widgets/capsule_segmented_tab_bar.dart` 新增，一进一出抵消；
`lib/pages/login/view.dart` 与 `lib/pages/login/sms_code_field.dart` 分别早在
#140 和 #162 就已在「有实质改动」与「新增」桶里，再改不改变归类。

修订：2026-11-26（#242 预回归的三个 P1：#255、#256、#257 为本次）——
`lib/pages/search_panel/view.dart` 此前只被重命名提交 `ba430476b` 触及，本次转为
实质改动：仅重命名 **340 → 339**、实质改动 **513 → 514**；修改总数、新增与删除不变。
同时改到的 `http_error.dart`（#106）、`search_panel/controller.dart`（S9）、
`reply_reply/{controller,view}.dart`（#176）早已在实质改动桶里，不改变归类。

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
| 修改的上游既有文件 | **853** |
| &nbsp;&nbsp;其中：仅因包名重命名而改动 | **339** |
| &nbsp;&nbsp;其中：有实质改动 | **514** |
| 新增文件 | **1042** |
| 删除文件 | **369** |

其后 #255–#257 让 `lib/pages/search_panel/view.dart` 转入实质改动（339 / 514），见文首
2026-11-26 那条修订。

数字为 2026-11-26 的重算值（#250 应用图标分支的工作树，按 §6 整表重算）。比 #242 那次的
852 / 340 / 512 / 1019 / 369：`android/app/src/main/res/values-v31/styles.xml` 因 Android 12+
启动画面单独配图第一次与上游不同（修改 +1、实质 +1）；新增 +23，其中 22 个来自 #250（各密度
`android12splash.png` 10 个、插件补齐的各密度前景/单色图 9 个、`assets/app_icon/` 的透明源与
启动图源、`tool/app-icon-generate.py`），另 1 个是 #242 那次少算的——按同一方法在 `213084d8c`
上实测已是 1020。

#242 那次的说明（852 / 340 / 512 / 1019 / 369）：比 11-25 的 851 / 341 / 510 / 1009 / 369：
#15 的经典粉色让 `theme_color_type.dart` 第一次与上游不同（修改 +1、实质 +1）；#242 让
`lib/pages/about/view.dart` 从「仅重命名」转入「实质改动」；新增多出的 10 个来自 #239–#241
的 spike 与工具（含 `spikes/slint-mpv/avsync.py`）。

「修改」总数一路下降（935 → 909 → 897 → 891 → 887），部分文件是因迁移被
删除，从 M 转到了 D。#108 的界面改动一度把它顶回 892；合入 #111 的关注/
粉丝切片后是 890，#140 的登录修复使其回到 891，S14 登录切片再删掉四个后
是 887；S11b 后为 882，S6/S7 后为 824；#168 cherry-pick 上游补丁更新后为 834
（那 10 个是上游自己的内容，见顶部 2026-11-18 修订条）。

**这串数字在 2026-11-18 到 2026-11-22 之间断了一次**：S2 续单（#176、#179）、
#184、#190 合并时都没有回填，于是表里停在 834 / 715 / 271，而实际已经走到
811 / 840 / 305。S12（#63）重算时才发现，按 §6 的方法整表重做，现为
800 / 841 / 334，S12 的 gRPC 一半之后是 799 / 875 / 344。断过这一次说明「每次改动同步更新文档」这条约定在合并
密集时最容易漏——漏的不是结论，是**数字**，而数字恰恰是这份文档唯一的内容。

2026-11-23 再次按 §6 整表重算，现为 **787 / 908 / 365**（含 #166 与 S5 续单
第一批 #150 两个分支的工作树改动）。本轮「修改」又少了 12 个、「删除」多了
21 个：专栏信息、文集、话题顶部与搜索、推荐话题、部落气泡六条迁走之后，
`lib/models_new/bubble/`（11 个）、`article/article_info` 与 `article_list`
（6 个）、`dynamic/dyn_topic_top` 与 `dyn_topic_pub_search`（5 个）、
`dynamic/dyn_reaction`（2 个，全仓库零调用点）整目录删除。新增主要是 Rust
侧与 M0 的 spike 代码。

同日再记一次（ADR-015 / #193 的图片上传）：**787 / 915 / 366**。「修改」没
变（`lib/http/msg.dart` 还剩 `removeDynamic`，归 #150，所以它仍是 M 而不是
D），删除多一个（`models_new/upload_bfs/`），新增七个是 Rust 侧的 upload
模块、桥接、ADR-015 与两份测试。

新增来源主要是 Rust 侧的 crate 与测试；删除来源是各切片迁完后删掉的上游模型，
到 S12 为止删得最多的是 `lib/models_new/` 下的 live、pgc 与 msg 三块。

## 3. 包名重命名：分叉面的主体

`ba430476b chore: rename app package identifiers to KiliKili` 把每个 Dart
文件的 `package:PiliPlus/` 改成了 `package:kilikili/`，一次触及 992 个文件。
**当前分叉面里 346 个文件只有这一处差异。**

对同步的影响很具体：上游每改动一个 Dart 文件，我们这边大概率冲突，但
**冲突内容永远是那一行 import**。处理方式是取上游版本后重新套用重命名，
而不是逐个手工合并。值得为此准备一条命令，而不是每次人肉处理。

这也意味着：**冲突数量多不代表分叉深**。真正需要人判断的是下面 §4 的
那些文件，其中大部分又集中在 §4.1 的一类上。

## 4. 有实质改动的上游文件

513 个。业务层 typed 直连迁移集中列在 §4.1，竖版与横版封面卡片改版
见 §4.4，应用图标与启动图见 §4.5，其余改动逐个列在 §4.3。

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
| S2 评论过滤规则 | #52 / 本次 | `lib/grpc/reply.dart`（`needRemoveGrpc` 与三个私有判定搬进 `bili_domain::reply_filter`，只剩取数与应用 mask）、`lib/common/constants.dart`（删 `goodsUrlPrefix`）。**只搬规则**：五条 gRPC取数、`reply/v1` pb 子树与 27 个吃 pb 的 UI 文件留给续单；`lib/services/rust/rust_reply_filter.dart` 里的投影随那一片删除 |

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
| `pubspec.yaml` / `pubspec.lock` | `ee41df52c` `57485ede8` #242 | 引入 `flutter_rust_bridge` 与 `rust_builder`；版本名改为 KiliKili 自己的 `0.1.0`（同步上游时**不要**带回上游的 `version:`；`+N` 由 CI 按提交数改写） |
| `lib/pages/about/view.dart`、`lib/common/constants.dart` | #242 | 「关于」页加「基于 PiliPlus」与「开源许可证」两项（只增不改，GPL-3.0 要求注明来源） |
| `lib/main.dart` | `ee41df52c` | 启动时初始化 Rust Core |
| `lib/common/widgets/image/network_img_layer.dart` | #109 | 头像图片与占位图统一绘制主题色圆边框 |
| `lib/pages/mine/view.dart` | #109 | 未登录头像改走共用头像组件，保持边框一致 |
| `lib/common/widgets/floating_navigation_bar.dart`、`lib/pages/main/view.dart` | #122 | 三种首页底栏共用 64px 高度（#248）、图标与文字的可见间距各减半（#259：悬浮底栏在布局代理里让文字上移 5；MD3 底栏 `labelPadding` 改为仅底部 8 并把图标下移 4；经典底栏图标下移 3、整条上移 1.5，组合仍垂直居中）、玻璃背景、胶囊裁切、4px 内容内边距与边缘留白；安全区仅在胶囊外处理，分栏中心对齐选中背景，图标与文字组合居中，降低底色与高光的不透明度；保留导航与隐藏行为 |
| `lib/pages/main/controller.dart` | #128、#127 | 遵循「首页切换页面动画」设置：关闭时底栏与侧栏均直接切换，避免固定 240ms 转场及途经页初始化；开启时仍由 TabBarView 按方向滑动 240ms。同步上游时保留开关语义，不对 PageView 强制加动画 |
| `lib/scripts/double_tap_gesture.patch` | #143 | 给 Flutter SDK 的 `DoubleTapGestureRecognizer` 加一个 `doubleTapTimeout` 参数（默认仍为 `kDoubleTapTimeout`，只有播放器选择性调短）；**改了它就多一个只在打过补丁的 SDK 上才存在的符号**，见 AGENTS §5 |
| `lib/scripts/material/*.patch`（8 个）、`lib/common/widgets/flutter/dyn_tab_bar.dart` | #168 | cherry-pick 上游 `915276c87`，把补丁对齐到 `material_ui` 1.11.x；`tabs.patch` 新引入 `minimumIndicatorWidth`，**又一个只在打过补丁的包里存在的符号**。补丁与 `material_ui` 版本强绑定：1.10.0 上 `scaffold.patch` 打不上，必须连同 `pubspec.lock` 的版本一起动 |
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
| `.github/workflows/linux_x64.yml` | `d9b23ba4e` `45cfe832c` `6d35ee6e9` 及本次 | RPM 选包修复、产物体积闸门、依赖自动生成、GPG 签名与校验（#121） |
| `.github/workflows/mac.yml` | `6d35ee6e9` 及本次 | 产物体积闸门、Developer ID 签名与公证（#121） |
| `.github/workflows/win_x64.yml` | `6d35ee6e9` 及本次 | 产物体积闸门、signtool 代码签名（#121） |
| `.github/workflows/build.yml` | #121、#146 | 四个可复用工作流补 `secrets: inherit`；Android 验证包与正式包使用独立证书，缺密钥或指纹不符即失败，防止产出临时 debug 签名的 Release |
| `android/app/build.gradle.kts` | #184、#146 | NDK 版本固定；无签名配置的正式 Android release 在 `preReleaseBuild` 失败，普通 debug 开发不受影响；`.dev` 验证包允许本地 debug 回退 |
| `.github/workflows/ios.yml` | 本次（#121） | 注释写明侧载没有仓库侧发布身份；构建行为未变 |
| `lib/pages/login/view.dart` | #140、#162、#236 | 短信登录交互修复（#140）之后，界面整页改胶囊：顶部登录方式是一条**分段胶囊**（`CapsuleSegmentedTabBar`，四格等分、选中块滑动，窄屏丢图标保文字），五个输入栏是填充 + 全圆角（多行的 Cookie 用 24 圆角），主操作是整行 56 高胶囊。**上游若改这个文件，照常同步里面的逻辑与字段，但不要把布局与装饰同步回来**——那一层这边已经整个换掉，判据同 §4.4 |
| `android/app/src/main/AndroidManifest.xml` | 本次（#230） | `io.flutter.embedding.android.EnableImpeller` 由 `false` 改为 `true`。该 opt-out 是 2024 年底随上游带进来的，Flutter 3.47 已把它标为 deprecated；关着它时 `ImageFilter.isShaderFilterSupported` 为 false，玻璃导航栏的折射 shader 根本不会运行，Android 只能退到 `ImageFilter.blur`——这就是 iOS 透明、Android 像高斯模糊的全部原因。**上游若再改这一段，不要跟着关回去**；上游自己迟早会删掉这个 meta-data，届时整行一并删除即可，默认就是开 |
| `lib/common/widgets/loading_widget/http_error.dart` | #106、#255 | 错误态胶囊（#106）；屏高 < 580 时缩小插图与上下边距，避免文案与重试按钮落到首页悬浮底栏下面，常规手机保持上游尺寸 |
| `lib/pages/video/reply_reply/{controller,view}.dart` | #176、#256 | 线程化进度条只在拉取期间显示（新增 `fetchingTree`），上游让它常驻，未拉取时是一根来历不明的主题色条。**同步上游时保留这个开关** |
| `lib/pages/search_panel/{controller,view}.dart` | S9、#257 | 搜索面板加页脚：加载更多失败时显示原因并可点重试，到底时显示「没有更多了」。刻意没改 `CommonListController`——几十个列表共用，改它的上游冲突面太大 |

`lib/plugin/pl_player/controller.dart` 不在这张表里，它已经由 §4.1 的 S3 一行
覆盖；issue #89 又给它加了 seek 代际 `_seekEpoch`，这里不重复计数。那条代际
带来一个**跨文件的约束**：所有跳转都必须走 `seekTo`，绕过去的跳转不会自增
代际，弹幕会把它当成回跳而整段不再显示。同步上游时若引入新的
`videoPlayerController.seek(...)` 调用点，要一并改掉。

`.github/workflows/build.yml` 与 `ios.yml` 在 #121 之前只有包名重命名，
现已进入上表。

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

### 默认主题色（#15）

`lib/models/common/theme/theme_color_type.dart` 的粉色改为经典粉 `#FB7299`，
`lib/utils/storage_pref.dart` 的默认色板索引改为 1，动态取色默认关闭。
亮色、暗色与纯黑主题继续使用现有种子配色生成方式。

刻意保留色板顺序：索引 0 仍是原绿色，已保存的颜色、动态取色开关与自定义
ARGB 值继续生效；没有保存过相关选项的用户使用新的默认值。同步上游时保留
这两个默认值与经典粉色值，不要因上游调整色板而重排持久化索引。

按 §6 检查这两个文件：`storage_pref.dart` 已有 fork 实质改动，色板文件此前
与上游一致，本次新增一个实质改动文件，不增加文件总数。

### 4.5 应用图标与启动图（#208、#14、#250）

#250（2026-11-26）把各端图标从深蓝底改成白底，并给 Android 12+ 启动画面单独
配了透明底标记——此前那一段没配 `image`，系统拿带深蓝背景层的启动器图标画在
白底启动画面上。上游文件里因此多了 `values-v31/styles.xml` 一个（修改 +1）。

上游资源文件全部由生成器产出，**没有一处是手改的**：

| 位置 | 个数 | 产出方式 |
|---|---:|---|
| `android/app/src/main/res/`（mipmap、drawable、`values*/colors.xml`、`values*/styles.xml`） | 22 | `flutter_launcher_icons` + `flutter_native_splash` |
| `ios/Runner/Assets.xcassets/`（AppIcon 20 张、LaunchImage/LaunchBackground 8 张） | 28 | 同上 |
| `macos/Runner/Assets.xcassets/AppIcon.appiconset/` | 7 | `flutter_launcher_icons` |
| `windows/runner/resources/app_icon.ico` | 1 | `tool/app-icon-generate.py` |
| `assets/images/logo/`（`logo.png`、`desktop/logo_large.png`、`ico/app_icon.ico`） | 3 | `tool/app-icon-generate.py` |
| `assets/images/logo/logo_2.png` | 1 | 源图（单色遮罩，见下） |

**唯一的源是 `assets/app_icon/source_transparent.png`**（1024²，透明底粉色标记，
#250 从旧深色图标按「粉 × α + 深蓝 × (1-α)」反解出来，抗锯齿边缘保留）。
`assets/app_icon/{icon,macos,splash_android12}.png` 与上表里插件不管的 Windows
ico、托盘图、Linux 图标都由 `tool/app-icon-generate.py generate` 从它生成；
`android_foreground.png` 本来就是透明底，直接作为自适应图标前景。iOS 不接受带
alpha 的图标，所以 `icon.png` 必须是不透明白底。

**同步策略与代码相反：上游再动图标或启动图时不要合并它们的改动，直接取我们
这边的版本，或者改完源图依次重跑**：

```sh
python3 tool/app-icon-generate.py generate   # 需要 Pillow + numpy
dart run flutter_launcher_icons
dart run flutter_native_splash:create
```

两处容易踩的地方：

- `logo_2.png` **不是普通图片，是单色遮罩**。`up_panel.dart` 与
  `save_panel/view.dart` 都用 `Image.asset(..., color: ...)` 显示它，整张图会被
  染成一种颜色。换图必须保持透明底、单色、内部无填充，否则会糊成一个色块。
  它在 UP 面板里只有 38px，标记占画布的比例要按那个尺寸确认可读，不能只看
  512px 的原图。
- `flutter_native_splash:create` 每次都会把 `styles.xml` 里 `statusBarColor`
  挪位置并删掉一个空行。那是生成器的固定行为，**不要还原**——还原了下次再生成
  又会变回去，白白制造一轮 diff。

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

967 个，不与上游冲突，按区域分布：

| 区域 | 文件数 | 内容 |
| --- | ---: | --- |
| `rust/` | 581 | Rust workspace：7 个 crate、golden fixture 与快照 |
| `rust_builder/` | 46 | cargokit，把 Rust 接入 Flutter 各端构建 |
| `lib/services/` | 48 | typed 取数封装、会话寿命与传输适配 |
| `test/` | 52 | parity、单元与真实桥接集成测试 |
| `lib/rust/` | 40 | FRB 生成的 Dart 绑定 |
| `tool/` | 35 | 构建、生成、签名校验与运行时验证脚本 |
| `docs/` | 54 | RFC、ADR、M0 记录、QA 与实施计划 |
| `spikes/` | 76 | 原生播放器与平台集成实验台 |
| `shaders/` | 1 | Liquid Glass 背板折射着色器 |
| `lib/common/` | 4 | 共用视图与交互组件 |
| `lib/pages/` | 7 | 弹幕引擎、验证码、SC 展示和页面辅助 |
| `lib/plugin/` | 4 | 音量指示组件、Player Port 平台适配与观察端 |
| `lib/utils/` | 1 | 字幕格式转换 |
| 其他 | 18 | workflow、平台集成、发布资源与仓库配置 |

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
