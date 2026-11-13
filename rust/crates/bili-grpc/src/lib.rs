//! protobuf 协议生成类型；只供 Rust 内部编码与投影，不进入 FRB 导出面。
//! 通过 `tool/proto-generate.sh` 重生成，普通构建不需要 protoc。

include!("generated/mod.rs");
