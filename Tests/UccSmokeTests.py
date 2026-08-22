#!/usr/bin/env python3
"""UCC smoke contracts: headless `ucc help` bootstrap verification.

Two modes:

1. Contract tests (default): deterministic unittest suite over the smoke
   runner using a stub ucc binary. No engine build required; safe for ctest.
2. Live smoke (--smoke): runs the real out/<preset>/hp2_ucc against the
   prototype data root, emits a report-schema-v1 JSON document on stdout,
   and exits 0 (pass), 1 (fail), or 3 (blocked).

Verified invariants for the live run:
- exit status 0
- expected stdout markers (commandlet listing banner)
- no failure markers (critical/assert/...)
- duration under budget
- data root unchanged (path/size/mtime inventory before vs after)
"""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import unittest

REPOSITORY_ROOT = Path(__file__).resolve().parent.parent

REPORT_SCHEMA_VERSION = 1
SMOKE_NAME = "ucc_help_smoke"
SMOKE_INVARIANT = (
    "ucc help bootstraps HP2 data paths headlessly, exits 0 with the "
    "commandlet listing, and leaves the data root unmodified"
)
DEFAULT_TIMEOUT_SECONDS = 120.0

# Build-tree presets searched in order; the plain preset wins so behavior is
# stable regardless of which sanitizer trees happen to exist.
PRESET_CANDIDATES = ("macos-arm64",)

# Case-insensitive substrings that must never appear in a healthy ucc run.
FAILURE_MARKERS = (
    "critical",
    "assert",
    "apperror",
    "access violation",
    "pure virtual",
    "segmentation",
)

# Substrings that must appear for `ucc help` to count as a successful
# bootstrap (registry enumeration + usage banner).
EXPECTED_STDOUT_MARKERS = (
    'Commands for "ucc":',
    "ucc help <command>",
)

BLOCKED_EXIT_CODE = 3


def discover_ucc_binary(explicit: str | None = None) -> Path | None:
    """Locate the hp2_ucc executable, or None when not built.

    An explicit argument or HP2_UCC_BINARY is authoritative: a missing or
    non-executable path yields None (reported as blocked/binary_missing)
    instead of silently falling back to another build. Otherwise search
    out/<preset>/hp2_ucc for each known preset, then any out/*/hp2_ucc
    sorted alphabetically.
    """
    named: list[Path] = []
    if explicit:
        named.append(Path(explicit))
    from_env = os.environ.get("HP2_UCC_BINARY")
    if from_env:
        named.append(Path(from_env))
    if named:
        for candidate in named:
            if candidate.is_file() and os.access(candidate, os.X_OK):
                return candidate
        return None
    candidates: list[Path] = []
    for preset in PRESET_CANDIDATES:
        candidates.append(REPOSITORY_ROOT / "out" / preset / "hp2_ucc")
    out_root = REPOSITORY_ROOT / "out"
    if out_root.is_dir():
        candidates.extend(sorted(out_root.glob("*/hp2_ucc")))
    for candidate in candidates:
        if candidate.is_file() and os.access(candidate, os.X_OK):
            return candidate
    return None


def classify_data_profile(data_root: Path) -> str:
    """Map a data root to the data-none|data-prototype|data-retail taxonomy."""
    resolved = str(data_root)
    if resolved.endswith("out/retail-data") or "/retail-data" in resolved:
        return "data-retail"
    if resolved.endswith("HarryPotter2/Unreal"):
        return "data-prototype"
    return "data-none"


def resolve_data_root(explicit: str | None = None) -> Path | None:
    """Pick the read-only data root, or None to synthesize a minimal one."""
    candidates: list[Path] = []
    if explicit:
        candidates.append(Path(explicit))
    from_env = os.environ.get("HP2_UCC_DATA_ROOT")
    if from_env:
        candidates.append(Path(from_env))
    candidates.append(REPOSITORY_ROOT / "HarryPotter2" / "Unreal")
    candidates.append(REPOSITORY_ROOT / "out" / "retail-data")
    for candidate in candidates:
        if (candidate / "System" / "Default.ini").is_file():
            return candidate
    return None


def build_launch_command(binary: Path, data_root: Path) -> list[str]:
    """Headless audit invocation: data bootstrap first, then the command."""
    return [str(binary), "help", f"-datadir={data_root}"]


def snapshot_tree(root: Path) -> dict[str, tuple[int, int]]:
    """Inventory every regular file as {relative path: (size, mtime_ns)}."""
    inventory: dict[str, tuple[int, int]] = {}
    for base, dirnames, filenames in os.walk(root):
        dirnames[:] = [name for name in dirnames if not os.path.islink(os.path.join(base, name))]
        for filename in filenames:
            path = Path(base) / filename
            if path.is_symlink():
                continue
            try:
                info = path.stat()
            except OSError:
                inventory[str(path.relative_to(root))] = (-1, -1)
                continue
            inventory[str(path.relative_to(root))] = (info.st_size, info.st_mtime_ns)
    return inventory


def _has_failure_marker(text: str) -> str | None:
    lowered = text.lower()
    for marker in FAILURE_MARKERS:
        if marker in lowered:
            return marker
    return None


def run_ucc_help_smoke(
    binary: Path | None,
    data_root: Path | None,
    timeout_seconds: float = DEFAULT_TIMEOUT_SECONDS,
    artifact_dir: Path | None = None,
) -> dict:
    """Execute the headless ucc audit and return a report-schema-v1 dict."""
    artifacts: list[dict] = []
    if artifact_dir is None:
        from_env = os.environ.get("HP2_ARTIFACT_DIR")
        artifact_dir = Path(from_env) if from_env else None
    if artifact_dir is not None:
        artifact_dir.mkdir(parents=True, exist_ok=True)

    def record_artifact(kind: str, path: Path) -> None:
        artifacts.append({"kind": kind, "path": str(path)})

    if binary is None:
        report = {
            "schema": REPORT_SCHEMA_VERSION,
            "name": SMOKE_NAME,
            "status": "blocked",
            "invariant": SMOKE_INVARIANT,
            "reason_code": "binary_missing",
            "data": {"profile": "data-none"},
            "artifacts": artifacts,
            "command": [],
            "exit_reason": "hp2_ucc executable not found in build tree or HP2_UCC_BINARY",
        }
        return _finalize(report, artifact_dir, record_artifact)

    if data_root is None:
        synthesized = Path(tempfile.mkdtemp(prefix="hp2-ucc-smoke-data-"))
        (synthesized / "System").mkdir()
        (synthesized / "System" / "Default.ini").write_text("[Core.System]\n")
        data_root = synthesized

    profile = classify_data_profile(data_root)
    command = build_launch_command(binary, data_root)
    report: dict = {
        "schema": REPORT_SCHEMA_VERSION,
        "name": SMOKE_NAME,
        "status": "pass",
        "reason_code": "",
        "invariant": SMOKE_INVARIANT,
        "data": {"profile": profile},
        "artifacts": artifacts,
        "command": command,
        "exit_reason": "",
    }

    environment = dict(os.environ)
    home = Path(tempfile.mkdtemp(prefix="hp2-ucc-smoke-home-"))
    tmp = Path(tempfile.mkdtemp(prefix="hp2-ucc-smoke-tmp-"))
    environment["HOME"] = str(home)
    environment["TMPDIR"] = str(tmp)
    environment["LC_ALL"] = "C"
    environment["TZ"] = "UTC"

    before = snapshot_tree(data_root)
    started = time.monotonic()
    try:
        completed = subprocess.run(
            command,
            cwd=str(REPOSITORY_ROOT),
            env=environment,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=timeout_seconds,
        )
    except subprocess.TimeoutExpired:
        report["status"] = "fail"
        report["reason_code"] = "ucc.timeout"
        report["exit_reason"] = f"exceeded {timeout_seconds:g}s budget"
        return _finalize(report, artifact_dir, record_artifact, log=(None, None))
    duration = time.monotonic() - started
    after = snapshot_tree(data_root)

    log_path = None
    if artifact_dir is not None:
        log_path = artifact_dir / "ucc-help.log"
        log_path.write_text(
            f"$ {' '.join(command)}\n"
            f"--- stdout ---\n{completed.stdout.decode('utf-8', 'replace')}"
            f"--- stderr ---\n{completed.stderr.decode('utf-8', 'replace')}",
            encoding="utf-8",
        )
        record_artifact("log", log_path)

    if completed.returncode != 0:
        report["status"] = "fail"
        report["reason_code"] = "ucc.exit_status"
        report["exit_reason"] = f"exit status {completed.returncode}"
        return _finalize(report, artifact_dir, record_artifact)

    stdout_text = completed.stdout.decode("utf-8", "replace")
    combined = stdout_text + completed.stderr.decode("utf-8", "replace")
    missing = [marker for marker in EXPECTED_STDOUT_MARKERS if marker not in stdout_text]
    if missing:
        report["status"] = "fail"
        report["reason_code"] = "ucc.stdout_marker_missing"
        report["exit_reason"] = f"missing expected markers: {missing}"
        return _finalize(report, artifact_dir, record_artifact)

    marker = _has_failure_marker(combined)
    if marker is not None:
        report["status"] = "fail"
        report["reason_code"] = "ucc.failure_marker"
        report["exit_reason"] = f"failure marker {marker!r} in output"
        return _finalize(report, artifact_dir, record_artifact)

    if after != before:
        changed = sorted(
            path
            for path in set(after) | set(before)
            if after.get(path) != before.get(path)
        )[:5]
        report["status"] = "fail"
        report["reason_code"] = "data.mutated"
        report["exit_reason"] = f"data root inventory changed: {changed}"
        return _finalize(report, artifact_dir, record_artifact)

    report["exit_reason"] = f"exit 0 in {duration:.2f}s; data root unmodified"
    return _finalize(report, artifact_dir, record_artifact)


def _finalize(
    report: dict,
    artifact_dir: Path | None,
    record_artifact,
    log: tuple[bytes | None, bytes | None] | None = None,
) -> dict:
    if artifact_dir is not None:
        report_path = artifact_dir / "ucc-help-report.json"
        report_path.write_text(
            json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )
        record_artifact("report", report_path)
    return report


class UccSmokeContracts(unittest.TestCase):
    """Deterministic contracts over the smoke runner using a stub binary."""

    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(prefix="hp2-ucc-smoke-contracts-")
        self.root = Path(self.temporary.name)
        self.addCleanup(self.temporary.cleanup)
        self.binary = self.root / "stub-ucc"
        self.binary.write_text("#!/bin/sh\nexit 0\n")
        self.binary.chmod(0o755)
        self.data_root = self.root / "Unreal"
        (self.data_root / "System").mkdir(parents=True)
        (self.data_root / "System" / "Default.ini").write_text("[Core.System]\n")
        self._saved_env = {
            key: os.environ.pop(key, None)
            for key in ("HP2_UCC_BINARY", "HP2_UCC_DATA_ROOT", "HP2_ARTIFACT_DIR")
        }
        for key, value in self._saved_env.items():
            if value is not None:
                os.environ[key] = value

    def tearDown(self) -> None:
        for key, value in self._saved_env.items():
            if value is None:
                os.environ.pop(key, None)
            else:
                os.environ[key] = value

    def _stub(self, body: str) -> None:
        self.binary.write_text(f"#!/bin/sh\n{body}\n")
        self.binary.chmod(0o755)

    def test_missing_binary_reports_blocked_with_reason(self) -> None:
        report = run_ucc_help_smoke(None, self.data_root)
        self.assertEqual(report["status"], "blocked")
        self.assertEqual(report["reason_code"], "binary_missing")
        self.assertEqual(report["schema"], 1)
        self.assertEqual(report["data"], {"profile": "data-none"})

    def test_pass_report_carries_full_schema(self) -> None:
        self._stub(
            'printf \'Usage:\\nCommands for "ucc":\\n   ucc help <command>\\n\''
        )
        report = run_ucc_help_smoke(self.binary, self.data_root)
        self.assertEqual(report["status"], "pass")
        self.assertEqual(report["schema"], 1)
        self.assertEqual(report["name"], SMOKE_NAME)
        self.assertEqual(report["invariant"], SMOKE_INVARIANT)
        self.assertEqual(report["command"], [str(self.binary), "help", f"-datadir={self.data_root}"])
        self.assertEqual(report["reason_code"], "")
        self.assertIn("exit 0", report["exit_reason"])
        self.assertEqual(set(report["data"]), {"profile"})

    def test_nonzero_exit_fails_with_exit_status_reason(self) -> None:
        self._stub("exit 7")
        report = run_ucc_help_smoke(self.binary, self.data_root)
        self.assertEqual(report["status"], "fail")
        self.assertEqual(report["reason_code"], "ucc.exit_status")
        self.assertIn("7", report["exit_reason"])

    def test_failure_marker_fails_even_with_zero_exit(self) -> None:
        self._stub('printf \'Commands for "ucc":\\nucc help <command>\\n\'; printf "Critical: bad" >&2')
        report = run_ucc_help_smoke(self.binary, self.data_root)
        self.assertEqual(report["status"], "fail")
        self.assertEqual(report["reason_code"], "ucc.failure_marker")
        self.assertIn("critical", report["exit_reason"])

    def test_missing_stdout_marker_fails(self) -> None:
        self._stub("printf 'nothing useful\\n'")
        report = run_ucc_help_smoke(self.binary, self.data_root)
        self.assertEqual(report["status"], "fail")
        self.assertEqual(report["reason_code"], "ucc.stdout_marker_missing")

    def test_timeout_exceeding_run_fails_with_budget_reason(self) -> None:
        self._stub("sleep 5")
        report = run_ucc_help_smoke(self.binary, self.data_root, timeout_seconds=0.5)
        self.assertEqual(report["status"], "fail")
        self.assertEqual(report["reason_code"], "ucc.timeout")

    def test_data_mutation_fails_with_mutation_reason(self) -> None:
        self._stub(
            'printf \'Commands for "ucc":\\nucc help <command>\\n\'; '
            f"touch '{self.data_root / 'System' / 'Default.ini'}'"
        )
        report = run_ucc_help_smoke(self.binary, self.data_root)
        self.assertEqual(report["status"], "fail")
        self.assertEqual(report["reason_code"], "data.mutated")
        self.assertIn("Default.ini", report["exit_reason"])

    def test_smoke_runs_with_isolated_home_and_c_locale(self) -> None:
        observed = self.root / "observed-env"
        self._stub(
            'printf \'Commands for "ucc":\\nucc help <command>\\n\'; '
            'printf "%s\\n" "$HOME" "$LC_ALL" "$TZ" > \'' + str(observed) + "'"
        )
        report = run_ucc_help_smoke(self.binary, self.data_root)
        self.assertEqual(report["status"], "pass")
        lines = observed.read_text().splitlines()
        self.assertTrue(lines[0].startswith(tempfile.gettempdir()))
        self.assertEqual(lines[1], "C")
        self.assertEqual(lines[2], "UTC")

    def test_artifacts_land_in_artifact_dir(self) -> None:
        self._stub('printf \'Commands for "ucc":\\nucc help <command>\\n\'')
        artifacts = self.root / "artifacts"
        report = run_ucc_help_smoke(self.binary, self.data_root, artifact_dir=artifacts)
        self.assertEqual(report["status"], "pass")
        kinds = {entry["kind"] for entry in report["artifacts"]}
        self.assertIn("log", kinds)
        self.assertIn("report", kinds)
        self.assertTrue((artifacts / "ucc-help.log").is_file())
        self.assertTrue((artifacts / "ucc-help-report.json").is_file())
        persisted = json.loads((artifacts / "ucc-help-report.json").read_text())
        self.assertEqual(persisted["status"], "pass")
    def test_discovery_prefers_plain_preset_and_env_override(self) -> None:
        self.assertIsNone(discover_ucc_binary(str(self.root / "absent")))
        self.assertEqual(discover_ucc_binary(str(self.binary)), self.binary)
        import unittest.mock

        with unittest.mock.patch.dict(
            os.environ, {"HP2_UCC_BINARY": str(self.binary)}
        ):
            self.assertEqual(discover_ucc_binary(), self.binary)

    def test_profile_classification(self) -> None:
        self.assertEqual(classify_data_profile(Path("/repo/HarryPotter2/Unreal")), "data-prototype")
        self.assertEqual(classify_data_profile(Path("/repo/out/retail-data")), "data-retail")
        self.assertEqual(classify_data_profile(self.data_root), "data-none")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--smoke", action="store_true", help="run the live ucc help smoke")
    parser.add_argument("--ucc-binary", help="explicit hp2_ucc path (default: auto-discover)")
    parser.add_argument("--data-root", help="explicit read-only data root (default: auto-discover)")
    arguments = parser.parse_args()

    if arguments.smoke:
        binary = discover_ucc_binary(arguments.ucc_binary)
        data_root = resolve_data_root(arguments.data_root)
        report = run_ucc_help_smoke(binary, data_root)
        print(json.dumps(report, indent=2, sort_keys=True))
        if report["status"] == "pass":
            return 0
        if report["status"] == "blocked":
            return BLOCKED_EXIT_CODE
        return 1

    unittest.main(module="__main__", argv=[sys.argv[0], "-v"])


if __name__ == "__main__":
    sys.exit(main())
