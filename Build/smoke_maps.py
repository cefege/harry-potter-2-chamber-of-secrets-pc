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
import re
import shutil
import struct
import sys
from typing import Iterator

if __package__:
    from . import game_test
    from . import package79_reference as _package79
else:
    import game_test
    import package79_reference as _package79

SmokeError = game_test.GameTestError


DEFAULT_APP = Path("dist/macos-arm64/HarryPotter2.app")
DEFAULT_DATA_ROOT = Path("HarryPotter2/Unreal")
DEFAULT_TICKS = 300
DEFAULT_TIMEOUT_SECONDS = 120.0
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
    result = game_test.run_game(
        app=executable,
        data_root=data_root,
        selected_map=map_relative,
        renderer=renderer,
        ticks=ticks,
        timeout_seconds=timeout_seconds,
        log_path=log_path,
        no_sound=True,
    )
    result.update(
        {
            "classification": inspection["classification"],
            "classification_evidence": inspection["classification_evidence"],
            "expected_player_start": inspection["expected_player_start"],
            "file_sha256": inspection["file_sha256"],
            "package_error": inspection["package_error"],
            "verification_mode": inspection["verification_mode"],
            "verification_error": None,
            "map": game_test._display_path(map_path, repo_root),
            "map_relative_to_data_root": map_relative,
        }
    )
    return result


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
    game_test._write_atomic(log_path, output)
    return {
        "map": game_test._display_path(map_path, repo_root),
        "map_relative_to_data_root": map_relative,
        "map_token": game_test._map_token(map_path, data_root),
        "classification": inspection["classification"],
        "classification_evidence": inspection["classification_evidence"],
        "expected_player_start": inspection["expected_player_start"],
        "file_sha256": inspection["file_sha256"],
        "package_error": inspection["package_error"],
        "verification_mode": inspection["verification_mode"],
        "verification_error": verification_error,
        "command": [],
        "log": game_test._display_path(log_path, repo_root),
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
        repo_root = game_test._repo_root()
        app = game_test._resolve_path(arguments.app, repo_root, strict=True)
        data_root = game_test._resolve_path(arguments.data_root, repo_root, strict=True)
        game_test._validate_data_root(data_root)
        executable = game_test._bundle_executable(app)
        game_test._validate_native_arm64(executable)
        maps = game_test._enumerate_maps(data_root)
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
        output = game_test._resolve_path(output_argument, repo_root, strict=False)
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
            for category in game_test.FAILURE_MARKER_PATTERNS
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
            "app": game_test._display_path(app, repo_root),
            "executable": game_test._display_path(executable, repo_root),
            "data_root": game_test._display_path(data_root, repo_root),
            "renderer": arguments.renderer,
            "ticks": arguments.ticks,
            "timeout_seconds": arguments.timeout,
            "log_directory": game_test._display_path(logs, repo_root),
            "failure_marker_policy": {
                "case_sensitive": False,
                "patterns": game_test.FAILURE_MARKER_PATTERNS,
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
        game_test._write_atomic(output, encoded)
        print(
            f"{game_test._display_path(output, repo_root)}: "
            f"{passed_count}/{len(records)} maps passed"
        )
        return 1 if failed_count else 0
    except SmokeError as error:
        print(f"smoke_maps.py: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
