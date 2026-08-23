#!/usr/bin/env python3
"""Compare the committed three-map smoke subset across render drivers.

Runs Build/smoke_maps.py twice over the same map allowlist -- once with
--renderer=xopengl and once with --renderer=vulkan, both with frame capture
enabled (HP2_CAPTURE_FRAMES) -- then pairs each map's last captured frame and
compares the drivers with Tools/baseline_compare.py semantics.

Per map the combined matrix report records:
    {map: {xopengl_sha256, vulkan_sha256, delta: <baseline_compare result>}}

delta.verdict is "same" when the frames match within --tolerance (the
downscaled-thumbnail fallback absorbs minor AA noise), so the whole matrix
passes only when every map renders equivalently under both drivers.

Exit codes: 0 pass, 1 fail, 2 blocked(reason).
"""
from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import sys
from pathlib import Path

if __package__:
    from . import baseline_compare
else:
    import baseline_compare

SCHEMA_VERSION = 1
REPORT_NAME = "renderer_matrix"
DEFAULT_MAPS = "PrivetDr,Entry,Ch2Skurge"
DEFAULT_TICKS = 120
DEFAULT_TIMEOUT_SECONDS = 90.0
SMOKE_MAPS_SCRIPT = Path("Build/smoke_maps.py")


class MatrixCompareError(Exception):
    """Raised when a matrix leg cannot be produced or interpreted."""


def _sha256_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _frame_path(frames_dir: Path, index: int) -> Path:
    return frames_dir / f"frame_{index:06d}.png"


def _representative_frame(record: dict[str, object], driver: str) -> Path:
    """Resolve the last captured frame for one map record."""
    artifacts = record.get("artifacts")
    if not isinstance(artifacts, dict):
        raise MatrixCompareError(f"{driver}: map record has no artifacts")
    frames_dir_value = artifacts.get("frames_dir")
    if not isinstance(frames_dir_value, str) or not frames_dir_value:
        raise MatrixCompareError(
            f"{driver}: map record has no frames directory (capture disabled?)"
        )
    frames_dir = Path(frames_dir_value)
    meta_path = frames_dir / "frame_meta.json"
    try:
        decoded = json.loads(meta_path.read_text(encoding="utf-8"))
    except (OSError, ValueError) as error:
        raise MatrixCompareError(
            f"{driver}: cannot read frame metadata {meta_path}: {error}"
        ) from error
    captures = decoded.get("captures") if isinstance(decoded, dict) else None
    if not isinstance(captures, list) or not captures:
        raise MatrixCompareError(
            f"{driver}: no captured frames listed in {meta_path}"
        )
    return _frame_path(frames_dir, len(captures) - 1)


def _leg_output(output: Path, driver: str) -> Path:
    stem = output.name[:-5] if output.name.lower().endswith(".json") else output.name
    return output.with_name(f"{stem}-{driver}.json")


def launch_leg(arguments: argparse.Namespace, driver: str, output: Path) -> None:
    """Run one smoke_maps leg; raises on failure or blockage."""
    repo_root = Path(__file__).resolve().parent.parent
    command = [
        sys.executable,
        str(repo_root / SMOKE_MAPS_SCRIPT),
        f"--renderer={driver}",
        f"--app={arguments.app}",
        f"--data-root={arguments.data_root}",
        f"--maps={arguments.maps}",
        f"--ticks={arguments.ticks}",
        f"--timeout={arguments.timeout}",
        "--capture-frames",
        f"--output={output}",
    ]
    if driver == "vulkan":
        command.append(f"--vulkan-icd={arguments.vulkan_icd}")
    completed = subprocess.run(
        command, cwd=repo_root, capture_output=True, text=True,
    )
    runner_log = output.with_name(output.stem + "-runner.log")
    runner_log.write_text(
        (completed.stdout or "") + (completed.stderr or ""), encoding="utf-8"
    )
    if completed.returncode != 0:
        raise MatrixCompareError(
            f"{driver} smoke leg {'failed' if completed.returncode == 1 else 'was blocked'} "
            f"(exit {completed.returncode}); see {runner_log}"
        )


def compare_maps(
    xopengl_report: dict[str, object],
    vulkan_report: dict[str, object],
    *,
    tolerance: int,
) -> tuple[dict[str, dict[str, object]], list[str]]:
    """Pair per-map captures and compare drivers; returns (matrix, failures)."""
    def by_stem(report: dict[str, object]) -> dict[str, dict[str, object]]:
        indexed: dict[str, dict[str, object]] = {}
        for record in report.get("maps", []):
            if isinstance(record, dict):
                relative = str(record.get("map_relative_to_data_root", ""))
                indexed[Path(relative).stem.casefold()] = record
        return indexed

    xopengl_by_map = by_stem(xopengl_report)
    vulkan_by_map = by_stem(vulkan_report)
    matrix: dict[str, dict[str, object]] = {}
    failures: list[str] = []
    for stem in sorted(set(xopengl_by_map) | set(vulkan_by_map)):
        entry: dict[str, object] = {}
        try:
            xopengl_record = xopengl_by_map.get(stem)
            vulkan_record = vulkan_by_map.get(stem)
            if xopengl_record is None or vulkan_record is None:
                raise MatrixCompareError(
                    f"{stem}: missing from "
                    f"{'xopengl' if xopengl_record is None else 'vulkan'} report"
                )
            for driver, record in (("xopengl", xopengl_record), ("vulkan", vulkan_record)):
                if record.get("passed") is not True:
                    raise MatrixCompareError(f"{stem}: {driver} leg did not pass")
            xopengl_frame = _representative_frame(xopengl_record, "xopengl")
            vulkan_frame = _representative_frame(vulkan_record, "vulkan")
            entry["xopengl_sha"] = _sha256_file(xopengl_frame)
            entry["vulkan_sha"] = _sha256_file(vulkan_frame)
            delta = baseline_compare.compare_png_files(
                xopengl_frame, vulkan_frame, tolerance=tolerance,
            )
            entry["delta"] = {
                key: delta[key]
                for key in (
                    "verdict", "max_channel_delta",
                    "fallback_used", "fallback_max_channel_delta",
                )
            }
            if delta["verdict"] == "size_mismatch":
                failures.append(f"{stem}: renderer.frame_size_mismatch")
            elif delta["verdict"] != "same":
                failures.append(f"{stem}: renderer.frame_differs")
        except (MatrixCompareError, OSError) as error:
            entry["error"] = str(error)
            failures.append(str(error))
        matrix[stem] = entry
    return matrix, failures


def build_report(
    matrix: dict[str, dict[str, object]],
    failures: list[str],
    tolerance: int,
    artifacts: list[str],
    command: list[str],
    error: str | None = None,
    force_blocked: bool = False,
) -> dict[str, object]:
    # Message-driven classification: a leg that ran and failed is "fail";
    # only a leg that could not run is "blocked" (infrastructure callers
    # force_blocked for errors with no leg outcome at all).
    blocked = force_blocked or any("was blocked" in item for item in failures)
    status = "blocked" if blocked else ("pass" if not failures else "fail")
    report: dict[str, object] = {
        "schema": SCHEMA_VERSION,
        "name": REPORT_NAME,
        "status": status,
        "invariant": (
            "each subset map's last captured frame matches within per-channel "
            "tolerance across the xopengl and vulkan drivers"
        ),
        "data": {"profile": {"tolerance": tolerance}, "maps": matrix},
        "artifacts": artifacts,
        "command": command,
        "exit_reason": {
            "pass": "every map matches within tolerance across drivers",
            "fail": "a map failed a leg, lacks captures, or exceeds tolerance",
            "blocked": "a smoke leg could not run (prerequisite missing)",
        }[status],
    }
    if error is not None:
        report["error"] = error
    reason_code: str | None = None
    if status == "blocked":
        reason_code = "renderer.matrix_prerequisite_missing"
    elif any("smoke leg failed" in item for item in failures):
        reason_code = "renderer.leg_failed"
    elif failures:
        reason_code = failures[-1].split(": ", 1)[-1] or "renderer.matrix_fail"
    if reason_code is not None:
        report["reason_code"] = reason_code
    return report


def _positive_int(value: str) -> int:
    parsed = int(value)
    if parsed <= 0:
        raise argparse.ArgumentTypeError("must be a positive integer")
    return parsed


def _arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=__doc__,
        formatter_class=argparse.ArgumentDefaultsHelpFormatter,
    )
    parser.add_argument(
        "--app",
        type=Path,
        default=Path("dist/macos-arm64/HarryPotter2.app"),
        help=".app bundle or Mach-O executable, relative to the repository",
    )
    parser.add_argument(
        "--data-root",
        type=Path,
        default=Path("HarryPotter2/Unreal"),
        help="Unreal data root, relative to the repository",
    )
    parser.add_argument(
        "--maps",
        type=str,
        default=DEFAULT_MAPS,
        help="comma-separated map allowlist passed to both legs",
    )
    parser.add_argument("--ticks", type=_positive_int, default=DEFAULT_TICKS)
    parser.add_argument(
        "--timeout", type=float, default=DEFAULT_TIMEOUT_SECONDS,
        help="per-map wall-clock timeout in seconds",
    )
    parser.add_argument(
        "--tolerance", type=int, default=8,
        help="maximum accepted per-channel delta between drivers",
    )
    parser.add_argument(
        "--vulkan-icd",
        type=Path,
        default=Path("/opt/homebrew/etc/vulkan/icd.d/MoltenVK_icd.json"),
        help="Vulkan ICD manifest forwarded to the vulkan leg",
    )
    parser.add_argument(
        "--output",
        type=Path,
        required=True,
        help="combined matrix report path; per-leg reports are written as "
             "<stem>-xopengl.json and <stem>-vulkan.json beside it",
    )
    return parser.parse_args()


def _emit(output: Path, report: dict[str, object]) -> None:
    """Write the combined matrix report and echo it to stdout."""
    encoded = (
        json.dumps(report, ensure_ascii=True, allow_nan=False, indent=2,
                   sort_keys=True)
        + "\n"
    ).encode("utf-8")
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_bytes(encoded)
    print(encoded.decode("utf-8"), end="")


def main() -> int:
    arguments = _arguments()
    command = [
        "matrix_compare.py", f"--maps={arguments.maps}",
        f"--ticks={arguments.ticks}", f"--tolerance={arguments.tolerance}",
    ]
    xopengl_output = _leg_output(arguments.output, "xopengl")
    vulkan_output = _leg_output(arguments.output, "vulkan")
    artifacts = [str(xopengl_output), str(vulkan_output)]
    try:
        launch_leg(arguments, "xopengl", xopengl_output)
        launch_leg(arguments, "vulkan", vulkan_output)
        xopengl_report = json.loads(xopengl_output.read_text(encoding="utf-8"))
        vulkan_report = json.loads(vulkan_output.read_text(encoding="utf-8"))
        matrix, failures = compare_maps(
            xopengl_report, vulkan_report, tolerance=arguments.tolerance,
        )
    except MatrixCompareError as error:
        report = build_report(
            {}, [str(error)],
            tolerance=arguments.tolerance, artifacts=artifacts,
            command=command, error=str(error),
        )
        _emit(arguments.output, report)
        return {"pass": 0, "fail": 1, "blocked": 2}[report["status"]]
    except (OSError, ValueError) as error:
        report = build_report(
            {}, [f"matrix infrastructure error: {error}"],
            tolerance=arguments.tolerance, artifacts=artifacts,
            command=command, error=str(error), force_blocked=True,
        )
        _emit(arguments.output, report)
        return {"pass": 0, "fail": 1, "blocked": 2}[report["status"]]
    report = build_report(
        matrix, failures,
        tolerance=arguments.tolerance, artifacts=artifacts, command=command,
    )
    _emit(arguments.output, report)
    return {"pass": 0, "fail": 1, "blocked": 2}[report["status"]]


if __name__ == "__main__":
    raise SystemExit(main())
