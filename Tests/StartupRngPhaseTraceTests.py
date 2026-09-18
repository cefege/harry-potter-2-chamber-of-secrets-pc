#!/usr/bin/env python3
"""Synthetic contracts for Build/startup_rng_phase_trace.py; no map execution."""
from __future__ import annotations

from copy import deepcopy
from pathlib import Path
import sys
import unittest

REPOSITORY_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPOSITORY_ROOT / "Build"))
import startup_rng_phase_trace  # noqa: E402


def _event(
    phase: str,
    *,
    map_token: str,
    map_url: str,
    load_ordinal: int,
    lifecycle_boundary: str,
    before: int,
    delta: int,
) -> dict[str, object]:
    return {
        "phase": phase,
        "map_token": map_token,
        "map_url": map_url,
        "load_ordinal": load_ordinal,
        "lifecycle_boundary": lifecycle_boundary,
        "ordinal_before": before,
        "ordinal_after": before + delta,
        "delta": delta,
    }


def _generation(
    token: str,
    url: str,
    load_ordinal: int,
    *,
    start: int,
    collision: int = 49_152,
    residual: int = 8,
) -> list[dict[str, object]]:
    map_loaded = start + 3
    collision_end = map_loaded + collision
    bring_up_end = collision_end + 5
    return [
        _event(
            "post_load_map",
            map_token=token,
            map_url=url,
            load_ordinal=load_ordinal,
            lifecycle_boundary="map_loaded",
            before=start,
            delta=3,
        ),
        _event(
            "collision_hash",
            map_token=token,
            map_url=url,
            load_ordinal=load_ordinal,
            lifecycle_boundary="map_loaded",
            before=map_loaded,
            delta=collision,
        ),
        _event(
            "post_load_map",
            map_token=token,
            map_url=url,
            load_ordinal=load_ordinal,
            lifecycle_boundary="bring_up_for_play_complete",
            before=collision_end,
            delta=5,
        ),
        _event(
            "pre_first_level_tick",
            map_token=token,
            map_url=url,
            load_ordinal=load_ordinal,
            lifecycle_boundary="before_actor_tick",
            before=bring_up_end + residual,
            delta=0,
        ),
    ]


def _cpp_entry_then_privet_trace(
    *,
    privet_collision: int = 0,
    privet_residual: int = 8,
) -> dict[str, object]:
    phases = [
        _event(
            "fire_init_tables",
            map_token="",
            map_url="",
            load_ordinal=0,
            lifecycle_boundary="map_loaded",
            before=0,
            delta=512,
        ),
        *_generation("Entry", "Entry", 1, start=512, collision=49_152, residual=3),
        *_generation(
            "PrivetDr",
            "PrivetDr?game=HP2",
            2,
            start=540,
            collision=privet_collision,
            residual=privet_residual,
        ),
    ]
    return {"version": 2, "enabled": True, "phases": phases}


def _rust_direct_privet_trace(
    *,
    collision: int = 49_152,
    residual: int = 8,
) -> dict[str, object]:
    phases = [
        _event(
            "fire_init_tables",
            map_token="",
            map_url="",
            load_ordinal=0,
            lifecycle_boundary="map_loaded",
            before=0,
            delta=512,
        ),
        *_generation(
            "PrivetDr",
            "Maps/PrivetDr.unr",
            1,
            start=512,
            collision=collision,
            residual=residual,
        ),
    ]
    return {"version": 2, "enabled": True, "phases": phases}


class StartupRngPhaseTraceSchemaContracts(unittest.TestCase):
    def test_accepts_bootstrap_and_distinct_map_generations(self) -> None:
        trace = _cpp_entry_then_privet_trace()
        self.assertIs(startup_rng_phase_trace.validate_trace(trace), trace)

    def test_rejects_legacy_schema_and_invalid_generation_identity(self) -> None:
        legacy = _rust_direct_privet_trace()
        legacy["version"] = 1
        with self.assertRaisesRegex(startup_rng_phase_trace.TraceError, r"must equal 2"):
            startup_rng_phase_trace.validate_trace(legacy)

        invalid = _rust_direct_privet_trace()
        phases = invalid["phases"]
        assert isinstance(phases, list)
        phases[1]["map_url"] = ""
        with self.assertRaisesRegex(startup_rng_phase_trace.TraceError, r"non-empty map_token and map_url"):
            startup_rng_phase_trace.validate_trace(invalid)

    def test_rejects_unknown_lifecycle_boundary_and_bad_delta(self) -> None:
        unknown = _rust_direct_privet_trace()
        phases = unknown["phases"]
        assert isinstance(phases, list)
        phases[1]["lifecycle_boundary"] = "startup_magic"
        with self.assertRaisesRegex(startup_rng_phase_trace.TraceError, r"supported lifecycle boundary"):
            startup_rng_phase_trace.validate_trace(unknown)

        bad_delta = _rust_direct_privet_trace()
        phases = bad_delta["phases"]
        assert isinstance(phases, list)
        phases[2]["delta"] = 1
        with self.assertRaisesRegex(startup_rng_phase_trace.TraceError, r"ordinal_after - ordinal_before"):
            startup_rng_phase_trace.validate_trace(bad_delta)


class StartupRngPhaseTraceComparisonContracts(unittest.TestCase):
    def test_compares_requested_privet_generation_not_cpp_entry_bootstrap(self) -> None:
        cpp = _cpp_entry_then_privet_trace()
        rust = _rust_direct_privet_trace()
        self.assertEqual(startup_rng_phase_trace.phase_totals(cpp)["collision_hash"], 49_152)
        self.assertEqual(
            startup_rng_phase_trace.compare_traces(
                cpp,
                rust,
                requested_map_token="PrivetDr",
            ),
            {"status": "match", "first_difference": None},
        )

    def test_reports_target_leaf_delta_before_target_tick_delta(self) -> None:
        cpp = _cpp_entry_then_privet_trace(privet_collision=7, privet_residual=27)
        rust = _rust_direct_privet_trace(collision=49_152, residual=8)
        report = startup_rng_phase_trace.compare_traces(
            cpp,
            rust,
            requested_map_token="PrivetDr",
        )
        self.assertEqual(report["status"], "mismatch")
        self.assertEqual(
            report["first_difference"],
            {
                "phase": "collision_hash",
                "path": "$.process_phase_totals.collision_hash.delta",
                "cpp": 49_159,
                "rust": 49_152,
            },
        )

    def test_reports_target_residual_after_matching_leaves(self) -> None:
        cpp = _cpp_entry_then_privet_trace(privet_residual=27)
        rust = _rust_direct_privet_trace(residual=8)
        report = startup_rng_phase_trace.compare_traces(
            cpp,
            rust,
            requested_map_token="PrivetDr",
        )
        self.assertEqual(report["status"], "mismatch")
        self.assertEqual(
            report["first_difference"],
            {
                "phase": "startup_residual",
                "path": "$.target_startup_residual.delta",
                "cpp": 35,
                "rust": 16,
            },
        )

    def test_rejects_missing_requested_target_generation(self) -> None:
        with self.assertRaisesRegex(startup_rng_phase_trace.TraceError, r"requested map token 'Hogwarts'"):
            startup_rng_phase_trace.compare_traces(
                _cpp_entry_then_privet_trace(),
                _rust_direct_privet_trace(),
                requested_map_token="Hogwarts",
            )


if __name__ == "__main__":
    unittest.main()
