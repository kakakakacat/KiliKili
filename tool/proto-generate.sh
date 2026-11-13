#!/usr/bin/env bash
# 只在协议描述符改变时运行；正常 app 构建不需要 prost-build 或 protoc。
set -euo pipefail
cd "$(dirname "$0")/.."
if [[ "${1:-}" == --check ]]; then
    output=$(mktemp -d)
    trap 'rm -rf "$output"' EXIT
    (cd rust && cargo run --locked --quiet --manifest-path ../tool/proto-generator/Cargo.toml -- "$output")
    diff -ru rust/crates/bili-grpc/src/generated "$output"
else
    (cd rust && cargo run --locked --quiet --manifest-path ../tool/proto-generator/Cargo.toml)
fi
