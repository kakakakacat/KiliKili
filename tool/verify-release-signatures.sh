#!/usr/bin/env bash
# 校验一次 Linux 发布产物的来源与完整性。
#
# 用法：
#   tool/verify-release-signatures.sh <产物目录> [公钥文件]
#
# 产物目录是从 GitHub Release 下载下来的那一堆文件所在的目录，至少要有
# SHA256SUMS 与 SHA256SUMS.asc。公钥默认取仓库里钉住的
# assets/linux/signing-key.asc，也可以指向自己另外拿到的一份。
#
# 做了什么：
#   1. 把公钥导进一个临时钥匙环（**不碰**使用者自己的 ~/.gnupg）；
#   2. 用它验 SHA256SUMS.asc，再用 SHA256SUMS 核对每个包的哈希；
#   3. 对目录里存在的其余 .asc 逐个验一遍。
#
# 刻意没做的事：不验 rpm / deb / AppImage 的**包内**签名。那三种各要一套
# 本机工具与配置（rpm 钥匙环、/etc/debsig 策略、AppImage 运行时），放进来
# 会让这个脚本在大多数机器上跑不动，而它们证明的来源与这里这条链完全相同。
# 包内签名的用途是让包管理器在安装时自己判断，不是给这个脚本用的。

set -euo pipefail

DIR="${1:-}"
KEY="${2:-}"

if [ -z "$DIR" ]; then
  printf '用法: %s <产物目录> [公钥文件]\n' "$0" >&2
  exit 2
fi

if [ ! -d "$DIR" ]; then
  printf '不是目录: %s\n' "$DIR" >&2
  exit 2
fi

if [ -z "$KEY" ]; then
  KEY="$(cd "$(dirname "$0")/.." && pwd)/assets/linux/signing-key.asc"
fi

if [ ! -f "$KEY" ]; then
  printf '找不到公钥: %s\n' "$KEY" >&2
  printf '仓库里还没有钉住的公钥时，请从 Release 页下载 KiliKili-linux-signing-key.asc，\n' >&2
  printf '并与 docs/release/SIGNING.md 里记录的指纹逐字核对后作为第二个参数传进来。\n' >&2
  exit 2
fi

command -v gpg > /dev/null || { printf '需要 gpg\n' >&2; exit 2; }
command -v sha256sum > /dev/null || { printf '需要 sha256sum\n' >&2; exit 2; }

GNUPGHOME="$(mktemp -d)"
export GNUPGHOME
chmod 700 "$GNUPGHOME"
cleanup() { rm -rf "$GNUPGHOME"; }
trap cleanup EXIT

gpg --batch --quiet --import "$KEY"
FPR="$(gpg --batch --with-colons --list-keys | awk -F: '/^fpr:/ { print $10; exit }')"
printf '公钥指纹: %s\n' "$FPR"
printf '（这串必须与 docs/release/SIGNING.md 记录的一致；只核对脚本输出等于没核对）\n\n'

cd "$DIR"

if [ ! -f SHA256SUMS ] || [ ! -f SHA256SUMS.asc ]; then
  printf '目录里没有 SHA256SUMS / SHA256SUMS.asc：这次发布要么没签名，要么没下全。\n' >&2
  exit 1
fi

printf '验 SHA256SUMS 的签名...\n'
gpg --batch --verify SHA256SUMS.asc SHA256SUMS

printf '\n核对哈希...\n'
# --ignore-missing: 允许只下载其中一两个包就来校验。
sha256sum --check --ignore-missing SHA256SUMS

printf '\n验其余分离签名...\n'
found=0
for sig in *.asc; do
  [ "$sig" = "SHA256SUMS.asc" ] && continue
  target="${sig%.asc}"
  # 公钥本身（KiliKili-linux-signing-key.asc）不是签名文件，跳过。
  [ -f "$target" ] || continue
  gpg --batch --verify "$sig" "$target"
  found=$((found + 1))
done
if [ "$found" -eq 0 ]; then
  printf '（目录里没有包的分离签名，只校验了 SHA256SUMS）\n'
fi

printf '\n全部通过。\n'
