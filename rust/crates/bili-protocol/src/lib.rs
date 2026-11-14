//! 易变协议层。
//!
//! RFC-0001 §16.7：WBI 签名、必要请求头、设备信息格式等**易变协议**放在
//! 单独 crate，配版本化 fixture、变更记录与独立回归测试。
//!
//! 这里的代码正确性不取决于它是否"合理"，而取决于它是否与服务端的实现
//! **逐字节一致**。因此每项都必须有从基线产出的 golden fixture 锁死，
//! 不接受"看起来对"的实现。
//!
//! 随机值（风控 token）无法用固定期望对照，锁的是它的**形状**——长度
//! 区间与字符集，见 [`risk`]。
//!
//! 协议变更记录与 fixture 版本约定见
//! `docs/architecture/adr/ADR-008-protocol-versioning.md`。

pub mod app_sign;
pub mod bvid;
pub mod grpc;
pub mod keys;
pub mod risk;
pub mod wbi;

/// 本 crate 所复刻的协议快照版本。
///
/// 每次因上游协议变化而改动本 crate（新增/删除参数、算法调整、常量
/// 更新）都要 +1，并在 `docs/architecture/protocol-changelog.md` 记一行。
/// fixture 里也带同一个值，两边对不上时 golden 测试会失败——这正是
/// 「fixture 还停留在旧协议」的信号（ADR-008）。
pub const PROTOCOL_VERSION: u32 = 1;
