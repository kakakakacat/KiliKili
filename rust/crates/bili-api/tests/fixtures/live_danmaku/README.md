# 直播消息样本

来源为开源协议文档提交 `271b123a083698bf576101c21f534b3418768a43` 中
实际接口响应与消息示例（2026-11-16 采集，非人工构造）：

- [gethistory 真实响应](https://github.com/pskdje/bilibili-API-collect/blob/271b123a083698bf576101c21f534b3418768a43/docs/live/danmaku.md)：`prefetch.json`，保留十条消息。
- [信息流真实示例](https://github.com/pskdje/bilibili-API-collect/blob/271b123a083698bf576101c21f534b3418768a43/docs/live/message_stream.md)：`token.json`、`danmaku.json`（2024-08-18 的含表情示例）、`super_chat.json`。

脱敏：用户/主播/房间/消息 id、昵称、token/hash 及所有图片/CDN 地址替换；保留
消息内容、颜色、时间、尺寸与协议结构。文档 JSONC 只移除注释，未增补字段。

`normalized.json` 是 Rust 输出，并以独立 Dart 3.13.7 脚本逐字段校验：抽取删除前
`DanmakuMsg.fromPrefetch`、控制器 `DANMU_MSG` 分支、`BaseEmote`、
`SuperChatItem/UserInfo`、`UinfoMedal.lightMedal` 与 token 的解析表达式，
脚本仅 import dart:convert / dart:io，不 import lib/，通过后删除。
比对结果：10 条预取 + 1 条含表情弹幕 + 1 条 SC + token 全字段相同。
`segments` 对照旧 `splitMapJoin(RegExp(...))`；`is_official` 对照旧 getter。

刻意归一：旧 `id/ts/ct` 中的 Object 换成 typed 的 String/i64/String；ct 只用
于举报透传，不做算术。旧模型的 `GlobalData.showMedal` 开关改由宿主在展示时
决定，实体始终保留已点亮勋章。SC 的 expired/deleted 属客户端生命周期状态，
不属于消息数据。旧 SC 缺失或非法 id 时使用随机值，Rust 保留可展示性，以原范围内循环递增的
本地 id 替代；该值只作为卡片展示键，并非有效的服务端举报 id。浮点 uid、时间
等数值按旧 safeToInt 向零截断，超出 i64 或非有限值拒绝。

表情分段的桥接表示采用 `emote_index` 引用原 `emotes` 数组，避免重复 URL 放大；
更换表示时逐字段展开索引与已通过 Dart parity 的旧投影比较，相等。
每条消息最多 256 个表情、64 KiB 标记和 2048 个片段，超过返回 protocol 并跳过该业务消息。
局部 DTO 保留表情插入顺序，单次 Aho-Corasick 匹配沿用原 RegExp 的左优先分支语义。
