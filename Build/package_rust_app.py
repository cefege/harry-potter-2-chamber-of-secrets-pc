#!/usr/bin/env python3
"""Package the Rust rewrite (hp2rs) as a self-contained macOS app bundle.

Builds the hp2rs release binary via cargo, then (re)creates
dist/macos-arm64-rs/HarryPotter2.app with:

- Contents/MacOS/HarryPotter2 copied from target/release/hp2rs.
- Contents/Info.plist generated from Rust bundle metadata owned below.

The Rust bundle intentionally has no external resources or embedded
frameworks: rodio talks to system audio services. It does not read, copy, or
otherwise depend on the legacy C++ bundle.

Like the CMake hp2_macos_app target, the bundle is always recreated from
scratch: stale binaries, signatures, plists, and framework payloads must
never survive a packaging invocation.

Exit codes: 0 packaged, 1 failure.
"""

from __future__ import annotations

import argparse
import os
import plistlib
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Any

BUNDLE_RELPATH = Path("dist") / "macos-arm64-rs" / "HarryPotter2.app"
CARGO_PACKAGE = "hp2rs"
EXECUTABLE_NAME = "HarryPotter2"

# Rust owns this bundle metadata. Keep it aligned with the workspace release
# identity without deriving it from the retired C++ bundle.
BUNDLE_IDENTIFIER = "com.hp2.native"
BUNDLE_VERSION = "0.1.0"

MACHO_ARM64_MAGIC = b"\xcf\xfa\xed\xfe"  # MH_MAGIC_64, little-endian (feedfacf)


class PackageError(Exception):
    """A fatal packaging condition with a human-readable detail."""



# Overridable so hermetic runs can pin the toolchain binary; falls back to
# the standard rustup install location when cargo is not on PATH.
CARGO_ENV_OVERRIDE = "HP2_CARGO_BIN"


def cargo_binary() -> str:
    override = os.environ.get(CARGO_ENV_OVERRIDE)
    if override:
        return override
    found = shutil.which("cargo")
    if found:
        return found
    default = Path.home() / ".cargo" / "bin" / "cargo"
    if default.is_file():
        return str(default)
    raise PackageError("cargo not found on PATH or in ~/.cargo/bin")


CODESIGN_ENV_OVERRIDE = "HP2_CODESIGN_BIN"


def codesign_binary() -> str:
    return os.environ.get(CODESIGN_ENV_OVERRIDE, "codesign")

def repo_root() -> Path:
    return Path(__file__).resolve().parent.parent


def run_cargo_build(repo: Path) -> dict[str, Any]:
    command = [cargo_binary(), "build", "--release", "--package", CARGO_PACKAGE]
    completed = subprocess.run(command, cwd=repo, check=False)
    if completed.returncode != 0:
        raise PackageError(
            f"{' '.join(command)} failed with exit code {completed.returncode}"
        )
    return {"command": command, "exit_code": completed.returncode}


def rust_info_plist() -> dict[str, Any]:
    """Return the complete, Rust-owned metadata for the app bundle."""
    return {
        "ApplePressAndHoldEnabled": False,
        "CFBundleDevelopmentRegion": "English",
        "CFBundleDisplayName": "Harry Potter 2",
        "CFBundleExecutable": EXECUTABLE_NAME,
        "CFBundleIdentifier": BUNDLE_IDENTIFIER,
        "CFBundleInfoDictionaryVersion": "6.0",
        "CFBundleName": EXECUTABLE_NAME,
        "CFBundlePackageType": "APPL",
        "CFBundleShortVersionString": BUNDLE_VERSION,
        "CFBundleVersion": BUNDLE_VERSION,
        "LSArchitecturePriority": ["arm64"],
        "LSMinimumSystemVersion": "15.0",
        "LSRequiresNativeExecution": True,
        "NSHighResolutionCapable": True,
    }


def sign_bundle(bundle: Path) -> None:
    """Ad-hoc sign, matching the CMake hp2_macos_app target's flags.

    cargo's linker embeds an ad-hoc signature in the raw binary whose seal
    expects bundle resources; re-signing the whole bundle replaces it with a
    correct directory signature.
    """
    command = [
        codesign_binary(), "--force", "--deep",
        "--sign", "-", "--timestamp=none", str(bundle),
    ]
    completed = subprocess.run(command, capture_output=True, text=True, check=False)
    if completed.returncode != 0:
        raise PackageError(
            f"codesign failed: {(completed.stderr or completed.stdout).strip()}"
        )


def verify_arm64_binary(executable: Path) -> None:
    try:
        with executable.open("rb") as handle:
            magic = handle.read(4)
    except OSError as error:
        raise PackageError(f"{executable}: {error}") from error
    if magic != MACHO_ARM64_MAGIC:
        raise PackageError(
            f"{executable}: expected thin arm64 Mach-O magic feedfacf, found "
            + magic.hex()
        )


def package(repo: Path, skip_build: bool) -> dict[str, Any]:
    resolved = repo.resolve()

    bundle = resolved / BUNDLE_RELPATH

    build: dict[str, Any] = {"skipped": skip_build}
    if not skip_build:
        build.update(run_cargo_build(resolved))
    built_binary = resolved / "target" / "release" / CARGO_PACKAGE
    if not built_binary.is_file():
        raise PackageError(
            f"{built_binary} does not exist after cargo build"
        )

    # Always recreate: idempotent by construction, no stale payload survives.
    shutil.rmtree(bundle, ignore_errors=True)
    contents = bundle / "Contents"
    macos_dir = contents / "MacOS"
    macos_dir.mkdir(parents=True)

    executable = macos_dir / EXECUTABLE_NAME
    shutil.copy2(built_binary, executable)
    verify_arm64_binary(executable)
    os.chmod(executable, 0o755)

    plist = rust_info_plist()
    with (contents / "Info.plist").open("wb") as handle:
        plistlib.dump(plist, handle, sort_keys=True)

    sign_bundle(bundle)

    return {
        "bundle": bundle,
        "executable": executable,
        "build": build,
        "version": plist.get("CFBundleShortVersionString"),
        "identifier": plist.get("CFBundleIdentifier"),
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--repo-root", type=Path,
                        default=Path(__file__).resolve().parent.parent)
    parser.add_argument("--skip-build", action="store_true",
                        help="reuse target/release/hp2rs instead of invoking cargo")
    arguments = parser.parse_args(argv)
    try:
        result = package(arguments.repo_root, arguments.skip_build)
    except PackageError as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    display = result["bundle"]
    try:
        display = display.relative_to(Path.cwd())
    except ValueError:
        pass
    print(f"packaged {display}")
    print(f"  executable: {result['executable'].name}"
          f" ({result['version']}, {result['identifier']})")
    print("  resources: none (Rust bundle embeds no external assets)")
    print(f"  build skipped: {result['build']['skipped']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
