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
| APP 签名（`appkey`/`ts`/`sign`） | 2026-11-11，S11a 第一批 / issue #61 | `tool/fixture-gen/app_sign_fixture.dart` | `bili-protocol/tests/app_sign_golden.rs` |
| 直播取流请求面（`protocol`/`format`/`codec`/`ptype`/`dolby`/`panorama`/`web_location`） | 2026-11-11，S11a 第二批 / issue #61 | 无独立生成器，核对方向相反，见下 | `bili-api/tests/live_golden.rs::room_play_info_request_matches_the_frozen_snapshot` |
| 直播剩余三条签名接口的请求面（搜索的 APP 参数集；屏蔽设置与贡献榜的 WBI 参数集，各带一个 `web_location=444.8`） | 2026-11-11，S11a 第三批 / issue #61 | 同上 | `bili-api/tests/live_golden.rs::signed_requests_match_the_frozen_snapshot` |

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
