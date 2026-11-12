//! 写操作共用的结果类型（ADR-013）。
//!
//! ## 为什么单独一个模块
//!
//! 绝大多数写操作成功时**没有载荷**：`code == 0` 就是全部结论。它们因此
//! 共用同一个结果形状，而这个形状不属于任何一个业务领域——与
//! [`crate::api::transport`] 里的 `TransportSettings` 同类，是写路径的
//! 基础设施。
//!
//! 第一版把它放在 `api::misc` 里（随弹幕屏蔽规则那条切片进来），`api::pgc`
//! 要用时就得跨领域引用 misc，读起来像 PGC 依赖「杂项」。搬到这里之后
//! 两边都只依赖基础设施。
//!
//! ## 为什么没有 `data` 字段
//!
//! 不写成 `data: Option<()>` 或塞个 bool：FRB 不支持 `Option<Option<T>>`，
//! 而「成功但没有值」在这套导出面里的既定形状就是「没有 data 字段的结果
//! 类型」（ADR-011，例见 `LiveMedalNameResult`）。调用方判成功看
//! `error_kind == null`。
//!
//! 需要回传载荷的写操作（例：新增弹幕屏蔽规则要回一个带 id 的 `FilterRule`）
//! 自己定带 `data` 的结果类型，不复用这个。

use bili_domain::error::DomainError;

/// 一次写操作的结果。成功时三个字段都是 `None`。
#[derive(Debug, Clone)]
pub struct WriteResult {
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub error_code: Option<i64>,
}

// 下面三个是**给 Rust 侧用的**构造helper，不是导出面的一部分：全部
// `pub(crate)`。写成 `pub` 的话 FRB 会把它们也当成导出项去生成绑定，而
// `from_result` 的参数是 `Result<(), DomainError>`——codegen 处理不了，
// 报出来的错像是 frb_generated.rs 自己坏了（`cannot find type DomainError`）。
impl WriteResult {
    /// 成功。
    #[must_use]
    pub(crate) const fn ok() -> Self {
        Self {
            error_kind: None,
            error_detail: None,
            error_code: None,
        }
    }

    /// 由领域错误构造。`error_detail` 只含分类后的文案与服务端 message，
    /// 不含请求参数——参数里可能有凭据，而错误对象会进日志。
    #[must_use]
    pub(crate) fn failed(e: &DomainError) -> Self {
        Self {
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
            error_code: e.code(),
        }
    }

    /// 把 `Result<(), DomainError>` 折成结果值。
    ///
    /// 写路径的每个导出都以这一句收尾，免得十几处各自抄一遍 match——
    /// 抄错一处的表现是「失败被当成了成功」，而那正是写操作最不该出的错。
    #[must_use]
    pub(crate) fn from_result(r: Result<(), DomainError>) -> Self {
        match r {
            Ok(()) => Self::ok(),
            Err(e) => Self::failed(&e),
        }
    }
}
