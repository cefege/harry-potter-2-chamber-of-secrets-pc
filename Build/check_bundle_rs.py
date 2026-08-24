#!/usr/bin/env python3
"""Verify the Rust rewrite's installed macOS app bundle.

Rust-side twin of check_bundle.py: same report-schema-v1 document, same
exit-code contract (0 pass, 1 fail, 2 blocked), with one deliberate
divergence — the embedded-OpenAL-framework expectation is dropped because
hp2rs plays audio through rodio against system audio services, so the
bundle carries no Contents/Frameworks payload to inspect.

Checks performed:
- Info.plist presence and sanity (CFBundleShortVersionString,
  CFBundleIdentifier, CFBundleExecutable).
- Main executable is a thin little-endian arm64 Mach-O (magic feedfacf /
  bytes cf fa ed fe).
- Codesign state via ``codesign -dv``/``codesign --verify --strict``; an
  unsigned bundle is a recorded warning, a clearly invalid signature fails.

The check itself needs no game data: it always reports data profile "none".
"""

from __future__ import annotations

import argparse
import json
import os
import plistlib
import subprocess
import sys
from pathlib import Path
from typing import Any

SCHEMA_VERSION = 1
REPORT_NAME = "check_bundle_rs"
DATA_PROFILE = "none"
BUNDLE_RELPATH = Path("dist") / "macos-arm64-rs" / "HarryPotter2.app"

MACHO_ARM64_MAGIC = b"\xcf\xfa\xed\xfe"  # MH_MAGIC_64, little-endian (feedfacf)

# Overridable so hermetic contract tests can stub the toolchain binary.
CODESIGN_ENV_OVERRIDE = "HP2_CODESIGN_BIN"


def codesign_binary() -> str:
    return os.environ.get(CODESIGN_ENV_OVERRIDE, "codesign")

EXIT_PASS = 0
EXIT_FAIL = 1
EXIT_BLOCKED = 2


class CheckFailure(Exception):
    """A single failed invariant with its reason code."""

    def __init__(self, reason_code: str, detail: str) -> None:
        super().__init__(detail)
        self.reason_code = reason_code
        self.detail = detail


def load_info_plist(bundle: Path) -> dict[str, Any]:
    plist_path = bundle / "Contents" / "Info.plist"
    if not plist_path.is_file():
        raise CheckFailure("bundle.info_plist_missing", f"{plist_path} does not exist")
    try:
        with plist_path.open("rb") as handle:
            plist = plistlib.load(handle)
    except (plistlib.InvalidFileException, ValueError) as error:
        raise CheckFailure("bundle.info_plist_unreadable", f"{plist_path}: {error}") from error
    if not isinstance(plist, dict):
        raise CheckFailure("bundle.info_plist_unreadable", f"{plist_path} is not a dictionary")
    return plist


def require_plist_keys(plist: dict[str, Any]) -> tuple[str, str, str]:
    version = plist.get("CFBundleShortVersionString")
    identifier = plist.get("CFBundleIdentifier")
    executable_name = plist.get("CFBundleExecutable")
    missing = [
        key
        for key, value in (
            ("CFBundleShortVersionString", version),
            ("CFBundleIdentifier", identifier),
            ("CFBundleExecutable", executable_name),
        )
        if not isinstance(value, str) or not value.strip()
    ]
    if missing:
        raise CheckFailure(
            "bundle.info_plist_incomplete",
            "Info.plist missing or empty keys: " + ", ".join(missing),
        )
    if "." not in identifier or any(part == "" for part in identifier.split(".")):
        raise CheckFailure(
            "bundle.identifier_malformed",
            f"CFBundleIdentifier {identifier!r} is not reverse-DNS style",
        )
    first_version_component = version.split(".")[0]
    if not first_version_component.isdigit():
        raise CheckFailure(
            "bundle.version_malformed",
            f"CFBundleShortVersionString {version!r} lacks a numeric leading component",
        )
    return version, identifier, executable_name


def verify_executable(bundle: Path, executable_name: str) -> dict[str, Any]:
    executable = bundle / "Contents" / "MacOS" / executable_name
    if not executable.is_file():
        raise CheckFailure(
            "bundle.executable_missing",
            f"CFBundleExecutable points at missing {executable}",
        )
    try:
        with executable.open("rb") as handle:
            magic = handle.read(4)
    except OSError as error:
        raise CheckFailure("bundle.executable_unreadable", str(error)) from error
    if magic != MACHO_ARM64_MAGIC:
        raise CheckFailure(
            "bundle.executable_not_arm64",
            "expected thin arm64 Mach-O magic feedfacf, found "
            + magic.hex(),
        )
    return {"executable": executable.name, "magic": magic.hex()}


def inspect_codesign(bundle: Path) -> dict[str, Any]:
    """Record codesign state. Unsigned -> warning; clearly invalid -> failure."""
    display = subprocess.run(
        [codesign_binary(), "-dv", str(bundle)],
        capture_output=True,
        text=True,
        check=False,
    )
    state: dict[str, Any] = {
        "display_exit_code": display.returncode,
        "signature": None,
    }
    for line in display.stderr.splitlines():
        if line.startswith("Signature="):
            state["signature"] = line.split("=", 1)[1].strip()
    verify = subprocess.run(
        [codesign_binary(), "--verify", "--strict", str(bundle)],
        capture_output=True,
        text=True,
        check=False,
    )
    state["verify_exit_code"] = verify.returncode
    output = (verify.stderr or "") + (verify.stdout or "")
    if verify.returncode != 0 and "code object is not signed at all" in output:
        state["warning"] = "bundle is unsigned"
        return state
    if verify.returncode != 0:
        detail = output.strip().splitlines()[0] if output.strip() else f"exit {verify.returncode}"
        raise CheckFailure("bundle.codesign_invalid", f"codesign --verify failed: {detail}")
    return state


def check_bundle(repo_root: Path, bundle: Path | None = None) -> dict[str, Any]:
    """Run every bundle check and return the report-schema-v1 document.

    Never raises for expected conditions: a missing bundle yields status
    "blocked"; broken bundles yield status "fail" with the first failing
    reason_code.
    """
    resolved_root = repo_root.resolve()
    target = (bundle if bundle is not None else resolved_root / BUNDLE_RELPATH).resolve()
    command = {
        "repo_root": _display(resolved_root),
        "bundle": _display(target),
    }
    report: dict[str, Any] = {
        "schema": SCHEMA_VERSION,
        "name": REPORT_NAME,
        "status": "pass",
        "invariant": (
            "installed rust app bundle matches build tree layout: sane "
            "Info.plist, thin arm64 executable, valid codesign; no embedded "
            "frameworks are expected (rodio uses system audio)"
        ),
        "data": {"profile": DATA_PROFILE},
        "artifacts": [],
        "command": command,
        "checks": {},
        "warnings": [],
    }
    if not target.is_dir():
        report["status"] = "blocked"
        report["reason_code"] = "bundle.dist_missing"
        report["exit_reason"] = f"{_display(target)} does not exist; nothing to verify"
        return _finalize(report)
    checks = report["checks"]
    try:
        plist = load_info_plist(target)
        version, identifier, executable_name = require_plist_keys(plist)
        checks["info_plist"] = {
            "CFBundleShortVersionString": version,
            "CFBundleIdentifier": identifier,
            "CFBundleExecutable": executable_name,
        }
        checks["executable"] = verify_executable(target, executable_name)
        codesign_state = inspect_codesign(target)
        checks["codesign"] = codesign_state
        if "warning" in codesign_state:
            report["warnings"].append(codesign_state["warning"])
    except CheckFailure as failure:
        report["status"] = "fail"
        report["reason_code"] = failure.reason_code
        report["exit_reason"] = failure.detail
        return _finalize(report)
    report["exit_reason"] = "all bundle integrity checks passed"
    return _finalize(report)


def artifact_report_path() -> Path | None:
    root = os.environ.get("HP2_ARTIFACT_DIR")
    return Path(root) if root else None


def _finalize(report: dict[str, Any]) -> dict[str, Any]:
    destination = artifact_report_path()
    if destination is not None:
        try:
            destination.mkdir(parents=True, exist_ok=True)
            path = destination / "bundle-integrity-rs.json"
            path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
            report["artifacts"].append({"kind": "report", "path": str(path)})
        except OSError as error:
            report["warnings"].append(f"could not write artifact report: {error}")
    return report


def _display(path: Path) -> str:
    try:
        return str(path.relative_to(Path.cwd()))
    except ValueError:
        return str(path)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--repo-root", type=Path, default=Path(__file__).resolve().parent.parent)
    parser.add_argument("--bundle", type=Path, default=None, help="override bundle path")
    parser.add_argument("--output", type=Path, default=None, help="also copy the JSON report here")
    arguments = parser.parse_args(argv)

    report = check_bundle(arguments.repo_root, arguments.bundle)
    encoded = json.dumps(report, indent=2, sort_keys=True) + "\n"
    sys.stdout.write(encoded)
    if arguments.output is not None:
        arguments.output.parent.mkdir(parents=True, exist_ok=True)
        arguments.output.write_text(encoded)
    return {"pass": EXIT_PASS, "fail": EXIT_FAIL, "blocked": EXIT_BLOCKED}[report["status"]]


if __name__ == "__main__":
    raise SystemExit(main())
