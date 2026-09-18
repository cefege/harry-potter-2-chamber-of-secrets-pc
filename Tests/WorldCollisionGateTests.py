#!/usr/bin/env python3
"""Data-free contracts for Build/world_collision_gate.py."""

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
import world_collision_gate  # noqa: E402


class WorldCollisionGateContracts(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(prefix="hp2-world-collision-gate-contract-")
        self.root = Path(self.temporary.name)

    def tearDown(self) -> None:
        self.temporary.cleanup()

    @staticmethod
    def _actor(
        *,
        location: list[float] | None = None,
        base_role: str | None = None,
        touching_roles: list[str] | None = None,
    ) -> dict[str, object]:
        return {
            "location": location or [1.23456, -2.34567, 3.45678],
            "rotation": [0, 16384, -1],
            "base_role": base_role,
            "zone": {"class": "Engine.ZoneInfo", "tag": "StudyZone"},
            "b_just_teleported": False,
            "delete_marked": False,
            "touching_roles": touching_roles or [],
        }

    @classmethod
    def _trace(
        cls,
        protocol: world_collision_gate.GateProtocol = world_collision_gate.FULL_PROTOCOL,
    ) -> dict[str, object]:
        steps: list[dict[str, object]] = []
        for index, (step_id, operation, parameter) in enumerate(protocol.step_specs):
            is_touch_sweep = step_id == "touch_sweep"
            is_carry = step_id == "set_base_and_carry"
            is_bump_sweep = protocol is world_collision_gate.BUMP_PROTOCOL and step_id == "block_sweep"
            probe_location = [1.23456, -2.34567, 19.45678] if is_carry else None
            blocker_location = [4.0, 5.0, 22.0] if is_carry else [4.0, 5.0, 6.0]
            events: list[str] = [] if protocol is world_collision_gate.BUMP_PROTOCOL else [step_id]
            if is_touch_sweep:
                events.extend(("Touch:probe:touch_target", "Touch:touch_target:probe"))
            if step_id == "block_sweep":
                events.append("Bump:probe:blocker")
            if is_bump_sweep:
                events = ["Bump:blocker:probe", "Bump:probe:blocker"]
            request_value = (
                list(world_collision_gate.BUMP_SWEEP_DELTA)
                if is_bump_sweep
                else [float(index), 0.0, 1.00004]
            )
            actors: dict[str, object] = {}
            for role in protocol.snapshot_roles:
                if role == "probe":
                    actors[role] = cls._actor(
                        location=probe_location,
                        base_role="blocker" if is_carry else None,
                        touching_roles=["touch_target"] if is_touch_sweep else [],
                    )
                elif role == "blocker":
                    actors[role] = cls._actor(location=blocker_location)
                else:
                    actors[role] = cls._actor(
                        location=[7.0, 8.0, 9.0],
                        touching_roles=["probe"] if is_touch_sweep else [],
                    )
            steps.append({
                "id": step_id,
                "request": {"operation": operation, parameter: request_value},
                "result": {
                    "moved": True,
                    "hit_time": (
                        1.0
                        if is_touch_sweep
                        or protocol is world_collision_gate.BUMP_PROTOCOL and not is_bump_sweep
                        else 0.078125
                        if is_bump_sweep
                        else 0.333339
                    ),
                    "blocked": step_id == "block_sweep",
                    "hit_role": "blocker" if step_id == "block_sweep" else None,
                },
                "actors": actors,
                "events": events,
            })
        return {
            "version": world_collision_gate.TRACE_VERSION,
            "fixture": protocol.fixture,
            "map": {
                "relative": world_collision_gate.MAP_RELATIVE.as_posix(),
                "sha256": world_collision_gate.MAP_SHA256,
            },
            "fixed_dt": world_collision_gate.EMITTED_FIXED_DT,
            "requested_ticks": world_collision_gate.REQUESTED_TICKS,
            "roles": dict(protocol.role_values),
            "steps": steps,
            "random_calls": [],
        }

    def _data_root(self) -> tuple[Path, Path]:
        data_root = self.root / "data"
        (data_root / "System").mkdir(parents=True)
        (data_root / "System" / "Default.ini").write_text("[Engine.Engine]\n", encoding="utf-8")
        source_map = data_root / world_collision_gate.MAP_RELATIVE.as_posix()
        source_map.parent.mkdir(parents=True)
        source_map.write_bytes(b"synthetic TriggerTest2 bytes for staging contract\x00\xff")
        return data_root, source_map

    def test_normalizes_full_role_trace_and_quantizes_vectors_and_hit_time(self) -> None:
        normalized = world_collision_gate._normalise_trace(self._trace(), "trace")
        self.assertEqual(normalized["steps"][0]["request"]["destination"], [0.0, 0.0, 1.0])
        self.assertEqual(normalized["steps"][0]["result"]["hit_time"], 0.3333)
        self.assertEqual(normalized["steps"][0]["actors"]["probe"]["location"], [1.2346, -2.3457, 3.4568])
        self.assertEqual(normalized["steps"][1]["actors"]["probe"]["touching_roles"], ["touch_target"])
        self.assertEqual(
            normalized["steps"][0]["actors"]["probe"]["zone"],
            {"class": "Engine.ZoneInfo", "tag": "StudyZone"},
        )
        self.assertEqual(normalized["steps"][-1]["actors"]["probe"]["base_role"], "blocker")

    def test_normalizes_touch_only_trace_with_required_nonblocking_reciprocal_contact(self) -> None:
        normalized = world_collision_gate._normalise_trace(
            self._trace(world_collision_gate.TOUCH_PROTOCOL),
            "touch",
            world_collision_gate.TOUCH_PROTOCOL,
        )
        self.assertEqual(normalized["fixture"], world_collision_gate.TOUCH_FIXTURE_NAME)
        self.assertEqual(normalized["roles"], world_collision_gate.TOUCH_ROLE_VALUES)
        self.assertEqual(
            [step["id"] for step in normalized["steps"]],
            ["resolve_safe_origin", "touch_sweep"],
        )
        touch = normalized["steps"][1]
        self.assertEqual(touch["result"], {
            "moved": True,
            "hit_time": 1.0,
            "blocked": False,
            "hit_role": None,
        })
        self.assertEqual(set(touch["actors"]), {"probe", "touch_target"})

    def test_normalizes_bump_trace_with_exact_ordered_callbacks(self) -> None:
        normalized = world_collision_gate._normalise_trace(
            self._trace(world_collision_gate.BUMP_PROTOCOL),
            "bump",
            world_collision_gate.BUMP_PROTOCOL,
        )
        self.assertEqual(normalized["fixture"], world_collision_gate.BUMP_FIXTURE_NAME)
        self.assertEqual(normalized["roles"], world_collision_gate.BUMP_ROLE_VALUES)
        self.assertEqual(
            [step["id"] for step in normalized["steps"]],
            ["resolve_safe_origin", "block_sweep"],
        )
        self.assertEqual(
            normalized["steps"][0]["request"],
            {"operation": "FindSpot", "destination": [0.0, 0.0, 1.0]},
        )
        bump = normalized["steps"][1]
        self.assertEqual(bump["request"], {"operation": "MoveActor", "delta": [256.0, 0.0, 0.0]})
        self.assertEqual(bump["result"]["hit_role"], "blocker")
        self.assertTrue(bump["result"]["blocked"])
        self.assertGreater(bump["result"]["hit_time"], 0.0)
        self.assertLess(bump["result"]["hit_time"], 1.0)
        self.assertEqual(bump["events"], ["Bump:blocker:probe", "Bump:probe:blocker"])
        self.assertEqual(bump["actors"]["probe"]["touching_roles"], [])
        self.assertEqual(bump["actors"]["blocker"]["touching_roles"], [])


    def test_bump_trace_rejects_noncanonical_callbacks_hit_time_and_touching(self) -> None:
        raw = self._trace(world_collision_gate.BUMP_PROTOCOL)
        raw["steps"][1]["events"].reverse()
        with self.assertRaisesRegex(world_collision_gate.WorldCollisionGateError, "exact ordered Bump callbacks"):
            world_collision_gate._normalise_trace(raw, "bump", world_collision_gate.BUMP_PROTOCOL)
        raw = self._trace(world_collision_gate.BUMP_PROTOCOL)
        raw["steps"][1]["result"]["hit_time"] = 1.0
        with self.assertRaisesRegex(world_collision_gate.WorldCollisionGateError, "must be fractional"):
            world_collision_gate._normalise_trace(raw, "bump", world_collision_gate.BUMP_PROTOCOL)
        raw = self._trace(world_collision_gate.BUMP_PROTOCOL)
        raw["steps"][1]["actors"]["probe"]["touching_roles"] = ["blocker"]
        raw["steps"][1]["actors"]["blocker"]["touching_roles"] = ["probe"]
        with self.assertRaisesRegex(world_collision_gate.WorldCollisionGateError, "must not retain a probe/blocker"):
            world_collision_gate._normalise_trace(raw, "bump", world_collision_gate.BUMP_PROTOCOL)
        raw = self._trace(world_collision_gate.BUMP_PROTOCOL)
        raw["steps"][1]["request"]["delta"] = [255, 0, 0]
        with self.assertRaisesRegex(world_collision_gate.WorldCollisionGateError, "canonical Bump sweep"):
            world_collision_gate._normalise_trace(raw, "bump", world_collision_gate.BUMP_PROTOCOL)
        raw = self._trace(world_collision_gate.BUMP_PROTOCOL)
        raw["steps"][1]["actors"]["probe"]["b_just_teleported"] = True
        with self.assertRaisesRegex(world_collision_gate.WorldCollisionGateError, "must be false before the Bump sweep"):
            world_collision_gate._normalise_trace(raw, "bump", world_collision_gate.BUMP_PROTOCOL)
        raw = self._trace(world_collision_gate.BUMP_PROTOCOL)
        raw["steps"][0]["request"]["origin"] = [0, 0, 26]
        with self.assertRaisesRegex(world_collision_gate.WorldCollisionGateError, "keys are invalid"):
            world_collision_gate._normalise_trace(raw, "bump", world_collision_gate.BUMP_PROTOCOL)

    def test_touch_trace_rejects_misordered_steps_and_blocking_hit(self) -> None:
        raw = self._trace(world_collision_gate.TOUCH_PROTOCOL)
        raw["steps"].append(raw["steps"][1].copy())
        with self.assertRaisesRegex(world_collision_gate.WorldCollisionGateError, "canonical bounded sequence"):
            world_collision_gate._normalise_trace(raw, "touch", world_collision_gate.TOUCH_PROTOCOL)
        raw = self._trace(world_collision_gate.TOUCH_PROTOCOL)
        raw["steps"][1]["result"].update({"blocked": True, "hit_role": "touch_target", "hit_time": 0.5})
        with self.assertRaisesRegex(world_collision_gate.WorldCollisionGateError, "unblocked touch sweep"):
            world_collision_gate._normalise_trace(raw, "touch", world_collision_gate.TOUCH_PROTOCOL)

    def test_trace_rejects_unknown_snapshot_identity_and_noncanonical_steps(self) -> None:
        raw = self._trace()
        raw["steps"][0]["actors"]["probe"]["slot"] = 41
        with self.assertRaisesRegex(world_collision_gate.WorldCollisionGateError, "keys are invalid"):
            world_collision_gate._normalise_trace(raw, "trace")
        raw = self._trace()
        raw["steps"][2]["id"] = "unexpected"
        with self.assertRaisesRegex(world_collision_gate.WorldCollisionGateError, "prepare_block"):
            world_collision_gate._normalise_trace(raw, "trace")
    def test_scenario_resolution_failure_is_explicitly_rejected(self) -> None:
        failure = {
            "scenario_error": {
                "code": "selector_cardinality",
                "selectors": [{
                    "id": "blocker",
                    "class": "Engine.Mover",
                    "path": None,
                    "match_count": 0,
                }],
            },
        }
        with self.assertRaisesRegex(world_collision_gate.WorldCollisionGateError, "selector_cardinality"):
            world_collision_gate._normalise_trace(failure, "trace")


    def test_comparator_reports_nested_callback_difference(self) -> None:
        cpp = world_collision_gate._normalise_trace(self._trace(), "cpp")
        rust_raw = self._trace()
        rust_raw["steps"][1]["events"].append("TraceMarker:rust")
        rust = world_collision_gate._normalise_trace(rust_raw, "rust")
        comparison = world_collision_gate.compare_traces(cpp, rust)
        self.assertEqual(comparison["status"], "mismatch")
        self.assertEqual(comparison["first_difference"]["path"], "$.steps[1].events.length")

    def test_atomic_trace_rejects_leftover_temporary(self) -> None:
        trace_path = self.root / "trace.json"
        trace_path.write_text(json.dumps(self._trace()), encoding="utf-8")
        trace_path.with_suffix(".json.tmp").write_text("partial", encoding="utf-8")
        with self.assertRaisesRegex(world_collision_gate.WorldCollisionGateError, "temporary remains"):
            world_collision_gate.read_atomic_trace(trace_path)

    def test_staging_preserves_map_bytes_and_writes_one_coordinate_free_scenario(self) -> None:
        data_root, source_map = self._data_root()
        source_sha256 = hashlib.sha256(source_map.read_bytes()).hexdigest()
        with patch.object(world_collision_gate, "MAP_SHA256", source_sha256):
            staged = world_collision_gate.stage_map(
                data_root=data_root,
                artifact_dir=self.root / "artifacts",
            )
            self.assertEqual(
                (staged.cpp_data_root / world_collision_gate.MAP_RELATIVE.as_posix()).read_bytes(),
                source_map.read_bytes(),
            )
            self.assertEqual(
                (staged.rust_data_root / world_collision_gate.MAP_RELATIVE.as_posix()).read_bytes(),
                source_map.read_bytes(),
            )
            scenario = json.loads(staged.scenario_path.read_text(encoding="utf-8"))
        self.assertEqual(scenario["selectors"]["blocker"], {"class": "Engine.Mover", "cardinality": 1})
        self.assertEqual(scenario["selectors"]["spawn_anchor"], {"class": "Engine.PlayerStart", "cardinality": 1})
        self.assertEqual(
            [operation["id"] for operation in scenario["operations"]],
            [step_id for step_id, _, _ in world_collision_gate.STEP_SPECS],
        )
        self.assertEqual(scenario["operations"][2]["placement"]["approach"], "negative_x")
        self.assertEqual(scenario["operations"][-1]["prelude"], {"op": "SetBase", "actor": "probe", "base": "blocker"})

    def test_bump_staging_writes_the_dedicated_two_step_scenario(self) -> None:
        data_root, source_map = self._data_root()
        source_sha256 = hashlib.sha256(source_map.read_bytes()).hexdigest()
        with patch.object(world_collision_gate, "MAP_SHA256", source_sha256):
            staged = world_collision_gate.stage_map(
                data_root=data_root,
                artifact_dir=self.root / "bump-artifacts",
                protocol=world_collision_gate.BUMP_PROTOCOL,
            )
            scenario = json.loads(staged.scenario_path.read_text(encoding="utf-8"))
        self.assertEqual(staged.scenario_path.name, world_collision_gate.BUMP_SCENARIO_FILENAME)
        self.assertEqual(scenario["fixture"], world_collision_gate.BUMP_FIXTURE_NAME)
        self.assertEqual(scenario["selectors"], {
            "spawn_anchor": {"class": "Engine.PlayerStart", "cardinality": 1},
        })
        self.assertEqual(scenario["transients"], {
            "probe": {
                "class": "Engine.Actor",
                "collision": {
                    "radius": 16,
                    "height": 24,
                    "collide_actors": True,
                    "collide_world": True,
                    "block_actors": True,
                    "block_players": True,
                },
            },
            "blocker": {
                "class": "Engine.Actor",
                "collision": {
                    "radius": 16,
                    "height": 24,
                    "collide_actors": True,
                    "collide_world": True,
                    "block_actors": True,
                    "block_players": True,
                },
            },
        })
        self.assertEqual(scenario["operations"], [
            {
                "id": "resolve_safe_origin",
                "op": "FindSpot",
                "actor": "probe",
                "origin": {"relative_to": "spawn_anchor", "offset": [0, 0, 26]},
            },
            {
                "id": "block_sweep",
                "op": "MoveActor",
                "actor": "probe",
                "placement": {
                    "actor": "blocker",
                    "relative_to": "safe_origin",
                    "offset": [96, 0, 0],
                },
                "delta": [256, 0, 0],
            },
        ])

    def test_side_launch_receives_shared_scenario_and_its_own_atomic_trace_path(self) -> None:
        artifact_dir = self.root / "artifacts"
        scenario_path = artifact_dir / world_collision_gate.SCENARIO_FILENAME
        scenario_path.parent.mkdir(parents=True)
        scenario_path.write_text(json.dumps(world_collision_gate.scenario_document()), encoding="utf-8")
        calls: list[dict[str, object]] = []

        def fake_run_command(*_args: object, **kwargs: object) -> dict[str, object]:
            calls.append(kwargs)
            trace_path = Path(kwargs["extra_env"][world_collision_gate.TRACE_ENVIRONMENT])
            trace_path.write_text(json.dumps(self._trace()), encoding="utf-8")
            return {"passed": True}

        with patch.object(world_collision_gate.game_test, "build_launch_command", return_value=["engine"]) as build_command, patch.object(
            world_collision_gate.game_test, "run_command", side_effect=fake_run_command
        ):
            result = world_collision_gate._run_side(
                name="cpp",
                executable=self.root / "cpp",
                data_root=self.root / "data",
                scenario_path=scenario_path,
                artifact_dir=artifact_dir,
                renderer="xopengl",
                timeout_seconds=1.0,
                start_gate=threading.Barrier(1),
            )
        self.assertIsNotNone(result["trace"])
        self.assertEqual(build_command.call_args.args[2], "..\\Maps\\Studies\\TriggerTest2.unr")
        self.assertEqual(build_command.call_args.kwargs["fixed_dt"], world_collision_gate.FIXED_DT)
        environment = calls[0]["extra_env"]
        self.assertEqual(environment[world_collision_gate.SCENARIO_ENVIRONMENT], str(scenario_path))
        self.assertEqual(environment[world_collision_gate.TRACE_ENVIRONMENT], str(artifact_dir / "cpp" / world_collision_gate.TRACE_FILENAME))

    def test_bump_side_launch_uses_the_dedicated_environment_and_trace_name(self) -> None:
        artifact_dir = self.root / "bump-artifacts"
        scenario_path = artifact_dir / world_collision_gate.BUMP_SCENARIO_FILENAME
        scenario_path.parent.mkdir(parents=True)
        scenario_path.write_text(
            json.dumps(world_collision_gate.scenario_document(world_collision_gate.BUMP_PROTOCOL)),
            encoding="utf-8",
        )
        calls: list[dict[str, object]] = []

        def fake_run_command(*_args: object, **kwargs: object) -> dict[str, object]:
            calls.append(kwargs)
            trace_path = Path(kwargs["extra_env"][world_collision_gate.BUMP_TRACE_ENVIRONMENT])
            trace_path.write_text(
                json.dumps(self._trace(world_collision_gate.BUMP_PROTOCOL)),
                encoding="utf-8",
            )
            return {"passed": True}

        with patch.object(world_collision_gate.game_test, "build_launch_command", return_value=["engine"]), patch.object(
            world_collision_gate.game_test, "run_command", side_effect=fake_run_command
        ):
            result = world_collision_gate._run_side(
                name="cpp",
                executable=self.root / "cpp",
                data_root=self.root / "data",
                scenario_path=scenario_path,
                artifact_dir=artifact_dir,
                renderer="xopengl",
                timeout_seconds=1.0,
                start_gate=threading.Barrier(1),
                protocol=world_collision_gate.BUMP_PROTOCOL,
            )
        self.assertIsNotNone(result["trace"])
        self.assertEqual(calls[0]["extra_env"], {
            world_collision_gate.BUMP_SCENARIO_ENVIRONMENT: str(scenario_path),
            world_collision_gate.BUMP_TRACE_ENVIRONMENT: str(
                artifact_dir / "cpp" / world_collision_gate.BUMP_TRACE_FILENAME
            ),
        })

    def test_cli_has_no_fixture_or_ucc_argument(self) -> None:
        arguments = world_collision_gate._arguments([
            "--app", "cpp.app",
            "--engine-bin", "hp2rs",
            "--data-root", "data",
            "--output", "report.json",
        ])
        self.assertEqual(arguments.renderer, "xopengl")
        self.assertFalse(hasattr(arguments, "fixture_root"))
        self.assertFalse(hasattr(arguments, "ucc"))
        self.assertFalse(arguments.bump_only)

    def test_cli_routes_the_bump_option_to_the_dedicated_runner(self) -> None:
        argv = [
            "--app", "cpp.app",
            "--engine-bin", "hp2rs",
            "--data-root", "data",
            "--output", "report.json",
            "--bump-only",
        ]
        with patch.object(world_collision_gate, "run_world_bump_gate", return_value={"passed": True}) as run_bump:
            self.assertEqual(world_collision_gate.main(argv), 0)
        run_bump.assert_called_once_with(
            app=Path("cpp.app"),
            engine_bin=Path("hp2rs"),
            data_root=Path("data"),
            output=Path("report.json"),
            renderer="xopengl",
            timeout_seconds=world_collision_gate.DEFAULT_TIMEOUT_SECONDS,
        )


if __name__ == "__main__":
    raise SystemExit(unittest.main())
