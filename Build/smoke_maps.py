#!/usr/bin/env python3
"""Verify every HP2 map with a deterministic package-aware smoke sweep.

Playable levels launch the isolated native app and retain the strict runtime
failure policy. A map is verified structurally instead only when its package
contains positive editor-template evidence independent of its filename. Maps
without that evidence remain playable candidates and launch normally, so a
missing PlayerStart cannot by itself turn a broken game level into a pass.
"""

from __future__ import annotations

import argparse
import json
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
import time
from typing import BinaryIO, Iterator

if __package__:
    from . import package79_reference as _package79
else:
    import package79_reference as _package79


DEFAULT_APP = Path("dist/macos-arm64/HarryPotter2.app")
DEFAULT_DATA_ROOT = Path("HarryPotter2/Unreal")
DEFAULT_TICKS = 300
DEFAULT_TIMEOUT_SECONDS = 120.0
ARM64_CPU_TYPE = 0x0100000C
TERMINATION_GRACE_SECONDS = 5.0
KILL_GRACE_SECONDS = 5.0
REPORT_FORMAT_VERSION = 2
SUPPORTED_MAP_VERSIONS = frozenset((76, 79))
MAP_CLASSIFICATIONS = (
    "playable_level",
    "editor_template_geometry_study",
    "streaming_sublevel",
    "malformed",
    "legacy_incomplete_dependency",
)
# These are semantic actor roles, resolved through the stock Core/Engine/HGame
# class inheritance graph. They are not map names or filesystem allowlists.
EDITOR_INFRASTRUCTURE_CLASSES = frozenset(
    ("Engine.Brush", "Engine.Camera", "Engine.LevelInfo")
)
VISUAL_ACTOR_ROOTS = (
    "Engine.Brush",
    "Engine.Decoration",
    "Engine.Light",
    "HGame.HProp",
)
TEMPLATE_MARKER_ROOTS = (
    "Engine.NavigationPoint",
    "HGame.Despawner",
    "HGame.TargetPoint",
)
BOOL_PROPERTY_TYPE = 3
STRUCT_PROPERTY_TYPE = 10
STR_PROPERTY_TYPE = 13
PACKAGE_ASSET_SUFFIXES = frozenset((".u", ".utx", ".uax"))

# ScriptWarning is included deliberately: UE1 reports runtime script faults such
# as Accessed None through that channel even though the label says "warning".
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


class SmokeError(Exception):
    """Raised when the harness cannot safely perform a complete sweep."""


def _repo_root() -> Path:
    try:
        return Path(__file__).resolve(strict=True).parent.parent
    except OSError as error:
        raise SmokeError(f"cannot resolve the repository root: {error}") from error


def _resolve_path(value: Path, repo_root: Path, *, strict: bool) -> Path:
    expanded = value.expanduser()
    candidate = expanded if expanded.is_absolute() else repo_root / expanded
    try:
        return candidate.resolve(strict=strict)
    except OSError as error:
        raise SmokeError(f"cannot resolve path {value}: {error}") from error


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
            raise SmokeError(f"cannot read app metadata {info_path}: {error}") from error

        executable_name = info.get("CFBundleExecutable")
        if (
            not isinstance(executable_name, str)
            or not executable_name
            or Path(executable_name).name != executable_name
        ):
            raise SmokeError(
                f"invalid CFBundleExecutable in {info_path}: {executable_name!r}"
            )
        executable = app / "Contents" / "MacOS" / executable_name
    else:
        raise SmokeError(f"app path is neither an app bundle nor an executable: {app}")

    try:
        executable = executable.resolve(strict=True)
        mode = executable.stat().st_mode
    except OSError as error:
        raise SmokeError(f"cannot inspect app executable {executable}: {error}") from error
    if not stat.S_ISREG(mode):
        raise SmokeError(f"app executable is not a regular file: {executable}")
    if not os.access(executable, os.X_OK):
        raise SmokeError(f"app executable is not executable: {executable}")
    return executable


def _thin_header_cpu(stream: BinaryIO, offset: int, size: int) -> int:
    if size < 8:
        raise SmokeError("Mach-O slice is too small to contain a header")
    stream.seek(offset)
    header = stream.read(8)
    if len(header) != 8:
        raise SmokeError("cannot read complete Mach-O slice header")

    magic = header[:4]
    if magic == b"\xcf\xfa\xed\xfe":
        endian = "<"
    elif magic == b"\xfe\xed\xfa\xcf":
        endian = ">"
    elif magic in (b"\xce\xfa\xed\xfe", b"\xfe\xed\xfa\xce"):
        raise SmokeError("app executable contains a 32-bit Mach-O slice")
    else:
        raise SmokeError(f"unrecognized Mach-O slice magic {magic.hex()}")
    return struct.unpack(f"{endian}I", header[4:8])[0]


def _validate_native_arm64(executable: Path) -> None:
    try:
        file_size = executable.stat().st_size
        with executable.open("rb") as stream:
            header = stream.read(8)
            if len(header) != 8:
                raise SmokeError(f"app executable is too small to be Mach-O: {executable}")

            magic = header[:4]
            if magic in (b"\xcf\xfa\xed\xfe", b"\xfe\xed\xfa\xcf"):
                cpu_type = _thin_header_cpu(stream, 0, file_size)
                if cpu_type != ARM64_CPU_TYPE:
                    raise SmokeError(
                        f"app executable is not arm64 (Mach-O CPU type 0x{cpu_type:08x}): "
                        f"{executable}"
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
                raise SmokeError(
                    f"app executable is not a recognized Mach-O binary: {executable}"
                )

            endian, is_64_bit_fat = fat_format
            architecture_count = struct.unpack(f"{endian}I", header[4:8])[0]
            entry_size = 32 if is_64_bit_fat else 20
            if architecture_count == 0:
                raise SmokeError("fat Mach-O app executable contains no architectures")
            if architecture_count > (file_size - 8) // entry_size:
                raise SmokeError("fat Mach-O architecture table extends past end of file")

            architectures: list[tuple[int, int, int]] = []
            for _ in range(architecture_count):
                entry = stream.read(entry_size)
                if len(entry) != entry_size:
                    raise SmokeError("cannot read complete fat Mach-O architecture table")
                if is_64_bit_fat:
                    cpu_type, _subtype, offset, size, _align, _reserved = struct.unpack(
                        f"{endian}IIQQII", entry
                    )
                else:
                    cpu_type, _subtype, offset, size, _align = struct.unpack(
                        f"{endian}IIIII", entry
                    )
                architectures.append((cpu_type, offset, size))

            for cpu_type, offset, size in architectures:
                if cpu_type != ARM64_CPU_TYPE:
                    raise SmokeError(
                        "app executable contains a non-arm64 Mach-O slice "
                        f"(CPU type 0x{cpu_type:08x})"
                    )
                if offset > file_size or size > file_size - offset:
                    raise SmokeError("fat Mach-O slice extends past end of file")
                slice_cpu_type = _thin_header_cpu(stream, offset, size)
                if slice_cpu_type != ARM64_CPU_TYPE:
                    raise SmokeError("fat Mach-O table and slice CPU types disagree")
    except SmokeError:
        raise
    except (OSError, struct.error) as error:
        raise SmokeError(f"cannot parse app executable {executable}: {error}") from error


def _map_files(directory: Path) -> Iterator[Path]:
    try:
        entries = sorted(
            os.scandir(directory), key=lambda entry: os.fsencode(entry.name)
        )
    except OSError as error:
        raise SmokeError(f"cannot enumerate data directory {directory}: {error}") from error

    for entry in entries:
        path = Path(entry.path)
        try:
            if entry.is_dir(follow_symlinks=False):
                yield from _map_files(path)
            elif entry.is_file(follow_symlinks=False) and entry.name[-4:].lower() == ".unr":
                yield path
            elif entry.is_symlink() and entry.name[-4:].lower() == ".unr":
                raise SmokeError(f"refusing symlinked map: {path}")
        except OSError as error:
            raise SmokeError(f"cannot inspect data path {path}: {error}") from error


def _enumerate_maps(data_root: Path) -> list[tuple[Path, str]]:
    maps: list[tuple[Path, str]] = []
    for path in _map_files(data_root):
        try:
            relative = path.relative_to(data_root).as_posix()
        except ValueError as error:
            raise SmokeError(f"map resolves outside data root: {path}") from error
        maps.append((path, relative))
    maps.sort(key=lambda item: os.fsencode(item[1]))
    if not maps:
        raise SmokeError(f"no .unr maps found under data root {data_root}")
    return maps


def _read_package(path: Path, relative: str) -> tuple[bytes, dict[str, object]]:
    try:
        data = path.read_bytes()
    except OSError as error:
        raise _package79.PackageFormatError(f"{relative}: cannot read package: {error}") from error
    if len(data) < 8:
        raise _package79.PackageFormatError(
            f"{relative}: package is too small for its tag and version"
        )

    version_word = struct.unpack_from("<I", data, 4)[0]
    version = version_word & 0xFFFF
    licensee = version_word >> 16
    if version not in SUPPORTED_MAP_VERSIONS or licensee != 0:
        supported = ", ".join(str(item) for item in sorted(SUPPORTED_MAP_VERSIONS))
        raise _package79.PackageFormatError(
            f"{relative}: unsupported package version/licensee {version}/{licensee}; "
            f"expected version {supported} with licensee 0"
        )

    expected_version = _package79.PACKAGE_VERSION
    try:
        _package79.PACKAGE_VERSION = version
        package = _package79.PackageReader(relative, data).read()
    finally:
        _package79.PACKAGE_VERSION = expected_version
    return data, package


def _class_catalog(data_root: Path) -> dict[str, str | None]:
    super_by_class: dict[str, str | None] = {}
    for package_name in ("Core.u", "Engine.u", "HGame.u"):
        path = data_root / "System" / package_name
        relative = f"System/{package_name}"
        if not path.is_file():
            raise SmokeError(f"class catalog package is missing: {path}")
        try:
            _data, package = _read_package(path, relative)
        except _package79.PackageFormatError as error:
            raise SmokeError(f"cannot parse class catalog package: {error}") from error
        for exported in package["exports"]:
            if exported["class_path"] == "Core.Class":
                super_path = exported["super_path"]
                super_by_class[exported["object_path"].casefold()] = (
                    super_path.casefold() if isinstance(super_path, str) else None
                )
    return super_by_class


def _available_package_paths(
    data_root: Path, required_paths: set[str]
) -> set[str]:
    files_by_stem: dict[str, list[Path]] = {}
    for path in data_root.rglob("*"):
        if path.is_file() and path.suffix.casefold() in PACKAGE_ASSET_SUFFIXES:
            files_by_stem.setdefault(path.stem.casefold(), []).append(path)

    available = {
        stem for stem, paths in files_by_stem.items() if len(paths) == 1
    }
    required_roots = {
        package_path.split(".", 1)[0].casefold()
        for package_path in required_paths
        if "." in package_path
    }
    for root in sorted(required_roots):
        paths = files_by_stem.get(root, [])
        if len(paths) != 1:
            continue
        path = paths[0]
        relative = path.relative_to(data_root).as_posix()
        try:
            _data, package = _read_package(path, relative)
        except _package79.PackageFormatError:
            continue
        available.update(
            exported["object_path"].casefold()
            for exported in package["exports"]
            if exported["class_path"] == "Core.Package"
        )
    return available


def _is_subclass(
    class_path: str, root: str, super_by_class: dict[str, str | None]
) -> bool:
    seen: set[str] = set()
    current: str | None = class_path.casefold()
    normalized_root = root.casefold()
    while current is not None and current not in seen:
        if current == normalized_root:
            return True
        seen.add(current)
        current = super_by_class.get(current)
    return False


def _tagged_properties(
    payload: bytes, names: list[dict[str, object]], relative: str, context: str
) -> tuple[list[tuple[str, int, bytes]], int]:
    cursor = _package79.Cursor(payload, relative, 0, context)
    properties: list[tuple[str, int, bytes]] = []
    while True:
        tag_offset = cursor.pos
        name_index = cursor.compact_index()
        if not 0 <= name_index < len(names):
            raise _package79.PackageFormatError(
                f"{relative}: offset {tag_offset}: {context} property name index "
                f"{name_index} is outside name table count {len(names)}"
            )
        name = names[name_index]["text"]
        if name == "None":
            return properties, cursor.pos

        info = cursor.u8()
        property_type = info & 0x0F
        if property_type == STRUCT_PROPERTY_TYPE:
            item_offset = cursor.pos
            item_name_index = cursor.compact_index()
            if not 0 <= item_name_index < len(names):
                raise _package79.PackageFormatError(
                    f"{relative}: offset {item_offset}: {context} struct name index "
                    f"{item_name_index} is outside name table count {len(names)}"
                )

        size_code = info & 0x70
        if size_code == 0x00:
            size = 1
        elif size_code == 0x10:
            size = 2
        elif size_code == 0x20:
            size = 4
        elif size_code == 0x30:
            size = 12
        elif size_code == 0x40:
            size = 16
        elif size_code == 0x50:
            size = cursor.u8()
        elif size_code == 0x60:
            size = cursor.u16()
        else:
            size = cursor.i32()
            if size < 0:
                raise _package79.PackageFormatError(
                    f"{relative}: offset {tag_offset}: negative {context} property size {size}"
                )

        if info & 0x80 and property_type != BOOL_PROPERTY_TYPE:
            first = cursor.u8()
            if first & 0x80:
                cursor.take(1 if first & 0xC0 == 0x80 else 3)
        properties.append((name, property_type, cursor.take(size)))


def _decode_fstring(payload: bytes, relative: str, context: str) -> str:
    cursor = _package79.Cursor(payload, relative, 0, context)
    count = cursor.compact_index()
    units = abs(count)
    if units == 0:
        return ""
    if count > 0:
        encoded = cursor.take(units)
        if encoded[-1] != 0:
            raise _package79.PackageFormatError(
                f"{relative}: unterminated ANSI {context}"
            )
        value = encoded[:-1].decode("latin-1")
    else:
        encoded = cursor.take(units * 2)
        if encoded[-2:] != b"\0\0":
            raise _package79.PackageFormatError(
                f"{relative}: unterminated UTF-16LE {context}"
            )
        value = encoded[:-2].decode("utf-16le", errors="strict")
    if cursor.pos != len(payload):
        raise _package79.PackageFormatError(
            f"{relative}: trailing bytes after {context}"
        )
    return value


def _level_summary_title(
    data: bytes, package: dict[str, object], relative: str
) -> str | None:
    summaries = [
        exported
        for exported in package["exports"]
        if exported["class_path"] == "Engine.LevelSummary"
    ]
    if len(summaries) != 1:
        raise _package79.PackageFormatError(
            f"{relative}: expected one Engine.LevelSummary export, found {len(summaries)}"
        )
    serial = summaries[0]["serial"]
    if serial is None:
        raise _package79.PackageFormatError(
            f"{relative}: Engine.LevelSummary has no serialized payload"
        )
    payload = data[serial["offset"] : serial["end"]]
    properties, end = _tagged_properties(
        payload, package["names"], relative, "LevelSummary"
    )
    if end != len(payload):
        raise _package79.PackageFormatError(
            f"{relative}: unexpected custom data after LevelSummary properties"
        )
    titles = [
        value
        for name, property_type, value in properties
        if name == "Title" and property_type == STR_PROPERTY_TYPE
    ]
    if len(titles) > 1:
        raise _package79.PackageFormatError(
            f"{relative}: LevelSummary serializes Title more than once"
        )
    return (
        _decode_fstring(titles[0], relative, "LevelSummary.Title")
        if titles
        else None
    )


def _active_actor_classes(
    data: bytes, package: dict[str, object], relative: str
) -> tuple[list[str], int]:
    levels = [
        exported
        for exported in package["exports"]
        if exported["class_path"] == "Engine.Level"
    ]
    if len(levels) != 1:
        raise _package79.PackageFormatError(
            f"{relative}: expected one Engine.Level export, found {len(levels)}"
        )
    serial = levels[0]["serial"]
    if serial is None:
        raise _package79.PackageFormatError(
            f"{relative}: Engine.Level has no serialized payload"
        )
    payload = data[serial["offset"] : serial["end"]]
    _properties, properties_end = _tagged_properties(
        payload, package["names"], relative, "Level"
    )
    cursor = _package79.Cursor(payload, relative, properties_end, "Level actor array")
    actor_count = cursor.i32()
    actor_capacity = cursor.i32()
    if actor_count < 0 or actor_capacity < actor_count:
        raise _package79.PackageFormatError(
            f"{relative}: invalid Level actor array count/capacity "
            f"{actor_count}/{actor_capacity}"
        )

    actor_classes: list[str] = []
    null_slots = 0
    exports = package["exports"]
    for actor_index in range(actor_count):
        reference_offset = cursor.pos
        reference = cursor.compact_index()
        if reference == 0:
            null_slots += 1
            continue
        if reference < 0 or reference > len(exports):
            raise _package79.PackageFormatError(
                f"{relative}: offset {reference_offset}: Level actor {actor_index} "
                f"has invalid package reference {reference}"
            )
        class_path = exports[reference - 1]["class_path"]
        if not isinstance(class_path, str):
            raise _package79.PackageFormatError(
                f"{relative}: Level actor {actor_index} has no class path"
            )
        actor_classes.append(class_path)
    return actor_classes, null_slots


def _inspect_map_package(
    path: Path,
    relative: str,
    super_by_class: dict[str, str | None],
) -> dict[str, object]:
    try:
        data, package = _read_package(path, relative)
        actor_classes, null_slots = _active_actor_classes(data, package, relative)
        title = _level_summary_title(data, package, relative)
    except (
        OSError,
        UnicodeError,
        ValueError,
        IndexError,
        KeyError,
        struct.error,
        _package79.PackageFormatError,
    ) as error:
        return {
            "classification": "malformed",
            "classification_evidence": {
                "rule": "package-structure-validation-failed",
                "package_structure_valid": False,
            },
            "expected_player_start": None,
            "file_sha256": None,
            "package_error": f"{type(error).__name__}: {error}",
            "verification_mode": "package_structure",
        }

    actor_counts: dict[str, int] = {}
    for class_path in actor_classes:
        actor_counts[class_path] = actor_counts.get(class_path, 0) + 1
    sorted_actor_counts = [
        {"class": class_path, "count": count}
        for class_path, count in sorted(
            actor_counts.items(), key=lambda item: item[0].encode("utf-8")
        )
    ]
    player_start_count = sum(
        count
        for class_path, count in actor_counts.items()
        if _is_subclass(class_path, "Engine.PlayerStart", super_by_class)
    )
    required_package_imports = sorted(
        {
            imported["object_path"]
            for imported in package["imports"]
            if imported["class_path"] == "Core.Package"
            and isinstance(imported["object_path"], str)
        },
        key=lambda value: value.encode("utf-8"),
    )
    evidence: dict[str, object] = {
        "package_file_size": package["file_size"],
        "active_actor_classes": sorted_actor_counts,
        "active_actor_count": len(actor_classes),
        "export_count": package["summary"]["export_count"],
        "level_export_count": 1,
        "level_summary_export_count": 1,
        "package_structure_valid": True,
        "level_summary_title": title,
        "null_actor_slots": null_slots,
        "package_licensee": package["summary"]["licensee"],
        "package_tag": package["summary"]["tag_hex"],
        "package_version": package["summary"]["version"],
        "player_start_count": player_start_count,
        "required_package_imports": required_package_imports,
    }
    return {
        "_actor_counts": actor_counts,
        "_required_package_imports": required_package_imports,
        "_level_summary_title": title,
        "classification": None,
        "classification_evidence": evidence,
        "expected_player_start": None,
        "file_sha256": package["file_sha256"],
        "package_error": None,
        "verification_mode": None,
    }


def _classify_map_package(
    inspection: dict[str, object],
    duplicate_payload_count: int,
    super_by_class: dict[str, str | None],
) -> None:
    if inspection["classification"] == "malformed":
        return

    actor_counts = inspection.pop("_actor_counts")
    unresolved_package_imports = inspection.pop("_unresolved_package_imports", [])
    inspection.pop("_required_package_imports", None)
    title = inspection.pop("_level_summary_title")
    evidence = inspection["classification_evidence"]
    evidence["duplicate_payload_count"] = duplicate_payload_count
    player_start_count = evidence["player_start_count"]
    evidence["unresolved_package_imports"] = unresolved_package_imports
    if unresolved_package_imports and evidence["package_version"] < 79:
        inspection["classification"] = "legacy_incomplete_dependency"
        inspection["expected_player_start"] = False
        inspection["verification_mode"] = "package_structure"
        evidence["rule"] = "legacy-unresolved-package-imports"
        return
    if player_start_count:
        inspection["classification"] = "playable_level"
        inspection["expected_player_start"] = True
        inspection["verification_mode"] = "game_launch"
        evidence["rule"] = "active-player-start"
        return

    functional_counts: dict[str, int] = {}
    for class_path, count in actor_counts.items():
        if class_path in EDITOR_INFRASTRUCTURE_CLASSES:
            continue
        if any(
            _is_subclass(class_path, root, super_by_class)
            for root in VISUAL_ACTOR_ROOTS
        ):
            continue
        functional_counts[class_path] = count
    evidence["functional_actor_classes"] = [
        {"class": class_path, "count": count}
        for class_path, count in sorted(
            functional_counts.items(), key=lambda item: item[0].encode("utf-8")
        )
    ]

    if title == "Untitled" and not functional_counts:
        inspection["classification"] = "editor_template_geometry_study"
        inspection["expected_player_start"] = False
        inspection["verification_mode"] = "package_structure"
        evidence["rule"] = "untitled-visual-actors-only"
        return

    marker_only = bool(functional_counts) and all(
        any(
            _is_subclass(class_path, root, super_by_class)
            for root in TEMPLATE_MARKER_ROOTS
        )
        for class_path in functional_counts
    )
    if duplicate_payload_count > 1 and marker_only:
        inspection["classification"] = "editor_template_geometry_study"
        inspection["expected_player_start"] = False
        inspection["verification_mode"] = "package_structure"
        evidence["rule"] = "duplicate-prop-and-marker-actor-template"
        return

    player_pawn_count = sum(
        count
        for class_path, count in functional_counts.items()
        if _is_subclass(class_path, "Engine.PlayerPawn", super_by_class)
    )
    if (
        title == "Untitled"
        and player_pawn_count == 1
        and player_pawn_count == sum(functional_counts.values())
    ):
        inspection["classification"] = "editor_template_geometry_study"
        inspection["expected_player_start"] = False
        inspection["verification_mode"] = "package_structure"
        evidence["rule"] = "untitled-geometry-with-single-preplaced-player-pawn"
        return

    inspection["classification"] = "playable_level"
    inspection["expected_player_start"] = True
    inspection["verification_mode"] = "game_launch"
    evidence["rule"] = "conservative-playable-fallback"


def _map_token(path: Path, data_root: Path) -> str:
    # FURL treats forward slashes as URL/portal delimiters. Backslashes survive
    # URL parsing, then FFileManagerUnix normalizes them to POSIX separators.
    system_directory = data_root / "System"
    relative = os.path.relpath(path, system_directory)
    return relative.replace(os.sep, "\\")


def _log_directory(output: Path) -> Path:
    name = output.name
    stem = name[:-5] if name.lower().endswith(".json") else name
    return output.with_name(f"{stem}-logs")


def _replace_log_directory(directory: Path) -> None:
    try:
        if directory.is_symlink() or (directory.exists() and not directory.is_dir()):
            directory.unlink()
        elif directory.exists():
            shutil.rmtree(directory)
        directory.mkdir(parents=True, exist_ok=False)
    except OSError as error:
        raise SmokeError(f"cannot replace log directory {directory}: {error}") from error


def _write_atomic(path: Path, contents: bytes) -> None:
    temporary: Path | None = None
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
        with tempfile.NamedTemporaryFile(
            mode="wb", prefix=f".{path.name}.", dir=path.parent, delete=False
        ) as stream:
            temporary = Path(stream.name)
            stream.write(contents)
            stream.flush()
            os.fsync(stream.fileno())
        os.chmod(temporary, 0o644)
        os.replace(temporary, path)
    except OSError as error:
        raise SmokeError(f"cannot write {path}: {error}") from error
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
    replacements = (
        (str(isolated_home), "<isolated-home>"),
        (str(repo_root) + os.sep, ""),
    )
    for source, replacement in replacements:
        text = text.replace(source, replacement)
    return text


def _failure_markers(
    output: bytes, repo_root: Path, isolated_home: Path
) -> list[dict[str, object]]:
    decoded = output.decode("utf-8", errors="replace")
    markers: list[dict[str, object]] = []
    for line_number, line in enumerate(decoded.splitlines(), start=1):
        for category, patterns in COMPILED_FAILURE_MARKERS.items():
            matching_pattern = next((pattern for pattern in patterns if pattern.search(line)), None)
            if matching_pattern is None:
                continue
            reported = _report_line(line.strip(), repo_root, isolated_home)
            truncated = len(reported) > 1000
            markers.append(
                {
                    "category": category,
                    "line": line_number,
                    "pattern": matching_pattern.pattern,
                    "text": reported[:1000],
                    "truncated": truncated,
                }
            )
    return markers


def _isolated_environment(home: Path) -> dict[str, str]:
    temporary = home / "tmp"
    config = home / ".config"
    cache = home / ".cache"
    for directory in (temporary, config, cache):
        directory.mkdir(parents=True, exist_ok=True)

    environment = os.environ.copy()
    environment.update(
        {
            "HOME": str(home),
            "CFFIXED_USER_HOME": str(home),
            "TMPDIR": str(temporary) + os.sep,
            "XDG_CONFIG_HOME": str(config),
            "XDG_CACHE_HOME": str(cache),
            "LANG": "C",
            "LC_ALL": "C",
            "TZ": "UTC",
        }
    )
    return environment


def _run_map(
    *,
    executable: Path,
    data_root: Path,
    map_path: Path,
    map_relative: str,
    renderer: str,
    ticks: int,
    timeout_seconds: float,
    repo_root: Path,
    log_path: Path,
    inspection: dict[str, object],
) -> dict[str, object]:
    token = _map_token(map_path, data_root)
    arguments = [
        str(executable),
        f"-datadir={data_root}",
        token,
    ]
    arguments.append("-xopengl" if renderer == "xopengl" else "-vulkan")
    arguments.extend(
        [
            "-NOFRONTEND",
            "-window",
            "-nosound",
            f"-testticks={ticks}",
            "-log",
        ]
    )

    started = time.monotonic()
    output = b""
    exit_status: int | None = None
    timed_out = False
    launch_error: str | None = None
    cleanup_action = "none"
    cleanup_error: str | None = None
    orphaned_process_group = False

    with tempfile.TemporaryDirectory(prefix="hp2-map-smoke-") as home_name:
        isolated_home = Path(home_name)
        environment = _isolated_environment(isolated_home)
        process: subprocess.Popen[bytes] | None = None
        try:
            process = subprocess.Popen(
                arguments,
                cwd=repo_root,
                env=environment,
                stdin=subprocess.DEVNULL,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                start_new_session=True,
            )
            try:
                output, _ = process.communicate(timeout=timeout_seconds)
            except subprocess.TimeoutExpired as timeout_error:
                timed_out = True
                partial_output = timeout_error.output or b""
                term_error = _signal_group(process.pid, signal.SIGTERM)
                cleanup_action = "sigterm"
                cleanup_error = term_error
                try:
                    output, _ = process.communicate(timeout=TERMINATION_GRACE_SECONDS)
                except subprocess.TimeoutExpired as term_timeout:
                    partial_output = term_timeout.output or partial_output
                    kill_error = _signal_group(process.pid, signal.SIGKILL)
                    cleanup_action = "sigkill"
                    if kill_error:
                        cleanup_error = "; ".join(
                            message for message in (cleanup_error, kill_error) if message
                        )
                    try:
                        output, _ = process.communicate(timeout=KILL_GRACE_SECONDS)
                    except subprocess.TimeoutExpired as kill_timeout:
                        output = kill_timeout.output or partial_output
                        cleanup_action = "failed"
                        message = "app did not exit after process-group SIGKILL"
                        cleanup_error = "; ".join(
                            item for item in (cleanup_error, message) if item
                        )
                if not output:
                    output = partial_output

            exit_status = process.returncode
            if _group_exists(process.pid):
                orphaned_process_group = not timed_out
                remaining_action, remaining_error = _stop_remaining_group(process.pid)
                if remaining_action != "none":
                    cleanup_action = remaining_action
                if remaining_error:
                    cleanup_error = "; ".join(
                        item for item in (cleanup_error, remaining_error) if item
                    )
        except OSError as error:
            launch_error = _report_line(
                f"{type(error).__name__}: {error}", repo_root, isolated_home
            )
            output = f"smoke harness launch error: {error}\n".encode(
                "utf-8", errors="backslashreplace"
            )
        finally:
            if process is not None and process.poll() is None:
                orphaned_process_group = True
                final_term_error = _signal_group(process.pid, signal.SIGTERM)
                cleanup_action = "sigterm"
                if final_term_error:
                    cleanup_error = "; ".join(
                        item for item in (cleanup_error, final_term_error) if item
                    )
                try:
                    final_output, _ = process.communicate(
                        timeout=TERMINATION_GRACE_SECONDS
                    )
                    if final_output:
                        output = final_output
                except subprocess.TimeoutExpired as final_timeout:
                    final_partial = final_timeout.output or output
                    final_kill_error = _signal_group(process.pid, signal.SIGKILL)
                    cleanup_action = "sigkill"
                    if final_kill_error:
                        cleanup_error = "; ".join(
                            item
                            for item in (cleanup_error, final_kill_error)
                            if item
                        )
                    try:
                        final_output, _ = process.communicate(
                            timeout=KILL_GRACE_SECONDS
                        )
                        output = final_output or final_partial
                    except subprocess.TimeoutExpired as unreaped:
                        output = unreaped.output or final_partial
                        cleanup_action = "failed"
                        cleanup_error = "; ".join(
                            item
                            for item in (
                                cleanup_error,
                                "app could not be reaped after SIGKILL",
                            )
                            if item
                        )
                exit_status = process.returncode
                if _group_exists(process.pid):
                    remaining_action, remaining_error = _stop_remaining_group(
                        process.pid
                    )
                    if remaining_action != "none":
                        cleanup_action = remaining_action
                    if remaining_error:
                        cleanup_error = "; ".join(
                            item
                            for item in (cleanup_error, remaining_error)
                            if item
                        )

        markers = _failure_markers(output, repo_root, isolated_home)

    duration_seconds = round(time.monotonic() - started, 6)
    _write_atomic(log_path, output)

    displayed_command = [
        _display_path(executable, repo_root),
        f"-datadir={_display_path(data_root, repo_root)}",
        token,
        *arguments[3:],
    ]
    passed = (
        exit_status == 0
        and not timed_out
        and launch_error is None
        and not markers
        and not orphaned_process_group
        and cleanup_error is None
    )
    return {
        "classification": inspection["classification"],
        "classification_evidence": inspection["classification_evidence"],
        "expected_player_start": inspection["expected_player_start"],
        "file_sha256": inspection["file_sha256"],
        "package_error": inspection["package_error"],
        "verification_mode": inspection["verification_mode"],
        "verification_error": None,
        "map": _display_path(map_path, repo_root),
        "map_relative_to_data_root": map_relative,
        "map_token": token,
        "command": displayed_command,
        "log": _display_path(log_path, repo_root),
        "captured_output_bytes": len(output),
        "duration_seconds": duration_seconds,
        "exit_status": exit_status,
        "timed_out": timed_out,
        "launch_error": launch_error,
        "failure_markers": markers,
        "orphaned_process_group": orphaned_process_group,
        "process_group_cleanup": cleanup_action,
        "cleanup_error": cleanup_error,
        "passed": passed,
    }


def _verify_map_package(
    *,
    map_path: Path,
    map_relative: str,
    data_root: Path,
    repo_root: Path,
    log_path: Path,
    inspection: dict[str, object],
) -> dict[str, object]:
    verification_error = inspection["package_error"]
    passed = verification_error is None
    markers: list[dict[str, object]] = []
    if verification_error is not None:
        markers.append(
            {
                "category": "package",
                "line": 0,
                "pattern": "package-structure-validation",
                "text": str(verification_error)[:1000],
                "truncated": len(str(verification_error)) > 1000,
            }
        )
    log_record = {
        "classification": inspection["classification"],
        "classification_evidence": inspection["classification_evidence"],
        "map_relative_to_data_root": map_relative,
        "package_error": verification_error,
        "package_verification_passed": passed,
        "verification_mode": inspection["verification_mode"],
    }
    output = (
        json.dumps(
            log_record,
            ensure_ascii=True,
            allow_nan=False,
            indent=2,
            sort_keys=True,
        )
        + "\n"
    ).encode("utf-8")
    _write_atomic(log_path, output)
    return {
        "map": _display_path(map_path, repo_root),
        "map_relative_to_data_root": map_relative,
        "map_token": _map_token(map_path, data_root),
        "classification": inspection["classification"],
        "classification_evidence": inspection["classification_evidence"],
        "expected_player_start": inspection["expected_player_start"],
        "file_sha256": inspection["file_sha256"],
        "package_error": inspection["package_error"],
        "verification_mode": inspection["verification_mode"],
        "verification_error": verification_error,
        "command": [],
        "log": _display_path(log_path, repo_root),
        "captured_output_bytes": len(output),
        "duration_seconds": 0.0,
        "exit_status": None,
        "timed_out": False,
        "launch_error": None,
        "failure_markers": markers,
        "orphaned_process_group": False,
        "process_group_cleanup": "none",
        "cleanup_error": None,
        "passed": passed,
    }


def _positive_int(value: str) -> int:
    try:
        parsed = int(value, 10)
    except ValueError as error:
        raise argparse.ArgumentTypeError("must be an integer") from error
    if parsed <= 0:
        raise argparse.ArgumentTypeError("must be greater than zero")
    return parsed


def _positive_float(value: str) -> float:
    try:
        parsed = float(value)
    except ValueError as error:
        raise argparse.ArgumentTypeError("must be a number") from error
    if not (parsed > 0.0) or parsed == float("inf"):
        raise argparse.ArgumentTypeError("must be a finite number greater than zero")
    return parsed


def _arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=__doc__,
        formatter_class=argparse.ArgumentDefaultsHelpFormatter,
        epilog=(
            "Failure policy: nonzero exit, timeout, launch/cleanup failure, an "
            "orphaned process group, or any case-insensitive script/native/package/"
            "render/assert/critical marker fails the map. UE1 ScriptWarning and "
            "Accessed None diagnostics count as script failures."
        ),
    )
    parser.add_argument(
        "--app",
        type=Path,
        default=DEFAULT_APP,
        help=".app bundle or direct Mach-O executable, relative to the repository",
    )
    parser.add_argument(
        "--data-root",
        type=Path,
        default=DEFAULT_DATA_ROOT,
        help="Unreal data root, relative to the repository",
    )
    parser.add_argument(
        "--renderer",
        choices=("xopengl", "vulkan"),
        required=True,
        help="renderer to smoke (-xopengl or -vulkan is passed explicitly)",
    )
    parser.add_argument(
        "--ticks",
        type=_positive_int,
        default=DEFAULT_TICKS,
        help="test ticks per map",
    )
    parser.add_argument(
        "--timeout",
        type=_positive_float,
        default=DEFAULT_TIMEOUT_SECONDS,
        help="wall-clock timeout per map in seconds",
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=None,
        help="JSON report path (default: build/smoke-maps-<renderer>.json)",
    )
    return parser.parse_args()


def main() -> int:
    arguments = _arguments()
    try:
        repo_root = _repo_root()
        app = _resolve_path(arguments.app, repo_root, strict=True)
        data_root = _resolve_path(arguments.data_root, repo_root, strict=True)
        if not data_root.is_dir():
            raise SmokeError(f"data root is not a directory: {data_root}")
        default_ini = data_root / "System" / "Default.ini"
        if not default_ini.is_file():
            raise SmokeError(f"data root does not contain System/Default.ini: {data_root}")

        executable = _bundle_executable(app)
        _validate_native_arm64(executable)
        maps = _enumerate_maps(data_root)
        super_by_class = _class_catalog(data_root)
        inspections = [
            _inspect_map_package(map_path, map_relative, super_by_class)
            for map_path, map_relative in maps
        ]
        required_package_paths = {
            package_path
            for inspection in inspections
            for package_path in inspection.get("_required_package_imports", [])
        }
        available_package_paths = _available_package_paths(
            data_root, required_package_paths
        )
        for inspection in inspections:
            inspection["_unresolved_package_imports"] = sorted(
                (
                    package_path
                    for package_path in inspection.get(
                        "_required_package_imports", []
                    )
                    if package_path.casefold() not in available_package_paths
                ),
                key=lambda value: value.encode("utf-8"),
            )
        duplicate_counts: dict[str, int] = {}
        for inspection in inspections:
            file_sha256 = inspection["file_sha256"]
            if file_sha256 is not None:
                duplicate_counts[file_sha256] = (
                    duplicate_counts.get(file_sha256, 0) + 1
                )
        for inspection in inspections:
            file_sha256 = inspection["file_sha256"]
            _classify_map_package(
                inspection,
                duplicate_counts.get(file_sha256, 0),
                super_by_class,
            )

        output_argument = arguments.output or Path(
            f"build/smoke-maps-{arguments.renderer}.json"
        )
        output = _resolve_path(output_argument, repo_root, strict=False)
        if not output.name:
            raise SmokeError(f"output path must name a JSON file: {output_argument}")
        logs = _log_directory(output)
        _replace_log_directory(logs)

        records: list[dict[str, object]] = []
        for index, ((map_path, map_relative), inspection) in enumerate(
            zip(maps, inspections), start=1
        ):
            log_path = logs / f"{index:06d}.log"
            if inspection["verification_mode"] == "game_launch":
                record = _run_map(
                    executable=executable,
                    data_root=data_root,
                    map_path=map_path,
                    map_relative=map_relative,
                    renderer=arguments.renderer,
                    ticks=arguments.ticks,
                    timeout_seconds=arguments.timeout,
                    repo_root=repo_root,
                    log_path=log_path,
                    inspection=inspection,
                )
            else:
                record = _verify_map_package(
                    map_path=map_path,
                    map_relative=map_relative,
                    data_root=data_root,
                    repo_root=repo_root,
                    log_path=log_path,
                    inspection=inspection,
                )
            records.append(record)

        passed_count = sum(1 for record in records if record["passed"])
        failed_count = len(records) - passed_count
        category_counts = {
            category: sum(
                1
                for record in records
                for marker in record["failure_markers"]
                if marker["category"] == category
            )
            for category in FAILURE_MARKER_PATTERNS
        }
        classification_counts = {
            classification: sum(
                record["classification"] == classification for record in records
            )
            for classification in MAP_CLASSIFICATIONS
        }
        verification_mode_counts = {
            mode: sum(record["verification_mode"] == mode for record in records)
            for mode in ("game_launch", "package_structure")
        }
        report = {
            "format_version": REPORT_FORMAT_VERSION,
            "script": "Build/smoke_maps.py",
            "app": _display_path(app, repo_root),
            "executable": _display_path(executable, repo_root),
            "data_root": _display_path(data_root, repo_root),
            "renderer": arguments.renderer,
            "ticks": arguments.ticks,
            "timeout_seconds": arguments.timeout,
            "log_directory": _display_path(logs, repo_root),
            "failure_marker_policy": {
                "case_sensitive": False,
                "patterns": FAILURE_MARKER_PATTERNS,
                "success_requires": (
                    "runtime verification requires no case-insensitive script/native/"
                    "package/render/assert/critical marker"
                ),
            },
            "verification_policy": {
                "game_launch": (
                    "exit status 0; no timeout, launch error, failure marker, "
                    "orphaned process group, or cleanup error"
                ),
                "package_structure": (
                    "supported package tag/version, internally valid name/import/"
                    "export/payload tables, exactly one Level and LevelSummary, "
                    "and a valid active Level actor array"
                ),
                "nonplayable_classification": (
                    "requires a positive editor/template actor-and-metadata signature; "
                    "an unrecognized map without PlayerStart is launched conservatively"
                ),
            },
            "summary": {
                "total": len(records),
                "passed": passed_count,
                "failed": failed_count,
                "failure_markers_by_category": category_counts,
                "by_classification": classification_counts,
                "by_verification_mode": verification_mode_counts,
            },
            "maps": records,
        }
        encoded = (
            json.dumps(
                report,
                ensure_ascii=True,
                allow_nan=False,
                indent=2,
                sort_keys=True,
            )
            + "\n"
        ).encode("utf-8")
        _write_atomic(output, encoded)
        print(
            f"{_display_path(output, repo_root)}: "
            f"{passed_count}/{len(records)} maps passed"
        )
        return 1 if failed_count else 0
    except SmokeError as error:
        print(f"smoke_maps.py: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
