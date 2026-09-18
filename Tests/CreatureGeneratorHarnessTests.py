#!/usr/bin/env python3
"""Synthetic paired-staging contracts for CreatureGenerator diagnostics."""

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


def _trace() -> dict[str, object]:
    return {
        "version": 1,
        "enabled": True,
        "limit": 4,
        "count": 0,
        "truncated": False,
        "events": [],
    }


class CreatureGeneratorHarnessContracts(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(prefix="hp2-creature-generator-harness-")
        self.root = Path(self.temporary.name)
        self.data_root = self.root / "Unreal"
        (self.data_root / "System").mkdir(parents=True)
        (self.data_root / "System" / "Default.ini").write_text("[Core.System]\n")
        map_path = self.data_root / "Maps" / "Test.unr"
        map_path.parent.mkdir()
        map_path.write_bytes(b"map")

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def test_paired_stages_side_specific_artifacts_and_comparison(self) -> None:
        app = self.root / "app"
        engine_bin = self.root / "engine-bin"
        app.write_bytes(b"app")
        engine_bin.write_bytes(b"engine")
        trace_dir = self.root / "generator-traces"
        trace_dir.mkdir()
        stale = trace_dir / "cpp-creature-generator-trace.json"
        stale.write_text('{"stale": true}')

        def fake_side(**kwargs: object) -> tuple[dict[str, object], dict[str, dict[str, object]], None]:
            path = kwargs["creature_generator_trace_path"]
            assert isinstance(path, Path)
            path.write_text(json.dumps(_trace()), encoding="utf-8")
            return {
                "passed": True,
                "creature_generator_trace": {"status": "valid", "path": str(path)},
            }, {}, None

        with (
            patch.object(game_test, "_bundle_executable", return_value=app),
            patch.object(game_test, "_validate_native_arm64"),
            patch.object(game_test, "_source_checkpoints", return_value=[]),
            patch.object(game_test, "_profile_for_data_root", return_value="test"),
            patch.object(game_test, "_run_fidelity_side", side_effect=fake_side),
            patch.object(
                game_test.creature_generator_trace,
                "compare_traces",
                wraps=game_test.creature_generator_trace.compare_traces,
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
                creature_generator_trace_dir=trace_dir,
            )

        persisted = json.loads((self.root / "paired.json").read_text(encoding="utf-8"))
        compare_traces.assert_called_once()
        self.assertEqual(json.loads(stale.read_text(encoding="utf-8")), _trace())
        self.assertTrue(result["passed"])
        self.assertEqual(
            persisted["creature_generator_trace"]["comparison"],
            {"status": "match", "first_difference": None},
        )
        self.assertTrue((trace_dir / "rust-creature-generator-trace.json").is_file())
        self.assertTrue((trace_dir / "creature-generator-trace-comparison.json").is_file())

    def test_cli_accepts_creature_generator_trace_directory(self) -> None:
        with patch.object(sys, "argv", [
            "game_test.py", "paired", "--app", "app", "--engine-bin", "engine",
            "--data-root", "data", "--map", "Maps/Test.unr", "--renderer", "xopengl",
            "--ticks", "1", "--fixed-dt", "0.02", "--checkpoints", "authored",
            "--output", "paired.json", "--creature-generator-trace-dir", "generator-traces",
        ]):
            arguments = game_test._arguments()
        self.assertEqual(arguments.creature_generator_trace_dir, Path("generator-traces"))

    def test_side_env_is_side_specific_and_missing_artifact_fails_leg(self) -> None:
        paths = game_test._prepare_creature_generator_trace_paths(self.root / "generator-traces")
        checkpoint_manifest = self.root / "checkpoints.json"
        checkpoint_manifest.write_text("{}")
        environments: list[dict[str, str]] = []

        def fake_run_command(*_args: object, **kwargs: object) -> dict[str, object]:
            environment = kwargs["extra_env"]
            assert isinstance(environment, dict)
            environments.append(environment)
            path = Path(environment["HP2_CREATURE_GENERATOR_TRACE"])
            if path == paths["cpp"]:
                path.write_text(json.dumps(_trace()), encoding="utf-8")
            return {"passed": True, "resources": {"script_deferred": 0, "script_deferral_reasons": {}, "script_deferral_subjects": {}}}

        common = {
            "data_root": self.data_root,
            "selected_map": "Maps/Test.unr",
            "map_token": "..\\Maps\\Test.unr",
            "renderer": "xopengl",
            "ticks": 1,
            "fixed_dt": 0.02,
            "input_script": None,
            "replay": None,
            "timeout_seconds": 1.0,
            "artifact_root": self.root / "artifacts",
            "checkpoint_manifest": checkpoint_manifest,
            "start_gate": threading.Barrier(1),
            "rng_trace_path": None,
            "shadow_update_trace_path": None,
            "creature_generator_trace_path": paths["cpp"],
        }
        with patch.object(game_test, "run_command", side_effect=fake_run_command):
            cpp_run, _, _ = game_test._run_fidelity_side(
                name="cpp", executable=self.root / "cpp", **common,
            )
            rust_run, _, _ = game_test._run_fidelity_side(
                name="rust", executable=self.root / "rust",
                **{**common, "creature_generator_trace_path": paths["rust"]},
            )
        self.assertEqual(
            [environment["HP2_CREATURE_GENERATOR_TRACE"] for environment in environments],
            [str(paths["cpp"]), str(paths["rust"])],
        )
        self.assertTrue(cpp_run["passed"])
        self.assertEqual(cpp_run["creature_generator_trace"]["status"], "valid")
        self.assertFalse(rust_run["passed"])
        self.assertEqual(rust_run["creature_generator_trace"]["status"], "missing")


if __name__ == "__main__":
    unittest.main()
