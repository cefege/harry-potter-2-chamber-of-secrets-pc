#!/usr/bin/env python3
"""Contract tests for Tools/matrix_compare.py without opening the HP2 application."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

REPOSITORY_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(REPOSITORY_ROOT / "Tools"))
sys.path.insert(0, str(REPOSITORY_ROOT / "Build"))
import baseline_compare  # noqa: E402
import matrix_compare  # noqa: E402
import smoke_maps  # noqa: E402
import game_test  # noqa: E402


def _write_frame(path: Path, width: int, height: int, shade: int) -> None:
    baseline_compare.write_png(
        path, width, height, bytes([shade % 256]) * (width * height * 3)
    )


def _capture_record(frames_dir: Path, frame_count: int) -> dict[str, object]:
    captures = [{"width_px": 4, "height_px": 4} for _ in range(frame_count)]
    (frames_dir / "frame_meta.json").write_text(
        json.dumps({"captures": captures}), encoding="utf-8"
    )
    return {
        "passed": True,
        "map_relative_to_data_root": "Maps/PrivetDr.unr",
        "artifacts": {"frames_dir": str(frames_dir), "frame_meta": str(frames_dir / "frame_meta.json")},
    }


class MatrixLegOutputContracts(unittest.TestCase):
    def test_leg_reports_are_named_after_the_output_stem(self) -> None:
        self.assertEqual(
            matrix_compare._leg_output(Path("out/renderer-matrix.json"), "vulkan"),
            Path("out/renderer-matrix-vulkan.json"),
        )
        self.assertEqual(
            matrix_compare._leg_output(Path("out/matrix"), "xopengl"),
            Path("out/matrix-xopengl.json"),
        )

    def test_representative_frame_is_the_last_capture(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            frames = root / "000001-PrivetDr"
            frames.mkdir()
            for index in range(3):
                _write_frame(matrix_compare._frame_path(frames, index), 4, 4, 10 * index)
            record = _capture_record(frames, 3)
            chosen = matrix_compare._representative_frame(record, "xopengl")
            self.assertEqual(chosen, matrix_compare._frame_path(frames, 2))

    def test_representative_frame_requires_capture_metadata(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            empty = Path(temporary) / "frames"
            empty.mkdir()
            record = {
                "passed": True,
                "artifacts": {"frames_dir": str(empty)},
            }
            with self.assertRaisesRegex(
                matrix_compare.MatrixCompareError, "cannot read frame metadata"
            ):
                matrix_compare._representative_frame(record, "xopengl")
            (empty / "frame_meta.json").write_text('{"captures": []}', encoding="utf-8")
            with self.assertRaisesRegex(
                matrix_compare.MatrixCompareError, "no captured frames"
            ):
                matrix_compare._representative_frame(record, "xopengl")


class MatrixComparisonContracts(unittest.TestCase):
    def _reports(self, root: Path, *, xopengl_shade: int, vulkan_shade: int,
                 xopengl_passed: bool = True, vulkan_passed: bool = True):
        for driver, passed in (("xopengl", xopengl_passed), ("vulkan", vulkan_passed)):
            frames = root / f"{driver}-frames"
            frames.mkdir(exist_ok=True)
            _write_frame(matrix_compare._frame_path(frames, 0), 4, 4, locals()[f"{driver}_shade"])
            report = {
                "maps": [
                    dict(_capture_record(frames, 1), passed=passed),
                ],
            }
            (root / f"{driver}.json").write_text(json.dumps(report), encoding="utf-8")
        return (
            json.loads((root / "xopengl.json").read_text()),
            json.loads((root / "vulkan.json").read_text()),
        )

    def test_identical_frames_match_across_drivers(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            xopengl, vulkan = self._reports(root, xopengl_shade=40, vulkan_shade=40)
            matrix, failures = matrix_compare.compare_maps(xopengl, vulkan, tolerance=8)
            self.assertEqual(failures, [])
            entry = matrix["privetdr"]
            self.assertEqual(entry["xopengl_sha"], entry["vulkan_sha"])
            self.assertEqual(entry["delta"]["verdict"], "same")

    def test_frames_beyond_tolerance_fail_with_reason_code(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            xopengl, vulkan = self._reports(root, xopengl_shade=0, vulkan_shade=255)
            matrix, failures = matrix_compare.compare_maps(xopengl, vulkan, tolerance=8)
            self.assertEqual(failures, ["privetdr: renderer.frame_differs"])
            self.assertEqual(matrix["privetdr"]["delta"]["verdict"], "differs")

    def test_size_mismatch_fails(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self._reports(root, xopengl_shade=0, vulkan_shade=0)
            frames = root / "vulkan-frames"
            baseline_compare.write_png(
                matrix_compare._frame_path(frames, 0), 6, 6, bytes(6 * 6 * 3)
            )
            xopengl = json.loads((root / "xopengl.json").read_text())
            vulkan = json.loads((root / "vulkan.json").read_text())
            matrix, failures = matrix_compare.compare_maps(xopengl, vulkan, tolerance=8)
            self.assertEqual(failures, ["privetdr: renderer.frame_size_mismatch"])

    def test_failed_or_missing_legs_block_the_map(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self._reports(root, xopengl_shade=0, vulkan_shade=0,
                          vulkan_passed=False)
            xopengl = json.loads((root / "xopengl.json").read_text())
            vulkan = json.loads((root / "vulkan.json").read_text())
            matrix, failures = matrix_compare.compare_maps(xopengl, vulkan, tolerance=8)
            self.assertEqual(len(failures), 1)
            self.assertIn("vulkan leg did not pass", failures[0])
            self.assertIn("error", matrix["privetdr"])


class MatrixReportContracts(unittest.TestCase):
    def test_status_and_reason_codes_track_failures(self) -> None:
        passed = matrix_compare.build_report(
            {}, [], tolerance=8, artifacts=[], command=[],
        )
        self.assertEqual(passed["status"], "pass")
        failed = matrix_compare.build_report(
            {}, ["privetdr: renderer.frame_differs"],
            tolerance=8, artifacts=[], command=[],
        )
        self.assertEqual(failed["status"], "fail")
        self.assertEqual(failed["reason_code"], "renderer.frame_differs")
        blocked = matrix_compare.build_report(
            {}, ["xopengl smoke leg was blocked (exit 2); see log"],
            tolerance=8, artifacts=[], command=[],
        )
        self.assertEqual(blocked["status"], "blocked")
        self.assertEqual(blocked["reason_code"], "renderer.matrix_prerequisite_missing")
        failed_leg = matrix_compare.build_report(
            {}, ["xopengl smoke leg failed (exit 1); see log"],
            tolerance=8, artifacts=[], command=[],
        )
        self.assertEqual(failed_leg["status"], "fail")
        self.assertNotEqual(failed_leg["status"], "blocked")

    def test_failed_leg_exits_one_not_two(self) -> None:
        original = matrix_compare.launch_leg

        def failing_leg(arguments, driver, output):
            raise matrix_compare.MatrixCompareError(
                f"{driver} smoke leg failed (exit 1); see log"
            )

        matrix_compare.launch_leg = failing_leg
        try:
            with tempfile.TemporaryDirectory() as temporary:
                sys.argv = [
                    "matrix_compare.py",
                    f"--output={Path(temporary) / 'matrix.json'}",
                ]
                self.assertEqual(matrix_compare.main(), 1)
        finally:
            matrix_compare.launch_leg = original
            sys.argv = ["matrix_compare.py"]


class SmokeVulkanContextContracts(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(prefix="hp2-matrix-contract-")
        self.root = Path(self.temporary.name)

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def test_vulkan_engine_ini_rewrites_all_video_keys(self) -> None:
        data_root = self.root / "Unreal"
        (data_root / "System").mkdir(parents=True)
        (data_root / "System" / "Default.ini").write_text(
            "[Engine.Engine]\nGameRenderDevice=D3DDrv.D3DRenderDevice\n"
            "WindowedRenderDevice=SoftDrv.SoftwareRenderDevice\n"
            "RenderDevice=GlideDrv.GlideRenderDevice\n",
            encoding="utf-8",
        )
        destination = smoke_maps._vulkan_engine_ini(
            data_root, self.root / "matrix-vulkan-engine.ini"
        )
        text = destination.read_text(encoding="utf-8")
        for key in smoke_maps.VIDEO_DEVICE_KEYS:
            self.assertIn(f"{key}=VulkanDrv.VulkanRenderDevice", text)
        self.assertNotIn("D3DDrv", text)

    def test_vulkan_engine_ini_requires_each_video_key(self) -> None:
        data_root = self.root / "Unreal"
        (data_root / "System").mkdir(parents=True)
        (data_root / "System" / "Default.ini").write_text(
            "[Engine.Engine]\nGameRenderDevice=D3DDrv.D3DRenderDevice\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(smoke_maps.SmokeError, "WindowedRenderDevice"):
            smoke_maps._vulkan_engine_ini(data_root, self.root / "broken.ini")

    def test_vulkan_environment_exports_loader_variables(self) -> None:
        environment = smoke_maps._vulkan_environment(Path("/icd/MoltenVK_icd.json"))
        self.assertEqual(
            environment,
            {
                "VK_DRIVER_FILES": "/icd/MoltenVK_icd.json",
                "DYLD_LIBRARY_PATH": smoke_maps.VULKAN_DYLD_LIBRARY_PATH,
                "DYLD_FALLBACK_LIBRARY_PATH":
                    smoke_maps.VULKAN_DYLD_FALLBACK_LIBRARY_PATH,
            },
        )


class LaunchLegContracts(unittest.TestCase):
    def test_blocked_smoke_leg_raises_and_retains_runner_log(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            arguments = argparse.Namespace(
                app=Path("dist/definitely-missing.app"),
                data_root=Path("HarryPotter2/Unreal"),
                maps="Entry",
                ticks=1,
                timeout=1.0,
                vulkan_icd=Path("/nonexistent/icd.json"),
            )
            output = root / "matrix-xopengl.json"
            with self.assertRaisesRegex(
                matrix_compare.MatrixCompareError, "was blocked"
            ):
                matrix_compare.launch_leg(arguments, "xopengl", output)
            runner_log = output.with_name("matrix-xopengl-runner.log")
            self.assertTrue(runner_log.is_file())
            self.assertIn("smoke_maps.py:", runner_log.read_text(encoding="utf-8"))


class LaunchCommandIniContract(unittest.TestCase):
    def test_ini_file_is_forwarded_to_the_launch_command(self) -> None:
        data_root = Path("HarryPotter2/Unreal")
        plain = game_test.build_launch_command(
            Path("app"), data_root, "..\\Maps\\Entry.unr", "vulkan", 60,
        )
        self.assertFalse(any(argument.startswith("-INI=") for argument in plain))
        selected = game_test.build_launch_command(
            Path("app"), data_root, "..\\Maps\\Entry.unr", "vulkan", 60,
            ini_file=Path("/tmp/vulkan.ini"),
        )
        self.assertIn("-INI=/tmp/vulkan.ini", selected)


if __name__ == "__main__":
    raise SystemExit(unittest.main())
