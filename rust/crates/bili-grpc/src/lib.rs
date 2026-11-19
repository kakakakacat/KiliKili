//! protobuf 协议生成类型；只供 Rust 内部编码与投影，不进入 FRB 导出面。
//! 通过 `tool/proto-generate.sh` 重生成，普通构建不需要 protoc。

// 生成码按 proto 的形状来，不按 Rust 的审美来：`MixedCard.item` 这种
// oneof 的分支大小本来就差得远，为它加 Box 等于手改生成产物。
#[allow(clippy::large_enum_variant)]
mod inner {
    include!("generated/mod.rs");
}
pub use inner::*;
