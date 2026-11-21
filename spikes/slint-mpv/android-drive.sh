#!/usr/bin/env bash
# SPIKE-002（issue #9）剩余五项的**脚本化**跑法：触摸分发、旋转、PiP、
# surface 生命周期、后台音频。
#
# 为什么要脚本化而不是手点：这几项都要「系统那边做一个动作、探针这边记一条
# 事实」，两边时间对不上就说不清谁先谁后。脚本把动作、时刻、取证三件事绑在
# 一起，跑完直接得到一份能贴进记录的日志。
#
# **阶段顺序是有讲究的**：PiP 排在后台音频前面。切过一次后台之后，
# `hwdec=mediacodec` 的画面就回不来了（见记录里的「坑 5」，要走 recover 配方
# 才行），这时候再去测 PiP，量到的是一个已经没有画面的小窗——第一轮就这么
# 误判过一次。
#
# 前置：
#   - 手机已解锁（**锁屏时 activity 根本不可见**，Slint 不渲染，看着像死锁）；
#   - 已 `./android-repack.sh` 打出带 supportsPictureInPicture 的包；
#   - probe.mp4 与 probe-audio.wav 已经在应用的外部数据目录里。
#
# 用法：./android-drive.sh [输出目录]
set -u

PKG=io.github.kakacat.kilikili.spike
ACT="$PKG/android.app.NativeActivity"
FILES="/sdcard/Android/data/$PKG/files"
OUT="${1:-target/android-evidence}"
APK="target/release/apk/spike-pip.apk"

export MSYS_NO_PATHCONV=1   # 否则 Git Bash 会把 /sdcard/... 当成 Windows 路径

mkdir -p "$OUT"
rm -f "$OUT"/*.png "$OUT"/*.txt

say() { printf '\n===== %s =====\n' "$*"; }

# 往探针的命令通道写一行。探针那边 4Hz 轮询，读到就执行并清空。
send() {
    adb shell "echo '$*' > $FILES/cmd.txt"
    sleep 1
}

# 阶段标记：日志里有它，事后对时间轴不用靠猜。
mark() {
    say "$*"
    send "note $*"
}

screencap() {
    adb exec-out screencap -p >"$OUT/sys-$1.png"
}

# 探针把按钮中心的**物理**坐标打在 BUTTONS 行里。尺寸一变（旋转、PiP）就会
# 重报一次，所以每次用之前都取最新那条，别缓存。
button() {
    local which="$1"
    adb logcat -d -s kk-spike:I 2>/dev/null |
        grep -o "BUTTONS .*" | tail -1 |
        sed -n "s/.*$which=\([0-9]*\),\([0-9]*\).*/\1 \2/p"
}

dump_state() {
    {
        echo "--- 时间: $(date -Iseconds)"
        echo "--- activity / PiP"
        adb shell dumpsys activity activities 2>/dev/null |
            grep -iE "ResumedActivity|supportsPip|inPip|PictureInPicture" | head -20
        echo "--- 音频播放器（state:started 才算真的在出声）"
        adb shell dumpsys audio 2>/dev/null |
            sed -n '/players:/,/^$/p' | head -30
        echo "--- 窗口"
        adb shell dumpsys window windows 2>/dev/null |
            grep -iE "mCurrentFocus|mFocusedApp|Window #.*spike" | head -10
    } >>"$OUT/state-$1.txt" 2>&1
}

# ---------------------------------------------------------------- 0. 准备
say "0 准备"
if adb shell dumpsys window 2>/dev/null | grep -q "isKeyguardShowing=true"; then
    echo "!! 手机还锁着：activity 不可见 → Slint 不渲染 → 什么都测不到。先解锁。"
    exit 1
fi
adb shell svc power stayon usb >/dev/null 2>&1
adb logcat -G 16M >/dev/null 2>&1
adb install -r "$APK" >/dev/null 2>&1 || { echo "装不上 $APK"; exit 1; }
adb shell am force-stop "$PKG"
adb shell "echo '' > $FILES/cmd.txt"
adb shell rm -f "$FILES"/shot-*.png
# 旋转统一从竖屏开始，否则上一次跑剩下的横屏会把「转屏」这一项变成空操作。
adb shell settings put system accelerometer_rotation 0
adb shell settings put system user_rotation 0
adb logcat -c
adb shell am start -n "$ACT" >/dev/null
sleep 10
mark "起播完成"
dump_state 0-start
screencap 0-start

# ---------------------------------------------------------------- 1. 触摸分发
mark "1 触摸分发"
SIZE=$(adb shell wm size | sed -n 's/.*: \([0-9]*\)x\([0-9]*\).*/\1 \2/p' | tail -1)
W=$(echo "$SIZE" | cut -d' ' -f1)
H=$(echo "$SIZE" | cut -d' ' -f2)
echo "屏幕 ${W}x${H}"

# 1a 点在画面上（没有按钮的地方）：应当是全屏 TouchArea 收到。
adb shell input tap $((W / 2)) $((H / 3))
sleep 1
# 1b 滑一下：手势分发要能出 move 事件，不然 Phase 2 的进度条/亮度手势没法做。
adb shell input swipe $((W / 4)) $((H / 2)) $((W * 3 / 4)) $((H / 2)) 400
sleep 1
# 1c 点 SHOT 按钮：命中测试要把事件给最上面那个元素，而不是全屏 TouchArea。
read -r SX SY < <(button shot)
if [[ -n "${SX:-}" ]]; then
    echo "SHOT 按钮 @ $SX,$SY"
    adb shell input tap "$SX" "$SY"
    sleep 2
else
    echo "!! 没拿到 BUTTONS 坐标"
fi
screencap 1-touch

# ---------------------------------------------------------------- 2. 旋转
mark "2 旋转 -> 横屏"
adb shell settings put system user_rotation 1
sleep 6
dump_state 2-landscape
screencap 2-landscape
mark "2 旋转 -> 竖屏"
adb shell settings put system user_rotation 0
sleep 6
screencap 2-portrait

# ---------------------------------------------------------------- 3. PiP
# 趁画面还活着测（见文件开头关于顺序的说明）。
mark "3 PiP：从状态线程调 enterPictureInPictureMode"
send "pip"
sleep 6
dump_state 3-pip
screencap 3-pip
mark "3 PiP：回到全屏"
adb shell am start -n "$ACT" >/dev/null
sleep 6
screencap 3-pip-expanded

mark "3 PiP：点按钮（UI 线程那条路）"
read -r PX PY < <(button pip)
if [[ -n "${PX:-}" ]]; then
    echo "PiP 按钮 @ $PX,$PY"
    adb shell input tap "$PX" "$PY"
    sleep 6
    dump_state 3-pip-button
    screencap 3-pip-button
    adb shell am start -n "$ACT" >/dev/null
    sleep 5
fi

mark "3 PiP：setAutoEnterEnabled(true) 之后按 HOME"
send "pip-auto on"
adb shell input keyevent KEYCODE_HOME
sleep 6
dump_state 3-pip-auto
screencap 3-pip-auto
adb shell am start -n "$ACT" >/dev/null
sleep 5
send "pip-auto off"

# ------------------------------------------------- 4. 后台音频（原样，不做策略）
mark "4 后台音频：原样 HOME"
adb shell input keyevent KEYCODE_HOME
sleep 12
dump_state 4-background
mark "4 回前台"
adb shell am start -n "$ACT" >/dev/null
sleep 8
send "shot-pair 4-resumed"
sleep 2
screencap 4-resumed

# --------------------------------- 5. 后台策略 + 画面恢复配方
# 重启一遍再测：上一阶段结束时画面已经没了（坑 5），不重启的话这一阶段量的
# 是一具尸体。
mark "5 后台音频：重启 + auto-lifecycle on"
adb shell am force-stop "$PKG"
sleep 2
adb shell am start -n "$ACT" >/dev/null
sleep 10
send "auto-lifecycle on"
adb shell input keyevent KEYCODE_HOME
sleep 12
dump_state 5-background-vidoff
mark "5 回前台"
adb shell am start -n "$ACT" >/dev/null
sleep 8
send "shot-pair 5-resumed"
sleep 2
screencap 5-resumed
mark "5 跑 recover 配方（记进度 -> loadfile -> seek 回去）"
send "recover"
sleep 8
send "shot-pair 5-recovered"
sleep 2
screencap 5-recovered

# ---------------------------------------------------------------- 6. 收尾取证
mark "6 收证据"
adb logcat -d -s kk-spike:I AndroidRuntime:E DEBUG:F >"$OUT/logcat.txt" 2>&1
adb shell "ls $FILES/shot-*.png" 2>/dev/null | tr -d '\r' | while read -r shot; do
    [[ -n "$shot" ]] && adb pull "$shot" "$OUT/" >/dev/null 2>&1
done
adb pull "$FILES/mpv.log" "$OUT/mpv.log" >/dev/null 2>&1
adb shell settings put system user_rotation 0
adb shell svc power stayon false >/dev/null 2>&1

say "完成，证据在 $OUT"
ls -la "$OUT"
