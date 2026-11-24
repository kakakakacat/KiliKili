#!/usr/bin/env bash
# 在 CI 上装 Linux 原生依赖，带重试与明确的失败归因（issue #91）。
#
# 为什么要有这个脚本：2026-11-11 runner 侧 DNS 抖动，少数几个包从
# security.ubuntu.com 解析不了，`apt-get install` 以 exit 100 结束。它在 PR
# 页面上的呈现是「Dart baseline and real FFI / test 失败」，看起来像测试挂了,
# 排查的人第一反应是去查代码。重试一次基本就能过，所以这里重试，并在最终失败
# 时打一条写明「装依赖失败」的注解，省掉下一个人翻日志那一段弯路。
#
# 用法：tool/ci-apt-install.sh <包名>...
#
# 可覆盖的环境变量（给 tool/test-ci-apt-install.sh 用，CI 上不设）：
#   APT_GET              apt-get 的路径，默认 apt-get
#   SUDO                 提权命令，默认 sudo；设成空字符串则直接调用
#   CI_APT_ATTEMPTS      总尝试次数，默认 3
#   CI_APT_RETRY_DELAY   两次尝试之间睡多少秒，默认 15
set -euo pipefail

if (( $# == 0 )); then
  echo "usage: $0 <package>..." >&2
  exit 2
fi

APT_GET=${APT_GET:-apt-get}
SUDO=${SUDO-sudo}
ATTEMPTS=${CI_APT_ATTEMPTS:-3}
RETRY_DELAY=${CI_APT_RETRY_DELAY:-15}
PACKAGES=("$@")

export DEBIAN_FRONTEND=noninteractive

run_apt() {
  if [[ -n "$SUDO" ]]; then
    "$SUDO" "$APT_GET" "$@"
  else
    "$APT_GET" "$@"
  fi
}

# 一次完整尝试：update 之后 install。update 失败不吞掉，它同样算这次尝试失败
# ——本次事故里失败的是 install，但两个阶段都该让错误可见（issue #91 第 3 条）。
attempt() {
  local n=$1
  if ! run_apt update; then
    echo "apt-get update failed (attempt ${n}/${ATTEMPTS})" >&2
    return 1
  fi
  # 刻意不加 --fix-missing：那会让 apt 跳过下不下来的包然后以 0 退出，
  # 依赖其实没装上，错误被推到后面的构建步骤，比现在更难查。
  if ! run_apt install -y "${PACKAGES[@]}"; then
    echo "apt-get install failed (attempt ${n}/${ATTEMPTS})" >&2
    return 1
  fi
}

for (( i = 1; i <= ATTEMPTS; i++ )); do
  if attempt "$i"; then
    exit 0
  fi
  if (( i == ATTEMPTS )); then
    echo "::error title=Dependency install failed::Installing ${PACKAGES[*]} failed after ${ATTEMPTS} attempts. This is a dependency install problem on the runner (often DNS, see issue #91), not a test failure."
    exit 1
  fi
  echo "::warning title=Dependency install retried::Installing ${PACKAGES[*]} failed on attempt ${i}/${ATTEMPTS}; retrying in ${RETRY_DELAY}s."
  sleep "$RETRY_DELAY"
done
