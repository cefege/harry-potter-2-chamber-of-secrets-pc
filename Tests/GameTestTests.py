#!/usr/bin/env python3
"""Contract tests for Build/game_test.py without opening the HP2 application."""

from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import threading
import time
from unittest.mock import patch
import unittest

REPOSITORY_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(REPOSITORY_ROOT / "Build"))
import game_test  # noqa: E402
import smoke_maps  # noqa: E402



class GameTestContracts(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(prefix="hp2-game-test-contract-")
        self.root = Path(self.temporary.name)
        self.data_root = self.root / "Unreal"
        (self.data_root / "System").mkdir(parents=True)
        (self.data_root / "System" / "Default.ini").write_text("[Core.System]\n")

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def _map(self, relative: str) -> Path:
        path = self.data_root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(b"map")
        return path

    @staticmethod
    def _observed_fidelity_state(actors: list[dict[str, object]]) -> dict[str, object]:
        return {
            field: {
                "value": actors if field == "actors" else f"{field}-value",
                "availability": {"status": "observed"},
            }
            for field, _owner in game_test.FIDELITY_FIELDS
        }

    @staticmethod
    def _successful_process() -> dict[str, object]:
        return {
            "passed": True,
            "resources": {
                "script_deferred": 0,
                "script_deferral_reasons": {},
                "script_deferral_subjects": {},
            },
        }

    def test_maps_are_bytewise_sorted_and_tokens_are_derived(self) -> None:
        self._map("Maps/zeta.unr")
        self._map("Maps/Alpha.unr")
        self._map("Other/level.UNR")
        (self.data_root / "Maps" / "ignored.txt").write_text("not a map")

        self.assertEqual(
            game_test.list_maps(self.data_root),
            [
                {"map": "Maps/Alpha.unr", "map_token": "..\\Maps\\Alpha.unr"},
                {"map": "Maps/zeta.unr", "map_token": "..\\Maps\\zeta.unr"},
                {"map": "Other/level.UNR", "map_token": "..\\Other\\level.UNR"},
            ],
        )
        selected_path, token = game_test._resolve_map(self.data_root, "Maps/Alpha.unr")
        self.assertEqual(selected_path, self.data_root / "Maps" / "Alpha.unr")
        self.assertEqual(token, "..\\Maps\\Alpha.unr")
        with self.assertRaisesRegex(game_test.GameTestError, "exact listed map"):
            game_test._resolve_map(self.data_root, "maps/Alpha.unr")

    def test_symlinked_maps_are_rejected(self) -> None:
        target = self._map("Maps/target.unr")
        (self.data_root / "Maps" / "linked.unr").symlink_to(target)
        with self.assertRaisesRegex(game_test.GameTestError, "refusing symlinked map"):
            game_test.list_maps(self.data_root)

    def test_maps_cli_emits_only_versioned_json(self) -> None:
        self._map("Maps/Entryhall_hub.unr")
        completed = subprocess.run(
            [sys.executable, str(REPOSITORY_ROOT / "Build" / "game_test.py"), "maps", "--data-root", str(self.data_root)],
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )
        self.assertEqual(
            json.loads(completed.stdout),
            {"version": 1, "maps": [{"map": "Maps/Entryhall_hub.unr", "map_token": "..\\Maps\\Entryhall_hub.unr"}]},
        )
        self.assertEqual(completed.stderr, "")

    def test_validation_rejects_invalid_inputs_before_spawn(self) -> None:
        with self.assertRaisesRegex(game_test.GameTestError, "System/Default.ini"):
            game_test.list_maps(self.root)
        self._map("Maps/Entryhall_hub.unr")
        with self.assertRaisesRegex(game_test.GameTestError, "unsupported renderer"):
            game_test.build_launch_command(Path("app"), self.data_root, "..\\Maps\\Entryhall_hub.unr", "metal", 1)
        with self.assertRaisesRegex(game_test.GameTestError, "positive integer"):
            game_test.build_launch_command(Path("app"), self.data_root, "..\\Maps\\Entryhall_hub.unr", "xopengl", 0)
        with self.assertRaisesRegex(game_test.GameTestError, "positive finite"):
            game_test.run_command([sys.executable, "-c", "raise SystemExit(0)"], repo_root=self.root, timeout_seconds=float("inf"), log_path=self.root / "never.log")

    def test_launch_command_preserves_sound_by_default(self) -> None:
        command = game_test.build_launch_command(
            Path("/application"), self.data_root, "..\\Maps\\Entryhall_hub.unr", "xopengl", 42
        )
        self.assertEqual(
            command,
            [
                "/application", f"-datadir={self.data_root}", "..\\Maps\\Entryhall_hub.unr",
                "-xopengl", "-NOFRONTEND", "-window", "-testticks=42", "-log",
            ],
        )
        self.assertNotIn("-nosound", command)
        self.assertIn(
            "-nosound",
            game_test.build_launch_command(
                Path("/application"), self.data_root, "..\\Maps\\Entryhall_hub.unr", "vulkan", 1, no_sound=True
            ),
        )

    def test_replay_launch_command_replaces_map_with_replay_source(self) -> None:
        command = game_test.build_launch_command(
            Path("/application"), self.data_root, None, "xopengl", 42,
            replay_stem="privet_rng",
        )
        self.assertEqual(command[2], "-REPLAY=privet_rng")
        self.assertNotIn("..\\Maps\\PrivetDr.unr", command)
        self.assertNotIn("--fixed-dt=0.02", command)

    def test_replay_fixture_metadata_and_home_staging_are_retained(self) -> None:
        fixture = self.root / "privet_rng.txt"
        fixture_bytes = b"tick 0.125\nidle 3\n"
        fixture.write_bytes(fixture_bytes)
        artifact_root = self.root / "paired-artifacts"

        replay = game_test._compile_replay_scenario(
            fixture, "..\\Maps\\PrivetDr.unr", artifact_root, self.root,
        )
        user_directory = game_test._stage_replay_home(self.root / "cpp-home", self.data_root, replay)
        metadata = json.loads((artifact_root / "replay" / "metadata.json").read_text())

        self.assertEqual(replay.frame_count, 4)
        self.assertEqual(replay.tick_delta, 0.125)
        self.assertEqual(metadata["url"], "..\\Maps\\PrivetDr.unr")
        self.assertEqual(metadata["fixture"]["sha256"], hashlib.sha256(fixture_bytes).hexdigest())
        self.assertEqual(metadata["compiled"]["sha256"], hashlib.sha256(replay.replay_bytes).hexdigest())
        self.assertEqual((artifact_root / "replay" / "privet_rng.rep").read_bytes(), replay.replay_bytes)
        self.assertEqual((user_directory / "privet_rng.rep").read_bytes(), replay.replay_bytes)

    def test_replay_scenario_rejects_event_first_fixture_before_staging(self) -> None:
        fixture = self.root / "event_first.txt"
        artifact_root = self.root / "artifacts"
        fixture.write_text("tick 0.05\npress K\n")

        with self.assertRaisesRegex(game_test.GameTestError, "begin with an empty frame"):
            game_test._compile_replay_scenario(
                fixture, "..\\Maps\\PrivetDr.unr", artifact_root, self.root,
            )

        self.assertFalse((artifact_root / "replay").exists())

    def test_replay_staging_uses_run_commands_actual_generated_home(self) -> None:
        fixture = self.root / "privet_rng.txt"
        fixture.write_text("tick 0.05\nidle 1\n")
        replay = game_test._compile_replay_scenario(
            fixture, "..\\Maps\\PrivetDr.unr", self.root / "artifacts", self.root,
        )
        staged_directories: list[Path] = []

        def prepare_home(home: Path) -> None:
            staged_directories.append(game_test._stage_replay_home(home, self.data_root, replay))

        result = game_test.run_command(
            [
                sys.executable,
                "-c",
                (
                    "import os, pathlib, sys; "
                    "path = pathlib.Path(os.environ['HOME']) / "
                    "'Library/Application Support/Harry Potter 2/User/privet_rng.rep'; "
                    "raise SystemExit(0 if path.is_file() else 1)"
                ),
            ],
            repo_root=self.root,
            timeout_seconds=5.0,
            log_path=self.root / "cpp" / "stdout-stderr.log",
            home_parent=self.root / "cpp",
            prepare_home=prepare_home,
        )

        self.assertTrue(result["passed"])
        self.assertEqual(staged_directories[0].parents[4], self.root / "cpp")
        self.assertEqual((staged_directories[0] / "privet_rng.rep").read_bytes(), replay.replay_bytes)

    def test_paired_replay_rejects_missing_rust_freplay_marker(self) -> None:
        fixture = self.root / "privet_rng.txt"
        fixture.write_text("tick 0.05\nidle 1\n")
        replay = game_test._compile_replay_scenario(
            fixture, "..\\Maps\\PrivetDr.unr", self.root / "artifacts", self.root,
        )
        result: dict[str, object] = {"passed": True, "replay": {}}
        log_path = self.root / "rust.log"
        log_path.write_bytes(b"hp2rs: ready\n")

        game_test._apply_rust_replay_marker(result, log_path, replay)

        self.assertFalse(result["passed"])
        self.assertEqual(result["replay"]["rust_capability"]["code"], "replay.rust_marker_absent")
        self.assertEqual(
            game_test._rust_replay_marker_observation(
                b"hp2rs: [replay.wire_v1] url=..\\Maps\\PrivetDr.unr "
                b"frames=2 first_tick=0.05 seed=1\n",
                replay,
            )["status"],
            "observed",
        )

    def test_paired_side_rejects_missing_or_nonzero_script_deferrals(self) -> None:
        checkpoint_manifest = self.root / "checkpoints.json"
        checkpoint_manifest.write_text("{}")

        def run_with(resources: dict[str, object]) -> dict[str, object]:
            with patch.object(
                game_test,
                "run_command",
                return_value={"passed": True, "resources": resources},
            ):
                result, _, _ = game_test._run_fidelity_side(
                    name="cpp",
                    executable=self.root / "cpp",
                    data_root=self.data_root,
                    selected_map="Maps/Test.unr",
                    map_token="..\\Maps\\Test.unr",
                    renderer="xopengl",
                    ticks=1,
                    fixed_dt=0.02,
                    input_script=None,
                    replay=None,
                    timeout_seconds=1.0,
                    artifact_root=self.root / "artifacts",
                    checkpoint_manifest=checkpoint_manifest,
                    start_gate=threading.Barrier(1),
                    rng_trace_path=None,
                    shadow_update_trace_path=None,
                )
            return result

        missing = run_with({})
        self.assertFalse(missing["passed"])
        self.assertEqual(
            missing["script_deferral_violation"],
            "script_deferred marker missing",
        )
        deferred = run_with({
            "script_deferred": 2,
            "script_deferral_reasons": {"native.body_deferred": 2},
            "script_deferral_subjects": {"Engine.Actor.Foo": 2},
        })
        self.assertFalse(deferred["passed"])
        self.assertEqual(deferred["script_deferral_violation"], "script_deferred=2")

    def test_supervised_command_captures_output_to_log(self) -> None:
        log_path = self.root / "logs" / "child.log"
        command = [sys.executable, "-c", "print('controlled child output')"]
        result = game_test.run_command(command, repo_root=self.root, timeout_seconds=5.0, log_path=log_path)
        self.assertTrue(result["passed"])
        self.assertEqual(result["command"], command)
        self.assertEqual(log_path.read_text(), "controlled child output\n")
        self.assertEqual(result["captured_output_bytes"], len(b"controlled child output\n"))
        self.assertNotIn("isolated_home", result)

    def test_timeout_kills_the_complete_process_group(self) -> None:
        child_pid_path = self.root / "worker.pid"
        log_path = self.root / "timeout.log"
        code = (
            "import pathlib, subprocess, sys, time; "
            "worker = subprocess.Popen([sys.executable, '-c', 'import time; time.sleep(60)']); "
            f"pathlib.Path({str(child_pid_path)!r}).write_text(str(worker.pid)); "
            "time.sleep(60)"
        )
        result = game_test.run_command(
            [sys.executable, "-c", code], repo_root=self.root, timeout_seconds=0.2, log_path=log_path
        )
        self.assertTrue(result["timed_out"])
        self.assertFalse(result["passed"])
        self.assertIn(result["process_group_cleanup"], ("sigterm", "sigkill"))
        worker_pid = int(child_pid_path.read_text())
        deadline = time.monotonic() + 2.0
        while True:
            try:
                os.kill(worker_pid, 0)
            except ProcessLookupError:
                break
            if time.monotonic() >= deadline:
                self.fail("timed-out child process group left its worker alive")
            time.sleep(0.05)

    def test_authored_checkpoints_follow_cutscript_crlf_and_comment_rules(self) -> None:
        cutscenes = self.data_root / "CutScenes"
        cutscenes.mkdir()
        (cutscenes / "00001PrivetIntro.txt").write_bytes(
            b"[Thread0Basecam]\r\n"
            b"BaseCam FollowSpline Crane Time=18 *CraneDone\r\n"
            b"BaseCam FlyTo Marker Time=2 / ignored\r\n"
            b"Sleep 1\r\n"
            b"[Thread4Ignored]\r\n"
            b"Actor Talk Ignored\r\n"
        )
        disk_rows = self.data_root / "System" / "CUTSCENES"
        disk_rows.mkdir()
        (disk_rows / "00001PrivetIntro.int").write_text(
            "[Thread_0]\n"
            "line_7=BaseCam FollowSpline Crane Time=18 *CraneDone\n"
            "line_12=BaseCam FlyTo Marker Time=2 / ignored\n"
            "line_18=Sleep 1\n"
        )
        map_path = self._map("Maps/PrivetDr.unr")
        map_path.write_bytes(b"CutScene\x0000001PrivetIntro\x00")

        checkpoints = game_test._source_checkpoints(self.data_root, map_path)

        self.assertEqual(
            [checkpoint["id"] for checkpoint in checkpoints],
            [
                "Thread0Basecam:2:before", "Thread0Basecam:2:at",
                "Thread0Basecam:2:after", "Thread0Basecam:3:before",
                "Thread0Basecam:3:at", "Thread0Basecam:3:after",
                "Thread0Basecam:4:before", "Thread0Basecam:4:at",
                "Thread0Basecam:4:after",
            ],
        )
        self.assertEqual(checkpoints[0]["source_line"]["cue"], "CraneDone")
        self.assertEqual(checkpoints[3]["source_line"]["text"], "BaseCam FlyTo Marker Time=2")
        self.assertEqual(
            checkpoints[0]["runtime_locator"],
            {
                "path": "System/CUTSCENES/00001PrivetIntro.int",
                "thread_slot": 0,
                "line_index": 7,
            },
        )
        self.assertEqual(checkpoints[3]["runtime_locator"]["line_index"], 12)

    def test_retail_only_checkpoints_use_manifest_without_prototype_sources(self) -> None:
        (self.data_root / "overlay-manifest.json").write_text(
            json.dumps({"profile": "retail-only"})
        )
        disk_rows = self.data_root / "System" / "CUTSCENES"
        disk_rows.mkdir()
        (disk_rows / "00003RetailIntro.int").write_text(
            "[Thread_0]\nline_7=Sleep 1\n"
        )
        map_path = self._map("Maps/RetailIntro.unr")
        map_path.write_bytes(b"CutScene\x00" b"00003RetailIntro\x00")

        checkpoints = game_test._source_checkpoints(self.data_root, map_path)

        self.assertFalse((self.data_root / "CutScenes").exists())
        self.assertEqual(
            game_test._profile_for_data_root(self.data_root, REPOSITORY_ROOT),
            "data-retail",
        )
        self.assertEqual(
            [checkpoint["id"] for checkpoint in checkpoints],
            ["Thread0:2:before", "Thread0:2:at", "Thread0:2:after"],
        )
        self.assertEqual(
            checkpoints[0]["runtime_locator"],
            {
                "path": "System/CUTSCENES/00003RetailIntro.int",
                "thread_slot": 0,
                "line_index": 7,
            },
        )

    def test_paired_comparison_rejects_missing_telemetry(self) -> None:
        checkpoint = {
            "source_line": {
                "path": "CutScenes/00001PrivetIntro.txt",
                "line": 12,
                "thread": "Thread0Basecam",
                "text": "Sleep 1",
            },
        }
        result = game_test._compare_checkpoint(
            checkpoint, None, None,
            {"code": "fidelity.telemetry_absent"},
            {"code": "fidelity.telemetry_absent"},
        )

        self.assertEqual(result["status"], "diverged")
        self.assertEqual(result["divergence"]["owner"], "render")
        self.assertIsNone(result["divergence"]["cpp"])
    def test_paired_comparison_requires_every_explicit_observation(self) -> None:
        checkpoint = {
            "source_line": {
                "path": "CutScenes/00001PrivetIntro.txt",
                "line": 12,
                "thread": "Thread0Basecam",
                "text": "Sleep 1",
            },
        }
        state = {
            field: {
                "value": None if field == "camera" else f"{field}-value",
                "availability": (
                    {"status": "known_null", "code": "camera.released"}
                    if field == "camera"
                    else {"status": "observed"}
                ),
            }
            for field, _owner in game_test.FIDELITY_FIELDS
        }
        self.assertEqual(
            game_test._compare_checkpoint(checkpoint, state, state, None, None)["status"],
            "match",
        )

        missing_frame_hash = dict(state)
        del missing_frame_hash["frame_hash"]
        result = game_test._compare_checkpoint(
            checkpoint, state, missing_frame_hash, None, None
        )

        self.assertEqual(result["status"], "diverged")
        self.assertEqual(result["divergence"]["field"], "frame_hash")
        self.assertEqual(
            result["divergence"]["diagnostic"]["rust"]["code"],
            "fidelity.field_missing",
        )

    def test_paired_comparison_requires_matching_runtime_locator(self) -> None:
        checkpoint = {
            "source_line": {"path": "CutScenes/test.txt", "line": 4},
            "runtime_locator": {
                "path": "System/CUTSCENES/test.int",
                "thread_slot": 2,
                "line_index": 9,
            },
        }
        result = game_test._compare_checkpoint(checkpoint, {}, {}, None, None)

        self.assertEqual(result["status"], "diverged")
        self.assertEqual(result["divergence"]["field"], "cutscene")
        self.assertEqual(
            result["divergence"]["diagnostic"]["cpp"]["code"],
            "fidelity.runtime_locator_missing",
        )

    def test_paired_comparison_allows_only_proven_shared_blocked_data(self) -> None:
        checkpoint = {"source_line": {"path": "CutScenes/test.txt", "line": 1}}
        availability = {
            "status": "blocked_data",
            "code": "asset.absent",
            "asset": "Music/Missing.ogg",
            "profile": "data-prototype",
            "proof": "C++ and Rust both reported package lookup failure",
        }
        state = {
            field: {"value": None, "availability": availability}
            for field, _owner in game_test.FIDELITY_FIELDS
        }

        result = game_test._compare_checkpoint(checkpoint, state, state, None, None)

        self.assertEqual(result["status"], "blocked_data")
        self.assertIsNone(result["divergence"])

    def test_paired_comparison_matches_equal_observed_values(self) -> None:
        checkpoint = {
            "source_line": {"path": "CutScenes/test.txt", "line": 1},
            "runtime_locator": {
                "path": "System/CUTSCENES/test.int",
                "thread_slot": 0,
                "line_index": 3,
            },
        }
        state = {
            "runtime_locator": checkpoint["runtime_locator"],
            **{
                field: {
                    "value": {"field": field},
                    "availability": {"status": "observed"},
                }
                for field, _owner in game_test.FIDELITY_FIELDS
            },
        }
        self.assertEqual(
            game_test._compare_checkpoint(checkpoint, state, state, None, None)["status"],
            "match",
        )

    def test_paired_comparison_rejects_availability_and_locator_mismatches(self) -> None:
        checkpoint = {
            "source_line": {"path": "CutScenes/test.txt", "line": 1},
            "runtime_locator": {
                "path": "System/CUTSCENES/test.int",
                "thread_slot": 0,
                "line_index": 3,
            },
        }
        observed = {
            "runtime_locator": checkpoint["runtime_locator"],
            **{
                field: {
                    "value": field,
                    "availability": {"status": "observed"},
                }
                for field, _owner in game_test.FIDELITY_FIELDS
            },
        }
        unavailable = dict(observed)
        unavailable["audio"] = {
            "value": None,
            "availability": {
                "status": "unobserved",
                "code": "fidelity.audio_not_captured",
            },
        }
        result = game_test._compare_checkpoint(checkpoint, observed, unavailable, None, None)
        self.assertEqual(result["status"], "diverged")
        self.assertEqual(result["divergence"]["field"], "audio")

        wrong_locator = dict(observed)
        wrong_locator["runtime_locator"] = {
            **checkpoint["runtime_locator"],
            "line_index": 4,
        }
        result = game_test._compare_checkpoint(checkpoint, observed, wrong_locator, None, None)
        self.assertEqual(result["status"], "diverged")
        self.assertEqual(result["divergence"]["field"], "cutscene")

    def test_two_command_cutscript_fixture_has_exact_triplets(self) -> None:
        cutscenes = self.data_root / "CutScenes"
        cutscenes.mkdir()
        (cutscenes / "00002Fixture.txt").write_bytes(
            b"[Thread0Basecam]\r\nSleep 1\r\nTalk Hello\r\n"
        )
        disk_rows = self.data_root / "System" / "CUTSCENES"
        disk_rows.mkdir()
        (disk_rows / "00002Fixture.int").write_text(
            "[Thread_0]\nline_4=Sleep 1\nline_9=Talk Hello\n"
        )
        map_path = self._map("Maps/PrivetDr.unr")
        map_path.write_bytes(b"CutScene\x0000002Fixture\x00")
        checkpoints = game_test._source_checkpoints(self.data_root, map_path)
        self.assertEqual(
            [checkpoint["id"] for checkpoint in checkpoints],
            [
                "Thread0Basecam:2:before",
                "Thread0Basecam:2:at",
                "Thread0Basecam:2:after",
                "Thread0Basecam:3:before",
                "Thread0Basecam:3:at",
                "Thread0Basecam:3:after",
            ],
        )
        self.assertEqual(
            [checkpoint["runtime_locator"]["line_index"] for checkpoint in checkpoints],
            [4, 4, 4, 9, 9, 9],
        )
        for checkpoint in checkpoints:
            state = {
                "runtime_locator": checkpoint["runtime_locator"],
                **{
                    field: {
                        "value": f"{checkpoint['id']}:{field}",
                        "availability": {"status": "observed"},
                    }
                    for field, _owner in game_test.FIDELITY_FIELDS
                },
            }
            self.assertEqual(
                game_test._compare_checkpoint(checkpoint, state, state, None, None)["status"],
                "match",
            )
    def test_script_deferral_telemetry_is_decoded_and_gates_launched_maps(self) -> None:
        resources = game_test._resource_markers(
            b"<HP2_RES> script_deferred=0\n"
            b"<HP2_RES> script_deferral_reasons=none\n"
            b"<HP2_RES> script_deferral_subjects=none\n"
        )
        self.assertEqual(
            resources,
            {
                "script_deferred": 0,
                "script_deferral_reasons": {},
                "script_deferral_subjects": {},
            },
        )
        record: dict[str, object] = {
            "resources": resources,
            "verification_mode": "game_launch",
            "duration_seconds": 1.0,
            "passed": True,
        }
        smoke_maps._apply_budget_contract(
            record,
            max_frame_ms=None,
            max_map_seconds=None,
            frame_samples_ms=[],
        )
        self.assertTrue(record["passed"])
        self.assertIsNone(record["script_deferral_violation"])
        self.assertEqual(
            game_test._script_deferral_violation({}),
            "script_deferred marker missing",
        )


        record["resources"] = game_test._resource_markers(
            b"<HP2_RES> script_deferred=2\n"
            b"<HP2_RES> script_deferral_reasons=2:native.body_deferred\n"
            b"<HP2_RES> script_deferral_subjects="
            b"2:slot%3D0x002A%20Engine.Actor.Foo\n"
        )
        record["passed"] = True
        smoke_maps._apply_budget_contract(
            record,
            max_frame_ms=None,
            max_map_seconds=None,
            frame_samples_ms=[],
        )
        self.assertFalse(record["passed"])
        self.assertEqual(record["reason_code"], "script.deferred")
        self.assertEqual(record["resources"]["script_deferral_reasons"], {"native.body_deferred": 2})
        self.assertEqual(
            record["resources"]["script_deferral_subjects"],
            {"slot=0x002A Engine.Actor.Foo": 2},
        )

    def test_structural_map_deferral_telemetry_stays_null(self) -> None:
        record: dict[str, object] = {
            "resources": {},
            "verification_mode": "package_structure",
            "duration_seconds": 0.0,
            "passed": True,
        }
        smoke_maps._apply_budget_contract(
            record,
            max_frame_ms=None,
            max_map_seconds=None,
            frame_samples_ms=[],
        )
        self.assertTrue(record["passed"])
        self.assertIsNone(record["script_deferral_violation"])
        self.assertEqual(
            {
                key: record["resources"][key]
                for key in (
                    "script_deferred",
                    "script_deferral_reasons",
                    "script_deferral_subjects",
                )
            },
            {
                "script_deferred": None,
                "script_deferral_reasons": None,
                "script_deferral_subjects": None,
            },
        )


    def test_rng_trace_artifacts_are_side_specific_and_require_attributed_v2_samples(self) -> None:
        trace_dir = self.root / "rng-traces"
        trace_dir.mkdir()
        stale = trace_dir / "cpp-rng-trace.json"
        stale.write_text('{"stale": true}')
        paths = game_test._prepare_rng_trace_paths(trace_dir)
        self.assertEqual(set(paths), {"cpp", "rust"})
        self.assertNotEqual(paths["cpp"], paths["rust"])
        self.assertFalse(stale.exists())

        document = {
            "version": 2,
            "enabled": True,
            "rand_max": 2_147_483_647,
            "seed": 1,
            "start_reason": "replay",
            "capture_available": True,
            "limit": 16_384,
            "count": 1,
            "call_count": 1,
            "truncated": False,
            "outputs": [{"ordinal": 0, "raw": 42}],
            "tick_boundaries": [{"index": 0, "phase": "startup"}],
            "samples": [{
                "ordinal": 0,
                "raw": 42,
                "tick": {"index": 0, "phase": "startup"},
                "consumer": {
                    "kind": "native",
                    "actor": "Maps.PrivetDr.GFemRav2_0",
                    "class": "HGame.GFemRav2",
                    "function": "HGame.GFemRav2.Tick",
                    "native": "engine.Rand",
                    "native_slot": 17,
                    "callsite_offset": 92,
                },
                "outer_consumer": {
                    "kind": "function",
                    "actor": "Maps.PrivetDr.Ron0",
                    "class": "HGame.HPawn",
                    "function": "Engine.Actor.PreBeginPlay",
                    "native": None,
                    "native_slot": None,
                    "callsite_offset": None,
                },
            }],
        }
        paths["rust"].write_text(json.dumps(document))
        observation = game_test._rng_trace_observation(paths["rust"], self.root)
        self.assertEqual(observation["status"], "valid")
        self.assertEqual(observation["metadata"]["seed"], 1)
        self.assertNotIn("requested_seed", observation["metadata"])
        self.assertEqual(observation["sample_count"], 1)
        diagnostic = json.loads(json.dumps(document))
        diagnostic["seed"] = 42
        diagnostic["requested_seed"] = 42
        diagnostic["start_reason"] = "diagnostic"
        self.assertIsNone(game_test._validate_rng_trace(diagnostic))
        diagnostic["requested_seed"] = True
        self.assertEqual(
            game_test._validate_rng_trace(diagnostic)["code"],
            "rng_trace.start_metadata_invalid",
        )
        diagnostic["requested_seed"] = 42
        diagnostic["start_reason"] = "replay"
        self.assertEqual(
            game_test._validate_rng_trace(diagnostic)["code"],
            "rng_trace.start_metadata_invalid",
        )
        diagnostic["start_reason"] = "unknown"
        self.assertEqual(
            game_test._validate_rng_trace(diagnostic)["code"],
            "rng_trace.start_metadata_invalid",
        )
        nested_consumer = json.loads(json.dumps(document))
        nested_sample = nested_consumer["samples"][0]
        nested_sample["tick"]["consumer"] = nested_sample.pop("consumer")
        self.assertEqual(
            game_test._validate_rng_trace(nested_consumer),
            {
                "code": "rng_trace.sample_schema_invalid",
                "detail": "sample 0 must contain ordinal, raw, tick, and consumer",
            },
        )


        missing = game_test._rng_trace_observation(paths["cpp"], self.root)
        self.assertEqual(missing["status"], "missing")
        self.assertEqual(missing["diagnostic"]["code"], "rng_trace.artifact_missing")

    def test_shadow_update_trace_artifacts_are_side_specific_and_diagnose_missing_or_invalid(self) -> None:
        trace_dir = self.root / "shadow-update-traces"
        trace_dir.mkdir()
        stale = trace_dir / "cpp-shadow-update-trace.json"
        stale.write_text('{"stale": true}')
        paths = game_test._prepare_shadow_update_trace_paths(trace_dir)

        self.assertEqual(
            paths,
            {
                "cpp": trace_dir / "cpp-shadow-update-trace.json",
                "rust": trace_dir / "rust-shadow-update-trace.json",
            },
        )
        self.assertFalse(stale.exists())
        missing = game_test._shadow_update_trace_observation(paths["cpp"], self.root)
        self.assertEqual(missing["status"], "missing")
        self.assertEqual(
            missing["diagnostic"]["code"],
            "shadow_update_trace.artifact_missing",
        )

        paths["rust"].write_text(json.dumps({
            "version": 1,
            "enabled": True,
            "limit": 4,
            "count": 0,
            "truncated": False,
            "events": [],
        }))
        valid = game_test._shadow_update_trace_observation(paths["rust"], self.root)
        self.assertEqual(valid["status"], "valid")
        self.assertEqual(valid["event_count"], 0)

        paths["rust"].write_text('{"version": 1}')
        invalid = game_test._shadow_update_trace_observation(paths["rust"], self.root)
        self.assertEqual(invalid["status"], "invalid")
        self.assertEqual(
            invalid["diagnostic"]["code"],
            "shadow_update_trace.artifact_invalid",
        )

    def test_shadow_update_trace_env_is_side_specific_and_missing_trace_fails_leg(self) -> None:
        paths = game_test._prepare_shadow_update_trace_paths(
            self.root / "shadow-update-traces",
        )
        checkpoint_manifest = self.root / "checkpoints.json"
        checkpoint_manifest.write_text("{}")
        published = {
            "version": 1,
            "enabled": True,
            "limit": 4,
            "count": 0,
            "truncated": False,
            "events": [],
        }
        environments: list[dict[str, str]] = []

        def fake_run_command(*_args: object, **kwargs: object) -> dict[str, object]:
            environment = kwargs["extra_env"]
            assert isinstance(environment, dict)
            environments.append(environment)
            path = Path(environment["HP2_SHADOW_UPDATE_TRACE"])
            if path == paths["cpp"]:
                path.write_text(json.dumps(published))
            return self._successful_process()

        with patch.object(game_test, "run_command", side_effect=fake_run_command):
            cpp_run, _, _ = game_test._run_fidelity_side(
                name="cpp",
                executable=self.root / "cpp",
                data_root=self.data_root,
                selected_map="Maps/Test.unr",
                map_token="..\\Maps\\Test.unr",
                renderer="xopengl",
                ticks=1,
                fixed_dt=0.02,
                input_script=None,
                replay=None,
                timeout_seconds=1.0,
                artifact_root=self.root / "artifacts",
                checkpoint_manifest=checkpoint_manifest,
                start_gate=threading.Barrier(1),
                rng_trace_path=None,
                shadow_update_trace_path=paths["cpp"],
                actor_slot_dump_path=None,
            )
            rust_run, _, _ = game_test._run_fidelity_side(
                name="rust",
                executable=self.root / "rust",
                data_root=self.data_root,
                selected_map="Maps/Test.unr",
                map_token="..\\Maps\\Test.unr",
                renderer="xopengl",
                ticks=1,
                fixed_dt=0.02,
                input_script=None,
                replay=None,
                timeout_seconds=1.0,
                artifact_root=self.root / "artifacts",
                checkpoint_manifest=checkpoint_manifest,
                start_gate=threading.Barrier(1),
                rng_trace_path=None,
                shadow_update_trace_path=paths["rust"],
                actor_slot_dump_path=None,
            )

        self.assertEqual(
            [environment["HP2_SHADOW_UPDATE_TRACE"] for environment in environments],
            [str(paths["cpp"]), str(paths["rust"])],
        )
        self.assertTrue(cpp_run["passed"])
        self.assertEqual(cpp_run["shadow_update_trace"]["status"], "valid")
        self.assertFalse(rust_run["passed"])
        self.assertEqual(rust_run["shadow_update_trace"]["status"], "missing")

    def test_paired_output_invokes_shadow_update_comparator_and_retains_report(self) -> None:
        self._map("Maps/Test.unr")
        app = self.root / "app"
        engine_bin = self.root / "engine-bin"
        app.write_bytes(b"app")
        engine_bin.write_bytes(b"engine")
        trace_dir = self.root / "shadow-update-traces"
        document = {
            "version": 1,
            "enabled": True,
            "limit": 4,
            "count": 0,
            "truncated": False,
            "events": [],
        }

        def fake_side(
            **kwargs: object,
        ) -> tuple[dict[str, object], dict[str, dict[str, object]], None]:
            path = kwargs["shadow_update_trace_path"]
            assert isinstance(path, Path)
            path.write_text(json.dumps(document))
            return {
                "passed": True,
                "shadow_update_trace": {"status": "valid", "path": str(path)},
            }, {}, None

        with (
            patch.object(game_test, "_bundle_executable", return_value=app),
            patch.object(game_test, "_validate_native_arm64"),
            patch.object(game_test, "_source_checkpoints", return_value=[]),
            patch.object(game_test, "_profile_for_data_root", return_value="test"),
            patch.object(game_test, "_run_fidelity_side", side_effect=fake_side),
            patch.object(
                game_test.shadow_update_trace,
                "compare_traces",
                wraps=game_test.shadow_update_trace.compare_traces,
            ) as compare_traces,
        ):
            result = game_test.run_paired_game(
                app=app,
                engine_bin=engine_bin,
                data_root=self.data_root,
                selected_map="Maps/Test.unr",
                renderer="xopengl",
                ticks=1,
                fixed_dt=0.02,
                timeout_seconds=1.0,
                output=self.root / "paired.json",
                shadow_update_trace_dir=trace_dir,
            )

        persisted = json.loads((self.root / "paired.json").read_text())
        compare_traces.assert_called_once()
        self.assertEqual(compare_traces.call_args.kwargs, {"map_stem": "Test"})
        self.assertTrue(result["passed"])
        self.assertEqual(
            persisted["shadow_update_trace"]["comparison"],
            {"status": "match", "first_difference": None},
        )
        self.assertTrue(
            (trace_dir / "shadow-update-trace-comparison.json").is_file(),
        )

    def test_paired_cli_accepts_shadow_update_trace_directory(self) -> None:
        with patch.object(sys, "argv", [
            "game_test.py",
            "paired",
            "--app", "app",
            "--engine-bin", "engine",
            "--data-root", "data",
            "--map", "Maps/Test.unr",
            "--renderer", "xopengl",
            "--ticks", "1",
            "--fixed-dt", "0.02",
            "--checkpoints", "authored",
            "--output", "paired.json",
            "--shadow-update-trace-dir", "shadow-update-traces",
        ]):
            arguments = game_test._arguments()

        self.assertEqual(
            arguments.shadow_update_trace_dir,
            Path("shadow-update-traces"),
        )

    def test_paired_rng_trace_limit_cli_is_strict_and_requires_trace_directory(self) -> None:
        base = [
            "game_test.py", "paired", "--app", "app", "--engine-bin", "engine",
            "--data-root", "data", "--map", "Maps/Test.unr", "--renderer", "xopengl",
            "--ticks", "1", "--fixed-dt", "0.02", "--checkpoints", "authored",
            "--output", "paired.json", "--rng-trace-dir", "rng-traces",
        ]
        with patch.object(sys, "argv", [*base, "--rng-trace-limit", "262144"]):
            self.assertEqual(game_test._arguments().rng_trace_limit, 262_144)
        for limit in ("", "+1", "-1", "0x1", "0", "262145"):
            with self.subTest(limit=limit), patch.object(
                sys, "argv", [*base, "--rng-trace-limit", limit],
            ):
                with self.assertRaises(SystemExit):
                    game_test._arguments()

        self._map("Maps/Test.unr")
        app = self.root / "app"
        engine_bin = self.root / "engine-bin"
        app.write_bytes(b"app")
        engine_bin.write_bytes(b"engine")
        with self.assertRaisesRegex(
            game_test.GameTestError, "--rng-trace-limit requires --rng-trace-dir",
        ):
            game_test.run_paired_game(
                app=app, engine_bin=engine_bin, data_root=self.data_root,
                selected_map="Maps/Test.unr", renderer="xopengl", ticks=1,
                fixed_dt=0.02, timeout_seconds=1.0, output=self.root / "paired.json",
                rng_trace_limit=65_536,
            )


    def test_paired_rng_trace_limit_is_shared_and_opt_in(self) -> None:
        checkpoint_manifest = self.root / "checkpoints.json"
        checkpoint_manifest.write_text("{}")
        trace_paths = {
            "cpp": self.root / "cpp-rng-trace.json",
            "rust": self.root / "rust-rng-trace.json",
        }
        environments: list[dict[str, str]] = []

        def fake_run_command(*_args: object, **kwargs: object) -> dict[str, object]:
            environment = kwargs["extra_env"]
            assert isinstance(environment, dict)
            environments.append(environment)
            return self._successful_process()

        with patch.object(game_test, "run_command", side_effect=fake_run_command):
            for side in ("cpp", "rust"):
                game_test._run_fidelity_side(
                    name=side, executable=self.root / side, data_root=self.data_root,
                    selected_map="Maps/Test.unr", map_token="..\\Maps\\Test.unr",
                    renderer="xopengl", ticks=1, fixed_dt=0.02, input_script=None,
                    replay=None, timeout_seconds=1.0, artifact_root=self.root / "artifacts",
                    checkpoint_manifest=checkpoint_manifest, start_gate=threading.Barrier(1),
                    rng_trace_path=trace_paths[side], shadow_update_trace_path=None,
                    rng_trace_limit=65_536,
                )
            game_test._run_fidelity_side(
                name="cpp", executable=self.root / "cpp", data_root=self.data_root,
                selected_map="Maps/Test.unr", map_token="..\\Maps\\Test.unr",
                renderer="xopengl", ticks=1, fixed_dt=0.02, input_script=None,
                replay=None, timeout_seconds=1.0, artifact_root=self.root / "artifacts",
                checkpoint_manifest=checkpoint_manifest, start_gate=threading.Barrier(1),
                rng_trace_path=trace_paths["cpp"], shadow_update_trace_path=None,
            )

        self.assertEqual(
            [environment["HP2_RNG_TRACE_LIMIT"] for environment in environments[:2]],
            ["65536", "65536"],
        )
        self.assertNotIn("HP2_RNG_TRACE_LIMIT", environments[2])
        self.assertIn("HP2_RNG_TRACE", environments[2])


    def test_paired_rng_trace_limit_report_records_effective_request(self) -> None:
        self._map("Maps/Test.unr")
        app = self.root / "app"
        engine_bin = self.root / "engine-bin"
        app.write_bytes(b"app")
        engine_bin.write_bytes(b"engine")
        trace_dir = self.root / "rng-traces"
        trace_limits: list[object] = []

        def fake_side(
            **kwargs: object,
        ) -> tuple[dict[str, object], dict[str, dict[str, object]], None]:
            trace_limits.append(kwargs["rng_trace_limit"])
            path = kwargs["rng_trace_path"]
            assert isinstance(path, Path)
            return {
                "passed": True,
                "rng_trace": {"status": "valid", "path": str(path)},
            }, {}, None

        with (
            patch.object(game_test, "_bundle_executable", return_value=app),
            patch.object(game_test, "_validate_native_arm64"),
            patch.object(game_test, "_source_checkpoints", return_value=[]),
            patch.object(game_test, "_profile_for_data_root", return_value="test"),
            patch.object(game_test, "_run_fidelity_side", side_effect=fake_side),
        ):
            result = game_test.run_paired_game(
                app=app, engine_bin=engine_bin, data_root=self.data_root,
                selected_map="Maps/Test.unr", renderer="xopengl", ticks=1,
                fixed_dt=0.02, timeout_seconds=1.0, output=self.root / "paired.json",
                rng_trace_dir=trace_dir, rng_trace_limit=65_536,
            )

        persisted = json.loads((self.root / "paired.json").read_text())
        self.assertEqual(trace_limits, [65_536, 65_536])
        self.assertEqual(result["rng_trace"]["requested_limit"], 65_536)
        self.assertEqual(persisted["rng_trace"]["requested_limit"], 65_536)


    def test_paired_seed_cli_is_strict_decimal(self) -> None:
        base = [
            "game_test.py", "paired", "--app", "app", "--engine-bin", "engine",
            "--data-root", "data", "--map", "Maps/Test.unr", "--renderer", "xopengl",
            "--ticks", "1", "--fixed-dt", "0.02", "--checkpoints", "authored",
            "--output", "paired.json",
        ]
        with patch.object(sys, "argv", [*base, "--rng-seed", "2147483647"]):
            self.assertEqual(game_test._arguments().rng_seed, 2_147_483_647)
        for seed in ("", "+1", "-1", "0x1", "2147483648"):
            with self.subTest(seed=seed), patch.object(sys, "argv", [*base, "--rng-seed", seed]):
                with self.assertRaises(SystemExit):
                    game_test._arguments()
        self.assertEqual(game_test._effective_rng_seed(0), 1)
        self.assertEqual(game_test._effective_rng_seed(game_test.RNG_SEED_MAX), 1)
        self.assertEqual(game_test._effective_rng_seed(42), 42)

    def test_paired_seed_uses_cpp_environment_and_rust_argument(self) -> None:
        checkpoint_manifest = self.root / "checkpoints.json"
        checkpoint_manifest.write_text("{}")
        launches: list[tuple[list[str], dict[str, str]]] = []

        def fake_run_command(command: list[str], **kwargs: object) -> dict[str, object]:
            environment = kwargs["extra_env"]
            assert isinstance(environment, dict)
            launches.append((command, environment))
            return self._successful_process()

        with patch.object(game_test, "run_command", side_effect=fake_run_command):
            cpp_run, _, _ = game_test._run_fidelity_side(
                name="cpp", executable=self.root / "cpp", data_root=self.data_root,
                selected_map="Maps/Test.unr", map_token="..\\Maps\\Test.unr",
                renderer="xopengl", ticks=1, fixed_dt=0.02, input_script=None,
                replay=None, timeout_seconds=1.0, artifact_root=self.root / "artifacts",
                checkpoint_manifest=checkpoint_manifest, start_gate=threading.Barrier(1),
                rng_trace_path=None, shadow_update_trace_path=None, rng_seed=42,
            )
            rust_run, _, _ = game_test._run_fidelity_side(
                name="rust", executable=self.root / "rust", data_root=self.data_root,
                selected_map="Maps/Test.unr", map_token="..\\Maps\\Test.unr",
                renderer="xopengl", ticks=1, fixed_dt=0.02, input_script=None,
                replay=None, timeout_seconds=1.0, artifact_root=self.root / "artifacts",
                checkpoint_manifest=checkpoint_manifest, start_gate=threading.Barrier(1),
                rng_trace_path=None, shadow_update_trace_path=None, rng_seed=42,
            )

        cpp_command, cpp_environment = launches[0]
        rust_command, rust_environment = launches[1]
        self.assertNotIn("--rng-seed=42", cpp_command)
        self.assertEqual(cpp_environment["HP2_RNG_SEED"], "42")
        self.assertIn("--rng-seed=42", rust_command)
        self.assertNotIn("HP2_RNG_SEED", rust_environment)
        expected_metadata = {"seed": 42, "start_reason": "diagnostic"}
        self.assertEqual(cpp_run["rng_seed"], expected_metadata)
        self.assertEqual(rust_run["rng_seed"], expected_metadata)

    def test_paired_seed_rejects_replay_before_launch(self) -> None:
        self._map("Maps/Test.unr")
        app = self.root / "app"
        engine_bin = self.root / "engine-bin"
        replay = self.root / "seed.rep.txt"
        app.write_bytes(b"app")
        engine_bin.write_bytes(b"engine")
        replay.write_text("tick 0.05\nidle 1\n")

        with self.assertRaisesRegex(game_test.GameTestError, "only for map paired runs"):
            game_test.run_paired_game(
                app=app, engine_bin=engine_bin, data_root=self.data_root,
                selected_map="Maps/Test.unr", renderer="xopengl", ticks=1,
                fixed_dt=0.02, timeout_seconds=1.0, output=self.root / "paired.json",
                replay_fixture=replay, replay_url="..\\Maps\\Test.unr", rng_seed=42,
            )

    def test_actor_slot_dump_artifacts_are_side_specific_and_required(self) -> None:
        dump_dir = self.root / "actor-slots"
        dump_dir.mkdir()
        stale = dump_dir / "cpp-actor-slots.json"
        stale.write_text('{"stale": true}')
        paths = game_test._prepare_actor_slot_dump_paths(dump_dir)

        self.assertEqual(
            paths,
            {
                "cpp": dump_dir / "cpp-actor-slots.json",
                "rust": dump_dir / "rust-actor-slots.json",
            },
        )
        self.assertFalse(stale.exists())
        document = {
            "version": 1,
            "checkpoints": [
                {
                    "checkpoint": "post_deserialize_raw", "sequence": "raw",
                    "i_first_net_relevant_actor": None,
                    "i_first_dynamic_actor": None, "actors": [],
                },
                {
                    "checkpoint": "post_startup_before_first_tick",
                    "sequence": "rearranged", "i_first_net_relevant_actor": 0,
                    "i_first_dynamic_actor": 0, "actors": [],
                },
                {
                    "checkpoint": "post_startup", "sequence": "rearranged",
                    "i_first_net_relevant_actor": 0,
                    "i_first_dynamic_actor": 0, "actors": [],
                },
            ],
        }
        paths["cpp"].write_text(json.dumps(document))
        observation = game_test._actor_slot_dump_observation(paths["cpp"], self.root)
        self.assertEqual(observation["status"], "valid")
        self.assertEqual(observation["metadata"]["checkpoints"], [
            "post_deserialize_raw",
            "post_startup_before_first_tick",
            "post_startup",
        ])
        missing = game_test._actor_slot_dump_observation(paths["rust"], self.root)
        self.assertEqual(missing["status"], "missing")
        self.assertEqual(
            missing["diagnostic"]["code"], "actor_slot_dump.artifact_missing",
        )

    def test_actor_slot_dump_env_is_side_specific_and_missing_dump_fails_leg(self) -> None:
        paths = game_test._prepare_actor_slot_dump_paths(self.root / "actor-slots")
        checkpoint_manifest = self.root / "checkpoints.json"
        checkpoint_manifest.write_text("{}")
        published = {
            "version": 1,
            "checkpoints": [
                {
                    "checkpoint": "post_deserialize_raw", "sequence": "raw",
                    "i_first_net_relevant_actor": None,
                    "i_first_dynamic_actor": None, "actors": [],
                },
                {
                    "checkpoint": "post_startup_before_first_tick",
                    "sequence": "rearranged", "i_first_net_relevant_actor": 0,
                    "i_first_dynamic_actor": 0, "actors": [],
                },
                {
                    "checkpoint": "post_startup", "sequence": "rearranged",
                    "i_first_net_relevant_actor": 0,
                    "i_first_dynamic_actor": 0, "actors": [],
                },
            ],
        }
        environments: list[dict[str, str]] = []

        def fake_run_command(*_args: object, **kwargs: object) -> dict[str, object]:
            environment = kwargs["extra_env"]
            assert isinstance(environment, dict)
            environments.append(environment)
            path = Path(environment["HP2_ACTOR_SLOT_DUMP"])
            if path == paths["cpp"]:
                path.write_text(json.dumps(published))
            return self._successful_process()

        with patch.object(game_test, "run_command", side_effect=fake_run_command):
            cpp_run, _, _ = game_test._run_fidelity_side(
                name="cpp",
                executable=self.root / "cpp",
                data_root=self.data_root,
                selected_map="Maps/Test.unr",
                map_token="..\\Maps\\Test.unr",
                renderer="xopengl",
                ticks=1,
                fixed_dt=0.02,
                input_script=None,
                replay=None,
                timeout_seconds=1.0,
                artifact_root=self.root / "artifacts",
                checkpoint_manifest=checkpoint_manifest,
                start_gate=threading.Barrier(1),
                rng_trace_path=None,
                shadow_update_trace_path=None,
                actor_slot_dump_path=paths["cpp"],
            )
            rust_run, _, _ = game_test._run_fidelity_side(
                name="rust",
                executable=self.root / "rust",
                data_root=self.data_root,
                selected_map="Maps/Test.unr",
                map_token="..\\Maps\\Test.unr",
                renderer="xopengl",
                ticks=1,
                fixed_dt=0.02,
                input_script=None,
                replay=None,
                timeout_seconds=1.0,
                artifact_root=self.root / "artifacts",
                checkpoint_manifest=checkpoint_manifest,
                start_gate=threading.Barrier(1),
                rng_trace_path=None,
                shadow_update_trace_path=None,
                actor_slot_dump_path=paths["rust"],
            )

        self.assertEqual(
            [environment["HP2_ACTOR_SLOT_DUMP"] for environment in environments],
            [str(paths["cpp"]), str(paths["rust"])],
        )
        self.assertTrue(cpp_run["passed"])
        self.assertEqual(cpp_run["actor_slot_dump"]["status"], "valid")
        self.assertFalse(rust_run["passed"])
        self.assertEqual(rust_run["actor_slot_dump"]["status"], "missing")

    def test_paired_output_retains_actor_slot_dump_metadata_and_failure(self) -> None:
        self._map("Maps/Test.unr")
        app = self.root / "app"
        engine_bin = self.root / "engine-bin"
        app.write_bytes(b"app")
        engine_bin.write_bytes(b"engine")
        dump_dir = self.root / "actor-slots"
        cpp_dump = {
            "status": "valid",
            "path": str(dump_dir / "cpp-actor-slots.json"),
            "metadata": {"version": 1, "checkpoints": []},
        }
        rust_dump = {
            "status": "missing",
            "path": str(dump_dir / "rust-actor-slots.json"),
            "diagnostic": {
                "code": "actor_slot_dump.artifact_missing",
                "detail": "actor-slot dump was not published",
            },
        }

        def fake_side(**kwargs: object) -> tuple[dict[str, object], dict[str, dict[str, object]], None]:
            name = kwargs["name"]
            assert isinstance(name, str)
            dump = cpp_dump if name == "cpp" else rust_dump
            return {"passed": name == "cpp", "actor_slot_dump": dump}, {}, None

        with (
            patch.object(game_test, "_bundle_executable", return_value=app),
            patch.object(game_test, "_validate_native_arm64"),
            patch.object(game_test, "_source_checkpoints", return_value=[]),
            patch.object(game_test, "_profile_for_data_root", return_value="test"),
            patch.object(game_test, "_run_fidelity_side", side_effect=fake_side),
        ):
            result = game_test.run_paired_game(
                app=app,
                engine_bin=engine_bin,
                data_root=self.data_root,
                selected_map="Maps/Test.unr",
                renderer="xopengl",
                ticks=1,
                fixed_dt=0.02,
                timeout_seconds=1.0,
                output=self.root / "paired.json",
                actor_slot_dump_dir=dump_dir,
            )

        persisted = json.loads((self.root / "paired.json").read_text())
        self.assertFalse(result["passed"])
        self.assertFalse(persisted["passed"])
        self.assertEqual(result["actor_slot_dump"]["directory"], str(dump_dir.resolve()))
        self.assertEqual(
            persisted["actor_slot_dump"]["rust"]["diagnostic"]["code"],
            "actor_slot_dump.artifact_missing",
        )

    def test_paired_actor_slot_semantic_join_keeps_raw_ids_diagnostic_only(self) -> None:
        self._map("Maps/Test.unr")
        app = self.root / "app"
        engine_bin = self.root / "engine-bin"
        app.write_bytes(b"app")
        engine_bin.write_bytes(b"engine")
        dump_dir = self.root / "actor-slots"

        def actor(slot: int, path: str) -> dict[str, object]:
            is_cpp = path.startswith("Package2.ActorShadow")
            return {
                "slot": slot,
                "path": path,
                "is_null": False,
                "class": "HGame.CutScript",
                "owner_path": "Package2.CutScene0",
                "owner_class": "HGame.CutScene",
                "role": 2,
                "remote_role": 1,
                "is_deleted": False,
                "is_pending_kill": False,
                "effective_shadow_class": None,
                "spawn_origin": (
                    "level_dynamic_slot"
                    if is_cpp else "HPawn.PreBeginPlay.ShadowClass"
                ),
                "spawn_owner_path": "Package2.CutScene0",
                "spawn_owner_class": "HGame.CutScene",
                "spawn_request_id": None,
                "spawn_order": 3,
                "transform": {"location": [1, 2, 3]},
                "state": {"name": "Running"},
            }

        cpp_document = {
            "version": 1,
            "checkpoints": [
                {
                    "checkpoint": "post_deserialize_raw",
                    "sequence": "raw",
                    "i_first_net_relevant_actor": None,
                    "i_first_dynamic_actor": None,
                    "actors": [actor(0, "Package2.ActorShadow_600")],
                },
                {
                    "checkpoint": "post_startup_before_first_tick",
                    "sequence": "rearranged",
                    "i_first_net_relevant_actor": 0,
                    "i_first_dynamic_actor": 0,
                    "actors": [actor(0, "Package2.ActorShadow_600")],
                },
                {
                    "checkpoint": "post_startup",
                    "sequence": "rearranged",
                    "i_first_net_relevant_actor": 0,
                    "i_first_dynamic_actor": 0,
                    "actors": [actor(0, "Package2.ActorShadow_600")],
                },
            ],
        }
        rust_document = {
            "version": 1,
            "checkpoints": [
                {
                    "checkpoint": "post_deserialize_raw",
                    "sequence": "raw",
                    "i_first_net_relevant_actor": None,
                    "i_first_dynamic_actor": None,
                    "actors": [actor(0, "ScriptSpawn_74")],
                },
                {
                    "checkpoint": "post_startup_before_first_tick",
                    "sequence": "rearranged",
                    "i_first_net_relevant_actor": 0,
                    "i_first_dynamic_actor": 0,
                    "actors": [actor(0, "ScriptSpawn_74")],
                },
                {
                    "checkpoint": "post_startup",
                    "sequence": "rearranged",
                    "i_first_net_relevant_actor": 0,
                    "i_first_dynamic_actor": 0,
                    "actors": [actor(0, "ScriptSpawn_74")],
                },
            ],
        }

        def fake_side(
            **kwargs: object,
        ) -> tuple[dict[str, object], dict[str, dict[str, object]], None]:
            name = kwargs["name"]
            dump_path = kwargs["actor_slot_dump_path"]
            assert isinstance(name, str)
            assert isinstance(dump_path, Path)
            document = cpp_document if name == "cpp" else rust_document
            dump_path.write_text(json.dumps(document))
            return {
                "passed": True,
                "actor_slot_dump": {"status": "valid", "path": str(dump_path)},
            }, {}, None

        with (
            patch.object(game_test, "_bundle_executable", return_value=app),
            patch.object(game_test, "_validate_native_arm64"),
            patch.object(game_test, "_source_checkpoints", return_value=[]),
            patch.object(game_test, "_profile_for_data_root", return_value="test"),
            patch.object(game_test, "_run_fidelity_side", side_effect=fake_side),
        ):
            result = game_test.run_paired_game(
                app=app,
                engine_bin=engine_bin,
                data_root=self.data_root,
                selected_map="Maps/Test.unr",
                renderer="xopengl",
                ticks=1,
                fixed_dt=0.02,
                timeout_seconds=1.0,
                output=self.root / "paired.json",
                actor_slot_dump_dir=dump_dir,
            )

        persisted = json.loads((self.root / "paired.json").read_text())
        join = persisted["actor_slot_semantic_join"]
        self.assertTrue(result["passed"])
        self.assertTrue(persisted["passed"])
        self.assertEqual(join["status"], "observed")
        raw_checkpoint = next(
            checkpoint
            for checkpoint in join["checkpoints"]
            if checkpoint["checkpoint"] == "post_deserialize_raw"
        )
        unique_group = raw_checkpoint["groups"][0]
        self.assertEqual(
            unique_group["cpp"]["actors"][0]["path"],
            "Package2.ActorShadow_600",
        )
        self.assertEqual(unique_group["rust"]["actors"][0]["path"], "ScriptSpawn_74")
        self.assertNotIn("path", unique_group["semantic"])
        self.assertEqual(unique_group["semantic"]["transform"], {"location": [1, 2, 3]})
        self.assertEqual(unique_group["semantic"]["state"], {"name": "Running"})
        self.assertNotIn("spawn_origin", unique_group["semantic"])
        self.assertEqual(
            unique_group["cpp"]["actors"][0]["spawn_origin"],
            "level_dynamic_slot",
        )
        self.assertEqual(
            unique_group["rust"]["actors"][0]["spawn_origin"],
            "HPawn.PreBeginPlay.ShadowClass",
        )
        self.assertTrue(join["matches"])
        for checkpoint in join["checkpoints"]:
            self.assertTrue(checkpoint["matches"])
            self.assertTrue(checkpoint["order_match"])
            self.assertTrue(checkpoint["null_positions_match"])
            self.assertTrue(checkpoint["boundaries_match"])
            self.assertEqual(checkpoint["multiset_difference"], [])
            self.assertEqual(checkpoint["ambiguous_groups"], [])

    def test_actor_slot_join_rejects_order_null_and_boundary_mismatches(self) -> None:
        def actor(slot: int, actor_class: str) -> dict[str, object]:
            return {
                "slot": slot,
                "path": f"Package.Actor{slot}",
                "class": actor_class,
                "is_null": False,
            }

        def null(slot: int) -> dict[str, object]:
            return {
                "slot": slot,
                "path": None,
                "class": None,
                "is_null": True,
            }

        def row(
            actors: list[dict[str, object]], first_net: int, first_dynamic: int,
        ) -> dict[str, object]:
            return {
                "checkpoint": "post_startup",
                "sequence": "rearranged",
                "i_first_net_relevant_actor": first_net,
                "i_first_dynamic_actor": first_dynamic,
                "actors": actors,
            }

        cpp = row([actor(0, "Class.A"), null(1), actor(2, "Class.B")], 1, 2)
        wrong_order = row(
            [actor(0, "Class.B"), null(1), actor(2, "Class.A")], 1, 2,
        )
        order = game_test._actor_slot_semantic_checkpoint(
            "post_startup", cpp, wrong_order,
        )
        self.assertEqual(order["multiset_difference"], [])
        self.assertFalse(order["order_match"])
        self.assertFalse(order["matches"])

        wrong_null = row(
            [actor(0, "Class.A"), actor(1, "Class.B"), null(2)], 1, 2,
        )
        nulls = game_test._actor_slot_semantic_checkpoint(
            "post_startup", cpp, wrong_null,
        )
        self.assertFalse(nulls["null_positions_match"])
        self.assertFalse(nulls["matches"])

        wrong_boundaries = row(
            [actor(0, "Class.A"), null(1), actor(2, "Class.B")], 0, 1,
        )
        boundaries = game_test._actor_slot_semantic_checkpoint(
            "post_startup", cpp, wrong_boundaries,
        )
        self.assertTrue(boundaries["order_match"])
        self.assertFalse(boundaries["boundaries_match"])
        self.assertFalse(boundaries["matches"])

    def test_paired_actor_transition_ledger_stages_side_specific_artifacts_and_compares(self) -> None:
        self._map("Maps/Test.unr")
        app = self.root / "app"
        engine_bin = self.root / "engine-bin"
        app.write_bytes(b"app")
        engine_bin.write_bytes(b"engine")
        trace_dir = self.root / "actor-transition-ledgers"
        trace_dir.mkdir()
        stale = trace_dir / "cpp-actor-transition-ledger.json"
        stale.write_text('{"stale": true}')
        document = {
            "version": 2,
            "enabled": True,
            "limit": 4,
            "count": 0,
            "truncated": False,
            "owners": [],
        }

        def fake_side(
            **kwargs: object,
        ) -> tuple[dict[str, object], dict[str, dict[str, object]], None]:
            path = kwargs["actor_transition_ledger_path"]
            assert isinstance(path, Path)
            path.write_text(json.dumps(document))
            return {
                "passed": True,
                "actor_transition_ledger": {"status": "valid", "path": str(path)},
            }, {}, None

        with (
            patch.object(game_test, "_bundle_executable", return_value=app),
            patch.object(game_test, "_validate_native_arm64"),
            patch.object(game_test, "_source_checkpoints", return_value=[]),
            patch.object(game_test, "_profile_for_data_root", return_value="test"),
            patch.object(game_test, "_run_fidelity_side", side_effect=fake_side),
            patch.object(
                game_test.actor_transition_ledger,
                "compare_traces",
                wraps=game_test.actor_transition_ledger.compare_traces,
            ) as compare_traces,
        ):
            result = game_test.run_paired_game(
                app=app,
                engine_bin=engine_bin,
                data_root=self.data_root,
                selected_map="Maps/Test.unr",
                renderer="xopengl",
                ticks=1,
                fixed_dt=0.02,
                timeout_seconds=1.0,
                output=self.root / "paired.json",
                actor_transition_ledger_dir=trace_dir,
            )

        persisted = json.loads((self.root / "paired.json").read_text())
        compare_traces.assert_called_once()
        self.assertEqual(json.loads(stale.read_text()), document)
        self.assertTrue(result["passed"])
        self.assertEqual(
            persisted["actor_transition_ledger"]["comparison"],
            {"status": "match", "first_difference": None},
        )
        self.assertTrue(
            (trace_dir / "rust-actor-transition-ledger.json").is_file(),
        )
        self.assertTrue(
            (trace_dir / "actor-transition-ledger-comparison.json").is_file(),
        )

    def test_paired_cli_accepts_actor_transition_ledger_directory(self) -> None:
        with patch.object(sys, "argv", [
            "game_test.py",
            "paired",
            "--app", "app",
            "--engine-bin", "engine",
            "--data-root", "data",
            "--map", "Maps/Test.unr",
            "--renderer", "xopengl",
            "--ticks", "1",
            "--fixed-dt", "0.02",
            "--checkpoints", "authored",
            "--output", "paired.json",
            "--actor-transition-ledger-dir", "actor-transition-ledgers",
        ]):
            arguments = game_test._arguments()

        self.assertEqual(
            arguments.actor_transition_ledger_dir,
            Path("actor-transition-ledgers"),
        )

    def test_actor_transition_ledger_env_is_side_specific_and_missing_artifact_fails_leg(self) -> None:
        paths = game_test._prepare_actor_transition_ledger_paths(
            self.root / "actor-transition-ledgers",
        )
        checkpoint_manifest = self.root / "checkpoints.json"
        checkpoint_manifest.write_text("{}")
        published = {
            "version": 2,
            "enabled": True,
            "limit": 4,
            "count": 0,
            "truncated": False,
            "owners": [],
        }
        environments: list[dict[str, str]] = []

        def fake_run_command(*_args: object, **kwargs: object) -> dict[str, object]:
            environment = kwargs["extra_env"]
            assert isinstance(environment, dict)
            environments.append(environment)
            path = Path(environment["HP2_ACTOR_TRANSITION_LEDGER"])
            if path == paths["cpp"]:
                path.write_text(json.dumps(published))
            return self._successful_process()

        with patch.object(game_test, "run_command", side_effect=fake_run_command):
            cpp_run, _, _ = game_test._run_fidelity_side(
                name="cpp",
                executable=self.root / "cpp",
                data_root=self.data_root,
                selected_map="Maps/Test.unr",
                map_token="..\\Maps\\Test.unr",
                renderer="xopengl",
                ticks=1,
                fixed_dt=0.02,
                input_script=None,
                replay=None,
                timeout_seconds=1.0,
                artifact_root=self.root / "artifacts",
                checkpoint_manifest=checkpoint_manifest,
                start_gate=threading.Barrier(1),
                rng_trace_path=None,
                shadow_update_trace_path=None,
                actor_transition_ledger_path=paths["cpp"],
            )
            rust_run, _, _ = game_test._run_fidelity_side(
                name="rust",
                executable=self.root / "rust",
                data_root=self.data_root,
                selected_map="Maps/Test.unr",
                map_token="..\\Maps\\Test.unr",
                renderer="xopengl",
                ticks=1,
                fixed_dt=0.02,
                input_script=None,
                replay=None,
                timeout_seconds=1.0,
                artifact_root=self.root / "artifacts",
                checkpoint_manifest=checkpoint_manifest,
                start_gate=threading.Barrier(1),
                rng_trace_path=None,
                shadow_update_trace_path=None,
                actor_transition_ledger_path=paths["rust"],
            )

        self.assertEqual(
            [environment["HP2_ACTOR_TRANSITION_LEDGER"] for environment in environments],
            [str(paths["cpp"]), str(paths["rust"])],
        )
        self.assertTrue(cpp_run["passed"])
        self.assertEqual(cpp_run["actor_transition_ledger"]["status"], "valid")
        self.assertFalse(rust_run["passed"])
        self.assertEqual(rust_run["actor_transition_ledger"]["status"], "missing")

if __name__ == "__main__":
    unittest.main()
