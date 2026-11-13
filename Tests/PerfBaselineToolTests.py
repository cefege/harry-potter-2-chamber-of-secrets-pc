#!/usr/bin/env python3
"""Offline contracts for the performance baseline process harness."""
import argparse
import contextlib
import io
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "Build"))
import perf_baseline as perf

TIMING = "<HP2_PERF> {} n=10 mean_ms=2.0 p50_ms=2.0 p95_ms=3.0 p99_ms=4.0 max_ms=5.0\n"
PRESENT = "WgpuDrv: frames presented=9 occluded=0 outdated=0 other_failure=0\n"
MARKERS = ("<HP2_VERIFY> input=isolated\n<HP2_VERIFY> audio=loopback rate=44100\n"
           "<HP2_VERIFY> visuals=tick\n")


class PerfBaselineToolTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(dir=os.environ["HP2_ARTIFACT_DIR"])
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.ini = self.root / "engine.ini"
        self.ini.write_text("FrameRateLimit=1000\n")
        self.log = self.root / "sample.log"
        self.text = TIMING.format("tick") + TIMING.format("interval") + PRESENT + MARKERS
        self.result = {"passed": True, "exit_status": 0, "timed_out": False, "failure_markers": []}

    def sample(self, deterministic=False):
        args = argparse.Namespace(log=self.log, data_root=self.root, renderer="wgpu", engine_bin=self.root,
                                  one="Entry", ticks=10, timeout=5, deterministic_clock=deterministic)
        def run_game(**kwargs):
            self.call = kwargs
            self.log.write_text(self.text)
            return self.result
        out = io.StringIO()
        with mock.patch.object(perf, "uncapped_ini", return_value=self.ini), \
             mock.patch.object(perf.game_test, "run_game", side_effect=run_game), contextlib.redirect_stdout(out):
            status = perf.one(args)
        return status, json.loads(out.getvalue().removeprefix("PERF_SAMPLE "))

    def test_default_launch_and_opt_in_use_current_api(self):
        status, _ = self.sample()
        self.assertEqual(status, 0)
        self.assertTrue(self.call["no_sound"])
        self.assertEqual(self.call["extra_env"], {"HP2_FRAME_TIMING": "1"})
        self.assertEqual(self.call["fixed_dt"], 1 / 60)
        status, sample = self.sample(True)
        self.assertEqual(status, 0)
        self.assertFalse(self.call["no_sound"])
        self.assertEqual(self.call["extra_env"], {"HP2_FRAME_TIMING": "1", "HP2_DETERMINISTIC_VERIFICATION": "1"})
        self.assertEqual(sample["verification"]["audio"], ["loopback"])

    def test_markers_missing_wrong_or_conflicting_fail_only_when_requested(self):
        original = self.text
        for key, good, bad in (("input", "isolated", "physical"), ("audio", "loopback", "device"),
                               ("visuals", "tick", "wall")):
            marker = f"<HP2_VERIFY> {key}={good}"
            for replacement in ("", f"<HP2_VERIFY> {key}={bad}", marker + f"\n<HP2_VERIFY> {key}={bad}"):
                with self.subTest(key=key, replacement=replacement):
                    self.text = original.replace(marker, replacement)
                    self.assertEqual(self.sample(True)[0], 1)
                    self.assertEqual(self.sample(False)[0], 0)

    def test_process_presentation_and_timing_rejects(self):
        original = self.text
        for change in (lambda s: s.replace(PRESENT, ""),
                       lambda s: s.replace("presented=9", "presented=0"),
                       lambda s: s.replace(TIMING.format("interval"), ""),
                       lambda s: s.replace("n=10", "n=0"),
                       lambda s: s.replace("mean_ms=2.0", "mean_ms=0.0")):
            with self.subTest(change=change):
                self.text = change(original)
                self.assertEqual(self.sample()[0], 1)
        self.text = original
        for key, value in (("exit_status", 2), ("timed_out", True), ("failure_markers", ["error"]), ("passed", False)):
            with self.subTest(key=key):
                self.result[key] = value
                self.assertEqual(self.sample()[0], 1)
                self.result = {"passed": True, "exit_status": 0, "timed_out": False, "failure_markers": []}

    def test_presentation_rejects_any_adverse_device_tally(self):
        for key in ("occluded", "outdated", "other_failure"):
            with self.subTest(key=key):
                self.text = TIMING.format("tick") + TIMING.format("interval") + PRESENT.replace(f"{key}=0", f"{key}=1")
                status, sample = self.sample()
                self.assertEqual(status, 1)
                self.assertEqual(sample["presentation"]["presented"], 9)
                self.assertEqual(sample["presentation"][key], 1)
                self.text += PRESENT
                status, sample = self.sample()
                self.assertEqual(status, 1)
                self.assertEqual(sample["presentation"]["presented"], 18)
                self.assertEqual(sample["presentation"][key], 1)

    def test_cutscene_errors_reject_even_a_successful_process(self):
        self.text += "ScriptLog: CutError: actor is not captured\n"
        status, sample = self.sample()
        self.assertEqual(status, 1)
        self.assertTrue(sample["run_passed"])
        self.assertEqual(sample["cut_error_count"], 1)
        self.assertEqual(self.sample(True)[0], 1)
        self.text += "ScriptLog: CutError: another unresolved actor\n"
        self.assertEqual(self.sample()[1]["cut_error_count"], 2)
        self.text = TIMING.format("tick") + TIMING.format("interval") + PRESENT + MARKERS
        status, sample = self.sample()
        self.assertEqual(status, 0)
        for invalid in (None, True, False, -1, 1, "0"):
            with self.subTest(counter=invalid):
                sample["cut_error_count"] = invalid
                self.assertFalse(perf.valid_sample(sample, False))
        del sample["cut_error_count"]
        self.assertFalse(perf.valid_sample(sample, False))

    def test_parent_forwards_opt_in_and_rejects_idle_and_child_failures(self):
        engine = self.root / "engine"
        engine.write_text("fake")
        _, sample = self.sample(True)
        args = ["perf_baseline.py", "--engine-bin", str(engine), "--data-root", str(self.root),
                "--output", str(self.root / "report.json"), "--maps", "Entry", "--repeats", "1"]
        calls = []
        def run(command, **kwargs):
            calls.append(command)
            if command[0] == "sysctl":
                return argparse.Namespace(returncode=0, stdout="cpu", stderr="")
            return argparse.Namespace(returncode=0, stdout="PERF_SAMPLE " + json.dumps(sample), stderr="")
        with mock.patch.object(perf.subprocess, "run", side_effect=run), \
             mock.patch.object(perf.platform, "platform", return_value="test"), \
             mock.patch.object(perf.platform, "processor", return_value="test"), \
             contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
            with mock.patch.object(sys, "argv", args + ["--deterministic-clock"]):
                self.assertEqual(perf.main(), 0)
            self.assertIn("--deterministic-clock", calls[0])
            calls.clear()
            with mock.patch.object(sys, "argv", args):
                self.assertEqual(perf.main(), 0)
            self.assertNotIn("--deterministic-clock", calls[0])
            sample["interval"]["mean"] = 10
            with mock.patch.object(sys, "argv", args):
                self.assertEqual(perf.main(), 1)
            sample["interval"]["mean"] = 2
            sample["exit_status"] = 1
            with mock.patch.object(sys, "argv", args):
                self.assertEqual(perf.main(), 1)


if __name__ == "__main__":
    unittest.main()
