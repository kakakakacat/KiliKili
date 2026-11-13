# gRPC 公共前置设计（#133）

用户已批准先执行 S2/S4/S5 方案中的第 1 步；业务切片本体不在本 PR。

## 范围与架构

执行已批准 ADR-012：在 bili-api 的 HttpTransport 增加二进制 POST 和保留响应头的返回类型，文本 REST 返回不变。统一复用 reqwest 客户端缓存、代理、证书、HTTP/1.1 和重试配置。只读 gRPC 使用读重试策略，写请求显式标记幂等级别，不注入 csrf。

gRPC 模块负责五字节帧、超过 64 字节才 gzip、响应长度校验、头部状态和双层 google.rpc.Status 业务码。头缺失、帧截断和多余帧必须返回 Protocol。protobuf 解析保留现有弹幕/字幕手写实现。

新增 bili-grpc 放描述符机械生成的 prost 消息。生成器按根消息及其依赖生成，不让 protoc 进入用户构建或跨平台矩阵；需要拒绝 required、package 不一致和未知类型。metadata 五个头在 Rust 编码，常量进 bili-protocol，凭据按请求传入且日志脱敏。未迁的 Dart gRPC 请求也调用 Rust metadata 编码，不保留两套编码算法。

先迁 DmGrpc.dmSegMobileRaw 与 dmViewSubtitles 的传输：前者返回解帧字节供现有弹幕会话处理，后者返回 typed 字幕轨。删除 requestRaw、被替代的字幕解析 JSON 接口与开关，不改其他 gRPC 业务 API。字幕 UI 若仍与未迁 JSON 来源共用 Subtitle 类，仅在宿主保留无决策的字段适配，并注明随其来源迁移删除。

## 验证与交付

先写边界测试并确认失败，再实现。测试覆盖 64/65 阈值、identity/gzip、长度错误、状态缺失、双层业务码、metadata 对照、真实 reqwest 桩请求和 typed FFI。fixture 优先抓真实匿名响应后脱敏；不可获取时明确区分手工构造，不能声称验证了线上协议。

生成一致性 CI 在 Ubuntu 独立运行，不改变五端构建的手动触发方式。所有 Rust 三项检查、Dart 改动文件分析、可运行 Flutter/FFI 测试与生成幂等检查必须完成。同步 RFC、ADR-012、协议记录、验证能力、分叉面和 QA。真机验证留 Phase 2，不自行合并。
