"""固定 apksigner 新旧输出格式，避免有效 APK 被指纹解析误拒。"""

import contextlib
import importlib.util
import io
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch


SPEC = importlib.util.spec_from_file_location(
    "android_signing", Path(__file__).with_name("android-signing.py")
)
SIGNING = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(SIGNING)
DIGEST = "a" * 64
PACKAGE = "io.github.kakacat.kilikili.dev"


class ApkIdentityTests(unittest.TestCase):
    def verify_output(self, output, package=PACKAGE, returncode=0):
        with tempfile.TemporaryDirectory() as directory:
            tools = Path(directory) / "build-tools/37.0.0"
            tools.mkdir(parents=True)
            (tools / "apksigner").touch()
            (tools / "aapt").touch()
            results = [
                subprocess.CompletedProcess([], returncode, output, ""),
                subprocess.CompletedProcess([], 0, f"package: name='{package}'\n", ""),
            ]
            with patch.dict(os.environ, {
                "ANDROID_HOME": directory,
                "ANDROID_SIGNING_CERT_SHA256": DIGEST,
                "ANDROID_EXPECTED_PACKAGE": PACKAGE,
            }), patch.object(SIGNING.subprocess, "run", side_effect=results), \
                    contextlib.redirect_stdout(io.StringIO()):
                SIGNING.verify(["fixture.apk"])

    def test_accepts_legacy_signer_output(self):
        self.verify_output(f"Signer #1 certificate SHA-256 digest: {DIGEST}\n")

    def test_accepts_sdk37_signer_output(self):
        for prefix in ["V2 Signer:", "V3.0 Signer:"]:
            with self.subTest(prefix=prefix):
                self.verify_output(f"{prefix} certificate SHA-256 digest: {DIGEST}\n")

    def test_rejects_wrong_certificate_for_both_formats(self):
        for prefix in ["Signer #1", "V3.0 Signer:"]:
            with self.subTest(prefix=prefix), self.assertRaises(ValueError):
                self.verify_output(f"{prefix} certificate SHA-256 digest: {'b' * 64}\n")

    def test_rejects_missing_certificate(self):
        with self.assertRaises(ValueError):
            self.verify_output("Verifies\n")

    def test_rejects_multiple_signers(self):
        with self.assertRaises(ValueError):
            self.verify_output("".join(
                f"Signer #{index} certificate SHA-256 digest: {DIGEST}\n"
                for index in [1, 2]
            ))

    def test_rejects_wrong_application_id(self):
        with self.assertRaises(ValueError):
            self.verify_output(
                f"V3.0 Signer: certificate SHA-256 digest: {DIGEST}\n",
                package=PACKAGE + ".wrong",
            )

    def test_rejects_failed_signature_verification(self):
        with self.assertRaises(ValueError):
            self.verify_output(
                f"Signer #1 certificate SHA-256 digest: {DIGEST}\n", returncode=1
            )


if __name__ == "__main__":
    unittest.main()
