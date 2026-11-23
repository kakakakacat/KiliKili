#!/bin/sh
# 从外面驱动实验台：切焦点、按键、切中文输入法、最小化/还原、抓 AX 树。
# Windows 版是 win/drive.ps1 + win/uia-dump.ps1，参数语义一一对应，
# 由 src/drive.rs 按平台选。
#
#   drive.sh build                                 # 编译两个 Swift 小工具
#   drive.sh <pid> keys <按键>                     # SendKeys 语法：字符原样，{TAB} {ENTER} {BACKSPACE}
#   drive.sh <pid> ime <klid> <工作目录>           # 00000409 → ABC，00000804 → 简体拼音
#   drive.sh <pid> ime-restore - <工作目录>        # 切回第一次 ime 之前的输入源
#   drive.sh <pid> minimize | restore
#   drive.sh <pid> uia <输出 JSON>
#
# 要求：终端在「系统设置 → 隐私与安全性 → 辅助功能」里打开（发按键、
# 最小化、抓树都要它）。没开时 System Events 报 -1719，本脚本照实输出。
#
# **会改系统状态的只有 ime**：它切的是全局输入源。第一次切之前把原来的
# 记进 <工作目录>/ime-original，实验结束时 drive.rs 调 ime-restore 切回去。
# 中途被杀掉的话，自己用 `input-source select <id>` 切回来。

set -u
here=$(cd "$(dirname "$0")" && pwd)

build() {
  for tool in ax-dump input-source; do
    if [ ! -x "$here/$tool" ] || [ "$here/$tool.swift" -nt "$here/$tool" ]; then
      swiftc -O "$here/$tool.swift" -o "$here/$tool" || exit 5
    fi
  done
}

if [ "${1:-}" = build ]; then build; echo built; exit 0; fi

pid=$1
action=$2
arg=${3:-}
work=${4:-.}

frontmost() {
  osascript -e "tell application \"System Events\" to set frontmost of (first process whose unix id is $pid) to true" >/dev/null 2>&1
  sleep 0.3
  front=$(osascript -e 'tell application "System Events" to get unix id of first process whose frontmost is true' 2>&1)
  # 切前台失败时按键会静默打到别的窗口上，看起来就像「程序不响应按键」。
  # 和 drive.ps1 检查 AppActivate 的返回值是同一个理由。
  if [ "$front" != "$pid" ]; then
    echo "activate=false（前台是 $front）"
    exit 3
  fi
}

window_minimized() {
  osascript -e "tell application \"System Events\" to tell (first process whose unix id is $pid) to set value of attribute \"AXMinimized\" of window 1 to $1" 2>&1
}

case $action in
  keys)
    frontmost
    # 一次一个键，给输入法留出候选更新的时间（drive.ps1 同理）。
    # `{TAB}` 这类转义整段识别；其余逐字符 keystroke，空格也是。
    rest=$arg
    while [ -n "$rest" ]; do
      case $rest in
        "{TAB}"*) osascript -e 'tell application "System Events" to key code 48'; rest=${rest#\{TAB\}} ;;
        "{ENTER}"*) osascript -e 'tell application "System Events" to key code 36'; rest=${rest#\{ENTER\}} ;;
        "{BACKSPACE}"*) osascript -e 'tell application "System Events" to key code 51'; rest=${rest#\{BACKSPACE\}} ;;
        *)
          ch=$(printf '%s' "$rest" | cut -c1)
          osascript -e "tell application \"System Events\" to keystroke \"$ch\""
          rest=${rest#?}
          ;;
      esac
      sleep 0.06
    done
    echo "activate=true"
    ;;
  ime)
    case $arg in
      00000409) id=com.apple.keylayout.ABC ;;
      00000804) id=com.apple.inputmethod.SCIM.ITABC ;;
      *) echo "不认识的 klid：$arg"; exit 2 ;;
    esac
    mkdir -p "$work"
    [ -f "$work/ime-original" ] || "$here/input-source" current > "$work/ime-original"
    frontmost
    "$here/input-source" select "$id" || exit 4
    sleep 0.5
    echo "input-source=$id"
    ;;
  ime-restore)
    if [ -f "$work/ime-original" ]; then
      original=$(cat "$work/ime-original")
      "$here/input-source" select "$original" >/dev/null && rm -f "$work/ime-original"
      echo "restored=$original"
    else
      echo "restored=（没切过）"
    fi
    ;;
  minimize) window_minimized true; echo minimized ;;
  restore) window_minimized false; echo restored ;;
  uia) "$here/ax-dump" "$pid" --json "$arg" ;;
  *) echo "不认识的动作：$action"; exit 2 ;;
esac
