# S11b 直播弹幕迁移 Implementation Plan

> **For agentic workers:** 使用 superpowers:subagent-driven-development 执行，实施后分范围符合性与代码质量两轮审查。

**Goal:** 完成 #62，Flutter 仅消费 Rust 直播弹幕批次和 typed 数据。

**Architecture:** bili-domain 定义语言中立实体，bili-api 负责 REST/协议/连接，bili-core（或连接模块内独立会话）维护取消与有界队列，bridge_flutter 负责会话注册及结果映射。宿主每 100ms 轮询，先校验 generation 再消费。

**Tech Stack:** Rust/Tokio、WebSocket、flate2/brotli、FRB、Dart/Flutter。

## 任务 1：领域、协议及 REST
- [x] 在 `bili-domain/src/live_danmaku.rs` 定义消息、表情、勋章、SC、房间事件和连接参数。
- [x] 在 `bili-api/tests/live_danmaku*.rs` 先写协议/解析/REST 请求测试并验证缺少实现时失败。
- [x] 在 `bili-api/src/live_danmaku.rs` 实现 16 字节包头、校验、压缩与 typed 映射，以及 token、prefetch、SC 请求。
- [x] 真实样本脱敏，离线 Dart 原解析一次性 parity 通过后删除，保留 golden 和来源说明。

## 任务 2：连接会话与桥接
- [x] 本地 socket 测试覆盖鉴权/心跳/候选地址、连接/接收取消、有界批次和终止错误。
- [x] `bili-api/src/live_socket.rs` 实现可取消连接和有界事件队列，无自动重连。
- [x] `bridge_flutter/src/api/live_danmaku.rs` 导出 open/poll/close/filter 与 typed REST 结果，错误一律值返回。
- [x] 修改导出清单后运行 `tool/frb-generate.sh`，不手工编辑生成器协议。

## 任务 3：宿主消费与旧实现删除
- [x] `lib/services/rust/rust_live_danmaku.dart` 注入正确账号、WBI、连接参数，直接返回 FRB 类型。
- [x] 改 `live_room/controller.dart` 和聊天/SC 消费者，timer 轮询，关闭时先递增 generation；在途响应双重失效检查。
- [x] 删除 `lib/tcp/live.dart`、被替代的直播弹幕/SC/token 模型与 HTTP 函数；共用模型最后 importer 消失才删。
- [x] 增加不依赖 SDK 补丁的生命周期验证；分析所有修改 Dart 文件的错误交集。

## 任务 4：交付验证和文档
- [x] 完成范围审查和质量审查，修复发现的问题。
- [x] `cargo fmt --check`、`cargo clippy --workspace --all-targets`、`cargo test --workspace`。
- [x] `dart analyze lib` 与适用 `flutter test`；记录环境限制。
- [x] 更新 ADR 长连接决策、RFC、RELEASE-VALIDATION 与 upstream-divergence 实测；协议请求面不变则不更新协议版本。
- [x] 提交关联 #62 的改动，交付分支/PR；未经许可不合并。


## 执行记录

真实脱敏 fixture 经独立 Dart parity 后删除旧模型；表情分段改为索引引用，
局部有序 DTO 和一次多模式匹配保留原分支顺序，不改变其他接口的 JSON 序列化。
队列按 typed 结构与堆容量估算限制 8 MiB；关闭覆盖连接、鉴权和接收阶段。
SC 迟到移除回调已按对象身份隔离，并在 Widget 销毁时取消延迟 Timer。

已合入最新 main，FRB 生成产物整体重生成；本机空间不足时只清理本工作区
可重建的 incremental 编译缓存，后续以 CARGO_INCREMENTAL=0 验证。
最终检查与真机未测边界记入 PR；不自行合并或关闭 issue。
