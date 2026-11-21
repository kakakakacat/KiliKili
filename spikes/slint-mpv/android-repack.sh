#!/usr/bin/env bash
# 给 cargo-apk 打出来的包换一份 AndroidManifest.xml，然后重新对齐并签名。
#
# ## 为什么要有这个脚本
#
# PiP 那一项要求 manifest 里有 `android:supportsPictureInPicture="true"`，
# 没有它 `enterPictureInPictureMode` 直接抛 IllegalStateException——这是系统
# 的硬门槛，不是调用姿势问题。而 cargo-apk 0.10 用的 ndk-build 0.10 的
# manifest 模型（`ndk-build/src/manifest.rs` 的 `Activity`）里**根本没有这个
# 字段**，Cargo.toml 的 `[package.metadata.android...]` 再怎么写也生成不出来。
#
# 真实产品那边没这个问题：Phase 2 的 Android 包是 Gradle 打的，manifest 自己
# 写。所以这里不值得去 fork cargo-apk，打完包换掉 manifest 就够了。
#
# ## 怎么换
#
# APK 里的 AndroidManifest.xml 是**二进制 XML**，不能直接塞文本进去。所以：
#
#   1. 拿 cargo-apk 生成的那份文本 manifest，插进缺的属性；
#   2. `aapt2 link` 单独编译一个只有 manifest 的 apk，从里面取编译好的那份；
#   3. 把它替换进 cargo-apk 的 unaligned 包；
#   4. `zipalign` + `apksigner`（签名必须在最后：改包内容会让旧签名失效）。
#
# ## 用法
#
#   ./android-repack.sh              # 正常包：声明 configChanges，旋转不重建
#   ./android-repack.sh --recreate   # 对照包：去掉 configChanges，旋转会重建
#
# `--recreate` 那个变体是 #9「surface 生命周期」要的另一半：不声明
# configChanges 时系统会重建 activity，`android_main` 整个重跑。两种行为都要
# 测过，才敢说 Phase 2 该选哪种。
set -euo pipefail

cd "$(dirname "$0")"

RECREATE=0
if [[ "${1:-}" == "--recreate" ]]; then
    RECREATE=1
fi

: "${ANDROID_HOME:=$HOME/AppData/Local/Android/Sdk}"
BUILD_TOOLS="${BUILD_TOOLS:-$ANDROID_HOME/build-tools/37.0.0}"
PLATFORM_JAR="${PLATFORM_JAR:-$ANDROID_HOME/platforms/android-37.0/android.jar}"
APK_DIR="target/release/apk"
SRC_MANIFEST="$APK_DIR/AndroidManifest.xml"
UNALIGNED="$APK_DIR/slint_mpv_spike-unaligned.apk"

for needed in "$SRC_MANIFEST" "$UNALIGNED" "$PLATFORM_JAR"; do
    if [[ ! -f "$needed" ]]; then
        echo "找不到 $needed —— 先跑 cargo apk build --release --lib" >&2
        exit 1
    fi
done

WORK="$APK_DIR/repack"
rm -rf "$WORK"
mkdir -p "$WORK"

OUT="$APK_DIR/spike-pip.apk"
if [[ "$RECREATE" == "1" ]]; then
    OUT="$APK_DIR/spike-pip-recreate.apk"
fi

python - "$SRC_MANIFEST" "$WORK/AndroidManifest.xml" "$RECREATE" <<'PY'
import re
import sys

src, dst, recreate = sys.argv[1], sys.argv[2], sys.argv[3] == "1"
xml = open(src, encoding="utf-8").read()

# supportsPictureInPicture 是这个脚本存在的理由，见文件头。
if "supportsPictureInPicture" not in xml:
    xml = xml.replace("<activity ", '<activity android:supportsPictureInPicture="true" ', 1)

if recreate:
    # 对照包：把 configChanges 整条拿掉，让系统按默认行为重建 activity。
    xml = re.sub(r'\sandroid:configChanges="[^"]*"', "", xml, count=1)

open(dst, "w", encoding="utf-8", newline="\n").write(xml)
print("manifest 已改写 ->", dst)
PY

"$BUILD_TOOLS/aapt2" link \
    -o "$WORK/manifest-only.apk" \
    --manifest "$WORK/AndroidManifest.xml" \
    -I "$PLATFORM_JAR"

python - "$WORK" "$UNALIGNED" "$WORK/patched.apk" <<'PY'
import shutil
import sys
import zipfile

work, unaligned, out = sys.argv[1], sys.argv[2], sys.argv[3]

with zipfile.ZipFile(f"{work}/manifest-only.apk") as z:
    compiled = z.read("AndroidManifest.xml")

# 整包重写一遍：zipfile 不支持原地替换条目。其余条目逐个照搬，压缩方式保持
# 原样——libmpv.so 那种 14MB 的东西没必要重压一次。
with zipfile.ZipFile(unaligned) as src, zipfile.ZipFile(out, "w") as dst:
    for item in src.infolist():
        data = compiled if item.filename == "AndroidManifest.xml" else src.read(item.filename)
        info = zipfile.ZipInfo(item.filename, date_time=item.date_time)
        info.compress_type = item.compress_type
        info.external_attr = item.external_attr
        dst.writestr(info, data)
print("已换入编译好的 manifest ->", out)
PY

rm -f "$OUT"
"$BUILD_TOOLS/zipalign" -p -f 4 "$WORK/patched.apk" "$OUT"
# apksigner 在 Windows 的 SDK 里是 .bat（要 java），别的平台是无后缀的脚本。
APKSIGNER="$BUILD_TOOLS/apksigner"
if [[ ! -f "$APKSIGNER" ]]; then
    APKSIGNER="$BUILD_TOOLS/apksigner.bat"
fi
if [[ ! -f "$APKSIGNER" ]]; then
    echo "找不到 apksigner（看过 $BUILD_TOOLS）" >&2
    exit 1
fi
# apksigner 自己要 java：JAVA_HOME 没设时它报的那句话很难懂，先替它说清楚。
if [[ -z "${JAVA_HOME:-}" ]] && ! command -v java >/dev/null 2>&1; then
    echo "JAVA_HOME 没设，apksigner 跑不起来（cargo apk build 也要它）" >&2
    exit 1
fi
"$APKSIGNER" sign \
    --ks debug.keystore --ks-pass pass:android --key-pass pass:android \
    "$OUT"

echo
echo "打好了：$OUT"
"$BUILD_TOOLS/aapt2" dump xmltree "$OUT" --file AndroidManifest.xml |
    grep -E "supportsPictureInPicture|configChanges|resizeableActivity" || true
