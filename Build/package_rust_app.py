#!/usr/bin/env python3
"""Package the Rust rewrite (hp2rs) as a macOS app bundle.

Builds the hp2rs release binary via cargo, then (re)creates
dist/macos-arm64-rs/HarryPotter2.app with:

- Contents/MacOS/HarryPotter2 copied from target/release/hp2rs
- Contents/Info.plist mirroring the existing C++ bundle's plist
  (dist/macos-arm64/HarryPotter2.app): identifier and version are taken
  from there verbatim so both bundles stay in lockstep.
- Any icon assets (.icns) from the existing bundle's Contents/Resources,
  reused as-is together with their CFBundleIconFile key.

Like the CMake hp2_macos_app target, the bundle is always recreated from
scratch: stale binaries, signatures, and plists must never survive a
packaging invocation. rodio talks to system audio, so unlike the C++
bundle no OpenAL dylib is embedded.

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
REFERENCE_BUNDLE_RELPATH = Path("dist") / "macos-arm64" / "HarryPotter2.app"
CARGO_PACKAGE = "hp2rs"
EXECUTABLE_NAME = "HarryPotter2"

MACHO_ARM64_MAGIC = b"\xcf\xfa\xed\xfe"  # MH_MAGIC_64, little-endian (feedfacf)

ICON_SUFFIX = ".icns"
ICON_PLIST_KEY = "CFBundleIconFile"


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


def load_reference_plist(reference_bundle: Path) -> dict[str, Any]:
    plist_path = reference_bundle / "Contents" / "Info.plist"
    if not plist_path.is_file():
        raise PackageError(
            f"{plist_path} does not exist; build the C++ bundle first "
            "(cmake --build ... --target hp2_macos_app)"
        )
    try:
        with plist_path.open("rb") as handle:
            plist = plistlib.load(handle)
    except (plistlib.InvalidFileException, ValueError, OSError) as error:
        raise PackageError(f"{plist_path}: {error}") from error
    if not isinstance(plist, dict):
        raise PackageError(f"{plist_path} is not a dictionary")
    return plist


def mirrored_plist(reference_plist: dict[str, Any]) -> dict[str, Any]:
    """Copy the reference plist, keeping executable identity stable."""
    plist = dict(reference_plist)
    plist["CFBundleExecutable"] = EXECUTABLE_NAME
    return plist


def copy_icon_assets(reference_bundle: Path, contents: Path,
                     plist: dict[str, Any]) -> list[str]:
    """Reuse the existing bundle's icon assets; returns copied names."""
    resources = reference_bundle / "Contents" / "Resources"
    if not resources.is_dir():
        return []
    icons = sorted(path for path in resources.iterdir()
                   if path.is_file() and path.suffix.casefold() == ICON_SUFFIX)
    if not icons:
        return []
    destination_dir = contents / "Resources"
    destination_dir.mkdir(parents=True, exist_ok=True)
    copied: list[str] = []
    for icon in icons:
        shutil.copy2(icon, destination_dir / icon.name)
        copied.append(icon.name)
    primary = copied[0]
    if ICON_PLIST_KEY not in plist:
        plist[ICON_PLIST_KEY] = primary.removesuffix(ICON_SUFFIX)
    return copied


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
    reference_bundle = resolved / REFERENCE_BUNDLE_RELPATH

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

    plist = mirrored_plist(load_reference_plist(reference_bundle))
    icons = copy_icon_assets(reference_bundle, contents, plist)
    with (contents / "Info.plist").open("wb") as handle:
        plistlib.dump(plist, handle, sort_keys=True)

    sign_bundle(bundle)

    return {
        "bundle": bundle,
        "executable": executable,
        "icons": icons,
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
    if result["icons"]:
        print(f"  icons: {', '.join(result['icons'])}")
    else:
        print("  icons: none (reference bundle ships no icon assets)")
    print(f"  build skipped: {result['build']['skipped']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
