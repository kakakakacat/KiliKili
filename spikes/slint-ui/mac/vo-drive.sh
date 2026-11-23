#!/bin/sh
# 用真实的旁白（VoiceOver）走一遍某个场景，逐步记下它念了什么。
#
#   ./vo-drive.sh <场景 0-3> <按键…>
#
# 按键：R / L = VO+→ / VO+←；IN / OUT = 进入 / 退出组（VO+Shift+↓ / ↑）；
#       SP = VO+空格（激活）；PGDN = VO+Fn+↓（翻页滚动）；
#       KUP / KDN / KLT / KRT = 不带修饰键的方向键（进入滑块后调值用）；
#       T:文字 = 打字；AX = 抓一次 AX 树里的控件取值（核对「念的」与「实际的」）。
#
# 例：./vo-drive.sh 3 R R R R SP SP          # 勾选框来回切两次
#     ./vo-drive.sh 0 R R IN R R R R R R R R # 列表逐行往下
#
# 跑之前（缺一项都是静默失败，看着像「旁白什么都没念」）：
#   1. 旁白已打开（⌘F5）。脚本只控制它，不负责开。
#   2. 旁白实用工具 →「通用」→ 勾选「允许使用 AppleScript 控制旁白」，然后
#      把旁白关掉再打开一次。没勾时只有 `get version` 这类通用查询能过，
#      `output` / `move` 一律报 -1708「不理解」，读 last phrase 报 -1728。
#   3. 终端在「辅助功能」里打开（System Events 发按键要它）。
#   4. 第一次会弹「"终端"想要控制"旁白"」，点允许。
#
# 为什么用 System Events 发 VO 键，而不是旁白自己的 `move right`：
# 2026-11-23 在 macOS 27.3 上实测，`move right` 返回成功但光标原地不动——
# 访达里也一样，所以不是 Slint 的问题。发真实按键才动。
#
# 为什么每步只等 0.5 秒就读：旁白停留约一秒后会补念一句操作提示
# （「你当前在文本元素上。」），它会覆盖 last phrase，读晚了就只剩提示。

set -e
here=$(cd "$(dirname "$0")" && pwd)
bin="$here/../target/release/slint-ui-spike"
[ -x "$bin" ] || { echo "先在 spikes/slint-ui 下 cargo build --release" >&2; exit 2; }
if [ ! -x "$here/ax-dump" ] || [ "$here/ax-dump.swift" -nt "$here/ax-dump" ]; then
  swiftc -O "$here/ax-dump.swift" -o "$here/ax-dump"
fi

scene=$1; shift
"$bin" --scene "$scene" >/dev/null 2>&1 &
pid=$!
trap 'kill $pid 2>/dev/null' EXIT
sleep 3
osascript -e "tell application \"System Events\" to set frontmost of (first process whose unix id is $pid) to true" >/dev/null
# 再等一下：AccessKit 的树是懒建的，焦点刚进来时旁白拿到的是占位。
sleep 2.5

vo() { timeout 10 osascript -e "tell application id \"com.apple.VoiceOver\" to $1" 2>&1; }
key() {
  if [ -n "$2" ]; then
    osascript -e "tell application \"System Events\" to key code $1 using {$2}"
  else
    osascript -e "tell application \"System Events\" to key code $1"
  fi
}
VO="control down, option down"

echo "[开始] 光标：$(vo 'return text under cursor of vo cursor')"
for k in "$@"; do
  case $k in
    R) key 124 "$VO" ;;
    L) key 123 "$VO" ;;
    IN) key 125 "$VO, shift down" ;;
    OUT) key 126 "$VO, shift down" ;;
    SP) key 49 "$VO" ;;
    PGDN) key 121 "$VO" ;;
    KUP) key 126 ;;
    KDN) key 125 ;;
    KLT) key 123 ;;
    KRT) key 124 ;;
    AX) "$here/ax-dump" "$pid" 1 | grep -E "TextField|Slider|CheckBox" ;;
    T:*) osascript -e "tell application \"System Events\" to keystroke \"${k#T:}\"" ;;
    *) echo "不认识的按键：$k" >&2; exit 2 ;;
  esac
  sleep 0.5
  echo "[$k] 念：$(vo 'return content of last phrase') ｜光标：$(vo 'return text under cursor of vo cursor')"
  sleep 0.7
done
