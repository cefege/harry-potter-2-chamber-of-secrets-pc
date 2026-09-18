#!/usr/bin/env python3
"""Verify every HP2 map with a deterministic package-aware smoke sweep.

Playable levels launch the isolated native app and retain the strict runtime
failure policy. A map is verified structurally instead only when its package
contains positive editor-template evidence independent of its filename. Maps
without that evidence remain playable candidates and launch normally, so a
missing PlayerStart cannot by itself turn a broken game level into a pass.

Budgets are enforced on the whole run: ``--max-map-seconds`` compares the
observed wall duration against the requested budget, and ``--max-frame-ms``
compares the worst observed per-tick frame time against its budget. The
engine emits per-tick timing only when ``HP2_FRAME_TIMING=1`` is set: each
tick then logs ``<HP2_RES> frame_ms=<milliseconds>``. Those markers are
parsed leniently like every other ``<HP2_RES> key=value`` line into each
map's ``resources`` field, and additionally aggregated into ``budgets``
(``frame_ms`` = max observed, ``frame_samples`` = count). When a launched
map produced no samples the engine-side reason ``engine_does_not_emit`` is
recorded instead. A launched map with both gl_textures_created and
gl_textures_destroyed present and unequal fails as a leak. Report
format_version stays 2; all new keys are additive.
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
VULKAN_RENDER_DEVICE_CLASS = "VulkanDrv.VulkanRenderDevice"
VIDEO_DEVICE_KEYS = ("GameRenderDevice", "WindowedRenderDevice", "RenderDevice")
# Loader environment mirrors Build/run_vulkan_smoke.py (MoltenVK via Homebrew).
DEFAULT_VULKAN_ICD = Path("/opt/homebrew/etc/vulkan/icd.d/MoltenVK_icd.json")
VULKAN_DYLD_LIBRARY_PATH = "/opt/homebrew/lib"
VULKAN_DYLD_FALLBACK_LIBRARY_PATH = "/opt/homebrew/opt/molten-vk/lib"
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


def _replace_directory(directory: Path) -> None:
    """Replace a scratch directory (log or frame-capture root) atomically."""
    try:
        if directory.is_symlink() or (directory.exists() and not directory.is_dir()):
            directory.unlink()
        elif directory.exists():
            shutil.rmtree(directory)
        directory.mkdir(parents=True, exist_ok=False)
    except OSError as error:
        raise SmokeError(f"cannot replace directory {directory}: {error}") from error


def _vulkan_engine_ini(data_root: Path, destination: Path) -> Path:
    """Generate an Engine.ini selecting the Vulkan render device.

    The stock Default.ini carries legacy video devices that ApplyPortableConfig
    would migrate to XOpenGLDrv at boot, so the Vulkan leg must hand the engine
    an explicit selection (same discipline as Build/run_vulkan_smoke.py).
    """
    default_path = data_root / "System" / "Default.ini"
    text = default_path.read_text(encoding="utf-8", errors="replace")
    for key in VIDEO_DEVICE_KEYS:
        text, count = re.subn(
            rf"^{key}=.*$", f"{key}={VULKAN_RENDER_DEVICE_CLASS}", text,
            count=1, flags=re.MULTILINE,
        )
        if count != 1:
            raise SmokeError(f"{default_path}: expected exactly one {key} entry")
    game_test._write_atomic(destination, text.encode("utf-8"))
    return destination


def _vulkan_environment(icd: Path) -> dict[str, str]:
    """MoltenVK loader environment for a -vulkan launch."""
    return {
        "VK_DRIVER_FILES": str(icd),
        "DYLD_LIBRARY_PATH": VULKAN_DYLD_LIBRARY_PATH,
        "DYLD_FALLBACK_LIBRARY_PATH": VULKAN_DYLD_FALLBACK_LIBRARY_PATH,
    }


def _frames_directory(output: Path) -> Path:
    name = output.name
    stem = name[:-5] if name.lower().endswith(".json") else name
    return output.with_name(f"{stem}-frames")


def _capture_record(
    frames_dir: Path | None, repo_root: Path
) -> tuple[dict[str, object], dict[str, object]]:
    """Summarize engine frame captures into capture + artifacts records.

    The engine writes frame_<6d>.png files plus a frame_meta.json sidecar
    ({"captures": [...]}) into the per-map frames directory. count reflects
    the number of captured entries and scale_factor the last observed HiDPI
    backing scale factor (null when capture was disabled or produced none).
    """
    if frames_dir is None:
        return (
            {"count": 0, "scale_factor": None},
            {"frames_dir": None, "frame_meta": None},
        )
    meta_path = frames_dir / "frame_meta.json"
    captures: list[dict[str, object]] = []
    try:
        decoded = json.loads(meta_path.read_text(encoding="utf-8"))
        if isinstance(decoded, dict) and isinstance(decoded.get("captures"), list):
            captures = [
                entry
                for entry in decoded["captures"]
                if isinstance(entry, dict)
            ]
    except (OSError, ValueError):
        captures = []
    scale_factor: float | None = None
    for capture in reversed(captures):
        candidate = capture.get("backing_scale_factor")
        if isinstance(candidate, (int, float)) and not isinstance(candidate, bool):
            scale_factor = float(candidate)
            break
    return (
        {"count": len(captures), "scale_factor": scale_factor},
        {
            "frames_dir": game_test._display_path(frames_dir, repo_root),
            "frame_meta": game_test._display_path(meta_path, repo_root),
        },
    )

def _run_map(
    *,
    executable: Path,
    data_root: Path,
    map_path: Path,
    map_relative: str,
    renderer: str,
    ticks: int,
    timeout_seconds: float,
    max_frame_ms: float | None,
    max_map_seconds: float | None,
    repo_root: Path,
    log_path: Path,
    inspection: dict[str, object],
    capture_frames_dir: Path | None = None,
    extra_environment: dict[str, str] | None = None,
    ini_file: Path | None = None,
    engine_bin: Path | None = None,
) -> dict[str, object]:
    # Per-tick timing markers are opt-in inside the engine; every smoke
    # launch turns them on so frame budgets measure real ticks.
    extra_env = {"HP2_FRAME_TIMING": "1"}
    if extra_environment:
        extra_env.update(extra_environment)
    if capture_frames_dir is not None:
        _replace_directory(capture_frames_dir)
        extra_env.update(
            {
                "HP2_CAPTURE_FRAMES": str(capture_frames_dir),
                "HP2_CAPTURE_MAP": Path(map_relative).stem,
                "HP2_CAPTURE_TICKS": str(ticks),
            }
        )
    result = game_test.run_game(
        app=executable,
        data_root=data_root,
        selected_map=map_relative,
        renderer=renderer,
        ticks=ticks,
        timeout_seconds=timeout_seconds,
        log_path=log_path,
        no_sound=True,
        extra_env=extra_env,
        ini_file=ini_file,
        engine_bin=engine_bin,
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
    result["capture"], result["artifacts"] = _capture_record(
        capture_frames_dir, repo_root
    )
    _apply_budget_contract(
        result,
        max_frame_ms=max_frame_ms,
        max_map_seconds=max_map_seconds,
        frame_samples_ms=_frame_ms_samples(log_path),
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
    max_frame_ms: float | None,
    max_map_seconds: float | None,
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
    record = {
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
    _apply_budget_contract(
        record,
        max_frame_ms=max_frame_ms,
        max_map_seconds=max_map_seconds,
        frame_samples_ms=[],
    )
    return record


def _resources_record(observed: dict[str, object]) -> dict[str, object]:
    """Normalize parsed <HP2_RES> fields; canonical keys pass through as null."""
    record: dict[str, object] = {
        key: observed.get(key) for key in game_test.RESOURCE_KEYS
    }
    for key, value in sorted(observed.items()):
        if key not in record:
            record[key] = value
    return record




def _resource_leak(resources: dict[str, object]) -> str | None:
    created = resources.get("gl_textures_created")
    destroyed = resources.get("gl_textures_destroyed")
    numeric = (
        lambda value: isinstance(value, (int, float))
        and not isinstance(value, bool)
    )
    if (
        numeric(created)
        and numeric(destroyed)
        and created != destroyed
    ):
        return (
            f"gl_textures_created={created} but "
            f"gl_textures_destroyed={destroyed}"
        )
    return None


def _frame_ms_samples(log_path: Path | None) -> list[float]:
    """Collect <HP2_RES> frame_ms values from a captured run log."""
    if log_path is None:
        return []
    try:
        text = log_path.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return []
    samples: list[float] = []
    for match in game_test.HP2_RESOURCE_MARKER.finditer(text):
        if match.group(1) != "frame_ms":
            continue
        try:
            sample = float(match.group(2))
        except ValueError:
            continue
        if sample >= 0.0 and sample != float("inf"):
            samples.append(sample)
    return samples


def _budgets_record(
    duration_seconds: float | None,
    *,
    max_frame_ms: float | None,
    max_map_seconds: float | None,
    frame_samples_ms: list[float],
) -> dict[str, object]:
    frame_samples = len(frame_samples_ms)
    record: dict[str, object] = {
        "max_frame_ms": max_frame_ms,
        "max_map_seconds": max_map_seconds,
        "frame_ms": max(frame_samples_ms) if frame_samples else None,
        "frame_samples": frame_samples,
        "map_seconds": duration_seconds,
    }
    if not frame_samples:
        # The engine emits per-tick frame timing only with HP2_FRAME_TIMING=1;
        # without samples the frame budget stays unmeasured and we say so.
        record["reason"] = "engine_does_not_emit"
    return record


def _budget_violations(budgets: dict[str, object]) -> list[dict[str, object]]:
    violations: list[dict[str, object]] = []
    map_limit = budgets["max_map_seconds"]
    map_seconds = budgets["map_seconds"]
    if (
        isinstance(map_limit, (int, float))
        and isinstance(map_seconds, (int, float))
        and map_seconds > map_limit
    ):
        violations.append(
            {
                "code": "budget.map_seconds_exceeded",
                "observed_seconds": map_seconds,
                "limit_seconds": map_limit,
            }
        )
    frame_limit = budgets["max_frame_ms"]
    frame_ms = budgets["frame_ms"]
    if (
        isinstance(frame_limit, (int, float))
        and isinstance(frame_ms, (int, float))
        and frame_ms > frame_limit
    ):
        violations.append(
            {
                "code": "budget.frame_ms_exceeded",
                "observed_ms": frame_ms,
                "limit_ms": frame_limit,
            }
        )
    return violations


_PROCESS_REASON_CODES = (
    ("launch_error", "process.launch_error"),
    ("exit_status", "process.exit_status"),
    ("timed_out", "process.timeout"),
    ("orphaned_process_group", "process.orphaned_group"),
    ("cleanup_error", "process.cleanup_error"),
)


def _failure_reason_code(record: dict[str, object]) -> str:
    """First failing condition as a dotted lowercase reason_code."""
    for key, code in _PROCESS_REASON_CODES:
        value = record.get(key)
        if key == "exit_status":
            if value not in (0, None):
                return code
            continue
        if value:
            return code
    if record.get("verification_error"):
        return "package.structure"
    if record.get("script_deferral_violation"):
        return "script.deferred"
    if record.get("resource_leak"):
        return "resources.leak"
    violations = record.get("budget_violations") or []
    if violations:
        return str(violations[0]["code"])
    markers = record.get("failure_markers") or []
    if markers:
        return f"marker.{markers[0]['category']}"
    return "unknown"


def _apply_budget_contract(
    record: dict[str, object],
    *,
    max_frame_ms: float | None,
    max_map_seconds: float | None,
    frame_samples_ms: list[float],
) -> None:
    """Attach additive schema-v1 alignment fields; format_version stays 2."""
    record["resources"] = _resources_record(record.get("resources") or {})
    record["resource_leak"] = _resource_leak(record["resources"])
    record["script_deferral_violation"] = (
        game_test._script_deferral_violation(record["resources"])
        if record.get("verification_mode") == "game_launch"
        else None
    )
    duration = record.get("duration_seconds")
    record["budgets"] = _budgets_record(
        duration if isinstance(duration, (int, float)) else None,
        max_frame_ms=max_frame_ms,
        max_map_seconds=max_map_seconds,
        frame_samples_ms=frame_samples_ms,
    )
    record["budget_violations"] = _budget_violations(record["budgets"])
    process_passed = bool(record["passed"])
    record["passed"] = bool(
        process_passed
        and not record["resource_leak"]
        and not record["script_deferral_violation"]
        and not record["budget_violations"]
    )
    record["status"] = "pass" if record["passed"] else "fail"
    record["reason_code"] = (
        None if record["passed"] else _failure_reason_code(record)
    )


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
            "Accessed None diagnostics count as script failures. Budgets: "
            "--max-map-seconds fails launched maps over the wall budget; "
            "--max-frame-ms fails launched maps whose worst observed per-tick "
            "frame time exceeds it (requires the engine's HP2_FRAME_TIMING=1 "
            "markers; unmeasured maps record frame_ms null). "
            "'<HP2_RES> key=value' log markers feed resources; unequal created/"
            "destroyed texture counts fail the map as a leak."
        ),
    )
    parser.add_argument(
        "--app",
        type=Path,
        default=DEFAULT_APP,
        help=".app bundle or direct Mach-O executable, relative to the repository",
    )
    parser.add_argument(
        "--engine-bin",
        type=Path,
        default=None,
        help="launch this engine executable instead of resolving --app as a "
        ".app bundle (native arm64 Mach-O validation still applies)",
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
        "--max-frame-ms",
        type=_positive_float,
        default=None,
        help=(
            "per-tick frame-time budget in milliseconds; fails launched maps "
            "whose worst observed <HP2_RES> frame_ms sample exceeds it "
            "(unmeasured runs record frame_ms null with reason "
            "engine_does_not_emit)"
        ),
    )
    parser.add_argument(
        "--max-map-seconds",
        type=_positive_float,
        default=None,
        help=(
            "per-map wall-clock budget in seconds, distinct from --timeout; "
            "maps whose total duration exceeds it fail"
        ),
    )
    parser.add_argument(
        "--output",
        type=Path,
        required=True,
        help="JSON report path (no default: every caller names its own "
        "report location; the retired default wrote under build/)",
    )
    parser.add_argument(
        "--maps",
        type=str,
        default=None,
        help=(
            "comma-separated map-name allowlist matched exactly and "
            "case-insensitively against map file stems (default: every map)"
        ),
    )
    parser.add_argument(
        "--capture-frames",
        action="store_true",
        default=False,
        help=(
            "enable engine end-of-frame capture (HP2_CAPTURE_FRAMES): every "
            "launched map writes frame_<6d>.png files plus frame_meta.json "
            "under <output-stem>-frames/<index>-<map>/ and its report record "
            "gains capture {count, scale_factor} fields"
        ),
    )
    parser.add_argument(
        "--vulkan-icd",
        type=Path,
        default=DEFAULT_VULKAN_ICD,
        help=(
            "Vulkan ICD manifest passed to the engine through VK_DRIVER_FILES "
            "when --renderer=vulkan (MoltenVK loader dylib paths are exported "
            "alongside it, mirroring Build/run_vulkan_smoke.py)"
        ),
    )
    return parser.parse_args()


def _select_maps(
    maps: list[tuple[Path, str]], allowlist: str | None
) -> list[tuple[Path, str]]:
    """Filter an enumerated map list by exact, case-insensitive stem names."""
    if not allowlist:
        return maps
    wanted = {
        name.strip().casefold()
        for name in allowlist.split(",")
        if name.strip()
    }
    selected = [
        (path, relative)
        for path, relative in maps
        if Path(relative).stem.casefold() in wanted
    ]
    missing = sorted(wanted - {Path(r).stem.casefold() for _, r in selected})
    if missing:
        raise SmokeError(
            "requested maps not found: " + ", ".join(missing)
        )
    return selected


def main() -> int:
    arguments = _arguments()
    try:
        repo_root = game_test._repo_root()
        app = game_test._resolve_path(arguments.app, repo_root, strict=True)
        data_root = game_test._resolve_path(arguments.data_root, repo_root, strict=True)
        game_test._validate_data_root(data_root)
        executable = (
            game_test._resolve_path(arguments.engine_bin, repo_root, strict=True)
            if arguments.engine_bin is not None
            else game_test._bundle_executable(app)
        )
        game_test._validate_native_arm64(executable)
        maps = _select_maps(game_test._enumerate_maps(data_root), arguments.maps)
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

        output = game_test._resolve_path(arguments.output, repo_root, strict=False)
        if not output.name:
            raise SmokeError(f"output path must name a JSON file: {arguments.output}")
        vulkan_environment: dict[str, str] | None = None
        vulkan_ini: Path | None = None
        if arguments.renderer == "vulkan":
            if not arguments.vulkan_icd.is_file():
                raise SmokeError(
                    f"vulkan ICD manifest not found: {arguments.vulkan_icd}"
                )
            vulkan_environment = _vulkan_environment(arguments.vulkan_icd)
            ini_stem = output.name[:-5] if output.name.lower().endswith(".json") else output.name
            vulkan_ini = _vulkan_engine_ini(
                data_root, output.with_name(f"{ini_stem}-vulkan-engine.ini")
            )
        logs = _log_directory(output)
        _replace_directory(logs)
        frames_root = (
            _frames_directory(output) if arguments.capture_frames else None
        )
        records: list[dict[str, object]] = []
        for index, ((map_path, map_relative), inspection) in enumerate(
            zip(maps, inspections), start=1
        ):
            log_path = logs / f"{index:06d}.log"
            capture_frames_dir = (
                frames_root / f"{index:06d}-{Path(map_relative).stem}"
                if frames_root is not None
                else None
            )
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
                    max_frame_ms=arguments.max_frame_ms,
                    max_map_seconds=arguments.max_map_seconds,
                    capture_frames_dir=capture_frames_dir,
                    extra_environment=vulkan_environment,
                    ini_file=vulkan_ini,
                    engine_bin=arguments.engine_bin,
                )
            else:
                record = _verify_map_package(
                    map_path=map_path,
                    map_relative=map_relative,
                    data_root=data_root,
                    repo_root=repo_root,
                    log_path=log_path,
                    inspection=inspection,
                    max_frame_ms=arguments.max_frame_ms,
                    max_map_seconds=arguments.max_map_seconds,
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
            "capture_frames": arguments.capture_frames,
            "vulkan_icd": (
                game_test._display_path(arguments.vulkan_icd, repo_root)
                if arguments.renderer == "vulkan"
                else None
            ),
            "vulkan_engine_ini": (
                game_test._display_path(vulkan_ini, repo_root)
                if vulkan_ini is not None
                else None
            ),
            "timeout_seconds": arguments.timeout,
            "max_frame_ms": arguments.max_frame_ms,
            "max_map_seconds": arguments.max_map_seconds,
            "status": "pass" if not failed_count else "fail",
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
                "resource_leaks": sum(
                    bool(record["resource_leak"]) for record in records
                ),
                "budget_violation_maps": sum(
                    bool(record["budget_violations"]) for record in records
                ),
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
