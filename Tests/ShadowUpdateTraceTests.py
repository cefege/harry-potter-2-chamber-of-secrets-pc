#!/usr/bin/env python3
"""Synthetic artifact contracts for Build/shadow_update_trace.py; no map execution."""

from __future__ import annotations

import json
import subprocess
import sys
import tempfile
import unittest
from copy import deepcopy
from pathlib import Path

REPOSITORY_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(REPOSITORY_ROOT / "Build"))
import shadow_update_trace  # noqa: E402


def _event(frame: int, owner_path: str, shadow_path: str = "Level.Shadow") -> dict[str, object]:
    return {
        "frame": frame,
        "shadow_path": shadow_path,
        "owner_path": owner_path,
        "pre": {"location": [1.0, 2.0, 3.0], "rotation": [4, 5, 6]},
        "post": {"location": [7.0, 8.0, 9.0], "rotation": [10, 11, 12]},
        "bone_name": "None",
        "bone_pos": [13.0, 14.0, 15.0],
        "owner": {"location": [16.0, 17.0, 18.0], "rotation": [19, 20, 21]},
        "mesh": {
            "path": "Package.Mesh",
            "active_animation": {"sequence": "Run", "frame": 0.5, "rate": 1.0, "finished": False},
        },
        "render_extent": [22.0, 23.0, 24.0],
        "shadow": {"direction": [25.0, 26.0, 27.0], "rotation": [28, 29, 30]},
        "decal": {
            "trace_origin": [31.0, 32.0, 33.0],
            "trace_direction": [34.0, 35.0, 36.0],
            "trace_distance": 37.0,
            "trace_hit": {"kind": "surface", "surface_index": 2},
        },
        "attach_decal": {"outcome": "attached", "surface_count": 1},
    }


def _trace(events: list[dict[str, object]], *, limit: int = 16, truncated: bool = False) -> dict[str, object]:
    return {
        "version": 1,
        "enabled": True,
        "limit": limit,
        "count": len(events),
        "truncated": truncated,
        "events": events,
    }


class ShadowUpdateSchemaContracts(unittest.TestCase):
    def test_accepts_complete_shared_schema(self) -> None:
        trace = _trace([_event(7, "Level.Owner")])
        self.assertIs(shadow_update_trace.validate_trace(trace), trace)

    def test_accepts_nulls_that_describe_skipped_owner_without_fabrication(self) -> None:
        event = _event(7, "Level.Owner")
        event["owner_path"] = None
        event["bone_name"] = None
        event["bone_pos"] = None
        event["owner"] = None
        event["mesh"]["path"] = None  # type: ignore[index]
        event["mesh"]["active_animation"] = None  # type: ignore[index]
        event["render_extent"] = None
        event["shadow"] = None
        event["decal"] = {
            "trace_origin": None,
            "trace_direction": None,
            "trace_distance": None,
            "trace_hit": {"kind": "not_attempted", "surface_index": None},
        }
        event["attach_decal"] = {"outcome": "skipped_owner", "surface_count": 0}
        shadow_update_trace.validate_trace(_trace([event]))

    def test_rejects_owner_transform_when_owner_path_is_null(self) -> None:
        event = _event(7, "Level.Owner")
        event["owner_path"] = None
        event["mesh"]["active_animation"] = None  # type: ignore[index]
        with self.assertRaisesRegex(
            shadow_update_trace.TraceError,
            r"events\[0\]\.owner: must be null when owner_path is null",
        ):
            shadow_update_trace.validate_trace(_trace([event]))

    def test_rejects_missing_capture_field(self) -> None:
        trace = _trace([_event(7, "Level.Owner")])
        del trace["events"][0]["decal"]  # type: ignore[index]
        with self.assertRaisesRegex(shadow_update_trace.TraceError, r"events\[0\]: missing required field 'decal'"):
            shadow_update_trace.validate_trace(trace)

    def test_rejects_noncanonical_transform_shape_without_normalizing(self) -> None:
        trace = _trace([_event(7, "Level.Owner")])
        trace["events"][0]["pre"]["location"] = {"x": 1, "y": 2, "z": 3}  # type: ignore[index]
        with self.assertRaisesRegex(shadow_update_trace.TraceError, r"pre\.location: must be a three-element JSON array"):
            shadow_update_trace.validate_trace(trace)

    def test_rejects_count_and_truncation_inconsistency(self) -> None:
        trace = _trace([_event(7, "Level.Owner")], limit=2, truncated=True)
        with self.assertRaisesRegex(shadow_update_trace.TraceError, r"truncated: requires count to equal limit"):
            shadow_update_trace.validate_trace(trace)


class ShadowUpdatePairingContracts(unittest.TestCase):
    def test_pairs_same_frame_owner_by_per_key_traversal_order(self) -> None:
        cpp = _trace([_event(9, "Level.A"), _event(9, "Level.B"), _event(9, "Level.A", "Level.Shadow2")])
        rust = _trace([_event(9, "Level.B"), _event(9, "Level.A"), _event(9, "Level.A", "Level.Shadow2")])
        self.assertEqual(shadow_update_trace.compare_traces(cpp, rust), {"status": "match", "first_difference": None})

    def test_selected_map_root_canonicalizes_paired_owner_paths(self) -> None:
        cpp = _trace([_event(0, "Package2.Harry1", "Package2.HarryShadow1")])
        rust = _trace([_event(0, "PrivetDr.Harry1", "PrivetDr.HarryShadow1")])

        comparison = shadow_update_trace.compare_traces(cpp, rust, map_stem="PrivetDr")

        self.assertEqual(comparison, {"status": "match", "first_difference": None})

    def test_selected_map_pairing_does_not_use_anonymous_shadow_paths_as_keys(self) -> None:
        cpp = _trace([_event(0, "Package2.Harry1", "Package2.HarryShadow1")])
        rust = _trace([_event(0, "PrivetDr.Harry1", "ScriptSpawn_583")])

        comparison = shadow_update_trace.compare_traces(cpp, rust, map_stem="PrivetDr")

        self.assertEqual(comparison["status"], "mismatch")
        self.assertEqual(
            comparison["first_difference"]["path"], "$.event.shadow_path",  # type: ignore[index]
        )

    def test_selected_map_pairing_does_not_normalize_external_package_paths(self) -> None:
        cpp = _trace([_event(0, "PackageOther.Harry1")])
        rust = _trace([_event(0, "PrivetDr.Harry1")])

        comparison = shadow_update_trace.compare_traces(cpp, rust, map_stem="PrivetDr")

        self.assertEqual(comparison["status"], "mismatch")
        self.assertEqual(
            comparison["first_difference"]["cpp"],  # type: ignore[index]
            {"index": 0, "key": [0, "PackageOther.Harry1"], "traversal": 0},
        )

    def test_reports_earliest_field_of_matched_pair(self) -> None:
        cpp = _trace([_event(9, "Level.A"), _event(9, "Level.A", "Level.Shadow2")])
        rust = deepcopy(cpp)
        rust["events"][1]["post"]["location"][1] = 99.0  # type: ignore[index]
        comparison = shadow_update_trace.compare_traces(cpp, rust)
        self.assertEqual(comparison["status"], "mismatch")
        difference = comparison["first_difference"]
        self.assertEqual(difference["path"], "$.event.post.location[1]")  # type: ignore[index]
        self.assertEqual(difference["key"], {"frame": 9, "owner_path": "Level.A", "traversal": 1})  # type: ignore[index]

    def test_reports_missing_event_at_cpp_traversal_position(self) -> None:
        cpp = _trace([_event(9, "Level.A"), _event(9, "Level.A", "Level.Shadow2")])
        rust = _trace([_event(9, "Level.A")])
        comparison = shadow_update_trace.compare_traces(cpp, rust)
        self.assertEqual(comparison["status"], "mismatch")
        self.assertEqual(comparison["first_difference"]["cpp"], {"index": 1, "key": [9, "Level.A"], "traversal": 1})  # type: ignore[index]
        self.assertEqual(comparison["first_difference"]["rust"], {"missing": True})  # type: ignore[index]

    def test_reports_first_unpaired_rust_event_in_rust_traversal_order(self) -> None:
        cpp = _trace([_event(9, "Level.A")])
        rust = _trace([_event(9, "Level.A"), _event(9, "Level.B")])
        comparison = shadow_update_trace.compare_traces(cpp, rust)
        self.assertEqual(comparison["status"], "mismatch")
        self.assertEqual(
            comparison["first_difference"]["rust"],  # type: ignore[index]
            {"index": 1, "key": [9, "Level.B"], "traversal": 0},
        )


class ShadowUpdateCliContracts(unittest.TestCase):
    def test_cli_reads_synthetic_side_artifacts_and_writes_first_difference(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            cpp_path = directory / "cpp.json"
            rust_path = directory / "rust.json"
            report_path = directory / "report.json"
            cpp_path.write_text(json.dumps(_trace([_event(3, "Level.Owner")])), encoding="utf-8")
            rust = _trace([_event(3, "Level.Owner")])
            rust["events"][0]["shadow"]["rotation"][2] = 42  # type: ignore[index]
            rust_path.write_text(json.dumps(rust), encoding="utf-8")
            completed = subprocess.run(
                [
                    sys.executable,
                    str(REPOSITORY_ROOT / "Build" / "shadow_update_trace.py"),
                    "--cpp-trace",
                    str(cpp_path),
                    "--rust-trace",
                    str(rust_path),
                    "--report",
                    str(report_path),
                ],
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(completed.returncode, 1, completed.stderr)
            report = json.loads(report_path.read_text(encoding="utf-8"))
            self.assertEqual(report["first_difference"]["path"], "$.event.shadow.rotation[2]")


if __name__ == "__main__":
    raise SystemExit(unittest.main())
