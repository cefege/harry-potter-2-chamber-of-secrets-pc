#!/usr/bin/env python3
"""Contract tests for Tools/wgpu_vs_xopengl.py without opening the HP2 application."""

from __future__ import annotations

import contextlib
import io
import json
from pathlib import Path
import struct
import sys
import tempfile
import unittest
import zlib

REPOSITORY_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(REPOSITORY_ROOT / "Tools"))
import wgpu_vs_xopengl  # noqa: E402

PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"


def _encode_row(row: bytes, filter_kind: int) -> bytes:
    """Filter one RGB scanline with None (0) or Sub (1)."""
    if filter_kind == 0:
        return row
    if filter_kind == 1:
        return bytes(
            (row[index] - (row[index - 3] if index >= 3 else 0)) & 0xFF
            for index in range(len(row))
        )
    raise ValueError(f"test encoder supports filters 0 and 1, not {filter_kind}")


def _write_png(
    path: Path, width: int, height: int, rgb: bytes, filters: list[int] | None = None
) -> None:
    """Write an 8-bit RGB PNG; filters gives one filter type per scanline."""
    stride = width * 3
    filters = filters if filters is not None else [0] * height
    raw = bytearray()
    for row_index in range(height):
        row = rgb[row_index * stride : (row_index + 1) * stride]
        raw.append(filters[row_index])
        raw.extend(_encode_row(row, filters[row_index]))

    def chunk(kind: bytes, body: bytes) -> bytes:
        return (
            struct.pack(">I", len(body))
            + kind
            + body
            + struct.pack(">I", zlib.crc32(kind + body) & 0xFFFFFFFF)
        )

    encoded = PNG_SIGNATURE
    encoded += chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0))
    encoded += chunk(b"IDAT", zlib.compress(bytes(raw), 9))
    encoded += chunk(b"IEND", b"")
    path.write_bytes(encoded)


def _map_record(
    root: Path,
    driver: str,
    stem: str,
    frame: tuple[int, int, bytes, list[int] | None] | None,
    *,
    frames_dir_none: bool = False,
    map_name: str | None = None,
) -> dict[str, object]:
    """Build one smoke_maps map record; the last captured frame is `frame`.

    A decoy 1x1 frame_000000.png precedes it, so resolving the wrong capture
    index shows up as a size mismatch.
    """
    frames_dir = root / driver / stem
    frames_dir.mkdir(parents=True, exist_ok=True)
    captures: list[dict[str, object]] = []
    if frame is not None:
        _write_png(frames_dir / "frame_000000.png", 1, 1, bytes([0, 0, 0]))
        width, height, rgb, filters = frame
        _write_png(frames_dir / "frame_000001.png", width, height, rgb, filters)
        captures = [{"width_px": 1, "height_px": 1}, {"width_px": width, "height_px": height}]
    (frames_dir / "frame_meta.json").write_text(
        json.dumps({"captures": captures}), encoding="utf-8"
    )
    return {
        "map": map_name or f"HarryPotter2/Unreal/Maps/{stem}.unr",
        "map_relative_to_data_root": f"Maps/{stem}.unr",
        "passed": True,
        "artifacts": {
            "frames_dir": None if frames_dir_none else str(frames_dir),
            "frame_meta": str(frames_dir / "frame_meta.json"),
        },
    }


def _write_report(root: Path, driver: str, records: list[dict[str, object]]) -> Path:
    path = root / f"{driver}-report.json"
    path.write_text(
        json.dumps({"format_version": 1, "renderer": driver, "maps": records}),
        encoding="utf-8",
    )
    return path


def _run(xopengl: Path, wgpu: Path, output: Path) -> int:
    with contextlib.redirect_stderr(io.StringIO()):
        return wgpu_vs_xopengl.main(
            [
                "--xopengl-report", str(xopengl),
                "--wgpu-report", str(wgpu),
                "--output", str(output),
            ]
        )


def _single_map_report(
    root: Path,
    xopengl_frame: tuple[int, int, bytes, list[int] | None] | None,
    wgpu_frame: tuple[int, int, bytes, list[int] | None] | None,
    stem: str = "Entry",
) -> dict[str, object]:
    """Run the tool over one-map reports and return the written JSON."""
    xopengl = _write_report(root, "xopengl", [_map_record(root, "xopengl", stem, xopengl_frame)])
    wgpu = _write_report(root, "wgpu", [_map_record(root, "wgpu", stem, wgpu_frame)])
    output = root / "out" / "wgpu-vs-xopengl.json"
    assert _run(xopengl, wgpu, output) == 0
    return json.loads(output.read_text(encoding="utf-8"))


class WgpuVsXopenglContracts(unittest.TestCase):
    def test_identical_frames_have_zero_difference(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            rgb = bytes(range(4 * 3 * 3))  # 4x3 with varied content
            report = _single_map_report(
                root,
                (4, 3, rgb, [0, 1, 0]),
                (4, 3, rgb, [1, 0, 0]),
            )
            entry = report["maps"]["Entry"]
            self.assertEqual(entry["mean_abs_rgb"], 0.0)
            self.assertEqual(entry["pct_max_delta_gt8"], 0.0)
            self.assertEqual((entry["width"], entry["height"]), (4, 3))
            self.assertNotEqual(entry["xopengl_sha256"], entry["wgpu_sha256"])
            self.assertEqual(report["summary"]["compared"], 1)
            self.assertEqual(report["summary"]["errors"], 0)

    def test_single_pixel_delta_of_nine_is_averaged_and_counted(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            xopengl_rgb = bytes([10, 20, 30, 100, 100, 100])
            wgpu_rgb = bytes([10, 20, 30, 109, 100, 100])
            report = _single_map_report(
                root,
                (2, 1, xopengl_rgb, None),
                (2, 1, wgpu_rgb, None),
            )
            entry = report["maps"]["Entry"]
            self.assertEqual(entry["mean_abs_rgb"], 1.5)
            self.assertEqual(entry["pct_max_delta_gt8"], 50.0)
            self.assertEqual(report["summary"]["mean_abs_rgb_mean"], 1.5)
            self.assertEqual(report["summary"]["pct_max_delta_gt8_mean"], 50.0)
            self.assertEqual(report["summary"]["worst_by_pct"], ["Entry"])

    def test_delta_of_exactly_eight_is_not_over_threshold(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            report = _single_map_report(
                root,
                (1, 1, bytes([100, 100, 100]), None),
                (1, 1, bytes([108, 100, 100]), None),
            )
            entry = report["maps"]["Entry"]
            self.assertEqual(entry["pct_max_delta_gt8"], 0.0)
            self.assertGreater(entry["mean_abs_rgb"], 0.0)
            self.assertEqual(entry["mean_abs_rgb"], 2.6667)

    def test_map_only_in_wgpu_report_is_missing_in_xopengl(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            xopengl = _write_report(root, "xopengl", [])
            wgpu = _write_report(
                root, "wgpu", [_map_record(root, "wgpu", "Entry", (1, 1, bytes([1, 2, 3]), None))]
            )
            output = root / "wgpu-vs-xopengl.json"
            self.assertEqual(_run(xopengl, wgpu, output), 0)
            report = json.loads(output.read_text(encoding="utf-8"))
            self.assertEqual(report["maps"]["Entry"], {"error": "missing_in_xopengl"})
            self.assertEqual(report["summary"]["errors"], 1)
            self.assertEqual(report["summary"]["compared"], 0)
            self.assertIsNone(report["summary"]["mean_abs_rgb_mean"])
            self.assertEqual(report["summary"]["worst_by_pct"], [])

    def test_size_mismatch_reports_both_sizes(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            report = _single_map_report(
                root,
                (2, 1, bytes(6), None),
                (1, 2, bytes(6), None),
            )
            entry = report["maps"]["Entry"]
            self.assertEqual(entry["error"], "size_mismatch")
            self.assertEqual(entry["xopengl_size"], [2, 1])
            self.assertEqual(entry["wgpu_size"], [1, 2])
            self.assertEqual(report["summary"]["errors"], 1)

    def test_map_without_frame_is_reported_per_driver(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            report = _single_map_report(
                root,
                (1, 1, bytes([1, 2, 3]), None),
                None,
            )
            entry = report["maps"]["Entry"]
            self.assertEqual(entry["error"], "no_frame_wgpu")
            self.assertEqual(report["summary"]["errors"], 1)

    def test_frames_dir_null_counts_as_no_frame(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            xopengl = _write_report(
                root, "xopengl",
                [_map_record(root, "xopengl", "Entry", None, frames_dir_none=True)],
            )
            wgpu = _write_report(
                root, "wgpu", [_map_record(root, "wgpu", "Entry", (1, 1, bytes(3), None))]
            )
            output = root / "wgpu-vs-xopengl.json"
            self.assertEqual(_run(xopengl, wgpu, output), 0)
            report = json.loads(output.read_text(encoding="utf-8"))
            self.assertEqual(report["maps"]["Entry"]["error"], "no_frame_xopengl")

    def test_stems_pair_case_insensitively(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            rgb = bytes([5, 5, 5])
            xopengl = _write_report(
                root, "xopengl",
                [_map_record(root, "xopengl", "Entry", (1, 1, rgb, None))],
            )
            wgpu = _write_report(
                root, "wgpu",
                [_map_record(
                    root, "wgpu", "entry", (1, 1, rgb, None),
                    map_name="HarryPotter2/Unreal/Maps/entry.unr",
                )],
            )
            output = root / "wgpu-vs-xopengl.json"
            self.assertEqual(_run(xopengl, wgpu, output), 0)
            report = json.loads(output.read_text(encoding="utf-8"))
            self.assertEqual(list(report["maps"]), ["Entry"])
            self.assertEqual(report["maps"]["Entry"]["mean_abs_rgb"], 0.0)
            self.assertEqual(report["summary"]["compared"], 1)

    def test_output_shape_is_sorted_indented_and_newline_terminated(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            xopengl = _write_report(root, "xopengl", [])
            wgpu = _write_report(root, "wgpu", [])
            output = root / "wgpu-vs-xopengl.json"
            self.assertEqual(_run(xopengl, wgpu, output), 0)
            text = output.read_text(encoding="utf-8")
            self.assertTrue(text.endswith("}\n"))
            report = json.loads(text)
            self.assertEqual(text, json.dumps(report, indent=2, sort_keys=True) + "\n")
            self.assertEqual(report["schema_version"], 1)
            self.assertEqual(report["report"], "wgpu_vs_xopengl")
            self.assertEqual(report["xopengl_report"], str(xopengl))
            self.assertEqual(report["wgpu_report"], str(wgpu))
            self.assertEqual(
                report["summary"],
                {
                    "compared": 0,
                    "errors": 0,
                    "mean_abs_rgb_mean": None,
                    "pct_max_delta_gt8_mean": None,
                    "worst_by_pct": [],
                },
            )

    def test_missing_report_path_exits_two(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            wgpu = _write_report(root, "wgpu", [])
            output = root / "wgpu-vs-xopengl.json"
            stderr = io.StringIO()
            with contextlib.redirect_stderr(stderr):
                code = wgpu_vs_xopengl.main(
                    [
                        "--xopengl-report", str(root / "absent.json"),
                        "--wgpu-report", str(wgpu),
                        "--output", str(output),
                    ]
                )
            self.assertEqual(code, 2)
            self.assertIn("absent.json", stderr.getvalue())
            self.assertFalse(output.exists())

    def test_invalid_report_json_exits_two(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            xopengl = root / "xopengl.json"
            xopengl.write_text("{not json", encoding="utf-8")
            wgpu = _write_report(root, "wgpu", [])
            self.assertEqual(_run(xopengl, wgpu, root / "out.json"), 2)


if __name__ == "__main__":
    unittest.main()
