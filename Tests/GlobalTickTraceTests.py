#!/usr/bin/env python3
"""Synthetic v2 contracts for Build/global_tick_trace.py; no map execution."""

from __future__ import annotations

from copy import deepcopy
from pathlib import Path
import sys
import unittest

REPOSITORY_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(REPOSITORY_ROOT / "Build"))
import global_tick_trace  # noqa: E402


def _event(
    *,
    path: str = "<map>.Actor0",
    cls: str = "Engine.Actor",
    frame: int = 7,
    ordinal: int = 1,
    relation: str = "direct",
    admission: str = "dynamic",
    rng_before: int = 50_000,
    rng_after: int = 50_000,
    tick_outcome: str = "dispatched",
    process_state_identity: str = "None",
    process_state_outcome: str = "not_called",
    tick_stamp: int = 1,
) -> dict[str, object]:
    return {
        "frame": frame,
        "ordinal": ordinal,
        "actor": {"path": path, "class": cls},
        "relation": relation,
        "root_admission": admission,
        "rng": {"before": rng_before, "after": rng_after},
        "tick": {"identity": "Tick", "outcome": tick_outcome},
        "process_state": {
            "identity": process_state_identity,
            "outcome": process_state_outcome,
        },
        "tick_stamp": tick_stamp,
    }


def _trace(events: list[dict[str, object]]) -> dict[str, object]:
    return {
        "version": 2,
        "enabled": True,
        "limit": 16,
        "count": len(events),
        "truncated": False,
        "events": events,
    }


class GlobalTickTraceValidationContracts(unittest.TestCase):
    def test_accepts_direct_and_recursive_method_entry_records(self) -> None:
        trace = _trace([
            _event(
                rng_before=50_010, rng_after=50_014, process_state_identity="HGame.Actor.Idle",
                process_state_outcome="dispatched",
            ),
            _event(
                path="<map>.Owner0", cls="HGame.Owner", ordinal=2,
                relation="owner", admission="recursive", rng_before=50_014, rng_after=50_014,
                tick_outcome="skip_already_ticked",
            ),
        ])
        self.assertIs(global_tick_trace.validate_trace(trace), trace)

    def test_rejects_noncanonical_actor_path(self) -> None:
        event = _event(path="PrivetDr.Actor0")
        with self.assertRaisesRegex(global_tick_trace.TraceError, r"actor\.path"):
            global_tick_trace.validate_trace(_trace([event]))

    def test_rejects_direct_recursive_admission(self) -> None:
        event = _event(admission="recursive")
        with self.assertRaisesRegex(global_tick_trace.TraceError, r"direct dispatch"):
            global_tick_trace.validate_trace(_trace([event]))

    def test_rejects_invalid_rng_interval_and_tick_outcome(self) -> None:
        with self.assertRaisesRegex(global_tick_trace.TraceError, r"rng\.after"):
            global_tick_trace.validate_trace(_trace([_event(rng_before=3, rng_after=2)]))
        with self.assertRaisesRegex(global_tick_trace.TraceError, r"tick\.outcome"):
            global_tick_trace.validate_trace(_trace([_event(tick_outcome="already_ticked")]))

    def test_rejects_nonmonotonic_global_ordinal(self) -> None:
        with self.assertRaisesRegex(global_tick_trace.TraceError, r"frame/ordinal"):
            global_tick_trace.validate_trace(_trace([_event(ordinal=2), _event(path="<map>.Actor1", ordinal=1)]))

    def test_rejects_unsupported_process_state_outcome(self) -> None:
        with self.assertRaisesRegex(global_tick_trace.TraceError, r"process_state\.outcome"):
            global_tick_trace.validate_trace(_trace([_event(process_state_outcome="skip_unknown")]))


class GlobalTickTraceComparisonContracts(unittest.TestCase):
    def test_rejects_reordered_global_dispatch_at_its_first_position(self) -> None:
        cpp = _trace([_event(path="<map>.A", ordinal=1), _event(path="<map>.B", ordinal=2)])
        rust = _trace([_event(path="<map>.B", ordinal=1), _event(path="<map>.A", ordinal=2)])
        report = global_tick_trace.compare_traces(cpp, rust)
        self.assertEqual(report["status"], "mismatch")
        difference = report["first_difference"]
        assert isinstance(difference, dict)
        self.assertEqual(difference["key"], {
            "frame": 7,
            "actor": {"path": "<map>.A", "class": "Engine.Actor"},
            "occurrence": 0,
        })
        self.assertEqual(difference["path"], "$.events[0].actor.path")
        self.assertEqual((difference["cpp"], difference["rust"]), ("<map>.A", "<map>.B"))

    def test_reports_first_admission_difference_at_same_position(self) -> None:
        cpp = _trace([_event()])
        rust = deepcopy(cpp)
        rust["events"][0]["root_admission"] = "recursive"  # type: ignore[index]
        rust["events"][0]["relation"] = "owner"  # type: ignore[index]
        report = global_tick_trace.compare_traces(cpp, rust)
        difference = report["first_difference"]
        assert isinstance(difference, dict)
        self.assertEqual(difference["path"], "$.events[0].relation")

    def test_reports_first_rng_interval_difference_at_same_method_entry(self) -> None:
        cpp = _trace([_event(rng_before=50_000, rng_after=50_017)])
        rust = _trace([_event(rng_before=50_000, rng_after=50_016)])
        report = global_tick_trace.compare_traces(cpp, rust)
        difference = report["first_difference"]
        assert isinstance(difference, dict)
        self.assertEqual(difference["path"], "$.events[0].rng.after")
        self.assertEqual((difference["cpp"], difference["rust"]), (50_017, 50_016))

    def test_reports_nested_tick_and_process_state_differences(self) -> None:
        cpp = _trace([
            _event(
                tick_outcome="skip_stasis", process_state_identity="HGame.Actor.Idle",
                process_state_outcome="skip_no_code",
            ),
        ])
        rust = deepcopy(cpp)
        rust["events"][0]["tick"]["outcome"] = "skip_special_pause"  # type: ignore[index]
        report = global_tick_trace.compare_traces(cpp, rust)
        difference = report["first_difference"]
        assert isinstance(difference, dict)
        self.assertEqual(difference["path"], "$.events[0].tick.outcome")

        rust = deepcopy(cpp)
        rust["events"][0]["process_state"]["identity"] = "HGame.Actor.Wander"  # type: ignore[index]
        report = global_tick_trace.compare_traces(cpp, rust)
        difference = report["first_difference"]
        assert isinstance(difference, dict)
        self.assertEqual(difference["path"], "$.events[0].process_state.identity")

    def test_reports_extra_rust_dispatch(self) -> None:
        cpp = _trace([])
        rust = _trace([_event()])
        report = global_tick_trace.compare_traces(cpp, rust)
        self.assertEqual(report["status"], "mismatch")
        difference = report["first_difference"]
        assert isinstance(difference, dict)
        self.assertEqual(difference["path"], "$.events.length")
        self.assertEqual((difference["cpp"], difference["rust"]), (0, 1))


if __name__ == "__main__":
    unittest.main()
