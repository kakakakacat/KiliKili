# 协议变更记录

`bili-protocol::PROTOCOL_VERSION` 的变更历史。机制见
[ADR-008](adr/ADR-008-protocol-versioning.md)。

每次因上游协议变化而改动协议实现，都在此记一行，并重新生成全部
fixture（它们带有同一个版本号，对不上时 golden 测试会失败）。

| 版本 | 日期 | 变更 | 来源 |
| --- | --- | --- | --- |
| 1 | 2026-11-10 | 初始快照：WBI 签名（`w_rid`/`wts`、`mixinKeyEncTab` 重排表）、nav 取 key 与按日缓存、playurl 请求面（`fnval=4048`、`fourk`、`fnver`、`voice_balance`、`gaia_source=pre-load`、`isGaiaAvoided=true`、`web_location=1315873`、`dm_img_list`/`dm_img_str`/`dm_cover_img_str`/`dm_img_inter`、PGC/PUGV 的 `biz_type`/`scene`/`from_client`/`drm_tech_type`/`app_id` 及其降级分支） | 基线 `lib/utils/wbi_sign.dart`、`lib/http/video.dart`（issue #7） |

## 新增覆盖面不计入版本号

上表记的是**上游协议变了、我们跟着改**。把一个此前没复刻过的协议面
搬进来（本 crate 多了一个模块，既有模块一行未动）不记在这里，也不 +1。

理由是版本号的用途：ADR-008 决策二让每份 fixture 带上它，是为了让
「fixture 停在旧协议」在 CI 里显形。新增一个模块不会让既有 fixture 过期，
+1 只会强制重新生成两份无关 fixture，产生一个没有信号的 diff——下次真的
有协议变更时，反而更难从历史里看出哪一次是真的。

新增的协议面各自带 fixture 与 golden 测试，按 ADR-008 决策三由 Dart 基线
产出期望值：

| 协议面 | 加入于 | 生成器 | 测试 |
| --- | --- | --- | --- |
| gRPC metadata（设备/locale/network/fawkes）与弹幕/字幕 unary 请求面 | 2026-11-13，issue #133 | 迁移前 `2705b39d6` 的 protobuf.dart 编码（占位凭据） | `bili-api/tests/grpc_metadata.rs`、`grpc_fetch.rs` |
| APP 签名（`appkey`/`ts`/`sign`） | 2026-11-11，S11a 第一批 / issue #61 | `tool/fixture-gen/app_sign_fixture.dart` | `bili-protocol/tests/app_sign_golden.rs` |
| BV 号 ↔ av 号编码（58 进制字母表、两次换位、异或掩码） | 2026-11-13，S4 / issue #54 | `tool/fixture-gen/bvid_fixture.dart` | `bili-protocol/tests/bvid_golden.rs` |
| 直播取流请求面（`protocol`/`format`/`codec`/`ptype`/`dolby`/`panorama`/`web_location`） | 2026-11-11，S11a 第二批 / issue #61 | 无独立生成器，核对方向相反，见下 | `bili-api/tests/live_golden.rs::room_play_info_request_matches_the_frozen_snapshot` |
| 直播剩余三条签名接口的请求面（搜索的 APP 参数集；屏蔽设置与贡献榜的 WBI 参数集，各带一个 `web_location=444.8`） | 2026-11-11，S11a 第三批 / issue #61 | 同上 | `bili-api/tests/live_golden.rs::signed_requests_match_the_frozen_snapshot` |
| 登录请求面：设备号（16 随机字节 + 7 字节 BCD 本地时间 + 8 随机字节 + 和校验）、`login_session_id`（md5(buvid+毫秒)）、密码与 `dt` 的 RSA PKCS#1 v1.5 加密及 `dt` 多出的一层百分号编码 | 2026-11-16，S14 / issue #65 | 无独立生成器，核对方向相反，见下 | `bili-protocol/src/login_ids.rs`、`login_crypto.rs` 的单测，`bili-api/tests/login_golden.rs`，`test/services/rust/login_parity_test.dart` |

APP 签名与 WBI 是两套，差异（盐、时间字段、签名字段、值的编码口径）列在
`bili-protocol/src/app_sign.rs` 的模块文档里。两者在当前参数集上碰巧同
结果，正因为是碰巧，没有合并。

直播取流那一行的**核对方向与其余相反**：不是先由 Dart 生成器产出期望值
再喂给 Rust 测试，而是 Rust 侧先冻结参数集与 `w_rid`，由
`test/services/rust/live_playurl_parity_test.dart` 用基线 `WbiSign.encWbi`
的逐字拷贝独立算一遍核对。两种方向都满足 ADR-008 决策三要的那件事——
期望值由两个独立实现分别产出。这里选后者，是因为被复刻的不只是签名算法
（那条已有 `wbi_golden.rs`），还有**参数集本身**：`web_location` 在基线
里是 double 字面量 `444.8`，`ptype`/`dolby`/`panorama` 是 int，而
`encWbi` 取值用的是 `toString()`。参数表连同类型一起拷进 Dart 测试，才
能钉住「Rust 的字符串 `"444.8"` 等于 Dart 的 `444.8.toString()`」——写成
`444` 或 `444.80` 都会让 `w_rid` 变成另一个值，而服务端回的是业务错误码，
从现象完全反推不出是参数类型错了。

## 2026-11-11：取流请求面的核对变成单侧（issue #79）

取流的请求组装（WBI 参数集与签名）**没有改**，但**基线那一侧删除了**：
`VideoHttp.videoUrl` 随 typed 直连一并消失，发请求的只剩
`bili_api::request::play_url`。

因此 `tool/fixture-gen/playurl_fixture.dart` 的性质变了——它原本逐字复制
基线的参数组装，上游改了就会与 Rust 产生差异、测试变红；现在它是一份
**冻结的基线快照**，不再跟着上游走。

**同步上游时要手动比对取流参数**：上游若增删了 `fnval` / `fourk` /
`gaia_source` 之类，CI 不会红，只有服务端的风控画像会变。比对位置是上游的
`lib/http/video.dart::videoUrl` 与 `rust/crates/bili-api/src/request.rs` 的
`play_url`。

## 2026-11-13：修正搜索发现的迁移请求面（issue #120）

搜索发现原接口是 App 主机的 `/x/v2/search/recommend`，S9 却将它写成
Web 主机的 `/x/web-interface/wbi/search/square`，并遗漏原来由
`AccountManager` 注入的 App 签名和推荐账号 `access_key`。真实匿名请求
对照：同一组参数发往 square 返回业务 `code=-400`，发往 recommend
返回 `code=0` 与 `data.list`；这不是搜索结果页或 HTTP 400。

本次恢复原 App 路径、主机与 `appkey` / `ts` / `sign`，登录态沿用
`AccountType.recommend` 的 `access_key`，不发送 Web Cookie。这是修正
迁移遗漏，未改变上游协议或签名算法，因此协议版本仍为 1。
`bili-api::search::tests::recommend_uses_app_endpoint` 覆盖实际取数 URL、
匿名 / 空凭据 / 登录凭据及独立计算的签名快照。

## 2026-11-13：视频页写操作的请求面（issue #54）

S4 把 `lib/http/video.dart` 的十四条写操作搬进 `bili-api::video_write`。
**没有新的签名算法**——APP 端那四条（点赞、点踩、投币、不感兴趣）复用
既有的 `bili-protocol::app_sign`，web 端那十条不签名。因此不记版本号，
按「新增覆盖面不计入版本号」处理。

新增的是一份协议**编码**：BV 号与 av 号的互转（上表最后一行）。它此前
只存在于 Dart 的 `IdUtils`，而写操作的参数是 `aid`、调用点只有 `bvid`，
于是它第一次成为 Rust 侧的必需品。

这一面值得单独一行，是因为错的表现特别隐蔽：算错一位不会报错，只会让
请求指向**另一个稿件**——点赞点到别人头上。golden 覆盖往返、边界值
（接近 51 位上限）与**字母表的每一位**，最后一项是必须的：样本未必用上
全部 58 个字符，改一个没被命中的位置，往返测试照样全绿。

**基线那一份还活着**：`IdUtils.bv2av` 仍被几十处未迁的 Dart 路径使用。
两份实现并存期间由上面那份 golden 对照同一组样本钉住；Dart 那份随最后
一个调用点删除。同步上游时若 `IdUtils` 的字母表或掩码变了，golden 会红
——这正是它存在的意义。

写操作的请求面本身不靠 golden 冻结，靠的是 ADR-013 决策六的请求快照：
Rust 单测钉住构造出来的 `PreparedWrite`，`test/bridge/video_write_bridge_test.dart`
起桩服务端钉住**真的发出去的**那一个。

## 2026-11-13：短信极验预检覆盖 WebView（#140）

既有 iOS 原生 SDK 预检继续使用 `client_type=ios&pt=20`；Android 等
WebView 路径现在也检查 `gettype.php`，只发送 `gt`，与页面获取配置一致。
两者都使用 `NoAccount`，仅明确的 `error_34` 触发现有的一次整组注册刷新；
网络错误不当作注册失效。短信请求参数与签名未变，未修改 bili-protocol，
因此不调整其版本号或重建无关 fixture。登录请求仍随 #65 迁移。

## 2026-11-15：动态预约与投票请求面（issue #55）

预约创建/更新继续使用原有表单字段与 body csrf；动态预约按钮仍为 JSON body、
查询串 csrf。投票创建/编辑继续使用 JSON `vote_info` 与查询串 csrf；投票提交
保留 `csrf`、`csrf_token` 与查询串三处同值。新增 `CsrfIn::BodyAndQueryWithToken`
只表达这一条接口的既有请求形状，不改变签名算法，也不增加协议版本。请求字段、
内容类型、CSRF 位置和写操作不重试行为由 `bili-api::dynamic` 单测覆盖；真实登录态
响应样本尚缺，解析字段需要后续以脱敏样本复核。

## 2026-11-16：登录请求面加入，核对方向同样相反（issue #65）

登录的签名算法没有新东西（仍是 APP 签名），**新增的是参数集与两处编码**：

- `device_id` / `bili_local_id`：基线 `LoginUtils.genDeviceId` 里那句
  `Digest(bytes).toString()` **不是 md5**，`crypto` 包的 `Digest` 只是字节
  序列的包装，`toString` 即十六进制化。当成哈希实现会得到长度只有一半的
  设备号，而服务端不会因此报参数错误，只会给一个风控分支。
- `dt` 字段：基线写的是
  `Uri.encodeComponent(Encrypter(RSA(...)).encrypt(...).base64)`，于是表单
  序列化时 `%` 会被**再编码一次**（`%2F` → `%252F`），签名覆盖的也是编码
  后的值。看起来像 bug，但它是服务端实际收到的形状，去掉这一层等于改请求面。
  `password` 字段**没有**这一层，两者的差异原样保留。

核对方向与直播取流那条相同（Rust 先冻结、Dart 侧独立算一遍），原因也相同：
被复刻的不只是算法，还有参数集本身，而基线那一侧随本切片删除了。

fixture 里的 token 一律是 `fake-*` 字面量，快照只记形状不记值——登录响应
就是凭据本身，详见 `rust/crates/bili-api/tests/login_golden.rs` 的模块文档。

## 2026-11-17：用户与空间请求迁入 Rust（#56、#57）

关注分组、用户资料、历史/稍后再看与空间各列表的请求构造改由 Rust 负责，
沿用既有 WBI / APP 签名和参数集，不调整签名算法或协议版本。空间专栏保留
App 主机及 `bili-http-engine` / UA；小店保留 mall 主机、APP 签名查询串、
JSON 请求体及不带 csrf 的形状。小店虽用 POST，操作仍是幂等只读。
空间 gRPC 投稿搜索与图文流保持原服务方法和分页字段；未设置的可选参数
保持省略。请求形状由 Rust 单测约束，真实账号与网关行为仍需 Phase 2 验证。
