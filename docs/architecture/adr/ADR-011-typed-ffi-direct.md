# ADR-011: typed 直连的跨 FFI 约定，以及旧模型删掉之后拿什么做对照

Status: **Accepted（2026-11-11 仓库所有者确认，决策四选 A：迁移期不保留旧
实现。本文以 S13/SponsorBlock 的实现为具体依据）**
Date: 2026-11-11
Related: RFC-0001 §2.2 / §5.3、ADR-008、ADR-010、issue #48、issue #64、EPIC #67
落地证据：S13 #64、S9 #59、S3 #53、S1 #51、S15 #66、S10 #73 #83、
S11a #76 #86 及第三批（文末按切片追加补充）

## 背景

已迁移的七条路径（videoDetail、playurl、danmaku、dmView、subtitle、
searchSuggest、mediaSelect）都是同一种形态：Rust 解析出领域实体 →
序列化成 **JSON 字符串** → Dart 侧 denormalize 回接口原始形状 → 交给既有
的 Dart 模型。这形态有两个好处：能 shadow 比对（同一份响应两种实现各自
规范化，逐字段比），以及随时能把开关一关降级回 Dart。

代价是每条路径要写两遍解析规则（Rust 一遍、Dart 的 normalize 转写一遍），
而 Dart 模型层一个都删不掉。剩下二十余个接口、四百余个模型文件如果照这个
形态做，等于把一份注定要删的代码再写一遍。

仓库所有者已决定：**剩余业务层改为 typed 结构直连**，Dart 模型随切片删除，
真机验证与回归统一留到 Phase 2。本 ADR 把由此产生的约定定死，否则十五个
切片会各自发明一套。

## 决策一：结果用值表达失败，形状统一

跨 FFI 的每个导出返回

```rust
XxxResult {
    data: Option<T>,              // 成功时有值
    error_kind: Option<String>,   // network|remote|protocol|auth|storage|player|cancelled
    error_detail: Option<String>, // 可直接展示的文案，不含凭据、不含完整 URL
    error_code: Option<i64>,      // 协议自带的码
}
```

**不用 `Result`**：FRB 把 `Err` 映射成 Dart 异常，会逼每个调用点写 try/catch，
漏掉一处就把一条本可降级的路径变成线上崩溃。

`error_code` 是「协议自带的码」，不同接口含义不同：bilibili 接口是业务
`code`，SponsorBlock 是 HTTP 状态码。调用点确实需要它——SponsorBlock 的
404 意思是「这个视频没有片段」，必须静默，而其余失败要提示用户，这个分支
从 `error_kind` 推不出来。

## 决策二：用户可见文案放 Rust，但只放「对协议的理解」那部分

SponsorBlock 对每个状态码都有确切含义（400 参数错误、403 被自动审核拒绝、
409 重复提交……），这张表是协议知识，放 `bili-api`。

而「您提交了 N 个片段 / 为大家节省了 …」这类措辞、单位换算与排版属于界面，
留在 Dart（本次迁移把它从 `UserInfo.toString()` 挪进了设置页）。

判据：**换一套 UI 仍然成立的，是协议；跟着 UI 走的，是界面。**

## 决策三：取值开放的字段不要收成枚举

片段分类（`sponsor` / `selfpromo` / `poi_highlight` …）保持 `String`。
服务端随时会加分类，收成 Rust 枚举的话新分类会变成解析错误，整条列表拿
不到，用户看到的是「跳过功能坏了」。未知值交给 UI 自行忽略，失败范围限于
那一条记录。

同理，**单条记录残缺时丢掉这一条，而不是让整批失败**。基线在这里是抛异常
作废整次查询（`json["UUID"]` 赋给非空 `String`），迁移时刻意改掉了，并在
测试里钉住。

## 决策四：typed 直连没有降级开关，这是取舍不是疏漏

往返模式下每条路径配一个 `rust.*` 开关，失败回落 Dart 原实现。typed 直连
**删掉了 Dart 原实现**，降级无处可去。

后果写在明处：`RustCore` 未就绪（原生库没打进产物）时该功能整体不可用，
表现为「查不到数据」而不是崩溃——各 Dart 服务入口统一返回一个
`error_kind: storage` 的结果。

**2026-11-11 仓库所有者已确认选 A：迁移期不保留旧代码。** 当时考虑过的两个
备选：

- **A（采用）**：直接删 Dart 实现，接受无降级，靠 Phase 2 的统一回归兜底。
  代价是迁移期间一旦有 bug，用户侧没有兜底路径。
- **B（否决）**：迁移期保留 Dart 实现与开关，Phase 2 结束后再删。代价是每个
  切片多维护一份注定要删的代码，且两份实现会漂移。

对后续切片的要求：迁移一条路径时**连同它的 Dart 原实现一起删**，不加
`rust.*` 开关、不留 fallback 分支。唯一允许留下的是决策六那种共享模型的
单向适配，且必须写明随哪个切片删除。

## 决策五：旧模型删掉之后，拿什么做对照

往返模式下 golden fixture 的期望值由 Dart 基线产出，保证不是「Rust 的自我
验证」。typed 直连把这个来源删掉了。替代机制分两半：

1. **Rust 侧快照**：`tests/sponsor_block_golden.rs` 把解析结果冻结成
   `segments.rust.json`（`UPDATE_GOLDEN=1` 重新生成）。
2. **Dart 侧 parity 测试**：`test/services/rust/sponsor_block_parity_test.dart`
   用**还活着的**基线模型解析同一份 `segments.raw.json`，与那份快照逐字段比对。

单看任何一侧都只是「自己跟自己一致」，合起来才构成对照。

**parity 测试是有寿命的**：基线模型被删干净的那个 PR 应当连同它一起删掉。
长期留在仓库里的是**快照**——它是迁移当时行为的冻结证据，日后 Rust 侧改动
导致输出变化时，快照会先红。

重新生成快照之后**必须**再跑一遍 parity 测试，否则等于把 Rust 的改动直接
盖进「期望值」。

## 决策六：迁移的边界是网络边界

共享的运行时模型要等它的**所有**生产者迁完才能删。SponsorBlock 这次只迁了
四个只读接口，而 `SegmentItemModel` 还有三个生产者未迁（PGC 的
`clip_info_list`、下载条目缓存、SponsorBlock 自己的写接口），所以它删不掉，
`lib/services/rust/sponsor_block_compat.dart` 留了一个单向适配，并写明随写
接口迁移（ADR-013 / #50）一起删。

对其他切片：**先数清楚共享模型有几个生产者**。只迁其中一个的话，模型删不掉
是正常的，不要为此把别的切片拖进自己的 PR。

## 影响

- 新切片照 `bili-api/src/sponsor_block.rs` + `bridge_flutter/src/api/sponsor_block.rs`
  + `lib/services/rust/rust_sponsor_block.dart` 这三件套写。
- 决策四已定为 A，十五个切片一律不加降级开关；旧 Dart 实现随切片删除。
- 已迁的七条 JSON 往返路径**暂不改造**，留到 Phase 2 统一评估（EPIC #67）。

## S9（搜索 / issue #59）落地后的补充

本节是**证据与补充约束**，不改动上面六条决策。S9 是照本 ADR 写的第一个
跟进切片，三条发现对后续每个切片都有约束力。

### 补充一：FRB 的 codegen 不展开宏，导出项必须逐个写开

搜索有七个结果结构（字段全同，只有 `data` 的类型不同）和六个取数函数
（骨架逐字相同）。用 `macro_rules!` 收掉是自然的写法，但 **codegen 用 `syn`
解析源码、不做宏展开**：宏生成的 `pub struct` 与 `pub async fn` 它一个都
看不见，生成的绑定会**静默**少掉这些类型和函数——不报错，只是 Dart 侧
找不到。

因此跨 FFI 的导出项一律逐个写开，重复在这里是代价最低的那一种。
`bridge_flutter/src/api/search.rs` 里留了注释说明，照抄时别顺手"优化"掉。

### 补充二：parity 测试的寿命规则要分两种情形

决策五说「基线模型被删干净的那个 PR 应当连同 parity 测试一起删掉」。那条
成立的前提是**基线还活在 `lib/` 里**、parity 只是临时借用——SponsorBlock
正是这种情形（`SegmentItemModel` 还有三个生产者）。

搜索这边不同：被取代的 `fromJson` 随同一个 PR 从 `lib/` 删除，基线和 parity
在同一刻消失。照字面执行，这次迁移就没有任何外部对照了。

S9 的做法：把迁移前的 Dart 解析代码**逐字拷贝**进测试文件
（`test/services/rust/search_parity_test.dart` 的 `_Baseline`），不引用
`lib/` 的任何东西，写下之后不再改。它和 `*.rust.json` 快照一样是冻结证据，
两者由不同实现产出，任何一边行为变了都会红。

**给后续切片的判据**：基线模型在本 PR 之后仍活在 `lib/` 里 → 照决策五，
parity 测试随基线一起删；基线在本 PR 内就删干净 → 把基线拷贝进测试保留，
并在文件头写明来源与"不要改"。

### 补充三：parity 测试第一次跑就抓到一处真分叉

`Em.regTitle` 的空串守卫 `if (str != '')` **只写在 `onNonMatch` 里，
`onMatch` 没有**。于是 `a<em></em>b` 在基线下是三段，中间是一个**空的
命中段**。Rust 的初版两边都跳过了空段，少一段。

差异本身无害（画一个空 TextSpan 看不出来），但它正是 parity 要拦的那类
东西：看着像基线的疏漏，顺手"修好"就是行为漂移。已按基线改回，并在
`search_article.raw.json` 里加了带空 `<em></em>` 的样本，让冻结快照替未来
的重构记住这件事。

**结论**：parity 测试值得写，而且要覆盖**真实样本里不会出现的边界**——
未闭合标签、空内层、位数不符的 `&#x` 实体。这些正是手写解析器与原正则
最容易悄悄分叉的地方。

### 决策四在 S9 的具体代价

搜索没有降级路径：`RustCore` 未就绪时整个搜索功能不可用，表现为"搜不到
结果"。这比 SponsorBlock 的"跳过功能失效"面大得多——搜索是主路径。

决策四已定为 A，这里不是翻案，而是把代价记在明处：**选 A 的后果随切片
体量放大**。SponsorBlock 丢的是"跳过功能"，搜索丢的是主路径，到 S14
（登录）就是"登录不上"，且登录那条连"关开关回 Dart"的退路都不存在。

S14（issue #65）的风险说明里已经写了这一条；真正的兜底只剩 Phase 2 的
统一回归（#13 的端到端功能矩阵）。建议 S14 开工前先把那份矩阵里与登录
相关的用例补齐，而不是等整个 Phase 1 做完再补。

## S1（评论 REST / issue #51）与 S15（杂项 / issue #66）落地后的补充

同上一节：**证据与补充约束，不改动六条决策**。这两片合起来删掉了 38 个
Dart 模型文件，踩到的四件事对后续每个切片都有约束力。

### 补充四：实体的宽度由消费者决定，不由基线模型决定

决策六说迁移边界是网络边界，没说**映射到实体里的字段**要有多宽。S1 给出
的判据是：**认领切片后先数消费者，再决定实体宽度。**

评论 REST 的 20 个模型文件把服务端每一棵装饰子树都照抄了一遍（勋章、挂件、
大会员标签、等级、认证、折叠、UP 互动）。但这条路径在迁移前只有一个
消费者：`ReplyUtils` 的 shadow ban 自检，它只读 `replies[].rpid`——评论区
UI 走的是 gRPC（S2 #52），消费 protobuf 类型。照搬那些字段，等于把注定要
删的代码换种语言再写一遍。

S1 因此只映射：身份（rpid/oid/type/mid/root/parent/dialog）、计数与状态、
ctime、正文、发布者 mid/昵称/头像、游标与分页。**未映射的字段在 PR 描述
里逐条列出**，这是这条判据的配套义务——否则「少了字段」与「漏了字段」
在 review 时分不开。

要显示更多内容时再加字段：解析层加字段是向后兼容的，删字段才不是。

### 补充五：同一接口的两条请求形态，响应形状可能真的不同

根评论列表未登录走 `/x/v2/reply/main`，分页在 `cursor.pagination_reply`；
登录走 `/x/v2/reply`，分页在 `page`，**响应里没有 `cursor`**（实测确认，
固定在 `reply_golden.rs::the_two_request_shapes_paginate_differently`）。
基线用同一个模型装两者，另一半字段为 null。

迁移时两个字段都留 `Option`，并把「哪条路径有哪个」写进 golden。不要
「顺手统一成一个分页结构」——那是把两个接口的差异藏进一层抽象，下次
上游只改其中一条时会很难发现。

### 补充六：「刻意匿名」是请求语义的一部分

shadow ban 自检要的正是匿名视角：同一条评论，登录看得到而匿名看不到，才
说明被限制了。基线靠 `extra: {'account': NoAccount()}` 实现——拦截器
第一行就 return，于是连它平时补的 `referer` 都没有。

迁移时要照抄这个**结果**（`rust_reply.dart` 的 `_anonymousContext`），而
不是理解成「不传 cookie」。凡是基线里显式指定账号或显式不带账号的请求，
都要去 `account_mgr.dart` 看拦截器在那条分支上还做了什么。

与之配套：**业务码判断一律改用 `error_code`**，不要继续匹配 `errMsg`
文案。基线靠 `errMsg.startsWith('12022')` 认「评论不可见」，迁移后用
`error_code == 12022`，行为等价且不随服务端文案变化失效。

### 补充七：FRB 结构不可变，会以界面 bug 的形式出现

音乐页的「想听」原先把新值写回详情模型当本地缓存（`item.wishCount--`、
`item.wishListen = !hasLike`）。FRB 生成的结构**不可变**，这几行迁移后
编译不过。

修法不是找个可变容器，而是认清那份状态属于谁：它是本地状态，不是服务端
快照的一部分，应当存在 controller 上（S15 放了 `wishListen` / `wishCount`
两个 Rx，按钮外包 `Obx`）。**凡是基线里「把响应写回模型」的地方，迁移时
都要先回答这个归属问题。**

反向的一条：写操作仍留在 Dart（等 #50）时，让它**直接构造 FRB 的 Dart
类**即可（`danmakuFilterAdd` 现在构造 `FilterRule`）——那是普通数据类。
这样读写两条路径共用一个类型，不需要为写路径再留一个等价的 Dart 模型，
也就没有「两边各自漂移」的机会。

### 补充八：手工 fixture 在「原样透传」的路径上验不出嵌套层级

S9 迁搜索时，赛事卡片因为 `MatchContest` 还有未迁的生产者（S15 的赛事
详情）而被原样当 JSON 串带回 Dart。那份手工 fixture 把 contest 对象直接
放在了 `data[0]`，而真实响应是 `data[0].contest[]`——透传路径上这一层
不参与解析，所以 golden 与 parity 都绿。S15 把它改成 typed 时立刻暴露。

**判据**：某一块数据在本切片里是「原样透传」而非解析时，它的 fixture
证明不了结构，只证明了字节搬运。后续真正解析它的那一片，**要重新确认
真实响应的形状**，不能把前一片的 fixture 当作事实来源。

### 决策六在 S15 的一次正向应用

`MatchContest` 的两个生产者（赛事详情 `snake_case`、搜索卡片 `camelCase`）
在 S15 里一起迁完，于是 Dart 的 `MatchContest` 与中转文件
`lib/models/search/search_esports.dart` 当场删除，没有留下任何适配层。

对比 S1：`HotVideoItemModel`（S3 留下）与 `FollowData`（S15 让给 S6/S7）
都因为生产者分散在多片而必须留着。**认领切片前数一下共享模型的生产者，
能决定这一片是「删得干净」还是「又留一个要等别人删的适配层」。**

## S10（PGC / issue #60）与 S11a（直播 / issue #61）落地后的补充

两片各分两批，第二批都留下了一个单向适配
（`pgc_detail_compat.dart`、`live_playurl_compat.dart`）。补充九是 S11a
第二批踩出来的，对后续每个切片都有约束力。

### 补充九：冻结响应快照管不住请求，签名参数要单独钉

决策五定的对照机制（Rust 冻结快照 + Dart parity）**只覆盖响应**。请求
那一侧——路径、参数集、签名——没有任何测试经过，而它恰恰是出错后最难
诊断的一侧：响应解析错了会抛异常或字段对不上，**请求构造错了只会换回
一个业务错误码**，从现象反推不出是哪个参数写错了。

S11a 第二批的取流接口把这件事放大了。它的参数里有一串看不出含义、又
必须逐字正确的常量：

```
protocol=0,1  format=0,1,2  codec=0,1,2
ptype=8  dolby=5  panorama=1  web_location=444.8
```

前三个决定响应里 `stream → format → codec` 那棵树有多少枝，后三个声明
本端支持的播放器类型与特殊档位——少报一项，对应档位就不出现在响应里，
表现为「这个房间没有 4K」而不是报错。而 `web_location` 在基线里是
**double 字面量 `444.8`**，`encWbi` 取值走 `toString()`。把它规整成
`444`、`444.80`，或者把 `ptype` 的 `8` 写成字符串，`w_rid` 都会变成另
一个值。

**给后续切片的判据**：

1. 迁移带签名的接口时，**除响应快照外，再冻结一份请求面快照**（参数集
   + 签名结果），像 `bili-api/tests/live_golden.rs` 的
   `room_play_info_request_*` 那样；
2. 期望值仍按 ADR-008 决策三由两个独立实现分别产出，但**方向可以反过来**
   ——Rust 先冻结，Dart 侧用基线签名函数的逐字拷贝核对。取流这条就是后者，
   因为要钉的不只是签名算法（那早有 `wbi_golden.rs`），而是**参数表本身**；
3. 把基线的参数表拷进测试时**连类型一起拷**。只拷字符串形式等于把
   `444.8` 这类陷阱抄成了既成事实，测试会和实现一起错。

### 补充十：sealed 层级里的子类，迁不动的是整个层级

S11a 第二批要迁的 `LiveQuality` 是 `BaseQuality` 的子类，与
`VideoQuality` / `AudioQuality` 同属一个 sealed 层级（Dart 的 sealed
子类必须在同一文件，它们是 `part`）。即使本切片把 `LiveQuality` 的全部
**网络生产者**都迁完了，它仍然搬不动——另外两个子类的生产者（视频取流）
不在本片范围内。

这比补充八说的「共享模型有多个生产者」更紧一层：那里数的是**同一个类**
的生产者，这里要数的是**整个封闭层级**的。层级里只要还有一个子类没迁，
层级就得留着。

还有第二个理由，与生产者无关：`LiveQuality` 带着一张客户端静态表
（原画 10000、蓝光 400……），设置页的画质下拉和 `Pref.liveQuality` 的
默认值都在读它，这两处根本不经过网络。把类型换成 FRB 结构，等于让设置
页依赖一次取流。

于是分工落在适配层上：Rust 只认响应里的 `g_qn_desc`，命中不了就给
`desc: None`，由 `live_playurl_compat.dart` 查静态表补齐。**这条分界线
本身要被测试覆盖**——parity 测试比的是「Rust + 适配层」的整体输出与基线
`LiveQuality.fromFormat` 的结果，拆开比任何一半都证明不了迁移没改行为。

### 补充十一：先问「这条接口是不是真的只读」，别信名字

S11a 第三批清点 `lib/http/live.dart` 剩余只读时，`liveShieldUser` 差点
被当成「查询屏蔽用户列表」迁走。它实际是**写操作**：`type: 1` 添加屏蔽
用户、`type: 0` 移除，返回值只是新增那条的回显；请求是 POST、带 `csrf`
与 `csrf_token`。调用点（`live_dm_block/controller.dart` 的 `onAdd` /
`onRemove`）一看就清楚，函数名与返回类型都不清楚。

**判据**：认领只读切片时，对每条接口确认三件事再决定——HTTP 方法、是否
带 `csrf`、**调用点拿返回值做什么**。只看函数签名会把「写完回显」误判成
查询，而写操作归 ADR-013（#50），误迁进只读切片等于绕开那份还没落地的
幂等/重试约定。

反过来的一条：`getInfoByUser`（读屏蔽设置）确实是只读，于是读走了 Rust、
写留在 Dart。此时**让写操作直接构造 FRB 的 Dart 类**（见补充七），读写
共用 `LiveShieldUser` 一个类型，不必为写路径另留一个等价的 Dart 模型。

### 补充十二：共享实体的生产者，要连「非 REST 的那些」一起数

补充八说「认领切片前数一下共享模型的生产者」。S11a 第三批补上一句：
**生产者不一定都在 `lib/http/*.dart` 里**。

`SuperChatItem` 的两个生产者，一个是 REST 的 `superChatMsg`，另一个是
WebSocket 推送 —— `live_room/controller.dart` 的
`case 'SUPER_CHAT_MESSAGE'` 直接调 `SuperChatItem.fromJson`。只搜
`lib/http/` 会漏掉后者，把它当成单生产者迁走，结果是模型一行都删不掉、
还多一个适配层。同样地 `UinfoMedal` 的四个生产者里有两个（SuperChat、
弹幕消息）在长连接那条路径上。

**判据**：数生产者要搜 `XxxModel.fromJson` 的**全部**调用点，而不是
`lib/http/` 里的接口清单。长连接、本地缓存反序列化、测试夹具都算。

`SuperChatItem` 另有两个与生产者无关的障碍，也值得作为判据记下来：

- `expired` / `deleted` 是**可变的客户端状态**（`removeWhere((e) =>
  e.expired)`），而 FRB 结构不可变——这是补充七说过的那类问题；
- `fromJson` 里读 `GlobalData().showMedal`，**解析依赖全局设置**。这种
  模型即便生产者单一也不该直接迁：协议解析应当是纯函数，客户端偏好要
  留在调用点，否则同一份响应在不同设置下解析出不同结构，golden 快照会
  变得不可复现。
