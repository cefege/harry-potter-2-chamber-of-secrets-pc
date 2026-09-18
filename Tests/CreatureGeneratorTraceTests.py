#!/usr/bin/env python3
"""Synthetic v1 contracts for Build/creature_generator_trace.py; no map execution."""

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
import creature_generator_trace  # noqa: E402


def _reference(path: str = "<map>.GFemSly5", cls: str = "HGame.GFemSly") -> dict[str, str]:
    return {"path": path, "class": cls}


def _event(
    *, identity: str = "<map>.CreatureGenerator0", frame: int = 7, order: int = 3,
    gate_failure: str | None = None, rng: list[dict[str, object]] | None = None,
) -> dict[str, object]:
    gates = []
    for name in creature_generator_trace.GATE_ORDER:
        gates.append({"name": name, "passed": name != gate_failure})
        if name == gate_failure:
            break
    tick_eligible = gate_failure != "tick_wait"
    return {
        "identity": identity,
        "frame": frame,
        "tick": {"eligible": tick_eligible, "order": order},
        "timing": {"curr_time": 1.0, "wait_time": 0.5, "delta_seconds": 0.02},
        "b_off": False,
        "trigger": {"waiting_time": 0.0, "generate_creature": False, "tag": None},
        "active": {"count": 0, "max": 10},
        "first_pp": _reference("<map>.Patrol0", "HGame.PatrolPoint"),
        "camera_visible": False,
        "base_creature_count": 2,
        "gates": gates,
        "rng": (
            [{"kind": "rand", "raw": 5, "index": 1, "resolved_class": "HGame.GFemSly", "bound": 2}]
            if rng is None and gate_failure is None
            else ([] if rng is None else rng)
        ),
        "spawn": (
            {"request": _reference(), "result": None, "published_child": None}
            if gate_failure is None
            else {"request": None, "result": None, "published_child": None}
        ),
    }


def _trace(events: list[dict[str, object]], *, limit: int = 8, truncated: bool = False) -> dict[str, object]:
    return {
        "version": 1,
        "enabled": True,
        "limit": limit,
        "count": len(events),
        "truncated": truncated,
        "events": events,
    }


class CreatureGeneratorTraceValidationContracts(unittest.TestCase):
    def test_accepts_source_ordered_tick_and_rng_records(self) -> None:
        event = _event(rng=[
            {"kind": "rand_range", "raw": 14, "index": None, "resolved_class": None, "min": 1.0, "max": 2.0, "result": 1.4},
            {"kind": "rand", "raw": 5, "index": 1, "resolved_class": "HGame.GFemSly", "bound": 2},
        ])
        event["spawn"] = {"request": _reference(), "result": _reference("<map>.GFemSly5_0"), "published_child": _reference("<map>.GFemSly5_0")}
        trace = _trace([event])
        self.assertIs(creature_generator_trace.validate_trace(trace), trace)

    def test_rejects_gate_out_of_cpp_source_order(self) -> None:
        event = _event(gate_failure="b_off")
        event["gates"][1]["name"] = "active_at_capacity"  # type: ignore[index]
        with self.assertRaisesRegex(creature_generator_trace.TraceError, r"gates\[1\]\.name"):
            creature_generator_trace.validate_trace(_trace([event]))

    def test_accepts_scheduler_eligibility_when_timing_gate_returns(self) -> None:
        event = _event(gate_failure="tick_wait")
        event["tick"]["eligible"] = True  # type: ignore[index]
        creature_generator_trace.validate_trace(_trace([event]))

    def test_rejects_selection_after_early_return_but_allows_prior_randrange(self) -> None:
        event = _event(gate_failure="b_off", rng=[
            {"kind": "rand_range", "raw": 3, "index": None, "resolved_class": None, "min": 1.0, "max": 2.0, "result": 1.2},
        ])
        creature_generator_trace.validate_trace(_trace([event]))
        event["rng"].append({"kind": "rand", "raw": 4, "index": 0, "resolved_class": "HGame.GFemSly", "bound": 2})  # type: ignore[index]
        with self.assertRaisesRegex(creature_generator_trace.TraceError, r"cannot select.*early-return gate"):
            creature_generator_trace.validate_trace(_trace([event]))

    def test_rejects_successful_multi_creature_gate_without_selection_or_spawn_request(self) -> None:
        event = _event()
        event["rng"] = []
        event["spawn"] = {"request": None, "result": None, "published_child": None}
        with self.assertRaisesRegex(
            creature_generator_trace.TraceError,
            r"must include a creature selection when more than one base creature exists",
        ):
            creature_generator_trace.validate_trace(_trace([event]))

    def test_accepts_failed_terminal_class_resolution_with_selection_and_no_spawn(self) -> None:
        event = _event(
            gate_failure="class_resolution",
            rng=[{"kind": "rand", "raw": 5, "index": 1, "resolved_class": None, "bound": 2}],
        )
        creature_generator_trace.validate_trace(_trace([event]))

    def test_rejects_spawn_after_failed_terminal_class_resolution(self) -> None:
        event = _event(
            gate_failure="class_resolution",
            rng=[{"kind": "rand", "raw": 5, "index": 1, "resolved_class": None, "bound": 2}],
        )
        event["spawn"] = {"request": _reference(), "result": None, "published_child": None}
        with self.assertRaisesRegex(
            creature_generator_trace.TraceError, r"must be entirely null after an early-return gate",
        ):
            creature_generator_trace.validate_trace(_trace([event]))

    def test_rejects_successful_class_resolution_without_spawn_request(self) -> None:
        event = _event()
        event["spawn"] = {"request": None, "result": None, "published_child": None}
        with self.assertRaisesRegex(
            creature_generator_trace.TraceError, r"spawn\.request: is required when every source gate passes",
        ):
            creature_generator_trace.validate_trace(_trace([event]))

    def test_rejects_class_resolution_before_empty_base_creatures(self) -> None:
        event = _event()
        event["gates"][6]["name"] = "class_resolution"  # type: ignore[index]
        with self.assertRaisesRegex(creature_generator_trace.TraceError, r"gates\[6\]\.name"):
            creature_generator_trace.validate_trace(_trace([event]))

    def test_rejects_class_resolution_outcome_inconsistent_with_selected_class(self) -> None:
        failed = _event(
            gate_failure="class_resolution",
            rng=[{"kind": "rand", "raw": 5, "index": 1, "resolved_class": "HGame.GFemSly", "bound": 2}],
        )
        with self.assertRaisesRegex(
            creature_generator_trace.TraceError, r"must equal whether the final selected class resolves",
        ):
            creature_generator_trace.validate_trace(_trace([failed]))
        passed = _event(rng=[
            {"kind": "rand", "raw": 5, "index": 1, "resolved_class": None, "bound": 2},
        ])
        with self.assertRaisesRegex(
            creature_generator_trace.TraceError, r"must equal whether the final selected class resolves",
        ):
            creature_generator_trace.validate_trace(_trace([passed]))

    def test_validates_trace_traversal_by_frame_then_tick_order(self) -> None:
        first = _event(identity="<map>.GeneratorB", frame=0, order=1)
        second = _event(identity="<map>.GeneratorA", frame=1, order=0)
        creature_generator_trace.validate_trace(_trace([first, second]))


class CreatureGeneratorTraceComparisonContracts(unittest.TestCase):
    def test_reports_the_first_gate_difference_with_key(self) -> None:
        cpp = _trace([_event(gate_failure="camera_visible")])
        rust = _trace([_event(gate_failure="missing_first_pp")])
        report = creature_generator_trace.compare_traces(cpp, rust)
        self.assertEqual(report["status"], "mismatch")
        difference = report["first_difference"]
        self.assertEqual(difference["key"], {"identity": "<map>.CreatureGenerator0", "frame": 7, "occurrence": 0})  # type: ignore[index]
        self.assertEqual(difference["path"], "$.events[0].gates[4].passed")  # type: ignore[index]

    def test_pairs_same_event_despite_different_tick_order(self) -> None:
        cpp = _event(order=0)
        rust = _event(order=115)
        report = creature_generator_trace.compare_traces(_trace([cpp]), _trace([rust]))
        self.assertEqual(report["status"], "mismatch")
        difference = report["first_difference"]
        self.assertEqual(difference["key"], {"identity": "<map>.CreatureGenerator0", "frame": 7, "occurrence": 0})  # type: ignore[index]
        self.assertEqual(difference["path"], "$.events[0].tick.order")  # type: ignore[index]

    def test_accepts_cxx_decimal_precision_for_diagnostic_timing(self) -> None:
        cpp = _event()
        cpp["timing"]["delta_seconds"] = 0.016667  # type: ignore[index]
        rust = deepcopy(cpp)
        rust["timing"]["delta_seconds"] = 0.0166666675359  # type: ignore[index]
        self.assertEqual(
            creature_generator_trace.compare_traces(_trace([cpp]), _trace([rust])),
            {"status": "match", "first_difference": None},
        )

    def test_reports_diagnostic_timing_drift_outside_cxx_precision(self) -> None:
        cpp = _event()
        cpp["timing"]["delta_seconds"] = 0.016667  # type: ignore[index]
        rust = deepcopy(cpp)
        rust["timing"]["delta_seconds"] = 0.016669  # type: ignore[index]
        report = creature_generator_trace.compare_traces(_trace([cpp]), _trace([rust]))
        self.assertEqual(report["first_difference"]["path"], "$.events[0].timing.delta_seconds")  # type: ignore[index]

    def test_normalizes_selected_map_roots_in_object_references(self) -> None:
        cpp = _event()
        cpp["first_pp"] = _reference("Package2.PatrolPoint1", "HGame.PatrolPoint")
        cpp["spawn"] = {
            "request": _reference("HGame.GFemSly", "Core.Class"),
            "result": _reference("Package2.GFemSly5_0"),
            "published_child": _reference("Package2.GFemSly5_0"),
        }
        rust = deepcopy(cpp)
        rust["first_pp"]["path"] = "PrivetDr.PatrolPoint1"  # type: ignore[index]
        rust["spawn"]["result"]["path"] = "PrivetDr.GFemSly5_0"  # type: ignore[index]
        rust["spawn"]["published_child"]["path"] = "PrivetDr.GFemSly5_0"  # type: ignore[index]
        self.assertEqual(
            creature_generator_trace.compare_traces(_trace([cpp]), _trace([rust])),
            {"status": "match", "first_difference": None},
        )

    def test_preserves_external_reference_paths(self) -> None:
        cpp = _event()
        cpp["first_pp"] = _reference("Package2.PatrolPoint1", "HGame.PatrolPoint")
        cpp["spawn"]["request"] = _reference("HGame.GFemSly", "Core.Class")  # type: ignore[index]
        rust = deepcopy(cpp)
        rust["spawn"]["request"]["path"] = "HGame.OtherCreature"  # type: ignore[index]
        report = creature_generator_trace.compare_traces(_trace([cpp]), _trace([rust]))
        self.assertEqual(report["first_difference"]["path"], "$.events[0].spawn.request.path")  # type: ignore[index]

    def test_reports_rng_raw_before_downstream_spawn_difference(self) -> None:
        cpp = _event(rng=[{"kind": "rand", "raw": 6, "index": 0, "resolved_class": "HGame.GFemSly", "bound": 2}])
        rust = deepcopy(cpp)
        rust["rng"][0]["raw"] = 7  # type: ignore[index]
        report = creature_generator_trace.compare_traces(_trace([cpp]), _trace([rust]))
        self.assertEqual(report["first_difference"]["path"], "$.events[0].rng[0].raw")  # type: ignore[index]

    def test_cli_writes_atomic_report_for_matching_artifacts(self) -> None:
        with tempfile.TemporaryDirectory(prefix="hp2-creature-generator-trace-") as temporary:
            root = Path(temporary)
            document = _trace([_event()])
            cpp = root / "cpp.json"
            rust = root / "rust.json"
            report = root / "nested" / "report.json"
            cpp.write_text(json.dumps(document), encoding="utf-8")
            rust.write_text(json.dumps(document), encoding="utf-8")
            completed = subprocess.run(
                [sys.executable, str(REPOSITORY_ROOT / "Build" / "creature_generator_trace.py"), "--cpp-trace", str(cpp), "--rust-trace", str(rust), "--report", str(report)],
                check=False, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
            )
            self.assertEqual(completed.returncode, 0, completed.stderr)
            self.assertEqual(completed.stdout, "")
            self.assertEqual(json.loads(report.read_text(encoding="utf-8")), {"status": "match", "first_difference": None})


if __name__ == "__main__":
    unittest.main()
