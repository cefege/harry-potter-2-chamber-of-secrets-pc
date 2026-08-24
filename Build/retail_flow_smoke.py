#!/usr/bin/env python3
"""Automate the scriptable portion of the HP2 Rust-era retail-flow gate.

The full product gate is partly manual (clicking through the frontend,
creating a save by playing). This harness pre-verifies everything a script
can observe so the manual pass starts from a known-good baseline:

1. **Windowed launch** — start the packaged app windowed against the retail
   data root with sounds enabled, let it run for ``--seconds-per-step``
   (via the ``HP2_RUN_SECONDS`` scripted auto-quit contract), and require a
   clean exit with ``<HP2_RES> presented_frames=N > 0``.
2. **Continue flag** — when the player's own ``Save0.usa`` exists in the
   Application Support tree (the harness never creates saves), relaunch with
   the ``-LOAD=Save0.usa`` continue flag and apply the same contract.

Launches reuse ``game_test.run_command`` for process-group isolation
(separate session, isolated HOME/TMPDIR, strict failure-marker scan), so a
run can never leak processes or touch the caller's mutable state. An app
that exits early is reported as **blocked** with its exact error line —
known retail gaps are findings, not crashes of the harness itself.

Interactive steps are never faked; the explicit manual instructions print
at the end of every run.

Exit codes: 0 all automated steps passed, 1 at least one hard failure,
2 steps passed/blocked but nothing failed.
"""

from __future__ import annotations

import argparse
import json
import math
import os
from pathlib import Path
import sys
import tempfile

if __package__:
    from . import game_test
else:
    import game_test

DEFAULT_APP = Path("dist/macos-arm64-rs/HarryPotter2.app")
DEFAULT_DATA_ROOT = Path(
    "~/Library/Application Support/Harry Potter 2/Data/Retail"
)
DEFAULT_MAP_TOKEN = "..\\Maps\\Entry.unr"
CONTINUE_FLAG = "-LOAD=Save0.usa"
SAVE_FILE_NAME = "Save0.usa"
# The legacy mutable-state tree the retail flow's saves live in. The
# harness reads it read-only to decide whether step 2 applies at all.
USER_SAVE_DIRECTORY = Path(
    "~/Library/Application Support/Harry Potter 2/User/Save"
)
# Headroom over HP2_RUN_SECONDS before run_command's SIGTERM/SIGKILL ladder
# may fire; the app should always close itself first.
SELF_EXIT_HEADROOM_SECONDS = 20.0


def _error_tail(log_text: str, limit: int = 6) -> str:
    lines = [line.rstrip() for line in log_text.splitlines() if line.strip()]
    return " | ".join(lines[-limit:]) if lines else "(no output captured)"


def _save_file() -> Path | None:
    expanded = USER_SAVE_DIRECTORY.expanduser() / SAVE_FILE_NAME
    return expanded if expanded.is_file() else None


def _launch_arguments(
    executable: Path, data_root: Path, map_token: str, *, continue_flag: bool
) -> list[str]:
    # Interactive windowed mode: no -testticks (a positive count selects the
    # headless harness path), no -nosound (the gate verifies sound startup),
    # no -NOFRONTEND markers. -log keeps stdout/stderr informative.
    arguments = [
        str(executable),
        f"-datadir={data_root}",
        map_token,
        "-window",
        "-log",
    ]
    if continue_flag:
        arguments.append(CONTINUE_FLAG)
    return arguments


def _classify(
    result: dict[str, object], seconds_per_step: float
) -> tuple[str, str]:
    """Map one lifecycle-facts dict onto pass/blocked/fail plus the reason."""
    if result["launch_error"] is not None:
        return "fail", f"launch error: {result['launch_error']}"
    if result["timed_out"]:
        return (
            "fail",
            "app did not self-close within "
            f"{seconds_per_step}+{SELF_EXIT_HEADROOM_SECONDS}s; "
            "HP2_RUN_SECONDS contract broken",
        )
    if result["failure_markers"]:
        return (
            "fail",
            "strict failure markers present: "
            + "; ".join(
                sorted(
                    {
                        str(marker["pattern"])
                        for marker in result["failure_markers"]
                    }
                )
            ),
        )
    if result["cleanup_error"] is not None or result["orphaned_process_group"]:
        return "fail", f"process-group cleanup problem: {result['cleanup_error']}"
    frames = result["resources"].get("presented_frames")
    if result["exit_status"] == 0:
        if isinstance(frames, (int, float)) and frames > 0:
            return "pass", f"clean self-exit after {frames} presented frame(s)"
        return (
            "fail",
            "clean exit but <HP2_RES> presented_frames marker missing or zero "
            f"(resources={result['resources']})",
        )
    # Nonzero exit: the app reported an engine-side condition. That is a
    # finding against the product gate (blocked), not a harness failure —
    # surface the exact error text verbatim.
    try:
        log_text = Path(str(result["log"])).read_text(
            encoding="utf-8", errors="backslashreplace"
        )
    except OSError:
        log_text = ""
    return (
        "blocked",
        f"app exited {result['exit_status']}: {_error_tail(log_text)}",
    )


def _run_step(
    *,
    name: str,
    executable: Path,
    data_root: Path,
    map_token: str,
    seconds_per_step: float,
    artifact_dir: Path,
    repo_root: Path,
    continue_flag: bool = False,
) -> dict[str, object]:
    log_path = artifact_dir / f"{name}.log"
    command = _launch_arguments(
        executable, data_root, map_token, continue_flag=continue_flag
    )
    result = game_test.run_command(
        command,
        repo_root=repo_root,
        timeout_seconds=seconds_per_step + SELF_EXIT_HEADROOM_SECONDS,
        log_path=log_path,
        displayed_command=[
            game_test._display_path(executable, repo_root),
            f"-datadir={game_test._display_path(data_root, repo_root)}",
            map_token,
            *command[3:],
        ],
        extra_env={"HP2_RUN_SECONDS": repr(float(seconds_per_step))},
    )
    status, detail = _classify(result, seconds_per_step)
    return {
        "step": name,
        "status": status,
        "detail": detail,
        "command": result["command"],
        "log": result["log"],
        "exit_status": result["exit_status"],
        "duration_seconds": result["duration_seconds"],
        "resources": result["resources"],
        "failure_markers": result["failure_markers"],
    }


def _manual_instructions(save_file: Path | None) -> list[str]:
    instructions = [
        "MANUAL steps remaining (the harness cannot click through UI):",
        f"  1. Open {DEFAULT_APP.as_posix()} normally (double-click or `open`).",
        "  2. Click New Game and click through the intro until gameplay is "
        "interactive; confirm audio is audible.",
    ]
    if save_file is not None:
        instructions.append(
            "  3. Return to the frontend and load "
            f"{SAVE_FILE_NAME} via Continue; confirm the saved level restores."
        )
        instructions.append(
            "  4. Play briefly and save again; confirm the new save appears."
        )
    else:
        instructions.append(
            f"  3. Create a save ({SAVE_FILE_NAME}) by playing; rerun this "
            "harness afterwards so the continue-flag step is exercised too."
        )
    return instructions


def _argument_positive_float(value: str) -> float:
    try:
        number = float(value)
    except ValueError as error:
        raise argparse.ArgumentTypeError(str(error)) from error
    if math.isnan(number) or math.isinf(number) or number <= 0.0:
        raise argparse.ArgumentTypeError("must be a positive finite number")
    return number


def _arguments(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--app", type=Path, default=DEFAULT_APP)
    parser.add_argument("--data-root", type=Path, default=DEFAULT_DATA_ROOT)
    parser.add_argument("--map", dest="selected_map", default=DEFAULT_MAP_TOKEN)
    parser.add_argument(
        "--seconds-per-step", type=_argument_positive_float, default=8.0
    )
    parser.add_argument(
        "--artifact-dir",
        type=Path,
        default=os.environ.get("HP2_ARTIFACT_DIR") or None,
        help="directory for logs and the JSON report "
        "(default: $HP2_ARTIFACT_DIR, else a fresh temp directory)",
    )
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    arguments = _arguments(argv)
    repo_root = game_test._repo_root()
    try:
        app = game_test._resolve_path(arguments.app, repo_root, strict=True)
        data_root = game_test._resolve_path(
            arguments.data_root, repo_root, strict=True
        )
        game_test._validate_data_root(data_root)
    except game_test.GameTestError as error:
        print(f"retail_flow_smoke: blocked: {error}", file=sys.stderr)
        return 2
    executable = game_test._bundle_executable(app)
    game_test._validate_native_arm64(executable)

    if arguments.artifact_dir is not None:
        artifact_dir = game_test._resolve_path(
            arguments.artifact_dir, repo_root, strict=False
        )
        artifact_dir.mkdir(parents=True, exist_ok=True)
    else:
        artifact_dir = Path(tempfile.mkdtemp(prefix="hp2-retail-flow-"))
    print(f"retail_flow_smoke: artifacts under {artifact_dir}")

    common = {
        "executable": executable,
        "data_root": data_root,
        "map_token": arguments.selected_map,
        "seconds_per_step": arguments.seconds_per_step,
        "artifact_dir": artifact_dir,
        "repo_root": repo_root,
    }
    steps = [_run_step(name="windowed_launch", **common)]
    save_file = _save_file()
    if save_file is None:
        steps.append(
            {
                "step": "continue_flag",
                "status": "blocked",
                "detail": f"no {SAVE_FILE_NAME} under {USER_SAVE_DIRECTORY}; "
                "the harness never creates saves, so the -LOAD step does not apply",
            }
        )
    else:
        steps.append(_run_step(name="continue_flag", continue_flag=True, **common))

    report = {
        "format_version": 1,
        "app": str(app),
        "engine": str(executable),
        "data_root": str(data_root),
        "map_token": arguments.selected_map,
        "seconds_per_step": arguments.seconds_per_step,
        "steps": steps,
        "manual_instructions": _manual_instructions(save_file),
    }
    report_path = artifact_dir / "retail-flow-smoke.json"
    game_test._write_atomic(
        report_path, json.dumps(report, indent=2).encode("utf-8")
    )

    statuses = [str(step["status"]) for step in steps]
    labels = {"pass": "PASS", "blocked": "BLOCKED", "fail": "FAIL"}
    for step in steps:
        prefix = labels[step["status"]]
        print(f"[{prefix}] {step['step']}: {step['detail']}")
    for line in report["manual_instructions"]:
        print(line)
    print(
        "retail_flow_smoke: report "
        f"{game_test._display_path(report_path, repo_root)}"
    )

    if "fail" in statuses:
        return 1
    if all(status == "pass" for status in statuses):
        return 0
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
