#!/usr/bin/env bash
# Media3 对照组的跑法，阶段与 `spikes/slint-mpv/android-drive.sh` **一一对应**：
# 同样的触摸、同样的转屏、同样的 HOME、同样的 PiP。两边只能差在实现上，
# 不能差在动作上，否则比的是脚本不是方案。
#
# 用法：./drive.sh [输出目录]
set -u

PKG=io.github.kakacat.kilikili.media3probe
ACT="$PKG/.MainActivity"
FILES="/sdcard/Android/data/$PKG/files"
OUT="${1:-app/build/evidence}"
APK="app/build/outputs/apk/release/app-release.apk"
# 两个探针放同一份素材，对照才有意义。
SRC_FILES="/sdcard/Android/data/io.github.kakacat.kilikili.spike/files"

export MSYS_NO_PATHCONV=1

mkdir -p "$OUT"
rm -f "$OUT"/*.png "$OUT"/*.txt

say() { printf '\n===== %s =====\n' "$*"; }
send() {
    adb shell "echo '$*' > $FILES/cmd.txt"
    sleep 1
}
mark() {
    say "$*"
    send "note $*"
}
screencap() { adb exec-out screencap -p >"$OUT/sys-$1.png"; }
dump_state() {
    {
        echo "--- 时间: $(date -Iseconds)"
        adb shell dumpsys activity activities 2>/dev/null |
            grep -iE "ResumedActivity|inPip|PictureInPicture" | head -20
        echo "--- 音频播放器"
        adb shell dumpsys audio 2>/dev/null | sed -n '/players:/,/^$/p' | head -30
    } >>"$OUT/state-$1.txt" 2>&1
}

say "0 准备"
if adb shell dumpsys window 2>/dev/null | grep -q "isKeyguardShowing=true"; then
    echo "!! 手机还锁着，先解锁"
    exit 1
fi
adb shell svc power stayon usb >/dev/null 2>&1
adb logcat -G 16M >/dev/null 2>&1
adb install -r "$APK" >/dev/null 2>&1 || { echo "装不上 $APK（先 ./gradlew assembleRelease）"; exit 1; }
# 素材用的是 Slint 探针那边同一份文件，但要**先拉回本机再 push**，不能在设备上
# `cp`：设备上 cp 出来的文件属 shell，app 读不到（权限 640，组是 ext_data_rw），
# 表现为 ExoPlayer 一声不吭地回到 IDLE。`adb push` 出来的是 644，app 读得到。
mkdir -p app/build/media
for f in probe.mp4 probe-audio.wav; do
    [[ -f "app/build/media/$f" ]] || adb pull "$SRC_FILES/$f" "app/build/media/$f" >/dev/null 2>&1
    adb push "app/build/media/$f" "$FILES/$f" >/dev/null 2>&1
done
# cmd.txt 不在这里建：由 app 自己建，否则同样属 shell，app 读不到。
adb shell settings put system accelerometer_rotation 0
adb shell settings put system user_rotation 0
adb shell am force-stop "$PKG"
adb logcat -c
adb shell am start -n "$ACT" >/dev/null
sleep 10
mark "起播完成"
dump_state 0-start
screencap 0-start

mark "1 触摸分发（按钮 vs 画面层）"
SIZE=$(adb shell wm size | sed -n 's/.*: \([0-9]*\)x\([0-9]*\).*/\1 \2/p' | tail -1)
W=$(echo "$SIZE" | cut -d' ' -f1)
H=$(echo "$SIZE" | cut -d' ' -f2)
adb shell input tap $((W / 2)) $((H / 3))
sleep 1
adb shell input swipe $((W / 4)) $((H / 2)) $((W * 3 / 4)) $((H / 2)) 400
sleep 2
screencap 1-touch

mark "2 旋转 -> 横屏"
adb shell settings put system user_rotation 1
sleep 6
screencap 2-landscape
mark "2 旋转 -> 竖屏"
adb shell settings put system user_rotation 0
sleep 6
screencap 2-portrait

mark "3 后台音频：HOME（Media3 这边不做任何策略，看框架的默认行为）"
adb shell input keyevent KEYCODE_HOME
sleep 12
dump_state 3-background
mark "3 回前台"
adb shell am start -n "$ACT" >/dev/null
sleep 8
screencap 3-resumed

mark "4 PiP：按钮进小窗"
send "pip"
sleep 6
dump_state 4-pip
screencap 4-pip
mark "4 PiP：回到全屏"
adb shell am start -n "$ACT" >/dev/null
sleep 6
screencap 4-expanded

mark "5 PiP：setAutoEnterEnabled(true) 之后按 HOME"
send "pip-auto on"
adb shell input keyevent KEYCODE_HOME
sleep 6
dump_state 5-pip-auto
screencap 5-pip-auto
adb shell am start -n "$ACT" >/dev/null
sleep 5
send "pip-auto off"

mark "6 收证据"
adb logcat -d -s kk-media3:I AndroidRuntime:E >"$OUT/logcat.txt" 2>&1
adb shell settings put system user_rotation 0

say "完成，证据在 $OUT"
ls -la "$OUT"
