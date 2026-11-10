//! 向 rustc 声明 `frb_expand` 为已知 cfg 名称。
//!
//! flutter_rust_bridge 的 `#[frb]` 宏展开时引用该 cfg，它仅在 codegen
//! 运行期由 CLI 注入，普通构建中不存在，否则 rustc 会报 unexpected_cfgs。
//!
//! 在此声明而非用 `allow(unexpected_cfgs)`：后者会连同真正的 cfg 拼写
//! 错误一起放过，而我们只想豁免这一个已知名称。
fn main() {
    println!("cargo::rustc-check-cfg=cfg(frb_expand)");
}
