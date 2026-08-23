#!/usr/bin/env python3
"""Scan the data-hp1 root (Harry Potter 1 US retail ISO contents) and maintain
its provenance manifest hp1-data-manifest.json.

Phase 0 of Docs/HP1_MIGRATION.md. This tool does not extract anything; the
data root is populated once from /Volumes/HARRY_POTTER_US_ (label
HARRY_POTTER_US_3010) and this script proves what is there:

- SHA-256 and byte size of every file under the canonical roots.
- Unreal package header facts (signature C1 83 2A 9E, FileVersion u16@4,
  LicenseeVersion u16@6, PackageFlags u32@8, little-endian) for every file
  that carries that signature; non-package files record nulls.

Compatibility decision recorded here (not silently applied): the ISO's
System/ ships DefUser.ini but no top-level Default.ini - the Windows installer
materializes Default.ini per language from the localized payload directories.
The genuine US English template is System/0/Default.ini (Language=int;
byte-identical to System/1/Default.ini except Language=spa). The extraction
copies it into place as System/Default.ini so HP2Paths legacy mode accepts the
root (HP2Paths.cpp expects a readable System/Default.ini) and UnMisc.cpp can
seed the game ini from it.

Out of scope by profile decision (US English only, Docs/HP1_MIGRATION.md 5):
localized payload subdirs System/{0,1}, Textures/{0,1}, Sounds/{0,1},
Help/{0,1}; localized compound-extension assets Textures/MenuArt.hun_utx,
Sounds/AllEmote.eng_uax; loose non-package audio Sounds/*.wav and Sounds/wavs/.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path, PurePosixPath
from typing import NoReturn

CHUNK_SIZE = 1024 * 1024

MANIFEST_NAME = "hp1-data-manifest.json"
MANIFEST_FORMAT_VERSION = 1
DATA_PROFILE = "data-hp1"
ISO_LABEL = "HARRY_POTTER_US_3010"

CANONICAL_ROOTS = ("System", "Maps", "Textures", "Sounds", "Music", "Help")

PACKAGE_SIGNATURE = b"\xc1\x83\x2a\x9e"

# Observed ISO inventory (top-level packages only; see module docstring for
# excluded localized variants). Regeneration fails loudly if the tree drifts.
EXPECTED_PACKAGE_COUNTS = {
    ".u": 29,
    ".unr": 41,
    ".utx": 57,
    ".umx": 91,
    ".uax": 24,
}


class ManifestError(RuntimeError):
    """The data root cannot be scanned or verified without violating its contract."""


def error(message: str) -> NoReturn:
    raise ManifestError(message)


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    default_root = Path(__file__).resolve().parent.parent / "HarryPotter1" / "Unreal"
    parser.add_argument(
        "--root",
        type=Path,
        default=default_root,
        help=f"data-hp1 root (default: {default_root})",
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="verify the existing tree against the existing manifest instead of regenerating",
    )
    return parser.parse_args(argv)


def hash_file(path: Path) -> tuple[int, str]:
    digest = hashlib.sha256()
    size = 0
    try:
        with path.open("rb") as handle:
            while True:
                chunk = handle.read(CHUNK_SIZE)
                if not chunk:
                    break
                size += len(chunk)
                digest.update(chunk)
    except OSError as exc:
        error(f"cannot read {path}: {exc}")
    return size, digest.hexdigest()


def package_header(path: Path) -> tuple[int | None, int | None, int | None]:
    """Return (file_version, licensee_version, package_flags) when the file
    starts with the Unreal package signature, else (None, None, None)."""
    try:
        with path.open("rb") as handle:
            header = handle.read(12)
    except OSError as exc:
        error(f"cannot read package header of {path}: {exc}")
    if len(header) < 12 or header[:4] != PACKAGE_SIGNATURE:
        return (None, None, None)
    file_version = int.from_bytes(header[4:6], "little")
    licensee_version = int.from_bytes(header[6:8], "little")
    package_flags = int.from_bytes(header[8:12], "little")
    return (file_version, licensee_version, package_flags)


def sorted_files(root: Path, directory: str) -> list[Path]:
    base = root / directory
    if not base.is_dir():
        error(f"data root is missing canonical directory {directory}/ ({base})")
    found: list[Path] = []
    for current, dir_names, file_names in base.walk(follow_symlinks=False):
        dir_names[:] = sorted(dir_names)
        found.extend(current / name for name in sorted(file_names))
    return found


def relative_posix(path: Path, root: Path) -> str:
    return PurePosixPath(path.relative_to(root)).as_posix()


def detect_case_collisions(entries: list[dict]) -> None:
    seen: dict[str, str] = {}
    for entry in entries:
        key = entry["path"].casefold()
        if key in seen and seen[key] != entry["path"]:
            error(
                "case-insensitive filesystem collision between "
                f"{seen[key]!r} and {entry['path']!r}"
            )
        seen[key] = entry["path"]


def scan_root(root: Path) -> list[dict]:
    resolved = root.resolve()
    if not resolved.is_dir():
        error(f"data root is not a directory: {root}")
    if not (resolved / "System" / "Default.ini").is_file():
        error(
            f"data root does not contain System/Default.ini: {resolved} "
            "(extraction must install the US template System/0/Default.ini there)"
        )

    entries: list[dict] = []
    counts: dict[str, int] = {}
    for directory in CANONICAL_ROOTS:
        for path in sorted_files(resolved, directory):
            size, sha256 = hash_file(path)
            file_version, licensee_version, package_flags = package_header(path)
            suffix = path.suffix.lower()
            if suffix in EXPECTED_PACKAGE_COUNTS:
                if file_version is None:
                    error(f"{suffix} file lacks the Unreal package signature: {path}")
                counts[suffix] = counts.get(suffix, 0) + 1
            elif suffix in {".dll", ".exe", ".exec"}:
                if file_version is not None:
                    error(f"unexpected package signature on executable payload: {path}")
            entries.append(
                {
                    "path": relative_posix(path, resolved),
                    "bytes": size,
                    "sha256": sha256,
                    "file_version": file_version,
                    "licensee_version": licensee_version,
                    "package_flags": package_flags,
                }
            )

    missing = {
        suffix: expected
        for suffix, expected in EXPECTED_PACKAGE_COUNTS.items()
        if counts.get(suffix, 0) != expected
    }
    if missing:
        detail = ", ".join(f"{suffix}: found {counts.get(suffix, 0)}, want {want}" for suffix, want in missing.items())
        error(f"package inventory mismatch ({detail})")

    detect_case_collisions(entries)
    return entries


def manifest_document(entries: list[dict]) -> dict:
    return {
        "format_version": MANIFEST_FORMAT_VERSION,
        "profile": DATA_PROFILE,
        "iso_label": ISO_LABEL,
        "files": entries,
    }


def render_manifest(document: dict) -> bytes:
    return (json.dumps(document, ensure_ascii=False, indent=2, sort_keys=True) + "\n").encode("utf-8")


def load_manifest(root: Path) -> dict:
    manifest_path = root / MANIFEST_NAME
    if not manifest_path.is_file():
        error(f"missing manifest: {manifest_path}")
    try:
        document = json.loads(manifest_path.read_bytes().decode("utf-8"))
    except (OSError, ValueError) as exc:
        error(f"cannot parse manifest {manifest_path}: {exc}")
    if document.get("format_version") != MANIFEST_FORMAT_VERSION:
        error(
            f"manifest format version {document.get('format_version')!r} != "
            f"{MANIFEST_FORMAT_VERSION}: regenerate with prepare_hp1_data.py"
        )
    if document.get("profile") != DATA_PROFILE:
        error(f"manifest profile {document.get('profile')!r} != {DATA_PROFILE!r}")
    if document.get("iso_label") != ISO_LABEL:
        error(f"manifest iso_label {document.get('iso_label')!r} != {ISO_LABEL!r}")
    return document


def check_tree(root: Path, document: dict) -> None:
    resolved = root.resolve()
    recorded = {entry["path"]: entry for entry in document["files"]}
    present: set[str] = set()
    for directory in CANONICAL_ROOTS:
        for path in sorted_files(resolved, directory):
            posix = relative_posix(path, resolved)
            present.add(posix)
            expected = recorded.get(posix)
            if expected is None:
                error(f"unrecorded file in data root: {posix}")
            size, sha256 = hash_file(path)
            if size != expected["bytes"]:
                error(f"size mismatch for {posix}: {size} != {expected['bytes']}")
            if sha256 != expected["sha256"]:
                error(f"sha256 mismatch for {posix}")
            file_version, licensee_version, package_flags = package_header(path)
            observed = (file_version, licensee_version, package_flags)
            wanted = (
                expected["file_version"],
                expected["licensee_version"],
                expected["package_flags"],
            )
            if observed != wanted:
                error(f"package header drift for {posix}: {observed} != {wanted}")

    for missing in sorted(set(recorded) - present):
        error(f"recorded file missing from data root: {missing}")


def run(args: argparse.Namespace) -> None:
    root = args.root
    if args.check:
        document = load_manifest(root)
        check_tree(root, document)
        print(f"ok: {len(document['files'])} files verified against {MANIFEST_NAME}")
        return

    entries = scan_root(root)
    document = manifest_document(entries)
    manifest_path = root / MANIFEST_NAME
    rendered = render_manifest(document)
    try:
        manifest_path.write_bytes(rendered)
    except OSError as exc:
        error(f"cannot write manifest {manifest_path}: {exc}")
    total_bytes = sum(entry["bytes"] for entry in entries)
    print(f"wrote {manifest_path}: {len(entries)} files, {total_bytes} bytes")


def main(argv: list[str] | None = None) -> int:
    try:
        run(parse_args(argv))
    except ManifestError as exc:
        print(f"prepare_hp1_data: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
