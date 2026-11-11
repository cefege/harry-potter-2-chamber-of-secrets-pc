#!/usr/bin/env python3
"""Exercise renderer-smoke executable selection without starting a GPU process."""

import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
ARTIFACTS = Path(os.environ["HP2_ARTIFACT_DIR"]).resolve()


class WgpuSmokeToolTests(unittest.TestCase):
    def setUp(self):
        ARTIFACTS.mkdir(parents=True, exist_ok=True)
        self.work = Path(tempfile.mkdtemp(prefix=self._testMethodName + "-", dir=ARTIFACTS))
        self.build = self.work / "build with spaces"
        self.build.mkdir()
        self.data = self.work / "data"
        (self.data / "System").mkdir(parents=True)
        (self.data / "System/Default.ini").write_text("[Engine.Engine]\nGameRenderDevice=fixture\n")

    def engine(self, name, tally=True, status=0, legacy=False, presented=2):
        binary = self.build / name
        marker = ('WgpuDrv: adapter "fixture adapter" backend Metal' if legacy else
                  'WgpuDrv: selected adapter fixture adapter')
        script = ["#!" + sys.executable, f"print({marker!r})"]
        if tally:
            script.append(f'print("WgpuDrv: frames presented={presented} occluded=0 outdated=0 other_failure=0")')
        script.append(f"raise SystemExit({status})")
        binary.write_text("\n".join(script) + "\n")
        binary.chmod(0o755)
        return binary.resolve()

    def run_smoke(self, *extra):
        env = dict(os.environ, HP2_ARTIFACT_DIR=str(self.work / "evidence"))
        command = [sys.executable, "-B", str(ROOT / "Build/run_wgpu_smoke.py"),
                   "--build-dir", str(self.build), "--data-root", str(self.data),
                   "--ticks", "2", *extra]
        result = subprocess.run(command, env=env, text=True, capture_output=True, check=False)
        (self.work / "stdout.log").write_text(result.stdout)
        (self.work / "stderr.log").write_text(result.stderr)
        self.assertTrue(result.stdout.strip(), result.stderr)
        return result.returncode, json.loads(result.stdout)

    def test_defaults_to_current_rust_product(self):
        binary = self.engine("hp2_game_rust")
        code, report = self.run_smoke()
        self.assertEqual(code, 0, report)
        self.assertEqual(report["status"], "pass")
        self.assertEqual(report["command"][0], str(binary))
        self.assertEqual(report["data"]["frame_tally"]["presented"], 2)

    def test_legacy_adapter_marker_remains_supported(self):
        self.engine("hp2_game_rust", legacy=True)
        code, report = self.run_smoke()
        self.assertEqual(code, 0, report)
        self.assertEqual(report["adapter"], "fixture adapter")
        self.assertEqual(report["backend"], "Metal")

    def test_current_marker_does_not_invent_a_backend(self):
        self.engine("hp2_game_rust")
        code, report = self.run_smoke()
        self.assertEqual(code, 0, report)
        self.assertEqual(report["adapter"], "fixture adapter")
        self.assertIsNone(report["backend"])

    def test_zero_presented_frames_remain_blocked(self):
        self.engine("hp2_game_rust", presented=0)
        code, report = self.run_smoke()
        self.assertEqual(code, 2, report)
        self.assertEqual(report["reason_code"], "renderer.zero_frames_presented")

    def test_explicit_executable_overrides_default(self):
        self.engine("hp2_game_rust", status=1)
        binary = self.engine("explicit engine")
        code, report = self.run_smoke("--engine-bin", str(binary))
        self.assertEqual(code, 0, report)
        self.assertEqual(report["command"][0], str(binary))

    def test_obsolete_filename_does_not_qualify_current_product(self):
        self.engine("hp2_game")
        code, report = self.run_smoke()
        self.assertEqual(code, 2, report)
        self.assertEqual(report["reason_code"], "renderer.binary_missing")

    def test_missing_tally_remains_blocked(self):
        self.engine("hp2_game_rust", tally=False)
        code, report = self.run_smoke()
        self.assertEqual(code, 2, report)
        self.assertEqual(report["reason_code"], "renderer.no_frame_tally")

    def test_nonzero_exit_remains_failure(self):
        self.engine("hp2_game_rust", status=1)
        code, report = self.run_smoke()
        self.assertEqual(code, 1, report)
        self.assertEqual(report["status"], "fail")


if __name__ == "__main__":
    unittest.main(verbosity=2)
