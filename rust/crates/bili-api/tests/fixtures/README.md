# golden fixture 的来源约定

**默认假设：这里的样本是手工构造的，没有真实响应作依据。** 只有下表显式
列出的几份取自真机/匿名抓取再脱敏；没列到的一律按「没有真实样本」对待。

## 为什么要把这件事写下来

手工构造的 fixture 会把**作者当时的类型假设一起冻进测试**，而那正是它该
拦住的东西。#161 的第三条 bug 就是这么漏到真机的：满级账号的
`level_info.next_exp` 服务端实际发的是字符串 `"--"`，当初那份
`user/user_info_max_level.raw.json` 里按「它当然是个数字」填的是 `0`，
于是真机上会炸的那一种形状正好被挡在测试之外，CI 全绿。

所以读 fixture 时要分清两件事：

- **它能证明的**：给定这份字节，解析结果是这样，重构不许改变它。
- **它不能证明的**：线上真的会发这种字节。构造样本对**字段类型**的断言
  只是作者的假设，不是证据。

发现某个字段的真实形状与假设不符时，改 fixture 并补一条回归用例，不要只改
DTO——否则下一个人照着同一份 fixture 又会猜回去。宽松解析的判据见
`crates/bili-api/src/lenient.rs` 的模块文档。

#271 是这条的第二个例子，而且比 #161 更难发现：`ogv_watch_progress.pgc`
里的片头片尾按「它当然和降级接口放在同一个地方」构造，放进了
`video_info.clip_info_list`。真实的 playview 把它放在外层
`video_extra.clip_info` 且类型是整数，`video_info.clip_info_list` 恒为
null——于是「番剧从来不跳片头」这个必现的线上故障，被一份自洽的构造样本
挡在了测试之外，CI 全绿。**构造样本连字段的「位置」都是假设**，不只是
类型。

## 有真实来源的那几份

| 位置 | 哪些是真的 | 其余 |
|---|---|---|
| `play_url/` | UGC 样本取自真机真实响应并脱敏；`clip_segments.pgc-fallback` 取自 2026-11-26 匿名抓的 `/pgc/player/web/v2/playurl` 并脱敏（地址、时长外的业务 id 已替换，`durl` 只留一条、`support_formats` 只留三档）；`ogv_watch_progress.pgc` 的 `video_extra.clip_info` **形状**（整数 `clip_type`、位置在外层）取自同日的真实 playview 响应 | PGC / PUGV 的其余部分为构造样本；`clip_segments.pgc-fallback` 里的 `current_watch_progress` 是构造值（匿名抓到的是 0，那样覆盖不到降级接口的进度路径） |
| `reply/` | `reply_list_paged`、`reply_reply` 取自**匿名**抓的真实响应并脱敏（rpid/oid/mid 等已替换） | 其余构造 |
| `video_page/` | `play_info`、`note_list` 取自真实响应并脱敏 | `ai_conclusion`、`play_info_rich` 手工构造——那两段要登录态才拿得到，匿名是 `code -101` |
| `space/` | 合集与列表取自真实响应再脱敏 | 投稿搜索那条手工构造 |
| `grpc/` | `dm_segment.frame.bin` / `.payload.pb` 是 2026-11-13 匿名请求 app 网关的真实响应，投影后重建单帧（详见该目录 README） | `metadata.dart.json` 是人工占位值 |

明确记录为**全部手工构造**的：`fav/`、`history/`、`msg/`、`pgc/`、`user/`、
`login/`、`grpc_im/`。其余目录没有留下来源说明，按默认假设处理。

## 脱敏

取自真实响应的样本一律脱敏后入库：CDN 地址、稿件 id、昵称、mid、评论 id
全部替换，仓库不保留未脱敏原件。响应体本身不含凭据（凭据在请求头）。

## 还没做完的

逐字段核对真实形状目前只做到了 `user.rs` / `login.rs`（issue #166）。其余
模块的 `Option<i64>` 仍然只有构造样本背书，新增字段时请按上面的判据自己
判一次，别照抄邻居。
