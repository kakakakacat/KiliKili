# ADR-013: 写操作的幂等、CSRF 与重试策略

Status: **Accepted（2026-11-12 仓库所有者确认，八条决策全部批准。本文的实测
清单取自 `b67330393`）**
Date: 2026-11-12
Related: RFC-0001 §2.2 / §2.3 / §5.3 / §16.6、ADR-008、ADR-011（决策四、决策五、补充九、
补充十一）、issue #50、EPIC #67
挡着：S4、S7、S8、S12、S14，以及 #60 / #61 / #66 里剩下的写操作尾巴

## 背景

只读切片已经基本迁完，剩下的全是写操作。而 Rust 侧的
[`HttpTransport`](../../../rust/crates/bili-api/src/http.rs) **只有 `get`**，
模块文档写着「写操作要先有幂等与重试策略，不该顺手打开」。这份 ADR 就是
那个前提。

实测（`b67330393`，`grep -c 'Request().post' lib/http/*.dart`）：

| 文件 | 条数 | 文件 | 条数 |
| --- | ---: | --- | ---: |
| `fav.dart` | 22 | `member.dart` | 7 |
| `video.dart` | 15 | `reply.dart` | 5 |
| `msg.dart` | 13 | `pgc.dart` | 5 |
| `dynamics.dart` | 13 | `danmaku.dart` | 5 |
| `login.dart` | 11 | `sponsor_block.dart` | 4 |
| `user.dart` | 10 | `validate.dart` / `danmaku_block.dart` | 2 / 2 |
| `live.dart` | 9 | `music.dart` / `init.dart` / `follow.dart` | 1 / 1 / 1 |

**合计 126 条**，是整个迁移里最大的一块未动区域。

不先定规矩的后果很具体：五个切片会各自发明一套「失败了要不要重发」，而
这件事**错一次就是用户可见的重复内容或重复扣费**，不像只读路径那样失败了
重来一次就好。

### 现有的重试规则对写操作不成立

`transport_reqwest.rs` 的 `should_retry` 现在是：

```rust
fn should_retry(e: &reqwest::Error) -> bool {
    e.is_connect() || e.is_timeout()
}
```

对 GET 这是对的。对 POST，**后半句是错的**：

- `is_connect()` —— 连接没建立起来，请求**没发出去**，重发安全；
- `is_timeout()` —— 可能是连接超时，也可能是**请求已经送达、服务端已经
  处理、只是没在时限内把响应写回来**。对写操作重发它，等于按一次「再提交
  一遍」。

两者在 reqwest 里是分得开的：客户端设了 `connect_timeout`（见
`build_client`），连接阶段超时归 `is_connect()`，整体 `.timeout()` 超时归
`is_timeout()`。所以这不是「做不到」，是必须显式写下来的分界。

## 决策一：POST 进传输层，重试规则按幂等级别分两档

`HttpTransport` 扩出 `post`，形状与 `get` 对称：

```rust
pub enum PostBody {
    /// `application/x-www-form-urlencoded`。基线绝大多数写操作用这个
    /// （调用点显式写 `Headers.formUrlEncodedContentType`）。
    Form(Vec<(String, String)>),
    /// `application/json`。dio 不指定 contentType 时的默认值就是它，
    /// 基线有一批调用点正是靠这个默认值，**不能统一成 Form**。
    Json(String),
}

pub struct HttpPost {
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: PostBody,
    pub timeout: Duration,
    /// 这条写操作重发安全不安全。由接口的迁移者填，见决策二。
    pub idempotency: Idempotency,
}
```

重试规则**只有两档**：

| 幂等级别 | 传输层行为 |
| --- | --- |
| `Idempotent` | 与 GET 同：只在 `is_connect()` 时重试，次数沿用 `retry_count` |
| `Verifiable` / `Unsafe` | **一律不重试**，`retry_count` 视为 0 |

为什么不给 `Unsafe` 也开 `is_connect()` 那一档——技术上它确实安全：因为
「安全」完全依赖于对 reqwest 错误分类边界的信任，而猜错的代价是用户发出
两条一样的评论。把判断压缩成一个 `if`，实现里就没有第三条路可以走错。这
是刻意的保守，有真实证据（抓到 `is_connect()` 被误分类之外的情形）再放开。

**超时一律不重试**，`Idempotent` 也不例外。幂等保证的是「状态一样」，不
保证「不被风控计数」——而写操作的风控计数是**账号级**的，代价比只读路径
高一个量级。基线的 `RetryInterceptor` 重试超时，这里刻意不跟；这是两条
路径行为不一致的一处，写在这里以免被当成 bug 修回去。

## 决策二：幂等级别是类型的一部分，不是文档里的一张表

每个写操作的导出函数**必须**给出它的 `Idempotency`，由类型系统逼着填：

```rust
pub enum Idempotency {
    /// 重发无副作用。服务端语义是「把状态置为 X」而不是「加一次 X」。
    Idempotent,
    /// 重发会多算，但**重发前能查到上一次是否已生效**。
    Verifiable,
    /// 重发会产生用户可见的重复内容或不可逆后果，且无从判定。
    Unsafe,
}
```

**不把它做成一张 ADR 里的 126 行表格。** 表格会过期：上游改一次接口语义，
表格还在这儿写着老答案，而代码照样编译通过。放进签名，漏填就编译不过，
改错了 code review 看得见。

**也不发明客户端幂等键。** B 站这些接口没有幂等键机制，自己造一个服务端
不认，发过去只是多一个参数——还会让请求面快照（决策六）与真实请求对不上。
「能不能重发」只能靠对接口语义的判断，这正是它必须进签名、被人看一眼的
原因。

## 决策三：定级的判据，以及一份做过的样例

判据按这个顺序问：

1. **它是不是真的发请求？** 有的写操作在未登录时根本不碰网络，只改本地。
   例：`VideoHttp.relationMod` 的拉黑分支在未登录时写 `Pref.setBlackMid(mid)`
   就返回了。这类不属于本 ADR，它是平台存储，留在宿主。
   （同 ADR-011 补充十一「先问这条接口是不是真的只读」，方向相反而已。）
2. **服务端语义是「置为」还是「加一次」？** 置为 → `Idempotent`；
   加一次 → 继续问第 3 条。
3. **重发前查得到上一次生没生效吗？** 查得到 → `Verifiable`；
   查不到，或者重复的后果用户直接看得见 → `Unsafe`。

`lib/http/video.dart` 的 15 条逐条过了一遍，作为样例：

| 写操作 | 级别 | 依据 |
| --- | --- | --- |
| `likeVideo` / `dislikeVideo` | `Idempotent` | 参数是 `like: '0'/'1'`，置为语义不是 toggle |
| `relationMod` | `Idempotent` | `act` 是目标关系而非增量（5 拉黑 / 6 取消拉黑，见 `lib/pages/blacklist/controller.dart`） |
| `pgcAdd` / `pgcDel` / `pgcUpdate` | `Idempotent` | 追番的加/删/改状态都是置为 |
| `replyDel` | `Idempotent` | 删第二次返回「不存在」，最终状态一致 |
| `historyReport` / `heartBeat` / `medialistHistory` / `roomEntryAction` | `Idempotent` | 进度上报是置为；但见下方「上报类的例外」 |
| `coinVideo` | `Verifiable` | 投币会**多扣硬币**；协议上 `/x/web-interface/archive/coins` 查得到本稿已投几枚，但**本仓库没接**——`hasCoinVideo` 在 `lib/http/api.dart:89` 是注释掉的，真要重发得先把它接上 |
| `ugcTriple` / `pgcTriple` | `Verifiable` | 三连含投币，跟着投币走 |
| `replyAdd` | `Unsafe` | 重发 = 用户看得见的两条相同评论 |

**上报类的例外**：`historyReport` / `heartBeat` 语义上幂等，但它们是**高频
自动触发**的。对它们重试的收益接近零（下一次心跳马上就来），代价却是
风控计数。定级填 `Idempotent` 没错，但调用方应当传 `retry_count: 0`——
这是传输配置的事，不是幂等级别的事，两者不要混。

其余 111 条**由各自切片在迁移时定级，并在 PR 里写明依据**。这里不替它们
猜：我没读过那些调用点，猜出来的级别比空着更危险——空着编译不过，猜错
了没人会再看一眼。

## 决策四：csrf 进 `RequestContext`，与 Cookie 同级

现状是 126 处各自写 `'csrf': Accounts.main.csrf`，散在每个 `data` map 里。

迁移后 **不** 让每个写操作的导出签名多一个 `String` 参数，而是：

```rust
pub struct RequestContext {
    pub base_url: String,
    pub cookie: Option<Redacted<String>>,
    /// `bili_jct`。写操作要把它放进表单体，**读操作不该有它**。
    pub csrf: Option<Redacted<String>>,
    pub headers: Vec<(String, String)>,
    pub timeout: Duration,
}
```

两条理由：

1. **它是凭据。** csrf 就是 cookie 里的 `bili_jct`（见
   `lib/utils/accounts/account.dart:95`），拿到它就能代用户发起写操作。
   散成普通 `String` 参数，它会跟着参数列表进日志、进 `error_detail`；
   包成 `Redacted` 则在类型上就打不出来（RFC §2.2）。
2. **一个注入点。** 126 处各自注入 = 126 次漏掉的机会，而漏掉的表现是
   服务端返回 `-111`，看起来像协议问题。

凭据仍然**不落 Rust**：`csrf` 和 `cookie` 一样按请求传入、用完即弃。

## 决策五：写操作没有降级开关，也不做 shadow

ADR-011 决策四对只读路径已经定了「不留旧实现、不设降级开关」。对写操作，
这条**从取舍升级成硬性要求**：

- 只读路径降级，代价是多发一次请求；
- **写操作降级，代价是可能重复提交**——Rust 发出去了、判定成失败、Dart
  再发一次，用户收获两条评论。

所以写操作的迁移是**原子替换**：同一个 PR 内删掉 Dart 原实现，不留并行
路径，不留开关。RFC §5.3 已经写了「禁止对写操作重复执行 shadow」，这里
是同一条约束在迁移期的具体形态。

推论：写操作切片**不能像只读切片那样先影子跑一阵再切**。它只有一次切换
机会，所以请求面快照（决策六）必须在切换前就位。

## 决策六：写操作的对照物是**请求**，不是响应

ADR-011 决策五定的对照机制是「Rust 冻结响应快照 + Dart parity 测试」。
补充九已经指出「冻结响应快照管不住请求」。对写操作，这不再是补充——
**它是主要矛盾**：

写操作的响应通常只有 `{"code":0}`，对它冻结快照几乎什么都证明不了。真正
会错、而且错了要到真机上才暴露的是**请求面**：URL、方法、content-type
（Form 还是 Json）、字段名与取值、csrf 放在体里还是查询串里。

因此每条写操作迁移时必须钉住一份**请求快照**：给定入参，断言最终发出的
`HttpPost` 长什么样。传输 Port 本来就是可替换的（RFC §16.6），拿桩件接
住请求即可，不需要网络。

## 决策七：风控（gaia）通道留在宿主，Rust 只把 `v_voucher` 带回来

回答 issue #50 评论里的四个问题：

1. **风控重试与普通重试不是同一套。** 普通重试在传输层、只认连接失败、
   用户无感；风控重放要走极验 WebView，**必然需要用户交互**，只能在调用
   点。传输层永远不自动重放被风控拦下的请求。
2. **`v_voucher` 不包 `Redacted`。** 它是一次性挑战票据，不是长期凭据，
   单独拿着它冒充不了用户；而且验证流程要把它显示/传给 WebView，包起来
   只会逼着到处 `expose()`，把 `Redacted` 的信号稀释掉。真正要包的是
   cookie 与 csrf。
3. **重放时 csrf 沿用，不重取。** csrf 在同一会话内不变，重取只会多一次
   读 cookie；而如果它真的变了，说明账号状态变了，这时**不该重放**，该让
   用户重新操作。
4. **这条通道不迁 Rust。** `gaiaVgateRegister` / `gaiaVgateValidate` 两个
   POST 本身可以迁（就是普通请求与解析），但它们夹着一个极验 WebView，
   交互必然留在宿主。按 RFC §2.3 的判据：换掉 Flutter 这层，WebView 那段
   要重写，请求与解析不用——所以边界划在这两者之间。

**另外记一笔，免得下一个人白走一遍**：基线里这条通道**是死代码**。
`RequestUtils.validate` 的唯一调用点是 `lib/http/search.dart` 里一句被注释
掉的 `RequestUtils.validate(vVoucher, onSuccess)`；S9 迁移时原样保留了钩子
（见 `lib/pages/search_panel/controller.dart` 的 `_onSuccess`，带
`ignore: unused_element`）。Rust 侧已经支持传 `gaia_vtoken`
（`bili-api/src/search.rs`）。**把它接通是一条独立的 issue，不要在写操作
切片里顺手做**——那会把「迁移是否等价」和「新功能是否正确」搅在一起。

## 决策八：本地副作用留在宿主，但顺序要定死

写操作常常带一个本地状态副作用。例：

```dart
if (res.data['code'] == 0) {
  GStorage.reply?.delete(rpid.toString());   // 删评论后清本地缓存
  return const Success(null);
}
```

本地存储是平台 API，按 RFC §2.3 留在宿主。但顺序必须是**先确认远端成功、
再改本地**，而且这个判断要用 Rust 返回的 `error_kind == null`，不要在 Dart
里重新解释 code——否则两边对「成功」的定义会漂。

## 落地进度

- **决策一已实现**（2026-11-12）：重试判定从一个布尔拆成
  `should_retry(RetryPolicy, FailureShape)`，`Idempotency` 三档进了
  `bili-api::http`，ADR 里那张表由 `retry_table_matches_adr_013` 逐格钉死。
  **只读行为一个字没改**——拆分前是 `is_connect() || is_timeout()`，拆分后
  `RetryPolicy::Read` 在同样两格为真，另有一条测试专门盯着这件事。
  `HttpTransport` 仍然只有 `get`：规则先就位，`post` 跟着第一个写操作
  切片开。
- 其余决策随各写操作切片落地。

## 影响

- `bili-api` 的 `HttpTransport` 从只读变成读写，`http.rs` 里那句「写操作
  要先有幂等与重试策略，不该顺手打开」随本 ADR 落地改写。
- 解锁 S4、S7、S8、S12、S14；以及 #60（短评五个写操作）、#61（SuperChat
  之外的写操作、`liveShieldUser`）、#66（`wishUpdate`、`sortFollowTag`、
  `danmakuFilterAdd` / `danmakuFilterDel`）的尾巴——这三个 issue 关不掉
  就是在等它。
- `lib/services/rust/sponsor_block_compat.dart` 随 SponsorBlock 的写接口
  迁移删除（ADR-011 决策六已记）。
- S14（登录）仍然单独对待：`login.dart` 那 11 条 + `init.dart` 的
  `buvidActive` 牵着凭据的产生而不只是使用，约定最后做、单独跑
  `/security-review`。

## 刻意没做的

- **没有逐条定级那 126 个接口。** 判据定了，样例做了一个文件，其余留给
  切片。理由见决策三末段。
- **没有定「写操作的 golden fixture 怎么脱敏」。** 写操作的响应没什么可
  冻的（决策六），真正要钉的请求快照里反而**不能有真凭据**——这一条跟着
  第一个写操作切片的实现补进来，现在写等于凭空规定。
- **没碰 gRPC 里的写操作。** 点赞、发评论的主路径可能走 gRPC，那条传输
  还没有（ADR-012 / #49）。本 ADR 定的是语义（幂等分级、csrf、不降级、
  请求面对照），它对两种传输都成立；gRPC 的具体承载由 ADR-012 回答，
  两份 ADR 的边界就在这里。
