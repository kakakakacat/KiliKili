//! 弹幕引擎：标记自己、合并、过滤、按 100ms 分桶，并按会话缓存结果。
//!
//! ## 这个 crate 为什么存在
//!
//! RFC-0001 §4 给 `bili-danmaku` 的定位是「解码、索引、过滤、调度」。
//! 其中**解码**在 PR #41 落在了 `bili-api`（协议解码与视频详情、取流的
//! 解析是同一类工作，见那边的说明），留给本 crate 的是真正独立于协议的
//! 部分：弹幕拿到手之后怎么处理。分界线是「换一种弹幕协议，本 crate
//! 不用改」。
//!
//! 因此本 crate **不依赖 `bili-api`**（dev-dependency 除外，golden 测试
//! 需要用它解 fixture 字节），入参是 `bili_domain::danmaku` 的实体。
//!
//! ## 跨 FFI 的形状（issue #42 的主要决策，见 ADR-010）
//!
//! 不把整段弹幕序列化后交给 Dart。一个热门视频的单个分段有数千条，
//! 序列化加 Dart 侧反序列化比基线直接用 protobuf.dart 解还慢——基线
//! 已经为此在 256 KiB 以上切 isolate。正确的形状是：
//!
//! - Rust 持有已解码、已过滤、已分桶的分段（[`session::DanmakuSession`]）
//! - Dart 按播放进度取**一桶**（100ms），每次只过几条
//!
//! 这正是 RFC §2.2 的「粗粒度接口与批处理，避免每帧 / 每条弹幕跨 FFI」。
//!
//! ## 与基线的等价性
//!
//! 引擎的每一步都对着 `lib/pages/danmaku/controller.dart` 的
//! `handleDanmaku` 写，顺序见 [`engine`] 的模块文档。等价性由两层测试
//! 钉住：各模块的单测说明「为什么是这个行为」，`tests/engine_golden.rs`
//! 用基线自己跑出来的 golden 说明「确实是这个行为」。
//!
//! 唯一**不**等价的地方是正则：Dart `RegExp` 与 Rust `regex` 的语法和
//! 语义不是一个集合。处理办法是只接受两端相同的子集，其余拒绝并让整条
//! 路径降级回 Dart，见 [`rule`]。

pub mod cursor;
pub mod engine;
pub mod rule;
pub mod session;
