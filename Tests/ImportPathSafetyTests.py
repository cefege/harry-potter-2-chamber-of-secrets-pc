#!/usr/bin/env python3
"""Exercise importer destination traversal through the shipping CLI."""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


IMPORTER: Path
ROOT = Path(__file__).resolve().parent.parent


def fixture_module():
    """Load the existing fixture helper without retaining its argv changes."""
    name = "hp2_import_path_safety_fixtures"
    spec = importlib.util.spec_from_file_location(name, ROOT / "Tests/ImporterParityTests.py")
    if spec is None or spec.loader is None:
        raise RuntimeError("cannot load importer fixture helper")
    module = importlib.util.module_from_spec(spec)
    original_argv = sys.argv
    original_bytecode = sys.dont_write_bytecode
    previous_oracle = sys.modules.get("prepare_retail_data")
    try:
        sys.dont_write_bytecode = True
        sys.argv = [str(spec.origin), str(IMPORTER), str(ROOT / "Build/prepare_retail_data.py")]
        spec.loader.exec_module(module)
    finally:
        sys.argv = original_argv
        sys.dont_write_bytecode = original_bytecode
        if previous_oracle is None:
            sys.modules.pop("prepare_retail_data", None)
        else:
            sys.modules["prepare_retail_data"] = previous_oracle
    return module


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as binary:
        for chunk in iter(lambda: binary.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def run_import(case: Path, label: str, retail: Path, output: Path, mode: str):
    command = [
        str(IMPORTER), "--retail-root", str(retail), "--output", str(output),
        "--profile", "retail-only", "--link-mode", mode,
    ]
    metadata = {
        "command": command,
        "cwd": str(case),
        "binary": str(IMPORTER),
        "binary_sha256_before": sha256(IMPORTER),
        "stdout": f"{label}.stdout",
        "stderr": f"{label}.stderr",
    }
    record = case / f"{label}.json"
    record.write_text(json.dumps(metadata, indent=2) + "\n", encoding="utf-8")
    try:
        result = subprocess.run(
            command, cwd=case, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            check=False, timeout=300,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        metadata["execution_error"] = repr(error)
        if isinstance(error, subprocess.TimeoutExpired):
            (case / f"{label}.stdout").write_bytes(error.stdout or b"")
            (case / f"{label}.stderr").write_bytes(error.stderr or b"")
        record.write_text(json.dumps(metadata, indent=2) + "\n", encoding="utf-8")
        raise
    (case / f"{label}.stdout").write_bytes(result.stdout)
    (case / f"{label}.stderr").write_bytes(result.stderr)
    metadata["returncode"] = result.returncode
    metadata["binary_sha256_after"] = sha256(IMPORTER)
    record.write_text(json.dumps(metadata, indent=2) + "\n", encoding="utf-8")
    return result, metadata


class ImportPathSafetyTests(unittest.TestCase):
    def test_rejects_collected_traversal_without_escape_or_commit(self) -> None:
        artifacts = Path(os.environ["HP2_ARTIFACT_DIR"]).resolve(strict=True)
        root = Path(tempfile.mkdtemp(prefix="import-path-safety-", dir=artifacts))
        self.assertEqual(root.stat().st_mode & 0o777, 0o700)
        fixtures = fixture_module()
        for mode in ("copy", "auto"):
            with self.subTest(mode=mode):
                case = root / mode
                case.mkdir(mode=0o700)
                retail, _ = fixtures.make_fixtures(case / "fixtures")
                control_output = case / "control-output"
                control, control_info = run_import(case, "control", retail, control_output, mode)
                self.assertEqual(control_info["binary_sha256_before"], control_info["binary_sha256_after"])
                self.assertEqual(control.returncode, 0, control.stderr)
                self.assertTrue((control_output / "overlay-manifest.json").is_file())
                self.assertTrue((control_output / "Maps/Entry.unr").is_file())

                source = retail / "System" / r"..\..\escaped.u"
                source.write_bytes(b"private traversal fixture\n")
                escaped = case / "escaped.u"
                output = case / "output"
                self.assertFalse(os.path.lexists(escaped))
                self.assertFalse(os.path.lexists(output))
                result, info = run_import(case, "traversal", retail, output, mode)
                self.assertEqual(info["binary_sha256_before"], info["binary_sha256_after"])
                self.assertEqual(info["binary_sha256_before"], control_info["binary_sha256_before"])
                self.assertEqual(result.returncode, 2, result.stderr)
                self.assertEqual(result.stdout, b"")
                self.assertEqual(
                    result.stderr,
                    b"hp2_import: invalid planned output path: System/../../escaped.u\n",
                )
                self.assertFalse(os.path.lexists(escaped), f"escaped file: {escaped}")
                self.assertFalse(os.path.lexists(output), f"committed output: {output}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--importer", required=True, type=Path)
    arguments = parser.parse_args()
    IMPORTER = arguments.importer.resolve(strict=True)
    unittest.main(argv=[sys.argv[0]])
