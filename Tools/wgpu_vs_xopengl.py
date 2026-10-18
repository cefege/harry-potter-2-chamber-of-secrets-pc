#!/usr/bin/env python3
"""Compare per-map representative frames from two smoke_maps reports.

Reads an --xopengl-report and a --wgpu-report produced by
Build/smoke_maps.py --capture-frames, pairs map records by map file stem
(case-insensitive), and for each pair resolves the last captured frame with
Tools/matrix_compare.py's _representative_frame. Both frames are decoded with
Tools/baseline_compare.py's read_png and compared over every pixel, RGB only:

    mean_abs_rgb        mean over pixels of (|dr|+|dg|+|db|)/3, 4 decimals
    pct_max_delta_gt8   100 * count(max(|dr|,|dg|,|db|) > 8) / pixels, 4 decimals

Per-map problems are recorded as "error" entries instead of aborting:
    missing_in_<driver>  map absent from that report
    no_frame_<driver>    record has no resolvable captured frame
    undecodable_<driver> frame PNG could not be decoded (detail attached)
    size_mismatch        frame dimensions differ (both sizes attached)

Exit codes: 0 report written (per-map errors included), 2 unusable input
report or unwritable output (reason on stderr).
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import sys
import tempfile
from pathlib import Path

if __package__:
    from . import baseline_compare, matrix_compare
else:
    import baseline_compare
    import matrix_compare

SCHEMA_VERSION = 1
REPORT_NAME = "wgpu_vs_xopengl"
CHANNEL_DELTA_THRESHOLD = 8
WORST_COUNT = 5
DRIVERS = ("xopengl", "wgpu")
REPO_ROOT = Path(__file__).resolve().parent.parent


class ReportInputError(Exception):
    """Raised when a smoke_maps report cannot be read or is not a report."""


def _load_report(path: Path) -> dict[str, object]:
    try:
        decoded = json.loads(path.read_text(encoding="utf-8"))
    except OSError as error:
        raise ReportInputError(f"cannot read report {path}: {error}") from error
    except ValueError as error:
        raise ReportInputError(f"report {path} is not valid JSON: {error}") from error
    if not isinstance(decoded, dict) or not isinstance(decoded.get("maps"), list):
        raise ReportInputError(f"report {path} has no top-level 'maps' list")
    return decoded


def _map_stem(record: dict[str, object]) -> str | None:
    for key in ("map", "map_relative_to_data_root"):
        value = record.get(key)
        if isinstance(value, str) and value:
            return Path(value).stem
    return None


def _index_maps(report: dict[str, object]) -> dict[str, tuple[str, dict[str, object]]]:
    """Map casefolded stem -> (display stem, record)."""
    indexed: dict[str, tuple[str, dict[str, object]]] = {}
    for record in report["maps"]:  # type: ignore[index]
        if not isinstance(record, dict):
            continue
        stem = _map_stem(record)
        if stem is not None:
            indexed[stem.casefold()] = (stem, record)
    return indexed


def _anchored(record: dict[str, object]) -> dict[str, object]:
    """smoke_maps writes repo-relative frames_dir display paths; anchor them."""
    artifacts = record.get("artifacts")
    if not isinstance(artifacts, dict):
        return record
    frames_dir = artifacts.get("frames_dir")
    if not isinstance(frames_dir, str) or not frames_dir or Path(frames_dir).is_absolute():
        return record
    anchored_artifacts = dict(artifacts)
    anchored_artifacts["frames_dir"] = str(REPO_ROOT / frames_dir)
    anchored = dict(record)
    anchored["artifacts"] = anchored_artifacts
    return anchored


def _frame_for(record: dict[str, object], driver: str) -> Path | None:
    try:
        frame = matrix_compare._representative_frame(_anchored(record), driver)
    except matrix_compare.MatrixCompareError:
        return None
    return frame if frame.is_file() else None


def _sha256_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _compare_pixels(xopengl_rgb: bytes, wgpu_rgb: bytes) -> tuple[float, float]:
    """Return (mean_abs_rgb, pct_max_delta_gt8) for equal-size RGB8 buffers."""
    pixel_count = len(xopengl_rgb) // 3
    deltas = [abs(a - b) for a, b in zip(xopengl_rgb, wgpu_rgb)]
    channel_total = sum(deltas)
    over_threshold = sum(
        1
        for largest in map(max, deltas[0::3], deltas[1::3], deltas[2::3])
        if largest > CHANNEL_DELTA_THRESHOLD
    )
    mean_abs_rgb = channel_total / (3 * pixel_count)
    pct_max_delta_gt8 = 100 * over_threshold / pixel_count
    return round(mean_abs_rgb, 4), round(pct_max_delta_gt8, 4)


def _frame_fields(frames: dict[str, Path]) -> dict[str, object]:
    fields: dict[str, object] = {}
    for driver in DRIVERS:
        frame = frames.get(driver)
        if frame is not None:
            fields[f"{driver}_frame"] = str(frame)
            fields[f"{driver}_sha256"] = _sha256_file(frame)
    return fields


def _compare_map(
    stem: str,
    records: dict[str, dict[str, object] | None],
) -> dict[str, object]:
    absent = [driver for driver in DRIVERS if records[driver] is None]
    if absent:
        return {"error": f"missing_in_{absent[0]}"}

    frames: dict[str, Path] = {}
    for driver in DRIVERS:
        frame = _frame_for(records[driver], driver)  # type: ignore[arg-type]
        if frame is not None:
            frames[driver] = frame
    absent_frames = [driver for driver in DRIVERS if driver not in frames]
    if absent_frames:
        return {"error": f"no_frame_{absent_frames[0]}", **_frame_fields(frames)}

    decoded: dict[str, tuple[int, int, bytes]] = {}
    for driver in DRIVERS:
        try:
            decoded[driver] = baseline_compare.read_png(frames[driver])
        except baseline_compare.BaselineCompareError as error:
            return {
                "error": f"undecodable_{driver}",
                "detail": str(error),
                **_frame_fields(frames),
            }

    xopengl_width, xopengl_height, xopengl_rgb = decoded["xopengl"]
    wgpu_width, wgpu_height, wgpu_rgb = decoded["wgpu"]
    if (xopengl_width, xopengl_height) != (wgpu_width, wgpu_height):
        return {
            "error": "size_mismatch",
            "xopengl_size": [xopengl_width, xopengl_height],
            "wgpu_size": [wgpu_width, wgpu_height],
            **_frame_fields(frames),
        }

    mean_abs_rgb, pct_max_delta_gt8 = _compare_pixels(xopengl_rgb, wgpu_rgb)
    return {
        "width": xopengl_width,
        "height": xopengl_height,
        "mean_abs_rgb": mean_abs_rgb,
        "pct_max_delta_gt8": pct_max_delta_gt8,
        **_frame_fields(frames),
    }


def _build_maps(
    xopengl_report: dict[str, object],
    wgpu_report: dict[str, object],
) -> dict[str, dict[str, object]]:
    xopengl_by_stem = _index_maps(xopengl_report)
    wgpu_by_stem = _index_maps(wgpu_report)
    maps: dict[str, dict[str, object]] = {}
    for key in sorted(set(xopengl_by_stem) | set(wgpu_by_stem)):
        display = (xopengl_by_stem.get(key) or wgpu_by_stem[key])[0]
        maps[display] = _compare_map(
            display,
            {
                "xopengl": xopengl_by_stem[key][1] if key in xopengl_by_stem else None,
                "wgpu": wgpu_by_stem[key][1] if key in wgpu_by_stem else None,
            },
        )
    return maps


def _summarize(maps: dict[str, dict[str, object]]) -> dict[str, object]:
    compared = {stem: entry for stem, entry in maps.items() if "error" not in entry}
    summary: dict[str, object] = {
        "compared": len(compared),
        "errors": len(maps) - len(compared),
        "mean_abs_rgb_mean": None,
        "pct_max_delta_gt8_mean": None,
        "worst_by_pct": [],
    }
    if compared:
        count = len(compared)
        summary["mean_abs_rgb_mean"] = round(
            sum(float(entry["mean_abs_rgb"]) for entry in compared.values()) / count, 4  # type: ignore[arg-type]
        )
        summary["pct_max_delta_gt8_mean"] = round(
            sum(float(entry["pct_max_delta_gt8"]) for entry in compared.values()) / count, 4  # type: ignore[arg-type]
        )
        ranked = sorted(
            compared,
            key=lambda stem: (-float(compared[stem]["pct_max_delta_gt8"]), stem),  # type: ignore[arg-type]
        )
        summary["worst_by_pct"] = ranked[:WORST_COUNT]
    return summary


def build_report(
    xopengl_path: Path,
    wgpu_path: Path,
    xopengl_report: dict[str, object],
    wgpu_report: dict[str, object],
) -> dict[str, object]:
    maps = _build_maps(xopengl_report, wgpu_report)
    return {
        "schema_version": SCHEMA_VERSION,
        "report": REPORT_NAME,
        "xopengl_report": str(xopengl_path),
        "wgpu_report": str(wgpu_path),
        "maps": maps,
        "summary": _summarize(maps),
    }


def _write_atomic(output: Path, report: dict[str, object]) -> None:
    output.parent.mkdir(parents=True, exist_ok=True)
    payload = json.dumps(report, indent=2, sort_keys=True) + "\n"
    descriptor, temporary = tempfile.mkstemp(
        dir=output.parent, prefix=f".{output.name}.", suffix=".tmp"
    )
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8") as handle:
            handle.write(payload)
        os.replace(temporary, output)
    except BaseException:
        Path(temporary).unlink(missing_ok=True)
        raise


def _parse_arguments(argv: list[str] | None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Compare each map's last captured frame between a smoke_maps "
            "--renderer=xopengl report and a --renderer=wgpu report."
        ),
    )
    parser.add_argument(
        "--xopengl-report", required=True, type=Path,
        help="smoke_maps JSON report from the xopengl run",
    )
    parser.add_argument(
        "--wgpu-report", required=True, type=Path,
        help="smoke_maps JSON report from the wgpu run",
    )
    parser.add_argument(
        "--output", required=True, type=Path,
        help="comparison report JSON to write (atomically)",
    )
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    arguments = _parse_arguments(argv)
    try:
        xopengl_report = _load_report(arguments.xopengl_report)
        wgpu_report = _load_report(arguments.wgpu_report)
    except ReportInputError as error:
        print(f"wgpu_vs_xopengl: {error}", file=sys.stderr)
        return 2
    report = build_report(
        arguments.xopengl_report, arguments.wgpu_report,
        xopengl_report, wgpu_report,
    )
    try:
        _write_atomic(arguments.output, report)
    except OSError as error:
        print(f"wgpu_vs_xopengl: cannot write {arguments.output}: {error}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
