#!/usr/bin/env bash
# Liquid Glass（issue #130）在 Android 真机上的跑法。
#
# 2026-11-25 跑通，证据与结论见
# docs/m0/spikes/2026-11-25-glass-android-notes.md。
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
#   ./android-drive.sh --perf          # 叠帧率 overlay（数字会出现在截图里）
#   ./android-drive.sh --out some/dir  # 证据落到别处
set -u

PKG=io.github.kakacat.kilikili.spike.glass
ACT="$PKG/android.app.NativeActivity"
FILES="/sdcard/Android/data/$PKG/files"
OUT="target/android-evidence"
BLUR=""
COUNT=""
PERF=""
FPS=""
TICK=""

ADB="${ADB:-adb}"
export MSYS_NO_PATHCONV=1   # 否则 Git Bash 会把 /sdcard/... 当成 Windows 路径

while [ $# -gt 0 ]; do
    case "$1" in
        --blur)  BLUR="$2"; shift 2 ;;
        --count) COUNT="$2"; shift 2 ;;
        # overlay 而不是 console：Android 上 Slint 的帧率日志走 eprintln，
        # 进不了 logcat；overlay 画在窗口里，截图就是读数。
        --perf)  PERF="refresh_full_speed,overlay"; shift ;;
        # 要求系统把面板切到这个帧率。adb 侧压不住 MIUI 的刷新率仲裁，
        # 只能由 app 自己用 ANativeWindow_setFrameRate 报，见 src/lib.rs。
        --fps)   FPS="$2"; shift 2 ;;
        # 背景 Timer 周期（ms）。16ms=62.5Hz 会和 60Hz vsync 撞在一起，
        # 要分清谁在限速就把它调小，见 ui/glass_spike.slint 的 tick-ms。
        --tick)  TICK="$2"; shift 2 ;;
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
if [ -n "$BLUR" ] || [ -n "$COUNT" ] || [ -n "$PERF" ] || [ -n "$FPS" ] || [ -n "$TICK" ]; then
    CONF="$OUT/glass.conf"
    : > "$CONF"
    [ -n "$BLUR" ]  && echo "BLUR=$BLUR"   >> "$CONF"
    [ -n "$COUNT" ] && echo "COUNT=$COUNT" >> "$CONF"
    [ -n "$PERF" ]  && echo "PERF=$PERF"   >> "$CONF"
    [ -n "$FPS" ]   && echo "FPS=$FPS"     >> "$CONF"
    [ -n "$TICK" ]  && echo "TICK=$TICK"   >> "$CONF"
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

# 帧率档多截一张：overlay 的读数是滑动平均，刚起来那几秒还混着冷启动
# （着色器编译、首次纹理上传）。只取第一张的话量到的是启动开销不是稳态。
if [ -n "$PERF" ]; then
    sleep 12
    "$ADB" exec-out screencap -p > "$OUT/shots/02-glass-settled.png"
    [ -s "$OUT/shots/02-glass-settled.png" ] || echo "!! 稳态截图是空的" >&2
fi

# 面板这一刻到底跑在多少赫兹——必须和截图同时取。
# 帧率读数的分母就是它：61fps 在 60Hz 面板上是「满了」，在 120Hz 面板上是
# 「只跑到一半」，两种解释差着一个结论。系统会在 app 起来几秒后重新仲裁，
# 所以**不能**拿启动时那次读数当准。
"$ADB" shell dumpsys SurfaceFlinger 2>/dev/null | grep -m1 activeMode > "$OUT/active-mode.txt"
echo "面板: $(cat "$OUT/active-mode.txt")"

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
