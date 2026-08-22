#!/usr/bin/env python3
"""Contract tests for Build/check_bundle.py without touching the real dist bundle."""

from __future__ import annotations

import json
import os
import plistlib
from pathlib import Path
import subprocess
import shutil
import sys
import tempfile
import unittest
from unittest import mock

REPOSITORY_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(REPOSITORY_ROOT / "Build"))
import check_bundle  # noqa: E402


class BundleIntegrityContracts(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(prefix="hp2-bundle-integrity-")
        self.root = Path(self.temporary.name)
        self.bundle = self.root / "HarryPotter2.app"
        self._make_bundle()

    def tearDown(self) -> None:
        self.temporary.cleanup()

    # -- fixture helpers ---------------------------------------------------

    def _make_bundle(
        self,
        *,
        version: str = "0.1.0",
        identifier: str = "com.hp2.native",
        executable_name: str = "HarryPotter2",
        magic: bytes = check_bundle.MACHO_ARM64_MAGIC,
        openal: bool = True,
    ) -> None:
        if self.bundle.exists():
            shutil.rmtree(self.bundle)
        macos = self.bundle / "Contents" / "MacOS"
        frameworks = self.bundle / "Contents" / "Frameworks"
        macos.mkdir(parents=True)
        frameworks.mkdir(parents=True)
        info_plist = {
            "CFBundleExecutable": executable_name,
            "CFBundleIdentifier": identifier,
            "CFBundleShortVersionString": version,
            "CFBundleName": "HarryPotter2",
        }
        (self.bundle / "Contents" / "Info.plist").write_bytes(plistlib.dumps(info_plist))
        (macos / executable_name).write_bytes(magic + b"\0" * 16)
        if openal:
            (frameworks / "libopenal.1.dylib").write_bytes(b"\0" * 4)

    def _codesign(self, *, verify_code: int = 0, output: str = "", signature: str | None = "adhoc") -> mock.Mock:
        def run(command: list[str], **_: object) -> mock.Mock:
            result = mock.Mock()
            result.returncode = verify_code if "--verify" in command else 0
            result.stderr = f"Signature={signature}\n{output}"
            result.stdout = ""
            return result

        return mock.patch.object(check_bundle.subprocess, "run", side_effect=run)

    def _report(self) -> dict:
        return check_bundle.check_bundle(self.root, self.bundle)

    # -- schema ------------------------------------------------------------

    def test_report_matches_schema_v1(self) -> None:
        with self._codesign():
            report = self._report()
        for key in ("schema", "name", "status", "invariant", "data", "artifacts", "command", "exit_reason"):
            self.assertIn(key, report)
        self.assertEqual(report["schema"], 1)
        self.assertEqual(report["name"], "check_bundle")
        self.assertEqual(report["data"]["profile"], "none")
        self.assertEqual(report["status"], "pass")

    def test_passing_report_is_written_to_artifact_dir(self) -> None:
        artifacts = self.root / "artifacts"
        with self._codesign(), mock.patch.dict("os.environ", {"HP2_ARTIFACT_DIR": str(artifacts)}):
            report = self._report()
        written = artifacts / "bundle-integrity.json"
        self.assertTrue(written.is_file())
        self.assertEqual(json.loads(written.read_text())["status"], "pass")
        self.assertEqual(report["artifacts"], [{"kind": "report", "path": str(written)}])

    def test_cli_exit_codes_follow_status(self) -> None:
        stub = self.root / "codesign-stub.sh"
        stub.write_text(
            "#!/bin/sh\n"
            'if [ "$1" = "--verify" ]; then exit 0; fi\n'
            'echo "Signature=adhoc-stub" >&2\n'
            "exit 0\n"
        )
        stub.chmod(0o755)
        environment = dict(os.environ, HP2_CODESIGN_BIN=str(stub))

        passed = subprocess.run(
            [sys.executable, str(REPOSITORY_ROOT / "Build" / "check_bundle.py"),
             "--repo-root", str(self.root), "--bundle", str(self.bundle)],
            capture_output=True, text=True, check=False, env=environment)
        self.assertEqual(passed.returncode, 0)
        self.assertEqual(json.loads(passed.stdout)["status"], "pass")

        blocked = subprocess.run(
            [sys.executable, str(REPOSITORY_ROOT / "Build" / "check_bundle.py"),
             "--repo-root", str(self.root), "--bundle", str(self.root / "absent.app")],
            capture_output=True, text=True, check=False, env=environment)
        self.assertEqual(blocked.returncode, 2)
        self.assertEqual(json.loads(blocked.stdout)["reason_code"], "bundle.dist_missing")

    # -- blocked -----------------------------------------------------------

    def test_missing_bundle_is_blocked_not_failed(self) -> None:
        report = check_bundle.check_bundle(self.root, self.root / "missing.app")
        self.assertEqual(report["status"], "blocked")
        self.assertEqual(report["reason_code"], "bundle.dist_missing")

    # -- Info.plist --------------------------------------------------------

    def test_missing_info_plist_fails_with_reason_code(self) -> None:
        (self.bundle / "Contents" / "Info.plist").unlink()
        report = self._report()
        self.assertEqual(report["status"], "fail")
        self.assertEqual(report["reason_code"], "bundle.info_plist_missing")

    def test_empty_version_or_identifier_fails(self) -> None:
        for key in ("CFBundleShortVersionString", "CFBundleIdentifier"):
            self._make_bundle()
            path = self.bundle / "Contents" / "Info.plist"
            plist = plistlib.loads(path.read_bytes())
            plist[key] = ""
            path.write_bytes(plistlib.dumps(plist))
            with self.subTest(key=key):
                report = self._report()
                self.assertEqual(report["status"], "fail")
                self.assertEqual(report["reason_code"], "bundle.info_plist_incomplete")

    def test_non_reverse_dns_identifier_fails(self) -> None:
        self._make_bundle(identifier="harrypotter")
        report = self._report()
        self.assertEqual(report["reason_code"], "bundle.identifier_malformed")

    # -- executable --------------------------------------------------------

    def test_wrong_magic_fails_as_not_arm64(self) -> None:
        self._make_bundle(magic=b"\xde\xad\xbe\xef")
        report = self._report()
        self.assertEqual(report["status"], "fail")
        self.assertEqual(report["reason_code"], "bundle.executable_not_arm64")

    def test_executable_named_by_plist_is_required(self) -> None:
        self._make_bundle()
        (self.bundle / "Contents" / "MacOS" / "HarryPotter2").rename(
            self.bundle / "Contents" / "MacOS" / "Elsewhere")
        report = self._report()
        self.assertEqual(report["reason_code"], "bundle.executable_missing")

    # -- codesign ----------------------------------------------------------

    def test_unsigned_bundle_warns_but_passes(self) -> None:
        with self._codesign(verify_code=1, output="main executable: code object is not signed at all", signature=None):
            report = self._report()
        self.assertEqual(report["status"], "pass")
        self.assertEqual(report["warnings"], ["bundle is unsigned"])

    def test_invalid_signature_fails(self) -> None:
        with self._codesign(verify_code=1, output="code signature in <...> not valid"):
            report = self._report()
        self.assertEqual(report["status"], "fail")
        self.assertEqual(report["reason_code"], "bundle.codesign_invalid")

    # -- frameworks --------------------------------------------------------

    def test_missing_openal_fails_with_framework_reason(self) -> None:
        self._make_bundle(openal=False)
        report = self._report()
        self.assertEqual(report["status"], "fail")
        self.assertEqual(report["reason_code"], "bundle.framework_missing")


if __name__ == "__main__":
    unittest.main()
