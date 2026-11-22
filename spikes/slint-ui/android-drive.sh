#!/usr/bin/env bash
# SPIKE-003（issue #10）在 Android 真机上的跑法。
#
# Windows 那一轮的外部驱动是**进程自己**起 PowerShell（`win/drive.ps1`）：
# 按键、切输入法、最小化、抓 UIA 树都在同一台机器上。Android 上做不到——
# 这些动作只能从宿主机经 adb 发，于是分工变成：
#
#   设备上的探针  只管画面、帧耗时、虚拟化与内存（`--no-driver`）
#   这个脚本      管触摸、软键盘、无障碍树、系统截图，并把证据收回来
#
# 两边靠 logcat 里的 `STEP` 行对时间轴：探针每执行一步就打一行，脚本等到
# 那一行再动手。**不要改成按绝对时间 sleep**——真机上冷启动可以差好几秒，
# 对不上的后果不是报错，是「在列表场景里对着输入框发按键」这种静默错位。
#
# 前置：
#   - 手机已解锁并保持亮屏（锁屏时 activity 不可见，Slint 不渲染，看着像卡死）；
#   - 已 `cargo apk build --release --lib` 并安装（见 README）。
#
# 用法：./android-drive.sh [输出目录]
set -u

PKG=io.github.kakacat.kilikili.spike.ui
ACT="$PKG/android.app.NativeActivity"
FILES="/sdcard/Android/data/$PKG/files"
OUT="${1:-target/android-evidence}"

ADB="${ADB:-adb}"
export MSYS_NO_PATHCONV=1   # 否则 Git Bash 会把 /sdcard/... 当成 Windows 路径

mkdir -p "$OUT"
rm -f "$OUT"/*.png "$OUT"/*.txt "$OUT"/*.xml "$OUT"/record.md
# 目录也要删：`adb pull a b` 在 b 已存在时会把 a 塞进 b 里面，于是第二轮的
# 截图落在 `shots/shots/`，而上一轮的还留在 `shots/` —— 两轮的证据混在一起，
# 看文件名完全分不出来。
rm -rf "$OUT/shots"

say() { printf '\n===== %s =====\n' "$*"; }

# 等 logcat 里出现某个标记。超时就返回 1，由调用方决定要不要继续——
# 这里刻意不 `exit`：拿到一半的证据也比什么都没有强，缺哪一项记录里写明。
wait_for() {
    local pattern="$1" timeout="${2:-30}" waited=0
    while [ "$waited" -lt "$timeout" ]; do
        if $ADB logcat -d -s kk-spike:I 2>/dev/null | grep -q -- "$pattern"; then
            return 0
        fi
        sleep 1
        waited=$((waited + 1))
    done
    echo "!! 等不到 '$pattern'（${timeout}s）" >&2
    return 1
}

screencap() { $ADB exec-out screencap -p >"$OUT/sys-$1.png"; }

# ── 0. 清场 ────────────────────────────────────────────────────────────────
say "清场"
$ADB shell input keyevent KEYCODE_WAKEUP
$ADB shell am force-stop "$PKG"
$ADB shell rm -rf "$FILES/record.md" "$FILES/shots"
$ADB logcat -c
WINSIZE=$($ADB shell wm size | sed -n 's/.*: //p' | tr -d '\r')
W=${WINSIZE%x*}
H=${WINSIZE#*x}
echo "屏幕 ${W}x${H}"

# ── 1. 起探针 ─────────────────────────────────────────────────────────────
say "启动"
$ADB shell am start -n "$ACT" >/dev/null
wait_for "STEP" 40 || true

# ── 2. 触控惯性：Windows 上量不了的那一项 ─────────────────────────────────
#
# 这一段是 Android 独有的。`--touch-ms` 会在列表场景里留出一个空窗，探针在
# 那段时间里不自己滚，单独统计帧耗时，滚动完全由下面这几次 swipe 驱动——
# 量的就是「Flickable 的惯性在真机上跑成什么样」。
#
# swipe 的第四个参数是**持续时间**：越短速度越高。300ms 走大半屏是一次正常
# 的甩动；跟手慢拖要用 1200ms 那档，两种都要量，惯性只在前者出现。
say "触控：甩动"
wait_for "触控滚动" 30 || true
X=$((W / 2))
Y1=$((H * 3 / 4))
Y2=$((H / 4))
for _ in 1 2 3; do
    $ADB shell input swipe "$X" "$Y1" "$X" "$Y2" 300
    sleep 1
done
screencap "10-after-fling"
say "触控：慢拖"
$ADB shell input swipe "$X" "$Y1" "$X" "$Y2" 1200
screencap "11-after-slow-drag"

# ── 3. 软键盘与 IME ───────────────────────────────────────────────────────
#
# 先点一下输入框，再问系统「输入法面板起来了没有」。`mInputShown` 是
# InputMethodManagerService 自己的状态，比截图上「看起来有键盘」可靠。
say "输入与焦点"
wait_for "输入" 30 || true
$ADB shell input tap $((W / 2)) $((H / 3))
sleep 2
$ADB shell dumpsys input_method >"$OUT/ime-state.txt" 2>/dev/null
grep -o 'mInputShown=[a-z]*' "$OUT/ime-state.txt" | head -1
$ADB shell ime list -s >"$OUT/ime-list.txt" 2>/dev/null
# ASCII 直接上屏，走的不是输入法的组字路径——先确认「字进不进得去」。
#
# **分两次发，中间隔一秒**：第一轮跑出来输入框里是 "ello"，`h` 没了。只发
# 一次的话分不清是「Slint 吞掉了获得焦点后的第一个字符」还是「软键盘刚起来
# 时 adb 的第一个字符没人接」。两次的结果摆在一起就能分开：
#   helloworld → 两次都没丢，第一轮那个 h 是软键盘启动期的偶发
#   elloworld  → 只有焦点之后的第一个字符会丢
#   elloorld   → 每一次注入都丢头一个字符（那就是注入路径的问题，不是 Slint）
$ADB shell input text "hello"
sleep 1
$ADB shell input text "world"
sleep 1
screencap "12-input-ascii"

# ── 4. 无障碍树 ───────────────────────────────────────────────────────────
#
# `uiautomator dump` 导出的就是 AccessibilityNodeInfo 树，**TalkBack 读的
# 正是这棵树**。它不等于「读屏软件念出来的话」，但「树里有没有这个节点、
# 有没有名字和角色」这一层，它给的答案是确定的。
say "无障碍"
wait_for "无障碍" 40 || true
$ADB shell uiautomator dump /sdcard/ui-a11y.xml >/dev/null 2>&1
$ADB pull /sdcard/ui-a11y.xml "$OUT/a11y.xml" >/dev/null 2>&1
screencap "13-a11y"

# ── 5. 收尾：等探针写完记录再取 ───────────────────────────────────────────
say "等记录"
wait_for "SPIKE DONE" 60 || echo "!! 探针没报 DONE，记录可能不完整" >&2
$ADB pull "$FILES/record.md" "$OUT/record.md" >/dev/null 2>&1
$ADB pull "$FILES/shots" "$OUT/shots" >/dev/null 2>&1
$ADB logcat -d -s kk-spike:I >"$OUT/logcat.txt" 2>/dev/null

# ── 6. 把宿主机这一侧的事实追加进记录 ─────────────────────────────────────
#
# 探针问不到这些：它看不见自己在无障碍树里长什么样，也不知道软键盘起没起。
# 两边的证据必须落在**同一个文件**里，否则下一个人只会读到一半。
if [ -f "$OUT/record.md" ]; then
    NODES=$(grep -o '<node ' "$OUT/a11y.xml" 2>/dev/null | wc -l | tr -d ' ')
    NAMED=$(grep -oE '(text|content-desc)="[^"]+"' "$OUT/a11y.xml" 2>/dev/null | wc -l | tr -d ' ')
    SHOWN=$(grep -o 'mInputShown=[a-z]*' "$OUT/ime-state.txt" 2>/dev/null | head -1)
    IME=$($ADB shell settings get secure default_input_method 2>/dev/null | tr -d '\r')
    {
        echo
        echo "## 宿主机侧证据（android-drive.sh）"
        echo
        echo "探针跑在设备上，下面这些只有宿主机问得到。"
        echo
        echo "| 项 | 值 |"
        echo "|---|---|"
        echo "| 设备 | $($ADB shell getprop ro.product.model 2>/dev/null | tr -d '\r') / Android $($ADB shell getprop ro.build.version.release 2>/dev/null | tr -d '\r') / ${W}x${H} |"
        echo "| 触控：甩动 | \`input swipe $X $Y1 $X $Y2 300\` × 3（300ms 走半屏，正常甩动的速度档） |"
        echo "| 触控：慢拖 | \`input swipe $X $Y1 $X $Y2 1200\` × 1（跟手拖，不应当有惯性） |"
        echo "| 软键盘 | ${SHOWN:-<没取到>} |"
        echo "| 当前输入法 | ${IME:-<没取到>} |"
        echo "| 无障碍树节点数 | $NODES（其中带 text/content-desc 的 $NAMED） |"
        echo "| 系统截图 | sys-10-after-fling / sys-11-after-slow-drag / sys-12-input-ascii / sys-13-a11y |"
        echo
        echo "无障碍树原文在 \`a11y.xml\`，它就是 AccessibilityNodeInfo 树——"
        echo "**TalkBack 读的正是这棵树**。树里有没有节点是确定的事实；读屏软件"
        echo "具体念成什么话不是，这份记录不回答后者。"
    } >>"$OUT/record.md"
fi

say "证据在 $OUT"
ls -1 "$OUT"
