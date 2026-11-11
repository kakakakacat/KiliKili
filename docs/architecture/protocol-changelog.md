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
| APP 签名（`appkey`/`ts`/`sign`） | 2026-11-11，S11a / issue #61 | `tool/fixture-gen/app_sign_fixture.dart` | `bili-protocol/tests/app_sign_golden.rs` |

APP 签名与 WBI 是两套，差异（盐、时间字段、签名字段、值的编码口径）列在
`bili-protocol/src/app_sign.rs` 的模块文档里。两者在当前参数集上碰巧同
结果，正因为是碰巧，没有合并。
