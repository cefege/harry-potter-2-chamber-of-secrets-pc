#!/usr/bin/env python3
"""List and launch HP2 maps through the direct engine bootstrap for tests."""

from __future__ import annotations

import argparse
import json
import math
import os
from pathlib import Path
import plistlib
import re
import signal
import stat
import struct
import subprocess
import sys
import tempfile
import time
from typing import BinaryIO, Iterator, Sequence

ARM64_CPU_TYPE = 0x0100000C
TERMINATION_GRACE_SECONDS = 5.0
KILL_GRACE_SECONDS = 5.0

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


def build_launch_command(
    executable: Path,
    data_root: Path,
    map_token: str,
    renderer: str,
    ticks: int,
    *,
    no_sound: bool = False,
) -> list[str]:
    if renderer not in ("xopengl", "vulkan"):
        raise GameTestError(f"unsupported renderer: {renderer}")
    _positive_int(ticks)
    arguments = [
        str(executable),
        f"-datadir={data_root}",
        map_token,
        "-xopengl" if renderer == "xopengl" else "-vulkan",
        "-NOFRONTEND",
        "-window",
    ]
    if no_sound:
        arguments.append("-nosound")
    arguments.extend((f"-testticks={ticks}", "-log"))
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
    for directory in (temporary, config, cache):
        directory.mkdir(parents=True, exist_ok=True)
    environment = os.environ.copy()
    environment.update({
        "HOME": str(home), "CFFIXED_USER_HOME": str(home),
        "TMPDIR": str(temporary) + os.sep, "XDG_CONFIG_HOME": str(config),
        "XDG_CACHE_HOME": str(cache), "LANG": "C", "LC_ALL": "C", "TZ": "UTC",
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
    command: Sequence[str],
    *,
    repo_root: Path,
    timeout_seconds: float,
    log_path: Path,
    displayed_command: Sequence[str] | None = None,
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
    with tempfile.TemporaryDirectory(prefix="hp2-game-test-") as home_name:
        isolated_home = Path(home_name)
        process: subprocess.Popen[bytes] | None = None
        try:
            process = subprocess.Popen(
                command, cwd=repo_root, env=_isolated_environment(isolated_home),
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
        "captured_output_bytes": len(output), "duration_seconds": round(time.monotonic() - started, 6),
        "exit_status": exit_status, "timed_out": timed_out, "launch_error": launch_error,
        "failure_markers": markers, "orphaned_process_group": orphaned_process_group,
        "process_group_cleanup": cleanup_action, "cleanup_error": cleanup_error,
    }
    result["passed"] = _passed(result)
    return result


def run_game(
    *, app: Path, data_root: Path, selected_map: str, renderer: str, ticks: int,
    timeout_seconds: float, log_path: Path, no_sound: bool = False,
) -> dict[str, object]:
    """Validate a real app, then launch one exact listed map."""
    repo_root = _repo_root()
    app = _resolve_path(app, repo_root, strict=True)
    data_root = _resolve_path(data_root, repo_root, strict=True)
    log_path = _resolve_path(log_path, repo_root, strict=False)
    _validate_data_root(data_root)
    _positive_float(timeout_seconds)
    executable = _bundle_executable(app)
    _validate_native_arm64(executable)
    map_path, map_token = _resolve_map(data_root, selected_map)
    command = build_launch_command(executable, data_root, map_token, renderer, ticks, no_sound=no_sound)
    displayed_command = [
        _display_path(executable, repo_root), f"-datadir={_display_path(data_root, repo_root)}",
        map_token, *command[3:],
    ]
    result = run_command(command, repo_root=repo_root, timeout_seconds=timeout_seconds, log_path=log_path, displayed_command=displayed_command)
    result.update({"map": selected_map, "map_token": map_token})
    return result


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
    return parser.parse_args()


def main() -> int:
    arguments = _arguments()
    try:
        repo_root = _repo_root()
        if arguments.subcommand == "maps":
            data_root = _resolve_path(arguments.data_root, repo_root, strict=True)
            print(json.dumps({"version": 1, "maps": list_maps(data_root)}, ensure_ascii=True, allow_nan=False))
            return 0
        result = run_game(
            app=arguments.app, data_root=arguments.data_root, selected_map=arguments.selected_map,
            renderer=arguments.renderer, ticks=arguments.ticks, timeout_seconds=arguments.timeout,
            log_path=arguments.log, no_sound=arguments.no_sound,
        )
        print(json.dumps(result, ensure_ascii=True, allow_nan=False, sort_keys=True))
        return 0 if result["passed"] else 1
    except GameTestError as error:
        print(f"game_test.py: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
