#!/usr/bin/env bash
# #218 插帧 spike 的 Android 驱动：推参数（和素材）、启动、等结束、拉记录。
#
#   ./interp-android.sh [--push <本地文件或目录>]... -- <interp 参数>
#
# 例：
#   ./interp-android.sh --push bbb30.mp4 -- --video bbb30.mp4 --mode mc --flip-y \
#       --hwdec mediacodec --duration 20 --delay-ms 50 --record rec.md
#
# 相对路径都按应用外部数据目录解析（不需要存储权限，adb 推得进去）。
# 记录同时逐行打进 logcat（tag kk-interp），脚本把 REPORT 行剥出来打印；
# --record 指定的文件拉回到 ./android-out/。
#
# 包要先用 `cargo apk build --release --lib --features interp` 打好并装上，
# 环境变量见 README「构建与运行」。
set -euo pipefail
cd "$(dirname "$0")"

PKG=io.github.kakacat.kilikili.spike
DIR=/sdcard/Android/data/$PKG/files
PUSH=()
while [[ $# -gt 0 && "$1" != "--" ]]; do
    case "$1" in
        --push) PUSH+=("$2"); shift 2 ;;
        *) echo "不认识的参数 $1（interp 的参数放在 -- 之后）" >&2; exit 2 ;;
    esac
done
[[ "${1:-}" == "--" ]] && shift
ARGS="$*"

adb shell mkdir -p "$DIR"
for item in "${PUSH[@]+"${PUSH[@]}"}"; do
    adb push "$item" "$DIR/" >/dev/null
    # adb 建出来的目录是 root:root 0770，应用（u0_aNNN）进不去，得放开。
    adb shell chmod -R a+rwX "$DIR/$(basename "$item")"
done
printf '%s\n' "$ARGS" > /tmp/kk-interp-args.txt
adb push /tmp/kk-interp-args.txt "$DIR/args.txt" >/dev/null

adb shell am force-stop "$PKG"
adb logcat -c
adb shell am start -n "$PKG/android.app.NativeActivity" >/dev/null

# 等 DONE / FAILED。上限按 --duration 再加一分钟。
LIMIT=$(( $(sed -n 's/.*--duration \([0-9]*\).*/\1/p' <<<"$ARGS" || true)0 / 10 + 60 ))
[[ $LIMIT -lt 60 ]] && LIMIT=120
for _ in $(seq 1 "$LIMIT"); do
    if adb logcat -d -s 'kk-interp:*' | grep -qE "DONE|FAILED"; then
        break
    fi
    sleep 1
done
adb logcat -d -s 'kk-interp:*' | sed -n 's/.*interp_android: //p' | grep -E "^(REPORT|FAILED|ARGS)" | sed 's/^REPORT //' || true

RECORD=$(sed -n 's/.*--record \([^ ]*\).*/\1/p' <<<"$ARGS")
if [[ -n "$RECORD" ]]; then
    mkdir -p android-out
    adb pull "$DIR/$RECORD" android-out/ >/dev/null && echo "记录已拉回 android-out/$RECORD"
fi
adb shell am force-stop "$PKG"
