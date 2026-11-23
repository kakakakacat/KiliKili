#!/usr/bin/env python3
"""为 Android CI 构建配置签名，并核对最终 APK 的证书。"""

import argparse
import base64
import binascii
import hashlib
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile


ROOT = Path(__file__).resolve().parents[1]
KEYSTORE = ROOT / "android/app/key.jks"
PROPERTIES = ROOT / "android/key.properties"


def required(name: str) -> str:
    value = os.environ.get(name, "")
    if not value:
        raise ValueError(f"缺少 {name}；本次 Android 构建不能使用临时 debug 签名")
    return value


def fingerprint() -> str:
    value = required("ANDROID_SIGNING_CERT_SHA256").replace(":", "").lower()
    if not re.fullmatch(r"[0-9a-f]{64}", value):
        raise ValueError("ANDROID_SIGNING_CERT_SHA256 必须是 64 位十六进制 SHA-256")
    return value


def property_value(value: str) -> str:
    if "\n" in value or "\r" in value:
        raise ValueError("Android 签名属性不能包含换行")
    escaped = "".join("\\" + char if char in "\\:=#!" else char for char in value)
    return "\\" + escaped if escaped.startswith(" ") else escaped


def configure() -> None:
    encoded = required("ANDROID_SIGNING_KEYSTORE_BASE64")
    store_password = required("ANDROID_SIGNING_STORE_PASSWORD")
    alias = required("ANDROID_SIGNING_KEY_ALIAS")
    key_password = required("ANDROID_SIGNING_KEY_PASSWORD")
    expected = fingerprint()

    if KEYSTORE.exists() or PROPERTIES.exists():
        raise ValueError("本地已有 Android 签名文件；拒绝覆盖")
    try:
        data = base64.b64decode("".join(encoded.split()), validate=True)
    except (binascii.Error, ValueError) as error:
        raise ValueError("Android keystore 不是有效的 base64") from error
    if not data:
        raise ValueError("Android keystore 为空")

    # 先核对公开证书指纹，再把密钥写到 Gradle 会读取的位置。
    with tempfile.TemporaryDirectory() as directory:
        candidate = Path(directory) / "candidate.jks"
        candidate.write_bytes(data)
        keytool = subprocess.run(
            ["keytool", "-exportcert", "-keystore", str(candidate), "-alias", alias,
             "-storepass:env", "ANDROID_SIGNING_STORE_PASSWORD"],
            capture_output=True, check=False,
        )
    if keytool.returncode:
        raise ValueError("keytool 无法读取签名密钥或别名；检查 secret 配置")
    actual = hashlib.sha256(keytool.stdout).hexdigest()
    if actual != expected:
        raise ValueError(f"Android 签名证书指纹不匹配：预期 {expected}，实际 {actual}")

    KEYSTORE.write_bytes(data)
    KEYSTORE.chmod(0o600)
    try:
        PROPERTIES.write_text(
            "storeFile=key.jks\n"
            f"storePassword={property_value(store_password)}\n"
            f"keyAlias={property_value(alias)}\n"
            f"keyPassword={property_value(key_password)}\n",
            encoding="utf-8",
        )
        PROPERTIES.chmod(0o600)
    except Exception:
        KEYSTORE.unlink(missing_ok=True)
        raise
    print(f"Android 签名证书 SHA-256: {actual}")


def verify(apks: list[str]) -> None:
    expected = fingerprint()
    expected_package = required("ANDROID_EXPECTED_PACKAGE")
    sdk = os.environ.get("ANDROID_HOME") or os.environ.get("ANDROID_SDK_ROOT")
    if not sdk:
        raise ValueError("找不到 Android SDK")
    tools = sorted(
        Path(sdk).glob("build-tools/*/apksigner"),
        key=lambda path: tuple(int(part) for part in re.findall(r"\d+", path.parent.name)),
    )
    if not tools:
        raise ValueError("找不到 apksigner")
    aapt = tools[-1].parent / "aapt"
    if not aapt.exists():
        raise ValueError("找不到 aapt")
    if not apks:
        raise ValueError("没有待核对的 APK")
    for apk in apks:
        result = subprocess.run(
            [str(tools[-1]), "verify", "--print-certs", apk],
            capture_output=True, text=True, check=False,
        )
        if result.returncode:
            raise ValueError(f"APK 签名无效：{apk}: {result.stderr.strip()}")
        digests = [value.lower() for value in re.findall(
            r"Signer #\d+ certificate SHA-256 digest: ([0-9a-fA-F]{64})",
            result.stdout,
        )]
        if digests != [expected]:
            raise ValueError(f"APK 证书与预期身份不符：{apk}，实际 {digests}")
        package_result = subprocess.run(
            [str(aapt), "dump", "badging", apk],
            capture_output=True, text=True, check=False,
        )
        package = re.search(r"^package: name='([^']+)'", package_result.stdout, re.MULTILINE)
        if package_result.returncode or not package or package.group(1) != expected_package:
            raise ValueError(f"APK 应用 ID 与预期 {expected_package} 不符：{apk}")
        print(f"APK 签名已核对：{apk}，应用 ID {expected_package}，SHA-256 {expected}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("configure", "verify"))
    parser.add_argument("apks", nargs="*")
    args = parser.parse_args()
    try:
        if args.action == "configure":
            if args.apks:
                parser.error("configure 不接受 APK 路径")
            configure()
        else:
            verify(args.apks)
    except (OSError, ValueError) as error:
        print(f"Android 签名检查失败：{error}", file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()
