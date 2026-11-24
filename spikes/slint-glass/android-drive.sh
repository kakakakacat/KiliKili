#!/usr/bin/env bash
# Liquid Glass（issue #130）在 Android 真机上的跑法。
#
# **这个脚本一次都没有跑过**，和它驱动的那个 APK 一样——写的时候机器不能占。
# 第一次跑之前先读 README 顶部的状态那段。
#
# 分工照 slint-ui（#10）：
#
#   设备上的 APK  只管画面（`src/lib.rs` 的 android_main）
#   这个脚本      管安装、配置、启动、截图，并把证据收回来
#
# 两边靠 logcat 里的 `GLASS STEP` 行对时间轴：APK 每走一步打一行，脚本等到
# 那一行再动手。**不要改成按绝对时间 sleep**——真机冷启动能差好几秒，对不上
# 的后果不是报错，是「窗口还没出来就截了一张黑图」，而记录照样产出。
#
# 前置：
#   - 手机已解锁并保持亮屏（锁屏时 activity 不可见，Slint 不渲染，看着像卡死）；
#   - 已 `cargo apk build --lib`（见 README；**别在别人用着机器的时候跑**，
#     Android 这轮要下 skia 的预编译二进制再交叉编译几百个 crate）。
#
# 用法：
#   ./android-drive.sh                 # 默认档：blur 开、一条玻璃
#   ./android-drive.sh --blur 0        # 关掉 backdrop-blur，做对照
#   ./android-drive.sh --count 5       # 摆 5 条玻璃
#   ./android-drive.sh --out some/dir  # 证据落到别处
set -u

PKG=io.github.kakacat.kilikili.spike.glass
ACT="$PKG/android.app.NativeActivity"
FILES="/sdcard/Android/data/$PKG/files"
OUT="target/android-evidence"
BLUR=""
COUNT=""

ADB="${ADB:-adb}"
export MSYS_NO_PATHCONV=1   # 否则 Git Bash 会把 /sdcard/... 当成 Windows 路径

while [ $# -gt 0 ]; do
    case "$1" in
        --blur)  BLUR="$2"; shift 2 ;;
        --count) COUNT="$2"; shift 2 ;;
        --out)   OUT="$2"; shift 2 ;;
        *) echo "不认识的参数: $1" >&2; exit 2 ;;
    esac
done

say() { printf '\n===== %s =====\n' "$*"; }

# 等 logcat 里出现某个标记。超时返回 1，由调用方决定要不要继续——
# 这里刻意不 `exit`：拿到一半的证据也比什么都没有强，缺哪一项记录里写明。
wait_for() {
    local needle="$1" secs="${2:-30}" i=0
    while [ "$i" -lt "$secs" ]; do
        if "$ADB" logcat -d -s kk-glass 2>/dev/null | grep -qF "$needle"; then
            return 0
        fi
        sleep 1
        i=$((i + 1))
    done
    echo "!! 等不到 logcat 标记: $needle（${secs}s）" >&2
    return 1
}

say "设备"
"$ADB" devices
if ! "$ADB" shell true >/dev/null 2>&1; then
    echo "!! adb 连不上设备" >&2
    exit 1
fi

APK=$(ls -t target/debug/apk/*.apk target/release/apk/*.apk 2>/dev/null | head -1)
if [ -z "$APK" ]; then
    echo "!! 没找到 APK，先跑 cargo apk build --lib" >&2
    exit 1
fi

mkdir -p "$OUT"
# 目录也要删：`adb pull a b` 在 b 已存在时会把 a 塞进 b 里面，于是第二轮的
# 截图落在 shots/shots/，而上一轮的还留在 shots/——两轮证据混在一起，看
# 文件名完全分不出来。这条是 slint-ui 交过的学费。
rm -rf "$OUT/shots"
rm -f "$OUT"/*.png "$OUT"/*.txt

say "安装 $APK"
"$ADB" install -r "$APK" || exit 1

say "配置"
"$ADB" shell mkdir -p "$FILES"
"$ADB" shell rm -f "$FILES/glass.conf"
if [ -n "$BLUR" ] || [ -n "$COUNT" ]; then
    CONF="$OUT/glass.conf"
    : > "$CONF"
    [ -n "$BLUR" ]  && echo "BLUR=$BLUR"   >> "$CONF"
    [ -n "$COUNT" ] && echo "COUNT=$COUNT" >> "$CONF"
    "$ADB" push "$CONF" "$FILES/glass.conf"
    echo "档位: $(tr '\n' ' ' < "$CONF")"
else
    echo "档位: 默认（blur 开、一条玻璃）"
fi

say "启动"
"$ADB" logcat -c
"$ADB" shell am force-stop "$PKG"
"$ADB" shell am start -n "$ACT" || exit 1

# 等 run 那一行，而不是等固定秒数。
wait_for "GLASS STEP run" 40 || echo "（继续，但下面的截图可能是空窗口）"
# 画面起来之后再给一拍：STEP run 打在 app.run() 之前，第一帧还没画。
sleep 2

say "截图"
mkdir -p "$OUT/shots"
"$ADB" exec-out screencap -p > "$OUT/shots/01-glass.png"
if [ ! -s "$OUT/shots/01-glass.png" ]; then
    echo "!! 截图是空的" >&2
fi

say "logcat"
"$ADB" logcat -d -s kk-glass > "$OUT/logcat-kk-glass.txt"
# 渲染器是哪个、有没有静默回退，只有这里看得出来。
"$ADB" logcat -d | grep -iE "slint|skia|vulkan|EGL" | tail -80 > "$OUT/logcat-render.txt"

say "收尾"
"$ADB" shell am force-stop "$PKG"
echo "证据在 $OUT/"
ls -l "$OUT" "$OUT/shots" 2>/dev/null

cat <<'NOTE'

接下来要人做的两件事，脚本代替不了：

1. **看截图**，判断胶囊后面的背景到底糊没糊。Android 上 Slint 走的是
   Skia，不是桌面那条 FemtoVG，**桌面那轮的结论不能代替这一轮**。
2. 跑一次 `--blur 0` 做对照，两张并排看。只有一张图的话，「糊了」和
   「背景本来就是这个样子」分不开——#10 那次「虚拟化极好 + 滚动极卡」
   就是只看数字不看画面栽的。
NOTE
