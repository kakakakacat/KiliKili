# gRPC 公共前置实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 落实 #133 的 ADR-012 公共基础并迁移弹幕/字幕传输。

**Architecture:** 描述符生成 bili-grpc；bili-api 负责帧与传输；bridge_flutter 导出中立 typed 数据，Dart 注入账号与设置。保留文本 REST Port 与现有 protobuf 解析。

**Tech Stack:** Rust、prost、flate2、reqwest、FRB 2.13.0、Dart。

## 1. 描述符生成工具（独立任务）

- [ ] 创建 tool/proto-generate.sh、描述符生成器、rust/crates/bili-grpc；根仅包含 metadata 与弹幕请求。
- [ ] 先测试 required、package、未知类型拒绝和两次生成一致，再实现工具。
- [ ] 运行生成器并验证无 protoc 构建依赖，签入生成消息。

## 2. HTTP 与 gRPC（独立任务）

- [ ] 在 bili-api/tests/grpc_transport.rs 先写帧阈值、错误码、长度校验测试并确认缺实现失败。
- [ ] 修改 http.rs、transport_reqwest.rs 与 client.rs 的必要分支，增加 PostBody::Bytes、HttpBytesResponse、post_bytes；沿用显式重试策略。
- [ ] 新增 grpc.rs，实现帧与状态解析，不重写弹幕/字幕 protobuf 解码。
- [ ] 用本机 TCP 桩检查真正发出的字节、头与 HTTP/1.1；验证错误与重试边界。

## 3. metadata 与两个调用点

- [ ] 先用 Dart 基线编码器固定凭据脱敏的 metadata 预期值，增加 Rust 对照测试。
- [ ] 在 bili-protocol 固定协议参数，bili-api 用生成消息编码 metadata。
- [ ] 新增 FRB GrpcIdentity/GrpcContext、typed 字幕结果、字节结果与取数接口。
- [ ] 修改 grpc_headers.dart、dm.dart、rust_dm_view.dart；删除 requestRaw、旧 JSON 解析导出与字幕开关，适配现有 Subtitle 消费端。
- [ ] 重跑 tool/frb-generate.sh，增加真正加载原生库的请求桩测试。

## 4. 文档、CI 与收尾

- [ ] Ubuntu 生成一致性 job 加入 Regression CI 聚合；不恢复 PR 自动检查。
- [ ] 同步 RFC、ADR、协议记录、验证能力、分叉面与人工验证路径。
- [ ] cargo fmt --check、cargo clippy --workspace --all-targets、cargo test --workspace；dart analyze lib 按改动文件判定；运行可用 Flutter 与 FFI 测试。
- [ ] 分别审查规格符合性和代码质量，修正后 push 并开 PR，引用 #133；不合并。
