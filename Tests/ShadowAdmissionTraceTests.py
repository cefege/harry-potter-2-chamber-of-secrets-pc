#!/usr/bin/env python3
"""Synthetic artifact contracts for Build/shadow_admission_trace.py; no map execution."""

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
import shadow_admission_trace  # noqa: E402


def _event(
    frame: int,
    render_pass: int,
    owner_path: str = "PrivetDr.Harry1",
    shadow_path: str | None = "PrivetDr.Harry1.ActorShadow0",
) -> dict[str, object]:
    return {
        "frame": frame,
        "pass": render_pass,
        "shadow_path": shadow_path,
        "owner_path": owner_path,
        "candidate_status": "admitted",
        "raw_facts": {
            "viewport_actor_path": "PrivetDr.Harry1",
            "view_target_path": None,
            "behind_view": False,
            "recursion_parent_path": None,
            "perspective": True,
            "world_dynamics": True,
        },
        "update_eligible": True,
    }


def _trace(
    events: list[dict[str, object]], *, limit: int = 16, truncated: bool = False,
) -> dict[str, object]:
    return {
        "version": 1,
        "enabled": True,
        "limit": limit,
        "count": len(events),
        "truncated": truncated,
        "events": events,
    }


def _cpp_rooted(event: dict[str, object]) -> dict[str, object]:
    rooted = deepcopy(event)
    for field in ("owner_path", "shadow_path"):
        if rooted[field] is not None:
            rooted[field] = f"Package47.{str(rooted[field]).removeprefix('PrivetDr.')}"
    facts = rooted["raw_facts"]
    assert isinstance(facts, dict)
    for field in ("viewport_actor_path", "view_target_path", "recursion_parent_path"):
        if facts[field] is not None:
            facts[field] = f"Package47.{str(facts[field]).removeprefix('PrivetDr.')}"
    return rooted


class ShadowAdmissionSchemaContracts(unittest.TestCase):
    def test_accepts_complete_shared_schema(self) -> None:
        trace = _trace([_event(7, 2)])
        self.assertIs(shadow_admission_trace.validate_trace(trace), trace)

    def test_accepts_missing_shadow_candidate_without_fabricating_path(self) -> None:
        event = _event(7, 2, shadow_path=None)
        event["candidate_status"] = "missing_shadow"
        event["update_eligible"] = False
        shadow_admission_trace.validate_trace(_trace([event]))

    def test_rejects_unknown_candidate_status(self) -> None:
        event = _event(7, 2)
        event["candidate_status"] = "rejected_unknown"
        with self.assertRaisesRegex(
            shadow_admission_trace.TraceError, r"candidate_status: must be one of",
        ):
            shadow_admission_trace.validate_trace(_trace([event]))

    def test_rejects_non_nullable_raw_path(self) -> None:
        event = _event(7, 2)
        facts = event["raw_facts"]
        assert isinstance(facts, dict)
        facts["viewport_actor_path"] = ""
        with self.assertRaisesRegex(
            shadow_admission_trace.TraceError, r"viewport_actor_path: must be a non-empty string or null",
        ):
            shadow_admission_trace.validate_trace(_trace([event]))

    def test_rejects_out_of_order_frame_pass_traversal(self) -> None:
        with self.assertRaisesRegex(
            shadow_admission_trace.TraceError, r"must be in frame/pass traversal order",
        ):
            shadow_admission_trace.validate_trace(_trace([_event(8, 0), _event(7, 1)]))


class ShadowAdmissionPairingContracts(unittest.TestCase):
    def test_canonicalizes_map_roots_for_pairing_and_compared_paths(self) -> None:
        rust_event = _event(7, 2)
        cpp_event = _cpp_rooted(rust_event)
        report = shadow_admission_trace.compare_traces(
            _trace([cpp_event]), _trace([rust_event]), map_stem="PrivetDr",
        )
        self.assertEqual(report, {"status": "match", "first_difference": None})

    def test_pairs_same_owner_pass_by_traversal_order(self) -> None:
        first = _event(7, 2)
        second = _event(7, 2)
        second["candidate_status"] = "rejected_bounds"
        second["update_eligible"] = False
        report = shadow_admission_trace.compare_traces(
            _trace([first, second]), _trace([deepcopy(first), deepcopy(second)]),
            map_stem="PrivetDr",
        )
        self.assertEqual(report["status"], "match")

    def test_reports_first_decision_difference_after_canonical_pairing(self) -> None:
        rust_event = _event(7, 2)
        cpp_event = _cpp_rooted(rust_event)
        cpp_event["candidate_status"] = "rejected_bounds"
        cpp_event["update_eligible"] = False
        report = shadow_admission_trace.compare_traces(
            _trace([cpp_event]), _trace([rust_event]), map_stem="PrivetDr",
        )
        self.assertEqual(report["status"], "mismatch")
        difference = report["first_difference"]
        assert isinstance(difference, dict)
        self.assertEqual(difference["path"], "$.event.candidate_status")
        self.assertEqual(difference["key"], {
            "frame": 7,
            "pass": 2,
            "owner_path": "<map>.Harry1",
            "traversal": 0,
        })

    def test_reports_missing_candidate_with_canonical_key(self) -> None:
        cpp_event = _cpp_rooted(_event(7, 2))
        report = shadow_admission_trace.compare_traces(
            _trace([cpp_event]), _trace([]), map_stem="PrivetDr",
        )
        self.assertEqual(report["status"], "mismatch")
        difference = report["first_difference"]
        assert isinstance(difference, dict)
        self.assertEqual(difference["cpp"], {
            "index": 0,
            "key": [7, 2, "<map>.Harry1"],
            "traversal": 0,
        })
        self.assertEqual(difference["occurrence"], 0)

    def test_pairs_repeated_owner_by_occurrence_ordinal_and_frame_offset(self) -> None:
        cpp_first = _cpp_rooted(_event(0, 2))
        cpp_second = _cpp_rooted(_event(1, 2))
        rust_first = _event(1, 2)
        rust_second = _event(2, 2)
        rust_second["raw_facts"] = deepcopy(rust_second["raw_facts"])
        rust_second["raw_facts"]["behind_view"] = True
        report = shadow_admission_trace.compare_traces(
            _trace([cpp_first, cpp_second]),
            _trace([rust_first, rust_second]),
            map_stem="PrivetDr",
        )
        self.assertEqual(report["status"], "mismatch")
        difference = report["first_difference"]
        assert isinstance(difference, dict)
        self.assertEqual(difference["path"], "$.event.raw_facts.behind_view")
        self.assertEqual(difference["cpp_index"], 1)
        self.assertEqual(difference["rust_index"], 1)
        self.assertEqual(difference["key"]["owner_path"], "<map>.Harry1")
        self.assertEqual(difference["occurrence"], 1)
        self.assertEqual(difference["rust_frame_offset"], 1)

    def test_ignores_nonshadow_population_and_preserves_raw_artifacts(self) -> None:
        cpp_nonshadow = _cpp_rooted(_event(0, 0, "PrivetDr.Light104", None))
        cpp_nonshadow["candidate_status"] = "missing_shadow"
        cpp_nonshadow["update_eligible"] = False
        cpp_harry = _cpp_rooted(_event(0, 0))
        rust_nonshadow = _event(1, 0, "PrivetDr.SkyLight", None)
        rust_nonshadow["candidate_status"] = "rejected_missing_location"
        rust_nonshadow["update_eligible"] = False
        rust_harry = _event(1, 0)
        cpp_trace = _trace([cpp_nonshadow, cpp_harry])
        rust_trace = _trace([rust_nonshadow, rust_harry])
        cpp_before, rust_before = deepcopy(cpp_trace), deepcopy(rust_trace)
        report = shadow_admission_trace.compare_traces(cpp_trace, rust_trace, map_stem="PrivetDr")
        self.assertEqual(report, {"status": "match", "first_difference": None})
        self.assertEqual(cpp_trace, cpp_before)
        self.assertEqual(rust_trace, rust_before)

    def test_reports_first_raw_decision_difference_after_candidate_alignment(self) -> None:
        cpp_nonshadow = _cpp_rooted(_event(0, 0, "PrivetDr.Light104", None))
        cpp_nonshadow["candidate_status"] = "missing_shadow"
        cpp_nonshadow["update_eligible"] = False
        cpp_harry = _cpp_rooted(_event(0, 0))
        rust_harry = _event(1, 0)
        rust_harry["raw_facts"] = deepcopy(rust_harry["raw_facts"])
        rust_harry["raw_facts"]["behind_view"] = True
        report = shadow_admission_trace.compare_traces(
            _trace([cpp_nonshadow, cpp_harry]), _trace([rust_harry]), map_stem="PrivetDr",
        )
        self.assertEqual(report["status"], "mismatch")
        difference = report["first_difference"]
        assert isinstance(difference, dict)
        self.assertEqual(difference["path"], "$.event.raw_facts.behind_view")
        self.assertEqual(difference["cpp_index"], 1)
        self.assertEqual(difference["rust_index"], 0)
        self.assertEqual(difference["key"]["owner_path"], "<map>.Harry1")
        self.assertEqual(difference["occurrence"], 0)
        self.assertEqual(difference["rust_frame_offset"], 1)

    def test_keeps_external_package_paths_literal(self) -> None:
        cpp_harry = _cpp_rooted(_event(0, 0))
        cpp_external = _event(0, 0, "Package88.ExternalOwner", "Package88.ExternalShadow")
        rust_harry = _event(1, 0)
        rust_external = _event(1, 0, "ExternalPackage.ExternalOwner", "ExternalPackage.ExternalShadow")
        report = shadow_admission_trace.compare_traces(
            _trace([cpp_harry, cpp_external]),
            _trace([rust_harry, rust_external]),
            map_stem="PrivetDr",
        )
        self.assertEqual(report["status"], "mismatch")
        difference = report["first_difference"]
        assert isinstance(difference, dict)
        self.assertEqual(difference["cpp"]["key"][2], "Package88.ExternalOwner")


class ShadowAdmissionCliContracts(unittest.TestCase):
    def test_cli_writes_synthetic_first_difference(self) -> None:
        cpp_event = _cpp_rooted(_event(7, 2))
        rust_event = _event(7, 2)
        rust_event["raw_facts"] = deepcopy(rust_event["raw_facts"])
        facts = rust_event["raw_facts"]
        assert isinstance(facts, dict)
        facts["behind_view"] = True
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            cpp_path = directory / "cpp.json"
            rust_path = directory / "rust.json"
            report_path = directory / "report.json"
            cpp_path.write_text(json.dumps(_trace([cpp_event])), encoding="utf-8")
            rust_path.write_text(json.dumps(_trace([rust_event])), encoding="utf-8")
            result = subprocess.run(
                [
                    sys.executable,
                    str(REPOSITORY_ROOT / "Build" / "shadow_admission_trace.py"),
                    "--cpp-trace", str(cpp_path),
                    "--rust-trace", str(rust_path),
                    "--map-stem", "PrivetDr",
                    "--report", str(report_path),
                ],
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(result.returncode, 1, result.stderr)
            report = json.loads(report_path.read_text(encoding="utf-8"))
            self.assertEqual(report["status"], "mismatch")
            self.assertEqual(report["first_difference"]["path"], "$.event.raw_facts.behind_view")


if __name__ == "__main__":
    raise SystemExit(unittest.main())
