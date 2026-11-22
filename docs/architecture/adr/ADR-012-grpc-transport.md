# ADR-012: Rust 侧 gRPC 传输：不要 tonic，要一个会解帧的 POST

Status: **Accepted（2026-11-13 仓库所有者批准；本文的实测数字取自 `35da9eedb`）**
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


## 首批落地（#133，2026-11-13）

`HttpPost` 的策略字段采用显式 `retry_policy: RetryPolicy`：只读 gRPC 为
`Read`，既有 REST 写构造为 `Write(Idempotency)`；仅有写幂等级别无法表达
只读 POST 的超时重试，这个差别已由真实 reqwest 桩测试锁定。重试判据本身不变。

`bili-grpc` 按根消息生成 metadata 与两个弹幕请求，不生成无消费者的响应树。
`tool/proto-generate.sh --check` 用临时目录重生成并逐文件比较；开发工具从
现有 pbjson 描述符及已删除 metadata 子树的机械快照取输入，正常构建无 protoc。
Dart protobuf 的 setter 会保留显式零值/空串，prost 默认 proto3 标量会省略它；
生成前给标量补 synthetic optional 来保留存在性，不改字段号、类型或 wire 格式。
匿名、空 access_key 和占位非空 access_key 的五个 metadata 头均与 Dart 编码对照。

`dmSegMobileRaw` 的传输及 `dmViewSubtitles` 的完整网络/解析边界由 Rust 接管，
`requestRaw`、字幕 JSON 桥接与开关/降级删除。字幕旧 Dart 类仍与视频详情 JSON
来源共用，只保留无算法的字段适配，随该来源迁移后删除。其他弹幕引擎开关及
下载用的 Dart 分段消息消费者不在本单范围。

一处修正原盘点：`metadata/device.pb.dart` 还有 generated relative import，
经 `dynamic/v2 → vas/garb → Device` 被动态 UI 间接消费，**不能**随 metadata
编码迁移删除。保留 Device 的三个 Dart 生成文件，待 S5/S6 最后消费者迁完；
其余 metadata 子树已删除。基线编码已不再引用这些残留类型。

离线解帧 parity 使用真实匿名响应的脱敏投影，来源与处理见
`rust/crates/bili-api/tests/fixtures/grpc/README.md`。网络桩与 FFI 证明完整链路
能返回字节/typed 字幕和错误值；登录态、代理/证书及真机回归仍留 Phase 2。

## 评论落地（S2 续单 #176，2026-11-19）

### 体积实测：决策四那条 +1 MiB 的线触发了一次回退

「影响」里写着「S2 落地时必须报一个实测 delta；单片若超过 +1 MiB，回到
『只生成用到的那些文件』重新评估」。它触发了。

本机 `aarch64-apple-darwin`、release profile（`lto = "thin"`、
`codegen-units = 1`、`strip = "debuginfo"`）对 `libbridge_flutter.dylib`
的实测：

| 配置 | 产物字节 | 相对基线 |
| --- | ---: | ---: |
| `e94f07e75`（基线） | 18 662 528 | — |
| 按根消息整棵生成（第一版） | 20 137 856 | **+1.41 MiB** |
| 只生成界面读的字段（现状） | 19 763 680 | **+1.05 MiB** |
| 基线 + 生成码（探针拉进链接，不含投影与导出面） | 18 740 368 | **+76.0 KiB** |

第四行是关键：**生成码本身只有 +76 KiB**，剩下的 +999 KiB 是投影代码与
FRB 的导出面（22 个实体结构的 sse/dco 编解码）。决策四那条线防的是
「生成码长成第二个 18 万行」，按这个判据现状是 +76 KiB，远在线下；而
FRB 导出面的体积是决策五的代价，它随 Phase 2 去掉 FRB 一并消失，不是
生成策略能动的。

第一版与现状的差 374 KiB 来自三处把**整棵别的树**拖进来的字段，它们
一个消费者都没有：`MemberV2` 的 `basic`/`official`/`vip`/…（通向
`dagw/component/avatar` 694 行、`vas/garb`、`account/service`）、
`Content.rich_text` 的 opus 分支（通向 `app/dynamic/v2` 5 500 行）、
以及 `MainListReply` 的活动/抽奖/问卷/运营位等十几棵子树。

### 决策四补充：按**字段**裁，不只按根消息裁

原文说「按切片生成，不一次性生成整棵树」，实现上是从根消息出发求依赖
闭包。实测表明只有根消息这一层粒度不够：一棵响应树上挂十几个没人读的
子树是常态。生成器因此多一张 `KEEP` 表
（`tool/proto-generator/src/main.rs`），每条消息列出**界面真的读的那些
字段**，其余字段在求闭包之前就裁掉。prost 对没生成的字段按未知字段跳
过，解码不受影响。

两条自检，都会让生成器直接失败：

- `KEEP` 点名的消息在上游消失 → 规则失效没人知道；
- `KEEP` 点名的字段在上游改名或删除 → 同上。

可达性也改成**按消息**算而不是按顶层消息算：嵌套消息原先跟着外层整棵
带进来，裁了 `MemberV2.basic` 而 `MemberV2.Basic` 还在，等于白裁。不可达
但仍有可达后代的外层消息留成空壳，因为 prost 把嵌套类型生成在外层的模块
里。

**代价**：要显示一个新字段时，除了写 UI 还要在 `KEEP` 里加一行。这比
「先把 18 万行都生成出来再说」麻烦，但那正是决策四不想要的东西。

### 落地范围

`MainList` / `DetailList` / `FoldList` / `DialogList` / `SearchItem` /
`TranslateReply` 六条全部迁入 `bili-api::grpc_reply`，请求面与基线逐字段
相同（没有协议变更，因此 `protocol-changelog.md` 不记）。`lib/grpc/reply.dart`
与 `lib/grpc/bilibili/main/community/reply/**`（20 864 行）同 PR 删除。

决策五点名的那批算法跟着下沉：整页被过滤光时自动翻下一页、简介复读的
剔除、表情图的 `webp → gif → 原图` 回退链、按埋点串前缀认笔记链接。过滤
规则（#52 已落地）不再由宿主逐页调用，`reply_filter_mask` 这个导出面随之
删除。

一处决策五没预见的东西：「我的评论」的本地存档与导入导出在基线里是
protobuf 生成类做的（`writeToBuffer` / `toProto3Json`），pb 子树一删就
没人实现，而它是**用户数据**。格式因此也下沉
（`bili-api::reply_grpc_store`），读两种写一种：老的 proto3 JSON 导出文件
与老盒子里的 protobuf 字节都照常读得进来。

**线程化评论视图（`reply_reply/tree.dart`）仍在 Dart**，连同它合成占位
父评论的那段规则。理由与退出条件写在那个文件里：合成的触发时机与折叠
状态是该视图自己的事，单独挪走只会让一个算法跨两边。它不在 #176 的范围
（那里只有取数那一半），需要自己的切片。

## IM 落地（S12 续单 #63，2026-11-22）

私信那 17 个方法迁完，`lib/grpc/im.dart` 与
`lib/grpc/bilibili/{app/im,im}/**`（三棵子树、16.6k 行生成的 Dart）整个
删除。决策一到决策七原样成立，补四条经验。

### 补充一：方法路径与消息 package 可以对不上，而且没人会告诉你

这一片两组服务：

| 服务路径 | 消息 package |
| --- | --- |
| `/bilibili.im.interface.v1.ImInterface/…`（interface，**单数**） | `bilibili.im.interfaces.v1`（**复数**） |
| `/bilibili.app.im.v1.im/…`（末段**小写** `im`） | `bilibili.app.im.v1` |

决策四已经写了「service 定义不在描述符里，路径照抄
`lib/grpc/url.dart`」。这一片是那条结论代价最高的一次：路径拼错的表现是
gRPC 状态 **12（UNIMPLEMENTED）**，读起来像「服务端下线了这个接口」，而
不像「我们把 interfaces 写成了 interface」。

落地形态：路径仍是 `bili-protocol::grpc` 的常量，外加
`test/bridge/im_bridge_test.dart` 里一张**十七条全列**的表逐条钉住。那张
表是那个文件最值钱的东西——它是唯一会在拼错时变红的东西。

### 补充二：裁剪不是省体积，是躲开别的树

决策四 4.1 说「按切片生成」。IM 的依赖闭包**一开始根本生成不出来**：
`im.type.RichTextMsgContent` 引着 `app/dynamic/v2` 的段落树，那棵树用了
`google.protobuf.Any`，而生成器的输入里没有 well-known types，直接报
`unknown type .google.protobuf.Any`。

顺着还会拖进 `account/service/v1`（名片的 `name_render`）与整棵
`dagw/component/avatar`（694 行）。裁完整片是 1034 行。

所以 `KEEP` 的用途要改写一句：它**首先**是「把别的树挡在外面」的工具，
省体积只是副作用。遇到 `unknown type` 时第一反应应该是「哪个字段把别的
子树引进来了」，而不是「生成器缺了输入」。

### 补充三：裁剪会留下孤儿文件，而 CI 的报错方向是反的

生成器只写自己这一轮的产物，**不删上一轮的**。裁掉一个字段把某棵子树整个
踢出闭包时（这次踢掉了 `account/service/v1` 与 `avatar/v1/plugin`），旧
文件留在原地、`mod.rs` 里已经没有它们，成了谁都不引用的孤儿。

CI 的 `tool/proto-generate.sh --check` 跑的是 `diff -ru`，确实会报
「Only in …」挡下来——但那条报错读起来像「生成器少生成了东西」，与真实
原因（生成器**多留**了东西）正好相反。

已改成生成前先清空输出目录。

### 补充四：删掉 Dart 子树之前，先把描述符快照留下

决策四 4.3 说描述符的真相源是仓库里的 `*.pbjson.dart`。那几个文件**就在
要删的子树里**：删完之后生成器再跑就少了输入，而 CI 只会说「少生成了
东西」。

既有做法是把它们抄进 `tool/proto-generator/inputs/`（注释里写着「已删除
Dart 子树从机械快照恢复」）。这次照做，但顺序值得写下来：**先抄快照、
跑一次 `--check` 确认生成器自洽，再删子树。** 反过来做的话，快照只能从
git 历史里捞，而那时你已经不确定该捞哪个版本了。
