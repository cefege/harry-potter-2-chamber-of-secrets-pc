#!/usr/bin/env python3
"""Generate a deterministic SHA-256 manifest for original HP2 assets."""

from __future__ import annotations

import argparse
import hashlib
import os
from pathlib import Path
import stat
import sys
import tempfile
from typing import Iterator


DEFAULT_SOURCE = Path("HarryPotter2/Unreal")
DEFAULT_OUTPUT = Path("Tests/Fixtures/original-assets.sha256")
ASSET_EXTENSIONS = frozenset(
    {".u", ".unr", ".utx", ".uax", ".ogg", ".ini", ".exe", ".dll"}
)
READ_SIZE = 1024 * 1024


class ManifestError(Exception):
    """Raised when a safe, complete manifest cannot be generated."""


def _utf8(value: str, description: str) -> bytes:
    try:
        return value.encode("utf-8")
    except UnicodeEncodeError as error:
        raise ManifestError(f"{description} is not valid UTF-8: {value!r}") from error


def _path_under_repo(
    value: Path, repo_root: Path, *, description: str, must_exist: bool
) -> Path:
    if value.is_absolute():
        try:
            relative = value.relative_to(repo_root)
        except ValueError as error:
            raise ManifestError(f"{description} is outside the repository: {value}") from error
    else:
        relative = value

    if ".." in relative.parts:
        raise ManifestError(f"{description} may not contain '..': {value}")

    candidate = repo_root.joinpath(relative)
    current = repo_root
    for component in relative.parts:
        if component in ("", "."):
            continue
        current = current / component
        try:
            mode = current.lstat().st_mode
        except FileNotFoundError:
            break
        except OSError as error:
            raise ManifestError(f"cannot inspect {description} component {current}: {error}") from error
        if stat.S_ISLNK(mode):
            raise ManifestError(f"refusing symlink in {description}: {current}")

    try:
        resolved = candidate.resolve(strict=must_exist)
    except OSError as error:
        raise ManifestError(f"cannot resolve {description} {candidate}: {error}") from error
    try:
        resolved.relative_to(repo_root)
    except ValueError as error:
        raise ManifestError(f"{description} is outside the repository: {candidate}") from error
    return resolved


def _asset_files(directory: Path) -> Iterator[Path]:
    try:
        with os.scandir(directory) as iterator:
            entries = sorted(
                iterator,
                key=lambda entry: _utf8(entry.name, "path component"),
            )
    except OSError as error:
        raise ManifestError(f"cannot read source directory {directory}: {error}") from error

    for entry in entries:
        path = Path(entry.path)
        try:
            if entry.is_symlink():
                raise ManifestError(f"refusing symlink under source directory: {path}")
            entry_stat = entry.stat(follow_symlinks=False)
        except OSError as error:
            raise ManifestError(f"cannot inspect source path {path}: {error}") from error

        if stat.S_ISDIR(entry_stat.st_mode):
            yield from _asset_files(path)
        elif stat.S_ISREG(entry_stat.st_mode) and path.suffix.casefold() in ASSET_EXTENSIONS:
            yield path


def _sha256(path: Path) -> str:
    flags = os.O_RDONLY
    if hasattr(os, "O_CLOEXEC"):
        flags |= os.O_CLOEXEC
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW

    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise ManifestError(f"cannot open asset {path}: {error}") from error

    digest = hashlib.sha256()
    try:
        before = os.fstat(descriptor)
        if not stat.S_ISREG(before.st_mode):
            raise ManifestError(f"asset is not a regular file: {path}")
        while True:
            chunk = os.read(descriptor, READ_SIZE)
            if not chunk:
                break
            digest.update(chunk)
        after = os.fstat(descriptor)
    except OSError as error:
        raise ManifestError(f"cannot hash asset {path}: {error}") from error
    finally:
        os.close(descriptor)

    identity_before = (before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns)
    identity_after = (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns)
    if identity_before != identity_after:
        raise ManifestError(f"asset changed while it was being hashed: {path}")
    return digest.hexdigest()


def _manifest(repo_root: Path, source: Path) -> bytes:
    rows: list[tuple[bytes, str]] = []
    seen: set[str] = set()

    for asset in _asset_files(source):
        try:
            resolved = asset.resolve(strict=True)
            relative = resolved.relative_to(repo_root).as_posix()
        except (OSError, ValueError) as error:
            raise ManifestError(f"asset resolves outside the repository: {asset}") from error
        if resolved != asset:
            raise ManifestError(f"refusing symlinked asset path: {asset}")
        if "\n" in relative or "\r" in relative:
            raise ManifestError(f"asset path contains a line break: {relative!r}")
        if relative in seen:
            raise ManifestError(f"duplicate asset path: {relative}")
        seen.add(relative)
        rows.append((_utf8(relative, "asset path"), _sha256(asset)))

    rows.sort(key=lambda row: row[0])
    return b"".join(
        digest.encode("ascii") + b"  " + relative + b"\n"
        for relative, digest in rows
    )


def _write_atomic(output: Path, contents: bytes) -> None:
    try:
        output.parent.mkdir(parents=True, exist_ok=True)
    except OSError as error:
        raise ManifestError(f"cannot create output directory {output.parent}: {error}") from error
    if output.is_symlink():
        raise ManifestError(f"refusing symlink output: {output}")

    temporary: Path | None = None
    try:
        with tempfile.NamedTemporaryFile(
            mode="wb", prefix=f".{output.name}.", dir=output.parent, delete=False
        ) as stream:
            temporary = Path(stream.name)
            stream.write(contents)
            stream.flush()
            os.fsync(stream.fileno())
        os.chmod(temporary, 0o644)
        os.replace(temporary, output)
    except OSError as error:
        raise ManifestError(f"cannot write manifest {output}: {error}") from error
    finally:
        if temporary is not None:
            try:
                temporary.unlink(missing_ok=True)
            except OSError:
                pass


def _arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--repo-root",
        type=Path,
        default=Path.cwd(),
        help="repository root (default: current directory)",
    )
    parser.add_argument(
        "--source",
        type=Path,
        default=DEFAULT_SOURCE,
        help=f"asset tree, relative to the repository (default: {DEFAULT_SOURCE})",
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=DEFAULT_OUTPUT,
        help=f"manifest path, relative to the repository (default: {DEFAULT_OUTPUT})",
    )
    return parser.parse_args()


def main() -> int:
    arguments = _arguments()
    try:
        raw_repo_root = arguments.repo_root.absolute()
        if raw_repo_root.is_symlink():
            raise ManifestError(f"refusing symlink repository root: {raw_repo_root}")
        repo_root = raw_repo_root.resolve(strict=True)
        if not repo_root.is_dir():
            raise ManifestError(f"repository root is not a directory: {repo_root}")

        source = _path_under_repo(
            arguments.source,
            repo_root,
            description="source path",
            must_exist=True,
        )
        if not source.is_dir():
            raise ManifestError(f"source path is not a directory: {source}")

        output = _path_under_repo(
            arguments.output,
            repo_root,
            description="output path",
            must_exist=False,
        )
        contents = _manifest(repo_root, source)
        _write_atomic(output, contents)
    except ManifestError as error:
        print(f"error: {error}", file=sys.stderr)
        return 1

    print(f"wrote {len(contents.splitlines())} asset hashes to {output.relative_to(repo_root)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
