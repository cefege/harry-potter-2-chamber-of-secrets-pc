#!/usr/bin/env python3
"""determinism_double_run: two full launches of one map must agree exactly.

Launches the installed app bundle twice against the prototype data root for
the same map with the same tick budget and HP2_CAPTURE_FRAMES enabled, then
asserts run-to-run determinism on two axes:

* normalized engine logs are byte-identical (timing/duration/pid-shaped noise
  is stripped through LOG_NOISE_PATTERNS / DROP_LINE_PATTERNS below), and
* the final captured frame PNGs compare "same" via Tools/baseline_compare.py
  with per-channel tolerance 2.

A failure reports the first divergent log line pair or the frame comparison
delta. Emits a schema-v1 runner report and exits nonzero on any failure.

Requires the dist app bundle and prototype data; ctest registers this test
only when both exist at configure time.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
from pathlib import Path

REPOSITORY_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(REPOSITORY_ROOT / "Build"))
sys.path.insert(0, str(REPOSITORY_ROOT / "Tools"))

import baseline_compare  # noqa: E402
import game_test  # noqa: E402

SCHEMA_VERSION = 1
REPORT_NAME = "determinism_double_run"
# Capture hook lives in the built hp2_game; the installed dist bundle may be
# stale. Prefer a fresh build-tree binary when one exists.
DEFAULT_APP = Path("out/macos-arm64/hp2_game")
DEFAULT_APP_FALLBACK = Path("dist/macos-arm64/HarryPotter2.app")
DEFAULT_DATA_ROOT = Path("HarryPotter2/Unreal")
DEFAULT_MAP_STEM = "PrivetDr"
DEFAULT_TICKS = 60
FRAME_TOLERANCE = 2
DEFAULT_TIMEOUT_SECONDS = 120.0

# Run-to-run noise scrubbed before the byte comparison. Each pattern removes
# one shape of nondeterministic text observed in real double-run logs:
# process ids, pointer/hex addresses, unit-bearing durations, throughput
# figures, the randomized isolated-home directory (game_test mkdtemp), and
# the per-launch frames directory label.
LOG_NOISE_PATTERNS: tuple[re.Pattern[str], ...] = tuple(
    re.compile(pattern, re.IGNORECASE)
    for pattern in (
        r"\bpid\s*[=: ]\s*\d+\b",
        r"\b0x[0-9a-f]{4,}\b",
        r"\b\d+(?:\.\d+)?\s*(?:ms|msec|milliseconds?)\b",
        r"\b\d+(?:\.\d+)?\s*(?:secs?|seconds?)\b",
        r"\b\d+(?:\.\d+)?\s*fps\b",
        r"\b\d+(?:\.\d+)?\s*k?bps\b",
        r"determinism_double_run/run_\d+",
        r"\bhp2-game-test-[0-9a-z_]+\b",
    )
)

# Whole lines dropped outright: pure timing/telemetry rows carry no contract
# content and resist substring scrubbing.
DROP_LINE_PATTERNS: tuple[re.Pattern[str], ...] = tuple(
    re.compile(pattern, re.IGNORECASE)
    for pattern in (
        r"^\s*\d+(?:\.\d+)?\s*ms\b",
        r"^Load time\b",
        r"\btiming\b.*\b\d",
        # Exit-time purge bookkeeping: the object total wobbles by a few
        # entries between identical runs (async finalizer ordering) while
        # refcounts stay stable, so the raw line is not byte-contractable.
        r"^Garbage: objects:",
    )
)

# Per-shader-program driver diagnostics. The engine emits these lines per
# compiled/linked program, but GLSL compilation is serviced asynchronously by
# the driver: whether (and how often) a line is emitted varies between
# otherwise pixel-identical runs. They carry no determinism contract content,
# so they are dropped outright; a genuinely broken compile breaks the frame
# comparison instead.
DROP_SHADER_DIAGNOSTIC_PATTERNS: tuple[re.Pattern[str], ...] = tuple(
    re.compile(pattern)
    for pattern in (
        r"^XOpenGL: No compiler messages for ",
        r"^XOpenGL: compiler messages for ",
        r"^XOpenGL: No linker messages for ",
        r"^XOpenGL: linker messages for ",
        r"^XOpenGL: Log linking ",
        r"^XOpenGL: invalid or unused shader var ",
        r"^WARNING: Output of \w+ shader ",
    )
)



class DeterminismError(Exception):
    """Raised when the double-run contract fails or cannot be attempted."""


def normalize_log_line(line: str) -> str | None:
    """Scrub one decoded log line; None means the line is dropped."""
    for pattern in DROP_LINE_PATTERNS:
        if pattern.search(line):
            return None
    for pattern in DROP_SHADER_DIAGNOSTIC_PATTERNS:
        if pattern.search(line):
            return None
    for pattern in LOG_NOISE_PATTERNS:
        line = pattern.sub("<noise>", line)
    return line


def normalize_log(payload: bytes) -> list[str]:
    text = payload.decode("utf-8", errors="backslashreplace")
    normalized = []
    for line in text.splitlines():
        cleaned = normalize_log_line(line)
        if cleaned is not None:
            normalized.append(cleaned)
    return normalized



def _first_divergence(
    baseline: list[str], candidate: list[str]
) -> tuple[int, str, str] | None:
    for index in range(max(len(baseline), len(candidate))):
        baseline_line = baseline[index] if index < len(baseline) else "<end>"
        candidate_line = candidate[index] if index < len(candidate) else "<end>"
        if baseline_line != candidate_line:
            return index, baseline_line, candidate_line
    return None


def _frames_directory(path: Path) -> list[Path]:
    return sorted(path.glob("frame_*.png"))


def _select_map(data_root: Path, map_stem: str) -> str:
    wanted = map_stem.casefold()
    for listed in game_test.list_maps(data_root):
        if Path(listed["map"]).stem.casefold() == wanted:
            return listed["map"]
    raise DeterminismError(f"map stem not found under {data_root}: {map_stem}")


def _launch(
    *,
    label: str,
    artifacts: Path,
    executable: Path,
    data_root: Path,
    selected_map: str,
    ticks: int,
    timeout_seconds: float,
) -> tuple[dict[str, object], list[Path]]:
    run_directory = artifacts / label
    frames_directory = run_directory / "frames"
    if frames_directory.exists():
        import shutil

        shutil.rmtree(frames_directory)
    frames_directory.mkdir(parents=True, exist_ok=True)
    result = game_test.run_game(
        app=executable,
        data_root=data_root,
        selected_map=selected_map,
        renderer="xopengl",
        ticks=ticks,
        timeout_seconds=timeout_seconds,
        log_path=run_directory / "engine.log",
        no_sound=True,
        extra_env={
            "HP2_CAPTURE_FRAMES": str(frames_directory),
            "HP2_CAPTURE_MAP": Path(selected_map).stem,
            "HP2_CAPTURE_TICKS": str(ticks),
        },
    )
    if not result["passed"]:
        raise DeterminismError(
            f"{label}: launch failed (exit_status={result['exit_status']}, "
            f"timed_out={result['timed_out']}, "
            f"failure_markers={result['failure_markers']!r})"
        )
    frames = _frames_directory(frames_directory)
    if not frames:
        raise DeterminismError(f"{label}: no frames captured")
    return result, frames


def run_contract(
    *,
    executable: Path,
    data_root: Path,
    map_stem: str,
    ticks: int,
    timeout_seconds: float,
    artifacts: Path,
) -> dict[str, object]:
    """Execute both launches and both comparisons; returns a schema-v1 report."""
    selected_map = _select_map(data_root, map_stem)
    command = {
        "app": game_test._display_path(executable, REPOSITORY_ROOT),
        "map": selected_map,
        "renderer": "xopengl",
        "ticks": ticks,
        "timeout_seconds": timeout_seconds,
        "launches": ["run_1", "run_2"],
    }
    profile = {
        "map": selected_map,
        "map_stem": map_stem,
        "ticks": ticks,
        "frame_tolerance": FRAME_TOLERANCE,
    }

    results: dict[str, dict[str, object]] = {}
    frames: dict[str, list[Path]] = {}
    for label in ("run_1", "run_2"):
        results[label], frames[label] = _launch(
            label=label,
            artifacts=artifacts,
            executable=executable,
            data_root=data_root,
            selected_map=selected_map,
            ticks=ticks,
            timeout_seconds=timeout_seconds,
        )

    def fail(reason_code: str, detail: dict[str, object]) -> dict[str, object]:
        return {
            "schema": SCHEMA_VERSION,
            "name": REPORT_NAME,
            "status": "fail",
            "invariant": (
                "two full launches of one map produce identical normalized "
                "logs and identical final frames within tolerance"
            ),
            "reason_code": reason_code,
            "data": {"profile": profile, **detail},
            "artifacts": [
                game_test._display_path(artifacts / "run_1" / "engine.log", REPOSITORY_ROOT),
                game_test._display_path(artifacts / "run_2" / "engine.log", REPOSITORY_ROOT),
                game_test._display_path(artifacts / "run_1" / "frames", REPOSITORY_ROOT),
                game_test._display_path(artifacts / "run_2" / "frames", REPOSITORY_ROOT),
            ],
            "command": command,
            "exit_reason": detail.get("summary", "determinism contract violated"),
        }

    normalized_1 = normalize_log(
        (artifacts / "run_1" / "engine.log").read_bytes()
    )
    normalized_2 = normalize_log(
        (artifacts / "run_2" / "engine.log").read_bytes()
    )
    divergence = _first_divergence(normalized_1, normalized_2)
    if divergence is not None:
        index, baseline_line, candidate_line = divergence
        return fail(
            "renderer.log_diverged",
            {
                "divergent_line_index": index,
                "run_1_line": baseline_line,
                "run_2_line": candidate_line,
                "summary": f"normalized logs diverge at line {index}",
            },
        )

    if len(frames["run_1"]) != len(frames["run_2"]):
        return fail(
            "renderer.frame_count_mismatch",
            {
                "run_1_frame_count": len(frames["run_1"]),
                "run_2_frame_count": len(frames["run_2"]),
                "summary": "capture produced different frame counts",
            },
        )
    final_baseline = frames["run_1"][-1]
    final_candidate = frames["run_2"][-1]
    comparison = baseline_compare.compare_png_files(
        final_baseline, final_candidate, tolerance=FRAME_TOLERANCE
    )
    if comparison["verdict"] != "same":
        return fail(
            "renderer.final_frame_differs"
            if comparison["verdict"] == "differs"
            else "renderer.final_frame_size_mismatch",
            {
                "final_frame_comparison": comparison,
                "summary": (
                    "final-frame PNG delta "
                    f"{comparison['max_channel_delta']} exceeds tolerance "
                    f"{FRAME_TOLERANCE}"
                ),
            },
        )

    profile["frame_count_per_run"] = len(frames["run_1"])
    profile["normalized_log_lines"] = len(normalized_1)
    return {
        "schema": SCHEMA_VERSION,
        "name": REPORT_NAME,
        "status": "pass",
        "invariant": (
            "two full launches of one map produce identical normalized "
            "logs and identical final frames within tolerance"
        ),
        "data": {
            "profile": profile,
            "final_frame_comparison": comparison,
        },
        "artifacts": [
            game_test._display_path(artifacts / "run_1" / "engine.log", REPOSITORY_ROOT),
            game_test._display_path(artifacts / "run_2" / "engine.log", REPOSITORY_ROOT),
            game_test._display_path(artifacts / "run_1" / "frames", REPOSITORY_ROOT),
            game_test._display_path(artifacts / "run_2" / "frames", REPOSITORY_ROOT),
        ],
        "command": command,
        "exit_reason": "both launches agree on logs and final frame",
    }


def _arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--app", type=Path, default=DEFAULT_APP)
    parser.add_argument("--data-root", type=Path, default=DEFAULT_DATA_ROOT)
    parser.add_argument("--map-stem", default=DEFAULT_MAP_STEM)
    parser.add_argument("--ticks", type=int, default=DEFAULT_TICKS)
    parser.add_argument("--timeout", type=float, default=DEFAULT_TIMEOUT_SECONDS)
    parser.add_argument(
        "--report",
        type=Path,
        default=None,
        help="optional path for the JSON report (also printed to stdout)",
    )
    return parser.parse_args()


def main() -> int:
    arguments = _arguments()
    artifact_dir = os.environ.get("HP2_ARTIFACT_DIR")
    try:
        if not artifact_dir:
            raise DeterminismError("HP2_ARTIFACT_DIR is not set")
        artifacts = Path(artifact_dir) / REPORT_NAME
        artifacts.mkdir(parents=True, exist_ok=True)
        repo_root = game_test._repo_root()
        app_arg = arguments.app
        if app_arg == DEFAULT_APP and not (repo_root / app_arg).is_file():
            app_arg = DEFAULT_APP_FALLBACK
        executable = game_test._resolve_path(app_arg, repo_root, strict=True)
        data_root = game_test._resolve_path(arguments.data_root, repo_root, strict=True)
        game_test._validate_data_root(data_root)
        report = run_contract(
            executable=executable,
            data_root=data_root,
            map_stem=arguments.map_stem,
            ticks=arguments.ticks,
            timeout_seconds=arguments.timeout,
            artifacts=artifacts,
        )
    except (DeterminismError, game_test.GameTestError, OSError) as error:
        report = {
            "schema": SCHEMA_VERSION,
            "name": REPORT_NAME,
            "status": "blocked",
            "invariant": (
                "two full launches of one map produce identical normalized "
                "logs and identical final frames within tolerance"
            ),
            "reason_code": "data.contract_blocked",
            "data": {"profile": {}, "error": str(error)},
            "artifacts": [],
            "command": {},
            "exit_reason": f"contract could not run: {error}",
        }
    encoded = json.dumps(report, indent=2, sort_keys=True) + "\n"
    if arguments.report is not None:
        arguments.report.parent.mkdir(parents=True, exist_ok=True)
        arguments.report.write_text(encoded, encoding="utf-8")
    sys.stdout.write(encoded)
    return 0 if report["status"] == "pass" else 1


if __name__ == "__main__":
    raise SystemExit(main())
