# ADR-010: 弹幕引擎的跨 FFI 形状，以及正则不等价时怎么办

Status: **Accepted（接口已落地，等真机验证分桶时延与内存）**
Date: 2026-11-11
Related: RFC-0001 §2.2 / §4 / §5.2（P1-05）、ADR-003、issue #41、issue #42

## 背景

PR #41 把弹幕分段的 protobuf 解码迁到了 Rust（`bili-api`），但没有桥接、
没有调用方。P1-05 的第二步是把基线 `PlDanmakuController.handleDanmaku`
那套处理（标记自己 / 合并 / 过滤 / 分桶）也搬过去，于是必须先回答两个
问题：**弹幕怎么跨 FFI**，以及**用户的正则屏蔽规则两端不等价时怎么办**。

两个问题都不是「实现细节」：前者定错了，性能会比基线更差；后者处理错了，
用户会以为自己拉黑的弹幕被挡住了，而它照常飘过去。

## 决策一：Rust 持有分桶结果，Dart 按桶取

**不**把整段弹幕序列化后交给 Dart。

一个热门视频的单个分段有数千条弹幕。整段过 FFI 意味着一次性序列化几千
条记录、Dart 侧再反序列化一遍——这比基线直接用 protobuf.dart 解还慢，
而基线已经为此在响应超过 256 KiB 时把解析切到 isolate
（`GrpcReq._isolateSize`）。**迁移不能让热路径变慢**。

采用的形状是：

| 环节 | 归属 | 调用形态 |
| --- | --- | --- |
| 解帧、解压（gRPC 5 字节前缀 + gzip） | Dart | 传输外壳，随传输一起迁 |
| 解码、标记自己、合并、过滤、分桶 | Rust | `danmaku_push_segment`，**异步** |
| 按播放进度取一桶（100ms） | Rust 持有，Dart 取用 | `danmaku_bucket`，**同步** |
| 阈值（`weight`）过滤、渲染 | Dart | 不跨 FFI |

同步性是刻意区分的：

- **并入分段异步**。要解几千条 protobuf 并跑一遍引擎，放在 Dart 线程上
  就是一次掉帧。FRB 默认把非 `sync` 函数放到 Rust 工作线程，等于白拿了
  基线那个 isolate 的效果，还少一次消息拷贝。
- **取桶同步**（`#[frb(sync)]`）。渲染侧每 100ms 取一次，且基线的
  `getCurrentDanmaku` 是同步返回的。若它返回 Future，调用方要改成异步，
  弹幕会晚一帧出现。一次 map 查询加几条记录的拷贝，同步是合适的。

这正是 RFC §2.2 的「粗粒度接口与批处理，避免每帧 / 每条弹幕跨 FFI」：
跨 FFI 的次数由「每条弹幕一次」降到「每 100ms 一次，每次几条」。

### 只带渲染要用的字段

`DanmakuBucketItem` 不是 `DanmakuElem` 的全量镜像，只有基线
`lib/pages/danmaku/view.dart` 真正读的字段（`id`/`progress`/`mode`/
`fontsize`/`color`/`mid_hash`/`content`/`weight`/`like_count`/`colorful`
+ 引擎产物 `count`/`is_self`）。`action`、`extra`、`id_str`、`oid` 没有
消费方，拷过去只是在每 100ms 的热路径上多拷几个字符串。需要时再加，并在
那里写明谁要用。

### 会话所有权与失效

一个会话 = 一个 cid = 基线的一个 `PlDanmakuController`。会话 id 单调
递增且**不复用**，`dispose` 时关闭。在途的分段响应若在关闭之后才回来，
它持有的 id 查不到会话，于是被丢弃——这正是 ADR-003 的 `generation` 要
达到的效果。

弹幕会话内部不再分代际：seek 不会让已解出的弹幕失效（它只是换个桶取），
会话内不存在「要作废在途请求」的情形。

## 决策二：正则不等价时，整条路径降级回 Dart

基线的屏蔽规则有三类：关键词（子串）、UID（集合）、正则。前两类精确
可移植，正则不是——基线用 Dart `RegExp`（ECMAScript 语法、UTF-16 码元
语义），Rust 用 `regex`（无回溯、Unicode 码点语义）。

差异分两种，危险程度完全不同：

1. **Rust 编不过**：lookaround、反向引用等。`regex` 会报错，不会蒙混。
2. **两端都编得过，但语义不同**：`\d` `\w` `\s` `\b` 及其大写（Dart 只
   认 ASCII，Rust 认 Unicode 全集，全角数字 `１` 命中 Rust 的 `\d`），
   以及 `{n,m}` 的计数单位（Dart 数 UTF-16 码元，Rust 数码点，一个 emoji
   在前者是 2、在后者是 1）。

第二种是静默的。**决策是两种一视同仁：拒绝，并让整条弹幕链路降级回
Dart。** 不接受「跳过这一条规则继续」——少过滤一条弹幕在界面上没有任何
提示，用户看到的是「我明明拉黑了，它还在飘」。

实现上多了一道**构造扫描**（`bili_danmaku::rule::divergent_construct`）：
编译前先扫出上述构造，命中即拒。扫描按转义配对前进，`\\d` 这类字面量
不会误判；宁可多拒——多拒只是回到基线，结果仍然正确。

为此给 `bili_domain::error::ErrorKind` 加了 `Unsupported`：它与 `Protocol`
的区别是责任方，那边是对面变了，这边是我们自己做不到，调用方唯一正确的
反应是整条降级。

### 代价

用户只要有一条带 `\d` 的规则，这个视频的弹幕就整条走 Dart。可以接受：
开关默认关闭，Rust 路径是**可选的加速**，不是功能本身。想放宽，得先拿出
两端等价的证据（例如把 ASCII 类改写成显式字符类并验证），而不是先放行
再观察。

## 等价性怎么钉住

沿用 PR #41 的 fixture 链，但钉的是另一件事（那条钉「字节里有什么」，
这条钉「拿到弹幕之后怎么处理」）：

```text
<name>.case.json      人写的样本：引擎参数 + 一个分段的弹幕
  ↓ 基线的 protobuf 编码器
<name>.raw.pb         原始字节
  ↓ 基线解码 + 基线 bucketDanmaku + danmaku_normalize.dart
<name>.buckets.json   期望值，Rust 引擎输出须与之逐字段相同
```

两端都是基线，中间换成 Rust。`options` 刻意不翻译成中间表示，由两边各自
解读（Dart 构造 `RuleFilter`，Rust 构造 `FilterRules`）——规则的解读本身
就是要对齐的东西。

为此把基线的 `handleDanmaku` 原样搬进 `lib/pages/danmaku/engine.dart` 的
`bucketDanmaku`：fixture 生成器是纯 dart 脚本，构造不了
`PlPlayerController`，逻辑留在控制器里就只能手写期望值，而手写的期望值
证明不了任何事。

测试分布：

- `rust/crates/bili-danmaku/src/*` 的单测：为什么是这个行为（顺序、
  isSelf 豁免、桶边界、正则拒绝）
- `rust/crates/bili-danmaku/tests/engine_golden.rs`：Rust 确实是这个行为
- `test/services/rust/danmaku_engine_test.dart`：golden 确实是基线的行为

## 后果

- 弹幕的**显示数据来源**第一次可以是 Rust。此前的迁移（详情、取流、
  搜索建议）都是一次性解析，这条路径持有状态（一个视频的全部弹幕），
  因此降级路径必须能在运行中触发，而不只是在开关处。
- 失败模式是「这几分钟没人发弹幕」，没有任何报错。所以：任何一步失败
  （解码失败、会话丢失、桥接异常）都整条关闭会话并清空请求记录，让分段
  按 Dart 路径重新取一遍；宁可多几次请求。
- `bili-danmaku` 这个 crate 从此存在，但只装引擎，不装解码（解码在
  `bili-api`，理由见 RFC §4 的落地偏离说明）。

## 回退

关掉 `rust.engine.danmaku`（默认即关）。会话建不起来或运行中失败时，
代码自己回退，不需要发版。

## 待验证（真机）

1. 长视频连续播放：分桶时延（取桶是同步调用，必须远小于一帧）与内存
   （一个视频的全部弹幕常驻 Rust 侧，需与基线的 `_dmSegMap` 对比）。
2. 切 P / 切视频 / 退出页面后 `danmaku_open_session_count()` 归零——
   泄漏一个会话等于泄漏一个视频的全部弹幕，界面上没有任何症状。
3. 弹幕密集的视频上，与基线逐秒对照显示条数与合并角标。
