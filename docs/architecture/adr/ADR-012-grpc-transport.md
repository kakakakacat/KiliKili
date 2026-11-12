# ADR-012: Rust 侧 gRPC 传输：不要 tonic，要一个会解帧的 POST

Status: **Proposed（待仓库所有者批准；本文的实测数字取自 `35da9eedb`）**
Date: 2026-11-12
Related: RFC-0001 §2.2 / §5.3 / §16.6、ADR-008、ADR-010、ADR-011（决策四、
决策六、补充四）、ADR-013（决策一、决策三、决策七）、issue #49、EPIC #67
挡着：S2（#52）、S5（#55）、S6（#56）、S12（#63）；S11b（#62）的情况见
「影响」最后一条——它被记成挡在本 ADR 上，但实测不是。

## 背景

评论、动态、用户空间、IM、音频、弹幕的主路径走的是 gRPC。实测
（`35da9eedb`）：

| 项 | 数 | 怎么数的 |
| --- | ---: | --- |
| `lib/grpc/*.dart` 手写文件 | 9 个 / 1154 行 | `wc -l lib/grpc/*.dart` |
| 方法路径 | 37 条 | `url.dart` 46 个常量减去 9 个前缀常量 |
| 生成产物 `lib/grpc/bilibili/**` | 99 个文件 / 8.8 MB | `find` + `du -sh` |
| `*.pb.dart` 行数 | 181 969 行 | `cat $(find …) \| wc -l` |
| message 类 | 1540 个不重名（1697 处声明） | `class X extends $pb.GeneratedMessage` |
| enum 类 | 261 个 | `extends $pb.ProtobufEnum` |
| 直接吃生成类型的 UI 文件 | 77 个 | `grep -rln grpc/bilibili lib \| grep -v ^lib/grpc/` |
| 这 77 个文件里的 import | 84 条，仅 7 条带 `show` | 其余 77 条整模块拉进来 |

这些数字是本 ADR 全部决策的来源，请先看一眼：**传输那一层很薄，消息那
一层很厚**。前者 1154 行手写代码，后者 18 万行生成代码。选型的代价几乎
全部落在后者上，而 issue #49 的问题 1（tonic vs 手搓）问的是前者。

还有一条已经跨在半路上的路径：`DmGrpc.dmSegMobileRaw` 与
`dmViewSubtitles` 现在是**Dart 发请求、解帧、解压，Rust 解析**
（`lib/grpc/grpc_req.dart` 的 `requestRaw`，ADR-010 落地时留下的）。
`grpc_req.dart` 的注释自己写着「等 Rust 接管传输时，这一步一并过去」
——本 ADR 就是那件事。

### 先纠正一个前提：这条链路现在不走 HTTP/2

issue #49 的约束里写着「gRPC 必须 HTTP/2，与 `http1_only` 默认真直接
冲突，这一条必须在 ADR 里明确处理」。**这个前提对 bili 的网关不成立**，
三条证据：

1. `Pref.enableHttp2` 的默认值是 `false`
   （`lib/utils/storage_pref.dart:761`）。默认配置下 `Request().post`
   用的是 `IOHttpClientAdapter`，即 **HTTP/1.1**；`GrpcReq` 走的就是
   这个 `Request()`。
2. `GrpcReq._post` 从**响应头**读 `Grpc-Status` 与
   `Grpc-Status-Details-Bin`，不读 trailer；`GrpcHeaders._base` 里
   `te: trailers` 被注释掉并标着 `// dio not supported`。
3. 如果网关只在 trailer 里给状态，基线每一个 gRPC 请求都会因为
   `response.headers.value('Grpc-Status') != '0'` 判成失败——而这是
   已发布版本的默认路径。

结论：**bili 的 app 网关接受「普通 HTTP POST + application/grpc 帧」，
响应是一个完整 buffer，状态在响应头里**。它不是一个严格的 gRPC 服务端，
我们也就不需要一个严格的 gRPC 客户端。`http1_only` 与 gRPC 之间没有
冲突可处理——冲突来自「gRPC 一定要 HTTP/2」这个一般性常识，而这条链路
不满足它。

### 体积实测

本机（`x86_64-pc-windows-gnu`，release profile：`lto = "thin"`、
`codegen-units = 1`、`strip = "debuginfo"`）对 `bridge_flutter.dll` 的
实测，每一档都带一个 `#[unsafe(no_mangle)] extern "C"` 的探针函数把代码
拉进链接（否则 LTO 会整段删掉，量出来是假的）：

| 配置 | 产物字节 | 相对基线 |
| --- | ---: | ---: |
| 当前 main（基线） | 17 958 509 | — |
| `+ prost`（运行时 + 两条手写 message） | 17 986 525 | **+27.4 KiB** |
| `+ tonic`（默认特性）`+ tonic-prost` | 19 322 418 | **+1.30 MiB** |
| `+ tonic`（仅 client：`channel`/`codegen`/`tls-ring`/`tls-webpki-roots`）`+ tonic-prost` | 19 394 177 | **+1.37 MiB** |

两点要解释，否则这张表会被读反：

- **「仅 client」反而更大**，因为默认特性那一档没开 TLS，根本发不了
  https；一开 TLS 就多链一套 `tokio-rustls` 通道。也就是说能用的那档
  是 +1.37 MiB，不是 +1.30 MiB。
- **HTTP/2 本身不要钱**。`h2`、`hyper`、`bytes`、`flate2` 已经在
  `Cargo.lock` 里（reqwest 的 `http2`/`gzip` 特性带进来的）。tonic 的
  +1.37 MiB 买的是 tower 中间件栈、它自己的连接池与 codec 层，不是
  HTTP/2 能力。

×5 个平台产物，且 Android 还要按 ABI 再乘一次。

## 决策一：不引入 tonic。gRPC 调用 = `HttpTransport` 上的一次 POST 加一层帧

在 `bili-api` 里加一个 `grpc` 模块，做四件事：打帧、解帧、gzip、把
`Grpc-Status` 映射成 `DomainError`。加起来是「`protobuf.rs` 那个量级」
的代码，不是一套传输栈。

理由按重要性排：

1. **tonic 要的东西这条链路给不了，也不需要。** tonic 的 `Channel` 只
   走 HTTP/2，并且按 gRPC 规范从 **trailer** 读 `grpc-status`。而网关
   把状态放在响应头里（见背景）。
2. **它不是「稳妥的标准选择」，而是第二套传输栈。** `TransportConfig`
   的四项（重试次数/间隔、系统代理、证书豁免、`http1_only`）现在由
   `transport_reqwest.rs` 实现，值来自 Dart 的 `Pref`。tonic 自带连接
   池、自带 TLS、自带超时，这四项要在它上面**再实现一遍**；两套实现
   在同一个设置开关下行为不一致，正是 issue #25 刚解决掉的那类问题。
3. **体积**：+1.37 MiB × 五端，换来的是一个我们用不上的规范实现。
4. **reqwest 读不到 trailer**（`reqwest-0.12` 源码里 `trailer` 零命中），
   所以「先用 reqwest，哪天网关改回 trailer 再说」这条退路，退的时候
   是换传输栈而不是改几行——这一点必须现在就知道，见下面的代价。

**代价，写下来**：我们依赖网关的非标行为。缓解措施不是加开关，而是让
它**失败得认得出来**：解帧器拿不到 `Grpc-Status` 响应头时返回
`ErrorKind::Protocol`，`error_detail` 固定写「没有 Grpc-Status 响应头，
网关可能改用 trailer，见 ADR-012 决策一」。不要把「头缺失」当成空响应
或网络错误——那会表现成「某些视频没有评论」，而真实原因是传输假设
失效，两者在日志里长得完全不一样。

## 决策二：传输 Port 加一条字节形态，不要把现有的 `HttpResponse` 改成字节

现状有两处对不上：`HttpResponse` 的 `body` 是 `String`，而且**丢掉了
响应头**。gRPC 两样都要（protobuf 是二进制，状态在头里）。

加法而非改法：

```rust
pub enum PostBody {
    Form(Vec<(String, String)>),
    Json(String),
    /// 原始字节 + 显式 content-type。gRPC 用 `application/grpc`。
    Bytes { content_type: String, data: Vec<u8> },
}

pub struct HttpBytesResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

// HttpTransport 多一个方法，语义与 post 相同，只是不把 body 当文本。
fn post_bytes(&self, request: HttpPost)
    -> impl Future<Output = Result<HttpBytesResponse, DomainError>> + Send;
```

**为什么不把 `HttpResponse.body` 统一成 `Vec<u8>`**：已迁的全部 REST
路径都按文本解析（`serde_json::from_str`），统一要动每一个切片，收益
是零；而且 `resp.text()` 处理 charset，换成字节就得自己做。ADR-011
决策六那条「迁移的边界是网络边界」管的是业务边界，不是「顺手重构传输
层」的许可。

**响应头原样带回，不在传输层筛**：传输层不懂哪个头有业务含义，筛选
放在 `grpc` 模块。日志仍走 `bili_domain::redact::is_credential_key`，
与请求头同一套规则。

**重试规则不另起一套**：`post_bytes` 照样按 `HttpPost::idempotency` 走
`should_retry`（ADR-013 决策一那张表），只读的 gRPC 方法等价于
`RetryPolicy::Read`。gRPC 不是一类新的请求，它只是 POST 的一种 body。

## 决策三：帧、压缩与状态码照抄基线的具体数值

照抄的清单（来源 `lib/grpc/grpc_req.dart` 与
`lib/utils/accounts/grpc_headers.dart`）：

- **帧头 5 字节**：第 0 字节是压缩标志，第 1–4 字节是大端 `int32` 的
  **压缩后**长度。
- **请求体超过 64 字节才 gzip**（`_gzipMinLength`），而
  `grpc-encoding: gzip` 这个头**恒发**。标志位是逐帧的，所以两者并不
  矛盾；照抄就行，不要「顺手改成按实际压缩与否发头」。
- **`gzip-accept-encoding: gzip,identity`**——注意规范里这个头叫
  `grpc-accept-encoding`。基线写的是 `gzip-`，而它在线上是工作的。
  **照抄**：改请求面就是改协议实现，要走 ADR-008（`PROTOCOL_VERSION`
  +1、`protocol-changelog.md` 记一行），不能夹在传输迁移里顺手试。
- **解帧按标志位判 gzip**，不看 `content-encoding`——HTTP 层那一层
  reqwest 已经解掉了，帧内的 gzip 是另一件事。
- **长度字段必须校验**：声明长度 > 剩余字节时返回 `Protocol`。基线
  `decompressProtobuf` 直接 `sublistView` 越界抛异常，Rust 这边越界
  切片是 panic，而 panic 会带走宿主 App。
- **读完第一帧还有剩字节 → `Protocol` 错误**。基线只按第一帧的长度取
  一段、多出来的静默丢掉；这里刻意不跟：37 条路径全是 unary，剩字节
  说明我们读错了位置，静默丢掉只会把错误推到更远的地方（与
  `protobuf.rs` 对 group 的处理同一条理由）。

**状态码映射要保住 code**。基线 `_post` 的失败分支做了两层解码：
`Grpc-Status-Details-Bin` 是 base64（**无填充**，基线自己补 `=`）的
`google.rpc.Status`，它的 `details[i]` 里再套一层 `Status`，UI 真正用
的 `code`（例：`-400`）在 `details[0].code`。映射成
`DomainError::with_code`，与 REST 的 `envelope::RemoteFailure` 一致——
ADR-011 的 `XxxResult.error_code` 是同一个字段，调用点不该因为底下走的
是 gRPC 就少一个 code。

## 决策四：消息用 prost 生成；proto 的真相源是仓库里已有的描述符；生成产物签入仓库

`protobuf.rs` 的模块文档写着「等需要解码的消息多到手写不划算（如
viewunite、dynamic 那几棵树），再引入 prost 并为构建期依赖单独做决策」。
**这就是那个决策**，三件事：

### 4.1 手写到此为止

手写读取器在弹幕的两条消息上成立（约 100 行，且能把「与
`protobuf.dart` 的语义对齐」写在注释里）。在 `reply/v1` 的 **97 条
message** 上不成立。已经手写的 `danmaku.rs` / `dm_view.rs` **不回头
改**：它们是仅有的两处把解码语义写下来、并被 parity 钉过的实现，重写
一遍换不到任何东西。新的树一律生成。

### 4.2 `protoc` 不进构建矩阵

生成产物**签入仓库**，与 `frb_generated.rs` / `frb_generated.dart` 同一
套做法：

- `tool/proto-generate.sh`（或一个小 Rust bin）只在协议变化时手动跑；
- CI 加一个 job：重跑生成器，`git diff --exit-code`。它跑在
  `ubuntu-latest`，**不进 `.github/workflows/rust.yml` 的 cross-build
  矩阵**——那五个 target 的环境里不需要任何额外二进制。

这样 `protobuf.rs` 当初拒绝 prost 的那条理由（「给每个构建环境再加一个
外部二进制依赖」）就不成立了：构建环境一个都不用改，只有改协议的那个
人要能跑生成器。

### 4.3 proto 从哪来：仓库里已经有描述符

issue #49 的问题 2 问「复用同一批 `.proto`（需要找到或还原它们），还是
手写需要的那几个消息」。**两者都不必**：`protoc-gen-dart` 已经把描述符
写进了仓库。33 个 `*.pbjson.dart`，每条 message 一个 base64 的
`google.protobuf.DescriptorProto`，每个 enum 一个 `EnumDescriptorProto`
（实测 `main/community/reply/v1.pbjson.dart`：97 + 9），字段号、label、
类型、`type_name`（全限定）、oneof、map entry（`options.map_entry`）
一应俱全。

生成器因此是**机械变换**，不是人工转录：

1. 从 `*.pbjson.dart` 取出所有 base64 描述符；
2. 按文件头注释（`// Generated from bilibili/....proto`）与全限定类型名
   拼出 `FileDescriptorProto`（package、文件名、依赖）；
3. 组成 `FileDescriptorSet` 交给 `prost-build` 的描述符入口
   （`compile_fds`，这条路径不需要 `protoc`）。退路是先吐 `.proto`
   文本再用 `protoc`——**那也只发生在生成器里**，不影响 4.2 的结论。

必须带的自检（生成器一发现就失败，不要容忍）：

- 出现 `label = 2`（proto2 的 `required`）→ 失败。整批协议按 proto3
  处理，这个假设要有人守着。
- 文件头注释里的 package 与消息全限定名的前缀对不上 → 失败。
- 引用了一个谁都没定义的 `type_name` → 失败（说明切片漏带了依赖文件）。

**按切片生成，不一次性生成整棵树**：18 万行 Dart 的对面不该是 18 万行
Rust，而我们一次只迁一条路径。

**service 定义不在描述符里**（Dart 侧没有 `.pbgrpc.dart`），所以方法
路径照抄 `lib/grpc/url.dart` 成一张常量表。它本来就是 37 个字符串。

## 决策五：FRB 的导出面上不出现 protobuf 生成类型

这是 ADR-011 决策六与补充四在 gRPC 上的必然推论，但因为代价很大，单独
写一条：

> Rust 侧 prost 解码 → **投影成 `bili-domain` 实体** → FRB 导出。
> 生成的 message 类型止步于 `bili-grpc` crate 内部，和 DTO 止步于
> `bili-api` 是同一条规矩。

如果反过来——把 pb 形状镜像成 FRB 结构——等于把一棵 1540 个节点的协议
树搬进宿主，并且让宿主代码继续按协议的形状写。那正是 ADR-011 决策六
（迁移的边界是网络边界）与补充四（实体宽度由消费者决定）要避免的事。
顺带，FRB 不支持 `Option<Option<T>>`，而 proto3 的 `optional` 嵌套
message 在 prost 里正是 `Option<Option<_>>` 的温床。

**代价，写清楚**（S2/S5/S6/S12 的估时按这个来，不要按已做完的只读切片）：

- 这几片不是「换个取数函数」。77 个 UI 文件直接吃 pb 对象，其中 70 条
  import 不带 `show`，整模块拿进去用；换成领域实体就是**连带重写这些
  文件**。
- 按 ADR-011 决策四，被取代的 `.pb.dart` 子树要在同一个 PR 里删掉。删
  之前用 `grep -rl` 确认没有别的 importer：`dagw/component/avatar`、
  `metadata/*`、`pagination` 这类是**共享树**，最后一个 importer 走了
  才能删。
- 评论那片（S2）里还藏着一批**算法**：`ReplyGrpc` 的
  `_needRemoveGoodGrpc`、`_blockAtOnlyReply`（逐 code unit 扫 `@`）、
  正则屏蔽。按 AGENTS.md 的铁律这些必须一并下沉 Rust，别只搬解码。

## 决策六：gRPC 的元数据在 Rust 构造，凭据按请求传入

issue #49 的问题 3。实测 `grpc_headers.dart`：7 个固定头 + 4 个每账号
头，其中 5 个是 **protobuf 编码后 base64**（`x-bili-metadata-bin`、
`x-bili-device-bin`、`x-bili-fawkes-req-bin`、`x-bili-locale-bin`、
`x-bili-network-bin`）。

按 AGENTS.md 的判据（「换掉宿主框架时这段要不要重写」）：这是「协议没
规定、但两端必须一致」的决定，**下沉 Rust**。形状照已有的先例
`bili-api::live::AppIdentity` / `feed_headers(id)`：

- `GrpcIdentity { access_key, buvid, trace_id, … }` 由 Dart 按请求注入
  （凭据不落 Rust，RFC §2.2）；
- `build = 2001100`、`mobi_app = android_hd`、`version_name = 2.0.1`、
  `channel = master`、`app_id = 5`、locale/network 那两组常量属于**协议
  常量**，进 `bili-protocol`；改动按 ADR-008 走（`PROTOCOL_VERSION` +1、
  `protocol-changelog.md` 记一行）。
- `session_id` 每次随机 8 字符、`trace_id` 是一个常量串：**照抄**，
  不要在迁移里「顺手修正」成更像真的东西。

**与 REST 的账号头共用多少**（问题 3 的后半句）：共用的是**值**，不是
编码。REST 的 app 路径把 `access_key` 放进参数里做 `AppSign` 签名；
gRPC 走 `authorization: identify_v1 <access_key>` 头。两边同源的只有
`access_key` / `buvid` / UA 这三个值的出处（Dart 账号体系）。

**落地时机**：`grpc_headers.dart` 是 `metadata/*` 那几棵 `.pb.dart` 的
最后一个 importer，所以这条随**第一片 gRPC 传输**落地（决策八），而不
是随某个业务切片。编码这 5 个头要用 prost 的 encode，正好与决策四同一
套生成产物。

## 决策七：gRPC 的写操作沿用 ADR-013，但 csrf 不适用，信封也不一样

issue #49 的问题 4。三条边界：

1. **`Idempotency` 照样进签名**（ADR-013 决策二），`post_bytes` 不给
   默认值。
2. **`RequestContext.csrf` 在 gRPC 路径上必须保持 `None`**。gRPC 没有
   `bili_jct`，鉴权完全在 `authorization` 头里。`client::send_write`
   那个统一注入点**不能**复用到这条路径上——多发一个 csrf 参数不会报
   错，只会悄悄多出一个签名面。
3. **成功/失败的判据不同**：REST 看信封里的 `code`，gRPC 看
   `Grpc-Status` 响应头（决策三）。ADR-013 补充三说的「写操作的信封没有
   data」在这里要再加一句：gRPC 的写操作**连信封都没有**，响应就是一条
   业务 message。

### 定级样例（13 条，落地时每条仍要按 ADR-013 决策三重新过）

| 方法 | 级别 | 理由 |
| --- | --- | --- |
| `ThumbUp` | `Idempotent` | 参数是 `action: ThumbType`，置为而非 toggle |
| `KeywordBlockingAdd` / `Delete` | `Idempotent` | 按关键词置为「在/不在」列表里 |
| `PinSession` / `UnPinSession` | `Idempotent` | 同上，且 `PinSession` 显式带 `top_time_micros` |
| `SetImSettings` | `Idempotent` | 整组设置覆盖写 |
| `ClearUnread` / `DeleteSessionList` | `Idempotent` | 置为「已读」/「已删」 |
| `CoinAdd` | `Verifiable` | `num` 是加一次；但本稿已投几枚查得到 |
| `TripleLike` | `Verifiable` | 复合操作，投币那半是加一次 |
| `SendMsg` | **`Unsafe`** | 重复的私信用户直接看得见，见下 |
| `SessionUpdate` | **不是写操作** | 见下 |

两条要单独说：

- **`SendMsg` 的 `dev_id` 是个陷阱**。基线每次 `const UuidV4().generate()`
  现生成一个，看起来就是个幂等键——但**每次都是新的**，所以重发一定
  产生新消息；而「复用同一个 dev_id 重发会不会被服务端去重」我们没有
  任何证据。按 ADR-013 决策三第三问（重复的后果用户直接看得见且无从
  判定）定 `Unsafe`，连 `NotSent` 都不重发。要改成「重试复用同一个
  dev_id」，先拿到服务端去重的证据，再单独改。
- **`SessionUpdate` 名字像写，实际是读**。唯一调用点是
  `lib/pages/whisper_link_setting/controller.dart:58` 的 `getIsPinned()`，
  拿 `response.session.isPinned` 显示开关状态。这正是 ADR-013 补充五
  （定级前先看调用点）与 ADR-011 补充十一（先问这条接口是不是真的
  只读）说的那种情况，而且它就埋在本片里。

**风控**：`x-bili-gaia-vtoken` 在基线里恒为空串，gRPC 侧没有
`v_voucher` 的样本。ADR-013 决策七（凭证原样带回宿主、Rust 不自行处理
验证流程）在这里先按「原样返回错误」执行，等真拿到样本再补——不要先
写一套没见过的分支。

## 决策八：第一片迁弹幕那条已经跨在半路的路径，只验传输

`DmGrpc.dmSegMobileRaw` 与 `dmViewSubtitles` 的解析**已经在 Rust**，
UI 吃的是自家的 `Subtitle` 模型，没有 pb 对象跨边界。所以把它们的传输
搬过去，改动面只有传输本身：

- 验的是：帧编解码、gzip 阈值、5 个 metadata 头、`Grpc-Status` 映射、
  `TransportConfig` 复用（代理/证书/重试/`http1_only` 照常生效）；
- 不验的是：消息生成、领域实体投影、UI 重写——那些留给业务切片；
- 顺带消掉现在这个「一段字节在两个实现里各走一半」的状态，并删掉
  `GrpcReq.requestRaw`。

**parity 怎么做**（ADR-011 决策五的形态）：对照物是**一份离线的原始帧
字节** fixture（取自真实响应并脱敏），断言「Dart 的 `decompressProtobuf`
与 Rust 解帧器对同一份字节给出相同输出」。**不要**用「发两次请求比字节」
——同一个分段两次请求的点赞数会变，而且那等于对同一账号重复请求
（`bili-api` 模块文档拒绝 shadow 的理由）。真机验证按 AGENTS.md 统一留到
Phase 2。

之后的顺序留给 EPIC #67 排，本 ADR 只给一条约束：**决策六（metadata）
必须跟第一片一起落地**，否则 Rust 发出去的 gRPC 请求没有设备头，服务端
的行为与基线不可比，后面每一片都建在一个没验过的地基上。

## 影响

- `bili-api` 多一个 `grpc` 模块（帧/状态/错误映射），传输 Port 多一个
  `post_bytes` 与 `PostBody::Bytes`。
- **新增 crate `bili-grpc`** 放生成码：生成码会长到几万行，塞进
  `bili-api` 会把它的增量编译一起拖慢，而 `bili-api` 是改得最勤的 crate。
- 依赖多两个：`prost`（+27.4 KiB，实测）与 `flate2`（**已在依赖树里**，
  reqwest 的 gzip 经 `async-compression` 带进来的，显式用它不新增编译
  单元）。生成码的体积按切片增长，**S2 落地时必须报一个实测 delta**；
  单片若超过 +1 MiB，回到「只生成用到的那些文件」重新评估，别让它长成
  第二个 18 万行。
- CI 多一个「生成码与描述符一致」的检查 job（ubuntu，不进 cross-build
  矩阵）。那次 PR 要同步改 `docs/m0/VALIDATION-CAPABILITY.md`。
- Dart 侧是**净减**：8.8 MB 生成产物按切片删除。
- 已迁的 REST 路径不受影响：`HttpResponse` 不动，`should_retry` 不动。
- **S11b（#62）并不挡在本 ADR 上**。实测 `lib/tcp/live.dart`：直播弹幕
  长连接是 WebSocket + 自定义 16 字节包头 + JSON 载荷 + zlib/brotli
  解压，**全程没有 protobuf，也没有 gRPC**。本 ADR 对它的帮助仅限于
  「消息生成工具链」，而它连 protobuf 都不用。#62 的真实依赖是长连接的
  生命周期与取消语义（RFC §2.2 的 `session_id`/`generation`），那需要
  它自己的决策。`rust/crates/bili-domain/src/live.rs:8`、
  `lib/http/live.dart:29` 等处记的「阻塞在 ADR-012」是记错了，本 ADR
  批准后应当在 #62 里更正，而不是默认它跟着解锁。

## 刻意没做的

- **不做 streaming**。37 条路径全是 unary，基线里一条流式调用都没有。
  真出现了再单独决策——届时「状态在响应头里」这个前提本来也得重估。
- **不做 shadow 比对**。ADR-011 决策四已经定了不留旧实现；gRPC 的
  shadow 还要对同一账号重复发请求，`bili-api` 的模块文档拒绝过一次。
- **不改 `gzip-accept-encoding` 这个看起来像拼错的头**（决策三）。
- **不自动跟随上游 proto 变化**。描述符随 PiliPlus 同步进来，重跑生成器
  是人决定的动作，机制走 ADR-008。
- **不实现完整的 `google.rpc.Status`**。只解基线实际用到的两层
  （`code` / `message` / `details[0]`），其余字节跳过。
- **不碰 `Pref.enableHttp2`**。用户开了它，Rust 这条路径照样按
  `TransportConfig.http1_only = false` 走 h2——网关两种都接受，这一点
  基线已经验证了十几个版本。
