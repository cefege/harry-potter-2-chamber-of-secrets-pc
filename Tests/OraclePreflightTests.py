#!/usr/bin/env python3
"""Synthetic contracts for the unavailable native oracle preflight."""
import contextlib
import io
import shutil
import subprocess
import sys
import tempfile
import threading
import unittest
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "Build"))
import game_test
import lifecycle_gate
import oracle_preflight
import parity_sweep
import static_movement_gate
import world_collision_gate


class OraclePreflightTests(unittest.TestCase):
    def test_static_gate_blocks_before_staging(self):
        with mock.patch.object(static_movement_gate, "_executable", side_effect=lambda path, label: path), \
             mock.patch.object(static_movement_gate, "stage_map", side_effect=AssertionError("staging reached")) as stage:
            with self.assertRaisesRegex(oracle_preflight.OracleUnavailableError, "native oracle.*unavailable"):
                static_movement_gate.run_static_movement_gate(
                    app=Path("synthetic-oracle"), engine_bin=Path("synthetic-engine"),
                    data_root=Path("synthetic-data"), output=Path("synthetic-output.json"))
            stage.assert_not_called()

    def test_all_callable_gates_block_before_resolution_or_staging(self):
        common = dict(app=Path("fake"), engine_bin=Path("fake-engine"),
                      data_root=Path("fake-data"), output=Path("fake-output"))
        gates = [
            (static_movement_gate, static_movement_gate.run_static_movement_gate, {}),
            (lifecycle_gate, lifecycle_gate.run_lifecycle_gate, {"fixture_root": Path("fake-fixture")}),
            (world_collision_gate, world_collision_gate.run_world_collision_gate, {}),
            (world_collision_gate, world_collision_gate.run_world_touch_gate, {}),
            (world_collision_gate, world_collision_gate.run_world_bump_gate, {}),
        ]
        for module, gate, extra in gates:
            with self.subTest(gate=gate.__name__), mock.patch.object(module, "_executable") as executable:
                with self.assertRaises(oracle_preflight.OracleUnavailableError):
                    gate(**common, **extra)
                executable.assert_not_called()

    def test_paired_api_blocks_before_resolution(self):
        with mock.patch.object(game_test, "_resolve_path") as resolve:
            with self.assertRaises(oracle_preflight.OracleUnavailableError):
                game_test.run_paired_game(app=Path("fake"), engine_bin=Path("engine"),
                    data_root=Path("data"), selected_map="fake", renderer="wgpu", ticks=1,
                    fixed_dt=0.02, timeout_seconds=1, output=Path("fake-output"))
            resolve.assert_not_called()

    def test_cpp_side_helpers_block_before_artifacts_or_launch(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            common = dict(name="cpp", executable=root / "fake", data_root=root / "data",
                          renderer="wgpu", timeout_seconds=1, start_gate=threading.Barrier(1))
            fidelity = dict(common, selected_map="fake", map_token="fake", ticks=1,
                            fixed_dt=0.02, input_script=None, replay=None,
                            artifact_root=root / "fidelity", checkpoint_manifest=root / "checkpoint",
                            rng_trace_path=None, shadow_update_trace_path=None)
            gate = dict(common, artifact_dir=root / "gate")
            cases = [
                (game_test._run_fidelity_side, fidelity),
                (static_movement_gate._run_side, dict(gate, scenario_path=root / "scenario", trace_environment="TEST")),
                (lifecycle_gate._run_side, gate),
                *[(world_collision_gate._run_side, dict(gate, scenario_path=root / "scenario", protocol=protocol))
                  for protocol in (world_collision_gate.FULL_PROTOCOL, world_collision_gate.TOUCH_PROTOCOL,
                                   world_collision_gate.BUMP_PROTOCOL)],
            ]
            for helper, kwargs in cases:
                with self.subTest(helper=helper.__module__, protocol=kwargs.get("protocol")), \
                     mock.patch.object(game_test, "run_command", side_effect=AssertionError("launch")) as launch:
                    with self.assertRaises(oracle_preflight.OracleUnavailableError):
                        helper(**kwargs)
                    launch.assert_not_called()
                    self.assertFalse((root / "fidelity").exists())
                    self.assertFalse((root / "gate").exists())

    def test_cmake_targets_header_defaults_empty_and_preserves_preseed(self):
        cmake = shutil.which("cmake")
        self.assertIsNotNone(cmake, "cmake is required for header policy contracts")
        text = (ROOT / "Build/CMake/HP2Targets.cmake").read_text(encoding="utf-8")
        marker = "# The Rust workspace builds"
        self.assertEqual(text.count(marker), 1)
        header = text.split(marker, 1)[0]
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            script = root / "header-policy.cmake"
            script.write_text(header + '\nget_property(_value CACHE HP2_CPP_ORACLE_APP PROPERTY VALUE)\n'
                              'get_property(_type CACHE HP2_CPP_ORACLE_APP PROPERTY TYPE)\n'
                              'file(WRITE "${RESULT_FILE}" "${_type}\\n${_value}")\n', encoding="utf-8")
            candidate = str(root / "reference path" / "Candidate.app")
            for value in (None, candidate):
                with self.subTest(preseed=value):
                    output = root / ("fresh.txt" if value is None else "preseed.txt")
                    command = [cmake, f"-DRESULT_FILE={output}"]
                    if value is not None:
                        command.append(f"-DHP2_CPP_ORACLE_APP:PATH={value}")
                    result = subprocess.run([*command, "-P", str(script)],
                                            text=True, capture_output=True, check=False)
                    self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                    self.assertEqual(output.read_text(encoding="utf-8"), f"PATH\n{value or ''}")

    def test_cmake_wrapper_blocked_and_unstartable_python(self):
        cmake = shutil.which("cmake")
        self.assertIsNotNone(cmake, "cmake is required for wrapper contracts")
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            build = root / "source" / "Build"
            build.mkdir(parents=True)
            shutil.copyfile(ROOT / "Build/oracle_preflight.py", build / "oracle_preflight.py")
            sentinel = root / "smoke-ran"
            (build / "smoke_maps.py").write_text(
                f"from pathlib import Path\nPath({str(sentinel)!r}).write_text('launched')\n")
            for python in (sys.executable, str(root / "nonexistent-python")):
                with self.subTest(python=python):
                    output = root / ("output-blocked" if python == sys.executable else "output-unstartable")
                    result = subprocess.run([
                        cmake, f"-DHP2_PYTHON={python}", f"-DHP2_SOURCE_DIR={build.parent}",
                        f"-DHP2_ORACLE_APP={root / 'fake-app'}", f"-DHP2_ENGINE_BIN={root / 'fake-engine'}",
                        f"-DHP2_DATA_ROOT={root / 'fake-data'}", f"-DHP2_OUTPUT_DIR={output}",
                        "-P", str(ROOT / "Build/parity_paired_maps.cmake"),
                    ], text=True, capture_output=True, check=False)
                    self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
                    self.assertIn("native oracle preflight blocked parity", result.stderr)
                    if python == sys.executable:
                        self.assertIn("native oracle parity unavailable", result.stderr)
                    self.assertFalse(output.exists())
                    self.assertFalse(sentinel.exists())

    def test_direct_clis_reject_before_launch(self):
        common = ["--app", "fake", "--engine-bin", "engine", "--data-root", "data", "--output", "out"]
        cases = [
            (game_test.main, ["game_test.py", "paired", *common, "--map", "fake", "--ticks", "1", "--fixed-dt", "0.02", "--checkpoints", "authored"]),
            (parity_sweep.main, ["parity_sweep.py", "--oracle-app", "fake", "--engine-bin", "engine", "--data-root", "data", "--smoke-report", "smoke", "--output", "out"]),
            (static_movement_gate.main, ["static_movement_gate.py", *common]),
            (lifecycle_gate.main, ["lifecycle_gate.py", *common]),
            (world_collision_gate.main, ["world_collision_gate.py", *common]),
            (world_collision_gate.main, ["world_collision_gate.py", *common, "--touch-only"]),
            (world_collision_gate.main, ["world_collision_gate.py", *common, "--bump-only"]),
        ]
        for main, argv in cases:
            with self.subTest(argv=argv), mock.patch.object(sys, "argv", argv), \
                 mock.patch.object(game_test, "run_command", side_effect=AssertionError("launch")) as launch, \
                 contextlib.redirect_stderr(io.StringIO()) as stderr:
                self.assertEqual(main(), 2)
                self.assertIn("native oracle parity unavailable", stderr.getvalue())
                launch.assert_not_called()

    def test_preflight_cli_rejects_without_inputs(self):
        with mock.patch.object(sys, "argv", ["oracle_preflight.py"]), contextlib.redirect_stderr(io.StringIO()) as stderr:
            self.assertEqual(oracle_preflight.main(), 2)
            self.assertIn("native oracle parity unavailable", stderr.getvalue())

    def test_cli_help_and_single_product_dispatch_remain_available(self):
        for main, argv in [(oracle_preflight.main, ["oracle_preflight.py", "--help"]),
                           (game_test.main, ["game_test.py", "--help"]),
                           (parity_sweep.main, ["parity_sweep.py", "--help"]),
                           (static_movement_gate.main, ["static_movement_gate.py", "--help"]),
                           (lifecycle_gate.main, ["lifecycle_gate.py", "--help"]),
                           (world_collision_gate.main, ["world_collision_gate.py", "--help"])]:
            with self.subTest(argv=argv), mock.patch.object(sys, "argv", argv), contextlib.redirect_stdout(io.StringIO()):
                with self.assertRaises(SystemExit) as result:
                    main()
                self.assertEqual(result.exception.code, 0)
        argv = ["game_test.py", "run", "--app", "fake", "--data-root", "data", "--map", "fake", "--ticks", "1", "--timeout", "1", "--log", "log"]
        with mock.patch.object(sys, "argv", argv), mock.patch.object(game_test, "run_game", return_value={"passed": True}) as run, \
             mock.patch.object(oracle_preflight, "require_native_oracle") as guard, contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(game_test.main(), 0)
            run.assert_called_once()
            guard.assert_not_called()

    def test_cmake_wrapper_checks_before_smoke_or_output(self):
        text = (ROOT / "Build/parity_paired_maps.cmake").read_text()
        self.assertLess(text.index('Build/oracle_preflight.py'), text.index('file(MAKE_DIRECTORY'))
        self.assertLess(text.index('message(FATAL_ERROR "native oracle preflight blocked parity'), text.index('Build/smoke_maps.py'))


if __name__ == "__main__":
    unittest.main(verbosity=2)
