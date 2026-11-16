# ADR-014：直播消息长连接、取消与批量跨桥

状态：已实施（#62），五端运行时回归留 Phase 2。
日期：2026-11-16
关联：RFC-0001 §2.2、ADR-010、ADR-011、ADR-012

## 背景

直播消息使用 WebSocket、自定义 16 字节包头、JSON 与 zlib/brotli，
不使用 gRPC，不受 ADR-012 的 unary 传输约束。迁移边界包括 token、
历史弹幕预取、SuperChat REST 和长连接，共用模型必须在同一切片删除。

## 决策

1. Rust 数据层持有连接任务、鉴权与 30 秒心跳，按 token 响应的 host 顺序
   尝试连接。连接断开即终止，沿用原行为，不新增自动重连。鉴权失败保留业务码。
   HTTP Upgrade 复用现有 reqwest/rustls 的代理、证书及根证书配置，WebSocket
   握手固定 HTTP/1.1；连接、鉴权、写操作各有 10 秒超时，整个长连接不设总超时。
2. `open` 同步返回单调递增的 `session_id`，联网在共享 Tokio runtime 中
   执行；因此连接未完成也能 `close`。桥接注册表记录宿主给出的 `generation`，
   `poll`、更新规则、`close` 都带双标识。过期 poll 返回 `cancelled`，过期
   close 不关闭其他代际；重复 close 成功。关闭先清队列再 abort 任务，Drop
   同样取消；不等待远端 WebSocket close 握手，以免路由销毁受网络阻塞。
3. Rust 保留最多 2048 条、按 typed 数据结构和字符串/数组容量估算最多 8 MiB 的队列，满时淘汰最旧消息，
   批次报告 `dropped`；每次最多交付 512 条。终止状态与错误独立保存，不会被
   挤掉，已收到消息可先排空再宣告 ended。宿主每约 100ms 读取一次，禁止逐条
   弹幕跨桥。队列记账覆盖 typed 分段的展开，不含系统分配器的额外管理开销。
4. 帧上限 8 MiB，累计嵌套解压预算 8 MiB，最多四层嵌套、4096 个包。
   截断、非法长度、未知版本返回 protocol，不能越界或无限循环；未知业务命令
   忽略，一条业务 JSON 无效不阻塞同批后续消息。心跳回复不作为业务 JSON。
5. 关键词/uid 屏蔽、表情分段、官方表情识别、勋章点亮判断与 SC 默认配色
   在 Rust 执行。表情使用保留原 Map 顺序的局部 DTO 与一次多模式匹配，
   每条最多 256 个表情、64 KiB 标记和 2048 个片段，片段只带表情索引，
   避免 URL 复制与反复扫描造成资源放大。历史预取在消费前用最新规则一次批量计算保留掩码，不把完整
   表情与勋章再次序列化回桥。宿主只负责用 typed 片段创建 Widget，和将报告
   元数据适配给现有播放器 SDK。
6. SC 的数据不可变；宿主可单独保存卡片过期、删除及全屏展示截止时间。
   这些状态不写回协议实体。预取、SC、token 等在途响应也必须在消费前校验
   代际，关闭页面后不能创建迟到连接或修改列表。

## 验证与边界

本地 WebSocket 桩覆盖握手、候选地址、鉴权业务失败、typed 事件、30 秒
心跳序号、网络终止和连接中取消；注册表测试覆盖过期代际与幂等关闭。
协议测试与真实脱敏 fixture 钉住映射，fixture 来源及一次性独立 Dart parity
见 `bili-api/tests/fixtures/live_danmaku/README.md`。

新增 WebSocket 依赖不自带 TLS 或 native-tls/OpenSSL 后端，升级后的流复用
现有 reqwest rustls；brotli 与 flate2 使用跨平台 Rust 实现。当前本地证据不等于
五端真机验证；登录态、代理/证书、后台和高频房间的运行时行为统一在 Phase 2
执行，见 RELEASE-VALIDATION 的 S11b 行。不要以本地桩通过宣称线上回归通过。
