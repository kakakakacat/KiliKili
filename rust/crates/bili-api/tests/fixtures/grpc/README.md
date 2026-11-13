# gRPC 前置 fixture（#133）

- `metadata.dart.json`：由迁移前 `2705b39d6` 的 protobuf.dart 生成类型和
  `GrpcHeaders` 相同参数产出，覆盖 access_key 为 null、显式空串、占位非空值。
  buvid、access_key 与 session_id 均为人工占位值；不是登录态抓包。
- `dm_segment.frame.bin` / `dm_segment.payload.pb`：2026-11-13 匿名请求
  app 网关的 `DM/DmSegMobile`，HTTP 200 / `Grpc-Status: 0`。输入不含 Cookie
  或真实 access_key。原始响应约 462 KB；解帧后只保留前三条弹幕的 progress、
  mode、fontsize、color、weight、colorful，id/id_str、mid_hash、content、ctime
  全部替换，其他字段不复制。再由基线 protobuf.dart 编码，保留原响应的 gzip
  帧标志重建单帧。仓库不保留未脱敏原件、原始稿件/用户标识或原始评论。
- `payload.pb` 是基线 `GrpcReq.decompressProtobuf` 对脱敏帧的输出。
  `grpc_frame_parity_test.dart` 保留无 lib 导入的基线解帧 oracle；Rust
  `grpc_fetch.rs` 对相同帧断言相同载荷，离线逐字节对齐，不重复线上请求。

这些样本证明 metadata 编码与基线一致，并覆盖真实响应投影后的解帧。
它们不证明登录态设备头、代理/证书、所有网络环境或字幕线上协议都已真机验证；
后者沿用 `dm_view` 既有手工样本，运行时使用 HTTP 桩，人工确认留 Phase 2。
