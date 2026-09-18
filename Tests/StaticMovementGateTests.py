#!/usr/bin/env python3
"""Data-free contracts for Build/static_movement_gate.py."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
import sys
import tempfile
import threading
import unittest
from unittest.mock import patch

REPOSITORY_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(REPOSITORY_ROOT / "Build"))
import static_movement_gate  # noqa: E402


class StaticMovementGateContracts(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(prefix="hp2-static-movement-gate-contract-")
        self.root = Path(self.temporary.name)

    def tearDown(self) -> None:
        self.temporary.cleanup()

    @staticmethod
    def _actor(
        location: list[float],
        rotation: list[float],
        *,
        teleported: bool,
    ) -> dict[str, object]:
        return {
            "location": location,
            "rotation": rotation,
            "zone": {"class": "Engine.ZoneInfo", "tag": "StudyZone"},
            "b_just_teleported": teleported,
        }

    @classmethod
    def _trace(cls) -> dict[str, object]:
        origin = [-3168.0, -7760.0, -181.5]
        return {
            "version": static_movement_gate.TRACE_VERSION,
            "fixture": static_movement_gate.FIXTURE_NAME,
            "map": {
                "relative": static_movement_gate.MAP_RELATIVE.as_posix(),
                "sha256": static_movement_gate.MAP_SHA256,
            },
            "fixed_dt": static_movement_gate.EMITTED_FIXED_DT,
            "requested_ticks": static_movement_gate.REQUESTED_TICKS,
            "roles": dict(static_movement_gate.ROLE_VALUES),
            "steps": [
                {
                    "id": "resolve_origin",
                    "request": {
                        "operation": "FindSpot",
                        "requested_origin": origin,
                        "extent": [16, 16, 24],
                        "check_actors": False,
                    },
                    "result": {"found": True, "resolved_origin": origin},
                    "actor": cls._actor(origin, [0, 0, 0], teleported=True),
                },
                {
                    "id": "static_sweep",
                    "request": {
                        "operation": "MoveActor",
                        "delta": [0, 0, -64],
                        "rotation": [10, 20, 30],
                    },
                    "result": {
                        "moved": True,
                        "hit_time": 0.7421875,
                        "blocked": True,
                        "hit_actor": None,
                        "hit_primitive": "static_bsp",
                        "location": [-3168.0, -7760.0, -231.5],
                        "normal": [0, 0, 1],
                        "item": 26,
                    },
                    "actor": cls._actor([-3168.0, -7760.0, -229.5], [10, 20, 30], teleported=False),
                },
                {
                    "id": "static_far_move",
                    "request": {
                        "operation": "FarMoveActor",
                        "destination": origin,
                        "rotation": [40, 50, 60],
                    },
                    "result": {"moved": True},
                    "actor": cls._actor(origin, [40, 50, 60], teleported=True),
                },
            ],
            "random_calls": [],
        }

    def _data_root(self) -> tuple[Path, Path]:
        data_root = self.root / "data"
        (data_root / "System").mkdir(parents=True)
        (data_root / "System" / "Default.ini").write_text("[Engine.Engine]\n", encoding="utf-8")
        source_map = data_root / static_movement_gate.MAP_RELATIVE.as_posix()
        source_map.parent.mkdir(parents=True)
        source_map.write_bytes(b"synthetic TriggerTest2 static movement bytes\x00\xff")
        return data_root, source_map

    def test_normalizes_full_static_trace_and_quantizes_sweep(self) -> None:
        normalized = static_movement_gate._normalise_trace(self._trace(), "trace")
        self.assertEqual(normalized["steps"][0]["request"]["extent"], [16.0, 16.0, 24.0])
        self.assertEqual(normalized["steps"][1]["result"]["hit_time"], 0.7422)
        self.assertEqual(normalized["steps"][1]["result"]["normal"], [0.0, 0.0, 1.0])
        self.assertEqual(normalized["steps"][1]["actor"]["rotation"], [10.0, 20.0, 30.0])
        self.assertEqual(normalized["steps"][-1]["actor"]["rotation"], [40.0, 50.0, 60.0])

    def test_trace_rejects_dynamic_or_noncanonical_step_shape(self) -> None:
        raw = self._trace()
        raw["steps"][1]["actor"]["base_role"] = "blocker"
        with self.assertRaisesRegex(static_movement_gate.StaticMovementGateError, "keys are invalid"):
            static_movement_gate._normalise_trace(raw, "trace")
        raw = self._trace()
        raw["steps"][2]["id"] = "unexpected"
        with self.assertRaisesRegex(static_movement_gate.StaticMovementGateError, "static_far_move"):
            static_movement_gate._normalise_trace(raw, "trace")

    def test_trace_requires_the_validated_static_sweep_policy(self) -> None:
        raw = self._trace()
        raw["steps"][1]["result"]["hit_primitive"] = "actor"
        with self.assertRaisesRegex(static_movement_gate.StaticMovementGateError, "static_bsp"):
            static_movement_gate._normalise_trace(raw, "trace")
        raw = self._trace()
        raw["steps"][1]["result"]["item"] = 25
        with self.assertRaisesRegex(static_movement_gate.StaticMovementGateError, "item 26"):
            static_movement_gate._normalise_trace(raw, "trace")
        raw = self._trace()
        raw["steps"][1]["result"]["hit_time"] = 0.7423
        with self.assertRaisesRegex(static_movement_gate.StaticMovementGateError, "validated static BSP time"):
            static_movement_gate._normalise_trace(raw, "trace")
        raw = self._trace()
        raw["steps"][1]["result"]["location"][2] = -231.25
        with self.assertRaisesRegex(static_movement_gate.StaticMovementGateError, "result.location"):
            static_movement_gate._normalise_trace(raw, "trace")
        raw = self._trace()
        raw["steps"][1]["actor"]["location"][2] = -229.25
        with self.assertRaisesRegex(static_movement_gate.StaticMovementGateError, "actor.location"):
            static_movement_gate._normalise_trace(raw, "trace")
        raw = self._trace()
        raw["steps"][2]["request"]["destination"] = [1, 2, 3]
        with self.assertRaisesRegex(static_movement_gate.StaticMovementGateError, "resolved origin"):
            static_movement_gate._normalise_trace(raw, "trace")

    def test_scenario_error_is_reported_as_a_gate_failure(self) -> None:
        failure = {"scenario_error": {"code": "selector_cardinality", "detail": "spawn_anchor matched 0 actors"}}
        with self.assertRaisesRegex(static_movement_gate.StaticMovementGateError, "selector_cardinality"):
            static_movement_gate._normalise_trace(failure, "trace")

    def test_comparator_reports_nested_static_zone_difference(self) -> None:
        cpp = static_movement_gate._normalise_trace(self._trace(), "cpp")
        rust_raw = self._trace()
        for step in rust_raw["steps"]:
            step["actor"]["zone"]["tag"] = "OtherStaticZone"
        rust = static_movement_gate._normalise_trace(rust_raw, "rust")
        comparison = static_movement_gate.compare_traces(cpp, rust)
        self.assertEqual(comparison["status"], "mismatch")
        self.assertEqual(comparison["first_difference"]["path"], "$.steps[0].actor.zone.tag")

    def test_atomic_trace_rejects_leftover_temporary(self) -> None:
        trace_path = self.root / "trace.json"
        trace_path.write_text(json.dumps(self._trace()), encoding="utf-8")
        trace_path.with_suffix(".json.tmp").write_text("partial", encoding="utf-8")
        with self.assertRaisesRegex(static_movement_gate.StaticMovementGateError, "temporary remains"):
            static_movement_gate.read_atomic_trace(trace_path)

    def test_staging_copies_verified_map_and_writes_static_only_scenario(self) -> None:
        data_root, source_map = self._data_root()
        source_sha256 = hashlib.sha256(source_map.read_bytes()).hexdigest()
        with patch.object(static_movement_gate, "MAP_SHA256", source_sha256):
            staged = static_movement_gate.stage_map(data_root=data_root, artifact_dir=self.root / "artifacts")
            self.assertEqual(
                (staged.cpp_data_root / static_movement_gate.MAP_RELATIVE.as_posix()).read_bytes(),
                source_map.read_bytes(),
            )
            self.assertEqual(
                (staged.rust_data_root / static_movement_gate.MAP_RELATIVE.as_posix()).read_bytes(),
                source_map.read_bytes(),
            )
            scenario = json.loads(staged.scenario_path.read_text(encoding="utf-8"))
        self.assertEqual(scenario["roles"], {
            "probe": "transient:Engine.Effects",
            "spawn_anchor": "Engine.PlayerStart",
        })
        self.assertEqual(scenario["selectors"], {"spawn_anchor": {"class": "Engine.PlayerStart", "cardinality": 1}})
        self.assertFalse(scenario["transients"]["probe"]["collide_actors"])
        self.assertEqual(
            [operation["id"] for operation in scenario["operations"]],
            [step_id for step_id, _ in static_movement_gate.STEP_SPECS],
        )
        self.assertEqual(scenario["operations"][1]["rotation"], [10, 20, 30])
        self.assertEqual(scenario["operations"][2]["rotation"], [40, 50, 60])

    def test_side_launches_use_engine_specific_trace_environment(self) -> None:
        artifact_dir = self.root / "artifacts"
        scenario_path = artifact_dir / static_movement_gate.SCENARIO_FILENAME
        scenario_path.parent.mkdir(parents=True)
        scenario_path.write_text(json.dumps(static_movement_gate.scenario_document()), encoding="utf-8")
        environments: list[dict[str, str]] = []

        def fake_run_command(*_args: object, **kwargs: object) -> dict[str, object]:
            environment = kwargs["extra_env"]
            assert isinstance(environment, dict)
            environments.append(environment)
            trace_variables = {
                variable: value
                for variable, value in environment.items()
                if variable in {
                    static_movement_gate.CPP_TRACE_ENVIRONMENT,
                    static_movement_gate.RUST_TRACE_ENVIRONMENT,
                }
            }
            trace_path = Path(next(iter(trace_variables.values())))
            trace_path.write_text(json.dumps(self._trace()), encoding="utf-8")
            return {"passed": True}

        with patch.object(static_movement_gate.game_test, "build_launch_command", return_value=["engine"]) as build_command, patch.object(
            static_movement_gate.game_test, "run_command", side_effect=fake_run_command
        ):
            cpp_result = static_movement_gate._run_side(
                name="cpp",
                executable=self.root / "cpp",
                data_root=self.root / "cpp-data",
                scenario_path=scenario_path,
                artifact_dir=artifact_dir,
                renderer="xopengl",
                timeout_seconds=1.0,
                start_gate=threading.Barrier(1),
                trace_environment=static_movement_gate.CPP_TRACE_ENVIRONMENT,
            )
            rust_result = static_movement_gate._run_side(
                name="rust",
                executable=self.root / "rust",
                data_root=self.root / "rust-data",
                scenario_path=scenario_path,
                artifact_dir=artifact_dir,
                renderer="xopengl",
                timeout_seconds=1.0,
                start_gate=threading.Barrier(1),
                trace_environment=static_movement_gate.RUST_TRACE_ENVIRONMENT,
            )
        self.assertIsNotNone(cpp_result["trace"])
        self.assertIsNotNone(rust_result["trace"])
        self.assertEqual(build_command.call_args.args[2], "..\\Maps\\Studies\\TriggerTest2.unr")
        self.assertEqual(environments[0][static_movement_gate.SCENARIO_ENVIRONMENT], str(scenario_path))
        self.assertEqual(environments[0][static_movement_gate.CPP_TRACE_ENVIRONMENT], str(artifact_dir / "cpp" / static_movement_gate.TRACE_FILENAME))
        self.assertNotIn(static_movement_gate.RUST_TRACE_ENVIRONMENT, environments[0])
        self.assertEqual(environments[1][static_movement_gate.RUST_TRACE_ENVIRONMENT], str(artifact_dir / "rust" / static_movement_gate.TRACE_FILENAME))
        self.assertNotIn(static_movement_gate.CPP_TRACE_ENVIRONMENT, environments[1])

    def test_cli_is_bounded_and_has_no_dynamic_fixture_arguments(self) -> None:
        arguments = static_movement_gate._arguments([
            "--app", "cpp.app",
            "--engine-bin", "hp2rs",
            "--data-root", "data",
            "--output", "report.json",
        ])
        self.assertEqual(arguments.renderer, "xopengl")
        self.assertEqual(arguments.timeout, static_movement_gate.DEFAULT_TIMEOUT_SECONDS)
        self.assertFalse(hasattr(arguments, "fixture_root"))
        self.assertFalse(hasattr(arguments, "ucc"))
        with self.assertRaises(SystemExit):
            static_movement_gate._arguments([
                "--app", "cpp.app",
                "--engine-bin", "hp2rs",
                "--data-root", "data",
                "--output", "report.json",
                "--timeout", "301",
            ])


if __name__ == "__main__":
    raise SystemExit(unittest.main())
