#!/usr/bin/env python3
"""Emit and validate the deterministic hp2-state.json build fingerprint.

The state file records the observable facts of a configured build directory:
toolchain identity, capability gates, and data-root presence. Output is
byte-identical for identical inputs -- no timestamps, no environment noise --
so ``--check`` can detect drift between what CMake configured and what the
verification layer last recorded.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Mapping

STATE_FORMAT = "hp2-state"
SCHEMA_VERSION = 1
STATE_FILENAME = "hp2-state.json"

DEFAULT_UNREAL_ROOT = Path("HarryPotter2/Unreal")
DEFAULT_APP_BUNDLE = Path("dist/macos-arm64/HarryPotter2.app")
DEFAULT_RETAIL_DATA_DIR = Path("out/retail-data")
OVERLAY_MANIFEST_NAME = "overlay-manifest.json"

NATIVE_TEXT_CACHE_VAR = "HP2_HAS_NATIVE_TEXT_BACKEND"
FULL_MAP_SMOKE_CACHE_VAR = "HP2_ENABLE_FULL_MAP_SMOKE"

# Matches `VAR:TYPE=value` or `VAR:INTERNAL=` lines in CMakeCache.txt.
_CACHE_LINE_RE = re.compile(r"^([A-Za-z0-9_./+-]+)(?::[^=]*)?=(.*)$")


class StateError(Exception):
    """Raised when the build directory cannot be fingerprinted."""


def parse_cmake_cache(text: str) -> dict[str, str]:
    """Parse CMakeCache.txt content into a plain variable mapping."""
    cache: dict[str, str] = {}
    for line in text.splitlines():
        line = line.strip()
        if not line or line.startswith(("#", "//")):
            continue
        match = _CACHE_LINE_RE.match(line)
        if match:
            cache[match.group(1)] = match.group(2)
    return cache


def parse_compiler_id_file(text: str) -> str | None:
    """Extract CMAKE_CXX_COMPILER_ID from a CMakeCXXCompiler.cmake file."""
    match = re.search(r'set\(CMAKE_CXX_COMPILER_ID\s+"?([^"\)\s]+)"?\)', text)
    return match.group(1) if match else None


def extract_compile_definitions(compile_commands_text: str) -> frozenset[str]:
    """Collect -D<NAME> definition names from compile_commands.json content."""
    names: set[str] = set()
    for match in re.finditer(r"(?<![\w-])-D([A-Za-z_][A-Za-z0-9_]*)", compile_commands_text):
        names.add(match.group(1))
    return frozenset(names)


def resolve_native_text_backend(
    cache: Mapping[str, str], compile_definitions: frozenset[str]
) -> bool:
    """Native text is on when the cache var says so, or when the compiled
    translation units carry the backend define (the var itself is a normal,
    non-cached CMake variable)."""
    cached = cache.get(NATIVE_TEXT_CACHE_VAR)
    if cached is not None:
        return _cache_bool(cached)
    return NATIVE_TEXT_CACHE_VAR in compile_definitions


def _cache_bool(value: str) -> bool:
    return value.strip().upper() in ("1", "ON", "YES", "TRUE", "Y")


def read_build_facts(build_dir: Path) -> dict[str, object]:
    """Read raw facts from a configured CMake build directory."""
    cache_path = build_dir / "CMakeCache.txt"
    try:
        cache_text = cache_path.read_text(encoding="utf-8", errors="replace")
    except OSError as exc:
        raise StateError(f"cannot read {cache_path}: {exc}") from exc
    cache = parse_cmake_cache(cache_text)

    compiler_id: str | None = None
    for compiler_file in sorted(build_dir.glob("CMakeFiles/*/CMakeCXXCompiler.cmake")):
        try:
            compiler_id = parse_compiler_id_file(
                compiler_file.read_text(encoding="utf-8", errors="replace")
            )
        except OSError:
            continue
        if compiler_id:
            break

    definitions: frozenset[str] = frozenset()
    commands_path = build_dir / "compile_commands.json"
    try:
        definitions = extract_compile_definitions(
            commands_path.read_text(encoding="utf-8", errors="replace")
        )
    except OSError:
        pass

    cxx_compiler = cache.get("CMAKE_CXX_COMPILER")
    deployment_target = cache.get("CMAKE_OSX_DEPLOYMENT_TARGET")
    missing: list[str] = []
    if not compiler_id:
        missing.append("CMAKE_CXX_COMPILER_ID (CMakeFiles/*/CMakeCXXCompiler.cmake)")
    if not cxx_compiler:
        missing.append("CMAKE_CXX_COMPILER (CMakeCache.txt)")
    if deployment_target is None:
        missing.append("CMAKE_OSX_DEPLOYMENT_TARGET (CMakeCache.txt)")
    if missing:
        raise StateError(
            f"{build_dir} does not look like a configured build dir; missing: "
            + ", ".join(missing)
        )

    return {
        "compiler_id": compiler_id,
        "cxx_compiler": cxx_compiler,
        "osx_deployment_target": deployment_target.strip(),
        "native_text_backend": resolve_native_text_backend(cache, definitions),
        "full_map_smoke": _cache_bool(cache.get(FULL_MAP_SMOKE_CACHE_VAR, "OFF")),
    }


def collect_state(repo_root: Path, build_dir: Path) -> dict[str, object]:
    """Build the full hp2-state.json document from repo and build facts."""
    facts = read_build_facts(build_dir)
    retail_dir = repo_root / DEFAULT_RETAIL_DATA_DIR
    return {
        "format": STATE_FORMAT,
        "schema_version": SCHEMA_VERSION,
        "toolchain": {
            "compiler_id": facts["compiler_id"],
            "cxx_compiler": facts["cxx_compiler"],
            "osx_deployment_target": facts["osx_deployment_target"],
        },
        "capabilities": {
            "native_text_backend": facts["native_text_backend"],
            "full_map_smoke": facts["full_map_smoke"],
        },
        "data": {
            "unreal_root_present": (repo_root / DEFAULT_UNREAL_ROOT).is_dir(),
            "dist_app_present": (repo_root / DEFAULT_APP_BUNDLE).exists(),
            "overlay_manifest_present": (retail_dir / OVERLAY_MANIFEST_NAME).is_file(),
        },
    }


def render_state(state: Mapping[str, object]) -> str:
    """Serialize deterministically: fixed key order, stable indentation."""
    return json.dumps(state, indent=2, sort_keys=False, ensure_ascii=True) + "\n"


def check_state(current: Mapping[str, object], recorded_path: Path) -> list[str]:
    """Compare freshly computed state against a recorded hp2-state.json.

    Returns human-readable drift descriptions; empty list means no drift.
    """
    try:
        recorded = json.loads(recorded_path.read_text(encoding="utf-8"))
    except OSError as exc:
        raise StateError(f"cannot read {recorded_path}: {exc}") from exc
    except json.JSONDecodeError as exc:
        raise StateError(f"{recorded_path} is not valid JSON: {exc}") from exc

    drift: list[str] = []
    for key in ("format", "schema_version"):
        if recorded.get(key) != current[key]:
            drift.append(f"{key}: recorded={recorded.get(key)!r} actual={current[key]!r}")
    for section in ("toolchain", "capabilities", "data"):
        actual_section = current[section]
        assert isinstance(actual_section, dict)
        recorded_section = recorded.get(section)
        if not isinstance(recorded_section, dict):
            drift.append(f"{section}: missing from recorded state")
            continue
        for key in sorted(set(recorded_section) | set(actual_section)):
            if recorded_section.get(key) != actual_section.get(key):
                drift.append(
                    f"{section}.{key}: recorded="
                    f"{recorded_section.get(key)!r} actual={actual_section.get(key)!r}"
                )
    return drift


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Write or validate the deterministic hp2-state.json build fingerprint."
    )
    parser.add_argument(
        "--build-dir",
        required=True,
        metavar="OUT/PRESET",
        help="configured CMake build directory (e.g. out/macos-arm64)",
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="validate an existing hp2-state.json instead of writing one; exit 1 on drift",
    )
    args = parser.parse_args(argv)

    build_dir = Path(args.build_dir)
    repo_root = Path(__file__).resolve().parent.parent

    try:
        state = collect_state(repo_root, build_dir)
    except StateError as exc:
        print(f"hp2_state: error: {exc}", file=sys.stderr)
        return 2

    state_path = build_dir / STATE_FILENAME
    rendered = render_state(state)
    if args.check:
        drift = check_state(state, state_path)
        if drift:
            print(f"hp2_state: DRIFT detected against {state_path}:", file=sys.stderr)
            for entry in drift:
                print(f"  {entry}", file=sys.stderr)
            return 1
        print(f"hp2_state: OK {state_path}")
        return 0

    try:
        state_path.write_text(rendered, encoding="utf-8")
    except OSError as exc:
        print(f"hp2_state: error: cannot write {state_path}: {exc}", file=sys.stderr)
        return 2
    sys.stdout.write(rendered)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
