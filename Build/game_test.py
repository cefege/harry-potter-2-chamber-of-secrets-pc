"""List and launch HP2 maps through the direct engine bootstrap for tests.

Launches are supervised end to end and scanned for strict failure markers.
The engine MAY additionally emit optional ``<HP2_RES> key=value`` log lines
(for example ``<HP2_RES> gl_textures_created=128``); these are parsed
leniently into the ``resources`` result field and are never fatal by
themselves. Absent markers leave consumers to report nulls.
"""

from __future__ import annotations

import argparse
from concurrent.futures import ThreadPoolExecutor
from contextlib import nullcontext
from dataclasses import dataclass
import hashlib
import importlib.util
import json
import math
import os
from pathlib import Path
import plistlib
import re
import shutil
import signal
import stat
import struct
import subprocess
import sys
import tempfile
import threading
import time
from typing import Any, BinaryIO, Callable, Iterator, Sequence

import actor_transition_ledger
import creature_generator_trace
import fire_texture_trace
import global_tick_trace
import shadow_admission_trace
import shadow_update_trace
import startup_rng_phase_trace

ARM64_CPU_TYPE = 0x0100000C
TERMINATION_GRACE_SECONDS = 5.0
KILL_GRACE_SECONDS = 5.0

FIDELITY_FORMAT_VERSION = 1
FIDELITY_FIELDS = (
    ("camera", "render"), ("actors", "sim"), ("cutscene", "vm"),
    ("cues", "vm"), ("animation", "render"), ("audio", "audio"),
    ("frame_hash", "render"),
)
RNG_SEED_MAX = 2_147_483_647
RNG_TRACE_FORMAT_VERSION = 2
RNG_TRACE_DEFAULT_LIMIT = 16_384
RNG_TRACE_MAX_LIMIT = 262_144
SHADOW_UPDATE_TRACE_FORMAT_VERSION = 1
SHADOW_ADMISSION_TRACE_FORMAT_VERSION = 1
ACTOR_TRANSITION_LEDGER_FORMAT_VERSION = 2
CREATURE_GENERATOR_TRACE_FORMAT_VERSION = 1
GLOBAL_TICK_TRACE_FORMAT_VERSION = 2
STARTUP_RNG_PHASE_TRACE_FORMAT_VERSION = 2
FIRE_TEXTURE_TRACE_FORMAT_VERSION = 1
ACTOR_SLOT_DUMP_FORMAT_VERSION = 1
_ACTOR_SLOT_DUMP_CHECKPOINTS = (
    "post_deserialize_raw", "post_startup_before_first_tick", "post_startup",
)
_ACTOR_SLOT_RAW_ID_FIELDS = frozenset(("slot", "path"))
_ACTOR_SLOT_NON_SEMANTIC_FIELDS = frozenset((
    "raw_reference", "spawn_origin", "spawn_owner_path", "spawn_owner_class",
    "spawn_request_id", "spawn_order",
))
_CUTSCRIPT_COMMANDS = frozenset((
    "talk", "playanim", "sleep", "triggerchangelevel", "animate", "waitfor", "release",
))
_REPLAY_USER_DIRECTORY = (
    Path("Library/Application Support/Harry Potter 2/User")
    if sys.platform == "darwin"
    else Path(".local/share/harry-potter-2/User")
)
REPLAY_STEM_PATTERN = re.compile(r"[A-Za-z0-9_-]+")
RUST_REPLAY_MARKER_PREFIX = "hp2rs: [replay.wire_v1]"
RUST_REPLAY_MARKER = re.compile(
    r"^hp2rs: \[replay\.wire_v1\] url=(?P<url>\S+) frames=(?P<frames>\d+) "
    r"first_tick=(?P<first_tick>[+-]?(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][+-]?\d+)?) "
    r"seed=(?P<seed>\d+)\s*$", re.MULTILINE,
)

# ScriptWarning is deliberately fatal: UE1 reports Accessed None through it.
FAILURE_MARKER_PATTERNS: dict[str, tuple[str, ...]] = {
    "script": (
        r"\bscript(?:error|exception|fatal)\b",
        r"\bscript(?:ing)?\s+(?:error|exception|failure|failed)\b",
        r"\bscriptwarning\s*:",
        r"\baccessed\s+none\b",
    ),
    "native": (
        r"\bnative(?:error|exception|fatal)\b",
        r"\bnative\s+(?:error|exception|failure|failed)\b",
        r"\b(?:segmentation fault|bus error|illegal instruction|abort trap)\b",
        r"\buncaught\s+(?:c\+\+\s+)?exception\b",
        r"\bterminate called\b",
    ),
    "package": (
        r"\bpackage(?:error|exception|fatal)\b",
        r"\bpackage\s+(?:error|exception|failure|failed)\b",
        r"\b(?:failed|unable) to (?:load|find) (?:file for )?package\b",
        r"\bcan(?:not|'t) (?:find|resolve) (?:file for )?package\b",
        r"\bpackage\b.*\b(?:not found|is corrupt|version mismatch)\b",
    ),
    "render": (
        r"\b(?:render|renderer|rendering)(?:error|exception|fatal)\b",
        r"\b(?:render|renderer|rendering)\s+(?:error|exception|failure|failed)\b",
        r"\b(?:xopengl|opengl|vulkan|gl)\s+(?:error|exception|failure|failed)\b",
        r"\bfailed to (?:initialize|initialise|create)\b.*\b(?:renderer|rendering|opengl|vulkan|context)\b",
        r"\bvulkan validation (?:error|failure)\b",
        r"\bgl_invalid_(?:enum|value|operation|framebuffer_operation)\b",
    ),
    "assert": (
        r"\bassert(?:ion)?\s*:",
        r"\bassert(?:ion)?\s+(?:failed|failure)\b",
        r"\bfailed assertion\b",
        r"\bcheck\s+failed\b",
    ),
    "critical": (
        r"\bcritical error\b",
        r"\bcritical\s*:",
        r"\bfatal error\b",
        r"\bappErrorf?\b",
    ),
}

# Optional engine-side resource/timing marker protocol. The engine emits lines
# like "<HP2_RES> gl_textures_created=128"; parsing is lenient (unknown keys
# are kept, non-numeric values stay raw strings) because the protocol is a
# forward contract the engine does not emit everywhere yet.
HP2_RESOURCE_MARKER = re.compile(
    r"<HP2_RES>\s+([A-Za-z_][A-Za-z0-9_]*)\s*=\s*([^\s]+)"
)
RESOURCE_KEYS = (
    "gl_textures_created", "gl_textures_destroyed", "script_deferred",
    "script_deferral_reasons", "script_deferral_subjects",
)


def _deferral_census(raw_value: str) -> object:
    if raw_value == "none":
        return {}
    from urllib.parse import unquote
    census: dict[str, int] = {}
    for item in raw_value.split(","):
        count_text, separator, encoded = item.partition(":")
        if not separator or not count_text.isdecimal() or not encoded:
            return raw_value
        census[unquote(encoded)] = int(count_text)
    return census


def _resource_markers(output: bytes) -> dict[str, object]:
    fields: dict[str, object] = {}
    text = output.decode("utf-8", errors="replace")
    for match in HP2_RESOURCE_MARKER.finditer(text):
        key, raw_value = match.group(1), match.group(2)
        if key in ("script_deferral_reasons", "script_deferral_subjects"):
            fields[key] = _deferral_census(raw_value)
            continue
        try:
            value: object = int(raw_value, 10)
        except ValueError:
            try:
                value = float(raw_value)
            except ValueError:
                value = raw_value
        fields[key] = value

    return fields
def _script_deferral_violation(resources: dict[str, object]) -> str | None:
    """Require complete zero-deferral telemetry for every launched map."""
    deferred = resources.get("script_deferred")
    if deferred is None:
        return "script_deferred marker missing"
    if isinstance(deferred, bool) or not isinstance(deferred, int):
        return f"script_deferred marker is not an integer: {deferred!r}"
    if deferred != 0:
        return f"script_deferred={deferred}"
    for key in ("script_deferral_reasons", "script_deferral_subjects"):
        census = resources.get(key)
        if census is None:
            return f"{key} marker missing"
        if not isinstance(census, dict):
            return f"{key} marker is malformed"
        if census:
            return f"{key} is non-empty while script_deferred=0"
    return None

COMPILED_FAILURE_MARKERS = {
    category: tuple(re.compile(pattern, re.IGNORECASE) for pattern in patterns)
    for category, patterns in FAILURE_MARKER_PATTERNS.items()
}


class GameTestError(Exception):
    """Raised when a map launch cannot be safely performed."""


def _repo_root() -> Path:
    try:
        return Path(__file__).resolve(strict=True).parent.parent
    except OSError as error:
        raise GameTestError(f"cannot resolve the repository root: {error}") from error


def _resolve_path(value: Path, repo_root: Path, *, strict: bool) -> Path:
    expanded = value.expanduser()
    candidate = expanded if expanded.is_absolute() else repo_root / expanded
    try:
        return candidate.resolve(strict=strict)
    except OSError as error:
        raise GameTestError(f"cannot resolve path {value}: {error}") from error


def _display_path(path: Path, repo_root: Path) -> str:
    try:
        return path.relative_to(repo_root).as_posix()
    except ValueError:
        return str(path)


def _bundle_executable(app: Path) -> Path:
    if app.is_file():
        executable = app
    elif app.is_dir():
        info_path = app / "Contents" / "Info.plist"
        try:
            with info_path.open("rb") as stream:
                info = plistlib.load(stream)
        except (OSError, plistlib.InvalidFileException) as error:
            raise GameTestError(f"cannot read app metadata {info_path}: {error}") from error
        executable_name = info.get("CFBundleExecutable")
        if (
            not isinstance(executable_name, str)
            or not executable_name
            or Path(executable_name).name != executable_name
        ):
            raise GameTestError(
                f"invalid CFBundleExecutable in {info_path}: {executable_name!r}"
            )
        executable = app / "Contents" / "MacOS" / executable_name
    else:
        raise GameTestError(f"app path is neither an app bundle nor an executable: {app}")

    try:
        executable = executable.resolve(strict=True)
        mode = executable.stat().st_mode
    except OSError as error:
        raise GameTestError(f"cannot inspect app executable {executable}: {error}") from error
    if not stat.S_ISREG(mode):
        raise GameTestError(f"app executable is not a regular file: {executable}")
    if not os.access(executable, os.X_OK):
        raise GameTestError(f"app executable is not executable: {executable}")
    return executable


def _thin_header_cpu(stream: BinaryIO, offset: int, size: int) -> int:
    if size < 8:
        raise GameTestError("Mach-O slice is too small to contain a header")
    stream.seek(offset)
    header = stream.read(8)
    if len(header) != 8:
        raise GameTestError("cannot read complete Mach-O slice header")
    magic = header[:4]
    if magic == b"\xcf\xfa\xed\xfe":
        endian = "<"
    elif magic == b"\xfe\xed\xfa\xcf":
        endian = ">"
    elif magic in (b"\xce\xfa\xed\xfe", b"\xfe\xed\xfa\xce"):
        raise GameTestError("app executable contains a 32-bit Mach-O slice")
    else:
        raise GameTestError(f"unrecognized Mach-O slice magic {magic.hex()}")
    return struct.unpack(f"{endian}I", header[4:8])[0]


def _validate_native_arm64(executable: Path) -> None:
    try:
        file_size = executable.stat().st_size
        with executable.open("rb") as stream:
            header = stream.read(8)
            if len(header) != 8:
                raise GameTestError(f"app executable is too small to be Mach-O: {executable}")
            magic = header[:4]
            if magic == b"\x7fELF":
                # ELF header: ei_class at byte 4 (2 = ELFCLASS64), ei_data at
                # byte 5 (1 = little-endian), e_machine at bytes 18-19
                # (0xb7 = EM_AARCH64).
                stream.seek(0)
                elf_header = stream.read(20)
                if len(elf_header) < 20:
                    raise GameTestError(f"app executable is too small to be ELF: {executable}")
                ei_class = elf_header[4]
                ei_data = elf_header[5]
                if ei_class != 2:
                    raise GameTestError(f"app executable is not a 64-bit ELF binary: {executable}")
                if ei_data != 1:
                    raise GameTestError(f"app executable is not a little-endian ELF binary: {executable}")
                e_machine = struct.unpack("<H", elf_header[18:20])[0]
                EM_AARCH64 = 0xB7
                if e_machine != EM_AARCH64:
                    raise GameTestError(
                        f"app executable is not arm64 (ELF e_machine 0x{e_machine:04x}): {executable}"
                    )
                return
            if magic in (b"\xcf\xfa\xed\xfe", b"\xfe\xed\xfa\xcf"):
                cpu_type = _thin_header_cpu(stream, 0, file_size)
                if cpu_type != ARM64_CPU_TYPE:
                    raise GameTestError(
                        f"app executable is not arm64 (Mach-O CPU type 0x{cpu_type:08x}): {executable}"
                    )
                return
            fat_formats = {
                b"\xca\xfe\xba\xbe": (">", False),
                b"\xbe\xba\xfe\xca": ("<", False),
                b"\xca\xfe\xba\xbf": (">", True),
                b"\xbf\xba\xfe\xca": ("<", True),
            }
            fat_format = fat_formats.get(magic)
            if fat_format is None:
                raise GameTestError(f"app executable is not a recognized Mach-O binary: {executable}")
            endian, is_64_bit_fat = fat_format
            architecture_count = struct.unpack(f"{endian}I", header[4:8])[0]
            entry_size = 32 if is_64_bit_fat else 20
            if architecture_count == 0:
                raise GameTestError("fat Mach-O app executable contains no architectures")
            if architecture_count > (file_size - 8) // entry_size:
                raise GameTestError("fat Mach-O architecture table extends past end of file")
            for _ in range(architecture_count):
                entry = stream.read(entry_size)
                if len(entry) != entry_size:
                    raise GameTestError("cannot read complete fat Mach-O architecture table")
                if is_64_bit_fat:
                    cpu_type, _subtype, offset, size, _align, _reserved = struct.unpack(
                        f"{endian}IIQQII", entry
                    )
                else:
                    cpu_type, _subtype, offset, size, _align = struct.unpack(
                        f"{endian}IIIII", entry
                    )
                if cpu_type != ARM64_CPU_TYPE:
                    raise GameTestError(
                        "app executable contains a non-arm64 Mach-O slice "
                        f"(CPU type 0x{cpu_type:08x})"
                    )
                if offset > file_size or size > file_size - offset:
                    raise GameTestError("fat Mach-O slice extends past end of file")
                if _thin_header_cpu(stream, offset, size) != ARM64_CPU_TYPE:
                    raise GameTestError("fat Mach-O table and slice CPU types disagree")
    except GameTestError:
        raise
    except (OSError, struct.error) as error:
        raise GameTestError(f"cannot parse app executable {executable}: {error}") from error


def _map_files(directory: Path) -> Iterator[Path]:
    try:
        entries = sorted(os.scandir(directory), key=lambda entry: os.fsencode(entry.name))
    except OSError as error:
        raise GameTestError(f"cannot enumerate data directory {directory}: {error}") from error
    for entry in entries:
        path = Path(entry.path)
        try:
            if entry.is_dir(follow_symlinks=False):
                yield from _map_files(path)
            elif entry.is_file(follow_symlinks=False) and entry.name[-4:].lower() == ".unr":
                yield path
            elif entry.is_symlink() and entry.name[-4:].lower() == ".unr":
                raise GameTestError(f"refusing symlinked map: {path}")
        except OSError as error:
            raise GameTestError(f"cannot inspect data path {path}: {error}") from error


def _enumerate_maps(data_root: Path) -> list[tuple[Path, str]]:
    maps: list[tuple[Path, str]] = []
    for path in _map_files(data_root):
        try:
            relative = path.relative_to(data_root).as_posix()
        except ValueError as error:
            raise GameTestError(f"map resolves outside data root: {path}") from error
        maps.append((path, relative))
    maps.sort(key=lambda item: os.fsencode(item[1]))
    if not maps:
        raise GameTestError(f"no .unr maps found under data root {data_root}")
    return maps


def _map_token(path: Path, data_root: Path) -> str:
    system_directory = data_root / "System"
    relative = os.path.relpath(path, system_directory)
    return relative.replace(os.sep, "\\")


def _validate_data_root(data_root: Path) -> None:
    if not data_root.is_dir():
        raise GameTestError(f"data root is not a directory: {data_root}")
    default_ini = data_root / "System" / "Default.ini"
    if not default_ini.is_file():
        raise GameTestError(f"data root does not contain System/Default.ini: {data_root}")


def list_maps(data_root: Path) -> list[dict[str, str]]:
    _validate_data_root(data_root)
    return [
        {"map": relative, "map_token": _map_token(path, data_root)}
        for path, relative in _enumerate_maps(data_root)
    ]


def _resolve_map(data_root: Path, selected_map: str) -> tuple[Path, str]:
    if not isinstance(selected_map, str):
        raise GameTestError("map selector must be a string")
    for path, relative in _enumerate_maps(data_root):
        if selected_map == relative:
            return path, _map_token(path, data_root)
    raise GameTestError(f"map is not an exact listed map: {selected_map}")


def _positive_int(value: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value <= 0:
        raise GameTestError("ticks must be a positive integer")
    return value


def _positive_float(value: float) -> float:
    if isinstance(value, bool) or not isinstance(value, (float, int)) or not math.isfinite(value) or value <= 0.0:
        raise GameTestError("timeout must be a positive finite number")
    return float(value)


def _rng_seed_value(value: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not 0 <= value <= RNG_SEED_MAX:
        raise GameTestError(f"RNG seed must be an integer from 0 through {RNG_SEED_MAX}")
    return value


def _rng_trace_limit_value(value: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not 1 <= value <= RNG_TRACE_MAX_LIMIT:
        raise GameTestError(f"RNG trace limit must be an integer from 1 through {RNG_TRACE_MAX_LIMIT}")
    return value


def _effective_rng_seed(value: int) -> int:
    value = _rng_seed_value(value)
    return value if 0 < value < RNG_SEED_MAX else 1


def build_launch_command(
    executable: Path, data_root: Path, map_token: str | None, renderer: str,
    ticks: int, *, no_sound: bool = False, ini_file: Path | None = None,
    fixed_dt: float | None = None, input_script: Path | None = None,
    replay_stem: str | None = None, rng_seed: int | None = None,
) -> list[str]:
    if renderer not in ("xopengl", "vulkan"):
        raise GameTestError(f"unsupported renderer: {renderer}")
    _positive_int(ticks)
    if fixed_dt is not None:
        _positive_float(fixed_dt)
    if (map_token is None) == (replay_stem is None):
        raise GameTestError("launch requires exactly one map token or replay stem")
    source = map_token if replay_stem is None else f"-REPLAY={replay_stem}"
    assert source is not None
    arguments = [
        str(executable), f"-datadir={data_root}", source,
        "-xopengl" if renderer == "xopengl" else "-vulkan",
        "-NOFRONTEND", "-window",
    ]
    if no_sound:
        arguments.append("-nosound")
    arguments.extend((f"-testticks={ticks}", "-log"))
    if fixed_dt is not None:
        arguments.append(f"--fixed-dt={fixed_dt:.9g}")
    if input_script is not None:
        arguments.append(f"--input-script={input_script}")
    if ini_file is not None:
        arguments.append(f"-INI={ini_file}")
    if rng_seed is not None:
        arguments.append(f"--rng-seed={_rng_seed_value(rng_seed)}")
    return arguments


def _write_atomic(path: Path, contents: bytes) -> None:
    temporary: Path | None = None
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
        with tempfile.NamedTemporaryFile(mode="wb", prefix=f".{path.name}.", dir=path.parent, delete=False) as stream:
            temporary = Path(stream.name)
            stream.write(contents)
            stream.flush()
            os.fsync(stream.fileno())
        os.chmod(temporary, 0o644)
        os.replace(temporary, path)
    except OSError as error:
        raise GameTestError(f"cannot write {path}: {error}") from error
    finally:
        if temporary is not None:
            try:
                temporary.unlink(missing_ok=True)
            except OSError:
                pass


def _group_exists(process_group: int) -> bool:
    try:
        os.killpg(process_group, 0)
        return True
    except ProcessLookupError:
        return False
    except PermissionError:
        return True


def _signal_group(process_group: int, requested_signal: signal.Signals) -> str | None:
    try:
        os.killpg(process_group, requested_signal)
        return None
    except ProcessLookupError:
        return None
    except OSError as error:
        return f"cannot send {requested_signal.name} to process group: {error}"


def _wait_for_group_exit(process_group: int, seconds: float) -> bool:
    deadline = time.monotonic() + seconds
    while _group_exists(process_group):
        if time.monotonic() >= deadline:
            return False
        time.sleep(0.05)
    return True


def _stop_remaining_group(process_group: int) -> tuple[str, str | None]:
    if not _group_exists(process_group):
        return "none", None
    error = _signal_group(process_group, signal.SIGTERM)
    if error is None and _wait_for_group_exit(process_group, TERMINATION_GRACE_SECONDS):
        return "sigterm", None
    kill_error = _signal_group(process_group, signal.SIGKILL)
    errors = [message for message in (error, kill_error) if message]
    if _wait_for_group_exit(process_group, KILL_GRACE_SECONDS):
        return "sigkill", "; ".join(errors) or None
    errors.append("process group still exists after SIGKILL")
    return "failed", "; ".join(errors)


def _report_line(text: str, repo_root: Path, isolated_home: Path) -> str:
    for source, replacement in ((str(isolated_home), "<isolated-home>"), (str(repo_root) + os.sep, "")):
        text = text.replace(source, replacement)
    return text


def _failure_markers(output: bytes, repo_root: Path, isolated_home: Path) -> list[dict[str, object]]:
    markers: list[dict[str, object]] = []
    for line_number, line in enumerate(output.decode("utf-8", errors="replace").splitlines(), start=1):
        for category, patterns in COMPILED_FAILURE_MARKERS.items():
            matching_pattern = next((pattern for pattern in patterns if pattern.search(line)), None)
            if matching_pattern is not None:
                reported = _report_line(line.strip(), repo_root, isolated_home)
                markers.append({
                    "category": category,
                    "line": line_number,
                    "pattern": matching_pattern.pattern,
                    "text": reported[:1000],
                    "truncated": len(reported) > 1000,
                })
    return markers


def _isolated_environment(home: Path) -> dict[str, str]:
    temporary = home / "tmp"
    config = home / ".config"
    cache = home / ".cache"
    data = home / ".local/share"
    for directory in (temporary, config, cache, data):
        directory.mkdir(parents=True, exist_ok=True)
    environment = os.environ.copy()
    environment.update({
        "HOME": str(home), "CFFIXED_USER_HOME": str(home),
        "TMPDIR": str(temporary) + os.sep, "XDG_CONFIG_HOME": str(config),
        "XDG_CACHE_HOME": str(cache), "XDG_DATA_HOME": str(data),
        "LANG": "C", "LC_ALL": "C", "TZ": "UTC",
    })
    return environment


def _passed(result: dict[str, object]) -> bool:
    return (
        result["exit_status"] == 0
        and not result["timed_out"]
        and result["launch_error"] is None
        and not result["failure_markers"]
        and not result["orphaned_process_group"]
        and result["cleanup_error"] is None
    )


def run_command(
    command: Sequence[str], *, repo_root: Path, timeout_seconds: float,
    log_path: Path, displayed_command: Sequence[str] | None = None,
    extra_env: dict[str, str] | None = None,
    start_gate: threading.Barrier | None = None,
    home_parent: Path | None = None,
    prepare_home: Callable[[Path], None] | None = None,
) -> dict[str, object]:
    """Run one isolated process group and return its complete lifecycle facts."""
    _positive_float(timeout_seconds)
    if not command:
        raise GameTestError("launch command must not be empty")
    started = time.monotonic()
    output = b""
    exit_status: int | None = None
    timed_out = False
    launch_error: str | None = None
    cleanup_action = "none"
    cleanup_error: str | None = None
    orphaned_process_group = False
    if home_parent is not None:
        home_parent.mkdir(parents=True, exist_ok=True)
    home_context = (
        nullcontext(tempfile.mkdtemp(prefix="hp2-game-test-", dir=home_parent))
        if home_parent is not None
        else tempfile.TemporaryDirectory(prefix="hp2-game-test-")
    )
    with home_context as home_name:
        isolated_home = Path(home_name)
        process: subprocess.Popen[bytes] | None = None
        try:
            child_environment = _isolated_environment(isolated_home)
            if prepare_home is not None:
                prepare_home(isolated_home)
            if extra_env:
                child_environment.update(extra_env)
            if start_gate is not None:
                try:
                    start_gate.wait(timeout=timeout_seconds)
                except threading.BrokenBarrierError as error:
                    raise GameTestError("paired launch start barrier failed") from error
            process = subprocess.Popen(
                command, cwd=repo_root, env=child_environment,
                stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                start_new_session=True,
            )
            try:
                output, _ = process.communicate(timeout=timeout_seconds)
            except subprocess.TimeoutExpired as error:
                timed_out = True
                partial_output = error.output or b""
                cleanup_action = "sigterm"
                cleanup_error = _signal_group(process.pid, signal.SIGTERM)
                try:
                    output, _ = process.communicate(timeout=TERMINATION_GRACE_SECONDS)
                except subprocess.TimeoutExpired as term_error:
                    partial_output = term_error.output or partial_output
                    cleanup_action = "sigkill"
                    kill_error = _signal_group(process.pid, signal.SIGKILL)
                    if kill_error:
                        cleanup_error = "; ".join(item for item in (cleanup_error, kill_error) if item)
                    try:
                        output, _ = process.communicate(timeout=KILL_GRACE_SECONDS)
                    except subprocess.TimeoutExpired as kill_error:
                        output = kill_error.output or partial_output
                        cleanup_action = "failed"
                        cleanup_error = "; ".join(item for item in (cleanup_error, "app did not exit after process-group SIGKILL") if item)
                if not output:
                    output = partial_output
            exit_status = process.returncode
            if _group_exists(process.pid):
                orphaned_process_group = not timed_out
                remaining_action, remaining_error = _stop_remaining_group(process.pid)
                if remaining_action != "none":
                    cleanup_action = remaining_action
                if remaining_error:
                    cleanup_error = "; ".join(item for item in (cleanup_error, remaining_error) if item)
        except OSError as error:
            launch_error = _report_line(f"{type(error).__name__}: {error}", repo_root, isolated_home)
            output = f"game test launch error: {error}\n".encode("utf-8", errors="backslashreplace")
        finally:
            if process is not None and process.poll() is None:
                orphaned_process_group = True
                cleanup_action = "sigterm"
                final_error = _signal_group(process.pid, signal.SIGTERM)
                if final_error:
                    cleanup_error = "; ".join(item for item in (cleanup_error, final_error) if item)
                try:
                    final_output, _ = process.communicate(timeout=TERMINATION_GRACE_SECONDS)
                    output = final_output or output
                except subprocess.TimeoutExpired as error:
                    cleanup_action = "sigkill"
                    partial_output = error.output or output
                    kill_error = _signal_group(process.pid, signal.SIGKILL)
                    if kill_error:
                        cleanup_error = "; ".join(item for item in (cleanup_error, kill_error) if item)
                    try:
                        final_output, _ = process.communicate(timeout=KILL_GRACE_SECONDS)
                        output = final_output or partial_output
                    except subprocess.TimeoutExpired as kill_error:
                        output = kill_error.output or partial_output
                        cleanup_action = "failed"
                        cleanup_error = "; ".join(item for item in (cleanup_error, "app could not be reaped after SIGKILL") if item)
                exit_status = process.returncode
                if _group_exists(process.pid):
                    remaining_action, remaining_error = _stop_remaining_group(process.pid)
                    if remaining_action != "none":
                        cleanup_action = remaining_action
                    if remaining_error:
                        cleanup_error = "; ".join(item for item in (cleanup_error, remaining_error) if item)
        markers = _failure_markers(output, repo_root, isolated_home)
    _write_atomic(log_path, output)
    result: dict[str, object] = {
        "command": list(displayed_command or command), "log": _display_path(log_path, repo_root),
        "extra_environment": dict(sorted((extra_env or {}).items())),
        "captured_output_bytes": len(output), "duration_seconds": round(time.monotonic() - started, 6),
        "exit_status": exit_status, "timed_out": timed_out, "launch_error": launch_error,
        "failure_markers": markers, "orphaned_process_group": orphaned_process_group,
        "process_group_cleanup": cleanup_action, "cleanup_error": cleanup_error,
        "resources": _resource_markers(output),
    }
    if home_parent is not None:
        result["isolated_home"] = _display_path(isolated_home, repo_root)
    result["passed"] = _passed(result)
    return result


def run_game(
    *, app: Path, data_root: Path, selected_map: str, renderer: str, ticks: int,
    timeout_seconds: float, log_path: Path, no_sound: bool = False,
    extra_env: dict[str, str] | None = None,
    ini_file: Path | None = None,
    engine_bin: Path | None = None,
) -> dict[str, object]:
    """Validate an app bundle or override engine binary, then launch one map."""
    repo_root = _repo_root()
    app = _resolve_path(app, repo_root, strict=True)
    data_root = _resolve_path(data_root, repo_root, strict=True)
    log_path = _resolve_path(log_path, repo_root, strict=False)
    _validate_data_root(data_root)
    _positive_float(timeout_seconds)
    if engine_bin is None:
        executable = _bundle_executable(app)
    else:
        executable = _resolve_path(engine_bin, repo_root, strict=True)
    _validate_native_arm64(executable)
    map_path, map_token = _resolve_map(data_root, selected_map)
    command = build_launch_command(
        executable, data_root, map_token, renderer, ticks,
        no_sound=no_sound, ini_file=ini_file,
    )
    displayed_command = [
        _display_path(executable, repo_root), f"-datadir={_display_path(data_root, repo_root)}",
        map_token, *command[3:],
    ]
    result = run_command(command, repo_root=repo_root, timeout_seconds=timeout_seconds, log_path=log_path, displayed_command=displayed_command, extra_env=extra_env)
    result.update({"map": selected_map, "map_token": map_token})
    return result


@dataclass(frozen=True)
class ReplayScenario:
    stem: str
    url: str
    tick_delta: float
    frame_count: int
    replay_bytes: bytes
    metadata: dict[str, object]


def _input_script_smoke_module() -> Any:
    module_path = _repo_root() / "Tests" / "InputScriptSmoke.py"
    spec = importlib.util.spec_from_file_location("_hp2_input_script_smoke", module_path)
    if spec is None or spec.loader is None:
        raise GameTestError(f"cannot load replay compiler: {module_path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _replay_stem(fixture: Path) -> str:
    stem = fixture.stem
    if stem.endswith(".rep"):
        stem = stem[:-4]
    if not stem or REPLAY_STEM_PATTERN.fullmatch(stem) is None:
        raise GameTestError(f"replay fixture name is not launch-safe: {fixture.name}")
    return stem


def _compile_replay_scenario(
    fixture: Path, url: str, artifact_root: Path, repo_root: Path,
) -> ReplayScenario:
    fixture = _resolve_path(fixture, repo_root, strict=True)
    try:
        fixture_bytes = fixture.read_bytes()
        compiler = _input_script_smoke_module()
        tick_delta, frames = compiler.parse_fixture(fixture_bytes.decode("utf-8"))
        compiler.validate_launch_safe_frames(frames)
        replay_bytes = compiler.compile_replay(url, tick_delta, frames)
    except GameTestError:
        raise
    except (OSError, UnicodeError, ValueError) as error:
        raise GameTestError(f"cannot compile replay fixture {fixture}: {error}") from error
    stem = _replay_stem(fixture)
    metadata: dict[str, object] = {
        "version": 1, "wire_format": "FReplay::FInputEvent", "url": url,
        "tick_delta": tick_delta, "frame_count": len(frames),
        "fixture": {
            "path": _display_path(fixture, repo_root),
            "sha256": hashlib.sha256(fixture_bytes).hexdigest(),
            "bytes": len(fixture_bytes),
        },
        "compiled": {
            "path": f"replay/{stem}.rep",
            "sha256": hashlib.sha256(replay_bytes).hexdigest(),
            "bytes": len(replay_bytes),
        },
    }
    replay = ReplayScenario(stem, url, tick_delta, len(frames), replay_bytes, metadata)
    replay_dir = artifact_root / "replay"
    _write_atomic(replay_dir / f"{stem}.rep", replay_bytes)
    _write_atomic(replay_dir / "metadata.json", (
        json.dumps(metadata, ensure_ascii=True, allow_nan=False, sort_keys=True, indent=2) + "\n"
    ).encode())
    return replay


def _stage_replay_home(home: Path, data_root: Path, replay: ReplayScenario) -> Path:
    user_directory = home / _REPLAY_USER_DIRECTORY
    user_directory.mkdir(parents=True, exist_ok=True)
    _write_atomic(user_directory / f"{replay.stem}.rep", replay.replay_bytes)
    return user_directory


def _rust_replay_marker_observation(output: bytes, replay: ReplayScenario) -> dict[str, object]:
    matches = list(RUST_REPLAY_MARKER.finditer(output.decode("utf-8", errors="replace")))
    if not matches:
        return {
            "status": "missing", "code": "replay.rust_marker_absent",
            "detail": f"Rust log did not publish {RUST_REPLAY_MARKER_PREFIX}",
        }
    match = matches[-1]
    observed = {
        "url": match.group("url"), "frames": int(match.group("frames")),
        "first_tick": float(match.group("first_tick")), "seed": int(match.group("seed")),
    }
    expected = {
        "url": replay.url, "frames": replay.frame_count,
        "first_tick": replay.tick_delta, "seed": 1,
    }
    if observed != expected:
        return {
            "status": "invalid", "code": "replay.rust_marker_mismatch",
            "detail": "Rust replay capability marker does not match the staged replay",
            "expected": expected, "observed": observed,
        }
    return {"status": "observed", **observed}


def _apply_rust_replay_marker(
    result: dict[str, object], log_path: Path, replay: ReplayScenario,
) -> None:
    try:
        observation = _rust_replay_marker_observation(log_path.read_bytes(), replay)
    except OSError as error:
        observation = {
            "status": "missing", "code": "replay.rust_log_unreadable", "detail": str(error),
        }
    replay_result = result.setdefault("replay", {})
    assert isinstance(replay_result, dict)
    replay_result["rust_capability"] = observation
    if observation["status"] != "observed":
        result["passed"] = False


def _json_integer(value: object) -> bool:
    return isinstance(value, int) and not isinstance(value, bool)


def _rng_trace_problem(code: str, detail: str) -> dict[str, str]:
    return {"code": code, "detail": detail}


def _validate_rng_trace(document: object) -> dict[str, str] | None:
    if not isinstance(document, dict):
        return _rng_trace_problem("rng_trace.document_invalid", "root must be an object")
    header = (
        "version", "enabled", "rand_max", "count", "call_count", "limit",
        "capture_available", "truncated", "outputs", "tick_boundaries", "samples",
    )
    missing = [key for key in header if key not in document]
    if missing:
        return _rng_trace_problem("rng_trace.header_missing", f"missing required field {missing[0]}")
    if document["version"] != RNG_TRACE_FORMAT_VERSION or isinstance(document["version"], bool):
        return _rng_trace_problem("rng_trace.version_invalid", "version must equal 2")
    if document["enabled"] is not True:
        return _rng_trace_problem("rng_trace.enabled_invalid", "enabled must be true")
    for field in ("rand_max", "count", "call_count", "limit"):
        if not _json_integer(document[field]) or document[field] < 0:
            return _rng_trace_problem("rng_trace.header_invalid", f"{field} must be a non-negative integer")
    if document["rand_max"] == 0:
        return _rng_trace_problem("rng_trace.rand_max_invalid", "rand_max must be positive")
    for field in ("capture_available", "truncated"):
        if not isinstance(document[field], bool):
            return _rng_trace_problem("rng_trace.header_invalid", f"{field} must be a boolean")
    has_seed = "seed" in document
    has_reason = "start_reason" in document
    has_requested = "requested_seed" in document
    if has_seed != has_reason:
        return _rng_trace_problem("rng_trace.replay_header_invalid", "seed and start_reason must either both be present or both be absent")
    if has_seed and (
        not _json_integer(document["seed"]) or not 0 <= document["seed"] <= document["rand_max"]
        or document["start_reason"] not in ("diagnostic", "replay")
    ):
        return _rng_trace_problem("rng_trace.start_metadata_invalid", "seed must be within rand_max and start_reason must be diagnostic or replay")
    if has_requested and (
        not has_seed or document["start_reason"] != "diagnostic"
        or not _json_integer(document["requested_seed"])
        or not 0 <= document["requested_seed"] <= document["rand_max"]
    ):
        return _rng_trace_problem("rng_trace.start_metadata_invalid", "requested_seed must be a diagnostic unsigned seed within rand_max")
    outputs, samples, boundaries = document["outputs"], document["samples"], document["tick_boundaries"]
    if not isinstance(outputs, list) or not isinstance(samples, list) or not isinstance(boundaries, list):
        return _rng_trace_problem("rng_trace.collection_invalid", "outputs, samples, and tick_boundaries must be arrays")
    if len(outputs) != document["count"] or len(samples) != document["count"]:
        return _rng_trace_problem("rng_trace.count_invalid", "count must equal both output and sample counts")
    if document["call_count"] < document["count"]:
        return _rng_trace_problem("rng_trace.call_count_invalid", "call_count cannot be less than count")
    if len(boundaries) > document["limit"]:
        return _rng_trace_problem("rng_trace.boundary_count_invalid", "tick boundaries exceed the configured limit")
    for index, boundary in enumerate(boundaries):
        if not isinstance(boundary, dict) or not _json_integer(boundary.get("index")) or boundary["index"] < 0 or not isinstance(boundary.get("phase"), str) or not boundary["phase"]:
            return _rng_trace_problem("rng_trace.boundary_invalid", f"tick boundary {index} is malformed")
    consumer_fields = ("kind", "actor", "class", "function", "native", "native_slot", "callsite_offset")
    for index, (output, sample) in enumerate(zip(outputs, samples, strict=True)):
        if not isinstance(output, dict) or not _json_integer(output.get("ordinal")) or not _json_integer(output.get("raw")) or output["ordinal"] != index or not 0 <= output["raw"] <= document["rand_max"]:
            return _rng_trace_problem("rng_trace.output_invalid", f"output {index} is malformed")
        if not isinstance(sample, dict):
            return _rng_trace_problem("rng_trace.sample_invalid", f"sample {index} does not match its output")
        required_sample = {"ordinal", "raw", "tick", "consumer"}
        allowed_sample = required_sample | {"outer_consumer"}
        if not required_sample.issubset(sample) or set(sample) - allowed_sample:
            return _rng_trace_problem("rng_trace.sample_schema_invalid", f"sample {index} must contain ordinal, raw, tick, and consumer")
        if sample["ordinal"] != output["ordinal"] or sample["raw"] != output["raw"] or not isinstance(sample["tick"], dict):
            return _rng_trace_problem("rng_trace.sample_invalid", f"sample {index} does not match its output")
        tick = sample["tick"]
        if set(tick) != {"index", "phase"} or not _json_integer(tick.get("index")) or tick["index"] < 0 or not isinstance(tick.get("phase"), str) or not tick["phase"]:
            return _rng_trace_problem("rng_trace.sample_tick_invalid", f"sample {index} has an invalid tick")
        for consumer_name in ("consumer", "outer_consumer"):
            if consumer_name not in sample:
                continue
            consumer = sample[consumer_name]
            if not isinstance(consumer, dict) or any(field not in consumer for field in consumer_fields):
                return _rng_trace_problem("rng_trace.consumer_invalid", f"sample {index} has an incomplete {consumer_name}")
            if not isinstance(consumer["kind"], str) or not consumer["kind"]:
                return _rng_trace_problem("rng_trace.consumer_invalid", f"sample {index} {consumer_name} kind is invalid")
            if any(value is not None and not isinstance(value, str) for value in (consumer["actor"], consumer["class"], consumer["function"], consumer["native"])):
                return _rng_trace_problem("rng_trace.consumer_invalid", f"sample {index} {consumer_name} paths must be strings or null")
            if any(value is not None and not _json_integer(value) for value in (consumer["native_slot"], consumer["callsite_offset"])):
                return _rng_trace_problem("rng_trace.consumer_invalid", f"sample {index} {consumer_name} offsets must be integers or null")
    return None


def _rng_trace_observation(path: Path, repo_root: Path) -> dict[str, object]:
    base: dict[str, object] = {"path": _display_path(path, repo_root)}
    if not path.is_file():
        return {**base, "status": "missing", "diagnostic": _rng_trace_problem(
            "rng_trace.artifact_missing", "RNG trace was not published",
        )}
    try:
        document = json.loads(path.read_text())
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        return {**base, "status": "invalid", "diagnostic": _rng_trace_problem(
            "rng_trace.artifact_invalid", str(error),
        )}
    problem = _validate_rng_trace(document)
    if problem:
        return {**base, "status": "invalid", "diagnostic": problem}
    assert isinstance(document, dict)
    metadata = {
        key: document[key] for key in (
            "version", "enabled", "rand_max", "seed", "requested_seed",
            "start_reason", "capture_available", "limit", "count", "call_count",
            "truncated",
        ) if key in document
    }
    return {
        **base, "status": "valid", "metadata": metadata,
        "output_count": len(document["outputs"]),
        "sample_count": len(document["samples"]),
        "tick_boundary_count": len(document["tick_boundaries"]),
    }


def _prepare_pair_paths(directory: Path, cpp_name: str, rust_name: str) -> dict[str, Path]:
    directory.mkdir(parents=True, exist_ok=True)
    paths = {"cpp": directory / cpp_name, "rust": directory / rust_name}
    for path in paths.values():
        path.unlink(missing_ok=True)
    return paths


def _prepare_rng_trace_paths(directory: Path) -> dict[str, Path]:
    return _prepare_pair_paths(directory, "cpp-rng-trace.json", "rust-rng-trace.json")


def _trace_observation(
    path: Path, repo_root: Path, *, prefix: str, validator: Callable[[object], object],
    count_key: str, event_key: str,
) -> dict[str, object]:
    base = {"path": _display_path(path, repo_root)}
    if not path.is_file():
        return {**base, "status": "missing", "diagnostic": {
            "code": f"{prefix}.artifact_missing",
            "detail": f"{prefix.replace('_', ' ')} was not published",
        }}
    try:
        document = json.loads(path.read_text())
        validated = validator(document)
    except (OSError, UnicodeError, json.JSONDecodeError, ValueError, TypeError) as error:
        return {**base, "status": "invalid", "diagnostic": {
            "code": f"{prefix}.artifact_invalid", "detail": str(error),
        }}
    return {
        **base, "status": "valid",
        "metadata": {
            key: validated[key] for key in ("version", "enabled", "limit", "count", "truncated")
            if key in validated
        },
        count_key: len(validated.get(event_key, [])),
    }


def _make_trace_family(
    family: str, stem: str, module: Any, count_key: str, event_key: str,
) -> tuple[Callable[[Path], dict[str, Path]], Callable[[Path, Path], dict[str, object]]]:
    def prepare(directory: Path) -> dict[str, Path]:
        return _prepare_pair_paths(directory, f"cpp-{stem}.json", f"rust-{stem}.json")
    def observe(path: Path, repo_root: Path) -> dict[str, object]:
        return _trace_observation(
            path, repo_root, prefix=family, validator=module.validate_trace,
            count_key=count_key, event_key=event_key,
        )
    return prepare, observe


_prepare_shadow_update_trace_paths, _shadow_update_trace_observation = _make_trace_family(
    "shadow_update_trace", "shadow-update-trace", shadow_update_trace, "event_count", "events",
)
_prepare_shadow_admission_trace_paths, _shadow_admission_trace_observation = _make_trace_family(
    "shadow_admission_trace", "shadow-admission-trace", shadow_admission_trace, "event_count", "events",
)
_prepare_actor_transition_ledger_paths, _actor_transition_ledger_observation = _make_trace_family(
    "actor_transition_ledger", "actor-transition-ledger", actor_transition_ledger, "owner_count", "owners",
)
_prepare_creature_generator_trace_paths, _creature_generator_trace_observation = _make_trace_family(
    "creature_generator_trace", "creature-generator-trace", creature_generator_trace, "event_count", "events",
)
_prepare_global_tick_trace_paths, _global_tick_trace_observation = _make_trace_family(
    "global_tick_trace", "global-tick-trace", global_tick_trace, "event_count", "events",
)
_prepare_startup_rng_phase_trace_paths, _startup_rng_phase_trace_observation = _make_trace_family(
    "startup_rng_phase_trace", "startup-rng-phase-trace", startup_rng_phase_trace, "phase_count", "phases",
)
_prepare_fire_texture_trace_paths, _generic_fire_texture_trace_observation = _make_trace_family(
    "fire_texture_trace", "fire-texture-trace", fire_texture_trace, "event_count", "transitions",
)


def _fire_texture_trace_observation(path: Path, repo_root: Path) -> dict[str, object]:
    observation = _generic_fire_texture_trace_observation(path, repo_root)
    if observation["status"] != "valid":
        return observation
    validated = fire_texture_trace.validate_trace(json.loads(path.read_text(encoding="utf-8")))
    target = validated["target"]
    observation["provenance"] = {
        key: target[key] for key in ("archive", "export", "class", "dimensions")
    }
    return observation
def _prepare_actor_slot_dump_paths(directory: Path) -> dict[str, Path]:
    return _prepare_pair_paths(directory, "cpp-actor-slots.json", "rust-actor-slots.json")


def _actor_slot_dump_observation(path: Path, repo_root: Path) -> dict[str, object]:
    observation: dict[str, object] = {"path": _display_path(path, repo_root)}
    if not path.is_file():
        return {**observation, "status": "missing", "diagnostic": {
            "code": "actor_slot_dump.artifact_missing", "detail": "actor-slot dump was not published",
        }}
    try:
        contents = path.read_bytes()
        document = json.loads(contents.decode("utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        return {**observation, "status": "invalid", "diagnostic": {
            "code": "actor_slot_dump.artifact_invalid", "detail": str(error),
        }}
    if not isinstance(document, dict) or document.get("version") != ACTOR_SLOT_DUMP_FORMAT_VERSION:
        return {**observation, "status": "invalid", "diagnostic": {
            "code": "actor_slot_dump.document_invalid", "detail": "expected version 1 object",
        }}
    rows = document.get("checkpoints")
    if not isinstance(rows, list) or any(not isinstance(item, dict) for item in rows):
        return {**observation, "status": "invalid", "diagnostic": {
            "code": "actor_slot_dump.document_invalid", "detail": "checkpoints must be an array",
        }}
    names = [item.get("checkpoint") for item in rows]
    missing = [name for name in _ACTOR_SLOT_DUMP_CHECKPOINTS if name not in names]
    if missing:
        return {**observation, "status": "invalid", "diagnostic": {
            "code": "actor_slot_dump.checkpoint_missing", "detail": f"missing required checkpoint {missing[0]}",
        }}
    for row in rows:
        detail = _actor_slot_checkpoint_schema_error(row)
        if detail is not None:
            return {**observation, "status": "invalid", "diagnostic": {
                "code": "actor_slot_dump.checkpoint_invalid", "detail": detail,
            }}
    return {
        **observation, "status": "valid", "bytes": len(contents),
        "sha256": hashlib.sha256(contents).hexdigest(),
        "metadata": {"version": document["version"], "checkpoints": names},
    }


def _actor_slot_semantic_fields(actor: dict[str, object]) -> dict[str, object]:
    return {
        field: value for field, value in actor.items()
        if field not in _ACTOR_SLOT_RAW_ID_FIELDS and field not in _ACTOR_SLOT_NON_SEMANTIC_FIELDS
    }


def _actor_slot_semantic_group(
    *, semantic: dict[str, object], cpp_actors: list[dict[str, object]],
    rust_actors: list[dict[str, object]],
) -> dict[str, object]:
    return {
        "semantic": semantic, "cpp": {"count": len(cpp_actors), "actors": cpp_actors},
        "rust": {"count": len(rust_actors), "actors": rust_actors},
    }


def _actor_slot_semantic_checkpoint(
    checkpoint: str, cpp_row: dict[str, object], rust_row: dict[str, object],
) -> dict[str, object]:
    cpp_actors = cpp_row["actors"]
    rust_actors = rust_row["actors"]
    assert isinstance(cpp_actors, list) and isinstance(rust_actors, list)
    groups: dict[str, dict[str, object]] = {}
    for side, actors in (("cpp", cpp_actors), ("rust", rust_actors)):
        for actor in actors:
            assert isinstance(actor, dict)
            if actor["is_null"]:
                continue
            semantic = _actor_slot_semantic_fields(actor)
            key = json.dumps(
                semantic, ensure_ascii=True, allow_nan=False, sort_keys=True,
                separators=(",", ":"),
            )
            group = groups.setdefault(key, {"semantic": semantic, "cpp": [], "rust": []})
            side_actors = group[side]
            assert isinstance(side_actors, list)
            side_actors.append(actor)
    grouped = [
        _actor_slot_semantic_group(
            semantic=group["semantic"], cpp_actors=group["cpp"], rust_actors=group["rust"],
        )
        for _, group in sorted(groups.items())
    ]
    normalized_order = {
        side: [
            None if actor["is_null"] else _actor_slot_semantic_fields(actor)
            for actor in actors
        ]
        for side, actors in (("cpp", cpp_actors), ("rust", rust_actors))
    }
    null_positions = {
        side: [index for index, actor in enumerate(actors) if actor["is_null"]]
        for side, actors in (("cpp", cpp_actors), ("rust", rust_actors))
    }
    boundaries = {
        side: {
            "i_first_net_relevant_actor": row["i_first_net_relevant_actor"],
            "i_first_dynamic_actor": row["i_first_dynamic_actor"],
        }
        for side, row in (("cpp", cpp_row), ("rust", rust_row))
    }
    multiset_difference = [
        group for group in grouped if group["cpp"]["count"] != group["rust"]["count"]
    ]
    ambiguous_groups = [
        group for group in grouped if group["cpp"]["count"] > 1 or group["rust"]["count"] > 1
    ]
    order_match = normalized_order["cpp"] == normalized_order["rust"]
    null_positions_match = null_positions["cpp"] == null_positions["rust"]
    boundaries_match = boundaries["cpp"] == boundaries["rust"]
    return {
        "checkpoint": checkpoint,
        "cpp_slot_count": len(cpp_actors),
        "rust_slot_count": len(rust_actors),
        "groups": grouped,
        "multiset_difference": multiset_difference,
        "ambiguous_groups": ambiguous_groups,
        "normalized_order": normalized_order,
        "order_match": order_match,
        "null_positions": null_positions,
        "null_positions_match": null_positions_match,
        "boundaries": boundaries,
        "boundaries_match": boundaries_match,
        "matches": (
            not multiset_difference
            and not ambiguous_groups
            and order_match
            and null_positions_match
            and boundaries_match
        ),
    }


def _actor_slot_checkpoint_schema_error(row: dict[str, object]) -> str | None:
    checkpoint = row.get("checkpoint")
    if checkpoint not in _ACTOR_SLOT_DUMP_CHECKPOINTS:
        return "checkpoint name is not recognized"
    is_raw = checkpoint == "post_deserialize_raw"
    if row.get("sequence") != ("raw" if is_raw else "rearranged"):
        return f"{checkpoint}: sequence kind is incorrect"
    actors = row.get("actors")
    if not isinstance(actors, list) or any(not isinstance(actor, dict) for actor in actors):
        return f"{checkpoint}: actors must be an actor-object array"
    for index, actor in enumerate(actors):
        assert isinstance(actor, dict)
        if actor.get("slot") != index:
            return f"{checkpoint}: actor slot {index} is missing or mislabeled"
        if type(actor.get("is_null")) is not bool:
            return f"{checkpoint}: actor slot {index} lacks a boolean is_null marker"
    first_net = row.get("i_first_net_relevant_actor")
    first_dynamic = row.get("i_first_dynamic_actor")
    if is_raw:
        if first_net is not None or first_dynamic is not None:
            return f"{checkpoint}: raw sequence must not publish rearranged boundaries"
    elif (
        type(first_net) is not int
        or type(first_dynamic) is not int
        or not 0 <= first_net <= first_dynamic <= len(actors)
    ):
        return f"{checkpoint}: rearranged boundaries are missing, reversed, or out of range"
    return None


def _actor_slot_dump_document_for_semantic_join(
    path: Path,
) -> tuple[dict[str, dict[str, object]] | None, dict[str, object] | None]:
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        return None, {"code": "actor_slot_dump.semantic_join_unavailable", "detail": str(error)}
    if not isinstance(document, dict) or document.get("version") != ACTOR_SLOT_DUMP_FORMAT_VERSION:
        return None, {"code": "actor_slot_dump.semantic_join_invalid_document", "detail": "expected version 1 actor-slot dump object"}
    rows = document.get("checkpoints")
    if not isinstance(rows, list):
        return None, {"code": "actor_slot_dump.semantic_join_invalid_document", "detail": "expected actor-slot dump checkpoints array"}
    checkpoints: dict[str, dict[str, object]] = {}
    for row in rows:
        if not isinstance(row, dict):
            return None, {"code": "actor_slot_dump.semantic_join_invalid_checkpoint", "detail": "checkpoint must be an object"}
        detail = _actor_slot_checkpoint_schema_error(row)
        if detail is not None:
            return None, {"code": "actor_slot_dump.semantic_join_invalid_checkpoint", "detail": detail}
        checkpoint = row["checkpoint"]
        assert isinstance(checkpoint, str)
        if checkpoint in checkpoints:
            return None, {"code": "actor_slot_dump.semantic_join_invalid_checkpoint", "detail": f"duplicate checkpoint {checkpoint}"}
        checkpoints[checkpoint] = row
    missing = [name for name in _ACTOR_SLOT_DUMP_CHECKPOINTS if name not in checkpoints]
    if missing:
        return None, {"code": "actor_slot_dump.semantic_join_checkpoint_missing", "detail": f"missing required checkpoint {missing[0]}"}
    return checkpoints, None


def _actor_slot_semantic_join_diagnostic(cpp_path: Path, rust_path: Path) -> dict[str, object]:
    cpp, cpp_diagnostic = _actor_slot_dump_document_for_semantic_join(cpp_path)
    rust, rust_diagnostic = _actor_slot_dump_document_for_semantic_join(rust_path)
    if cpp is None or rust is None:
        return {"status": "unavailable", "diagnostic": {
            side: diagnostic for side, diagnostic in (("cpp", cpp_diagnostic), ("rust", rust_diagnostic))
            if diagnostic is not None
        }}
    checkpoints = [
        _actor_slot_semantic_checkpoint(name, cpp[name], rust[name])
        for name in _ACTOR_SLOT_DUMP_CHECKPOINTS
    ]
    return {
        "status": "observed",
        "matches": all(checkpoint["matches"] for checkpoint in checkpoints),
        "checkpoints": checkpoints,
    }


def _profile_for_data_root(data_root: Path, repo_root: Path) -> str:
    if data_root == (repo_root / "HarryPotter2" / "Unreal").resolve():
        return "data-prototype"
    manifest = data_root / "overlay-manifest.json"
    if not manifest.is_file():
        return "data-custom"
    try:
        document = json.loads(manifest.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise GameTestError(
            f"fidelity.data_profile_invalid: cannot read {manifest}: {error}"
        ) from error
    if not isinstance(document, dict):
        raise GameTestError(
            f"fidelity.data_profile_invalid: manifest is not an object: {manifest}"
        )
    return "data-retail" if document.get("profile") == "retail-only" else "data-custom"


def _cutscript_lines(data: bytes) -> Iterator[tuple[int, str]]:
    for line_number, line in enumerate(re.split(b"\r\n|\r|\n", data), start=1):
        if line_number == 1 or line or line_number <= len(data.splitlines()):
            yield line_number, line.decode("cp1252", errors="replace")


def _cutscript_command(line: str) -> str:
    comments = [index for index in (line.find(";"), line.find("/")) if index >= 0]
    return line[:min(comments)].rstrip() if comments else line.rstrip()


def _thread_slot(thread: str) -> int:
    match = re.fullmatch(r"thread_?(\d+).*", thread, flags=re.IGNORECASE)
    if match is None:
        raise GameTestError(f"fidelity.cutscript_thread_unresolved: unsupported thread name {thread}")
    return int(match.group(1))


def _cutscene_script(data_root: Path, map_path: Path) -> Path:
    try:
        raw_map = map_path.read_bytes()
    except OSError as error:
        raise GameTestError(f"cannot read map for CutScriptDisk.Script: {error}") from error
    candidates: set[Path] = set()
    for match in re.findall(rb"(?<![A-Za-z0-9_])[A-Za-z0-9_-]{3,}(?![A-Za-z0-9_])", raw_map):
        path = data_root / "CutScenes" / f"{match.decode('ascii', errors='ignore')}.txt"
        if path.is_file():
            candidates.add(path.resolve())
    if len(candidates) != 1:
        names = ", ".join(sorted(str(path) for path in candidates)) or "none"
        raise GameTestError(f"fidelity.cutscript_unresolved: expected one committed CutScriptDisk.Script source in {map_path}, found {names}")
    return candidates.pop()


def _cutscript_disk_locators(data_root: Path, script_path: Path) -> dict[tuple[int, str], list[int]]:
    disk_path = data_root / "System" / "CUTSCENES" / f"{script_path.stem}.int"
    try:
        disk = disk_path.read_text(encoding="cp1252")
    except OSError as error:
        raise GameTestError(f"fidelity.cutscript_disk_unresolved: cannot read CutScriptDisk rows {disk_path}: {error}") from error
    active_thread: int | None = None
    locators: dict[tuple[int, str], list[int]] = {}
    for line in disk.splitlines():
        section = re.fullmatch(r"\s*\[thread_(\d+)\]\s*", line, flags=re.IGNORECASE)
        if section:
            active_thread = int(section.group(1))
            continue
        entry = re.fullmatch(r"line_(\d+)=(.*)", line, flags=re.IGNORECASE)
        if active_thread is None or entry is None:
            continue
        command = _cutscript_command(entry.group(2))
        locators.setdefault((active_thread, " ".join(command.split()).casefold()), []).append(int(entry.group(1)))
    return locators


def _selected_cutscript_command(command: str) -> bool:
    words = {word.casefold() for word in command.split()}
    return bool(words & _CUTSCRIPT_COMMANDS or words & {"followspline", "flyto"} or re.search(r"(?:^|\s)time\s*=", command.casefold()))


def _retail_source_checkpoints(data_root: Path, map_path: Path) -> list[dict[str, object]]:
    try:
        raw_map = map_path.read_bytes()
    except OSError as error:
        raise GameTestError(f"cannot read map for retail CutScript census: {error}") from error
    directory = data_root / "System" / "CUTSCENES"
    try:
        candidates = [
            path for path in directory.iterdir()
            if path.is_file() and path.suffix.casefold() == ".int"
            and path.stem.encode("ascii", errors="ignore") in raw_map
        ]
    except OSError as error:
        raise GameTestError(f"fidelity.cutscript_disk_unresolved: cannot enumerate {directory}: {error}") from error
    candidates.sort(key=lambda path: os.fsencode(path.name))
    if len(candidates) != 1:
        raise GameTestError(
            "fidelity.cutscript_unresolved: retail-only profile requires exactly "
            f"one referenced System/CUTSCENES locator file, found {len(candidates)}"
        )
    script_path = candidates[0]
    source = script_path.read_bytes()
    source_hash = hashlib.sha256(source).hexdigest()
    active_slot: int | None = None
    checkpoints: list[dict[str, object]] = []
    for physical_line, line in enumerate(source.decode("cp1252").splitlines(), start=1):
        section = re.fullmatch(r"\s*\[thread_(\d+)\]\s*", line, flags=re.IGNORECASE)
        if section:
            active_slot = int(section.group(1))
            continue
        entry = re.fullmatch(r"line_(\d+)=(.*)", line, flags=re.IGNORECASE)
        if active_slot is None or active_slot > 3 or entry is None:
            continue
        command = _cutscript_command(entry.group(2))
        if not command or not _selected_cutscript_command(command):
            continue
        thread = f"Thread{active_slot}"
        source_line: dict[str, object] = {
            "path": _display_path(script_path, _repo_root()), "line": physical_line,
            "thread": thread, "text": command, "sha256": source_hash,
        }
        cue = re.search(r"\*([^\s]+)", command)
        if cue:
            source_line["cue"] = cue.group(1)
        locator = {
            "path": f"System/CUTSCENES/{script_path.name}",
            "thread_slot": active_slot, "line_index": int(entry.group(1)),
        }
        for position in ("before", "at", "after"):
            checkpoints.append({
                "id": f"{thread}:{physical_line}:{position}", "position": position,
                "runtime_locator": locator, "source_line": source_line,
            })
    if not checkpoints:
        raise GameTestError("fidelity.checkpoints_empty: profile-selected locator census is empty")
    return checkpoints


def _source_checkpoints(data_root: Path, map_path: Path) -> list[dict[str, object]]:
    profile = _profile_for_data_root(data_root, _repo_root())
    if profile == "data-retail":
        return _retail_source_checkpoints(data_root, map_path)
    if profile == "data-custom" and not (data_root / "CutScenes").is_dir():
        raise GameTestError(
            "fidelity.data_profile_unsupported: locator extraction requires "
            "repository prototype data or a retail-only overlay manifest"
        )
    script_path = _cutscene_script(data_root, map_path)
    try:
        source = script_path.read_bytes()
    except OSError as error:
        raise GameTestError(f"cannot read CutScriptDisk.Script {script_path}: {error}") from error
    locators = _cutscript_disk_locators(data_root, script_path)
    active_thread: str | None = None
    rows: list[tuple[str, int, str, int]] = []
    for line_number, line in _cutscript_lines(source):
        section = re.fullmatch(r"\s*\[([^\]]+)\]\s*", line)
        if section:
            active_thread = section.group(1)
            continue
        command = _cutscript_command(line)
        if active_thread is None or active_thread.casefold() not in {"thread0basecam", "thread1harry", "thread2narrator", "thread3dobby"} or not command or not _selected_cutscript_command(command):
            continue
        key = (_thread_slot(active_thread), " ".join(command.split()).casefold())
        line_indices = locators.get(key)
        if not line_indices:
            raise GameTestError(f"fidelity.cutscript_disk_locator_missing: no CutScriptDisk row for {active_thread}:{line_number} {command}")
        rows.append((active_thread, line_number, command, line_indices.pop(0)))
    checkpoints: list[dict[str, object]] = []
    source_hash = hashlib.sha256(source).hexdigest()
    for thread, line_number, command, line_index in rows:
        source_line: dict[str, object] = {
            "path": _display_path(script_path, _repo_root()), "line": line_number,
            "thread": thread, "text": command, "sha256": source_hash,
        }
        cue = re.search(r"\*([^\s]+)", command)
        if cue:
            source_line["cue"] = cue.group(1)
        locator = {"path": f"System/CUTSCENES/{script_path.stem}.int", "thread_slot": _thread_slot(thread), "line_index": line_index}
        for position in ("before", "at", "after"):
            checkpoints.append({"id": f"{thread}:{line_number}:{position}", "position": position, "runtime_locator": locator, "source_line": source_line})
    if not checkpoints:
        raise GameTestError(f"fidelity.checkpoints_empty: no authored CutScript checkpoints in {script_path}")
    return checkpoints


def _read_fidelity_state(path: Path) -> tuple[dict[str, dict[str, object]], dict[str, object] | None]:
    if not path.is_file():
        return {}, {"code": "fidelity.telemetry_absent", "path": str(path)}
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        return {}, {"code": "fidelity.telemetry_invalid", "detail": str(error)}
    if not isinstance(document, dict) or document.get("version") != FIDELITY_FORMAT_VERSION:
        return {}, {"code": "fidelity.telemetry_invalid", "detail": "expected version 1 object"}
    rows = document.get("checkpoints")
    if not isinstance(rows, list):
        return {}, {"code": "fidelity.telemetry_invalid", "detail": "checkpoints must be an array"}
    diagnostic = document.get("diagnostic")
    states: dict[str, dict[str, object]] = {}
    for item in rows:
        if not isinstance(item, dict) or not isinstance(item.get("id"), str):
            return {}, {"code": "fidelity.telemetry_invalid", "detail": "checkpoint lacks string id"}
        if item["id"] in states:
            return {}, {"code": "fidelity.telemetry_invalid", "detail": f"duplicate checkpoint {item['id']}"}
        states[item["id"]] = item
    return states, diagnostic if isinstance(diagnostic, dict) else None


def _field_observation(state: dict[str, object] | None, field: str, run_diagnostic: dict[str, object] | None) -> tuple[object | None, dict[str, object]]:
    if state is None:
        return None, run_diagnostic or {"code": "fidelity.checkpoint_missing"}
    if field not in state:
        return None, {"code": "fidelity.field_missing", "field": field}
    observation = state[field]
    if not isinstance(observation, dict) or "value" not in observation:
        return None, {"code": "fidelity.field_invalid", "field": field}
    availability = observation.get("availability")
    if not isinstance(availability, dict) or not isinstance(availability.get("status"), str):
        return None, {"code": "fidelity.availability_missing", "field": field}
    status, value = availability["status"], observation["value"]
    if status == "observed" and value is not None:
        return value, availability
    if status == "known_null" and value is None and isinstance(availability.get("code"), str):
        return value, availability
    if status == "blocked_data" and value is None and all(isinstance(availability.get(key), str) and availability[key] for key in ("code", "asset", "profile", "proof")):
        return value, availability
    if status == "unobserved" and value is None and isinstance(availability.get("code"), str):
        return value, availability
    return None, {"code": "fidelity.availability_invalid", "field": field}


def _locator_observation(checkpoint: dict[str, object], state: dict[str, object] | None, run_diagnostic: dict[str, object] | None) -> tuple[object | None, dict[str, object]]:
    expected = checkpoint.get("runtime_locator")
    if expected is None:
        return None, {"status": "not_required"}
    if state is None:
        return None, run_diagnostic or {"code": "fidelity.checkpoint_missing"}
    if "runtime_locator" not in state:
        return None, {"code": "fidelity.runtime_locator_missing"}
    actual = state["runtime_locator"]
    return (actual, {"status": "observed"}) if actual == expected else (actual, {"code": "fidelity.runtime_locator_mismatch", "expected": expected})


def _side_checkpoint(state: dict[str, object] | None, diagnostic: dict[str, object] | None) -> dict[str, object]:
    if state is None:
        return {"state": None, "diagnostic": diagnostic}
    values: dict[str, object] = {}
    diagnostics: dict[str, object] = {}
    for field, _owner in FIDELITY_FIELDS:
        values[field], diagnostics[field] = _field_observation(state, field, diagnostic)
    values.update({"state": state, "diagnostic": diagnostics})
    return values


def _blocked_data_pair(cpp_availability: dict[str, object], rust_availability: dict[str, object]) -> bool:
    return cpp_availability.get("status") == rust_availability.get("status") == "blocked_data" and all(cpp_availability.get(key) == rust_availability.get(key) for key in ("code", "asset", "profile")) and bool(cpp_availability.get("proof")) and bool(rust_availability.get("proof"))


def _canonical_actor_values(value: object) -> tuple[str, ...] | None:
    if not isinstance(value, list) or any(not isinstance(actor, dict) for actor in value):
        return None
    return tuple(sorted(json.dumps(actor, ensure_ascii=False, allow_nan=False, separators=(",", ":"), sort_keys=True) for actor in value))


def _actor_values_match(cpp_value: object, rust_value: object) -> bool:
    cpp_actors, rust_actors = _canonical_actor_values(cpp_value), _canonical_actor_values(rust_value)
    return cpp_value == rust_value if cpp_actors is None or rust_actors is None else cpp_actors == rust_actors


def _compare_checkpoint(checkpoint: dict[str, object], cpp_state: dict[str, object] | None, rust_state: dict[str, object] | None, cpp_diagnostic: dict[str, object] | None, rust_diagnostic: dict[str, object] | None) -> dict[str, object]:
    cpp_locator, cpp_locator_diagnostic = _locator_observation(checkpoint, cpp_state, cpp_diagnostic)
    rust_locator, rust_locator_diagnostic = _locator_observation(checkpoint, rust_state, rust_diagnostic)
    if cpp_locator_diagnostic.get("status") != "not_required" and (cpp_locator_diagnostic.get("status") != "observed" or rust_locator_diagnostic.get("status") != "observed"):
        return {"status": "diverged", "divergence": {"source_line": checkpoint["source_line"], "owner": "vm", "field": "cutscene", "cpp": cpp_locator, "rust": rust_locator, "diagnostic": {"cpp": cpp_locator_diagnostic, "rust": rust_locator_diagnostic}}}
    for field, owner in FIDELITY_FIELDS:
        cpp_value, cpp_availability = _field_observation(cpp_state, field, cpp_diagnostic)
        rust_value, rust_availability = _field_observation(rust_state, field, rust_diagnostic)
        if _blocked_data_pair(cpp_availability, rust_availability):
            return {"status": "blocked_data", "divergence": None, "diagnostic": {"cpp": cpp_availability, "rust": rust_availability}}
        comparable = cpp_availability.get("status") in ("observed", "known_null") and rust_availability.get("status") in ("observed", "known_null") and cpp_availability == rust_availability
        values_match = _actor_values_match(cpp_value, rust_value) if field == "actors" else cpp_value == rust_value
        if not comparable or not values_match:
            return {"status": "diverged", "divergence": {"source_line": checkpoint["source_line"], "owner": owner, "field": field, "cpp": cpp_value, "rust": rust_value, "diagnostic": {"cpp": cpp_availability, "rust": rust_availability}}}
    return {"status": "match", "divergence": None}
def _read_json_document(path: Path) -> tuple[object | None, dict[str, str] | None]:
    try:
        return json.loads(path.read_text(encoding="utf-8")), None
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        return None, {"code": "trace.comparison_unavailable", "detail": str(error)}


def _compare_trace_paths(
    cpp_path: Path, rust_path: Path, module: Any, **kwargs: object,
) -> dict[str, object]:
    cpp_document, cpp_problem = _read_json_document(cpp_path)
    rust_document, rust_problem = _read_json_document(rust_path)
    if cpp_problem or rust_problem:
        return {"status": "unavailable", "first_difference": {"cpp": cpp_problem, "rust": rust_problem}}
    try:
        return module.compare_traces(cpp_document, rust_document, **kwargs)
    except (ValueError, TypeError) as error:
        return {"status": "unavailable", "first_difference": {"detail": str(error)}}


def _compare_shadow_update_traces(cpp_path: Path, rust_path: Path, *, map_stem: str) -> dict[str, object]:
    return _compare_trace_paths(cpp_path, rust_path, shadow_update_trace, map_stem=map_stem)


def _compare_shadow_admission_traces(cpp_path: Path, rust_path: Path, *, map_stem: str) -> dict[str, object]:
    return _compare_trace_paths(cpp_path, rust_path, shadow_admission_trace, map_stem=map_stem)


def _compare_actor_transition_ledgers(cpp_path: Path, rust_path: Path) -> dict[str, object]:
    return _compare_trace_paths(cpp_path, rust_path, actor_transition_ledger)


def _compare_creature_generator_traces(cpp_path: Path, rust_path: Path) -> dict[str, object]:
    return _compare_trace_paths(cpp_path, rust_path, creature_generator_trace)


def _compare_global_tick_traces(cpp_path: Path, rust_path: Path) -> dict[str, object]:
    return _compare_trace_paths(cpp_path, rust_path, global_tick_trace)


def _compare_startup_rng_phase_traces(cpp_path: Path, rust_path: Path, *, requested_map_token: str) -> dict[str, object]:
    return _compare_trace_paths(cpp_path, rust_path, startup_rng_phase_trace, requested_map_token=requested_map_token)


def _compare_fire_texture_traces(cpp_path: Path, rust_path: Path) -> dict[str, object]:
    return _compare_trace_paths(cpp_path, rust_path, fire_texture_trace)


def _rng_seed_metadata_for_run(run: dict[str, object]) -> object:
    trace = run.get("rng_trace")
    if isinstance(trace, dict) and trace.get("status") == "valid":
        metadata = trace.get("metadata")
        if isinstance(metadata, dict):
            return {key: metadata.get(key) for key in ("seed", "start_reason")}
    return run.get("rng_seed")


def _run_fidelity_side(
    *, name: str, executable: Path, data_root: Path, selected_map: str,
    map_token: str, renderer: str, ticks: int, fixed_dt: float,
    input_script: Path | None, replay: ReplayScenario | None,
    timeout_seconds: float, artifact_root: Path, checkpoint_manifest: Path,
    start_gate: threading.Barrier, rng_trace_path: Path | None,
    shadow_update_trace_path: Path | None, rng_trace_limit: int | None = None,
    shadow_admission_trace_path: Path | None = None,
    actor_transition_ledger_path: Path | None = None,
    creature_generator_trace_path: Path | None = None,
    global_tick_trace_path: Path | None = None,
    startup_rng_phase_trace_path: Path | None = None,
    fire_texture_trace_path: Path | None = None,
    actor_slot_dump_path: Path | None = None,
    rng_seed: int | None = None,
) -> tuple[dict[str, object], dict[str, dict[str, object]], dict[str, object] | None]:
    repo_root = _repo_root()
    side_root = artifact_root / name
    side_root.mkdir(parents=True, exist_ok=True)
    report_path = side_root / "fidelity-state.json"
    log_path = side_root / "stdout-stderr.log"
    command = build_launch_command(
        executable, data_root, None if replay else map_token, renderer, ticks,
        fixed_dt=None if replay else fixed_dt,
        input_script=None if replay else input_script,
        replay_stem=replay.stem if replay else None,
        rng_seed=rng_seed if name == "rust" else None,
    )
    displayed = [
        _display_path(executable, repo_root),
        f"-datadir={_display_path(data_root, repo_root)}", command[2], *command[3:],
    ]
    replay_user_directory: Path | None = None
    def prepare_home(home: Path) -> None:
        nonlocal replay_user_directory
        if replay is not None:
            replay_user_directory = _stage_replay_home(home, data_root, replay)
    extra_env = {
        "HP2_ARTIFACT_DIR": str(side_root),
        "HP2_FIDELITY_REPORT": str(report_path),
        "HP2_FIDELITY_CHECKPOINTS": str(checkpoint_manifest),
    }
    requested = (
        ("HP2_RNG_TRACE", rng_trace_path),
        ("HP2_SHADOW_UPDATE_TRACE", shadow_update_trace_path),
        ("HP2_SHADOW_ADMISSION_TRACE", shadow_admission_trace_path),
        ("HP2_ACTOR_TRANSITION_LEDGER", actor_transition_ledger_path),
        ("HP2_CREATURE_GENERATOR_TRACE", creature_generator_trace_path),
        ("HP2_GLOBAL_TICK_TRACE", global_tick_trace_path),
        ("HP2_STARTUP_RNG_PHASE_TRACE", startup_rng_phase_trace_path),
        ("HP2_FIRE_TEXTURE_TRACE", fire_texture_trace_path),
        ("HP2_ACTOR_SLOT_DUMP", actor_slot_dump_path),
    )
    for key, path in requested:
        if path is not None:
            extra_env[key] = str(path)
    if rng_trace_path is not None and rng_trace_limit is not None:
        extra_env["HP2_RNG_TRACE_LIMIT"] = str(rng_trace_limit)
    if rng_seed is not None and name == "cpp":
        extra_env["HP2_RNG_SEED"] = str(rng_seed)
    result = run_command(
        command, repo_root=repo_root, timeout_seconds=timeout_seconds,
        log_path=log_path, displayed_command=displayed, extra_env=extra_env,
        start_gate=start_gate, home_parent=side_root, prepare_home=prepare_home,
    )
    resources = result.get("resources")
    deferral_violation = _script_deferral_violation(
        resources if isinstance(resources, dict) else {}
    )
    result["script_deferral_violation"] = deferral_violation
    if deferral_violation is not None:
        result["passed"] = False
    result.update({"map": selected_map, "map_token": map_token, "fidelity_state": _display_path(report_path, repo_root)})
    if rng_seed is not None:
        result["rng_seed"] = {"seed": _effective_rng_seed(rng_seed), "start_reason": "diagnostic"}
    if replay is not None:
        result["replay"] = {**replay.metadata, "user_directory": _display_path(replay_user_directory, repo_root) if replay_user_directory else None}
        if name == "rust":
            _apply_rust_replay_marker(result, log_path, replay)
    observations = (
        ("rng_trace", rng_trace_path, _rng_trace_observation),
        ("shadow_update_trace", shadow_update_trace_path, _shadow_update_trace_observation),
        ("shadow_admission_trace", shadow_admission_trace_path, _shadow_admission_trace_observation),
        ("actor_transition_ledger", actor_transition_ledger_path, _actor_transition_ledger_observation),
        ("creature_generator_trace", creature_generator_trace_path, _creature_generator_trace_observation),
        ("global_tick_trace", global_tick_trace_path, _global_tick_trace_observation),
        ("startup_rng_phase_trace", startup_rng_phase_trace_path, _startup_rng_phase_trace_observation),
        ("fire_texture_trace", fire_texture_trace_path, _fire_texture_trace_observation),
        ("actor_slot_dump", actor_slot_dump_path, _actor_slot_dump_observation),
    )
    for key, path, observe in observations:
        if path is None:
            continue
        observation = observe(path, repo_root)
        result[key] = observation
        if observation["status"] != "valid":
            result["passed"] = False
    _write_atomic(side_root / "process.json", (
        json.dumps(result, ensure_ascii=True, allow_nan=False, sort_keys=True, indent=2) + "\n"
    ).encode())
    states, diagnostic = _read_fidelity_state(report_path)
    return result, states, diagnostic


def run_paired_game(
    *, app: Path, engine_bin: Path, data_root: Path, selected_map: str,
    renderer: str, ticks: int, fixed_dt: float, timeout_seconds: float,
    output: Path, input_script: Path | None = None,
    replay_fixture: Path | None = None, replay_url: str | None = None,
    checkpoints: str = "authored", rng_trace_dir: Path | None = None,
    rng_trace_limit: int | None = None, rng_seed: int | None = None,
    shadow_update_trace_dir: Path | None = None,
    shadow_admission_trace_dir: Path | None = None,
    actor_transition_ledger_dir: Path | None = None,
    creature_generator_trace_dir: Path | None = None,
    global_tick_trace_dir: Path | None = None,
    startup_rng_phase_trace_dir: Path | None = None,
    fire_texture_trace_dir: Path | None = None,
    actor_slot_dump_dir: Path | None = None,
) -> dict[str, object]:
    repo_root = _repo_root()
    app = _resolve_path(app, repo_root, strict=True)
    engine_bin = _resolve_path(engine_bin, repo_root, strict=True)
    data_root = _resolve_path(data_root, repo_root, strict=True)
    output = _resolve_path(output, repo_root, strict=False)
    _validate_data_root(data_root)
    _positive_int(ticks)
    _positive_float(fixed_dt)
    _positive_float(timeout_seconds)
    directories = {
        "rng_trace": rng_trace_dir, "shadow_update_trace": shadow_update_trace_dir,
        "shadow_admission_trace": shadow_admission_trace_dir,
        "actor_transition_ledger": actor_transition_ledger_dir,
        "creature_generator_trace": creature_generator_trace_dir,
        "global_tick_trace": global_tick_trace_dir,
        "startup_rng_phase_trace": startup_rng_phase_trace_dir,
        "fire_texture_trace": fire_texture_trace_dir,
        "actor_slot_dump": actor_slot_dump_dir,
    }
    directories = {
        key: _resolve_path(value, repo_root, strict=False) if value is not None else None
        for key, value in directories.items()
    }
    if input_script is not None:
        input_script = _resolve_path(input_script, repo_root, strict=True)
    if replay_fixture is not None:
        replay_fixture = _resolve_path(replay_fixture, repo_root, strict=True)
    if (replay_fixture is None) != (replay_url is None):
        raise GameTestError("paired replay requires both a fixture and URL")
    if replay_fixture is not None and input_script is not None:
        raise GameTestError("paired replay cannot be combined with --input-script")
    if rng_trace_limit is not None:
        if directories["rng_trace"] is None:
            raise GameTestError("--rng-trace-limit requires --rng-trace-dir")
        rng_trace_limit = _rng_trace_limit_value(rng_trace_limit)
    effective_rng_trace_limit = RNG_TRACE_DEFAULT_LIMIT if rng_trace_limit is None else rng_trace_limit
    if rng_seed is not None:
        rng_seed = _rng_seed_value(rng_seed)
        if replay_fixture is not None:
            raise GameTestError("--rng-seed is available only for map paired runs; -REPLAY retains its fixed seed")
    if checkpoints != "authored":
        raise GameTestError(f"unsupported checkpoint source: {checkpoints}")
    map_path, map_token = _resolve_map(data_root, selected_map)
    derived = _source_checkpoints(data_root, map_path)
    artifact_root = output.parent / f"{output.stem}-artifacts"
    artifact_root.mkdir(parents=True, exist_ok=True)
    prepare_functions = {
        "rng_trace": _prepare_rng_trace_paths,
        "shadow_update_trace": _prepare_shadow_update_trace_paths,
        "shadow_admission_trace": _prepare_shadow_admission_trace_paths,
        "actor_transition_ledger": _prepare_actor_transition_ledger_paths,
        "creature_generator_trace": _prepare_creature_generator_trace_paths,
        "global_tick_trace": _prepare_global_tick_trace_paths,
        "startup_rng_phase_trace": _prepare_startup_rng_phase_trace_paths,
        "fire_texture_trace": _prepare_fire_texture_trace_paths,
        "actor_slot_dump": _prepare_actor_slot_dump_paths,
    }
    paths = {
        key: prepare_functions[key](directory) if directory is not None else None
        for key, directory in directories.items()
    }
    replay = _compile_replay_scenario(replay_fixture, replay_url or "", artifact_root, repo_root) if replay_fixture else None
    cpp_executable = _bundle_executable(app)
    _validate_native_arm64(cpp_executable)
    _validate_native_arm64(engine_bin)
    checkpoint_manifest = artifact_root / "checkpoints.json"
    _write_atomic(checkpoint_manifest, (
        json.dumps({"version": FIDELITY_FORMAT_VERSION, "checkpoints": derived}, ensure_ascii=True, allow_nan=False, sort_keys=True, indent=2) + "\n"
    ).encode())
    start_gate = threading.Barrier(2)
    common = {
        "data_root": data_root, "selected_map": selected_map, "map_token": map_token,
        "renderer": renderer, "ticks": ticks, "fixed_dt": fixed_dt,
        "input_script": input_script, "replay": replay,
        "timeout_seconds": timeout_seconds, "artifact_root": artifact_root,
        "checkpoint_manifest": checkpoint_manifest, "start_gate": start_gate,
        "rng_trace_limit": rng_trace_limit, "rng_seed": rng_seed,
    }
    path_arguments = {
        "rng_trace_path": "rng_trace", "shadow_update_trace_path": "shadow_update_trace",
        "shadow_admission_trace_path": "shadow_admission_trace",
        "actor_transition_ledger_path": "actor_transition_ledger",
        "creature_generator_trace_path": "creature_generator_trace",
        "global_tick_trace_path": "global_tick_trace",
        "startup_rng_phase_trace_path": "startup_rng_phase_trace",
        "fire_texture_trace_path": "fire_texture_trace",
        "actor_slot_dump_path": "actor_slot_dump",
    }
    with ThreadPoolExecutor(max_workers=2, thread_name_prefix="hp2-paired") as executor:
        futures = {}
        for name, executable in (("cpp", cpp_executable), ("rust", engine_bin)):
            side_paths = {
                argument: paths[key][name] if paths[key] is not None else None
                for argument, key in path_arguments.items()
            }
            futures[name] = executor.submit(_run_fidelity_side, name=name, executable=executable, **common, **side_paths)
        cpp_run, cpp_states, cpp_diagnostic = futures["cpp"].result()
        rust_run, rust_states, rust_diagnostic = futures["rust"].result()
    profile = _profile_for_data_root(data_root, repo_root)
    records = []
    for checkpoint in derived:
        checkpoint_id = checkpoint["id"]
        cpp_state = cpp_states.get(checkpoint_id)
        rust_state = rust_states.get(checkpoint_id)
        records.append({
            "profile": profile, "checkpoint": checkpoint,
            "cpp": _side_checkpoint(cpp_state, cpp_diagnostic),
            "rust": _side_checkpoint(rust_state, rust_diagnostic),
            "comparison": _compare_checkpoint(checkpoint, cpp_state, rust_state, cpp_diagnostic, rust_diagnostic),
        })
    report: dict[str, object] = {
        "version": FIDELITY_FORMAT_VERSION, "runs": {"cpp": cpp_run, "rust": rust_run},
        "reports": records,
    }
    if rng_seed is not None:
        expected = {"seed": _effective_rng_seed(rng_seed), "start_reason": "diagnostic"}
        cpp_seed, rust_seed = _rng_seed_metadata_for_run(cpp_run), _rng_seed_metadata_for_run(rust_run)
        report["rng_seed"] = {
            "requested_seed": rng_seed, "expected": expected, "cpp": cpp_seed, "rust": rust_seed,
            "status": "match" if cpp_seed == rust_seed == expected else "mismatch",
        }
    if directories["rng_trace"] is not None:
        report["rng_trace"] = {
            "directory": _display_path(directories["rng_trace"], repo_root),
            "requested_limit": effective_rng_trace_limit,
            "cpp": cpp_run["rng_trace"], "rust": rust_run["rng_trace"],
        }
    comparison_specs = {
        "shadow_update_trace": (_compare_shadow_update_traces, {"map_stem": map_path.stem}, "shadow-update-trace-comparison.json"),
        "shadow_admission_trace": (_compare_shadow_admission_traces, {"map_stem": map_path.stem}, "shadow-admission-trace-comparison.json"),
        "actor_transition_ledger": (_compare_actor_transition_ledgers, {}, "actor-transition-ledger-comparison.json"),
        "creature_generator_trace": (_compare_creature_generator_traces, {}, "creature-generator-trace-comparison.json"),
        "global_tick_trace": (_compare_global_tick_traces, {}, "global-tick-trace-comparison.json"),
        "startup_rng_phase_trace": (_compare_startup_rng_phase_traces, {"requested_map_token": map_path.stem}, "startup-rng-phase-trace-comparison.json"),
        "fire_texture_trace": (_compare_fire_texture_traces, {}, "fire-texture-trace-comparison.json"),
    }
    for key, (compare, kwargs, filename) in comparison_specs.items():
        directory, pair_paths = directories[key], paths[key]
        if directory is None or pair_paths is None:
            continue
        comparison = compare(pair_paths["cpp"], pair_paths["rust"], **kwargs)
        comparison_path = directory / filename
        _write_atomic(comparison_path, (json.dumps(comparison, ensure_ascii=True, allow_nan=False, sort_keys=True, indent=2) + "\n").encode())
        report[key] = {
            "directory": _display_path(directory, repo_root),
            "cpp": cpp_run[key], "rust": rust_run[key], "comparison": comparison,
            "comparison_report": _display_path(comparison_path, repo_root),
        }
    if directories["actor_slot_dump"] is not None:
        pair_paths = paths["actor_slot_dump"]
        assert pair_paths is not None
        report["actor_slot_dump"] = {
            "directory": str(directories["actor_slot_dump"]),
            "cpp": cpp_run["actor_slot_dump"], "rust": rust_run["actor_slot_dump"],
        }
        report["actor_slot_semantic_join"] = _actor_slot_semantic_join_diagnostic(pair_paths["cpp"], pair_paths["rust"])
    if replay is not None:
        report["replay"] = replay.metadata
    comparisons_match = all(
        report[key]["comparison"]["status"] == "match"
        for key in comparison_specs if key in report
    )
    passed = bool(cpp_run.get("passed")) and bool(rust_run.get("passed")) and all(
        record["comparison"]["status"] == "match" for record in records
    ) and comparisons_match
    if "rng_seed" in report:
        passed = passed and report["rng_seed"]["status"] == "match"
    if "actor_slot_semantic_join" in report:
        passed = passed and report["actor_slot_semantic_join"].get("matches") is True
    report["passed"] = passed
    _write_atomic(output, (
        json.dumps(report, ensure_ascii=True, allow_nan=False, sort_keys=True, indent=2) + "\n"
    ).encode())
    return report
def _argument_positive_int(value: str) -> int:
    try:
        return _positive_int(int(value, 10))
    except ValueError as error:
        raise argparse.ArgumentTypeError("must be an integer") from error
    except GameTestError as error:
        raise argparse.ArgumentTypeError(str(error)) from error


def _argument_positive_float(value: str) -> float:
    try:
        return _positive_float(float(value))
    except ValueError as error:
        raise argparse.ArgumentTypeError("must be a number") from error
    except GameTestError as error:
        raise argparse.ArgumentTypeError(str(error)) from error

def _argument_rng_seed(value: str) -> int:
    if not value or not value.isascii() or not value.isdecimal():
        raise argparse.ArgumentTypeError("must be an ASCII decimal integer")
    try:
        return _rng_seed_value(int(value, 10))
    except GameTestError as error:
        raise argparse.ArgumentTypeError(str(error)) from error


def _argument_rng_trace_limit(value: str) -> int:
    if not value or not value.isascii() or not value.isdecimal():
        raise argparse.ArgumentTypeError("must be an ASCII decimal integer")
    try:
        return _rng_trace_limit_value(int(value, 10))
    except GameTestError as error:
        raise argparse.ArgumentTypeError(str(error)) from error


def _arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    subcommands = parser.add_subparsers(dest="subcommand", required=True)
    maps = subcommands.add_parser("maps", help="emit listed map selectors as JSON")
    maps.add_argument("--data-root", required=True, type=Path)
    run = subcommands.add_parser("run", help="launch one exact listed map")
    run.add_argument("--app", required=True, type=Path)
    run.add_argument("--data-root", required=True, type=Path)
    run.add_argument("--map", dest="selected_map", required=True)
    run.add_argument("--renderer", required=True, choices=("xopengl", "vulkan"))
    run.add_argument("--ticks", required=True, type=_argument_positive_int)
    run.add_argument("--timeout", required=True, type=_argument_positive_float)
    run.add_argument("--log", required=True, type=Path)
    run.add_argument("--no-sound", action="store_true")
    run.add_argument("--engine-bin", type=Path, default=None)
    paired = subcommands.add_parser("paired", help="launch the C++ oracle and hp2rs with identical inputs")
    paired.add_argument("--app", required=True, type=Path)
    paired.add_argument("--engine-bin", required=True, type=Path)
    paired.add_argument("--data-root", required=True, type=Path)
    paired.add_argument("--map", dest="selected_map", required=True)
    paired.add_argument("--renderer", required=True, choices=("xopengl", "vulkan"))
    paired.add_argument("--ticks", required=True, type=_argument_positive_int)
    paired.add_argument("--fixed-dt", required=True, type=_argument_positive_float)
    paired.add_argument("--input-script", type=Path, default=None)
    paired.add_argument("--replay-fixture", type=Path, default=None)
    paired.add_argument("--replay-url", default=None)
    paired.add_argument("--rng-trace-dir", type=Path, default=None)
    paired.add_argument("--rng-trace-limit", type=_argument_rng_trace_limit, default=None)
    paired.add_argument("--rng-seed", type=_argument_rng_seed, default=None)
    paired.add_argument("--shadow-update-trace-dir", type=Path, default=None)
    paired.add_argument("--shadow-admission-trace-dir", type=Path, default=None)
    paired.add_argument("--actor-transition-ledger-dir", type=Path, default=None)
    paired.add_argument("--creature-generator-trace-dir", type=Path, default=None)
    paired.add_argument("--global-tick-trace-dir", type=Path, default=None)
    paired.add_argument("--startup-rng-phase-trace-dir", type=Path, default=None)
    paired.add_argument("--fire-texture-trace-dir", type=Path, default=None)
    paired.add_argument("--actor-slot-dump-dir", type=Path, default=None)
    paired.add_argument("--checkpoints", required=True, choices=("authored",))
    paired.add_argument("--timeout", type=_argument_positive_float, default=120.0)
    paired.add_argument("--output", required=True, type=Path)
    return parser.parse_args()


def main() -> int:
    arguments = _arguments()
    try:
        repo_root = _repo_root()
        if arguments.subcommand == "maps":
            data_root = _resolve_path(arguments.data_root, repo_root, strict=True)
            print(json.dumps({"version": 1, "maps": list_maps(data_root)}, ensure_ascii=True, allow_nan=False))
            return 0
        if arguments.subcommand == "paired":
            result = run_paired_game(
                app=arguments.app, engine_bin=arguments.engine_bin,
                data_root=arguments.data_root, selected_map=arguments.selected_map,
                renderer=arguments.renderer, ticks=arguments.ticks,
                fixed_dt=arguments.fixed_dt, timeout_seconds=arguments.timeout,
                output=arguments.output, input_script=arguments.input_script,
                replay_fixture=arguments.replay_fixture, replay_url=arguments.replay_url,
                checkpoints=arguments.checkpoints, rng_trace_dir=arguments.rng_trace_dir,
                rng_trace_limit=arguments.rng_trace_limit, rng_seed=arguments.rng_seed,
                shadow_update_trace_dir=arguments.shadow_update_trace_dir,
                shadow_admission_trace_dir=arguments.shadow_admission_trace_dir,
                actor_transition_ledger_dir=arguments.actor_transition_ledger_dir,
                creature_generator_trace_dir=arguments.creature_generator_trace_dir,
                global_tick_trace_dir=arguments.global_tick_trace_dir,
                startup_rng_phase_trace_dir=arguments.startup_rng_phase_trace_dir,
                fire_texture_trace_dir=arguments.fire_texture_trace_dir,
                actor_slot_dump_dir=arguments.actor_slot_dump_dir,
            )
        else:
            result = run_game(
                app=arguments.app, data_root=arguments.data_root,
                selected_map=arguments.selected_map, renderer=arguments.renderer,
                ticks=arguments.ticks, timeout_seconds=arguments.timeout,
                log_path=arguments.log, no_sound=arguments.no_sound,
                engine_bin=arguments.engine_bin,
            )
        print(json.dumps(result, ensure_ascii=True, allow_nan=False, sort_keys=True))
        return 0 if result["passed"] else 1
    except GameTestError as error:
        print(f"game_test.py: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
