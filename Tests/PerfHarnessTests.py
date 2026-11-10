#!/usr/bin/env python3
"""Data-free contracts for Build/perf_baseline.py (cap forcing and target verdicts)."""
import sys
import unittest
from pathlib import Path

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "Build"))
import perf_baseline as perf  # noqa: E402

INI = """[Engine.GameEngine]
FrameRateLimit=144.000000
Other=1

[WgpuDrv.WgpuRenderDevice]
UseVSync=True
FrameRateLimit=7
"""


class PerfHarness(unittest.TestCase):
    def test_set_ini_value_changes_only_the_named_section_key(self):
        text = perf.set_ini_value(INI, "Engine.GameEngine", "FrameRateLimit", "1000.000000")
        self.assertIn("[Engine.GameEngine]\nFrameRateLimit=1000.000000\nOther=1", text)
        self.assertIn("UseVSync=True\nFrameRateLimit=7", text)  # same key in another section untouched

    def test_set_ini_value_rejects_missing_or_duplicate_keys(self):
        with self.assertRaises(SystemExit):
            perf.set_ini_value(INI, "Engine.GameEngine", "Missing", "1")
        with self.assertRaises(SystemExit):
            perf.set_ini_value(INI, "No.Section", "FrameRateLimit", "1")
        with self.assertRaises(SystemExit):
            perf.set_ini_value("[A]\nK=1\nK=2\n", "A", "K", "3")

    def test_target_verdict_uses_p95_against_the_frame_budget(self):
        def row(p95):
            return {"tick_p50": {"median": 5.0}, "tick_p95": {"median": p95}, "tick_p99": {"median": p95 + 1}}
        verdicts = {r["map"]: r for r in perf.evaluate({"fast": row(16.0), "slow": row(20.0)}, 60.0)}
        self.assertTrue(verdicts["fast"]["meets_target"])
        self.assertFalse(verdicts["slow"]["meets_target"])
        self.assertAlmostEqual(verdicts["slow"]["speedup_needed"], 20.0 / (1000.0 / 60.0))
        self.assertEqual(verdicts["fast"]["speedup_needed"], 1.0)


if __name__ == "__main__":
    unittest.main()
