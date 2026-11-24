#!/usr/bin/env bash
# 两个模式分别起进程，避免上一次播放器影响 RSS；先跑 runtime 脚本准备资源。
set -euo pipefail
if [[ "$(uname -s)" != Darwin ]]; then
  echo '当前脚本使用 macOS 动态库；其他平台尚未验证。' >&2
  exit 1
fi
cd "$(dirname "$0")/.."
runtime_dir="$PWD/build/player-port-runtime"
export KILIKILI_PLAYER_BRIDGE_LIBRARY="$PWD/rust/target/debug/libbridge_flutter.dylib"
export KILIKILI_LIBMPV="$(pkg-config --variable=libdir mpv)/libmpv.dylib"
export KILIKILI_PLAYER_MEDIA="$runtime_dir/fixture.mp4"
export KILIKILI_MEDIA_KIT_EVENT_LOOP_LIBRARY="$runtime_dir/media_kit_native_event_loop.framework/media_kit_native_event_loop"
for round in 1 2 3; do
  # 交替顺序，减小系统缓存和温度的单向偏差。
  if [[ "$round" == 2 ]]; then modes='port baseline'; else modes='baseline port'; fi
  for mode in $modes; do
    KILIKILI_PLAYER_BENCH_MODE="$mode" flutter test --no-pub test/player_port/media_kit_benchmark_test.dart
  done
done
