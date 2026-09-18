#!/usr/bin/env python3
"""Hermetic contracts for Rust-only macOS bundle packaging and checking."""

from __future__ import annotations

import json
import os
from pathlib import Path
import plistlib
import shutil
import subprocess
import sys
import tempfile
import unittest

REPOSITORY_ROOT = Path(__file__).resolve().parent.parent
BUILD_DIR = REPOSITORY_ROOT / "Build"


class RustBundleContracts(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(prefix="hp2-rust-bundle-")
        self.root = Path(self.temporary.name)
        self.codesign = self.root / "codesign-stub.sh"
        self.codesign.write_text(
            "#!/bin/sh\n"
            'if [ "$1" = "-dv" ]; then echo "Signature=adhoc-test" >&2; fi\n'
            "exit 0\n"
        )
        self.codesign.chmod(0o755)
        self.environment = dict(os.environ, HP2_CODESIGN_BIN=str(self.codesign))
        self.release_binary = self.root / "target" / "release" / "hp2rs"
        self.release_binary.parent.mkdir(parents=True)
        self.release_binary.write_bytes(b"\xcf\xfa\xed\xfe" + b"\0" * 16)
        self.release_binary.chmod(0o755)
        self.bundle = self.root / "dist" / "macos-arm64-rs" / "HarryPotter2.app"

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def test_skip_build_recreates_rust_bundle_without_cxx_bundle(self) -> None:
        stale = self.bundle / "Contents"
        (stale / "MacOS").mkdir(parents=True)
        (stale / "MacOS" / "obsolete").write_text("stale")
        (stale / "Frameworks").mkdir()
        (stale / "Frameworks" / "libopenal.dylib").write_text("stale")
        (stale / "Info.plist").write_bytes(plistlib.dumps({"stale": True}))
        cxx_bundle = self.root / "dist" / "macos-arm64" / "HarryPotter2.app"
        self.assertFalse(cxx_bundle.exists())

        packaged = subprocess.run(
            [
                sys.executable,
                str(BUILD_DIR / "package_rust_app.py"),
                "--repo-root",
                str(self.root),
                "--skip-build",
            ],
            capture_output=True,
            text=True,
            check=False,
            env=self.environment,
        )

        self.assertEqual(packaged.returncode, 0, packaged.stderr)
        self.assertFalse(cxx_bundle.exists())
        self.assertFalse((self.bundle / "Contents" / "MacOS" / "obsolete").exists())
        self.assertFalse((self.bundle / "Contents" / "Frameworks").exists())
        info = plistlib.loads((self.bundle / "Contents" / "Info.plist").read_bytes())
        self.assertEqual(info["CFBundleIdentifier"], "com.hp2.native")
        self.assertEqual(info["CFBundleShortVersionString"], "0.1.0")
        self.assertEqual(info["CFBundleExecutable"], "HarryPotter2")
        self.assertEqual(
            (self.bundle / "Contents" / "MacOS" / "HarryPotter2").read_bytes()[:4],
            b"\xcf\xfa\xed\xfe",
        )

        checked = subprocess.run(
            [
                sys.executable,
                str(BUILD_DIR / "check_bundle_rs.py"),
                "--repo-root",
                str(self.root),
            ],
            capture_output=True,
            text=True,
            check=False,
            env=self.environment,
        )
        self.assertEqual(checked.returncode, 0, checked.stderr)
        report = json.loads(checked.stdout)
        self.assertEqual(report["status"], "pass")
        self.assertEqual(report["checks"]["frameworks"], {"payload": []})

    def test_checker_rejects_embedded_framework_payload(self) -> None:
        contents = self.bundle / "Contents"
        macos = contents / "MacOS"
        macos.mkdir(parents=True)
        (macos / "HarryPotter2").write_bytes(b"\xcf\xfa\xed\xfe" + b"\0" * 16)
        (contents / "Info.plist").write_bytes(plistlib.dumps({
            "CFBundleIdentifier": "com.hp2.native",
            "CFBundleShortVersionString": "0.1.0",
            "CFBundleExecutable": "HarryPotter2",
        }))
        frameworks = contents / "Frameworks"
        frameworks.mkdir()
        (frameworks / "libopenal.dylib").write_text("forbidden")

        checked = subprocess.run(
            [
                sys.executable,
                str(BUILD_DIR / "check_bundle_rs.py"),
                "--repo-root",
                str(self.root),
            ],
            capture_output=True,
            text=True,
            check=False,
            env=self.environment,
        )

        self.assertEqual(checked.returncode, 1, checked.stderr)
        report = json.loads(checked.stdout)
        self.assertEqual(report["status"], "fail")
        self.assertEqual(report["reason_code"], "bundle.framework_payload_unexpected")


if __name__ == "__main__":
    unittest.main()
