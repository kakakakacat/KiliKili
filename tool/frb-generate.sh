#!/usr/bin/env bash
# 重新生成 flutter_rust_bridge 绑定。
#
# 必须用本脚本而非直接调 codegen：
#   --no-web  RFC-0001 §1.2 明确不做 Web/WASM，该 flag 只能走命令行
#             （flutter_rust_bridge.yaml 不支持 no_web 键）
#   cargo fmt rustfmt 的 edition 2024 import 排序与 codegen 输出不一致，
#             不跑的话 CI 的 fmt --check 必红
#
# 三处版本必须一致，否则产生难查的 ABI 错配：
#   pubspec.yaml 的 flutter_rust_bridge
#   rust/crates/bridge-flutter/Cargo.toml 的 flutter_rust_bridge = "=x.y.z"
#   本机 flutter_rust_bridge_codegen CLI
set -euo pipefail

cd "$(dirname "$0")/.."

CLI_VER="$(flutter_rust_bridge_codegen --version | awk '{print $2}')"
CARGO_VER="$(grep -oE 'flutter_rust_bridge = "=[0-9.]+"' rust/crates/bridge-flutter/Cargo.toml | grep -oE '[0-9]+\.[0-9]+\.[0-9]+')"
PUB_VER="$(grep -E '^\s+flutter_rust_bridge: [0-9]' pubspec.yaml | grep -oE '[0-9]+\.[0-9]+\.[0-9]+')"

if [ "$CLI_VER" != "$CARGO_VER" ] || [ "$CLI_VER" != "$PUB_VER" ]; then
    echo "版本不一致，拒绝生成：" >&2
    echo "  codegen CLI : $CLI_VER" >&2
    echo "  Cargo.toml  : $CARGO_VER" >&2
    echo "  pubspec.yaml: $PUB_VER" >&2
    echo "修正方式：cargo install flutter_rust_bridge_codegen --version $CARGO_VER --locked" >&2
    exit 1
fi

echo "FRB $CLI_VER —— 生成绑定"
flutter_rust_bridge_codegen generate --no-web

echo "格式化 Rust"
(cd rust && cargo fmt)

echo "完成。记得复核 rust/crates/bridge-flutter/src/lib.rs 中"
echo "codegen 注入的 'mod frb_generated;' 位置——它会被插到模块文档注释之前，"
echo "导致 E0753 语法错误，需移到文档块之后。"
