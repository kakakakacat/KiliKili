# 协议变更记录

`bili-protocol::PROTOCOL_VERSION` 的变更历史。机制见
[ADR-008](adr/ADR-008-protocol-versioning.md)。

每次因上游协议变化而改动协议实现，都在此记一行，并重新生成全部
fixture（它们带有同一个版本号，对不上时 golden 测试会失败）。

| 版本 | 日期 | 变更 | 来源 |
| --- | --- | --- | --- |
| 1 | 2026-11-10 | 初始快照：WBI 签名（`w_rid`/`wts`、`mixinKeyEncTab` 重排表）、nav 取 key 与按日缓存、playurl 请求面（`fnval=4048`、`fourk`、`fnver`、`voice_balance`、`gaia_source=pre-load`、`isGaiaAvoided=true`、`web_location=1315873`、`dm_img_list`/`dm_img_str`/`dm_cover_img_str`/`dm_img_inter`、PGC/PUGV 的 `biz_type`/`scene`/`from_client`/`drm_tech_type`/`app_id` 及其降级分支） | 基线 `lib/utils/wbi_sign.dart`、`lib/http/video.dart`（issue #7） |
