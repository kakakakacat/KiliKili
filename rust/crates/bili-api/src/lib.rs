//! 数据层：接口请求的**构造**、响应 DTO 与到领域实体的映射。
//!
//! RFC-0001 §16.6 把 bili-api 归入 data 层的 HTTP Adapter。本 crate 当前
//! 只做其中可被纯函数测试的两端——**请求长什么样**与**响应变成什么**，
//! 中间的字节收发留在调用方。
//!
//! ## 为什么现在不含 HTTP 客户端
//!
//! Stage 1 的 Dart 侧已经持有一整套传输设施：Cookie/账号管理、buvid
//! 激活、HTTP/2、重试拦截器、风控请求头（见 `lib/http/init.dart`）。
//! 若在 Rust 这边再起一个客户端，有两条路，都更糟：
//!
//! - 把凭据与设备指纹复制一份到 Rust —— 同一账号状态出现两个真相源，
//!   且凭据多跨一次边界；
//! - 让 Rust 发第二次请求做 shadow 比对 —— 对同一账号重复请求只读接口，
//!   平白加一倍流量并触发风控，而 RFC §5.3 要的恰恰是「不重复执行」。
//!
//! 因此 Stage 1 的分工是：**Dart 发请求，Rust 解析**。shadow mode 比对的
//! 是同一份响应字节在两种实现下的规范化结果，差异必然来自解析本身。
//!
//! 等 Rust 接管传输（含风控参数，见 issue #7）时，再按 Port/Adapter 加入
//! 真正的客户端：届时 TLS 栈在 Android NDK 上的交叉编译是一个独立决策，
//! 需要自己的 ADR，不该夹带在本次迁移里。
//!
//! ## 分层边界
//!
//! DTO（`#[derive(Deserialize)]` 的那些类型）是**协议的形状**，只在本
//! crate 内可见；对外只返回 `bili_domain` 的实体。接口加字段、改类型、
//! 换嵌套层级都止步于此。

pub mod client;
pub mod danmaku;
pub mod dm_view;
pub mod envelope;
pub mod http;
pub mod nav;
pub mod playurl;
pub(crate) mod protobuf;
pub mod reply;
pub mod request;
pub mod search;
pub mod search_suggest;
pub mod search_text;
pub mod sponsor_block;
pub mod subtitle;
#[cfg(feature = "reqwest-transport")]
pub mod transport_reqwest;
pub mod video_detail;
pub mod video_feed;
pub mod video_page;
