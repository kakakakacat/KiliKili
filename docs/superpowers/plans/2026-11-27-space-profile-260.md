# 个人空间信息失败修复计划（#260）

目标：修复空间主页缺少 APP 签名导致的 -400，并使资料错误与昵称分离。

设计：空间请求继续使用主账号 Cookie，Rust 复用 bili-protocol::app_sign，宿主传入公开客户端标识与时间。失败保留已有资料与标签；首次失败生成原有兜底标签；独立错误提示提供重试，成功后清除。重试期间不卸载已有内容。

实测：2026-11-27，相同匿名参数未签名返回 -400，签名返回 0 和 card。登录态仍需 Phase 2 验证，不读取本机私人凭据。

- [x] 在 space_golden.rs 增加签名覆盖、可选来源参数与固定时间回归，先确认失败。
- [x] 修改 bili-api/src/space.rs、bridge_flutter/src/api/space.rs 和 rust_space.dart，透传 appkey/appsec/ts，重跑 tool/frb-generate.sh。
- [x] MemberController 分离错误与昵称，失败保留已加载资料，页面提供独立提示和重试；添加行为回归。
- [x] 更新协议变更、上游分叉与人工验证文档。
- [x] cargo fmt --check、cargo clippy --workspace --all-targets、cargo test --workspace；dart analyze lib，运行适用的 Flutter 回归。

验证：Rust 全量测试 1054 通过、2 忽略；fmt 与 Clippy 通过。Flutter 控制器回归
5 项通过；Dart 分析 0 error，55 条既有 warning/info，修改文件没有诊断。
独立审查补齐了「无标签但已有资料」时的保留与页面刷新通知回归。
登录态和真机观感仍按 RELEASE-VALIDATION 的 Phase 2 路径验收。
