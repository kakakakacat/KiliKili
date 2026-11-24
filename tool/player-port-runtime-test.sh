#!/usr/bin/env bash
# Player Port 的真实 media-kit/libmpv 自动化；不改全局 Flutter SDK 或 pub cache。
set -euo pipefail
cd "$(dirname "$0")/.."
command -v flutter >/dev/null
command -v ffmpeg >/dev/null
command -v pkg-config >/dev/null
# 只用客户端头文件，不链接 mpv；避免 cflags 连带要求无关的静态依赖 .pc。
mpv_include="$(pkg-config --variable=includedir mpv)"
test -f "$mpv_include/mpv/client.h"

flutter pub get --offline
runtime_dir="$PWD/build/player-port-runtime"
mkdir -p "$runtime_dir"
event_loop_src="$(python3 - <<'PY'
import json
from pathlib import Path
from urllib.parse import urljoin, urlparse, unquote
config = Path('.dart_tool/package_config.json').resolve()
pkg = next(p for p in json.loads(config.read_text())['packages'] if p['name'] == 'media_kit_native_event_loop')
print(unquote(urlparse(urljoin(config.as_uri(), pkg['rootUri'])).path) + '/src')
PY
)"
case "$(uname -s)" in
  Darwin)
    mkdir -p "$runtime_dir/media_kit_native_event_loop.framework"
    event_loop_library="$runtime_dir/media_kit_native_event_loop.framework/media_kit_native_event_loop"
    c++ -std=c++17 -dynamiclib -install_name media_kit_native_event_loop.framework/media_kit_native_event_loop \
      -I"$mpv_include" -o "$event_loop_library" "$event_loop_src/media_kit_native_event_loop.cc"
    bridge_name=libbridge_flutter.dylib
    ;;
  Linux)
    event_loop_library="$runtime_dir/libmedia_kit_native_event_loop.so"
    c++ -std=c++17 -shared -fPIC -Wl,-soname,libmedia_kit_native_event_loop.so \
      -I"$mpv_include" -o "$event_loop_library" "$event_loop_src/media_kit_native_event_loop.cc"
    bridge_name=libbridge_flutter.so
    ;;
  *) echo '当前脚本支持 macOS / Linux；移动端表面与生命周期另按 ADR-003 真机验证。' >&2; exit 1 ;;
esac

ffmpeg -hide_banner -loglevel error -y -f lavfi -i 'testsrc2=size=320x180:rate=25' \
  -f lavfi -i 'sine=frequency=440' -t 15 -c:v libx264 -pix_fmt yuv420p -c:a aac "$runtime_dir/fixture.mp4"
ffmpeg -hide_banner -loglevel error -y -i "$runtime_dir/fixture.mp4" -map 0:v -c copy "$runtime_dir/video.mp4"
ffmpeg -hide_banner -loglevel error -y -i "$runtime_dir/fixture.mp4" -map 0:a -c copy "$runtime_dir/audio.m4a"
# 大于解复用缓存且 moov 前置，受控 HTTP 断流才能实际耗尽缓存。
ffmpeg -hide_banner -loglevel error -y -f lavfi -i 'testsrc2=size=640x360:rate=25' \
  -f lavfi -i 'sine=frequency=880' -t 30 -c:v libx264 -preset ultrafast \
  -b:v 3M -minrate 3M -maxrate 3M -bufsize 3M -g 25 -pix_fmt yuv420p \
  -c:a aac -movflags +faststart "$runtime_dir/quality.mp4"
(cd rust && cargo build -p bridge_flutter)
target_dir="$( (cd rust && cargo metadata --format-version 1 --no-deps) | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')"
export KILIKILI_RUST_BRIDGE_TESTS=1
export KILIKILI_PLAYER_RUNTIME_TESTS=1
export KILIKILI_PLAYER_BRIDGE_LIBRARY="$target_dir/debug/$bridge_name"
export KILIKILI_LIBMPV="$(pkg-config --variable=libdir mpv)/libmpv.$([[ "$bridge_name" == *.dylib ]] && echo dylib || echo so)"
export KILIKILI_PLAYER_MEDIA="$runtime_dir/fixture.mp4"
export KILIKILI_MEDIA_KIT_EVENT_LOOP_LIBRARY="$event_loop_library"
flutter test --no-pub test/bridge/player_port_bridge_test.dart test/player_port/media_kit_runtime_test.dart
