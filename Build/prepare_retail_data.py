#!/usr/bin/env python3
"""Build and validate a deterministic retail-first HP2 runtime data overlay."""

from __future__ import annotations

import argparse
import collections
import errno
import hashlib
import json
import os
import shutil
import stat
import subprocess
import sys
import tarfile
import tempfile
import unicodedata
import uuid
import zipfile
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Iterable, NoReturn

CHUNK_SIZE = 1024 * 1024
MANIFEST_NAME = "overlay-manifest.json"
CHECKSUMS_NAME = "overlay-checksums.txt"
CANONICAL_ROOTS = ("System", "Maps", "Textures", "Sounds", "Music", "Help")
PROHIBITED_EXTENSIONS = {
    ".bat",
    ".cmd",
    ".com",
    ".dll",
    ".drv",
    ".exe",
    ".lib",
    ".ocx",
    ".sys",
}
DEBRIS_NAMES = {
    ".ds_store",
    "desktop.ini",
    "mssccprj.scc",
    "thumbs.db",
    "vssver.scc",
    "ws_ftp.log",
}
CRITICAL_RETAIL_PATHS = {
    "System/Core.u",
    "System/Editor.u",
    "System/Engine.u",
    "System/Fire.u",
    "System/HGame.u",
    "System/HPModels.u",
    "System/HPParticle.u",
    "System/HProps.u",
    "System/HPSounds.u",
    "System/UnrealShare.u",
    "System/UWindow.u",
    "Textures/HP2_Master.utx",
    "Textures/HP2_Menu.utx",
    "Sounds/AllDialog.USA_uax",
}
RETAIL_ONLY_REQUIRED_PATHS = {
    "Maps/Entry.unr",
    "Textures/HP_Menu.utx",
}
RETAIL_USA_LOCALIZATION = {
    "System/BumpDialog.usa",
    "System/hpcredits.usa",
    "System/HpDialog.usa",
    "System/HpMenu.usa",
}
RETAIL_FINAL_LOCALIZATION = {
    "System/BumpDialog.int",
    "System/BumpSet.int",
    "System/Credits.int",
    "System/Duelset.int",
    "System/hpcredits.int",
    "System/hpdialog.int",
    "System/HPMenu.int",
    "System/QuidSet.int",
} | RETAIL_USA_LOCALIZATION

FULL_DEFAULT_MERGES = (
    "base: prototype System/Default.ini",
    "FrontEnd: retail EASplashWaitTime, WBSplashWaitTime, LaunchCode, UseSaveSlot",
    "Engine.Engine: XOpenGLDrv, SDLDrv, OpenAL, Language=usa",
    "Engine.GameEngine: FrameRateLimit=60 and no unavailable ServerActors",
    "Core.System: forward-slash portable relative data paths",
    "SDLDrv.SDLClient: native portable client defaults",
    "XOpenGLDrv.XOpenGLRenderDevice: OpenGL 4.1-safe feature set",
    "ALAudio.ALAudioSubsystem: retail final MusicVolume=0.53",
    "HGame console: retail final debug mode disabled",
)
FULL_DEFUSER_MERGES = (
    "base: prototype System/DefUser.ini, retaining prototype controller fallback",
    "Engine.Input: retail final keyboard/mouse broom, map, cutscene, potion, and duel bindings",
    "Engine.PlayerPawn: retail ObjectDetailMedium and Modern controls disabled by default",
    "HGame.Harry: retail final difficulty damage multipliers",
)
RETAIL_ONLY_DEFAULT_MERGES = (
    "base: retail System/default.ini; no prototype bytes",
    "FrontEnd: retail final splash, launch, and save-slot values",
    "Engine.Engine: XOpenGLDrv, SDLDrv, OpenAL, Language=usa",
    "Engine.GameEngine: FrameRateLimit=60 and no unavailable ServerActors",
    "Core.System: forward-slash portable relative data paths",
    "SDLDrv.SDLClient: native portable client defaults",
    "XOpenGLDrv.XOpenGLRenderDevice: OpenGL 4.1-safe feature set",
    "ALAudio.ALAudioSubsystem: retail final MusicVolume=0.53",
    "HGame console: retail final debug mode disabled",
)
RETAIL_ONLY_DEFUSER_MERGES = (
    "base: retail System/DefUser.ini; no prototype bytes",
    "Engine.Input: retail final keyboard/mouse broom, map, cutscene, potion, and duel bindings",
    "Engine.PlayerPawn: retail ObjectDetailMedium and Modern controls disabled by default",
    "HGame.Harry: retail final difficulty damage multipliers",
)
SAFE_DEFAULT_MERGES = (
    "base: prototype System/Default.ini",
    "Engine.Engine: XOpenGLDrv, SDLDrv, OpenAL, prototype Language=int",
    "Engine.GameEngine: FrameRateLimit=60 and no unavailable ServerActors",
    "Core.System: forward-slash portable relative data paths",
    "SDLDrv.SDLClient: native portable client defaults",
    "XOpenGLDrv.XOpenGLRenderDevice: OpenGL 4.1-safe feature set",
)
SAFE_DEFUSER_MERGES = (
    "base: prototype System/DefUser.ini with Modern controls disabled; prototype bindings retained",
)


class OverlayError(RuntimeError):
    """An overlay cannot be produced without violating its data contract."""


def error(message: str) -> NoReturn:
    raise OverlayError(message)


@dataclass(frozen=True)
class SourceAsset:
    source: str
    source_path: str
    disk_path: Path
    output_path: str
    size: int
    sha256: str


@dataclass(frozen=True)
class PlannedAsset:
    source: str
    source_path: str | None
    output_path: str
    classification: str
    size: int
    sha256: str
    link_mode: str
    disk_path: Path | None = None
    content: bytes | None = None
    source_paths: tuple[str, ...] = ()


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--prototype-root",
        type=Path,
        help="prototype Unreal root (required by safe/full; unused by retail-only)",
    )
    retail = parser.add_mutually_exclusive_group(required=True)
    retail.add_argument("--retail-root", type=Path, help="extracted retail installation root")
    retail.add_argument("--archive", type=Path, help="ZIP, TAR, or 7z archive containing the retail tree")
    parser.add_argument("--output", required=True, type=Path, help="normalized overlay directory")
    parser.add_argument(
        "--profile",
        choices=("safe", "full", "retail-only"),
        default="safe",
        help="safe prototype baseline, full union candidate, or pure retail installation data",
    )
    parser.add_argument(
        "--link-mode",
        choices=("auto", "copy", "hardlink"),
        default="auto",
        help="asset materialization (auto hardlinks on the same filesystem)",
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="validate output and manifest without changing any files",
    )
    return parser.parse_args(argv)


def validate_directory(path: Path, label: str) -> Path:
    try:
        if path.is_symlink():
            error(f"{label} must not be a symbolic link: {path}")
        resolved = path.expanduser().resolve(strict=True)
    except OSError as exc:
        error(f"cannot resolve {label} {path}: {exc}")
    if not resolved.is_dir():
        error(f"{label} is not a directory: {resolved}")
    return resolved


def validate_file(path: Path, label: str) -> Path:
    try:
        if path.is_symlink():
            error(f"{label} must not be a symbolic link: {path}")
        resolved = path.expanduser().resolve(strict=True)
    except OSError as exc:
        error(f"cannot resolve {label} {path}: {exc}")
    if not resolved.is_file():
        error(f"{label} is not a regular file: {resolved}")
    return resolved


def is_relative_to(path: Path, parent: Path) -> bool:
    try:
        path.relative_to(parent)
        return True
    except ValueError:
        return False


def find_child(directory: Path, wanted: str, required: bool = True) -> Path | None:
    try:
        matches = [entry for entry in directory.iterdir() if entry.name.casefold() == wanted.casefold()]
    except OSError as exc:
        error(f"cannot inspect {directory}: {exc}")
    if len(matches) > 1:
        error(f"ambiguous case-insensitive component {wanted!r} under {directory}")
    if not matches:
        if required:
            error(f"missing {wanted} directory under {directory}")
        return None
    match = matches[0]
    if match.is_symlink() or not match.is_dir():
        error(f"expected real directory for {wanted}: {match}")
    return match


def normalized_component(value: str) -> str:
    normalized = unicodedata.normalize("NFC", value)
    if not normalized or normalized in {".", ".."} or "/" in normalized or "\\" in normalized:
        error(f"unsafe path component: {value!r}")
    if "\0" in normalized:
        error("NUL in path component")
    return normalized


def output_path(root: str, relative: PurePosixPath) -> str:
    parts = [normalized_component(part) for part in relative.parts]
    if not parts:
        error(f"empty relative path under {root}")
    return "/".join((root, *parts))


def sorted_direct_files(directory: Path) -> list[Path]:
    found: list[Path] = []
    try:
        entries = sorted(directory.iterdir(), key=lambda path: (path.name.casefold(), path.name))
    except OSError as exc:
        error(f"cannot inspect {directory}: {exc}")
    for entry in entries:
        if entry.is_symlink():
            error(f"symbolic link in input tree: {entry}")
        if entry.is_file():
            found.append(entry)
        elif not entry.is_dir():
            error(f"unsupported input entry: {entry}")
    return found


def sorted_recursive_files(directory: Path) -> list[Path]:
    found: list[Path] = []
    for current, directory_names, file_names in os.walk(directory, followlinks=False):
        directory_names.sort(key=lambda value: (value.casefold(), value))
        file_names.sort(key=lambda value: (value.casefold(), value))
        current_path = Path(current)
        for name in directory_names:
            path = current_path / name
            if path.is_symlink():
                error(f"symbolic-link directory in input tree: {path}")
        for name in file_names:
            path = current_path / name
            if path.is_symlink() or not path.is_file():
                error(f"non-regular file in input tree: {path}")
            found.append(path)
    return sorted(
        found,
        key=lambda path: (
            path.relative_to(directory).as_posix().casefold(),
            path.relative_to(directory).as_posix(),
        ),
    )


def is_debris(path: Path) -> bool:
    name = path.name.casefold()
    suffix = path.suffix.casefold()
    return (
        name in DEBRIS_NAMES
        or suffix == ".scc"
        or name.endswith((".bak", ".old", ".orig", ".tmp", "~"))
        or name.startswith("unins")
    )


def is_prohibited_path(relative: str) -> bool:
    posix = PurePosixPath(relative)
    name = posix.name.casefold()
    suffix = posix.suffix.casefold()
    return (
        suffix in PROHIBITED_EXTENSIONS
        or is_debris(Path(posix.name))
        or any(part.casefold() in {".git", ".svn", "cvs", "__pycache__"} for part in posix.parts)
        or name.startswith("unins")
    )


def hash_file(path: Path) -> tuple[int, str]:
    digest = hashlib.sha256()
    size = 0
    try:
        with path.open("rb") as stream:
            while chunk := stream.read(CHUNK_SIZE):
                size += len(chunk)
                digest.update(chunk)
    except OSError as exc:
        error(f"cannot hash {path}: {exc}")
    return size, digest.hexdigest()


def source_asset(source: str, source_root: Path, disk_path: Path, destination: str) -> SourceAsset:
    if disk_path.is_symlink() or not disk_path.is_file():
        error(f"input asset is not a real regular file: {disk_path}")
    try:
        relative = disk_path.relative_to(source_root).as_posix()
    except ValueError:
        error(f"input asset escapes {source} root: {disk_path}")
    if is_prohibited_path(destination):
        error(f"prohibited asset selected for overlay: {destination}")
    size, digest = hash_file(disk_path)
    return SourceAsset(source, relative, disk_path, destination, size, digest)


def insert_asset(target: dict[str, SourceAsset], asset: SourceAsset) -> None:
    key = asset.output_path.casefold()
    prior = target.get(key)
    if prior is not None:
        error(
            f"case-insensitive output collision for {asset.output_path}: "
            f"{prior.source_path} and {asset.source_path}"
        )
    target[key] = asset


def collect_side(root: Path, source: str) -> dict[str, SourceAsset]:
    assets: dict[str, SourceAsset] = {}
    system = find_child(root, "System")
    maps = find_child(root, "Maps")
    textures = find_child(root, "Textures")
    sounds = find_child(root, "Sounds")
    music = find_child(root, "Music")
    help_directory = find_child(root, "Help")
    assert system and maps and textures and sounds and music and help_directory

    for path in sorted_direct_files(system):
        suffix = path.suffix.casefold()
        if suffix == ".u" or suffix in {".int", ".usa"}:
            insert_asset(
                assets,
                source_asset(source, root, path, output_path("System", PurePosixPath(path.name))),
            )

    for path in sorted_recursive_files(maps):
        if path.suffix.casefold() == ".unr":
            relative = PurePosixPath(path.relative_to(maps).as_posix())
            insert_asset(assets, source_asset(source, root, path, output_path("Maps", relative)))

    # Nested retail "Old textures" are redundant, search-path-invisible debris.  Prototype
    # packages are direct files, and prototype-only direct packages remain fallback content.
    for path in sorted_direct_files(textures):
        if path.suffix.casefold() == ".utx":
            insert_asset(
                assets,
                source_asset(source, root, path, output_path("Textures", PurePosixPath(path.name))),
            )

    for path in sorted_recursive_files(sounds):
        lower_name = path.name.casefold()
        if lower_name.endswith(".uax") or lower_name.endswith("_uax"):
            relative = PurePosixPath(path.relative_to(sounds).as_posix())
            insert_asset(assets, source_asset(source, root, path, output_path("Sounds", relative)))

    for path in sorted_recursive_files(music):
        if path.suffix.casefold() == ".ogg":
            relative = PurePosixPath(path.relative_to(music).as_posix())
            insert_asset(assets, source_asset(source, root, path, output_path("Music", relative)))

    # Only the canonical outer Help directory is eligible.  Retail help/help is an
    # installer-created duplicate root and must never become Help/Help.
    for path in sorted_direct_files(help_directory):
        if not is_debris(path) and path.suffix.casefold() not in PROHIBITED_EXTENSIONS:
            insert_asset(
                assets,
                source_asset(source, root, path, output_path("Help", PurePosixPath(path.name))),
            )

    return assets


def collect_retail_cutscenes(retail_root: Path, assets: dict[str, SourceAsset]) -> None:
    system = find_child(retail_root, "System")
    assert system
    cutscenes = find_child(system, "cutscenes")
    assert cutscenes
    for path in sorted_recursive_files(cutscenes):
        if path.suffix.casefold() != ".int":
            if not is_debris(path):
                error(f"unexpected retail cutscene payload: {path}")
            continue
        relative = PurePosixPath(path.relative_to(cutscenes).as_posix())
        insert_asset(
            assets,
            source_asset(
                "retail",
                retail_root,
                path,
                output_path("System", PurePosixPath("Cutscenes") / relative),
            ),
        )


def collect_retail_required_nested_textures(
    retail_root: Path,
    assets: dict[str, SourceAsset],
) -> None:
    # Retail HGame dynamically loads HP_Menu, but the installer leaves its only
    # physical provider below a search-path-invisible "Old textures" directory.
    textures = find_child(retail_root, "Textures")
    assert textures
    old_textures = find_child(textures, "Old textures")
    assert old_textures
    hp_menu = find_child_file(old_textures, "HP_Menu.utx")
    insert_asset(
        assets,
        source_asset(
            "retail",
            retail_root,
            hp_menu,
            "Textures/HP_Menu.utx",
        ),
    )



def collect_prototype_runtime_cutscenes(prototype_root: Path) -> dict[str, SourceAsset]:
    assets: dict[str, SourceAsset] = {}
    system = find_child(prototype_root, "System")
    assert system
    cutscenes = find_child(system, "cutscenes")
    assert cutscenes
    for path in sorted_recursive_files(cutscenes):
        if path.suffix.casefold() != ".int":
            if not is_debris(path):
                error(f"unexpected prototype runtime cutscene payload: {path}")
            continue
        relative = PurePosixPath(path.relative_to(cutscenes).as_posix())
        insert_asset(
            assets,
            source_asset(
                "prototype",
                prototype_root,
                path,
                output_path("System", PurePosixPath("Cutscenes") / relative),
            ),
        )
    return assets



def collect_prototype_cutscene_references(prototype_root: Path) -> dict[str, SourceAsset]:
    references: dict[str, SourceAsset] = {}
    cutscenes = find_child(prototype_root, "CutScenes", required=False)
    if cutscenes is None:
        return references
    for path in sorted_recursive_files(cutscenes):
        if path.suffix.casefold() != ".txt":
            continue
        relative = PurePosixPath(path.relative_to(cutscenes).as_posix()).with_suffix(".int")
        asset = source_asset(
            "prototype",
            prototype_root,
            path,
            output_path("System", PurePosixPath("Cutscenes") / relative),
        )
        insert_asset(references, asset)
    return references


def decode_ini(path: Path) -> list[str]:
    try:
        data = path.read_bytes()
    except OSError as exc:
        error(f"cannot read config {path}: {exc}")
    if data.startswith((b"\xff\xfe", b"\xfe\xff")):
        error(f"UTF-16 config is unsupported for deterministic merge: {path}")
    try:
        text = data.decode("utf-8-sig")
    except UnicodeDecodeError as exc:
        error(f"config is not UTF-8/ASCII {path}: {exc}")
    return text.splitlines()


def section_span(lines: list[str], section: str) -> tuple[int, int] | None:
    wanted = section.casefold()
    start = None
    for index, raw in enumerate(lines):
        stripped = raw.strip()
        if stripped.startswith("[") and stripped.endswith("]"):
            name = stripped[1:-1].strip().casefold()
            if start is not None:
                return start, index
            if name == wanted:
                start = index
    if start is not None:
        return start, len(lines)
    return None


def ensure_section(lines: list[str], section: str) -> tuple[int, int]:
    span = section_span(lines, section)
    if span is not None:
        return span
    while lines and not lines[-1].strip():
        lines.pop()
    if lines:
        lines.append("")
    lines.append(f"[{section}]")
    return len(lines) - 1, len(lines)


def remove_ini_section(lines: list[str], section: str) -> None:
    span = section_span(lines, section)
    if span is None:
        return
    start, end = span
    while start > 0 and not lines[start - 1].strip():
        start -= 1
    del lines[start:end]


def set_ini_value(lines: list[str], section: str, key: str, value: str) -> None:
    start, end = ensure_section(lines, section)
    wanted = key.casefold()
    hits: list[int] = []
    for index in range(start + 1, end):
        stripped = lines[index].lstrip()
        if not stripped or stripped.startswith((";", "#")) or "=" not in stripped:
            continue
        candidate = stripped.split("=", 1)[0].strip().casefold()
        if candidate == wanted:
            hits.append(index)
    if len(hits) > 1:
        error(f"ambiguous scalar config value [{section}] {key}")
    replacement = f"{key}={value}"
    if hits:
        lines[hits[0]] = replacement
    else:
        insert_at = end
        while insert_at > start + 1 and not lines[insert_at - 1].strip():
            insert_at -= 1
        lines.insert(insert_at, replacement)


def remove_ini_key(lines: list[str], section: str, key: str, all_values: bool = False) -> None:
    span = section_span(lines, section)
    if span is None:
        return
    start, end = span
    wanted = key.casefold()
    hits = []
    for index in range(start + 1, end):
        stripped = lines[index].lstrip()
        if stripped and not stripped.startswith((";", "#")) and "=" in stripped:
            if stripped.split("=", 1)[0].strip().casefold() == wanted:
                hits.append(index)
    if len(hits) > 1 and not all_values:
        error(f"ambiguous scalar config value [{section}] {key}")
    for index in reversed(hits):
        del lines[index]


def ini_value(lines: list[str], section: str, key: str) -> str:
    span = section_span(lines, section)
    if span is None:
        error(f"missing config section [{section}]")
    start, end = span
    wanted = key.casefold()
    values = []
    for raw in lines[start + 1 : end]:
        stripped = raw.lstrip()
        if stripped and not stripped.startswith((";", "#")) and "=" in stripped:
            candidate, value = stripped.split("=", 1)
            if candidate.strip().casefold() == wanted:
                values.append(value.strip())
    if len(values) != 1:
        error(f"expected one [{section}] {key}, found {len(values)}")
    return values[0]


def retail_ini_value(retail_lines: list[str], section: str, key: str) -> str:
    return ini_value(retail_lines, section, key)


def encode_ini(lines: list[str]) -> bytes:
    while lines and not lines[-1].strip():
        lines.pop()
    return "\r\n".join(lines).encode("utf-8")


def build_default_ini(template_path: Path, retail_path: Path, profile: str) -> bytes:
    lines = decode_ini(template_path)
    retail_lines = decode_ini(retail_path)

    if profile in {"full", "retail-only"}:
        remove_ini_key(lines, "Engine.Engine", "EASplashWaitTime")
        remove_ini_key(lines, "Engine.Engine", "WBSplashWaitTime")
        for key in ("EASplashWaitTime", "WBSplashWaitTime", "LaunchCode", "UseSaveSlot"):
            set_ini_value(lines, "FrontEnd", key, retail_ini_value(retail_lines, "FrontEnd", key))
    if profile == "retail-only":
        remove_ini_section(lines, "D3D11Drv.D3D11RenderDevice")
        remove_ini_section(lines, "Editor.EditorEngine")

    native_engine = {
        "GameRenderDevice": "XOpenGLDrv.XOpenGLRenderDevice",
        "AudioDevice": "ALAudio.ALAudioSubsystem",
        "WindowedRenderDevice": "XOpenGLDrv.XOpenGLRenderDevice",
        "RenderDevice": "XOpenGLDrv.XOpenGLRenderDevice",
        "ViewportManager": "SDLDrv.SDLClient",
        "Language": "usa" if profile in {"full", "retail-only"} else "int",
    }
    for key, value in native_engine.items():
        set_ini_value(lines, "Engine.Engine", key, value)

    set_ini_value(lines, "Engine.GameEngine", "FrameRateLimit", "60.000000")
    remove_ini_key(lines, "Engine.GameEngine", "ServerActors", all_values=True)

    portable_paths = {
        "SavePath": "../Save",
        "CachePath": "../Cache",
    }
    for key, value in portable_paths.items():
        set_ini_value(lines, "Core.System", key, value)
    expected_paths = [
        "../System/*.u",
        "../Maps/*.unr",
        "../Textures/*.utx",
        "../Sounds/*.uax",
        "../Music/*.umx",
        "../save/*.usa",
    ]
    span = section_span(lines, "Core.System")
    if span is None:
        error("Default.ini template has no [Core.System]")
    start, end = span
    path_indices = []
    for index in range(start + 1, end):
        stripped = lines[index].lstrip()
        if stripped and not stripped.startswith((";", "#")) and "=" in stripped:
            if stripped.split("=", 1)[0].strip().casefold() == "paths":
                path_indices.append(index)
    for index in reversed(path_indices):
        del lines[index]
    span = section_span(lines, "Core.System")
    assert span is not None
    insert_at = span[1]
    while insert_at > span[0] + 1 and not lines[insert_at - 1].strip():
        insert_at -= 1
    for value in expected_paths:
        lines.insert(insert_at, f"Paths={value}")
        insert_at += 1

    sdl_values = {
        "WindowedViewportX": "800",
        "WindowedViewportY": "600",
        "WindowedColorBits": "32",
        "FullscreenViewportX": "800",
        "FullscreenViewportY": "600",
        "FullscreenColorBits": "32",
        "Brightness": "0.400000",
        "UseJoystick": "True",
        "CaptureMouse": "True",
        "StartupFullscreen": "False",
        "BorderlessWindow": "False",
        "UseDesktopResolution": "False",
        "AllowUnicodeKeys": "True",
        "NativeText": "True",
        "AllowCommandQKeys": "True",
        "MacKeepAllScreensOn": "False",
    }
    for key, value in sdl_values.items():
        set_ini_value(lines, "SDLDrv.SDLClient", key, value)

    xopengl_values = {
        "OpenGLVersion": "Core",
        "UseBindlessTextures": "False",
        "UsePersistentBuffers": "False",
        "UseShaderDrawParameters": "False",
        "UseBufferInvalidation": "False",
        "UseShaderCache": "False",
        "DetailTextures": "True",
        "MacroTextures": "True",
        "UseTrilinear": "True",
        "UsePrecache": "True",
        "UseVSync": "Off",
    }
    for key, value in xopengl_values.items():
        set_ini_value(lines, "XOpenGLDrv.XOpenGLRenderDevice", key, value)

    if profile in {"full", "retail-only"}:
        set_ini_value(
            lines,
            "ALAudio.ALAudioSubsystem",
            "MusicVolume",
            retail_ini_value(retail_lines, "ALAudio.ALAudioSubsystem", "MusicVolume"),
        )
        set_ini_value(lines, "HGame.baseConsole", "bDebugMode", "False")
        set_ini_value(lines, "HGame.HPConsole", "bDebugMode", "False")

    validate_default_ini(lines, profile)
    return encode_ini(lines)


def build_defuser_ini(template_path: Path, retail_path: Path, profile: str) -> bytes:
    lines = decode_ini(template_path)
    set_ini_value(lines, "Engine.PlayerPawn", "bModernThirdPersonControls", "False")
    if profile == "safe":
        return encode_ini(lines)
    retail_lines = decode_ini(retail_path)
    input_keys = (
        "Aliases[8]",
        "RightMouse",
        "LeftMouse",
        "Tab",
        "Enter",
        "Ctrl",
        "Alt",
        "Space",
        "Z",
        "Slash",
        "A",
    )
    for key in input_keys:
        set_ini_value(lines, "Engine.Input", key, retail_ini_value(retail_lines, "Engine.Input", key))
    set_ini_value(
        lines,
        "Engine.PlayerPawn",
        "ObjectDetail",
        retail_ini_value(retail_lines, "Engine.PlayerPawn", "ObjectDetail"),
    )
    for key in (
        "fDamageMultiplier_Easy",
        "fDamageMultiplier_Medium",
        "fDamageMultiplier_Hard",
    ):
        set_ini_value(lines, "HGame.Harry", key, retail_ini_value(retail_lines, "HGame.Harry", key))

    if profile == "full":
        # The full union keeps the prototype's working controller defaults.
        for key in ("Joy1", "Joy2", "Joy3", "Joy4", "JoyY", "JoyU"):
            expected = ini_value(decode_ini(template_path), "Engine.Input", key)
            if ini_value(lines, "Engine.Input", key) != expected:
                error(f"controller fallback was not preserved: [Engine.Input] {key}")

    validate_defuser_ini(lines, retail_lines)
    return encode_ini(lines)


def validate_default_ini(lines: list[str], profile: str) -> None:
    expected = {
        ("Engine.Engine", "GameRenderDevice"): "XOpenGLDrv.XOpenGLRenderDevice",
        ("Engine.Engine", "WindowedRenderDevice"): "XOpenGLDrv.XOpenGLRenderDevice",
        ("Engine.Engine", "RenderDevice"): "XOpenGLDrv.XOpenGLRenderDevice",
        ("Engine.Engine", "ViewportManager"): "SDLDrv.SDLClient",
        ("Engine.Engine", "AudioDevice"): "ALAudio.ALAudioSubsystem",
        ("Engine.Engine", "Language"): "usa" if profile in {"full", "retail-only"} else "int",
        ("Engine.GameEngine", "FrameRateLimit"): "60.000000",
        ("SDLDrv.SDLClient", "NativeText"): "True",
    }
    for (section, key), value in expected.items():
        actual = ini_value(lines, section, key)
        if actual.casefold() != value.casefold():
            error(f"non-portable Default.ini [{section}] {key}={actual}")
    text = "\n".join(lines)
    if "d3d11" in text.casefold():
        error("D3D11 leaked into generated Default.ini")
    malformed_tuple_keys = {"x", "y", "z", "pitch", "yaw", "roll", "r", "g", "b", "a"}
    for raw in lines:
        stripped = raw.strip()
        if "=" in stripped and stripped.split("=", 1)[0].strip().casefold() in malformed_tuple_keys:
            error(f"malformed retail tuple fragment leaked into Default.ini: {raw}")
    span = section_span(lines, "Core.System")
    assert span is not None
    for raw in lines[span[0] + 1 : span[1]]:
        stripped = raw.strip()
        if stripped.casefold().startswith(("paths=", "savepath=", "cachepath=")) and "\\" in stripped:
            error(f"Windows path separator in generated Default.ini: {raw}")
    if section_span(lines, "Engine.GameEngine"):
        start, end = section_span(lines, "Engine.GameEngine") or (0, 0)
        if any(raw.lstrip().casefold().startswith("serveractors=") for raw in lines[start + 1 : end]):
            error("unavailable ServerActors leaked into generated Default.ini")


def validate_defuser_ini(lines: list[str], retail_lines: list[str]) -> None:
    for key in (
        "Aliases[8]",
        "RightMouse",
        "LeftMouse",
        "Tab",
        "Enter",
        "Ctrl",
        "Alt",
        "Space",
        "Z",
        "Slash",
        "A",
    ):
        if ini_value(lines, "Engine.Input", key) != ini_value(retail_lines, "Engine.Input", key):
            error(f"retail final binding was not merged: [Engine.Input] {key}")
    if ini_value(lines, "Engine.PlayerPawn", "bModernThirdPersonControls") != "False":
        error("Modern controls must default to False")
    if ini_value(lines, "Engine.PlayerPawn", "ObjectDetail") != "ObjectDetailMedium":
        error("retail ObjectDetailMedium was not merged")
    expected_damage = {"fDamageMultiplier_Easy": "1.2", "fDamageMultiplier_Medium": "2.0", "fDamageMultiplier_Hard": "3.0"}
    for key, value in expected_damage.items():
        if ini_value(lines, "HGame.Harry", key) != value:
            error(f"retail difficulty value was not merged: {key}")


def link_mode_for(path: Path, output_parent: Path, requested: str) -> str:
    if requested == "copy":
        return "copy"
    try:
        same_device = path.stat().st_dev == output_parent.stat().st_dev
    except OSError as exc:
        error(f"cannot determine link mode for {path}: {exc}")
    if requested == "hardlink" and not same_device:
        error(f"cannot hardlink across filesystems: {path} -> {output_parent}")
    return "hardlink" if same_device else "copy"


def generated_asset(
    path: str,
    content: bytes,
    source_paths: tuple[str, ...],
    classification: str,
) -> PlannedAsset:
    return PlannedAsset(
        source="generated",
        source_path=None,
        output_path=path,
        classification=classification,
        size=len(content),
        sha256=hashlib.sha256(content).hexdigest(),
        link_mode="generated",
        content=content,
        source_paths=source_paths,
    )


def plan_overlay(
    prototype_root: Path | None,
    retail_root: Path,
    output_parent: Path,
    requested_link_mode: str,
    profile: str,
) -> list[PlannedAsset]:
    retail = collect_side(retail_root, "retail")
    planned: list[PlannedAsset] = []

    def append_source(chosen: SourceAsset, destination: str, classification: str) -> None:
        planned.append(
            PlannedAsset(
                source=chosen.source,
                source_path=chosen.source_path,
                output_path=destination,
                classification=classification,
                size=chosen.size,
                sha256=chosen.sha256,
                link_mode=link_mode_for(chosen.disk_path, output_parent, requested_link_mode),
                disk_path=chosen.disk_path,
            )
        )

    if profile == "retail-only":
        collect_retail_cutscenes(retail_root, retail)
        collect_retail_required_nested_textures(retail_root, retail)
        canonical_paths = {
            path.casefold(): path
            for path in CRITICAL_RETAIL_PATHS
            | RETAIL_FINAL_LOCALIZATION
            | RETAIL_ONLY_REQUIRED_PATHS
        }
        for key in sorted(retail):
            asset = retail[key]
            destination = canonical_paths.get(asset.output_path.casefold(), asset.output_path)
            append_source(asset, destination, "retail_asset")
    else:
        assert prototype_root is not None
        prototype = collect_side(prototype_root, "prototype")
        prototype_cutscene_references = collect_prototype_cutscene_references(prototype_root)
        prototype_runtime_cutscenes = collect_prototype_runtime_cutscenes(prototype_root)
        if profile == "full":
            collect_retail_cutscenes(retail_root, retail)
            prototype_comparison = dict(prototype)
            for key, asset in prototype_cutscene_references.items():
                if key not in prototype_comparison:
                    prototype_comparison[key] = asset
            for key in sorted(set(prototype) | set(retail)):
                retail_asset = retail.get(key)
                prototype_asset = prototype.get(key)
                if retail_asset is not None:
                    comparison = prototype_comparison.get(key)
                    if comparison is None:
                        classification = "retail_only"
                    elif comparison.sha256 == retail_asset.sha256:
                        classification = "identical"
                    else:
                        classification = "changed"
                    destination = (
                        comparison.output_path if comparison is not None else retail_asset.output_path
                    )
                    append_source(retail_asset, destination, classification)
                else:
                    assert prototype_asset is not None
                    append_source(prototype_asset, prototype_asset.output_path, "prototype_only")
        else:
            # The retail Engine/HGame class surface is not yet native-compatible.  Keep the
            # proven prototype runtime as one coherent baseline and import only isolated,
            # load-safe retail data: music and the retail-only HP2 menu texture package.
            for key in sorted(prototype):
                asset = prototype[key]
                if not asset.output_path.casefold().startswith("music/"):
                    append_source(asset, asset.output_path, "prototype_baseline")
            for key in sorted(prototype_runtime_cutscenes):
                asset = prototype_runtime_cutscenes[key]
                append_source(asset, asset.output_path, "prototype_baseline")
            for key in sorted(retail):
                asset = retail[key]
                if not asset.output_path.casefold().startswith("music/"):
                    continue
                comparison = prototype.get(key)
                if comparison is None:
                    classification = "retail_only"
                    destination = asset.output_path
                else:
                    classification = "identical" if comparison.sha256 == asset.sha256 else "changed"
                    destination = comparison.output_path
                append_source(asset, destination, classification)
            hp2_menu = retail.get("textures/hp2_menu.utx")
            if hp2_menu is None:
                error("safe profile requires retail Textures/HP2_Menu.utx")
            append_source(hp2_menu, "Textures/HP2_Menu.utx", "retail_only")

    retail_system = find_child(retail_root, "System")
    assert retail_system
    retail_default = find_child_file(retail_system, "Default.ini")
    retail_defuser = find_child_file(retail_system, "DefUser.ini")
    if profile == "retail-only":
        default_template = retail_default
        defuser_template = retail_defuser
        config_classification = "generated_retail_native"
        default_sources = (f"retail:{retail_default.relative_to(retail_root).as_posix()}",)
        defuser_sources = (f"retail:{retail_defuser.relative_to(retail_root).as_posix()}",)
    else:
        assert prototype_root is not None
        prototype_system = find_child(prototype_root, "System")
        assert prototype_system
        default_template = find_child_file(prototype_system, "Default.ini")
        defuser_template = find_child_file(prototype_system, "DefUser.ini")
        config_classification = "generated_full_merge" if profile == "full" else "generated_safe_native"
        default_sources = (
            ("prototype:System/Default.ini", "retail:System/default.ini")
            if profile == "full"
            else ("prototype:System/Default.ini",)
        )
        defuser_sources = (
            ("prototype:System/DefUser.ini", "retail:System/DefUser.ini")
            if profile == "full"
            else ("prototype:System/DefUser.ini",)
        )
    planned.extend(
        (
            generated_asset(
                "System/Default.ini",
                build_default_ini(default_template, retail_default, profile),
                default_sources,
                config_classification,
            ),
            generated_asset(
                "System/DefUser.ini",
                build_defuser_ini(defuser_template, retail_defuser, profile),
                defuser_sources,
                config_classification,
            ),
        )
    )
    planned.sort(key=lambda asset: (asset.output_path.casefold(), asset.output_path))
    validate_plan(planned, profile)
    return planned


def find_child_file(directory: Path, wanted: str) -> Path:
    try:
        matches = [entry for entry in directory.iterdir() if entry.name.casefold() == wanted.casefold()]
    except OSError as exc:
        error(f"cannot inspect {directory}: {exc}")
    if len(matches) != 1:
        error(f"expected exactly one {wanted} under {directory}, found {len(matches)}")
    path = matches[0]
    if path.is_symlink() or not path.is_file():
        error(f"expected real config file: {path}")
    return path


def validate_plan(planned: list[PlannedAsset], profile: str) -> None:
    by_key: dict[str, PlannedAsset] = {}
    for asset in planned:
        key = asset.output_path.casefold()
        if key in by_key:
            error(f"case-insensitive planned output collision: {by_key[key].output_path} and {asset.output_path}")
        by_key[key] = asset
        parts = PurePosixPath(asset.output_path).parts
        if not parts or parts[0] not in CANONICAL_ROOTS or is_prohibited_path(asset.output_path):
            error(f"invalid planned output path: {asset.output_path}")

    exact = {asset.output_path: asset for asset in planned}
    exact_casefold = {path.casefold(): asset for path, asset in exact.items()}

    def require(path: str, source: str) -> PlannedAsset:
        asset = exact.get(path)
        if asset is None:
            wrong_case = exact_casefold.get(path.casefold())
            if wrong_case is not None:
                error(
                    f"payload has incorrect canonical case: "
                    f"{wrong_case.output_path}, expected {path}"
                )
            error(f"missing required payload: {path}")
        if asset.source != source:
            error(f"{path} must be {source}-owned, found {asset.source}")
        return asset

    retail_maps = [
        asset
        for asset in planned
        if asset.source == "retail"
        and asset.output_path.casefold().startswith("maps/")
        and asset.output_path.casefold().endswith(".unr")
    ]
    prototype_maps = [
        asset
        for asset in planned
        if asset.source == "prototype"
        and asset.output_path.casefold().startswith("maps/")
        and asset.output_path.casefold().endswith(".unr")
    ]
    retail_oggs = [
        asset
        for asset in planned
        if asset.source == "retail"
        and asset.output_path.casefold().startswith("music/")
        and asset.output_path.casefold().endswith(".ogg")
    ]
    if len(retail_oggs) != 141:
        error(f"retail Ogg contract mismatch: expected 141, found {len(retail_oggs)}")

    if profile == "full":
        cutscenes = [
            asset
            for asset in planned
            if asset.source == "retail"
            and asset.output_path.casefold().startswith("system/cutscenes/")
            and asset.output_path.casefold().endswith(".int")
        ]
        if len(retail_maps) != 42 or len(prototype_maps) != 58:
            error(f"full map contract mismatch: retail={len(retail_maps)}, prototype-only={len(prototype_maps)}")
        if any(asset.classification != "prototype_only" for asset in prototype_maps):
            error("a full-profile prototype fallback map is not classified prototype_only")
        if len(cutscenes) != 214:
            error(f"full retail cutscene contract mismatch: expected 214, found {len(cutscenes)}")
        for path in sorted(CRITICAL_RETAIL_PATHS | RETAIL_FINAL_LOCALIZATION):
            require(path, "retail")
    elif profile == "retail-only":
        cutscenes = [
            asset
            for asset in planned
            if asset.source == "retail"
            and asset.output_path.casefold().startswith("system/cutscenes/")
            and asset.output_path.casefold().endswith(".int")
        ]
        if len(retail_maps) != 42 or prototype_maps:
            error(
                f"retail-only map contract mismatch: "
                f"retail={len(retail_maps)}, prototype={len(prototype_maps)}"
            )
        if len(cutscenes) != 214:
            error(f"retail-only cutscene contract mismatch: expected 214, found {len(cutscenes)}")
        for asset in planned:
            if asset.source not in {"retail", "generated"}:
                error(
                    f"retail-only payload has forbidden source {asset.source}: "
                    f"{asset.output_path}"
                )
            if asset.source == "retail" and asset.classification != "retail_asset":
                error(f"retail-only asset has invalid classification: {asset.output_path}")
            if asset.source == "generated":
                if asset.output_path not in {"System/Default.ini", "System/DefUser.ini"}:
                    error(f"unexpected generated retail-only payload: {asset.output_path}")
                if asset.classification != "generated_retail_native":
                    error(f"generated config is not marked retail-native: {asset.output_path}")
                if any(not path.startswith("retail:") for path in asset.source_paths):
                    error(f"generated config has a non-retail template: {asset.output_path}")
        for path in sorted(
            CRITICAL_RETAIL_PATHS
            | RETAIL_FINAL_LOCALIZATION
            | RETAIL_ONLY_REQUIRED_PATHS
            | {"System/Default.ini", "System/DefUser.ini"}
        ):
            require(path, "generated" if path.endswith(".ini") and "/" in path else "retail")

        default_asset = exact["System/Default.ini"]
        assert default_asset.content is not None
        default_lines = default_asset.content.decode("utf-8").splitlines()
        path_span = section_span(default_lines, "Core.System")
        assert path_span is not None
        configured_paths = []
        for raw in default_lines[path_span[0] + 1 : path_span[1]]:
            stripped = raw.lstrip()
            if stripped and not stripped.startswith((";", "#")) and "=" in stripped:
                key, value = stripped.split("=", 1)
                if key.strip().casefold() == "paths":
                    configured_paths.append(value.strip())
        expected_paths = {
            "../System/*.u",
            "../Maps/*.unr",
            "../Textures/*.utx",
            "../Sounds/*.uax",
            "../Music/*.umx",
            "../save/*.usa",
        }
        if set(configured_paths) != expected_paths or len(configured_paths) != len(expected_paths):
            error(f"retail-only configured Paths mismatch: {configured_paths}")
        category_extensions = {
            "System": (".u",),
            "Maps": (".unr",),
            "Textures": (".utx",),
            "Sounds": (".uax", "_uax"),
            "Music": (".ogg",),
        }
        for root, extensions in category_extensions.items():
            category_assets = [
                asset
                for asset in planned
                if asset.source == "retail"
                and asset.output_path.startswith(f"{root}/")
                and asset.output_path.casefold().endswith(extensions)
            ]
            if not category_assets:
                error(f"configured retail-only Paths category has no payload: {root}")
        if not retail_maps or not retail_oggs:
            error("retail-only requires at least one map and one music file")
        help_assets = [
            asset
            for asset in planned
            if asset.source == "retail" and asset.output_path.startswith("Help/")
        ]
        if not help_assets:
            error("retail-only requires canonical retail Help payload")
    else:
        cutscenes = [
            asset
            for asset in planned
            if asset.source == "prototype"
            and asset.output_path.casefold().startswith("system/cutscenes/")
            and asset.output_path.casefold().endswith(".int")
        ]
        if retail_maps or len(prototype_maps) != 100:
            error(f"safe map contract mismatch: retail={len(retail_maps)}, prototype={len(prototype_maps)}")
        if any(asset.classification != "prototype_baseline" for asset in prototype_maps):
            error("a safe-profile map is not classified prototype_baseline")
        if len(cutscenes) != 185:
            error(f"safe prototype cutscene contract mismatch: expected 185, found {len(cutscenes)}")
        if any(
            asset.source == "retail"
            for asset in planned
            if asset.output_path.casefold().startswith(("system/", "maps/", "sounds/", "help/"))
        ):
            error("safe profile contains retail runtime packages, maps, sounds, localization, or Help")
        prototype_textures = [
            asset
            for asset in planned
            if asset.source == "prototype" and asset.output_path.casefold().startswith("textures/")
        ]
        retail_textures = [
            asset
            for asset in planned
            if asset.source == "retail" and asset.output_path.casefold().startswith("textures/")
        ]
        if len(prototype_textures) != 52 or [asset.output_path for asset in retail_textures] != ["Textures/HP2_Menu.utx"]:
            error(
                f"safe texture contract mismatch: prototype={len(prototype_textures)}, "
                f"retail={[asset.output_path for asset in retail_textures]}"
            )
        for path in sorted(
            {
                "System/Core.u",
                "System/Editor.u",
                "System/Engine.u",
                "System/Fire.u",
                "System/HGame.u",
                "System/HPModels.u",
                "System/HPParticle.u",
                "System/HProps.u",
                "System/HPSounds.u",
                "System/UnrealShare.u",
                "System/UWindow.u",
                "Textures/HP2_Master.utx",
                "Sounds/AllDialog.uax",
            }
        ):
            require(path, "prototype")
        require("Textures/HP2_Menu.utx", "retail")
        if "Sounds/AllDialog.USA_uax" in exact or any(path.casefold().endswith(".usa") for path in exact):
            error("safe profile must use prototype INT/AllDialog localization, not retail USA")


def manifest_bytes(planned: list[PlannedAsset], profile: str) -> bytes:
    entries = []
    for asset in planned:
        entry: dict[str, object] = {
            "classification": asset.classification,
            "link_mode": asset.link_mode,
            "path": asset.output_path,
            "sha256": asset.sha256,
            "size": asset.size,
            "source": asset.source,
        }
        if asset.source_path is not None:
            entry["source_path"] = asset.source_path
        if asset.source_paths:
            entry["source_paths"] = list(asset.source_paths)
        entries.append(entry)

    def counts(attribute: str) -> dict[str, int]:
        counter = collections.Counter(str(getattr(asset, attribute)) for asset in planned)
        return dict(sorted(counter.items()))

    root_counts = collections.Counter(PurePosixPath(asset.output_path).parts[0] for asset in planned)
    if profile == "full":
        default_merges = FULL_DEFAULT_MERGES
        defuser_merges = FULL_DEFUSER_MERGES
        policy = {
            "compatibility": "candidate retail class/package surface; requires complete retail native parity",
            "cutscenes": "complete retail System/Cutscenes bundle; no prototype raw TXT mixing",
            "help": "canonical outer Help only; nested help/help excluded",
            "localization": "retail coherent USA bundle with prototype-only INT fallback",
            "precedence": "retail changed/identical/retail-only runtime data, then prototype-only fallback",
        }
    elif profile == "retail-only":
        default_merges = RETAIL_ONLY_DEFAULT_MERGES
        defuser_merges = RETAIL_ONLY_DEFUSER_MERGES
        policy = {
            "compatibility": "final retail package/map surface with no prototype fallback",
            "cutscenes": "complete retail System/Cutscenes bundle",
            "help": "canonical outer retail Help only; nested help/help excluded",
            "localization": "coherent retail Language=usa and AllDialog.USA_uax bundle",
            "precedence": "retail files only; portable configs are generated from retail templates",
        }
    else:
        default_merges = SAFE_DEFAULT_MERGES
        defuser_merges = SAFE_DEFUSER_MERGES
        policy = {
            "compatibility": "native-smokeable prototype class/package baseline",
            "cutscenes": "compiled prototype System/Cutscenes INT runtime bundle",
            "help": "prototype canonical outer Help only",
            "localization": "coherent prototype Language=int and AllDialog.uax bundle",
            "precedence": "prototype runtime data; retail Music Oggs and retail-only HP2_Menu.utx only",
        }
    policy["prohibited"] = "Windows binaries/installers/scripts, source-control metadata, and debris"
    provenance = None
    if profile == "retail-only":
        provenance = {
            "asset_byte_sources": ["retail"],
            "generated_entries": [
                asset.output_path for asset in planned if asset.source == "generated"
            ],
            "manifest": "generated deterministically from the validated retail-only plan",
            "prototype_asset_entry_count": sum(
                1 for asset in planned if asset.source == "prototype"
            ),
            "prototype_template_input_count": sum(
                1
                for asset in planned
                if any(path.startswith("prototype:") for path in asset.source_paths)
            ),
        }
    manifest = {
        "config_merges": {
            "System/Default.ini": list(default_merges),
            "System/DefUser.ini": list(defuser_merges),
        },
        "entries": entries,
        "format": "hp2-retail-data-overlay",
        "policy": policy,
        "profile": profile,
        "schema_version": 3 if profile == "retail-only" else 2,
        "summary": {
            "classification": counts("classification"),
            "entry_count": len(planned),
            "link_mode": counts("link_mode"),
            "root": dict(sorted(root_counts.items())),
            "source": counts("source"),
        },
    }
    if provenance is not None:
        manifest["provenance"] = provenance
    return (json.dumps(manifest, ensure_ascii=False, indent=2, sort_keys=True) + "\n").encode("utf-8")


def checksums_bytes(planned: list[PlannedAsset]) -> bytes:
    """Render the compact sidecar index: '<path>\t<size>\t<sha256>' lines.

    Sorted by path bytes (not casefolded), LF-terminated, with a trailing
    newline. The runtime bootstrap consumes this file to stream-verify every
    overlay entry without trusting the JSON manifest alone.
    """
    rows = sorted(planned, key=lambda asset: asset.output_path.encode("utf-8"))
    lines = [
        f"{asset.output_path}\t{asset.size}\t{asset.sha256}" for asset in rows
    ]
    if not lines:
        return b""
    return ("\n".join(lines) + "\n").encode("utf-8")


def hash_bytes(path: Path) -> tuple[int, str]:
    return hash_file(path)


def walk_output(output: Path) -> dict[str, Path]:
    if output.is_symlink() or not output.is_dir():
        error(f"output is not a real directory: {output}")
    found: dict[str, Path] = {}
    for current, directory_names, file_names in os.walk(output, followlinks=False):
        directory_names.sort(key=lambda value: (value.casefold(), value))
        file_names.sort(key=lambda value: (value.casefold(), value))
        current_path = Path(current)
        for name in directory_names:
            path = current_path / name
            if path.is_symlink():
                error(f"symbolic link in output (dangling links are forbidden): {path}")
        for name in file_names:
            path = current_path / name
            relative = path.relative_to(output).as_posix()
            if path.is_symlink():
                error(f"symbolic link in output (dangling links are forbidden): {path}")
            try:
                mode = path.stat().st_mode
            except OSError as exc:
                error(f"cannot stat output {path}: {exc}")
            if not stat.S_ISREG(mode):
                error(f"non-regular output entry: {path}")
            key = relative.casefold()
            if key in found:
                error(f"case-insensitive output collision: {found[key]} and {path}")
            if is_prohibited_path(relative):
                error(f"prohibited output payload: {relative}")
            try:
                with path.open("rb") as stream:
                    if stream.read(2) == b"MZ":
                        error(f"Windows PE payload disguised in output: {relative}")
            except OSError as exc:
                error(f"cannot inspect output {path}: {exc}")
            found[key] = path
    return found


def validate_output(output: Path, planned: list[PlannedAsset], expected_manifest: bytes) -> None:
    found = walk_output(output)
    expected_paths = {asset.output_path.casefold(): asset for asset in planned}
    expected_paths[MANIFEST_NAME.casefold()] = None  # type: ignore[assignment]
    expected_paths[CHECKSUMS_NAME.casefold()] = None  # type: ignore[assignment]
    missing = sorted(key for key in expected_paths if key not in found)
    extra = sorted(key for key in found if key not in expected_paths)
    if missing or extra:
        error(f"output path set mismatch: missing={missing}, extra={extra}")
    manifest_path = found[MANIFEST_NAME.casefold()]
    manifest_relative = manifest_path.relative_to(output).as_posix()
    if manifest_relative != MANIFEST_NAME:
        error(
            f"manifest has incorrect canonical case: {manifest_relative}, "
            f"expected {MANIFEST_NAME}"
        )
    checksums_path = found[CHECKSUMS_NAME.casefold()]
    checksums_relative = checksums_path.relative_to(output).as_posix()
    if checksums_relative != CHECKSUMS_NAME:
        error(
            f"checksums index has incorrect canonical case: {checksums_relative}, "
            f"expected {CHECKSUMS_NAME}"
        )
    for asset in planned:
        path = found[asset.output_path.casefold()]
        actual_relative = path.relative_to(output).as_posix()
        if actual_relative != asset.output_path:
            error(
                f"output has incorrect canonical case: {actual_relative}, "
                f"expected {asset.output_path}"
            )


    try:
        actual_manifest = manifest_path.read_bytes()
    except OSError as exc:
        error(f"cannot read manifest {manifest_path}: {exc}")
    if actual_manifest != expected_manifest:
        error(f"manifest is out of date or non-deterministic: {manifest_path}")

    try:
        actual_checksums = checksums_path.read_bytes()
    except OSError as exc:
        error(f"cannot read checksums index {checksums_path}: {exc}")
    if actual_checksums != checksums_bytes(planned):
        error(f"checksums index is out of date or non-deterministic: {checksums_path}")

    for asset in planned:
        path = found[asset.output_path.casefold()]
        size, digest = hash_bytes(path)
        if size != asset.size or digest != asset.sha256:
            error(
                f"output hash mismatch for {asset.output_path}: "
                f"expected {asset.size}/{asset.sha256}, found {size}/{digest}"
            )


def materialize(stage: Path, planned: list[PlannedAsset], manifest: bytes) -> None:
    for root in CANONICAL_ROOTS:
        (stage / root).mkdir(parents=True, exist_ok=False)
    for asset in planned:
        destination = stage.joinpath(*PurePosixPath(asset.output_path).parts)
        destination.parent.mkdir(parents=True, exist_ok=True)
        try:
            if asset.link_mode == "generated":
                assert asset.content is not None
                with destination.open("xb") as stream:
                    stream.write(asset.content)
            elif asset.link_mode == "hardlink":
                assert asset.disk_path is not None
                os.link(asset.disk_path, destination, follow_symlinks=False)
            elif asset.link_mode == "copy":
                assert asset.disk_path is not None
                shutil.copyfile(asset.disk_path, destination, follow_symlinks=False)
            else:
                error(f"unknown link mode: {asset.link_mode}")
        except OSError as exc:
            error(f"cannot materialize {asset.output_path}: {exc}")
    try:
        with (stage / MANIFEST_NAME).open("xb") as stream:
            stream.write(manifest)
        with (stage / CHECKSUMS_NAME).open("xb") as stream:
            stream.write(checksums_bytes(planned))
    except OSError as exc:
        error(f"cannot write overlay identity files: {exc}")


def commit_stage(stage: Path, output: Path) -> None:
    if output.exists() or output.is_symlink():
        if output.is_symlink() or not output.is_dir():
            error(f"refusing to replace non-directory output: {output}")
        backup = output.parent / f".{output.name}.old-{uuid.uuid4().hex}"
        try:
            os.replace(output, backup)
            try:
                os.replace(stage, output)
            except BaseException:
                os.replace(backup, output)
                raise
            shutil.rmtree(backup)
        except OSError as exc:
            error(f"cannot atomically replace {output}: {exc}")
    else:
        try:
            os.replace(stage, output)
        except OSError as exc:
            error(f"cannot atomically install {output}: {exc}")


def archive_member_path(name: str) -> PurePosixPath:
    normalized = name.replace("\\", "/")
    path = PurePosixPath(normalized)
    if not normalized or path.is_absolute() or any(part in {"", ".", ".."} for part in path.parts):
        error(f"unsafe archive member path: {name!r}")
    if path.parts and ":" in path.parts[0]:
        error(f"drive-qualified archive member path: {name!r}")
    for part in path.parts:
        normalized_component(part)
    return path


def extract_zip(archive: Path, destination: Path) -> None:
    seen: set[str] = set()
    try:
        with zipfile.ZipFile(archive) as package:
            for member in sorted(package.infolist(), key=lambda item: (item.filename.casefold(), item.filename)):
                relative = archive_member_path(member.filename.rstrip("/"))
                key = relative.as_posix().casefold()
                if key in seen:
                    error(f"case-insensitive duplicate archive member: {member.filename}")
                seen.add(key)
                unix_mode = (member.external_attr >> 16) & 0xFFFF
                if unix_mode and stat.S_ISLNK(unix_mode):
                    error(f"archive symbolic link is forbidden: {member.filename}")
                target = destination.joinpath(*relative.parts)
                if member.is_dir():
                    target.mkdir(parents=True, exist_ok=True)
                    continue
                target.parent.mkdir(parents=True, exist_ok=True)
                with package.open(member) as source, target.open("xb") as output:
                    shutil.copyfileobj(source, output, CHUNK_SIZE)
    except (OSError, zipfile.BadZipFile) as exc:
        error(f"cannot extract ZIP archive {archive}: {exc}")


def extract_tar(archive: Path, destination: Path) -> None:
    seen: set[str] = set()
    try:
        with tarfile.open(archive, "r:*") as package:
            members = sorted(package.getmembers(), key=lambda item: (item.name.casefold(), item.name))
            for member in members:
                relative = archive_member_path(member.name.rstrip("/"))
                key = relative.as_posix().casefold()
                if key in seen:
                    error(f"case-insensitive duplicate archive member: {member.name}")
                seen.add(key)
                target = destination.joinpath(*relative.parts)
                if member.isdir():
                    target.mkdir(parents=True, exist_ok=True)
                    continue
                if not member.isfile():
                    error(f"archive link/device entry is forbidden: {member.name}")
                source = package.extractfile(member)
                if source is None:
                    error(f"cannot read archive member: {member.name}")
                target.parent.mkdir(parents=True, exist_ok=True)
                with source, target.open("xb") as output:
                    shutil.copyfileobj(source, output, CHUNK_SIZE)
    except (OSError, tarfile.TarError) as exc:
        error(f"cannot extract TAR archive {archive}: {exc}")


def system_bsdtar() -> Path:
    candidate = Path("/usr/bin/bsdtar")
    try:
        mode = candidate.stat().st_mode
    except OSError as exc:
        error(f"7z import requires trusted system /usr/bin/bsdtar: {exc}")
    if not stat.S_ISREG(mode) or not os.access(candidate, os.X_OK):
        error("7z import requires executable regular file /usr/bin/bsdtar")
    return candidate


def run_bsdtar(bsdtar: Path, arguments: list[str], action: str) -> bytes:
    try:
        completed = subprocess.run(
            [str(bsdtar), *arguments],
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
            env={"LC_ALL": "C", "PATH": "/usr/bin:/bin"},
        )
    except OSError as exc:
        error(f"cannot {action} with {bsdtar}: {exc}")
    if completed.returncode != 0:
        detail = completed.stderr.decode("utf-8", errors="replace").strip()
        error(f"cannot {action} with {bsdtar} (exit {completed.returncode}): {detail}")
    return completed.stdout


def bsdtar_listing_lines(data: bytes, label: str) -> list[str]:
    try:
        text = data.decode("utf-8")
    except UnicodeDecodeError as exc:
        error(f"7z {label} is not UTF-8: {exc}")
    lines = text.splitlines()
    if not lines:
        error(f"7z {label} is empty")
    return lines


def validate_7z_members(archive: Path, bsdtar: Path) -> None:
    names = bsdtar_listing_lines(
        run_bsdtar(bsdtar, ["-tf", str(archive)], f"list 7z archive {archive}"),
        "member listing",
    )
    details = bsdtar_listing_lines(
        run_bsdtar(bsdtar, ["-tvf", str(archive)], f"inspect 7z archive {archive}"),
        "verbose member listing",
    )
    if len(names) != len(details):
        error(
            f"7z member listing is ambiguous: "
            f"{len(names)} names but {len(details)} type records"
        )
    seen: set[str] = set()
    for name, detail in zip(names, details):
        directory_name = name.endswith(("/", "\\"))
        relative = archive_member_path(name.rstrip("/\\"))
        key = relative.as_posix().casefold()
        if key in seen:
            error(f"case-insensitive duplicate 7z member: {name}")
        seen.add(key)
        member_type = detail[:1]
        if member_type not in {"-", "d"}:
            error(f"7z link/device entry is forbidden: {name}")
        if directory_name != (member_type == "d"):
            error(f"7z member type/name mismatch: {name}")


def validate_extracted_tree(destination: Path) -> None:
    for current, directory_names, file_names in os.walk(destination, followlinks=False):
        directory_names.sort(key=lambda value: (value.casefold(), value))
        file_names.sort(key=lambda value: (value.casefold(), value))
        current_path = Path(current)
        for name in directory_names:
            path = current_path / name
            if path.is_symlink() or not path.is_dir():
                error(f"7z extracted non-directory/link entry: {path}")
            archive_member_path(path.relative_to(destination).as_posix())
        for name in file_names:
            path = current_path / name
            if path.is_symlink():
                error(f"7z extracted symbolic link: {path}")
            try:
                file_stat = path.stat()
            except OSError as exc:
                error(f"cannot stat extracted 7z member {path}: {exc}")
            if not stat.S_ISREG(file_stat.st_mode) or file_stat.st_nlink != 1:
                error(f"7z extracted link/device entry is forbidden: {path}")
            archive_member_path(path.relative_to(destination).as_posix())


def extract_7z(archive: Path, destination: Path) -> None:
    bsdtar = system_bsdtar()
    validate_7z_members(archive, bsdtar)
    try:
        if any(destination.iterdir()):
            error(f"7z staging directory is not empty: {destination}")
    except OSError as exc:
        error(f"cannot inspect 7z staging directory {destination}: {exc}")
    run_bsdtar(
        bsdtar,
        [
            "--no-same-owner",
            "--no-same-permissions",
            "-xf",
            str(archive),
            "-C",
            str(destination),
        ],
        f"extract 7z archive {archive}",
    )
    validate_extracted_tree(destination)




def locate_retail_root(extracted: Path) -> Path:
    candidates: list[Path] = []
    for current, directory_names, _ in os.walk(extracted, followlinks=False):
        directory_names.sort(key=lambda value: (value.casefold(), value))
        current_path = Path(current)
        for name in directory_names:
            if (current_path / name).is_symlink():
                error(f"symbolic-link directory in extracted archive: {current_path / name}")
        names = collections.Counter(name.casefold() for name in directory_names)
        if all(names[name.casefold()] == 1 for name in ("System", "Maps", "Textures", "Sounds", "Music", "Help")):
            candidates.append(current_path)
    candidates = sorted(set(candidates), key=lambda path: (len(path.parts), path.as_posix().casefold(), path.as_posix()))
    if len(candidates) != 1:
        error(f"expected one retail tree in archive, found {len(candidates)}: {candidates}")
    return candidates[0]


def retail_root_from_archive(archive: Path, temporary: Path) -> Path:
    lower = archive.name.casefold()
    if lower.endswith(".7z"):
        extract_7z(archive, temporary)
    elif lower.endswith(".zip"):
        extract_zip(archive, temporary)
    elif lower.endswith((".tar", ".tar.gz", ".tgz", ".tar.bz2", ".tbz2", ".tar.xz", ".txz")):
        extract_tar(archive, temporary)
    else:
        error(f"unsupported archive format (use ZIP, TAR, or 7z): {archive}")
    return validate_directory(locate_retail_root(temporary), "retail archive root")


def run(args: argparse.Namespace) -> None:
    if args.prototype_root is None:
        if args.profile != "retail-only":
            error("--prototype-root is required for safe and full profiles")
        prototype_root = None
    else:
        prototype_root = validate_directory(args.prototype_root, "prototype root")
    output = args.output.expanduser().absolute()
    if output.is_symlink():
        error(f"output must not be a symbolic link: {output}")
    output_parent = output.parent
    try:
        if args.check:
            if output_parent.is_symlink() or not output_parent.is_dir():
                error(f"output parent does not exist for --check: {output_parent}")
        else:
            output_parent.mkdir(parents=True, exist_ok=True)
        output_parent = output_parent.resolve(strict=True)
        output = output_parent / output.name
    except OSError as exc:
        error(f"cannot prepare output parent {output.parent}: {exc}")

    if prototype_root is not None:
        if is_relative_to(output, prototype_root) or is_relative_to(prototype_root, output):
            error("output and prototype source trees must not overlap")

    with tempfile.TemporaryDirectory(prefix="hp2-retail-archive-") as archive_temporary:
        if args.retail_root is not None:
            retail_root = validate_directory(args.retail_root, "retail root")
        else:
            archive = validate_file(args.archive, "retail archive")
            retail_root = retail_root_from_archive(archive, Path(archive_temporary))
        if is_relative_to(output, retail_root) or is_relative_to(retail_root, output):
            error("output and retail source trees must not overlap")

        planned = plan_overlay(
            prototype_root,
            retail_root,
            output_parent,
            args.link_mode,
            args.profile,
        )
        manifest = manifest_bytes(planned, args.profile)
        if args.check:
            validate_output(output, planned, manifest)
            return

        if output.exists():
            try:
                validate_output(output, planned, manifest)
                return
            except OverlayError:
                pass

        stage_path = Path(tempfile.mkdtemp(prefix=f".{output.name}.tmp-", dir=output_parent))
        try:
            materialize(stage_path, planned, manifest)
            validate_output(stage_path, planned, manifest)
            commit_stage(stage_path, output)
        finally:
            if stage_path.exists():
                shutil.rmtree(stage_path)


def main(argv: list[str] | None = None) -> int:
    try:
        args = parse_args(argv)
        run(args)
        return 0
    except OverlayError as exc:
        print(f"prepare_retail_data.py: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
