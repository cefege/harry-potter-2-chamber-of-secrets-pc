#!/usr/bin/env python3
"""Framing-baseline contracts for the frame-capture pipeline.

Exercises Tools/baseline_compare.py end to end without any game data,
network, or wall clock: every fixture PNG is synthesized in-process through
the module's own writer, so the codec and the verdict policy are both under
test against known bytes.

Covered behavior:

* PNG round trip (write_png -> read_png) is lossless for RGB8 payloads.
* Verdicts: identical frames compare "same"; per-channel deltas within
  tolerance stay "same"; deltas beyond both the primary tolerance and the
  thumbnail fallback report "differs"; dimension mismatches report
  "size_mismatch" before any pixel work.
* The downscaled-thumbnail fallback absorbs sparse antialiasing noise that
  breaks the primary per-channel comparison.
* The CLI exits 0 only for "same", 1 for a failing verdict, and emits a
  schema-v1 report with dotted-lowercase reason codes.
"""

from __future__ import annotations

import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

REPOSITORY_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(REPOSITORY_ROOT / "Tools"))

import baseline_compare as bc  # noqa: E402

BASELINE_COMPARE_CLI = REPOSITORY_ROOT / "Tools" / "baseline_compare.py"


def _gradient(width: int, height: int) -> bytes:
    """Deterministic RGB8 payload; byte i is (i * 7) % 256."""
    return bytes((index * 7) % 256 for index in range(width * height * 3))


class PngCodecTests(unittest.TestCase):
    def setUp(self) -> None:
        self.directory = Path(tempfile.mkdtemp(prefix="hp2-framing-"))

    def test_round_trip_is_lossless(self) -> None:
        width, height = 33, 17  # non-multiple-of-4 dimensions exercise strides
        payload = _gradient(width, height)
        path = self.directory / "roundtrip.png"
        bc.write_png(path, width, height, payload)
        decoded_width, decoded_height, decoded = bc.read_png(path)
        self.assertEqual((decoded_width, decoded_height), (width, height))
        self.assertEqual(decoded, payload)

    def test_read_rejects_non_png(self) -> None:
        path = self.directory / "not.png"
        path.write_bytes(b"definitely not a png")
        with self.assertRaisesRegex(bc.BaselineCompareError, "not a PNG"):
            bc.read_png(path)

    def test_write_rejects_wrong_payload_size(self) -> None:
        path = self.directory / "bad.png"
        with self.assertRaisesRegex(bc.BaselineCompareError, "does not match"):
            bc.write_png(path, 4, 4, b"\x00" * 8)


class VerdictTests(unittest.TestCase):
    def setUp(self) -> None:
        self.width, self.height = 64, 48
        self.baseline_bytes = _gradient(self.width, self.height)
        self.directory = Path(tempfile.mkdtemp(prefix="hp2-framing-"))
        self.baseline = self.directory / "baseline.png"
        bc.write_png(
            self.baseline, self.width, self.height, self.baseline_bytes
        )

    def _candidate(self, name: str, payload: bytes) -> Path:
        path = self.directory / name
        bc.write_png(path, self.width, self.height, payload)
        return path

    def test_identical_frames_are_same(self) -> None:
        result = bc.compare_png_files(
            self.baseline,
            self._candidate("same.png", self.baseline_bytes),
        )
        self.assertEqual(result["verdict"], "same")
        self.assertEqual(result["max_channel_delta"], 0)
        self.assertFalse(result["fallback_used"])

    def test_delta_within_tolerance_is_same(self) -> None:
        candidate = bytearray(self.baseline_bytes)
        candidate[0] += 2
        candidate[123] -= 2
        result = bc.compare_png_files(
            self.baseline,
            self._candidate("within.png", bytes(candidate)),
            tolerance=2,
        )
        self.assertEqual(result["verdict"], "same")
        self.assertEqual(result["max_channel_delta"], 2)

    def test_delta_beyond_tolerance_differs(self) -> None:
        candidate = bytearray(self.baseline_bytes)
        # A dense uniform shift defeats both the primary check and the
        # thumbnail average.
        candidate = bytes((value + 40) % 256 for value in candidate)
        result = bc.compare_png_files(
            self.baseline,
            self._candidate("beyond.png", candidate),
            tolerance=2,
        )
        self.assertEqual(result["verdict"], "differs")
        self.assertGreater(result["max_channel_delta"], 2)
        self.assertTrue(result["fallback_used"])

    def test_thumbnail_fallback_absorbs_aa_noise(self) -> None:
        # Sparse edge-like noise (200 of ~9k channels shifted by <=25)
        # breaks the strict per-channel check but washes out in the
        # downscaled thumbnails.
        noise = bytearray(self.baseline_bytes)
        step = max(1, len(noise) // 200)
        for index in range(0, len(noise), step):
            delta = (index % 23) + 3  # deterministic 3..25
            noise[index] = min(255, noise[index] + delta)
        result = bc.compare_png_files(
            self.baseline,
            self._candidate("noise.png", bytes(noise)),
            tolerance=2,
        )
        self.assertTrue(result["fallback_used"])
        self.assertEqual(result["verdict"], "same")
        self.assertGreater(result["max_channel_delta"], 2)
        self.assertLessEqual(result["fallback_max_channel_delta"], 8)

    def test_size_mismatch_short_circuits(self) -> None:
        other = self.directory / "other.png"
        bc.write_png(other, self.width + 8, self.height, b"\x00" * ((self.width + 8) * self.height * 3))
        result = bc.compare_png_files(self.baseline, other)
        self.assertEqual(result["verdict"], "size_mismatch")
        self.assertIsNone(result["max_channel_delta"])


class CliReportTests(unittest.TestCase):
    def setUp(self) -> None:
        self.width, self.height = 16, 12
        self.payload = _gradient(self.width, self.height)
        self.directory = Path(tempfile.mkdtemp(prefix="hp2-framing-cli-"))
        self.baseline = self.directory / "baseline.png"
        bc.write_png(self.baseline, self.width, self.height, self.payload)

    def _run(self, candidate: Path) -> tuple[int, dict[str, object]]:
        completed = subprocess.run(
            [
                sys.executable,
                str(BASELINE_COMPARE_CLI),
                str(self.baseline),
                str(candidate),
                "--tolerance",
                "2",
            ],
            capture_output=True,
            text=True,
            check=False,
        )
        return completed.returncode, json.loads(completed.stdout)

    def test_same_frame_passes_with_schema_v1_report(self) -> None:
        candidate = self.directory / "same.png"
        bc.write_png(candidate, self.width, self.height, self.payload)
        exit_code, report = self._run(candidate)
        self.assertEqual(exit_code, 0)
        self.assertEqual(report["schema"], 1)
        self.assertEqual(report["name"], "baseline_compare")
        self.assertEqual(report["status"], "pass")
        self.assertNotIn("reason_code", report)
        self.assertEqual(report["data"]["profile"]["tolerance"], 2)
        self.assertEqual(report["exit_reason"], "frames match within tolerance")

    def test_differing_frame_fails_with_reason_code(self) -> None:
        candidate = self.directory / "differs.png"
        bc.write_png(
            candidate,
            self.width,
            self.height,
            bytes((value + 90) % 256 for value in self.payload),
        )
        exit_code, report = self._run(candidate)
        self.assertEqual(exit_code, 1)
        self.assertEqual(report["status"], "fail")
        self.assertEqual(report["reason_code"], "renderer.frame_differs")

    def test_undecodable_input_blocks(self) -> None:
        candidate = self.directory / "garbage.png"
        candidate.write_bytes(b"\x00" * 32)
        exit_code, report = self._run(candidate)
        self.assertEqual(exit_code, 2)
        self.assertEqual(report["status"], "blocked")
        self.assertEqual(report["reason_code"], "renderer.frame_undecodable")


if __name__ == "__main__":
    unittest.main()
