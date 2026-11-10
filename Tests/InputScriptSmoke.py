#!/usr/bin/env python3
"""Scripted-input smoke: deterministic keybind execution driven by the engine.

Two modes:

1. Contract tests (default): deterministic unittest suite over the replay
   compiler, fixture parser, and smoke runner plumbing using stub binaries.
   No engine build required; safe for ctest.
2. Live smoke (--smoke): compiles Tests/Fixtures/input_script_smoke.txt into
   an FReplay ``.rep`` input stream, seeds an isolated user directory with it
   plus a ``K=SHOT`` probe binding, launches the selected executable with
   ``-REPLAY=... -TESTTICKS=...``, and emits a report-schema-v1 JSON document
   on stdout. Exits 0 (pass), 1 (fail), or 3 (blocked).

Mechanism (no engine hook needed): FReplay (Engine/Src/UnReplay.cpp) replays
binary input-event records through UGameEngine::InputEvent each frame — the
same path SDL input takes — while suppressing real input and reseeding
srand(1), so the run is fully deterministic. ``-TESTTICKS`` bounds the main
loop with a clean exit. The probe screenshots written by the ``SHOT``
viewport command through the ``K=SHOT`` binding are hard evidence that
scripted input flowed through the real keybind pipeline.
"""

from __future__ import annotations

import argparse
import json
import math
import os
import re
import struct
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path
from typing import Union

REPOSITORY_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(REPOSITORY_ROOT / "Build"))
import game_test  # noqa: E402
import hp2_proc  # noqa: E402

REPORT_SCHEMA_VERSION = 1
SMOKE_NAME = "input_script_smoke"
SMOKE_INVARIANT = (
    "the engine replays a deterministic input script through the real "
    "keybind pipeline on the selected map, writes the fixture-requested "
    "probe screenshots and required user files, and exits within the tick budget"
)
DEFAULT_APP = REPOSITORY_ROOT / "dist" / "macos-arm64" / "HarryPotter2.app"
DEFAULT_DATA_ROOT = REPOSITORY_ROOT / "HarryPotter2" / "Unreal"
DEFAULT_FIXTURE = REPOSITORY_ROOT / "Tests" / "Fixtures" / "input_script_smoke.txt"
DEFAULT_TICKS = 240
DEFAULT_TIMEOUT_SECONDS = 120.0
BLOCKED_EXIT_CODE = 3
REPLAY_STEM = "InputScriptSmoke"
PROBE_KEY = "K"
PROBE_BINDING = "SHOT"
LOOP_MARKER = "Entering main loop."

# EInputKey values used by fixtures (Engine/Inc/EngineClasses.h).
KEY_IDS = {
    "LeftMouse": 1, "RightMouse": 2, "MiddleMouse": 4,
    "Backspace": 8, "Tab": 9, "Enter": 13, "Shift": 16, "Ctrl": 17,
    "Alt": 18, "Pause": 19, "CapsLock": 20, "Escape": 27, "Space": 32,
    "PageUp": 33, "PageDown": 34, "End": 35, "Home": 36,
    "Left": 37, "Up": 38, "Right": 39, "Down": 40, "Insert": 45, "Delete": 46,
    **{str(digit): 48 + digit for digit in range(10)},
    **{chr(code): code for code in range(ord("A"), ord("Z") + 1)},
    "F1": 112, "F2": 113, "F3": 114, "F4": 115, "F5": 116, "F6": 117,
    "F7": 118, "F8": 119, "F9": 120, "F10": 121, "F11": 122, "F12": 123,
    "Comma": 124, "Minus": 125, "Period": 126, "Slash": 127,
}

IST_PRESS, IST_HOLD, IST_RELEASE, IST_AXIS = 1, 2, 3, 4
# EInputKey::IK_Play in Engine/Inc/EngineClasses.h; replay frame terminator.
IK_PLAY = 250
DELTA_FLAG = 0x80


class FixtureError(game_test.GameTestError):
    """Raised when the fixture DSL cannot be parsed."""


class ProbeBindingError(game_test.GameTestError):
    """Raised when the isolated DefUser.ini lacks the probe key row."""


def _compact_index(value: int) -> bytes:
    """Encode a UE FCompactIndex (Core/Src/UnObj.cpp FCompactIndex<<)."""
    if value < 0:
        raise ValueError("replay compiler only emits non-negative indices")
    if value < 0x40:
        return bytes((value,))
    out = bytearray(((value & 0x3F) + 0x40,))
    v = value >> 6
    while True:
        byte = v if v < 0x80 else (v & 0x7F) + 0x80
        out.append(byte)
        if not byte & 0x80:
            break
        v >>= 7
    return bytes(out)


def serialize_fstring(text: str) -> bytes:
    """Pure-ANSI FString wire form: compact-index count (incl. NUL) + bytes."""
    payload = text.encode("ascii")
    return _compact_index(len(payload) + 1) + payload + b"\x00"


def _event_bytes(key: int, state: int, delta: float = 0.0) -> bytes:
    """One FReplay::FInputEvent record (UnReplay.cpp operator<<)."""
    header = bytes((key, state | (DELTA_FLAG if delta != 0.0 else 0)))
    return header + (struct.pack("<f", delta) if delta != 0.0 else b"")


def _tick_record(tick_delta: float) -> bytes:
    """Frame terminator record {IK_Play, IST_Axis, delta}."""
    return _event_bytes(IK_PLAY, IST_AXIS, tick_delta)


ReplayEvent = Union[tuple[int, int], tuple[int, int, float]]


def parse_fixture(text: str) -> tuple[float, list[list[ReplayEvent]]]:
    """Parse the fixture DSL into (tick_delta, frames of replay events).

    Raises FixtureError on unknown directives, unknown keys, misplaced
    ``tick``, or unbalanced down/up sequences.
    """
    tick: float | None = None
    frames: list[list[ReplayEvent]] = []
    down_keys: set[int] = set()

    def new_frame() -> None:
        frames.append([])

    def add(key_name: str, state: int) -> None:
        if key_name not in KEY_IDS:
            raise FixtureError(f"unknown input key name: {key_name!r}")
        key = KEY_IDS[key_name]
        if state == IST_PRESS:
            if key in down_keys:
                raise FixtureError(f"{key_name} pressed while already down")
            down_keys.add(key)
        elif state == IST_RELEASE:
            if key not in down_keys:
                raise FixtureError(f"{key_name} released while not down")
            down_keys.discard(key)
        frames[-1].append((key, state))

    new_frame()
    for line_number, raw in enumerate(text.splitlines(), start=1):
        line = raw.split("#", 1)[0].strip()
        if not line:
            continue
        parts = line.split()
        directive, arguments = parts[0], parts[1:]
        if directive == "tick":
            if tick is not None or len(frames) != 1 or frames[0]:
                raise FixtureError(f"line {line_number}: tick must be the first directive")
            if len(arguments) != 1:
                raise FixtureError(f"line {line_number}: tick needs one value")
            try:
                tick = float(arguments[0])
            except ValueError as error:
                raise FixtureError(f"line {line_number}: bad tick value") from error
            if not tick > 0.0:
                raise FixtureError(f"line {line_number}: tick must be positive")
        elif directive == "idle":
            if len(arguments) != 1 or not arguments[0].isdigit():
                raise FixtureError(f"line {line_number}: idle needs a frame count")
            for _ in range(int(arguments[0])):
                new_frame()
        elif directive == "mouse":
            if len(arguments) != 2:
                raise FixtureError(f"line {line_number}: mouse needs x and y")
            try:
                x, y = (float(value) for value in arguments)
            except ValueError as error:
                raise FixtureError(f"line {line_number}: bad mouse coordinate") from error
            if not all(math.isfinite(value) and abs(value) <= 1_000_000 for value in (x, y)):
                raise FixtureError(f"line {line_number}: mouse coordinates must be finite and bounded")
            # IST_MAX mouse records are the native replay's absolute viewport position.
            frames[-1].extend(((228, 5, x), (229, 5, y)))
            new_frame()
        elif directive == "press":
            if len(arguments) != 1:
                raise FixtureError(f"line {line_number}: press needs a key")
            add(arguments[0], IST_PRESS)
            new_frame()
            add(arguments[0], IST_RELEASE)
            new_frame()
        elif directive == "hold":
            if len(arguments) != 2 or not arguments[1].isdigit():
                raise FixtureError(f"line {line_number}: hold needs a key and frame count")
            key_name, count = arguments[0], int(arguments[1])
            if count < 1:
                raise FixtureError(f"line {line_number}: hold needs at least one frame")
            add(key_name, IST_PRESS)
            for _ in range(count - 1):
                new_frame()
                add(key_name, IST_HOLD)
            new_frame()
            add(key_name, IST_RELEASE)
            new_frame()
        else:
            raise FixtureError(f"line {line_number}: unknown directive {directive!r}")

    if tick is None:
        raise FixtureError("fixture has no tick directive")
    if down_keys:
        names = sorted(name for name, key in KEY_IDS.items() if key in down_keys)
        raise FixtureError(f"fixture ends with keys still held: {names}")
    return tick, frames

def validate_launch_safe_frames(frames: list[list[ReplayEvent]]) -> None:
    """Require the initial empty frame that gives FReplay its first tick."""
    if not frames or frames[0]:
        raise FixtureError(
            "launch-safe replay fixtures must begin with an empty frame (use idle 1)"
        )


def compile_replay(url: str, tick_delta: float, frames: list[list[ReplayEvent]]) -> bytes:
    """Compile parsed frames into the exact FReplay .rep byte stream.

    Stream layout (UnReplay.cpp): serialized starting URL FString, then per
    frame zero or more FInputEvent records followed by the frame-tick record
    {IK_Play, IST_Axis, delta}. This raw encoder deliberately preserves
    event-first frames for wire coverage; callers that stage a launch replay
    must validate the initial empty frame separately.
    """
    out = bytearray()
    out += serialize_fstring(url)
    for events in frames:
        for event in events:
            out += _event_bytes(*event)
        out += _tick_record(tick_delta)
    return bytes(out)


def inject_probe_binding(defuser_text: str) -> str:
    """Rewrite the EMPTY probe-key row in DefUser.ini content to the probe."""
    pattern = re.compile(rf"^{re.escape(PROBE_KEY)}=[ \t]*$", re.MULTILINE)
    replaced = pattern.sub(f"{PROBE_KEY}={PROBE_BINDING}", defuser_text, count=1)
    if replaced == defuser_text:
        raise ProbeBindingError(
            "no empty K= row in DefUser.ini to host the SHOT probe binding"
        )
    return replaced




_USER_DIR_RELATIVE = Path("Library/Application Support/Harry Potter 2/User")


def apply_bindings(user_ini: str, bindings: tuple[str, ...]) -> str:
    """Replace target key rows so existing defaults cannot override the scenario."""
    if not bindings:
        return user_ini
    updates: dict[str, str] = {}
    for binding in bindings:
        key, separator, command = binding.partition("=")
        key = key.strip()
        if not separator or not key or "\n" in binding or "\r" in binding:
            raise ProbeBindingError("additional bindings must be single-line KEY=COMMAND rows")
        updates[key.casefold()] = f"{key}={command}"
    section = re.search(r"(?im)^\[Engine\.Input\][ \t]*$", user_ini)
    if section is None:
        raise ProbeBindingError("no [Engine.Input] section for additional bindings")
    next_section = re.search(r"(?m)^\[", user_ini[section.end():])
    end = section.end() + next_section.start() if next_section else len(user_ini)
    lines = [
        line for line in user_ini[section.end():end].splitlines()
        if line.partition("=")[0].strip().casefold() not in updates
    ]
    return user_ini[:section.end()] + "\n".join(lines + list(updates.values())) + "\n" + user_ini[end:]


def prepare_user_home(
    sandbox_home: Path,
    data_root: Path,
    replay_bytes: bytes,
    *,
    replay_stem: str = REPLAY_STEM,
    bindings: tuple[str, ...] = (),
) -> Path:
    """Seed isolated user settings and replay stream."""
    user_dir = sandbox_home / _USER_DIR_RELATIVE
    user_dir.mkdir(parents=True, exist_ok=True)
    system = data_root / "System"
    (user_dir / "Game.ini").write_text(
        (system / "Default.ini").read_text(encoding="utf-8"), encoding="utf-8",
    )
    replacement = os.environ.get("HP2_SMOKE_GAME_INI")
    if replacement:
        # Opt-in for layout/visual investigations: run with a player's own system ini.
        (user_dir / "Game.ini").write_text(Path(replacement).read_text(encoding="utf-8"), encoding="utf-8")
    defuser = (system / "DefUser.ini").read_text(encoding="utf-8")
    user_ini = apply_bindings(inject_probe_binding(defuser), bindings)
    (user_dir / "User.ini").write_text(user_ini, encoding="utf-8")
    (user_dir / f"{replay_stem}.rep").write_bytes(replay_bytes)
    return user_dir


def build_launch_command(executable: Path, data_root: Path, ticks: int) -> list[str]:
    return [
        str(executable),
        f"-datadir={data_root}",
        f"-REPLAY={REPLAY_STEM}",
        "-wgpu",
        *([] if os.environ.get("HP2_SMOKE_FRONTEND") == "1" else ["-NOFRONTEND"]),
        "-window",
        # Verification requires the real loopback backend; disabling audio
        # would correctly fail its tick contract before any replay input.
        *([] if os.environ.get("HP2_DETERMINISTIC_VERIFICATION") == "1" else ["-nosound"]),
        f"-testticks={ticks}",
        "-log",
    ]


def _supervised_run(
    command: list[str], *, home: Path, cwd: Path, timeout_seconds: float
) -> tuple[bytes, int | None, bool, str | None]:
    """Run one supervised process group with an explicit sandbox HOME.

    Returns (captured output, exit status or None, timed_out, launch_error).
    """
    environment = os.environ.copy()
    temporary = home / "tmp"
    config = home / ".config"
    cache = home / ".cache"
    for directory in (temporary, config, cache):
        directory.mkdir(parents=True, exist_ok=True)
    environment.update({
        "HOME": str(home), "CFFIXED_USER_HOME": str(home),
        "TMPDIR": str(temporary) + os.sep, "XDG_CONFIG_HOME": str(config),
        "XDG_CACHE_HOME": str(cache), "LANG": "C", "LC_ALL": "C", "TZ": "UTC",
    })
    try:
        process = hp2_proc.popen_group(
            command, cwd=str(cwd), env=environment,
            stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
        )
    except OSError as error:
        message = f"{type(error).__name__}: {error}"
        return (
            f"input script smoke launch error: {message}\n".encode("utf-8", "backslashreplace"),
            None, False, message,
        )
    try:
        output, _ = process.communicate(timeout=timeout_seconds)
        return output, process.returncode, False, None
    except subprocess.TimeoutExpired:
        try:
            os.killpg(process.pid, 15)
        except OSError:
            pass
        try:
            output, _ = process.communicate(timeout=game_test.TERMINATION_GRACE_SECONDS)
        except subprocess.TimeoutExpired:
            try:
                os.killpg(process.pid, 9)
            except OSError:
                pass
            try:
                output, _ = process.communicate(timeout=game_test.KILL_GRACE_SECONDS)
            except subprocess.TimeoutExpired as error:
                output = error.output or b""
        return output, process.returncode, True, None


def classify_data_profile(data_root: Path) -> str:
    resolved = str(data_root)
    if resolved.endswith("out/retail-data") or "/retail-data" in resolved:
        return "data-retail"
    if resolved.endswith("HarryPotter2/Unreal"):
        return "data-prototype"
    return "data-none"


def discover_app(explicit: Path | None) -> Path | None:
    candidates = [explicit] if explicit else [DEFAULT_APP]
    for candidate in candidates:
        if candidate.is_dir():
            return candidate
        if candidate.is_file() and os.access(candidate, os.X_OK):
            return candidate
    return None


def resolve_data_root(explicit: Path | None) -> Path | None:
    """An explicit root is authoritative; otherwise HP2_TEST_DATA_ROOT, then
    the prototype tree. A missing explicit root is reported, never silently
    substituted."""
    candidates: list[Path] = []
    if explicit is not None:
        candidates.append(explicit)
    else:
        from_env = os.environ.get("HP2_TEST_DATA_ROOT")
        if from_env:
            candidates.append(Path(from_env))
        candidates.append(DEFAULT_DATA_ROOT)
    for candidate in candidates:
        if candidate is not None and (candidate / "System" / "Default.ini").is_file():
            return candidate
    return None


def run_input_script_smoke(
    *,
    app: Path | None,
    data_root: Path | None,
    artifact_dir: Path,
    fixture_path: Path = DEFAULT_FIXTURE,
    ticks: int = DEFAULT_TICKS,
    timeout_seconds: float = DEFAULT_TIMEOUT_SECONDS,
    validate_binary: bool = True,
    engine_bin: Path | None = None,
    selected_map: str = "Entry",
    bindings: tuple[str, ...] = (),
    expected_user_files: tuple[str, ...] = (),
    expected_map_loads: tuple[str, ...] = (),
) -> dict:
    """Run the live smoke and return a report-schema-v1 dict."""
    profile = classify_data_profile(data_root) if data_root else "data-none"

    def report(status: str, reason_code: str = "", exit_reason: str = "",
               command: list[str] | None = None,
               artifacts: list[dict] | None = None) -> dict:
        return {
            "schema": REPORT_SCHEMA_VERSION,
            "name": SMOKE_NAME,
            "status": status,
            "reason_code": reason_code,
            "invariant": SMOKE_INVARIANT,
            "data": {"profile": profile},
            "artifacts": artifacts or [],
            "command": command or [],
            "exit_reason": exit_reason,
        }

    root = resolve_data_root(data_root)
    if root is None:
        return report("blocked", "data.root_missing",
                      "no data root with System/Default.ini found")
    if engine_bin is not None:
        try:
            executable = game_test._resolve_path(engine_bin, REPOSITORY_ROOT, strict=True)
        except (OSError, game_test.GameTestError) as error:
            return report("blocked", "binary_missing", str(error))
    else:
        bundle = discover_app(app)
        if bundle is None:
            return report("blocked", "binary_missing",
                          "app bundle not found; build and install dist/macos-arm64 first")
        executable = game_test._bundle_executable(bundle)

    artifacts: list[dict] = []
    artifact_dir.mkdir(parents=True, exist_ok=True)

    tick_delta, frames = parse_fixture(fixture_path.read_text(encoding="utf-8"))
    validate_launch_safe_frames(frames)
    expected_probe_shots = sum(
        event[0] == KEY_IDS[PROBE_KEY] and event[1] == IST_PRESS
        for frame in frames for event in frame
    )
    replay_bytes = compile_replay(f"{selected_map}.unr", tick_delta, frames)

    sandbox_home = Path(tempfile.mkdtemp(prefix="home-", dir=artifact_dir))
    try:
        user_dir = prepare_user_home(
            sandbox_home, root, replay_bytes, bindings=bindings,
        )
    except ProbeBindingError as error:
        return report("blocked", "settings.probe_binding_missing", str(error))
    artifacts.append({"kind": "replay_stream", "path": str(user_dir / f"{REPLAY_STEM}.rep")})
    artifacts.append({"kind": "probe_user_ini", "path": str(user_dir / "User.ini")})

    if validate_binary:
        game_test._validate_native_arm64(executable)
    command = build_launch_command(executable, root, ticks)
    # Select the staged system INI on both owners; a missing native default
    # would refresh User.ini and discard the scenario's input bindings.
    command.append(f'-INI="{user_dir / "Game.ini"}"')
    log_path = artifact_dir / "replay-run.log"
    displayed = [
        game_test._display_path(executable, REPOSITORY_ROOT),
        *[part.replace(str(REPOSITORY_ROOT) + os.sep, "") for part in command[1:]],
    ]
    started = time.monotonic()
    output, exit_status, timed_out, launch_error = _supervised_run(
        command, home=sandbox_home, cwd=REPOSITORY_ROOT, timeout_seconds=timeout_seconds,
    )
    duration = time.monotonic() - started
    game_test._write_atomic(log_path, output)
    artifacts.append({"kind": "run_log", "path": str(log_path)})
    markers = game_test._failure_markers(output, REPOSITORY_ROOT, sandbox_home)
    text = output.decode("utf-8", errors="replace")
    completed_map_loads = [
        match.group(1).split("?", 1)[0].replace("\\", "/").rsplit("/", 1)[-1]
        for match in re.finditer(r"^Load time (.+): [-+\d.eE]+ seconds total,", text, re.MULTILINE)
    ]
    completed_map_names = {name.casefold() for name in completed_map_loads}
    shots = sorted(user_dir.glob("Shot*.bmp"))
    required_files = {
        name: sorted(path for path in user_dir.rglob(name) if path.is_file())
        for name in expected_user_files
    }
    for paths in required_files.values():
        artifacts.extend({"kind": "expected_user_file", "path": str(path)} for path in paths)

    facts = {
        "duration_seconds": round(duration, 3),
        "ticks_requested": ticks,
        "frames_replayed": len(frames),
        "exit_status": exit_status,
        "timed_out": timed_out,
        "launch_error": launch_error,
        "failure_markers": markers,
        "probe_shots": [shot.name for shot in shots],
        "expected_probe_shots": expected_probe_shots,
        "loop_marker_present": LOOP_MARKER in text,
        "missing_user_files": [name for name, paths in required_files.items() if not paths],
        "completed_map_loads": completed_map_loads,
        "missing_map_loads": [name for name in expected_map_loads if name.casefold() not in completed_map_names],
    }
    def fail(reason_code: str, exit_reason: str) -> dict:
        result = report("fail", reason_code, exit_reason, displayed, artifacts)
        result["data"] = facts
        return result

    if launch_error is not None:
        return fail("input.launch_error", f"could not launch app bundle: {launch_error}")
    if timed_out:
        return fail("budget.timeout", f"exceeded {timeout_seconds:g}s budget")
    if exit_status != 0:
        return fail("input.exit_status", f"exit status {exit_status}")
    if markers:
        return fail("input.failure_marker", f"failure marker {markers[0]['text']!r} in output")
    if LOOP_MARKER not in text:
        return fail("runner.loop_marker_missing", f"missing {LOOP_MARKER!r} log marker")
    if facts["missing_user_files"]:
        return fail(
            "settings.expected_user_file_missing",
            f"expected user files were not written: {facts['missing_user_files']}",
        )
    if facts["missing_map_loads"]:
        return fail(
            "input.map_load_missing",
            f"expected map loads did not complete: {facts['missing_map_loads']}",
        )
    if len(shots) < expected_probe_shots:
        return fail(
            "renderer.screenshot_missing",
            f"expected {expected_probe_shots} fixture-requested probe screenshots through the K={PROBE_BINDING} "
            f"binding, found {[shot.name for shot in shots]}",
        )
    passed = report("pass", "", "", displayed, artifacts)
    passed["data"] = {
        "profile": classify_data_profile(root),
        "duration_seconds": facts["duration_seconds"],
        "ticks_requested": ticks,
        "frames_replayed": len(frames),
        "map_url": f"{selected_map}.unr",
        "completed_map_loads": facts["completed_map_loads"],
    }
    passed["exit_reason"] = (
        f"exit 0 in {duration:.2f}s; {len(frames)} replay frames executed through the "
        f"real keybind pipeline; {len(shots)} probe screenshots written"
    )
    return passed


class InputScriptContracts(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(prefix="hp2-input-script-contract-")
        self.root = Path(self.temporary.name)

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def test_raw_event_first_encoding_matches_the_farchive_wire_format(self) -> None:
        # compile_replay is a raw encoder: it preserves event-first frames for
        # byte-level coverage. Launch scenarios must validate their first frame.
        tick, frames = parse_fixture("tick 0.5\npress K\n")
        self.assertEqual(frames, [
            [(KEY_IDS["K"], IST_PRESS)],
            [(KEY_IDS["K"], IST_RELEASE)],
            [],
        ])
        self.assertEqual(IK_PLAY, 250)
        blob = compile_replay("Entry.unr", tick, frames)
        tick_record = bytes((IK_PLAY, IST_AXIS | DELTA_FLAG)) + struct.pack("<f", 0.5)
        expected = (
            b"\x0aEntry.unr\x00"                    # FString: compact len 10 + bytes + NUL
            + bytes((KEY_IDS["K"], IST_PRESS))      # event: key + action, no delta payload
            + tick_record
            + bytes((KEY_IDS["K"], IST_RELEASE))
            + tick_record
            + tick_record
        )
        self.assertEqual(blob, expected)

    def test_launch_safe_frames_require_a_delimiter_first_stream(self) -> None:
        with self.assertRaisesRegex(FixtureError, "begin with an empty frame"):
            validate_launch_safe_frames([])

        _, event_first = parse_fixture("tick 0.5\npress K\n")
        event_first_blob = compile_replay("Entry.unr", 0.5, event_first)
        self.assertEqual(event_first_blob[11:13], bytes((KEY_IDS["K"], IST_PRESS)))
        with self.assertRaisesRegex(FixtureError, "begin with an empty frame"):
            validate_launch_safe_frames(event_first)

        tick, delimiter_first = parse_fixture("tick 0.5\nidle 1\npress K\n")
        validate_launch_safe_frames(delimiter_first)
        self.assertEqual(
            compile_replay("Entry.unr", tick, delimiter_first)[11:17],
            bytes((IK_PLAY, IST_AXIS | DELTA_FLAG)) + struct.pack("<f", tick),
        )

    def test_absolute_mouse_records_and_click_match_replay_wire_format(self) -> None:
        tick, frames = parse_fixture("tick 0.5\nidle 1\nmouse 80 378\npress LeftMouse\n")
        validate_launch_safe_frames(frames)
        self.assertEqual(frames[1], [(228, 5, 80.0), (229, 5, 378.0)])
        self.assertEqual(frames[2], [(KEY_IDS["LeftMouse"], IST_PRESS)])
        self.assertEqual(frames[3], [(KEY_IDS["LeftMouse"], IST_RELEASE)])
        blob = compile_replay("Entry.unr", tick, frames)
        self.assertIn(bytes((228, 5 | DELTA_FLAG)) + struct.pack("<f", 80.0)
                      + bytes((229, 5 | DELTA_FLAG)) + struct.pack("<f", 378.0), blob)
        for value in ("nan 1", "inf 1", "1000001 1", "1", "bad 2"):
            with self.subTest(value=value), self.assertRaises(FixtureError):
                parse_fixture(f"tick 0.5\nmouse {value}\n")

    def test_parser_rejects_unknown_or_unbalanced_scripts(self) -> None:
        with self.assertRaisesRegex(FixtureError, "unknown directive"):
            parse_fixture("tick 0.1\nwarp 3\n")
        with self.assertRaisesRegex(FixtureError, "unknown input key"):
            parse_fixture("tick 0.1\npress Tilde\n")
        with self.assertRaisesRegex(FixtureError, "no tick directive"):
            parse_fixture("idle 1\n")

    def test_compact_index_matches_the_engine_encoder(self) -> None:
        self.assertEqual(_compact_index(0), b"\x00")
        self.assertEqual(_compact_index(0x3F), b"\x3f")
        self.assertEqual(_compact_index(0x40), b"\x40\x01")
        self.assertEqual(_compact_index(0x3FFF), b"\x7f\xff\x01")
        self.assertEqual(_compact_index(0x400000), b"\x40\x80\x80\x04")


    def test_scenario_bindings_override_duplicates_without_changing_other_sections(self) -> None:
        import configparser
        text = "[Engine.Input]\nK=SHOT\nF5=QuickSave\nf5=OldSave\nW=MoveForward\n[Other]\nF5=preserve\n"
        changed = apply_bindings(text, ("F5=first", "f5=SAVEGAME 9", "F9=LOADGAME 9"))
        parsed = configparser.RawConfigParser(strict=True)
        parsed.read_string(changed)
        self.assertEqual(parsed["Engine.Input"]["F5"], "SAVEGAME 9")
        self.assertEqual(parsed["Engine.Input"]["F9"], "LOADGAME 9")
        self.assertEqual(parsed["Engine.Input"]["W"], "MoveForward")
        self.assertEqual(parsed["Other"]["F5"], "preserve")
        with self.assertRaises(ProbeBindingError):
            apply_bindings(text, ("F5=SAVEGAME 9\nF9=QUIT",))


    def _stub_binary(self, body: str) -> Path:
        stub = self.root / "stub-binary"
        stub.write_text(f"#!/bin/sh\n{body}\n")
        stub.chmod(0o755)
        return stub

    def _smoke_with_stub(self, body: str, **overrides) -> dict:
        data_root = self.root / "Unreal"
        system = data_root / "System"
        system.mkdir(parents=True, exist_ok=True)
        (system / "Default.ini").write_text("[Core.System]\n")
        (system / "DefUser.ini").write_text("[Engine.Input]\nK=\n")
        (system / "Default.ini").write_text("[Core.System]\n")
        fixture = self.root / "script.txt"
        fixture.write_text("tick 0.033333335\nidle 2\npress K\nidle 2\n")
        arguments = {
            "app": self._stub_binary(body),
            "data_root": data_root,
            "artifact_dir": self.root / "artifacts",
            "fixture_path": fixture,
            "timeout_seconds": 30.0,
            "validate_binary": False,
        }
        arguments.update(overrides)
        return run_input_script_smoke(**arguments)


    def test_failure_markers_fail_the_report(self) -> None:
        result = self._smoke_with_stub('echo "Entering main loop."; echo "Critical error: boom"')
        self.assertEqual(result["status"], "fail")
        self.assertEqual(result["reason_code"], "input.failure_marker")

    def test_missing_probe_screenshots_fail_the_report(self) -> None:
        result = self._smoke_with_stub('echo "Entering main loop."')
        self.assertEqual(result["status"], "fail")
        self.assertEqual(result["reason_code"], "renderer.screenshot_missing")
    def test_starting_a_requested_map_load_does_not_count_as_completion(self) -> None:
        result = self._smoke_with_stub(
            'echo "Entering main loop."; echo "LoadMap: /Save/Save9.usa"; '
            'echo "Load time Adv1Willow.unr: 0.5 seconds total, 0.4 app"',
            expected_map_loads=("Save9.usa",),
        )
        self.assertEqual(result["status"], "fail")
        self.assertEqual(result["reason_code"], "input.map_load_missing")
        self.assertEqual(result["data"]["missing_map_loads"], ["Save9.usa"])


    def test_nonzero_exit_fails_the_report(self) -> None:
        result = self._smoke_with_stub('echo "Entering main loop."; exit 7')
        self.assertEqual(result["status"], "fail")
        self.assertEqual(result["reason_code"], "input.exit_status")
        self.assertIn("7", result["exit_reason"])

    def test_timeout_fails_within_budget(self) -> None:
        result = self._smoke_with_stub('echo "Entering main loop."; sleep 30', timeout_seconds=1.0)
        self.assertEqual(result["status"], "fail")
        self.assertEqual(result["reason_code"], "budget.timeout")

    def test_missing_app_is_blocked(self) -> None:
        data_root = self.root / "Unreal"
        system = data_root / "System"
        system.mkdir(parents=True)
        (system / "Default.ini").write_text("[Core.System]\n")
        result = run_input_script_smoke(
            app=self.root / "missing.app", data_root=data_root,
            artifact_dir=self.root / "artifacts",
        )
        self.assertEqual(result["status"], "blocked")
        self.assertEqual(result["reason_code"], "binary_missing")

    def test_missing_data_root_is_blocked(self) -> None:
        result = run_input_script_smoke(
            app=self.root / "whatever.app", data_root=self.root / "nope",
            artifact_dir=self.root / "artifacts",
        )
        self.assertEqual(result["status"], "blocked")
        self.assertEqual(result["reason_code"], "data.root_missing")


def _arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--smoke", action="store_true", help="run the live engine smoke")
    parser.add_argument("--app", type=Path, default=None, help="app bundle (ignored with --engine-bin)")
    parser.add_argument("--engine-bin", type=Path, default=None,
                        help="raw executable override (same semantics as game_test.py run)")
    parser.add_argument("--map", dest="selected_map", default="Entry")
    parser.add_argument("--bind", action="append", default=[], metavar="KEY=COMMAND")
    parser.add_argument("--expect-user-file", action="append", default=[], metavar="NAME")
    parser.add_argument("--expect-map-load", action="append", default=[], metavar="MAP",
                        help="require a completed LoadMap of this map or saved-package filename")
    parser.add_argument("--data-root", type=Path, default=None, help="prototype data root")
    parser.add_argument("--fixture", type=Path, default=DEFAULT_FIXTURE)
    parser.add_argument("--artifact-dir", type=Path, default=None)
    parser.add_argument("--ticks", type=int, default=DEFAULT_TICKS)
    parser.add_argument("--timeout", type=float, default=DEFAULT_TIMEOUT_SECONDS)
    return parser.parse_args()


def main() -> int:
    arguments = _arguments()
    if not arguments.smoke:
        program = unittest.main(argv=[sys.argv[0]], verbosity=2, exit=False)
        return 0 if program.result.wasSuccessful() else 1
    artifact_dir = arguments.artifact_dir
    if artifact_dir is None:
        from_env = os.environ.get("HP2_ARTIFACT_DIR")
        artifact_dir = Path(from_env) if from_env else REPOSITORY_ROOT / "out" / "input-script-smoke"
    result = run_input_script_smoke(
        app=arguments.app,
        data_root=arguments.data_root,
        artifact_dir=artifact_dir,
        fixture_path=arguments.fixture,
        ticks=arguments.ticks,
        timeout_seconds=arguments.timeout,
        engine_bin=arguments.engine_bin,
        selected_map=arguments.selected_map,
        bindings=tuple(arguments.bind),
        expected_user_files=tuple(arguments.expect_user_file),
        expected_map_loads=tuple(arguments.expect_map_load),
    )
    (artifact_dir / "report.json").write_text(
        json.dumps(result, indent=2, sort_keys=True) + "\n", encoding="utf-8",
    )
    print(json.dumps(result, indent=2, sort_keys=True))
    return {"pass": 0, "fail": 1, "blocked": BLOCKED_EXIT_CODE}[result["status"]]


if __name__ == "__main__":
    raise SystemExit(main())
