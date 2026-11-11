# ADR-011: typed 直连的跨 FFI 约定，以及旧模型删掉之后拿什么做对照

Status: **Accepted（2026-11-11 仓库所有者确认，决策四选 A：迁移期不保留旧
实现。本文以 S13/SponsorBlock 的实现为具体依据）**
Date: 2026-11-11
Related: RFC-0001 §2.2 / §5.3、ADR-008、ADR-010、issue #48、issue #64、EPIC #67

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
