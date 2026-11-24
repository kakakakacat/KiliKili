#!/bin/sh
# 把 media_kit 带的 iOS libmpv（mpv 0.39 + ffmpeg 等 19 个 xcframework）软链到
# ios/Frameworks/，供 project.yml 链接与嵌入。不进仓库：65MB，而且 App 本身
# 跑 `flutter pub get` 时就已经下载在 pub 缓存里了。
set -eu
here=$(cd "$(dirname "$0")" && pwd)
src=$(find "${PUB_CACHE:-$HOME/.pub-cache}" -type d -path "*media_kit_libs_ios_video/ios/Frameworks" 2>/dev/null | head -1)
[ -n "$src" ] || { echo "pub 缓存里找不到 media_kit_libs_ios_video：先在仓库根目录 flutter pub get" >&2; exit 2; }
mkdir -p "$here/Frameworks"
for fw in "$src"/*.xcframework; do
  ln -sfn "$fw" "$here/Frameworks/$(basename "$fw")"
done
ls "$here/Frameworks" | sed 's/^/  /'
