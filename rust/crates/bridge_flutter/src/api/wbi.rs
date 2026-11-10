//! WBI 签名的 Dart 导出。
//!
//! Stage 1 的迁移样板（RFC-0001 §5.3）：Dart 侧保留原实现，通过功能开关
//! 决定走哪条路径，Rust 出问题可立刻降级，无需回滚版本。
//!
//! 粗粒度边界（RFC §2.2）：一次调用完成「取 mixin_key + 排序 + 编码 +
//! md5」整个过程，而非把每一步都暴露成单独的 FFI 调用。

use bili_protocol::wbi;

/// 由 nav 接口返回的 `img_key` / `sub_key` 推导 `mixin_key`。
///
/// 返回 `None` 表示输入长度不足，调用方应降级到 Dart 实现并上报，
/// 而不是用空串继续签名——那会让服务端返回业务错误码，掩盖真正原因。
#[must_use]
pub fn wbi_mixin_key(img_key: String, sub_key: String) -> Option<String> {
    wbi::mixin_key(&img_key, &sub_key)
}

/// 为参数计算 `w_rid`。
///
/// `params` 为键值对列表而非 Map：跨 FFI 的 Map 需要额外转换，而签名
/// 本就要自行排序，列表足够且更省一次拷贝。
///
/// 调用方须自行放入 `wts`（签名覆盖它）。由调用方注入时间可使本函数保持
/// 纯函数，从而能被 golden fixture 锁死。
#[must_use]
pub fn wbi_sign(params: Vec<(String, String)>, mixin_key: String) -> String {
    wbi::sign(&params, &mixin_key)
}
