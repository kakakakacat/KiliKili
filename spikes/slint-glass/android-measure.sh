#!/usr/bin/env bash
# 量 GPU 占用与显存（issue #130 关口 5）。
#
# 为什么不量帧率：**帧率在这台设备上量不出代价**。面板钉在 60 或 120Hz，
# 六组对照读数全一样——这不是「不贵」，是 vsync 把差异吃掉了。第二轮记录
# §3.1 有完整经过。这里改读 Adreno 的 `gpubusy`，它和帧率撞不撞 vsync 无关。
#
# **用它之前先读第二轮记录 §3.4**：60Hz 下量到的占用率是**错的参照**，
# GPU 每帧大段空闲、降频运行，占用率里混进了「时钟低」这个因素，那一组
# 数字非单调（σ=60 比 σ=10 还便宜）。所以这个脚本**强制把面板顶到 120Hz**
# （app 自己用 ANativeWindow_setFrameRate 报 + TICK=4 让 app 真的产得出
# 120 帧），负载更满、DVFS 的残留更小。时钟本身仍然读不到
# （`gpuclk` permission denied），这条不确定性至今没关掉。
#
# 用法：
#   ./android-measure.sh                  # 默认跑四档对照
#   ./android-measure.sh --window 20      # 每档采样 20 秒
set -u

PKG=io.github.kakacat.kilikili.spike.glass
ACT="$PKG/android.app.NativeActivity"
FILES="/sdcard/Android/data/$PKG/files"
OUT="target/android-measure"
WINDOW=12
ADB="${ADB:-adb}"
export MSYS_NO_PATHCONV=1

while [ $# -gt 0 ]; do
    case "$1" in
        --window) WINDOW="$2"; shift 2 ;;
        --out)    OUT="$2"; shift 2 ;;
        *) echo "不认识的参数: $1" >&2; exit 2 ;;
    esac
done

mkdir -p "$OUT"
REPORT="$OUT/report.txt"
: > "$REPORT"

say() { printf '\n===== %s =====\n' "$*" | tee -a "$REPORT"; }
note() { printf '%s\n' "$*" | tee -a "$REPORT"; }

if ! "$ADB" shell true >/dev/null 2>&1; then
    echo "!! adb 连不上设备" >&2
    exit 1
fi

# 一档 = 一组 glass.conf。七档的设计（前四档是 Phase 1 的，后三档 Phase 2 加）：
#   native      只有原生 backdrop-blur（GL 探针关掉）——基准
#   gl-p0       原生 + GL 覆盖（单级模糊+折射+Fresnel+高光）——P0 的代价
#   gl-base     原生 + GL 覆盖但不模糊（与 Flutter 同式）——拆出「模糊」那一笔
#   noblur      连原生 backdrop-blur 都关掉——地板，判上面几个的分母
#   gl-phase2   在 gl-p0 上换成三级多尺度模糊 + 动态高光——多尺度那一笔
#   gl-overblur 原生负责模糊、自绘只负责折射与高光——ADR-016 决策四那个组合
#   gl-merge    两个形状 + 融合——并集外接框变大那一笔
#
# **档与档之间只差一件事**，否则减出来的差值说明不了任何东西。gl-phase2 与
# gl-p0 只差级数与高光，gl-overblur 与 gl-base 只差「原生那条开不开模糊」。
run_case() {
    local name="$1" conf="$2"
    say "$name"
    note "档位: $conf"

    "$ADB" shell am force-stop "$PKG"
    printf '%s\n' "$conf" | tr ' ' '\n' > "$OUT/glass.conf"
    "$ADB" push "$OUT/glass.conf" "$FILES/glass.conf" >/dev/null
    "$ADB" shell am start -n "$ACT" >/dev/null

    # 等窗口真的起来。帧率仲裁要几秒才稳，所以这里等得比 drive 脚本久。
    sleep 8

    # 面板必须和数字同时取：同一个占用率在 60Hz 和 120Hz 下含义完全不同。
    local mode
    mode=$("$ADB" shell dumpsys SurfaceFlinger 2>/dev/null | grep -m1 activeMode | tr -d '\r')
    note "面板: $mode"

    # gpubusy 读一次清零，所以第一次读是丢弃的（清掉等待期的累积），
    # 第二次读才是这个采样窗口里的。
    "$ADB" shell cat /sys/class/kgsl/kgsl-3d0/gpubusy >/dev/null 2>&1
    sleep "$WINDOW"
    local busy
    busy=$("$ADB" shell cat /sys/class/kgsl/kgsl-3d0/gpubusy 2>/dev/null | tr -d '\r')
    note "gpubusy(忙 总): $busy"
    echo "$busy" | awk -v w="$WINDOW" '
        NF >= 2 && $2 > 0 {
            pct = 100.0 * $1 / $2
            printf "GPU 忙: %.1f%%\n", pct
            # 120Hz 下每帧预算 8.33ms。折算只是换个单位，**不是**说这些
            # 时间都花在玻璃上——它是整帧的 GPU 时间。
            printf "折算每帧 GPU 时间（8.33ms 预算）: %.2f ms\n", pct / 100.0 * 8.33
        }' | tee -a "$REPORT"

    # 显存。Android 上 GPU 内存记在 meminfo 的 Graphics / GL mtrack 两行里，
    # 它包含 surface、纹理与驱动自己的分配，**不是**只有我们那几张纹理。
    # 对照着看才有意义，绝对值不要单独引用。
    note "--- meminfo ---"
    "$ADB" shell dumpsys meminfo "$PKG" 2>/dev/null \
        | grep -E "Graphics|GL mtrack|EGL mtrack|TOTAL PSS|Native Heap" | tr -d '\r' | tee -a "$REPORT"

    "$ADB" shell am force-stop "$PKG"
}

# 七档都把面板顶到 120Hz（FPS=120 + TICK=4），理由见文件头。
#
# 光源角一律钉死（LIGHT=180）：自转本身不要钱（一次 cos/sin 在 CPU 侧），
# 但钉住之后每一档画的是同一帧，省掉「这档碰巧光斑在别处」这种解释空间。
run_case "native（只有原生 backdrop-blur）"   "COUNT=1 GL=0 FPS=120 TICK=4"
run_case "gl-p0（原生 + GL 单级 P0）"         "COUNT=1 GL=1 MATERIAL=p0 LIGHT=180 FPS=120 TICK=4"
run_case "gl-phase2（原生 + GL 三级多尺度）"  "COUNT=1 GL=1 MATERIAL=phase2 LIGHT=180 FPS=120 TICK=4"
run_case "gl-overblur（原生模糊 + 自绘折射）" "COUNT=1 GL=1 LAYOUT=over-blur LIGHT=180 FPS=120 TICK=4"
run_case "gl-merge（两个形状 + 融合）"        "COUNT=1 GL=1 LAYOUT=merge MERGEGAP=12 LIGHT=180 FPS=120 TICK=4"
run_case "gl-base（原生 + GL 无模糊）"        "COUNT=1 GL=1 MATERIAL=baseline LIGHT=180 FPS=120 TICK=4"
run_case "noblur（连原生模糊都关）"            "COUNT=1 GL=0 BLUR=0 FPS=120 TICK=4"

say "完"
note "报告: $REPORT"
note ""
note "读数之前先确认面板那一行都是 120Hz。只要有一档掉回 60，那一档的"
note "占用率就和别档不可比——见文件头关于 DVFS 的那段。"
