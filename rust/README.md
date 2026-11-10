# Rust Core

PiliPlus / KiliKili 的共享业务核心。见 [RFC-0001](../docs/architecture/RFC-0001-rust-slint-migration.md)。

## 分层

| crate | 层 | 职责 | 禁止依赖 |
|---|---|---|---|
| `bili-domain` | domain | 业务实体、规则、错误、Port traits | Flutter / Slint / 平台 SDK / 协议实现 |
| `bili-core` | application | UseCase、播放器意图编排、事件与状态快照 | Flutter Widget / Slint 具体类型 |

按 RFC §4，crate **按功能边界逐个加入**，不一次性创建全部。拆分原则是**可独立测试、可替换**，不是目录越多越好。

## 开发

```sh
cd rust
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

工具链版本由 `rust-toolchain.toml` 锁定。**不要在本地覆盖**——开发在 macOS 与 Windows 两台机器间轮换，rustc 版本漂移会改变 borrow checker 行为与 clippy lint 集，导致同一份代码在两端结果不一致。

## 约定

- **错误**：统一经 `bili-domain::error`。诊断字段**不得包含 Cookie、Token 或其他凭据**（RFC §2.2）。
- **FFI 粒度**：对外暴露粗粒度异步 API。禁止每帧 / 每条弹幕跨 FFI；高频数据走批处理（RFC §2.2）。
- **取消**：事件需携带 `request_id` / `generation`，支持路由切换、Seek、搜索输入变更时丢弃过期结果。
- **lint**：`unwrap_used` / `expect_used` 在 workspace 级别警告——业务代码中它们会把可恢复错误变成崩溃。

## CI

独立的 [`rust.yml`](../.github/workflows/rust.yml)，不改动上游的五个 Flutter workflow，以免同步上游时冲突。含 fmt / clippy / test 与五个目标的交叉编译验证。

交叉编译只证明能产出产物，**不代表运行时正确**——真机与硬解验证见 [VALIDATION-CAPABILITY.md](../docs/m0/VALIDATION-CAPABILITY.md)。
