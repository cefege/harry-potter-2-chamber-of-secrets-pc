#!/usr/bin/env python3
"""Scripted-input smoke: deterministic keybind execution driven by the engine.

Two modes:

1. Contract tests (default): deterministic unittest suite over the replay
   compiler, fixture parser, and smoke runner plumbing using stub binaries.
   No engine build required; safe for ctest.
2. Live smoke (--smoke): compiles Tests/Fixtures/input_script_smoke.txt into
   an FReplay ``.rep`` input stream, seeds an isolated user directory with it
   plus a ``K=SHOT`` probe binding, launches the installed app bundle with
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
import os
import re
import struct
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path

REPOSITORY_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(REPOSITORY_ROOT / "Build"))
import game_test  # noqa: E402

REPORT_SCHEMA_VERSION = 1
SMOKE_NAME = "input_script_smoke"
SMOKE_INVARIANT = (
    "the engine replays a deterministic input script through the real "
    "keybind pipeline on the Entry map, executes gameplay bindings "
    "(movement, rotation, jump, fire, spell switch), writes probe "
    "screenshots, and exits cleanly within the tick budget"
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
EXPECTED_PROBE_SHOTS = 2
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
IK_PLAY = 0
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


def parse_fixture(text: str) -> tuple[float, list[list[tuple[int, int]]]]:
    """Parse the fixture DSL into (tick_delta, frames of (key, state) events).

    Raises FixtureError on unknown directives, unknown keys, misplaced
    ``tick``, or unbalanced down/up sequences.
    """
    tick: float | None = None
    frames: list[list[tuple[int, int]]] = []
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


def compile_replay(url: str, tick_delta: float, frames: list[list[tuple[int, int]]]) -> bytes:
    """Compile parsed frames into the exact FReplay .rep byte stream.

    Stream layout (UnReplay.cpp): serialized starting URL FString, then per
    frame zero or more FInputEvent records followed by the frame-tick record
    {IK_Play, IST_Axis, delta}. The engine reads the first record at open
    time to seed NextTick, so the stream opens with a tick terminator.
    """
    out = bytearray()
    out += serialize_fstring(url)
    for events in frames:
        for key, state in events:
            out += _event_bytes(key, state)
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


def prepare_user_home(sandbox_home: Path, data_root: Path, replay_bytes: bytes) -> Path:
    """Seed the isolated user directory with User.ini and the replay stream.

    Both ini seeds are required: when bootstrap finds Game.ini missing it
    recreates it from Default.ini and flags ``bDefaultIniChanged``, which
    forces User.ini to be regenerated from DefUser.ini — wiping any probe
    binding seeded alone. Pre-seeding Game.ini from Default.ini keeps both
    mtimes newer than their prototypes, so neither regeneration fires.
    """
    user_dir = sandbox_home / _USER_DIR_RELATIVE
    user_dir.mkdir(parents=True, exist_ok=True)
    system = data_root / "System"
    (user_dir / "Game.ini").write_text(
        (system / "Default.ini").read_text(encoding="utf-8"), encoding="utf-8",
    )
    defuser = (system / "DefUser.ini").read_text(encoding="utf-8")
    (user_dir / "User.ini").write_text(inject_probe_binding(defuser), encoding="utf-8")
    (user_dir / f"{REPLAY_STEM}.rep").write_bytes(replay_bytes)
    return user_dir


def build_launch_command(executable: Path, data_root: Path, ticks: int) -> list[str]:
    return [
        str(executable),
        f"-datadir={data_root}",
        f"-REPLAY={REPLAY_STEM}",
        "-xopengl",
        "-NOFRONTEND",
        "-window",
        "-nosound",
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
        process = subprocess.Popen(
            command, cwd=str(cwd), env=environment,
            stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
            start_new_session=True,
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
    bundle = discover_app(app)
    if bundle is None:
        return report("blocked", "binary_missing",
                      "app bundle not found; build and install dist/macos-arm64 first")

    artifacts: list[dict] = []
    artifact_dir.mkdir(parents=True, exist_ok=True)

    tick_delta, frames = parse_fixture(fixture_path.read_text(encoding="utf-8"))
    replay_bytes = compile_replay("Entry.unr", tick_delta, frames)

    sandbox_home = artifact_dir / "home"
    try:
        user_dir = prepare_user_home(sandbox_home, root, replay_bytes)
    except ProbeBindingError as error:
        return report("blocked", "settings.probe_binding_missing", str(error))
    artifacts.append({"kind": "replay_stream", "path": str(user_dir / f"{REPLAY_STEM}.rep")})
    artifacts.append({"kind": "probe_user_ini", "path": str(user_dir / "User.ini")})

    executable = game_test._bundle_executable(bundle)
    if validate_binary:
        game_test._validate_native_arm64(executable)
    command = build_launch_command(executable, root, ticks)

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
    shots = sorted(user_dir.glob("Shot*.bmp"))

    facts = {
        "duration_seconds": round(duration, 3),
        "ticks_requested": ticks,
        "frames_replayed": len(frames),
        "exit_status": exit_status,
        "timed_out": timed_out,
        "launch_error": launch_error,
        "failure_markers": markers,
        "probe_shots": [shot.name for shot in shots],
        "loop_marker_present": LOOP_MARKER in text,
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
    if len(shots) < EXPECTED_PROBE_SHOTS:
        return fail(
            "renderer.screenshot_missing",
            f"expected {EXPECTED_PROBE_SHOTS} probe screenshots through the K={PROBE_BINDING} "
            f"binding, found {[shot.name for shot in shots]}",
        )

    passed = report("pass", "", "", displayed, artifacts)
    passed["data"] = {
        "profile": classify_data_profile(root),
        "duration_seconds": facts["duration_seconds"],
        "ticks_requested": ticks,
        "frames_replayed": len(frames),
        "tick_delta_seconds": tick_delta,
        "probe_shots": facts["probe_shots"],
        "map_url": "Entry.unr",
    }
    passed["exit_reason"] = (
        f"exit 0 in {duration:.2f}s; {len(frames)} replay frames drove movement, rotation, "
        f"jump, fire, and spell-switch bindings; {len(shots)} probe screenshots written"
    )
    return passed


class InputScriptContracts(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(prefix="hp2-input-script-contract-")
        self.root = Path(self.temporary.name)

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def test_fixture_parses_to_the_documented_script(self) -> None:
        tick, frames = parse_fixture(DEFAULT_FIXTURE.read_text(encoding="utf-8"))
        self.assertAlmostEqual(tick, 0.033333335)
        # idle 8 | probe K | hold W 40 | hold S 20 | hold L/R 15 | Ctrl |
        # RightMouse | 1 2 3 | LeftMouse | probe K | idle 8
        self.assertEqual(len(frames), 127)
        self.assertEqual([e for frame in frames[:8] for e in frame], [])
        self.assertEqual(frames[8], [(KEY_IDS[PROBE_KEY], IST_PRESS)])
        self.assertEqual(frames[9], [(KEY_IDS[PROBE_KEY], IST_RELEASE)])
        self.assertEqual(frames[10], [(KEY_IDS["W"], IST_PRESS)])
        self.assertEqual(frames[11], [(KEY_IDS["W"], IST_HOLD)])
        self.assertEqual(frames[49], [(KEY_IDS["W"], IST_HOLD)])
        self.assertEqual(frames[50], [(KEY_IDS["W"], IST_RELEASE)])
        self.assertEqual(frames[51], [(KEY_IDS["S"], IST_PRESS)])
        self.assertEqual(frames[116], [(KEY_IDS[PROBE_KEY], IST_PRESS)])
        self.assertEqual(frames[117], [(KEY_IDS[PROBE_KEY], IST_RELEASE)])
        held_states = {state for frame in frames for _, state in frame}
        self.assertEqual(held_states, {IST_PRESS, IST_HOLD, IST_RELEASE})

    def test_compile_replay_matches_the_farchive_wire_format(self) -> None:
        tick, frames = parse_fixture("tick 0.5\npress K\n")
        # parse_fixture leaves a trailing empty frame after the release; the
        # engine reads the FIRST record at open time to seed its tick, so the
        # stream opens with the first frame's tick terminator.
        self.assertEqual(frames, [
            [(KEY_IDS["K"], IST_PRESS)],
            [(KEY_IDS["K"], IST_RELEASE)],
            [],
        ])
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

    def test_compilation_is_deterministic(self) -> None:
        text = DEFAULT_FIXTURE.read_text(encoding="utf-8")
        tick, frames = parse_fixture(text)
        self.assertEqual(compile_replay("Entry.unr", tick, frames),
                         compile_replay("Entry.unr", tick, parse_fixture(text)[1]))

    def test_parser_rejects_unknown_or_unbalanced_scripts(self) -> None:
        with self.assertRaisesRegex(FixtureError, "unknown directive"):
            parse_fixture("tick 0.1\nwarp 3\n")
        with self.assertRaisesRegex(FixtureError, "unknown input key"):
            parse_fixture("tick 0.1\npress Tilde\n")
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

    def test_probe_binding_replaces_only_the_empty_row(self) -> None:
        injected = inject_probe_binding("[Engine.Input]\nZ=Button bBroomBrake\nK=\nW=MoveForward\n")
        self.assertIn("K=SHOT\n", injected)
        self.assertIn("Z=Button bBroomBrake\n", injected)
        self.assertIn("W=MoveForward\n", injected)
        with self.assertRaisesRegex(ProbeBindingError, "no empty K= row"):
            inject_probe_binding("[Engine.Input]\nK=quit\n")

    def test_user_home_seeding_writes_ini_and_replay(self) -> None:
        data_root = self.root / "Unreal"
        (data_root / "System").mkdir(parents=True)
        (data_root / "System" / "Default.ini").write_text("[Core.System]\nPaths=../Maps/*.unr\n")
        (data_root / "System" / "DefUser.ini").write_text("[Engine.Input]\nK=\n")
        user_dir = prepare_user_home(self.root / "home", data_root, b"\x00\x84")
        self.assertEqual((user_dir / "User.ini").read_text(), "[Engine.Input]\nK=SHOT\n")
        # Game.ini must be seeded so bootstrap never flags bDefaultIniChanged
        # and regenerates User.ini from DefUser.ini behind our back.
        self.assertIn("Paths=../Maps/*.unr", (user_dir / "Game.ini").read_text())
        self.assertEqual((user_dir / f"{REPLAY_STEM}.rep").read_bytes(), b"\x00\x84")

    def test_launch_command_shape(self) -> None:
        command = build_launch_command(Path("/app/Contents/MacOS/HarryPotter2"), Path("/data"), 240)
        self.assertEqual(command[:2], ["/app/Contents/MacOS/HarryPotter2", "-datadir=/data"])
        self.assertIn("-REPLAY=InputScriptSmoke", command)
        self.assertIn("-NOFRONTEND", command)
        self.assertIn("-window", command)
        self.assertIn("-nosound", command)
        self.assertIn("-testticks=240", command)

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

    def test_stubbed_run_passes_end_to_end(self) -> None:
        body = (
            'echo "Entering main loop."\n'
            'user="$HOME/Library/Application Support/Harry Potter 2/User"\n'
            'mkdir -p "$user"\n'
            'touch "$user/Shot0000.bmp" "$user/Shot0001.bmp"\n'
            'test -s "$user/InputScriptSmoke.rep" || { echo "critical error: no replay"; exit 1; }\n'
            'grep -q "^K=SHOT$" "$user/User.ini" || { echo "critical error: no probe"; exit 1; }\n'
        )
        result = self._smoke_with_stub(body)
        self.assertEqual(result["status"], "pass")
        self.assertEqual(result["schema"], 1)
        self.assertIn("exit 0", result["exit_reason"])
        self.assertEqual(result["data"]["probe_shots"], ["Shot0000.bmp", "Shot0001.bmp"])

    def test_failure_markers_fail_the_report(self) -> None:
        result = self._smoke_with_stub('echo "Entering main loop."; echo "Critical error: boom"')
        self.assertEqual(result["status"], "fail")
        self.assertEqual(result["reason_code"], "input.failure_marker")

    def test_missing_probe_screenshots_fail_the_report(self) -> None:
        result = self._smoke_with_stub('echo "Entering main loop."')
        self.assertEqual(result["status"], "fail")
        self.assertEqual(result["reason_code"], "renderer.screenshot_missing")

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
    parser.add_argument("--app", type=Path, default=None, help="app bundle override")
    parser.add_argument("--data-root", type=Path, default=None, help="prototype data root")
    parser.add_argument("--fixture", type=Path, default=DEFAULT_FIXTURE)
    parser.add_argument("--artifact-dir", type=Path, default=None)
    parser.add_argument("--ticks", type=int, default=DEFAULT_TICKS)
    parser.add_argument("--timeout", type=float, default=DEFAULT_TIMEOUT_SECONDS)
    return parser.parse_args()


def main() -> int:
    arguments = _arguments()
    if not arguments.smoke:
        unittest.main(argv=[sys.argv[0]], verbosity=2, exit=False)
        return 0
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
    )
    (artifact_dir / "report.json").write_text(
        json.dumps(result, indent=2, sort_keys=True) + "\n", encoding="utf-8",
    )
    print(json.dumps(result, indent=2, sort_keys=True))
    return {"pass": 0, "fail": 1, "blocked": BLOCKED_EXIT_CODE}[result["status"]]


if __name__ == "__main__":
    raise SystemExit(main())
