#!/usr/bin/env python3
"""Synthetic schema contracts for Build/fire_texture_trace.py; no game launch."""

from __future__ import annotations

from copy import deepcopy
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

REPOSITORY_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(REPOSITORY_ROOT / "Build"))
import fire_texture_trace  # noqa: E402


class FireTextureTraceContracts(unittest.TestCase):
    def test_accepts_pinned_fire1_observation(self) -> None:
        trace = fire_texture_trace._synthetic_trace()
        self.assertIs(fire_texture_trace.validate_trace(trace), trace)

    def test_rejects_wrong_asset_provenance(self) -> None:
        trace = fire_texture_trace._synthetic_trace()
        trace["target"]["archive"]["sha256"] = "0" * 64  # type: ignore[index]
        with self.assertRaisesRegex(fire_texture_trace.TraceError, r"archive\.sha256"):
            fire_texture_trace.validate_trace(trace)

    def test_rejects_non_native_init_tables_interval(self) -> None:
        trace = fire_texture_trace._synthetic_trace()
        trace["init_tables"]["rng"]["after"] -= 1  # type: ignore[index]
        with self.assertRaisesRegex(fire_texture_trace.TraceError, r"512 appRand"):
            fire_texture_trace.validate_trace(trace)

    def test_rejects_lifecycle_without_update_dirty_transition(self) -> None:
        trace = fire_texture_trace._synthetic_trace()
        trace["lifecycle"][1]["dirty"]["after"] = False  # type: ignore[index]
        with self.assertRaisesRegex(fire_texture_trace.TraceError, r"Update setting dirty"):
            fire_texture_trace.validate_trace(trace)

    def test_rejects_reordered_burn_write(self) -> None:
        trace = fire_texture_trace._synthetic_trace()
        duplicate = deepcopy(trace["burn_mip0_writes"][0])  # type: ignore[index]
        trace["burn_mip0_writes"].append(duplicate)  # type: ignore[index]
        with self.assertRaisesRegex(fire_texture_trace.TraceError, r"strictly increasing native spark order"):
            fire_texture_trace.validate_trace(trace)



class FireTextureTraceComparisonContracts(unittest.TestCase):
    def test_matches_identical_valid_artifacts(self) -> None:
        trace = fire_texture_trace._synthetic_trace()
        self.assertEqual(
            fire_texture_trace.compare_traces(trace, deepcopy(trace)),
            {"status": "match", "first_difference": None},
        )

    def test_reports_first_speed_rand_difference(self) -> None:
        cpp = fire_texture_trace._synthetic_trace()
        rust = deepcopy(cpp)
        rust["first_speed_rand"]["value"] = 43  # type: ignore[index]
        self.assertEqual(
            fire_texture_trace.compare_traces(cpp, rust),
            {
                "status": "mismatch",
                "first_difference": {
                    "path": "$.first_speed_rand.value",
                    "cpp": 42,
                    "rust": 43,
                },
            },
        )

    def test_rejects_invalid_rust_artifact_before_comparing(self) -> None:
        cpp = fire_texture_trace._synthetic_trace()
        rust = deepcopy(cpp)
        rust["target"]["export"]["index"] = 885  # type: ignore[index]
        with self.assertRaisesRegex(fire_texture_trace.TraceError, r"export\.index"):
            fire_texture_trace.compare_traces(cpp, rust)

    def test_cli_writes_atomic_report_for_matching_artifacts(self) -> None:
        with tempfile.TemporaryDirectory(prefix="hp2-fire-texture-trace-") as temporary:
            root = Path(temporary)
            cpp = root / "cpp.json"
            rust = root / "rust.json"
            report = root / "reports" / "comparison.json"
            rendered = json.dumps(fire_texture_trace._synthetic_trace())
            cpp.write_text(rendered, encoding="utf-8")
            rust.write_text(rendered, encoding="utf-8")
            completed = subprocess.run(
                [
                    sys.executable,
                    str(REPOSITORY_ROOT / "Build" / "fire_texture_trace.py"),
                    "--cpp-trace",
                    str(cpp),
                    "--rust-trace",
                    str(rust),
                    "--report",
                    str(report),
                ],
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(completed.returncode, 0, completed.stderr)
            self.assertEqual(
                json.loads(report.read_text(encoding="utf-8")),
                {"status": "match", "first_difference": None},
            )
if __name__ == "__main__":
    unittest.main()
