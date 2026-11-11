#!/usr/bin/env bash
# 汇总 Rust 与 Flutter tests 两条工作流对同一个 commit 的结论。
#
# 为什么需要这个脚本：ruleset「main regression gate」要求一个名为
# `regression gate` 的状态检查，而 GitHub Actions 的 `needs:` 只能在
# **同一个工作流内**串联 job。要把两条工作流汇成一个检查，只能等它们各自
# 的结论，于是有了这里的轮询。
#
# 依赖只有 `gh`（它自带 jq 表达式求值，不需要系统装 jq）与 coreutils。
#
# ## 三件容易弄错的事
#
# 1. **「没有 run」不等于「还没排上」。** 那两条工作流都有 paths 过滤：
#    只改 docs 时它们根本不会产生 run。本脚本把「settle 期内始终没有 run」
#    判为未触发并放行；settle 期之前不下这个结论，否则会在工作流还没排上
#    的那几秒里误判为全部跳过，把门变成摆设。
#
# 2. **要按 head_sha 过滤。** 不然会读到同一分支上更早那次推送的结论，
#    代码已经变了而门是绿的——这正是这道门要防的事情本身。
#
# 3. **跳过算通过，取消不算。** 工作流被 paths 过滤或 `if:` 跳过是预期内
#    的；被取消（新推送抢占、超时）则意味着没有结论，不能当成通过。
set -euo pipefail

: "${GH_TOKEN:?需要 GH_TOKEN}"
: "${HEAD_SHA:?需要 HEAD_SHA}"
: "${GITHUB_REPOSITORY:?需要 GITHUB_REPOSITORY}"

# 必须等待的工作流，名字取自各自 YAML 的 `name:`。
WATCHED=("Rust" "Flutter tests")

# 判定「未触发」前至少观察这么久。GitHub 通常在数秒内就把 run 排上，
# 留足余量。
SETTLE_SECONDS=${SETTLE_SECONDS:-120}
# 总超时。两条工作流里最慢的是 bridge（约 5 分钟），留出排队时间。
TIMEOUT_SECONDS=${TIMEOUT_SECONDS:-2400}
POLL_SECONDS=${POLL_SECONDS:-15}

# 每行一条 run：`name <TAB> status <TAB> conclusion <TAB> url`，按 id 升序。
# 升序是为了下面「同名取最后一条」能拿到最新的那次重跑。
runs_tsv() {
    gh api \
        "repos/${GITHUB_REPOSITORY}/actions/runs?head_sha=${HEAD_SHA}&per_page=100" \
        --jq '.workflow_runs | sort_by(.id) | .[]
              | [.name, .status, (.conclusion // ""), .html_url] | @tsv'
}

started=$(date +%s)

while :; do
    now=$(date +%s)
    elapsed=$((now - started))

    all=$(runs_tsv)
    pending=()
    failed=()
    summary=()

    for name in "${WATCHED[@]}"; do
        # 同一个 sha 可能有多次 run（重跑），取最后一条。
        line=$(printf '%s\n' "$all" | awk -F'\t' -v n="$name" '$1 == n { last = $0 } END { print last }')

        if [ -z "$line" ]; then
            if [ "$elapsed" -lt "$SETTLE_SECONDS" ]; then
                pending+=("$name（还没出现 run）")
            else
                summary+=("$name: 未触发（paths 过滤），放行")
            fi
            continue
        fi

        IFS=$'\t' read -r _ status conclusion url <<<"$line"

        if [ "$status" != "completed" ]; then
            pending+=("$name（$status）")
            continue
        fi

        case "$conclusion" in
            success | skipped)
                summary+=("$name: $conclusion")
                ;;
            *)
                failed+=("$name: ${conclusion:-无结论} — $url")
                ;;
        esac
    done

    if [ ${#failed[@]} -gt 0 ]; then
        echo "回归门不通过："
        printf '  %s\n' "${failed[@]}"
        exit 1
    fi

    if [ ${#pending[@]} -eq 0 ] && [ "$elapsed" -ge "$SETTLE_SECONDS" ]; then
        echo "回归门通过："
        printf '  %s\n' "${summary[@]}"
        exit 0
    fi

    if [ "$elapsed" -ge "$TIMEOUT_SECONDS" ]; then
        echo "回归门超时（${TIMEOUT_SECONDS}s），仍在等待："
        printf '  %s\n' "${pending[@]}"
        exit 1
    fi

    if [ ${#pending[@]} -gt 0 ]; then
        echo "[${elapsed}s] 等待：${pending[*]}"
    else
        echo "[${elapsed}s] 结论齐了，等 settle 期（${SETTLE_SECONDS}s）走完"
    fi
    sleep "$POLL_SECONDS"
done
