#!/usr/bin/env bash
# tool/ci-apt-install.sh 的验收（issue #91）：用一个假的 apt-get 人为让它失败，
# 验「重试后能成功」「重试仍失败时明确报错退出而不是挂着」。
#
# 不需要 apt，也不需要 Linux——Git Bash 上同样跑得过。CI 上由
# .github/workflows/flutter-test.yml 的 test job 在装依赖之前跑一次（约 1 秒），
# 免得这个脚本烂掉没人发现。
set -euo pipefail

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
target="$script_dir/ci-apt-install.sh"
workdir=$(mktemp -d)
trap 'rm -rf "$workdir"' EXIT

failures=0

# 造一个假 apt-get：按 FAKE_FAIL_* 指定的次数失败，之后成功。
# 调用次数记在 $workdir/<stage>.count 里。
make_fake_apt() {
  local fail_update=$1 fail_install=$2
  cat > "$workdir/apt-get" <<FAKE
#!/usr/bin/env bash
stage=\$1
count_file="$workdir/\$stage.count"
n=0
[[ -f "\$count_file" ]] && n=\$(cat "\$count_file")
n=\$(( n + 1 ))
echo "\$n" > "\$count_file"
case "\$stage" in
  update)  limit=$fail_update ;;
  install) limit=$fail_install ;;
  *)       limit=0 ;;
esac
if (( n <= limit )); then
  echo "E: Temporary failure resolving 'security.ubuntu.com'" >&2
  exit 100
fi
echo "fake apt-get \$*"
FAKE
  chmod +x "$workdir/apt-get"
  rm -f "$workdir"/*.count
}

run_target() {
  APT_GET="$workdir/apt-get" SUDO= CI_APT_ATTEMPTS=3 CI_APT_RETRY_DELAY=0 \
    bash "$target" libgtk-3-dev libx11-dev libxi-dev
}

count_of() {
  local stage=$1
  [[ -f "$workdir/$stage.count" ]] && cat "$workdir/$stage.count" || echo 0
}

check() {
  local name=$1 expected=$2 actual=$3
  if [[ "$expected" == "$actual" ]]; then
    echo "ok   - $name"
  else
    echo "FAIL - $name: expected [$expected], got [$actual]" >&2
    failures=$(( failures + 1 ))
  fi
}

# 1. 一次就成功：只调一次 install，退出 0。
make_fake_apt 0 0
out=$(run_target 2>&1) && rc=0 || rc=$?
check "一次成功时退出码为 0" 0 "$rc"
check "一次成功时只装一次" 1 "$(count_of install)"

# 2. install 失败一次后成功：这是事故里的形状，必须自愈。
make_fake_apt 0 1
out=$(run_target 2>&1) && rc=0 || rc=$?
check "install 失败一次后退出码为 0" 0 "$rc"
check "install 失败一次后共尝试两次" 2 "$(count_of install)"
case "$out" in
  *"::warning title=Dependency install retried"*) echo "ok   - 重试时打了 warning 注解" ;;
  *) echo "FAIL - 重试时没有 warning 注解" >&2; failures=$(( failures + 1 )) ;;
esac

# 3. update 失败一次后成功：update 的失败不能被吞掉，也要走重试。
make_fake_apt 1 0
out=$(run_target 2>&1) && rc=0 || rc=$?
check "update 失败一次后退出码为 0" 0 "$rc"
check "update 失败一次后共跑两次 update" 2 "$(count_of update)"

# 4. 一直失败：在尝试次数内明确报错退出，不是挂满一小时。
make_fake_apt 0 99
out=$(run_target 2>&1) && rc=0 || rc=$?
check "一直失败时退出码为 1" 1 "$rc"
check "一直失败时尝试满 3 次就停" 3 "$(count_of install)"
case "$out" in
  *"::error title=Dependency install failed"*) echo "ok   - 最终失败时注解写明是装依赖" ;;
  *) echo "FAIL - 最终失败时没有写明是装依赖的 error 注解" >&2; failures=$(( failures + 1 )) ;;
esac

# 5. 不传包名时直接报用法错误，不去动 apt。
make_fake_apt 0 0
out=$(APT_GET="$workdir/apt-get" SUDO= bash "$target" 2>&1) && rc=0 || rc=$?
check "不传包名时退出码为 2" 2 "$rc"
check "不传包名时不调用 apt" 0 "$(count_of install)"

if (( failures > 0 )); then
  echo "$failures check(s) failed" >&2
  exit 1
fi
echo "all checks passed"
