#!/bin/sh
# 构建、安装、启动实验台。用法：
#   DEVELOPMENT_TEAM=<团队 ID> SPIKE_DEVICE=<UDID> ./build-run.sh <env.json> [秒数] [日志文件]
# env.json 是 devicectl 的环境变量（含带签名的取流 URL，别放进仓库）。
set -eu
here=$(cd "$(dirname "$0")" && pwd)
cd "$here/ios"
./link-frameworks.sh >/dev/null
xcodegen generate -q
xcodebuild -project SlintIosSpike.xcodeproj -scheme SlintIosSpike -configuration Release \
  -destination "id=$SPIKE_DEVICE" -derivedDataPath ../target/xcode \
  DEVELOPMENT_TEAM="$DEVELOPMENT_TEAM" -allowProvisioningUpdates build -quiet
xcrun devicectl device install app --device "$SPIKE_DEVICE" \
  ../target/xcode/Build/Products/Release-iphoneos/SlintIosSpike.app >/dev/null
[ $# -ge 1 ] || exit 0
timeout "${2:-40}" xcrun devicectl device process launch --device "$SPIKE_DEVICE" --console \
  --terminate-existing --environment-variables "$(cat "$1")" dev.kilikili.spike.slintios \
  > "${3:-/dev/stdout}" 2>&1 || true
