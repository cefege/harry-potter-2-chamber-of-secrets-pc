#!/usr/bin/env python3
"""Repair one HP2 save through the native Unreal loader and serializer."""

from __future__ import annotations

import argparse
from dataclasses import dataclass
from datetime import datetime, timezone
import os
from pathlib import Path
import re
import shutil
import stat
import subprocess
import sys
import tempfile
from typing import NoReturn

USER_SUFFIX = Path("Library/Application Support/Harry Potter 2/User")
SAVE_NAME = re.compile(r"Save(0|[1-9][0-9]*)\.usa")
DEFAULT_TIMEOUT_SECONDS = 120.0
COPY_BUFFER_SIZE = 1024 * 1024


class RepairError(RuntimeError):
    """The save could not be repaired without risking the original."""


@dataclass(frozen=True)
class RepairResult:
    target: Path
    backup: Path


def error(message: str) -> NoReturn:
    raise RepairError(message)


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--engine", required=True, type=Path, help="hp2_game executable")
    parser.add_argument(
        "--data-root", required=True, type=Path,
        help="retail or prototype Unreal data root containing System/Default.ini",
    )
    parser.add_argument(
        "--user-root", required=True, type=Path,
        help="active profile/user root containing Game.ini, User.ini, and Save/",
    )
    parser.add_argument("target", type=Path, help="existing SaveN.usa inside USER_ROOT/Save")
    parser.add_argument(
        "--timeout-seconds", type=float, default=DEFAULT_TIMEOUT_SECONDS,
        help=f"engine timeout in seconds (default: {DEFAULT_TIMEOUT_SECONDS:g})",
    )
    return parser.parse_args(argv)


def _regular_file(path: Path, description: str, *, executable: bool = False) -> Path:
    expanded = path.expanduser().absolute()
    try:
        if expanded.is_symlink():
            error(f"{description} must not be a symbolic link: {expanded}")
        resolved = expanded.resolve(strict=True)
        mode = resolved.stat().st_mode
    except OSError as exc:
        error(f"cannot access {description} {expanded}: {exc}")
    if not stat.S_ISREG(mode):
        error(f"{description} is not a regular file: {resolved}")
    if not os.access(resolved, os.R_OK):
        error(f"{description} is not readable: {resolved}")
    if executable and not os.access(resolved, os.X_OK):
        error(f"{description} is not executable: {resolved}")
    return resolved


def _directory(path: Path, description: str) -> Path:
    expanded = path.expanduser().absolute()
    try:
        if expanded.is_symlink():
            error(f"{description} must not be a symbolic link: {expanded}")
        resolved = expanded.resolve(strict=True)
        mode = resolved.stat().st_mode
    except OSError as exc:
        error(f"cannot access {description} {expanded}: {exc}")
    if not stat.S_ISDIR(mode):
        error(f"{description} is not a directory: {resolved}")
    return resolved


def validate_inputs(
    engine: Path, data_root: Path, user_root: Path, target: Path, timeout_seconds: float,
) -> tuple[Path, Path, Path, Path, int]:
    if timeout_seconds <= 0 or not timeout_seconds < float("inf"):
        error("--timeout-seconds must be a positive finite number")

    validated_engine = _regular_file(engine, "engine executable", executable=True)
    validated_data = _directory(data_root, "data root")
    for relative in (Path("System/Default.ini"), Path("System/DefUser.ini")):
        _regular_file(validated_data / relative, f"data file {relative}")

    validated_user = _directory(user_root, "user root")
    save_directory = _directory(validated_user / "Save", "user Save directory")

    expanded_target = target.expanduser().absolute()
    match = SAVE_NAME.fullmatch(expanded_target.name)
    if match is None:
        error(f"target must be named exactly SaveN.usa: {expanded_target}")
    validated_target = _regular_file(expanded_target, "target save")
    if validated_target.parent != save_directory:
        error(f"target must be directly inside {save_directory}: {validated_target}")
    if validated_target.stat().st_size == 0:
        error(f"target save is empty: {validated_target}")
    return validated_engine, validated_data, validated_user, validated_target, int(match.group(1))


def _copy_regular(source: Path, destination: Path) -> None:
    if source.is_symlink():
        error(f"profile contains a symbolic link, which cannot be isolated safely: {source}")
    try:
        mode = source.stat().st_mode
    except OSError as exc:
        error(f"cannot inspect profile entry {source}: {exc}")
    if not stat.S_ISREG(mode):
        error(f"profile contains a non-regular file: {source}")
    destination.parent.mkdir(parents=True, exist_ok=True)
    try:
        shutil.copy2(source, destination)
    except OSError as exc:
        error(f"cannot copy profile entry {source}: {exc}")


def _copy_tree(source: Path, destination: Path) -> None:
    if source.is_symlink():
        error(f"profile contains a symbolic link, which cannot be isolated safely: {source}")
    destination.mkdir(parents=True, exist_ok=False)
    try:
        entries = sorted(source.iterdir(), key=lambda item: os.fsencode(item.name))
    except OSError as exc:
        error(f"cannot read profile directory {source}: {exc}")
    for entry in entries:
        if entry.is_symlink():
            error(f"profile contains a symbolic link, which cannot be isolated safely: {entry}")
        try:
            mode = entry.stat().st_mode
        except OSError as exc:
            error(f"cannot inspect profile entry {entry}: {exc}")
        destination_entry = destination / entry.name
        if stat.S_ISDIR(mode):
            _copy_tree(entry, destination_entry)
        elif stat.S_ISREG(mode):
            _copy_regular(entry, destination_entry)
        else:
            error(f"profile contains a non-regular file: {entry}")


def stage_profile(user_root: Path, isolated_home: Path) -> Path:
    staged_user = isolated_home / USER_SUFFIX
    staged_user.parent.mkdir(parents=True)
    staged_user.mkdir(mode=0o700)
    for name in ("Game.ini", "User.ini"):
        source = user_root / name
        if source.exists() or source.is_symlink():
            _copy_regular(source, staged_user / name)
    _copy_tree(user_root / "Save", staged_user / "Save")
    (staged_user / "Save" / "cache").mkdir(mode=0o700, exist_ok=True)
    (staged_user / "Cache").mkdir(mode=0o700)
    return staged_user


def isolated_environment(home: Path) -> dict[str, str]:
    temporary = home / "tmp"
    config = home / ".config"
    cache = home / ".cache"
    for directory in (temporary, config, cache):
        directory.mkdir()
    environment = os.environ.copy()
    environment.update({
        "HOME": str(home), "CFFIXED_USER_HOME": str(home),
        "TMPDIR": str(temporary) + os.sep, "XDG_CONFIG_HOME": str(config),
        "XDG_CACHE_HOME": str(cache),
    })
    return environment


def decode_log(payload: bytes) -> str:
    if payload.startswith((b"\xff\xfe", b"\xfe\xff")):
        try:
            return payload.decode("utf-16")
        except UnicodeDecodeError as exc:
            error(f"engine log has invalid UTF-16 encoding: {exc}")
    try:
        return payload.decode("utf-8-sig")
    except UnicodeDecodeError:
        if len(payload) % 2 == 0:
            for encoding in ("utf-16-le", "utf-16-be"):
                try:
                    return payload.decode(encoding)
                except UnicodeDecodeError:
                    pass
    error("engine log is neither valid UTF-8 nor UTF-16")


def _marker_present(log_text: str, slot: int) -> bool:
    marker = f"Save repair completed: {slot}"
    for line in log_text.splitlines():
        stripped = line.strip()
        if stripped == marker or stripped.endswith(f": {marker}"):
            return True
    return False


def _diagnostic_tail(text: str, line_count: int = 40) -> str:
    lines = text.splitlines()
    return "\n".join(lines[-line_count:]) if lines else "<empty>"


def run_engine(
    engine: Path, data_root: Path, staged_user: Path, isolated_home: Path,
    slot: int, timeout_seconds: float,
) -> None:
    log_path = staged_user / "repair-save.log"
    command = [
        str(engine), f"-datadir={data_root}", f"-REPAIRSAVE={slot}",
        "-windowed", "-NOFRONTEND", f"-ABSLOG={log_path}",
    ]
    try:
        completed = subprocess.run(
            command, cwd=data_root / "System", env=isolated_environment(isolated_home),
            stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
            timeout=timeout_seconds, check=False,
        )
    except subprocess.TimeoutExpired as exc:
        output = (exc.stdout or b"").decode("utf-8", errors="replace")
        error(
            f"engine timed out after {timeout_seconds:g} seconds; process output tail:\n"
            f"{_diagnostic_tail(output)}"
        )
    except OSError as exc:
        error(f"cannot launch engine {engine}: {exc}")

    process_output = completed.stdout.decode("utf-8", errors="replace")
    if not log_path.is_file() or log_path.is_symlink():
        error(
            f"engine did not generate the expected log {log_path}; exit status {completed.returncode}; "
            f"process output tail:\n{_diagnostic_tail(process_output)}"
        )
    try:
        log_text = decode_log(log_path.read_bytes())
    except OSError as exc:
        error(f"cannot read engine log {log_path}: {exc}")
    log_tail = _diagnostic_tail(log_text)
    if completed.returncode != 0:
        error(
            f"engine exited with status {completed.returncode}; engine log tail:\n{log_tail}\n"
            f"process output tail:\n{_diagnostic_tail(process_output)}"
        )
    if not _marker_present(log_text, slot):
        error(
            f"engine log is missing exact success marker 'Save repair completed: {slot}'; "
            f"engine log tail:\n{log_tail}\nprocess output tail:\n{_diagnostic_tail(process_output)}"
        )


def _backup_path(target: Path) -> tuple[Path, int]:
    stamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%S.%fZ")
    for suffix in range(1000):
        discriminator = "" if suffix == 0 else f"-{suffix}"
        candidate = target.with_name(f"{target.name}.backup-{stamp}{discriminator}")
        try:
            descriptor = os.open(candidate, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
            return candidate, descriptor
        except FileExistsError:
            continue
        except OSError as exc:
            error(f"cannot create backup {candidate}: {exc}")
    error(f"cannot choose a non-clobbering backup name beside {target}")


def create_backup(target: Path) -> Path:
    backup, descriptor = _backup_path(target)
    try:
        with target.open("rb") as source, os.fdopen(descriptor, "wb") as destination:
            shutil.copyfileobj(source, destination, length=COPY_BUFFER_SIZE)
            destination.flush()
            os.fsync(destination.fileno())
            os.fchmod(destination.fileno(), stat.S_IMODE(target.stat().st_mode))
    except Exception as exc:
        try:
            backup.unlink(missing_ok=True)
        except OSError:
            pass
        error(f"cannot complete backup {backup}: {exc}")
    return backup


def atomic_replace(source: Path, target: Path, backup: Path) -> None:
    descriptor, temporary_name = tempfile.mkstemp(prefix=f".{target.name}.repair-", dir=target.parent)
    temporary = Path(temporary_name)
    try:
        with source.open("rb") as repaired, os.fdopen(descriptor, "wb") as destination:
            shutil.copyfileobj(repaired, destination, length=COPY_BUFFER_SIZE)
            destination.flush()
            os.fsync(destination.fileno())
            os.fchmod(destination.fileno(), stat.S_IMODE(target.stat().st_mode))
        os.replace(temporary, target)
    except Exception as exc:
        try:
            os.close(descriptor)
        except OSError:
            pass
        try:
            temporary.unlink(missing_ok=True)
        except OSError:
            pass
        error(f"cannot atomically replace {target}; original remains in place and backup is {backup}: {exc}")


def repair_save(
    engine: Path, data_root: Path, user_root: Path, target: Path, *,
    timeout_seconds: float = DEFAULT_TIMEOUT_SECONDS,
) -> RepairResult:
    engine, data_root, user_root, target, slot = validate_inputs(
        engine, data_root, user_root, target, timeout_seconds
    )
    with tempfile.TemporaryDirectory(prefix="hp2-save-repair-") as isolated_home_name:
        isolated_home = Path(isolated_home_name)
        staged_user = stage_profile(user_root, isolated_home)
        staged_target = staged_user / "Save" / target.name
        sentinel_mtime = 1_000_000_000
        try:
            os.utime(staged_target, ns=(staged_target.stat().st_atime_ns, sentinel_mtime))
            staged_mtime = staged_target.stat().st_mtime_ns
        except OSError as exc:
            error(f"cannot prepare isolated target {staged_target}: {exc}")

        run_engine(engine, data_root, staged_user, isolated_home, slot, timeout_seconds)
        try:
            if staged_target.is_symlink() or not staged_target.is_file():
                error(f"engine did not produce repaired target {staged_target}")
            staged_stat = staged_target.stat()
        except OSError as exc:
            error(f"cannot inspect repaired target {staged_target}: {exc}")
        if staged_stat.st_size == 0:
            error(f"engine produced an empty repaired target {staged_target}")
        if staged_stat.st_mtime_ns == staged_mtime:
            error(f"engine reported success but did not rewrite {staged_target}")

        backup = create_backup(target)
        atomic_replace(staged_target, target, backup)
    return RepairResult(target=target, backup=backup)


def run(args: argparse.Namespace) -> RepairResult:
    return repair_save(
        args.engine, args.data_root, args.user_root, args.target,
        timeout_seconds=args.timeout_seconds,
    )


def main(argv: list[str] | None = None) -> int:
    try:
        result = run(parse_args(argv))
        print(f"Backup: {result.backup}")
        print(f"Repaired save: {result.target}")
        return 0
    except RepairError as exc:
        print(f"repair_save.py: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
