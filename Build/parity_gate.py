#!/usr/bin/env python3
"""Run data-qualified C++/Rust functional-parity tranches.

The gate is intentionally strict about profile identity before it starts any
Rust test: a test that skips because its default fixture root is absent is not
allowed to turn an invalid data-root invocation green. Every successful parse
creates one artifact directory and records the canonical data identity in
``manifest.json`` before executing the requested stage.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import plistlib
import shlex
import shutil
import subprocess
import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Iterable, Mapping, Sequence


FORMAT_VERSION = 2
STAGES = ("format", "vm", "paired", "world", "campaign", "product")
FIXED_DT = "0.016666667"
RNG_SEED = "1337"
RENDERER = "xopengl"
COMPARATOR_PATHS = (
    "Build/parity_gate.py",
    "Build/game_test.py",
    "Build/package79_reference.py",
    "Build/check_bundle_rs.py",
    "Build/verify_playable.py",
    "Build/retail_flow_smoke.py",
    "Build/actor_transition_ledger.py",
    "Build/creature_generator_trace.py",
    "Build/fire_texture_trace.py",
    "Build/global_tick_trace.py",
    "Build/shadow_admission_trace.py",
    "Build/shadow_update_trace.py",
    "Build/startup_rng_phase_trace.py",
    "Tests/LocalizationTests.py",
    "Tests/SaveFormatTests.py",
    "crates/hp-engine/examples/data_identity.rs",
    "crates/hp-engine/src/data_identity.rs",
)
OPTIONAL_COMPARATOR_PATHS = ("Build/campaign_parity.py",)
FIDELITY_FIELD_NAMES = (
    "camera", "actors", "cutscene", "cues", "animation", "audio", "frame_hash",
)
FIXTURE_ROOTS = ("Tests/Fixtures",)
class GateError(RuntimeError):
    """A gate precondition or tranche failed with an actionable message."""


@dataclass
class GateContext:
    repo_root: Path
    profile: str
    data_root: Path
    cpp_app: Path
    rust_bin: Path
    rust_app: Path | None
    artifact_dir: Path
    data_identity: dict[str, object]
    source_identity: dict[str, object]
    input_hashes: list[dict[str, object]]
    stages: dict[str, dict[str, object]] = field(default_factory=dict)
    process_records: dict[str, list[dict[str, object]]] = field(default_factory=dict)

    def write_manifest(self) -> None:
        document = {
            "version": FORMAT_VERSION,
            "profile": self.profile,
            "repository_root": str(self.repo_root),
            "source": self.source_identity,
            "data_root": self.data_identity,
            "cpp_app": _executable_identity(self.cpp_app, "C++ application"),
            "rust_bin": _file_identity(self.rust_bin, self.repo_root),
            "rust_app": (
                _bundle_identity(self.rust_app, self.repo_root)
                if self.rust_app is not None
                else None
            ),
            "inputs": self.input_hashes,
            "stages": self.stages,
            # manifest.json is deliberately excluded: hashing it here would recurse.
            "generated_inventory": _generated_inventory(self.artifact_dir),
        }
        _write_json(self.artifact_dir / "manifest.json", document)


def _repo_root() -> Path:
    return Path(__file__).resolve().parent.parent


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()

def _display_path(path: Path, root: Path) -> str:
    try:
        return path.relative_to(root).as_posix()
    except ValueError:
        try:
            return path.resolve().relative_to(root.resolve()).as_posix()
        except ValueError:
            return str(path)


def _file_identity(path: Path, root: Path) -> dict[str, object]:
    return {
        "path": _display_path(path, root),
        "size": path.stat().st_size,
        "sha256": _sha256(path),
    }


def _bundle_files(bundle: Path) -> list[Path]:
    return sorted(
        (path for path in bundle.rglob("*") if path.is_file() or path.is_symlink()),
        key=lambda path: path.relative_to(bundle).as_posix(),
    )


def _bundle_executable(app: Path, label: str) -> Path:
    if not app.is_dir():
        return app
    plist_path = app / "Contents" / "Info.plist"
    try:
        with plist_path.open("rb") as stream:
            plist = plistlib.load(stream)
    except (OSError, plistlib.InvalidFileException, ValueError) as error:
        raise GateError(f"{label} Info.plist cannot be read: {plist_path}: {error}") from error
    name = plist.get("CFBundleExecutable") if isinstance(plist, dict) else None
    if not isinstance(name, str) or not name:
        raise GateError(f"{label} Info.plist has no CFBundleExecutable: {plist_path}")
    return app / "Contents" / "MacOS" / name


def _bundle_identity(bundle: Path, root: Path) -> dict[str, object]:
    inventory = []
    for path in _bundle_files(bundle):
        if path.is_symlink():
            target = os.readlink(path)
            inventory.append({
                "path": path.relative_to(bundle).as_posix(),
                "kind": "symlink",
                "target": target,
                "sha256": hashlib.sha256(os.fsencode(target)).hexdigest(),
            })
        else:
            inventory.append({
                "path": path.relative_to(bundle).as_posix(),
                "kind": "file",
                "size": path.stat().st_size,
                "sha256": _sha256(path),
            })
    encoded = json.dumps(inventory, separators=(",", ":"), sort_keys=True).encode("ascii")
    return {
        "path": _display_path(bundle, root),
        "inventory_sha256": hashlib.sha256(encoded).hexdigest(),
        "inventory": inventory,
        "executable": _file_identity(_bundle_executable(bundle, "Rust application"), bundle),
    }


def _executable_identity(app: Path, label: str) -> dict[str, object]:
    executable = _bundle_executable(app, label)
    return {"path": str(app), "executable": _file_identity(executable, app if app.is_dir() else app.parent)}


def _git_output(repo_root: Path, arguments: Sequence[str]) -> bytes:
    completed = subprocess.run(
        ("git", *arguments), cwd=repo_root, stdout=subprocess.PIPE,
        stderr=subprocess.PIPE, check=False,
    )
    if completed.returncode != 0:
        detail = completed.stderr.decode("utf-8", errors="replace").strip()
        raise GateError(f"cannot capture repository identity: git {' '.join(arguments)}: {detail}")
    return completed.stdout


def _source_identity(repo_root: Path) -> dict[str, object]:
    head = _git_output(repo_root, ("rev-parse", "HEAD")).decode("ascii").strip()
    changed = {
        part.decode("utf-8")
        for arguments in (
            ("ls-files", "-m", "-d", "-o", "--exclude-standard", "-z"),
            ("diff", "--cached", "--name-only", "-z"),
        )
        for part in _git_output(repo_root, arguments).split(b"\0")
        if part
    }
    records: list[dict[str, object]] = []
    for relative in sorted(changed):
        path = repo_root / relative
        if not (
            relative.startswith(("Build/", "Tests/", "crates/"))
            or Path(relative).suffix in {".c", ".cc", ".cpp", ".h", ".hpp", ".rs", ".py", ".json", ".toml"}
        ):
            continue
        records.append({"path": relative, "sha256": _sha256(path) if path.is_file() else None})
    encoded = json.dumps(records, separators=(",", ":"), sort_keys=True).encode("ascii")
    return {
        "head": head,
        "worktree_sha256": hashlib.sha256(encoded).hexdigest(),
        "worktree": records,
    }


def _input_hashes(repo_root: Path, data_root: Path) -> list[dict[str, object]]:
    paths = [repo_root / relative for relative in COMPARATOR_PATHS]
    paths.extend(
        path for path in (repo_root / relative for relative in OPTIONAL_COMPARATOR_PATHS)
        if path.is_file()
    )
    for relative in FIXTURE_ROOTS:
        root = repo_root / relative
        if root.is_dir():
            paths.extend(path for path in root.rglob("*") if path.is_file())
    for name in ("overlay-manifest.json", "overlay-checksums.txt"):
        path = data_root / name
        if path.is_file():
            paths.append(path)
    unique = sorted(set(paths), key=lambda path: _display_path(path, repo_root))
    missing = [path for path in unique if not path.is_file()]
    if missing:
        raise GateError("required comparator input is missing: " + ", ".join(map(str, missing)))
    return [_file_identity(path, repo_root) for path in unique]


def _generated_inventory(artifact_dir: Path) -> list[dict[str, object]]:
    if not artifact_dir.is_dir():
        return []
    return [
        _file_identity(path, artifact_dir)
        for path in sorted(
            (
                path for path in artifact_dir.rglob("*")
                if path.is_file() and path != artifact_dir / "manifest.json"
            ),
            key=lambda path: path.relative_to(artifact_dir).as_posix(),
        )
    ]


def _prepare_stage(context: GateContext, name: str) -> Path:
    stage_dir = context.artifact_dir / name
    if stage_dir.exists():
        raise GateError(f"stage artifact path already exists: {stage_dir}")
    stage_dir.mkdir()
    return stage_dir


def _seal_stage(
    context: GateContext,
    stage_dir: Path,
    report: dict[str, object],
    *,
    declared_files: Iterable[Path],
    declared_directories: Iterable[Path] = (),
) -> dict[str, object]:
    stage_root = stage_dir.resolve()
    files = {path.resolve() for path in declared_files}
    directories = tuple(path.resolve() for path in declared_directories)
    actual = {
        path.resolve() for path in stage_dir.rglob("*")
        if path.is_file() and path.resolve() != stage_root / "stage.json"
    }
    unexpected = sorted(
        path for path in actual
        if path not in files and not any(path.is_relative_to(root) for root in directories)
    )
    if unexpected:
        raise GateError(
            f"{stage_dir.name} stage produced undeclared artifact(s): "
            + ", ".join(str(path.relative_to(stage_root)) for path in unexpected)
        )
    missing = sorted(path for path in files if not path.is_file())
    if missing:
        raise GateError(
            f"{stage_dir.name} stage omitted declared artifact(s): " + ", ".join(map(str, missing))
        )
    report["declarations"] = {
        "files": sorted(_display_path(path, context.artifact_dir) for path in files),
        "directories": sorted(_display_path(path, context.artifact_dir) for path in directories),
    }
    report["artifacts"] = [
        _file_identity(path, context.artifact_dir)
        for path in sorted(actual, key=lambda path: path.relative_to(stage_root).as_posix())
    ]
    _write_json(stage_dir / "stage.json", report)
    return report

def _seal_failed_stage(
    context: GateContext,
    stage: str,
    error: GateError,
) -> dict[str, object]:
    """Seal every artifact and process result retained by a failed stage."""
    stage_dir = context.artifact_dir / stage
    stage_dir.mkdir(exist_ok=True)
    stage_root = stage_dir.resolve()
    actual = {
        path.resolve() for path in stage_dir.rglob("*")
        if path.is_file() and path.resolve() != stage_root / "stage.json"
    }
    ordered = sorted(
        actual, key=lambda path: path.relative_to(stage_root).as_posix()
    )
    report: dict[str, object] = {
        "identity": context.data_identity,
        "status": "failed",
        "error": str(error),
        "commands": context.process_records.get(stage, []),
        "declarations": {
            "files": [
                _display_path(path, context.artifact_dir) for path in ordered
            ],
            "directories": [],
        },
        "artifacts": [
            _file_identity(path, context.artifact_dir) for path in ordered
        ],
    }
    _write_json(stage_dir / "stage.json", report)
    return report


def _canonical_existing(path: Path, label: str) -> Path:
    try:
        resolved = path.expanduser().resolve(strict=True)
    except OSError as error:
        raise GateError(f"{label} cannot be resolved: {path}: {error}") from error
    return resolved


def _canonical_data_root(path: Path) -> Path:
    root = _canonical_existing(path, "data root")
    if not root.is_dir():
        raise GateError(f"data root is not a directory: {root}")
    default_ini = root / "System" / "Default.ini"
    if not default_ini.is_file():
        raise GateError(f"data root does not contain System/Default.ini: {root}")
    return root


def _validate_retail_overlay_with_runtime(
    root: Path,
    repo_root: Path,
) -> dict[str, object]:
    command = (
        _cargo_command(),
        "run",
        "-q",
        "-p",
        "hp-engine",
        "--example",
        "data_identity",
        "--",
        "--data-root",
        str(root),
    )
    completed = subprocess.run(
        command,
        cwd=repo_root,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    try:
        report = json.loads(completed.stdout)
    except json.JSONDecodeError as error:
        detail = completed.stderr.decode("utf-8", errors="replace").strip()
        raise GateError(
            "app.data_identity_validator: shared Rust validator emitted invalid "
            f"JSON: {error}; stderr={detail!r}"
        ) from error
    if not isinstance(report, dict):
        raise GateError(
            "app.data_identity_validator: shared Rust validator report is not an object"
        )
    if completed.returncode != 0 or report.get("status") != "passed":
        reason = report.get("reason_code")
        message = report.get("message")
        raise GateError(f"{reason}: {message}")
    if report.get("profile") != "retail-only":
        raise GateError(
            "app.data_identity_validator: validator did not qualify retail-only"
        )
    return report


def _validate_profile(profile: str, data_root: Path, repo_root: Path) -> dict[str, object]:
    prototype = _canonical_data_root(repo_root / "HarryPotter2" / "Unreal")
    default_ini = data_root / "System" / "Default.ini"
    identity: dict[str, object] = {
        "canonical_path": str(data_root),
        "default_ini_sha256": _sha256(default_ini),
    }
    if profile == "prototype":
        if data_root != prototype:
            raise GateError(
                "prototype profile requires the repository prototype root "
                f"{prototype}, got {data_root}"
            )
        identity["qualification"] = "repository-prototype"
        return identity

    if profile != "retail":
        raise GateError(f"unsupported profile: {profile!r}")
    if data_root == prototype:
        raise GateError("retail profile cannot use the repository prototype root")
    validation = _validate_retail_overlay_with_runtime(data_root, repo_root)
    overlay_path = data_root / "overlay-manifest.json"
    checksums_path = data_root / "overlay-checksums.txt"
    identity.update(
        {
            "qualification": "retail-overlay",
            "overlay_profile": validation["profile"],
            "overlay_manifest_sha256": _sha256(overlay_path),
            "overlay_checksums_sha256": _sha256(checksums_path),
            "validator": "hp-engine::data_identity::validate_retail_overlay_identity",
        }
    )
    return identity


def _validate_cpp_app(path: Path) -> Path:
    app = _canonical_existing(path, "C++ application")
    executable = _bundle_executable(app, "C++ application")
    if not executable.is_file():
        raise GateError(f"C++ application executable is missing: {executable}")
    if not os.access(executable, os.X_OK):
        raise GateError(f"C++ application is not executable: {executable}")
    return app


def _validate_rust_bin(path: Path) -> Path:
    executable = _canonical_existing(path, "Rust binary")
    if not executable.is_file():
        raise GateError(f"Rust binary is not a file: {executable}")
    if not os.access(executable, os.X_OK):
        raise GateError(f"Rust binary is not executable: {executable}")
    return executable

def _validate_rust_app(path: Path) -> Path:
    app = _canonical_existing(path, "Rust application")
    if not app.is_dir():
        raise GateError(f"Rust application is not a bundle directory: {app}")
    executable = _bundle_executable(app, "Rust application")
    if not executable.is_file() or not os.access(executable, os.X_OK):
        raise GateError(f"Rust application executable is missing or not executable: {executable}")
    return app


def _write_json(path: Path, document: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_text(
        json.dumps(document, ensure_ascii=True, allow_nan=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    temporary.replace(path)


def _write_bytes(path: Path, contents: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_bytes(contents)
    temporary.replace(path)


def _cargo_command() -> str:
    configured = Path.home() / ".cargo" / "bin" / "cargo"
    if configured.is_file():
        return str(configured)
    found = shutil.which("cargo")
    if found is None:
        raise GateError("cargo is unavailable; install Cargo or expose it in PATH")
    return found


def _command_text(command: Sequence[str]) -> str:
    return " ".join(shlex.quote(value) for value in command)


def _run_command(
    context: GateContext,
    *,
    stage_dir: Path,
    name: str,
    command: Sequence[str],
    environment: Mapping[str, str] | None = None,
    stdout_path: Path | None = None,
) -> dict[str, object]:
    merged_environment = os.environ.copy()
    if environment is not None:
        merged_environment.update(environment)
    completed = subprocess.run(
        list(command),
        cwd=context.repo_root,
        env=merged_environment,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if stdout_path is not None:
        _write_bytes(stdout_path, completed.stdout)
    log_path = stage_dir / f"{name}.log"
    log = (
        f"$ {_command_text(command)}\n"
        f"exit_status={completed.returncode}\n"
        "--- stdout ---\n"
    ).encode("utf-8") + completed.stdout + b"\n--- stderr ---\n" + completed.stderr
    _write_bytes(log_path, log)
    record: dict[str, object] = {
        "command": list(command),
        "exit_status": completed.returncode,
        "log": str(log_path.relative_to(context.artifact_dir)),
    }
    if stdout_path is not None:
        record["stdout"] = str(stdout_path.relative_to(context.artifact_dir))
    context.process_records.setdefault(stage_dir.name, []).append(record)
    if completed.returncode != 0:
        raise GateError(
            f"{name} failed with exit status {completed.returncode}; "
            f"see {log_path}"
        )
    return record


def _first_json_difference(reference: object, rust: object, path: str = "$") -> dict[str, object] | None:
    if type(reference) is not type(rust):
        return {"path": path, "reference": reference, "rust": rust}
    if isinstance(reference, dict):
        reference_keys = set(reference)
        rust_keys = set(rust)
        for key in sorted(reference_keys | rust_keys):
            key_path = f"{path}.{key}"
            if key not in reference:
                return {"path": key_path, "reference": {"missing": True}, "rust": rust[key]}
            if key not in rust:
                return {"path": key_path, "reference": reference[key], "rust": {"missing": True}}
            difference = _first_json_difference(reference[key], rust[key], key_path)
            if difference is not None:
                return difference
        return None
    if isinstance(reference, list):
        common = min(len(reference), len(rust))
        for index in range(common):
            difference = _first_json_difference(reference[index], rust[index], f"{path}[{index}]")
            if difference is not None:
                return difference
        if len(reference) != len(rust):
            return {
                "path": f"{path}.length",
                "reference": len(reference),
                "rust": len(rust),
            }
        return None
    if reference != rust:
        return {"path": path, "reference": reference, "rust": rust}
    return None


def _compare_audits(reference_path: Path, rust_path: Path, report_path: Path) -> None:
    reference_bytes = reference_path.read_bytes()
    rust_bytes = rust_path.read_bytes()
    if reference_bytes == rust_bytes:
        _write_json(report_path, {"status": "match", "byte_equal": True})
        return
    try:
        reference = json.loads(reference_bytes)
        rust = json.loads(rust_bytes)
    except json.JSONDecodeError as error:
        _write_json(
            report_path,
            {
                "status": "mismatch",
                "byte_equal": False,
                "first_difference": {"path": "$", "reference": "invalid JSON", "rust": str(error)},
            },
        )
        raise GateError(f"package79 audit byte mismatch and JSON parse failed; see {report_path}") from error
    difference = _first_json_difference(reference, rust)
    if difference is None:
        difference = {
            "path": "$",
            "reference": "canonical JSON byte representation",
            "rust": "canonical JSON byte representation differs",
        }
    _write_json(
        report_path,
        {"status": "mismatch", "byte_equal": False, "first_difference": difference},
    )
    raise GateError(
        "package79 audit byte mismatch at "
        f"{difference['path']}; see {report_path}"
    )


def _run_format(context: GateContext) -> dict[str, object]:
    stage_dir = _prepare_stage(context, "format")
    prototype_root = _canonical_data_root(
        context.repo_root / "HarryPotter2" / "Unreal"
    )
    roots = [("prototype", prototype_root)]
    if context.profile == "retail":
        roots.append(("retail", context.data_root))

    commands: list[dict[str, object]] = []
    audits: list[dict[str, object]] = []
    declared_files: list[Path] = []
    for label, data_root in roots:
        reference = stage_dir / f"{label}-package79-reference.json"
        rust = stage_dir / f"{label}-p79audit.json"
        diff = stage_dir / f"{label}-package79-diff.json"
        reference_name = f"{label}-package79-reference"
        rust_name = f"{label}-p79audit"
        commands.append(_run_command(
            context,
            stage_dir=stage_dir,
            name=reference_name,
            command=(
                sys.executable,
                "Build/package79_reference.py",
                "--data-root",
                str(data_root),
                "--output",
                str(reference),
            ),
        ))
        commands.append(_run_command(
            context,
            stage_dir=stage_dir,
            name=rust_name,
            command=(
                _cargo_command(),
                "run",
                "-q",
                "-p",
                "hp-format",
                "--example",
                "p79audit",
                "--",
                "--data-root",
                str(data_root),
                "--repo-root",
                str(context.repo_root),
            ),
            stdout_path=rust,
        ))
        if not reference.is_file() or not rust.is_file():
            raise GateError(
                f"{label} format audit did not create both canonical JSON documents"
            )
        _compare_audits(reference, rust, diff)
        audits.append({
            "profile": label,
            "data_root": str(data_root),
            "reference": str(reference.relative_to(context.artifact_dir)),
            "rust": str(rust.relative_to(context.artifact_dir)),
            "comparison": str(diff.relative_to(context.artifact_dir)),
        })
        declared_files.extend((
            reference,
            rust,
            diff,
            stage_dir / f"{reference_name}.log",
            stage_dir / f"{rust_name}.log",
        ))

    commands.append(_run_command(
        context,
        stage_dir=stage_dir,
        name="config-ini",
        command=(
            _cargo_command(),
            "test",
            "-p",
            "hp-ini",
            "--test",
            "config_ini_oracle",
        ),
    ))
    declared_files.append(stage_dir / "config-ini.log")
    report = {
        "identity": context.data_identity,
        "status": "passed",
        "commands": commands,
        "audits": audits,
    }
    return _seal_stage(
        context,
        stage_dir,
        report,
        declared_files=declared_files,
    )


def _vm_report_violations(
    census: object,
    ledger: object,
    expected_maps: Sequence[str],
) -> list[str]:
    violations: list[str] = []
    expected = list(expected_maps)

    def report_maps(document: object, label: str) -> list[object]:
        if not isinstance(document, dict) or not isinstance(document.get("maps"), list):
            violations.append(f"{label} has no maps array")
            return []
        return document["maps"]

    def map_tokens(items: Sequence[object], label: str) -> list[str]:
        tokens: list[str] = []
        for index, item in enumerate(items):
            identity = item.get("identity") if isinstance(item, dict) else None
            token = identity.get("map_token") if isinstance(identity, dict) else None
            if not isinstance(token, str):
                violations.append(f"{label} map[{index}] has no identity.map_token")
            else:
                tokens.append(token)
        return tokens

    census_maps = report_maps(census, "script census")
    ledger_maps = report_maps(ledger, "execution ledger")
    for label, items in (("script census", census_maps), ("execution ledger", ledger_maps)):
        tokens = map_tokens(items, label)
        if tokens != expected:
            violations.append(
                f"{label} map coverage mismatch: expected {expected!r}, observed {tokens!r}"
            )

    for item in census_maps:
        if not isinstance(item, dict):
            continue
        identity = item.get("identity")
        token = identity.get("map_token") if isinstance(identity, dict) else "<unknown>"
        if item.get("zero_streams") is not False:
            violations.append(f"{token}: structural census has zero or unreported streams")
        if item.get("map_failure") is not None:
            violations.append(f"{token}: structural census map load failed")
        for field in (
            "token_aborts",
            "native_aborts",
            "actors_without_class",
            "class_chain_skips",
        ):
            entries = item.get(field)
            if not isinstance(entries, list):
                violations.append(f"{token}: structural census omitted {field}")
            elif entries:
                violations.append(f"{token}: structural census reported {field}")

    if isinstance(ledger, dict):
        numeric_zero_fields = (
            "failed_count",
            "aborted_count",
            "deferred_count",
            "script_deferred",
        )
        for field in numeric_zero_fields:
            value = ledger.get(field)
            if type(value) is not int or value != 0:
                violations.append(f"execution ledger {field} must be zero, found {value!r}")
        stream_count = ledger.get("stream_count")
        completed_count = ledger.get("completed_count")
        suspended_count = ledger.get("suspended_count")
        if type(stream_count) is not int or stream_count <= 0:
            violations.append("execution ledger stream_count must be positive")
        if type(completed_count) is not int or type(suspended_count) is not int:
            violations.append(
                "execution ledger completed_count and suspended_count must be integers"
            )
        elif completed_count + suspended_count != stream_count:
            violations.append(
                "execution ledger completed_count plus suspended_count does not equal stream_count"
            )
        for field in ("reason_census", "subject_census"):
            census_value = ledger.get(field)
            if not isinstance(census_value, dict) or census_value:
                violations.append(f"execution ledger {field} must be an empty object")
        for item in ledger_maps:
            if not isinstance(item, dict):
                continue
            identity = item.get("identity")
            token = identity.get("map_token") if isinstance(identity, dict) else "<unknown>"
            records = item.get("records")
            if item.get("map_outcome") != "completed":
                violations.append(f"{token}: map execution did not complete")
            if not isinstance(records, list) or not records:
                violations.append(f"{token}: map execution has no stream outcomes")
            elif any(
                not isinstance(record, dict)
                or record.get("outcome") not in ("completed", "suspended")
                for record in records
            ):
                violations.append(f"{token}: map execution contains failed outcomes")
            for field in ("reason_census", "subject_census"):
                value = item.get(field)
                if not isinstance(value, dict) or value:
                    violations.append(f"{token}: map {field} must be an empty object")
    return violations


def _run_vm(context: GateContext) -> dict[str, object]:
    stage_dir = _prepare_stage(context, "vm")
    expected_maps = _all_maps(context.data_root, context.repo_root)
    selected = ",".join(expected_maps)
    environment = {
        "HP2_PARITY_DATA_ROOT": str(context.data_root),
        "HP2_ARTIFACT_DIR": str(stage_dir),
        "HP2_CENSUS_MD": str(stage_dir / "script-census.md"),
        "HP2_CENSUS_MAPS": selected,
        "HP2_VM_EXEC_MAPS": selected,
    }
    cargo = _cargo_command()
    commands: list[dict[str, object]] = []
    command_errors: list[str] = []
    for name, test in (
        ("script-census", "script_census"),
        ("script-vm-exec", "script_vm_exec"),
    ):
        try:
            commands.append(_run_command(
                context,
                stage_dir=stage_dir,
                name=name,
                command=(cargo, "test", "-p", "hp-engine", "--test", test, "--", "--nocapture"),
                environment=environment,
            ))
        except GateError as error:
            command_errors.append(str(error))
            commands.append(context.process_records["vm"][-1])

    reports = {
        "script_census_markdown": stage_dir / "script-census.md",
        "script_census_json": stage_dir / "script-census.json",
        "script_execution_ledger": stage_dir / "script-vm-exec.json",
    }
    missing = [name for name, path in reports.items() if not path.is_file()]
    census: object = {}
    ledger: object = {}
    parse_errors: list[str] = []
    for label, path in (
        ("script census", reports["script_census_json"]),
        ("execution ledger", reports["script_execution_ledger"]),
    ):
        if not path.is_file():
            continue
        try:
            document = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError) as error:
            parse_errors.append(f"cannot read {label}: {error}")
            continue
        if label == "script census":
            census = document
        else:
            ledger = document
    violations = _vm_report_violations(census, ledger, expected_maps)
    violations.extend(f"missing required artifact: {name}" for name in missing)
    violations.extend(parse_errors)
    violations.extend(command_errors)

    validation = stage_dir / "script-vm-exec-validation.json"
    _write_json(
        validation,
        {
            "status": "failed" if violations else "passed",
            "expected_maps": expected_maps,
            "violations": violations,
        },
    )
    if violations:
        raise GateError(
            "VM coverage/execution validation failed; see "
            f"{validation.relative_to(context.artifact_dir)}"
        )
    report = {
        "identity": context.data_identity,
        "status": "passed",
        "commands": commands,
        "reports": {
            **{
                name: str(path.relative_to(context.artifact_dir))
                for name, path in reports.items()
            },
            "script_execution_validation": str(validation.relative_to(context.artifact_dir)),
        },
    }
    return _seal_stage(
        context,
        stage_dir,
        report,
        declared_files=(
            *reports.values(),
            validation,
            stage_dir / "script-census.log",
            stage_dir / "script-vm-exec.log",
        ),
    )


def _game_test_module(repo_root: Path) -> Any:
    sys.path.insert(0, str(repo_root / "Build"))
    try:
        import game_test  # pylint: disable=import-outside-toplevel
    except ImportError as error:
        raise GateError(f"cannot import paired harness: {error}") from error
    return game_test


def _checkpoint_maps(data_root: Path, repo_root: Path) -> list[str]:
    game_test = _game_test_module(repo_root)
    selected: list[str] = []
    for path, map_name in game_test._enumerate_maps(data_root):
        try:
            checkpoints = game_test._source_checkpoints(data_root, path)
        except game_test.GameTestError:
            continue
        if checkpoints:
            selected.append(map_name)
    if not selected:
        raise GateError(
            "no map has a resolvable authored CutScript checkpoint; "
            "paired parity cannot be inferred from a smoke launch"
        )
    return selected


def _all_maps(data_root: Path, repo_root: Path) -> list[str]:
    maps = [map_name for _path, map_name in _game_test_module(repo_root)._enumerate_maps(data_root)]
    if not maps:
        raise GateError("world parity requires at least one enumerated map")
    return maps


def _load_json_object(path: Path, label: str) -> dict[str, object]:
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise GateError(f"cannot read {label} {path}: {error}") from error
    if not isinstance(document, dict):
        raise GateError(f"{label} is not a JSON object: {path}")
    return document


def _validate_paired_report(
    document: Mapping[str, object],
    *,
    required_auxiliaries: Iterable[str] = (),
) -> None:
    if document.get("passed") is not True:
        raise GateError("paired report did not pass")
    runs = document.get("runs")
    if not isinstance(runs, dict):
        raise GateError("paired report has no runs object")
    for side in ("cpp", "rust"):
        run = runs.get(side)
        if not isinstance(run, dict) or run.get("passed") is not True:
            raise GateError(f"paired report {side} run is missing or failed")
        if run.get("script_deferral_violation") is not None:
            raise GateError(f"paired report {side} run has script deferral telemetry violation")
        resources = run.get("resources")
        if not isinstance(resources, dict):
            raise GateError(f"paired report {side} run has no resources telemetry")
        if resources.get("script_deferred") != 0:
            raise GateError(f"paired report {side} run deferred scripts")
        for key in ("script_deferral_reasons", "script_deferral_subjects"):
            if resources.get(key) != {}:
                raise GateError(f"paired report {side} run has incomplete or non-empty {key}")
    comparisons = document.get("reports")
    if not isinstance(comparisons, list) or not comparisons:
        raise GateError("paired report has no checkpoint comparisons")
    for index, record in enumerate(comparisons):
        comparison = record.get("comparison") if isinstance(record, dict) else None
        if not isinstance(comparison, dict) or comparison.get("status") != "match":
            raise GateError(f"paired report checkpoint comparison {index} is missing or failed")
        if comparison.get("divergence", object()) is not None:
            raise GateError(f"paired report checkpoint comparison {index} is incomplete")
        for side in ("cpp", "rust"):
            observation = record.get(side) if isinstance(record, dict) else None
            missing_fields = [
                field for field in FIDELITY_FIELD_NAMES
                if not isinstance(observation, dict) or field not in observation
            ]
            if missing_fields:
                raise GateError(
                    f"paired report checkpoint {index} {side} observation is incomplete: "
                    + ", ".join(missing_fields)
                )
    actor_join = document.get("actor_slot_semantic_join")
    actor_checkpoints = actor_join.get("checkpoints") if isinstance(actor_join, dict) else None
    if (
        not isinstance(actor_join, dict)
        or actor_join.get("status") != "observed"
        or actor_join.get("matches") is not True
        or not isinstance(actor_checkpoints, list)
        or len(actor_checkpoints) != 3
    ):
        raise GateError("paired report actor-slot semantic comparison is missing")
    expected_actor_checkpoints = {
        "post_deserialize_raw",
        "post_startup_before_first_tick",
        "post_startup",
    }
    if {
        checkpoint.get("checkpoint") for checkpoint in actor_checkpoints
        if isinstance(checkpoint, dict)
    } != expected_actor_checkpoints:
        raise GateError("paired report actor-slot checkpoints are incomplete")
    for checkpoint in actor_checkpoints:
        if not isinstance(checkpoint, dict):
            raise GateError("paired report actor-slot semantic comparison is incomplete")
        orders = checkpoint.get("normalized_order")
        null_positions = checkpoint.get("null_positions")
        boundaries = checkpoint.get("boundaries")
        if (
            checkpoint.get("matches") is not True
            or checkpoint.get("multiset_difference") != []
            or checkpoint.get("ambiguous_groups") != []
            or checkpoint.get("order_match") is not True
            or checkpoint.get("null_positions_match") is not True
            or checkpoint.get("boundaries_match") is not True
            or not isinstance(orders, dict)
            or not isinstance(orders.get("cpp"), list)
            or orders.get("cpp") != orders.get("rust")
            or not isinstance(null_positions, dict)
            or not isinstance(null_positions.get("cpp"), list)
            or null_positions.get("cpp") != null_positions.get("rust")
            or not isinstance(boundaries, dict)
            or not isinstance(boundaries.get("cpp"), dict)
            or boundaries.get("cpp") != boundaries.get("rust")
        ):
            raise GateError("paired report actor-slot order, nulls, boundaries, or semantics diverged")
        expected_nulls = [
            index for index, actor in enumerate(orders["cpp"]) if actor is None
        ]
        if null_positions["cpp"] != expected_nulls:
            raise GateError("paired report actor-slot null positions are inconsistent")
        cpp_boundaries = boundaries["cpp"]
        first_net = cpp_boundaries.get("i_first_net_relevant_actor")
        first_dynamic = cpp_boundaries.get("i_first_dynamic_actor")
        if checkpoint["checkpoint"] == "post_deserialize_raw":
            valid_boundaries = first_net is None and first_dynamic is None
        else:
            valid_boundaries = (
                type(first_net) is int
                and type(first_dynamic) is int
                and 0 <= first_net <= first_dynamic <= len(orders["cpp"])
            )
        if not valid_boundaries:
            raise GateError("paired report actor-slot boundaries are invalid")
    seed = document.get("rng_seed")
    if not isinstance(seed, dict) or seed.get("status") != "match":
        raise GateError("paired report RNG seed comparison is missing or failed")
    for name in required_auxiliaries:
        auxiliary = document.get(name)
        comparison = auxiliary.get("comparison") if isinstance(auxiliary, dict) else None
        if not isinstance(comparison, dict) or comparison.get("status") != "match":
            raise GateError(f"paired report {name} comparison is missing or failed")


PAIRED_AUXILIARIES = (
    "shadow_update_trace",
    "shadow_admission_trace",
    "actor_transition_ledger",
    "creature_generator_trace",
    "global_tick_trace",
    "startup_rng_phase_trace",
    "fire_texture_trace",
)


def _paired_command(
    context: GateContext,
    map_name: str,
    report: Path,
    trace_root: Path,
    ticks: int,
) -> tuple[str, ...]:
    command = [
        sys.executable, "Build/game_test.py", "paired",
        "--app", str(context.cpp_app),
        "--engine-bin", str(context.rust_bin),
        "--data-root", str(context.data_root),
        "--map", map_name,
        "--renderer", RENDERER,
        "--ticks", str(ticks),
        "--fixed-dt", FIXED_DT,
        "--rng-seed", RNG_SEED,
        "--rng-trace-dir", str(trace_root / "rng"),
        "--rng-trace-limit", "4096",
        "--checkpoints", "authored",
        "--timeout", "180",
        "--output", str(report),
    ]
    for option, directory in (
        ("--shadow-update-trace-dir", "shadow-update"),
        ("--shadow-admission-trace-dir", "shadow-admission"),
        ("--actor-transition-ledger-dir", "actor-transition"),
        ("--creature-generator-trace-dir", "creature-generator"),
        ("--global-tick-trace-dir", "global-tick"),
        ("--startup-rng-phase-trace-dir", "startup-rng-phase"),
        ("--fire-texture-trace-dir", "fire-texture"),
        ("--actor-slot-dump-dir", "actor-slot"),
    ):
        command.extend((option, str(trace_root / directory)))
    return tuple(command)


def _run_pair_set(
    context: GateContext,
    stage_name: str,
    maps: Sequence[str],
    *,
    ticks: int,
) -> dict[str, object]:
    stage_dir = _prepare_stage(context, stage_name)
    commands: list[dict[str, object]] = []
    reports: list[str] = []
    declared_files: list[Path] = []
    declared_directories: list[Path] = []
    for index, map_name in enumerate(maps):
        stem = f"{index:04d}-{Path(map_name.replace(chr(92), '/')).stem.casefold()}"
        report_path = stage_dir / "reports" / f"{stem}.json"
        trace_root = stage_dir / "traces" / stem
        commands.append(_run_command(
            context,
            stage_dir=stage_dir,
            name=stem,
            command=_paired_command(context, map_name, report_path, trace_root, ticks),
        ))
        if not report_path.is_file():
            raise GateError(f"paired harness did not create its report: {report_path}")
        document = _load_json_object(report_path, "paired report")
        _validate_paired_report(document, required_auxiliaries=PAIRED_AUXILIARIES)
        reports.append(str(report_path.relative_to(context.artifact_dir)))
        declared_files.extend((report_path, stage_dir / f"{stem}.log"))
        declared_directories.extend((trace_root, report_path.parent / f"{report_path.stem}-artifacts"))
    result = {
        "identity": context.data_identity,
        "status": "passed",
        "settings": {
            "fixed_dt": FIXED_DT,
            "rng_seed": int(RNG_SEED),
            "renderer": RENDERER,
            "ticks": ticks,
        },
        "commands": commands,
        "reports": reports,
    }
    return _seal_stage(
        context, stage_dir, result,
        declared_files=declared_files,
        declared_directories=declared_directories,
    )


def _run_paired(context: GateContext) -> dict[str, object]:
    return _run_pair_set(
        context, "paired", _checkpoint_maps(context.data_root, context.repo_root), ticks=120
    )


def _run_world(context: GateContext) -> dict[str, object]:
    return _run_pair_set(
        context, "world", _all_maps(context.data_root, context.repo_root), ticks=300
    )


def _run_campaign(context: GateContext) -> dict[str, object]:
    stage_dir = _prepare_stage(context, "campaign")
    manifest = context.repo_root / "Tests" / "Fixtures" / "campaign-parity" / "manifest.json"
    runner = context.repo_root / "Build" / "campaign_parity.py"
    missing = [path for path in (manifest, runner) if not path.is_file()]
    prerequisite = {
        "status": "failed" if missing else "passed",
        "manifest": str(manifest),
        "runner": str(runner),
        "missing": [str(path) for path in missing],
    }
    _write_json(stage_dir / "prerequisites.json", prerequisite)
    if missing:
        raise GateError(
            "campaign parity prerequisites are absent: " + ", ".join(str(path) for path in missing)
        )
    raise GateError(
        "campaign parity runner exists but step 11 integration is not yet implemented in this gate"
    )


def _run_product(context: GateContext) -> dict[str, object]:
    stage_dir = _prepare_stage(context, "product")
    if context.rust_app is None:
        raise GateError("product stage requires --rust-app")
    bundle_report = stage_dir / "bundle-integrity.json"
    commands = [
        _run_command(
            context, stage_dir=stage_dir, name="bundle-integrity",
            command=(
                sys.executable, "Build/check_bundle_rs.py",
                "--repo-root", str(context.repo_root),
                "--bundle", str(context.rust_app),
                "--output", str(bundle_report),
            ),
            environment={"HP2_ARTIFACT_DIR": str(stage_dir / "bundle-artifacts")},
        ),
        _run_command(
            context, stage_dir=stage_dir, name="launcher-input",
            command=(_cargo_command(), "test", "-p", "hp-app"),
        ),
        _run_command(
            context, stage_dir=stage_dir, name="localization",
            command=(sys.executable, "Tests/LocalizationTests.py"),
        ),
        _run_command(
            context, stage_dir=stage_dir, name="save-format",
            command=(sys.executable, "Tests/SaveFormatTests.py"),
        ),
        _run_command(
            context, stage_dir=stage_dir, name="replay",
            command=(_cargo_command(), "test", "-p", "hp-engine", "replay::tests"),
        ),
        _run_command(
            context, stage_dir=stage_dir, name="launch-services",
            command=(sys.executable, "Build/verify_playable.py", "--bundle", str(context.rust_app)),
            environment={"HP2_ARTIFACT_DIR": str(stage_dir / "launch-services-artifacts")},
        ),
        _run_command(
            context, stage_dir=stage_dir, name="retail-flow",
            command=(
                sys.executable, "Build/retail_flow_smoke.py",
                "--app", str(context.rust_app),
                "--data-root", str(context.data_root),
                "--artifact-dir", str(stage_dir / "retail-flow-artifacts"),
            ),
        ),
    ]
    bundle = _load_json_object(bundle_report, "bundle integrity report")
    if bundle.get("status") != "pass":
        raise GateError("Rust bundle integrity report did not pass")
    missing_capabilities = [
        "verify_playable.py --expected-profile",
        "retail_flow_smoke.py --save",
        "retail_flow_smoke.py --expected-state",
    ]
    campaign_final = context.artifact_dir / "campaign" / "final" / "rust"
    campaign_inputs = {
        name: (
            _file_identity(campaign_final / name, context.artifact_dir)
            if (campaign_final / name).is_file()
            else None
        )
        for name in ("Save0.usa", "campaign-state.json")
    }
    prerequisite_path = stage_dir / "later-flow-prerequisites.json"
    _write_json(
        prerequisite_path,
        {
            "status": "failed",
            "missing_capabilities": missing_capabilities,
            "retail_campaign_inputs": campaign_inputs if context.profile == "retail" else None,
        },
    )
    raise GateError(
        "product parity prerequisites are not implemented by the current product scripts: "
        + ", ".join(missing_capabilities)
    )


STAGE_RUNNERS = {
    "format": _run_format,
    "vm": _run_vm,
    "paired": _run_paired,
    "world": _run_world,
    "campaign": _run_campaign,
    "product": _run_product,
}


def _selected_stages(stage: str) -> tuple[str, ...]:
    if stage == "all":
        return STAGES
    if stage not in STAGE_RUNNERS:
        raise GateError(f"unsupported stage: {stage!r}")
    return (stage,)


def _require_rust_app(stage: str, rust_app: Path | None) -> None:
    if stage in ("product", "all") and rust_app is None:
        raise GateError(f"--rust-app is required for --stage {stage}")


def _prepare_artifact_dir(path: Path) -> Path:
    artifact_dir = path.expanduser().resolve()
    if artifact_dir.exists():
        if not artifact_dir.is_dir():
            raise GateError(f"artifact directory is not a directory: {artifact_dir}")
        try:
            occupied = next(artifact_dir.iterdir(), None)
        except OSError as error:
            raise GateError(f"cannot inspect artifact directory {artifact_dir}: {error}") from error
        if occupied is not None:
            raise GateError(f"artifact directory must be new and empty: {artifact_dir}")
    else:
        artifact_dir.mkdir(parents=True)
    return artifact_dir


def _arguments(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--profile", required=True, choices=("prototype", "retail"))
    parser.add_argument("--data-root", required=True, type=Path)
    parser.add_argument("--cpp-app", required=True, type=Path)
    parser.add_argument("--rust-bin", required=True, type=Path)
    parser.add_argument("--rust-app", type=Path)
    parser.add_argument("--artifact-dir", required=True, type=Path)
    parser.add_argument("--stage", required=True, choices=(*STAGES, "all"))
    return parser.parse_args(argv)


def _context(arguments: argparse.Namespace) -> GateContext:
    repo_root = _repo_root()
    data_root = _canonical_data_root(arguments.data_root)
    # This happens before any stage can invoke a data-backed Rust test.
    identity = _validate_profile(arguments.profile, data_root, repo_root)
    cpp_app = _validate_cpp_app(arguments.cpp_app)
    rust_bin = _validate_rust_bin(arguments.rust_bin)
    stage = getattr(arguments, "stage", "format")
    rust_app_argument = getattr(arguments, "rust_app", None)
    _require_rust_app(stage, rust_app_argument)
    rust_app = _validate_rust_app(rust_app_argument) if rust_app_argument is not None else None
    artifact_dir = _prepare_artifact_dir(arguments.artifact_dir)
    return GateContext(
        repo_root=repo_root,
        profile=arguments.profile,
        data_root=data_root,
        cpp_app=cpp_app,
        rust_bin=rust_bin,
        rust_app=rust_app,
        artifact_dir=artifact_dir,
        data_identity=identity,
        source_identity=_source_identity(repo_root),
        input_hashes=_input_hashes(repo_root, data_root),
    )


def main(argv: Sequence[str] | None = None) -> int:
    try:
        arguments = _arguments(argv)
        context = _context(arguments)
        context.write_manifest()
        for stage in _selected_stages(arguments.stage):
            try:
                result = STAGE_RUNNERS[stage](context)
            except GateError as error:
                failure = _seal_failed_stage(context, stage, error)
                context.stages[stage] = failure
                context.write_manifest()
                print(f"parity_gate.py: {error}", file=sys.stderr)
                return 1
            context.stages[stage] = result
            context.write_manifest()
        return 0
    except GateError as error:
        print(f"parity_gate.py: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
