//! SPIKE-002（issue #9）的 Android 入口所在的 cdylib。
//!
//! 桌面那条路是 `src/main.rs` 这个 bin，Android 上没有 `main`：
//! android-activity 的入口是 cdylib 里导出的 `android_main`。两个 target
//! 共存于同一个 package，共用 `gl` 与 `mpv` 两个模块。
//!
//! 模块用 `#[path]` 引进来而不是把 `main.rs` 改成「薄壳 + lib」：`main.rs`
//! 是在 Windows 上实测跑通过的那一份（见 docs/m0/spikes/2026-11-21-windows-dryrun.md），
//! 为了给 Android 搭个入口就动它的结构，等于拿已有的证据去冒险。代价是这
//! 两个模块会被编译两遍，spike 不在乎这点编译时间。

// 桌面上这个 lib 是空的——它只为 Android 存在。不加这层 cfg 的话，
// `cargo clippy` 在 Windows 上会为了一个用不到的 lib 把 mpv 模块再编一遍。
// `gl` 现在来自 ../spike-harness（与 SPIKE-003 共用），用 `use` 接回
// `crate::gl`，android_probe 那边的路径不变。
#[cfg(target_os = "android")]
pub(crate) use spike_harness::gl;

#[cfg(target_os = "android")]
#[path = "mpv/mod.rs"]
mod mpv;

// 默认打的是 SPIKE-002 探针；`--features interp` 换成 #218 插帧 spike 的入口。
// 两者都导出 `android_main`，所以只能二选一。
#[cfg(all(target_os = "android", not(feature = "interp")))]
mod android_probe;

#[cfg(all(target_os = "android", feature = "interp"))]
#[path = "interp/mod.rs"]
mod interp;

#[cfg(all(target_os = "android", feature = "interp"))]
mod interp_android;
