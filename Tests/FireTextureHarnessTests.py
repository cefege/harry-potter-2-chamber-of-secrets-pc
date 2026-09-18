#!/usr/bin/env python3
"""Synthetic paired-staging contracts for FireTexture trace artifacts."""

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
import fire_texture_trace  # noqa: E402
import game_test  # noqa: E402


class FireTextureHarnessContracts(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(prefix="hp2-fire-texture-harness-")
        self.root = Path(self.temporary.name)
        self.data_root = self.root / "Unreal"
        (self.data_root / "System").mkdir(parents=True)
        (self.data_root / "System" / "Default.ini").write_text("[Core.System]\n")
        map_path = self.data_root / "Maps" / "Test.unr"
        map_path.parent.mkdir()
        map_path.write_bytes(b"map")

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def test_side_paths_require_requested_artifacts_from_both_legs(self) -> None:
        paths = game_test._prepare_fire_texture_trace_paths(self.root / "fire-traces")
        checkpoint_manifest = self.root / "checkpoints.json"
        checkpoint_manifest.write_text("{}", encoding="utf-8")
        environments: list[dict[str, str]] = []

        def fake_run_command(*_args: object, **kwargs: object) -> dict[str, object]:
            environment = kwargs["extra_env"]
            assert isinstance(environment, dict)
            environments.append(environment)
            if environment["HP2_FIRE_TEXTURE_TRACE"] == str(paths["cpp"]):
                paths["cpp"].write_text(json.dumps(fire_texture_trace._synthetic_trace()), encoding="utf-8")
            return {"passed": True, "resources": {"script_deferred": 0, "script_deferral_reasons": {}, "script_deferral_subjects": {}}}

        common = {
            "data_root": self.data_root,
            "selected_map": "Maps/Test.unr",
            "map_token": "Maps/Test.unr",
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
        }
        with patch.object(game_test, "run_command", side_effect=fake_run_command):
            cpp_run, _, _ = game_test._run_fidelity_side(
                name="cpp", executable=self.root / "cpp", fire_texture_trace_path=paths["cpp"],
                **common,
            )
            rust_run, _, _ = game_test._run_fidelity_side(
                name="rust", executable=self.root / "rust", fire_texture_trace_path=paths["rust"],
                **common,
            )

        self.assertEqual(environments[0]["HP2_FIRE_TEXTURE_TRACE"], str(paths["cpp"]))
        self.assertEqual(environments[1]["HP2_FIRE_TEXTURE_TRACE"], str(paths["rust"]))
        self.assertTrue(cpp_run["passed"])
        self.assertEqual(cpp_run["fire_texture_trace"]["status"], "valid")
        self.assertEqual(
            cpp_run["fire_texture_trace"]["provenance"]["archive"]["sha256"],
            fire_texture_trace.ARCHIVE_SHA256,
        )
        self.assertFalse(rust_run["passed"])
        self.assertEqual(rust_run["fire_texture_trace"]["status"], "missing")

    def test_comparator_matches_when_both_side_artifacts_exist(self) -> None:
        paths = game_test._prepare_fire_texture_trace_paths(self.root / "fire-traces")
        trace = fire_texture_trace._synthetic_trace()
        paths["cpp"].write_text(json.dumps(trace), encoding="utf-8")
        paths["rust"].write_text(json.dumps(trace), encoding="utf-8")

        self.assertEqual(
            game_test._compare_fire_texture_traces(paths["cpp"], paths["rust"]),
            {"status": "match", "first_difference": None},
        )

    def test_paired_report_fails_when_requested_rust_artifact_is_missing(self) -> None:
        app = self.root / "app"
        engine_bin = self.root / "engine-bin"
        app.write_bytes(b"app")
        engine_bin.write_bytes(b"engine")
        trace_dir = self.root / "fire-traces"
        trace_dir.mkdir()
        stale = trace_dir / "cpp-fire-texture-trace.json"
        stale.write_text('{"stale": true}', encoding="utf-8")

        def fake_side(**kwargs: object) -> tuple[dict[str, object], dict[str, dict[str, object]], None]:
            name = kwargs["name"]
            path = kwargs["fire_texture_trace_path"]
            assert isinstance(name, str)
            assert isinstance(path, Path)
            if name == "cpp":
                path.write_text(json.dumps(fire_texture_trace._synthetic_trace()), encoding="utf-8")
            trace = game_test._fire_texture_trace_observation(path, REPOSITORY_ROOT)
            return {"passed": True, "fire_texture_trace": trace}, {}, None

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
                fire_texture_trace_dir=trace_dir,
            )

        persisted = json.loads((self.root / "paired.json").read_text(encoding="utf-8"))
        self.assertFalse(result["passed"])
        self.assertEqual(json.loads(stale.read_text(encoding="utf-8")), fire_texture_trace._synthetic_trace())
        self.assertEqual(persisted["fire_texture_trace"]["cpp"]["status"], "valid")
        self.assertEqual(persisted["fire_texture_trace"]["rust"]["status"], "missing")
        comparison = persisted["fire_texture_trace"]["comparison"]
        self.assertEqual(comparison["status"], "unavailable")
        self.assertEqual(
            comparison["first_difference"]["rust"]["code"],
            "trace.comparison_unavailable",
        )
        self.assertTrue((trace_dir / "fire-texture-trace-comparison.json").is_file())

    def test_paired_valid_trace_mismatch_fails_diagnostic_gate(self) -> None:
        app = self.root / "app"
        engine_bin = self.root / "engine-bin"
        app.write_bytes(b"app")
        engine_bin.write_bytes(b"engine")

        def fake_side(**kwargs: object) -> tuple[dict[str, object], dict[str, dict[str, object]], None]:
            name = kwargs["name"]
            path = kwargs["fire_texture_trace_path"]
            assert isinstance(name, str)
            assert isinstance(path, Path)
            trace = fire_texture_trace._synthetic_trace()
            if name == "rust":
                trace["first_speed_rand"]["value"] = 43  # type: ignore[index]
            path.write_text(json.dumps(trace), encoding="utf-8")
            return {
                "passed": True,
                "fire_texture_trace": game_test._fire_texture_trace_observation(path, REPOSITORY_ROOT),
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
                fire_texture_trace_dir=self.root / "fire-traces",
            )

        self.assertFalse(result["passed"])
        self.assertEqual(result["fire_texture_trace"]["comparison"]["status"], "mismatch")

    def test_cli_accepts_fire_texture_trace_directory(self) -> None:
        with patch.object(sys, "argv", [
            "game_test.py", "paired", "--app", "app", "--engine-bin", "engine",
            "--data-root", "data", "--map", "Maps/Test.unr", "--renderer", "xopengl",
            "--ticks", "1", "--fixed-dt", "0.02", "--checkpoints", "authored",
            "--output", "paired.json", "--fire-texture-trace-dir", "fire-traces",
        ]):
            arguments = game_test._arguments()
        self.assertEqual(arguments.fire_texture_trace_dir, Path("fire-traces"))

    def test_omitted_option_leaves_side_environment_unchanged(self) -> None:
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
                shadow_update_trace_path=None,
            )

        self.assertTrue(run["passed"])
        self.assertNotIn("HP2_FIRE_TEXTURE_TRACE", environments[0])
        self.assertNotIn("fire_texture_trace", run)


if __name__ == "__main__":
    unittest.main()
