#!/usr/bin/env bash
# 先用 player-port-runtime-test.sh 生成同一套媒体，再对照原生原型的契约。
set -euo pipefail
if [[ "$(uname -s)" != Darwin ]]; then
  echo '当前脚本使用 macOS 动态库；其他平台尚未验证。' >&2
  exit 1
fi
cd "$(dirname "$0")/.."
export KILIKILI_NATIVE_PLAYER_RUNTIME_TESTS=1
export KILIKILI_PLAYER_MEDIA="$PWD/build/player-port-runtime/fixture.mp4"
export KILIKILI_LIBMPV="$(pkg-config --variable=libdir mpv)/libmpv.dylib"
test -f "$KILIKILI_PLAYER_MEDIA"
cd spikes/slint-mpv
cargo test --release --test player_port_runtime native_matches_flutter -- --nocapture
