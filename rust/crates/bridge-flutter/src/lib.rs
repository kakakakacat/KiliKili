//! flutter_rust_bridge 对 Dart 的导出层。
//!
//! **仅属 Stage 1**（RFC-0001 §3）：Phase 2 改用 Slint 后整个 crate 移除。
//! 因此这里只做转换与生命周期管理，**不得**承载业务逻辑——业务逻辑
//! 放在 `bili-core` / `bili-domain`，才能在 Phase 2 原样复用。
//!
//! 设计约束（RFC §2.2）：
//! - 对外仅暴露**粗粒度异步 API**，禁止每帧 / 每条弹幕跨 FFI
//! - 事件携带 `session_id` / `generation`，过期结果必须被丢弃
//! - 日志不得包含 Cookie/Token，统一经 `bili_domain::redact`

pub mod api;

// 由 flutter_rust_bridge codegen 生成，勿手工编辑。
//
// 生成代码必然违反我们的 workspace lint：FFI 样板需要 `unsafe` 跨越
// 语言边界，需要 `unwrap` 处理「只可能由 codegen 自身 bug 触发」的
// 情形。豁免**局部施加于此模块**，而非放宽整个 workspace——否则我们
// 自己手写的代码也会失去这层保护。
//
// 重新生成后若 fmt 失败，跑 `cargo fmt` 即可：rustfmt 的 edition 2024
// import 排序与 codegen 输出不一致，这是预期的。
#[allow(
    unsafe_code,
    missing_debug_implementations,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::all,
    clippy::pedantic,
    unexpected_cfgs
)]
mod frb_generated;
