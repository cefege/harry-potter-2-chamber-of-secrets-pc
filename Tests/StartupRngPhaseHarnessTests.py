#!/usr/bin/env python3
"""Synthetic paired-staging contracts for startup RNG phase diagnostics."""
from __future__ import annotations

import json
from pathlib import Path
import sys
import tempfile
import threading
import unittest
from unittest.mock import patch

REPOSITORY_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(REPOSITORY_ROOT / "Build"))
import game_test  # noqa: E402


def _event(
    phase: str,
    *,
    boundary: str,
    before: int,
    delta: int,
) -> dict[str, object]:
    return {
        "phase": phase,
        "map_token": "Test",
        "map_url": "Maps/Test.unr",
        "load_ordinal": 1,
        "lifecycle_boundary": boundary,
        "ordinal_before": before,
        "ordinal_after": before + delta,
        "delta": delta,
    }


def _trace() -> dict[str, object]:
    fire = 512
    collision = 49_152
    residual = 8
    phases = [
        {
            "phase": "fire_init_tables",
            "map_token": "",
            "map_url": "",
            "load_ordinal": 0,
            "lifecycle_boundary": "map_loaded",
            "ordinal_before": 0,
            "ordinal_after": fire,
            "delta": fire,
        },
        _event("post_load_map", boundary="map_loaded", before=fire, delta=3),
        _event("collision_hash", boundary="map_loaded", before=fire + 3, delta=collision),
        _event(
            "post_load_map",
            boundary="bring_up_for_play_complete",
            before=fire + 3 + collision,
            delta=5,
        ),
        _event(
            "pre_first_level_tick",
            boundary="before_actor_tick",
            before=fire + 3 + collision + 5 + residual,
            delta=0,
        ),
    ]
    return {"version": 2, "enabled": True, "phases": phases}


class StartupRngPhaseHarnessContracts(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(prefix="hp2-startup-rng-phase-harness-")
        self.root = Path(self.temporary.name)
        self.data_root = self.root / "Unreal"
        (self.data_root / "System").mkdir(parents=True)
        (self.data_root / "System" / "Default.ini").write_text("[Core.System]\n")
        map_path = self.data_root / "Maps" / "Test.unr"
        map_path.parent.mkdir()
        map_path.write_bytes(b"map")

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def test_paired_stages_side_specific_artifacts_and_selects_requested_map(self) -> None:
        app = self.root / "app"
        engine_bin = self.root / "engine-bin"
        app.write_bytes(b"app")
        engine_bin.write_bytes(b"engine")
        trace_dir = self.root / "startup-rng-phase-traces"
        trace_dir.mkdir()
        stale = trace_dir / "cpp-startup-rng-phase-trace.json"
        stale.write_text('{"stale": true}', encoding="utf-8")

        def fake_side(**kwargs: object) -> tuple[dict[str, object], dict[str, dict[str, object]], None]:
            path = kwargs["startup_rng_phase_trace_path"]
            assert isinstance(path, Path)
            path.write_text(json.dumps(_trace()), encoding="utf-8")
            return {
                "passed": True,
                "startup_rng_phase_trace": {"status": "valid", "path": str(path)},
            }, {}, None

        with (
            patch.object(game_test, "_bundle_executable", return_value=app),
            patch.object(game_test, "_validate_native_arm64"),
            patch.object(game_test, "_source_checkpoints", return_value=[]),
            patch.object(game_test, "_profile_for_data_root", return_value="test"),
            patch.object(game_test, "_run_fidelity_side", side_effect=fake_side),
            patch.object(
                game_test.startup_rng_phase_trace,
                "compare_traces",
                wraps=game_test.startup_rng_phase_trace.compare_traces,
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
                startup_rng_phase_trace_dir=trace_dir,
            )

        persisted = json.loads((self.root / "paired.json").read_text(encoding="utf-8"))
        compare_traces.assert_called_once()
        self.assertEqual(compare_traces.call_args.kwargs["requested_map_token"], "Test")
        self.assertEqual(json.loads(stale.read_text(encoding="utf-8")), _trace())
        self.assertTrue(result["passed"])
        self.assertEqual(
            persisted["startup_rng_phase_trace"]["comparison"],
            {"status": "match", "first_difference": None},
        )
        self.assertTrue((trace_dir / "rust-startup-rng-phase-trace.json").is_file())
        self.assertTrue((trace_dir / "startup-rng-phase-trace-comparison.json").is_file())

    def test_cli_accepts_startup_rng_phase_trace_directory(self) -> None:
        with patch.object(sys, "argv", [
            "game_test.py", "paired", "--app", "app", "--engine-bin", "engine",
            "--data-root", "data", "--map", "Maps/Test.unr", "--renderer", "xopengl",
            "--ticks", "1", "--fixed-dt", "0.02", "--checkpoints", "authored",
            "--output", "paired.json", "--startup-rng-phase-trace-dir", "phase-traces",
        ]):
            arguments = game_test._arguments()
        self.assertEqual(arguments.startup_rng_phase_trace_dir, Path("phase-traces"))

    def test_side_env_is_side_specific_and_missing_artifact_fails_leg(self) -> None:
        path = game_test._prepare_startup_rng_phase_trace_paths(
            self.root / "startup-rng-phase-traces",
        )["cpp"]
        checkpoint_manifest = self.root / "checkpoints.json"
        checkpoint_manifest.write_text("{}", encoding="utf-8")
        environments: list[dict[str, str]] = []

        def fake_run_command(*_args: object, **kwargs: object) -> dict[str, object]:
            environment = kwargs["extra_env"]
            assert isinstance(environment, dict)
            environments.append(environment)
            return {"passed": True, "resources": {"script_deferred": 0, "script_deferral_reasons": {}, "script_deferral_subjects": {}}}

        with patch.object(game_test, "run_command", side_effect=fake_run_command):
            run, _, _ = game_test._run_fidelity_side(
                name="cpp", executable=self.root / "cpp", data_root=self.data_root,
                selected_map="Maps/Test.unr", map_token="Maps/Test.unr", renderer="xopengl",
                ticks=1, fixed_dt=0.02, input_script=None, replay=None, timeout_seconds=1.0,
                artifact_root=self.root / "artifacts", checkpoint_manifest=checkpoint_manifest,
                start_gate=threading.Barrier(1), rng_trace_path=None,
                shadow_update_trace_path=None, startup_rng_phase_trace_path=path,
            )
        self.assertEqual(environments[0]["HP2_STARTUP_RNG_PHASE_TRACE"], str(path))
        self.assertFalse(run["passed"])
        self.assertEqual(run["startup_rng_phase_trace"]["status"], "missing")


if __name__ == "__main__":
    unittest.main()
