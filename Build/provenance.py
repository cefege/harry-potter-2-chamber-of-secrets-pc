#!/usr/bin/env python3
"""Assemble a promotion provenance bundle for an HP2 verification run.

A provenance bundle is the evidence package attached to a feature-gate
promotion (experimental -> runtime-verified -> retail-verified ->
default-enabled). It captures enough state for a later agent or human to
reproduce the same verification: source identity, build configuration,
feature-gate policy, generated machine state, data identity, and the exact
commands with their report/artifact locations.

Usage:
    python3 Build/provenance.py --preset macos-arm64 --output out/macos-arm64/provenance.json
    python3 Build/provenance.py --preset macos-arm64 --check out/macos-arm64/provenance.json

The bundle contains no timestamps and no absolute home paths; everything is
derived deterministically from the repository and build tree so two runs of
the same inputs produce byte-identical JSON.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import sys
from pathlib import Path

FORMAT = "hp2-provenance"
SCHEMA_VERSION = 1


class ProvenanceError(Exception):
    pass


def _git(args: list[str], repo: Path) -> str:
    result = subprocess.run(
        ["git", *args], cwd=repo, capture_output=True, text=True, check=False
    )
    if result.returncode != 0:
        raise ProvenanceError(f"git {' '.join(args)} failed: {result.stderr.strip()}")
    return result.stdout.strip()


def _file_sha256(path: Path) -> str | None:
    digest = hashlib.sha256()
    try:
        with path.open("rb") as handle:
            for chunk in iter(lambda: handle.read(1 << 20), b""):
                digest.update(chunk)
    except OSError:
        return None
    return digest.hexdigest()


def _cache_value(cache_text: str, key: str) -> str | None:
    marker = f"{key}:"
    for line in cache_text.splitlines():
        if line.startswith(marker):
            _, _, value = line.partition("=")
            return value.strip()
    return None


def collect(repo: Path, preset: str) -> dict[str, object]:
    build_dir = repo / "out" / preset
    cache_path = build_dir / "CMakeCache.txt"
    if not cache_path.is_file():
        raise ProvenanceError(
            f"build tree missing: {cache_path} (configure the preset first)"
        )
    cache_text = cache_path.read_text(encoding="utf-8", errors="replace")

    gates_path = repo / "Build" / "feature-gates.json"
    gates = json.loads(gates_path.read_text(encoding="utf-8")) if gates_path.is_file() else None

    state_path = build_dir / "hp2-state.json"
    state = json.loads(state_path.read_text(encoding="utf-8")) if state_path.is_file() else None

    unreal_root = repo / "HarryPotter2" / "Unreal"
    manifest_path = unreal_root / "overlay-manifest.json"

    bundle: dict[str, object] = {
        "format": FORMAT,
        "schema_version": SCHEMA_VERSION,
        "source": {
            "branch": _git(["rev-parse", "--abbrev-ref", "HEAD"], repo),
            "commit": _git(["rev-parse", "HEAD"], repo),
            "dirty": bool(_git(["status", "--porcelain"], repo)),
        },
        "build": {
            "preset": preset,
            "build_type": _cache_value(cache_text, "CMAKE_BUILD_TYPE"),
            "compiler": _cache_value(cache_text, "CMAKE_CXX_COMPILER"),
            "sanitizers": {
                "asan_ubsan": (_cache_value(cache_text, "HP2_ENABLE_ASAN_UBSAN") == "ON"),
                "tsan": (_cache_value(cache_text, "HP2_ENABLE_TSAN") == "ON"),
                "full_map_smoke": (
                    _cache_value(cache_text, "HP2_ENABLE_FULL_MAP_SMOKE") == "ON"
                ),
            },
        },
        "feature_gates_policy": {"path": "Build/feature-gates.json", "content": gates},
        "machine_state": {"path": f"out/{preset}/hp2-state.json", "content": state},
        "data": {
            "profile": "prototype",
            "unreal_root_present": unreal_root.is_dir(),
            "overlay_manifest_sha256": _file_sha256(manifest_path),
        },
        "verification": {
            "contract": (
                "cmake --build --preset {preset} --target hp2_verification_binaries "
                "&& ctest --preset {preset}"
            ).format(preset=preset),
            "report_root": f"out/{preset}/Testing/HP2",
        },
    }
    return bundle


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", type=Path, default=None)
    parser.add_argument("--preset", required=True)
    parser.add_argument("--output", type=Path, default=None)
    parser.add_argument("--check", type=Path, default=None, metavar="BUNDLE")
    args = parser.parse_args()

    try:
        repo = (args.repo_root or Path(__file__).resolve().parent.parent).resolve()
        bundle = collect(repo, args.preset)
        payload = json.dumps(bundle, indent=2, sort_keys=True) + "\n"
        if args.check:
            existing = Path(args.check).read_text(encoding="utf-8")
            if existing != payload:
                print("provenance drift detected against", args.check, file=sys.stderr)
                return 1
            print("provenance OK:", args.check)
            return 0
        output = args.output or (repo / "out" / args.preset / "provenance.json")
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(payload, encoding="utf-8")
        print("wrote", output)
        return 0
    except (ProvenanceError, OSError, json.JSONDecodeError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
