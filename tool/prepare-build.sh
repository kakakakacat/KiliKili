#!/usr/bin/env bash
# 为指定平台准备构建环境。本机构建**必须**先跑本脚本。
#
# 为什么需要它：lib/scripts/patch.ps1 修改三处状态，其中两处是全局的，
# 且只有一处能用 git 复位。漏掉任何一步都会产生难以定位的编译错误。
#
#   1. Flutter SDK（$FLUTTER_ROOT）  —— git 仓库，可 reset
#   2. pub cache 的 cupertino_ui / material_ui —— **全局共享、非 git、
#      跨平台跨会话累积残留**，只能删掉重新拉取
#   3. 项目源码 —— git 仓库，可 reset
#
# 真实踩过的坑：先跑 `patch.ps1 iOS` 再跑 `patch.ps1 android`，SDK 已复位
# 但 pub cache 仍停在 iOS 状态，cupertino_ui 引用 iOS 补丁才提供的
# `popGestureEnabled_`，编译报 "getter isn't defined for type PageRoute"。
#
# 注意：pub cache 是全局目录，本脚本的清理会影响本机其他 Flutter 项目。
# CI 上不存在此问题（每个 job 均为全新环境），故 CI 绿不代表本机可构建。
set -euo pipefail

PLATFORM="${1:-}"
case "$PLATFORM" in
    android|iOS|macOS|Linux|windows) ;;
    *)
        echo "用法: $0 <android|iOS|macOS|Linux|windows>" >&2
        exit 1
        ;;
esac

cd "$(dirname "$0")/.."
REPO="$PWD"
: "${FLUTTER_ROOT:?请先 export FLUTTER_ROOT 指向 Flutter SDK}"

echo "[1/4] 复位 Flutter SDK"
git -C "$FLUTTER_ROOT" reset --hard HEAD --quiet
git -C "$FLUTTER_ROOT" clean -fd --quiet

echo "[2/4] 清除 pub cache 中被打过补丁的包"
rm -rf "$HOME"/.pub-cache/hosted/pub.dev/cupertino_ui-* \
       "$HOME"/.pub-cache/hosted/pub.dev/material_ui-*

echo "[3/4] 重新拉取依赖"
flutter pub get

echo "[4/4] 应用 $PLATFORM 补丁"
GITHUB_WORKSPACE="$REPO" FLUTTER_ROOT="$FLUTTER_ROOT" \
    pwsh -NoProfile -File lib/scripts/patch.ps1 "$PLATFORM"

echo
echo "就绪。现在可以构建 $PLATFORM。"
echo "构建后请检查 git status：工具链与补丁脚本会改动受版本控制的文件，"
echo "这些改动不得提交（详见 docs/m0/BASELINE.md §6.1）。"
