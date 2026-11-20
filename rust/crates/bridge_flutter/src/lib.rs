//! flutter_rust_bridge 对 Dart 的导出层。
//!
//! **仅属 Stage 1**（RFC-0001 §3）：Phase 2 改用 Slint 后整个 crate 移除。
//! 因此这里只做转换与生命周期管理，**不得**承载业务逻辑——业务逻辑
//! 放在 `bili-core` / `bili-domain`，才能在 Phase 2 原样复用。
//!
//! 设计约束（RFC §2.2）：
//! - 对外仅暴露**粗粒度 API**，禁止每帧 / 每条弹幕跨 FFI。默认异步
//!   （FRB 会把非 `sync` 函数放到 Rust 工作线程，重活不占 Dart 线程）；
//!   `#[frb(sync)]` 只用于「基线那一侧本来就是同步的、且确实轻」的调用：
//!   弹幕会话的开关与取桶（`api::danmaku`）、清晰度/音质选择
//!   （`api::media_select`）、封面遮罩配色（`api::cover_scrim`）、评论
//!   正文分词与本地存档（`api::reply_grpc`）。判据见 ADR-010：改成异步会
//!   迫使调用方也变成异步，而这几处分别在渲染路径、起播路径、列表滚动
//!   路径与评论列表路径上。
//!
//!   其中分词（`build_reply_segments`）是唯一一个落在 **widget build**
//!   里的，值得单独说明为什么它不算「每帧跨 FFI」：基线在同一个位置每次
//!   build 都要新编译一条正则，这一笔比过一趟 FFI 更贵；改成异步则会让
//!   正文晚一帧出现。开销见该函数的文档
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
