#!/usr/bin/env python3
"""Contract tests for Build/verify_prototype_archive.py.

The dev-data checks are exercised hermetically through small zip fixtures
(bsdtar reads zip transparently); one smoke invocation uses the repository's
real prototype-data.7z/bag when present and otherwise accepts a blocked
report so the contract stays green on machines without dev data.
"""

from __future__ import annotations

import hashlib
import json
import subprocess
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path

REPOSITORY_ROOT = Path(__file__).resolve().parent.parent
FINDINGS_KEY = "findings"

SCRIPT = REPOSITORY_ROOT / "Build" / "verify_prototype_archive.py"

SCHEMA_KEYS = {
    "schema", "name", "status", "invariant", "reason_code",
    "data", "artifacts", "command", "exit_reason",
}


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def build_bag(root: Path) -> dict[str, bytes]:
    """Write a minimal valid BagIt bag and return its file map."""
    payload = b"hogwarts payload\n"
    bagit = b"BagIt-Version: 1.0\nTag-File-Character-Encoding: UTF-8\n"
    info = f"Payload-Oxum: {len(payload)}.1\n".encode()
    manifest = f"{sha256(payload)}  data/payload.txt\n".encode()
    tag_payloads = {"bagit.txt": bagit, "bag-info.txt": info,
                    "manifest-sha256.txt": manifest}
    tagmanifest = "".join(
        f"{sha256(body)}  {name}\n" for name, body in sorted(tag_payloads.items())
    ).encode()

    root.mkdir(parents=True)
    (root / "data").mkdir()
    (root / "data" / "payload.txt").write_bytes(payload)
    (root / "bagit.txt").write_bytes(bagit)
    (root / "bag-info.txt").write_bytes(info)
    (root / "manifest-sha256.txt").write_bytes(manifest)
    (root / "tagmanifest-sha256.txt").write_bytes(tagmanifest)
    return {
        "data/payload.txt": payload,
        "bagit.txt": bagit,
        "bag-info.txt": info,
        "manifest-sha256.txt": manifest,
        "tagmanifest-sha256.txt": tagmanifest,
    }


def build_archive(path: Path, files: dict[str, bytes], extra_members: dict[str, bytes] | None = None) -> None:
    with zipfile.ZipFile(path, "w") as archive:
        for name, body in sorted(files.items()):
            archive.writestr(f"bag/{name}", body)
        for name, body in sorted((extra_members or {}).items()):
            archive.writestr(f"bag/{name}", body)


class PrototypeArchiveContracts(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(prefix="hp2-prototype-archive-")
        self.root = Path(self.temporary.name)

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def _run(self, *arguments: str) -> tuple[int, dict]:
        completed = subprocess.run(
            [sys.executable, str(SCRIPT), *arguments],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )
        report = json.loads(completed.stdout.decode())
        return completed.returncode, report

    def _fixture(self, extra_members: dict[str, bytes] | None = None) -> tuple[Path, Path]:
        bag_root = self.root / "bag"
        files = build_bag(bag_root)
        archive = self.root / "fixture-data.zip"
        build_archive(archive, files, extra_members)
        return archive, bag_root

    def test_missing_archive_is_blocked_not_failed(self) -> None:
        bag_root = self.root / "bag"
        build_bag(bag_root)
        code, report = self._run(
            "--archive", str(self.root / "absent-data.7z"),
            "--bag", str(bag_root),
            "--output", str(self.root / "out.json"),
        )
        self.assertEqual(code, 0)
        self.assertEqual(report["status"], "blocked")
        self.assertEqual(report["reason_code"], "data.archive_missing")

    def test_missing_bag_is_blocked(self) -> None:
        archive = self.root / "fixture-data.zip"
        build_archive(archive, build_bag(self.root / "unused-bag"))
        code, report = self._run("--archive", str(archive),
                                 "--bag", str(self.root / "no-such-bag"))
        self.assertEqual(code, 0)
        self.assertEqual(report["status"], "blocked")
        self.assertEqual(report["reason_code"], "data.bag_incomplete")

    def test_unreadable_archive_fails_with_listing_reason(self) -> None:
        _, bag_root = self._fixture()
        garbage = self.root / "garbage.7z"
        garbage.write_bytes(b"this is not an archive\n")
        code, report = self._run("--archive", str(garbage), "--bag", str(bag_root))
        self.assertEqual(code, 1)
        self.assertEqual(report["status"], "fail")
        self.assertEqual(report["reason_code"], "data.archive_listing_failed")
        self.assertIn("bsdtar", report["exit_reason"].lower())

    def test_consistent_fixture_passes_and_is_deterministic(self) -> None:
        archive, bag_root = self._fixture()
        first = self._run("--archive", str(archive), "--bag", str(bag_root))
        second = self._run("--archive", str(archive), "--bag", str(bag_root))
        self.assertEqual(first[0], 0)
        self.assertEqual(first[0], second[0])
        # Deterministic ordering: byte-identical reports across runs, no timestamps.
        self.assertEqual(json.dumps(first[1], sort_keys=True),
                         json.dumps(second[1], sort_keys=True))
        report = first[1]
        self.assertEqual(report["schema"], 1)
        findings = report["findings"]
        self.assertEqual(findings["verdict_counts"]["payload_hash_recorded"], 1)
        self.assertEqual(findings["verdict_counts"]["tag_hash_recorded"], 3)
        self.assertEqual(findings["verdict_counts"]["tag_self_referential"], 1)
        self.assertEqual(findings["local_verification"]["verified"], 1)
        self.assertEqual(findings["gaps"], [])
        self.assertTrue(findings["local_verification"]["payload_oxum_matches"])

    def test_uncovered_payload_member_becomes_an_explicit_gap(self) -> None:
        archive, bag_root = self._fixture(extra_members={"data/unlisted.bin": b"sneaky\n"})
        code, report = self._run("--archive", str(archive), "--bag", str(bag_root))
        self.assertEqual(code, 1)
        self.assertEqual(report["status"], "fail")
        self.assertEqual(report["reason_code"], "data.identity_gap")
        gap_members = [gap["member"] for gap in report["findings"]["gaps"]]
        self.assertIn("bag/data/unlisted.bin", gap_members)
        self.assertEqual(report["findings"]["verdict_counts"]["unverifiable_payload"], 1)

    def test_report_schema_and_artifact_contract(self) -> None:
        archive, bag_root = self._fixture()
        output = self.root / "artifacts" / "prototype-archive.json"
        code, report = self._run(
            "--archive", str(archive), "--bag", str(bag_root), "--output", str(output)
        )
        self.assertEqual(code, 0)
        self.assertEqual(set(report) - SCHEMA_KEYS, {FINDINGS_KEY})
        self.assertTrue(SCHEMA_KEYS <= set(report))
        self.assertEqual(report["data"]["profile"], "data-none")
        self.assertEqual(report["artifacts"],
                         [{"kind": "json-report", "path": str(output)}])
        self.assertTrue(output.is_file())
        persisted = json.loads(output.read_text(encoding="utf-8"))
        self.assertEqual(persisted["status"], "pass")
        self.assertNotIn("timestamp", json.dumps(persisted).lower())
        self.assertTrue(report["command"][0].endswith("verify_prototype_archive.py"))

    def test_repository_defaults_produce_valid_report(self) -> None:
        """Smoke against the real dev data; blocked is acceptable when absent."""
        output = self.root / "repo.json"
        code, report = self._run("--output", str(output))
        self.assertEqual(code, 0)
        self.assertIn(report["status"], ("pass", "blocked"))
        self.assertIn(report.get("reason_code"),
                      (None, "data.archive_missing", "data.bag_incomplete"))
        if report["status"] == "pass":
            self.assertEqual(report["findings"]["member_sets"],
                             {"archive_only": [], "bag_only": []})
            self.assertEqual(report["findings"]["gaps"], [])


if __name__ == "__main__":
    unittest.main()
