#!/usr/bin/env python3
"""Run the bounded native-package ``LifecycleGate`` C++/Rust parity scenario.

This harness deliberately has no dependency on the CutScript checkpoint protocol
or UCC. It verifies the committed fixture package's declared provenance and SHA-256,
stages its package/map bytes into isolated C++ and Rust data roots, launches both
engines for exactly two fixed-delta ticks, and compares the atomic lifecycle traces.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
import shutil
import sys
import threading
from typing import Mapping, Sequence

import game_test


TRACE_VERSION = 1
FIXTURE_NAME = "LifecycleGate"
FIXED_DT = 0.016666667
EMITTED_FIXED_DT = 0.0166666675
REQUESTED_TICKS = 2
MAP_PATH = "Maps/LifecycleGate.unr"
ACTOR_CLASS = "LifecycleGateActor"
ACTOR_TAG = "LifecycleGate"
GAME_CLASS = "LifecycleGate.LifecycleGateGame"
TRACE_ENVIRONMENT = "HP2_LIFECYCLE_GATE_TRACE"
MANIFEST_NAME = "manifest.json"
DEFAULT_TIMEOUT_SECONDS = 60.0
LIFECYCLE_PHASES = (
    "Spawned",
    "PreBeginPlay",
    "BeginPlay",
    "PostBeginPlay",
    "SetInitialState",
)
CHECKPOINT_IDS = ("spawn", "scan", "await_resume", "destroy")
CHECKPOINT_EVENTS = {
    "spawn": LIFECYCLE_PHASES,
    "scan": ("AllActorsYield", "AllActorsYield", "AllActorsExhausted"),
    "await_resume": ("EndState", "BeginState", "Sleep"),
    "destroy": ("EndState", "Destroyed", "DestroyActor"),
}
PROPERTY_FIELDS = (
    "SpawnedCount",
    "PreBeginPlayCount",
    "BeginPlayCount",
    "PostBeginPlayCount",
    "SetInitialStateCount",
    "BeginStateCount",
    "EndStateCount",
    "IteratorYieldCount",
    "DestroyedCount",
    "StateStage",
)
EXPECTED_COUNTERS = {
    "spawn": (1, 1, 1, 1, 1, 1, 0, 0, 0, 0),
    "scan": (1, 1, 1, 1, 1, 1, 0, 2, 0, 10),
    "await_resume": (1, 1, 1, 1, 1, 2, 1, 2, 0, 20),
    "destroy": (1, 1, 1, 1, 1, 2, 2, 2, 1, 30),
}
EXPECTED_TICKS = {"spawn": 0, "scan": 0, "await_resume": 0, "destroy": 1}
EXPECTED_STATES = {
    "spawn": "Scan",
    "scan": "Scan",
    "await_resume": "AwaitResume",
    "destroy": "AwaitResume",
}



class LifecycleGateError(RuntimeError):
    """The committed lifecycle fixture or either trace violates its contract."""


@dataclass(frozen=True)
class FixtureManifest:
    root: Path
    package_source: PurePosixPath
    package_output: PurePosixPath
    package_sha256: str
    package_provenance: dict[str, str]
    map_path: PurePosixPath
    map_source: PurePosixPath
    manifest_sha256: str


@dataclass(frozen=True)
class StagedFixture:
    cpp_data_root: Path
    rust_data_root: Path
    files: dict[str, dict[str, str]]
    package_artifact: dict[str, object]


def _repo_root() -> Path:
    return Path(__file__).resolve().parent.parent


def _write_json_atomic(path: Path, document: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_text(
        json.dumps(document, ensure_ascii=True, allow_nan=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    temporary.replace(path)


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def _resolve(path: Path, label: str, *, must_exist: bool = True) -> Path:
    candidate = path.expanduser()
    try:
        return candidate.resolve(strict=must_exist)
    except OSError as error:
        raise LifecycleGateError(f"cannot resolve {label} {path}: {error}") from error


def _require_file(path: Path, label: str, *, executable: bool = False) -> Path:
    resolved = _resolve(path, label)
    if not resolved.is_file():
        raise LifecycleGateError(f"{label} is not a regular file: {resolved}")
    if executable and not os.access(resolved, os.X_OK):
        raise LifecycleGateError(f"{label} is not executable: {resolved}")
    return resolved


def _require_data_root(path: Path) -> Path:
    root = _resolve(path, "data root")
    if not root.is_dir():
        raise LifecycleGateError(f"data root is not a directory: {root}")
    if not (root / "System" / "Default.ini").is_file():
        raise LifecycleGateError(f"data root lacks System/Default.ini: {root}")
    return root


def _manifest_relative_path(value: object, label: str) -> PurePosixPath:
    if not isinstance(value, str) or not value:
        raise LifecycleGateError(f"fixture manifest {label} must be a non-empty string")
    candidate = PurePosixPath(value)
    if candidate.is_absolute() or not candidate.parts or any(part in ("", ".", "..") for part in candidate.parts):
        raise LifecycleGateError(f"fixture manifest {label} must be a safe relative POSIX path: {value!r}")
    return candidate


def _required_object(value: object, label: str) -> Mapping[str, object]:
    if not isinstance(value, dict):
        raise LifecycleGateError(f"{label} must be a JSON object")
    return value


def _manifest_sha256(value: object, label: str) -> str:
    if value is None:
        raise LifecycleGateError(
            f"fixture manifest {label} is missing; "
            "commit the native artifact and record its lowercase SHA-256"
        )
    if not isinstance(value, str) or len(value) != 64 or any(character not in "0123456789abcdef" for character in value):
        raise LifecycleGateError(f"fixture manifest {label} must be a lowercase SHA-256")
    return value


def _manifest_provenance(value: object) -> dict[str, str]:
    provenance = _required_object(value, "fixture manifest package_artifact.provenance")
    if not provenance or any(
        not isinstance(key, str) or not key or not isinstance(entry, str) or not entry
        for key, entry in provenance.items()
    ):
        raise LifecycleGateError(
            "fixture manifest package_artifact.provenance must be a non-empty string map"
        )
    return dict(provenance)


def _read_fixture_manifest(fixture_root: Path) -> FixtureManifest:
    root = _resolve(fixture_root, "fixture root")
    if not root.is_dir():
        raise LifecycleGateError(f"fixture root is not a directory: {root}")
    manifest_path = root / MANIFEST_NAME
    raw = _require_file(manifest_path, "fixture manifest").read_bytes()
    try:
        document = json.loads(raw)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise LifecycleGateError(f"fixture manifest is not valid JSON: {manifest_path}: {error}") from error
    document = _required_object(document, "fixture manifest")
    if document.get("version") != TRACE_VERSION:
        raise LifecycleGateError(f"fixture manifest version must be {TRACE_VERSION}")
    if document.get("fixture") != FIXTURE_NAME:
        raise LifecycleGateError(f"fixture manifest fixture must be {FIXTURE_NAME!r}")
    if document.get("package") != FIXTURE_NAME:
        raise LifecycleGateError(f"fixture manifest package must be {FIXTURE_NAME!r}")
    if document.get("map") != MAP_PATH:
        raise LifecycleGateError(f"fixture manifest map must be {MAP_PATH!r}")
    map_source = _manifest_relative_path(document.get("map_source"), "map_source")
    if map_source.as_posix() != "Maps/Entry.unr":
        raise LifecycleGateError("fixture manifest map_source must be 'Maps/Entry.unr'")
    if document.get("game_class") != GAME_CLASS:
        raise LifecycleGateError(f"fixture manifest game_class must be {GAME_CLASS!r}")
    if document.get("launch_url") != f"{MAP_PATH}?game={GAME_CLASS}":
        raise LifecycleGateError(
            f"fixture manifest launch_url must be {MAP_PATH}?game={GAME_CLASS!r}"
        )
    if document.get("requested_ticks") != REQUESTED_TICKS:
        raise LifecycleGateError(f"fixture manifest requested_ticks must be {REQUESTED_TICKS}")
    fixed_dt = document.get("fixed_dt")
    if isinstance(fixed_dt, bool) or not isinstance(fixed_dt, (int, float)) or not math.isfinite(fixed_dt):
        raise LifecycleGateError("fixture manifest fixed_dt must be a finite number")
    if not math.isclose(float(fixed_dt), FIXED_DT, rel_tol=0.0, abs_tol=1.0e-12):
        raise LifecycleGateError(f"fixture manifest fixed_dt must be {FIXED_DT:.9g}")
    emitted_fixed_dt = document.get("emitted_fixed_dt")
    if (
        isinstance(emitted_fixed_dt, bool)
        or not isinstance(emitted_fixed_dt, (int, float))
        or not math.isfinite(emitted_fixed_dt)
        or not math.isclose(float(emitted_fixed_dt), EMITTED_FIXED_DT, rel_tol=0.0, abs_tol=1.0e-12)
    ):
        raise LifecycleGateError(
            f"fixture manifest emitted_fixed_dt must be {EMITTED_FIXED_DT:.10g}"
        )
    actor = _required_object(document.get("actor"), "fixture manifest actor")
    if actor != {"class": ACTOR_CLASS, "tag": ACTOR_TAG}:
        raise LifecycleGateError(
            "fixture manifest actor must identify "
            f"{ACTOR_CLASS}/{ACTOR_TAG}"
        )
    if document.get("probes") != [
        "LifecycleGateProbe:ProbeA",
        "LifecycleGateProbe:ProbeB",
    ]:
        raise LifecycleGateError(
            "fixture manifest probes must be LifecycleGateProbe:ProbeA and ProbeB"
        )
    if document.get("lifecycle_phases") != list(LIFECYCLE_PHASES):
        raise LifecycleGateError(
            "fixture manifest lifecycle_phases must declare "
            + " -> ".join(LIFECYCLE_PHASES)
        )
    artifact = _required_object(document.get("package_artifact"), "fixture manifest package_artifact")
    package_source = _manifest_relative_path(
        artifact.get("source"), "package_artifact.source",
    )
    if package_source.as_posix() != f"Package/{FIXTURE_NAME}.u":
        raise LifecycleGateError(
            f"fixture package artifact source must be 'Package/{FIXTURE_NAME}.u'"
        )
    package_output = _manifest_relative_path(
        artifact.get("stage"), "package_artifact.stage",
    )
    expected_output = f"System/{FIXTURE_NAME}.u"
    if package_output.as_posix() != expected_output:
        raise LifecycleGateError(f"fixture package artifact stage must be {expected_output!r}")
    package_sha256 = _manifest_sha256(
        artifact.get("sha256"), "package_artifact.sha256",
    )
    package_provenance = _manifest_provenance(artifact.get("provenance"))
    map_path = _manifest_relative_path(document["map"], "map")
    if map_path.as_posix() != MAP_PATH:
        raise LifecycleGateError(f"fixture map must be {MAP_PATH!r}")
    return FixtureManifest(
        root=root,
        package_source=package_source,
        package_output=package_output,
        package_sha256=package_sha256,
        package_provenance=package_provenance,
        map_path=map_path,
        map_source=map_source,
        manifest_sha256=hashlib.sha256(raw).hexdigest(),
    )


def _copy_file(source: Path, destination: Path) -> None:
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, destination)


def _copy_data_root(source: Path, destination: Path) -> None:
    """Copy rather than link: each engine may legitimately write state."""
    try:
        shutil.copytree(source, destination, symlinks=True)
    except OSError as error:
        raise LifecycleGateError(f"cannot stage data root {source} to {destination}: {error}") from error


def _require_package_artifact(manifest: FixtureManifest) -> Path:
    artifact = manifest.root / manifest.package_source.as_posix()
    if not artifact.is_file():
        raise LifecycleGateError(
            "fixture package artifact is missing: "
            f"{artifact}; commit the provenance-checked {FIXTURE_NAME}.u package"
        )
    actual_sha256 = _sha256(artifact)
    if actual_sha256 != manifest.package_sha256:
        raise LifecycleGateError(
            "fixture package artifact SHA-256 mismatch: "
            f"expected {manifest.package_sha256}, got {actual_sha256}: {artifact}"
        )
    return artifact


def _install_runtime_files(
    manifest: FixtureManifest,
    package_artifact: Path,
    source_data_root: Path,
    staged_root: Path,
) -> None:
    source_map = source_data_root / manifest.map_source.as_posix()
    if not source_map.is_file():
        raise LifecycleGateError(f"fixture base map is missing: {source_map}")
    _copy_file(package_artifact, staged_root / manifest.package_output.as_posix())
    _copy_file(source_map, staged_root / manifest.map_path.as_posix())


def stage_fixture(
    *,
    data_root: Path,
    fixture_root: Path,
    artifact_dir: Path,
) -> StagedFixture:
    """Give C++ and Rust byte-identical committed package/map inputs."""
    source_data = _require_data_root(data_root)
    manifest = _read_fixture_manifest(fixture_root)
    package_artifact = _require_package_artifact(manifest)
    source_map = source_data / manifest.map_source.as_posix()
    if not source_map.is_file():
        raise LifecycleGateError(f"fixture base map is missing: {source_map}")
    stage_root = _resolve(artifact_dir, "artifact directory", must_exist=False)
    if stage_root.exists():
        raise LifecycleGateError(f"artifact directory already exists: {stage_root}")
    stage_root.mkdir(parents=True, exist_ok=False)
    cpp_data_root = stage_root / "cpp-data"
    rust_data_root = stage_root / "rust-data"
    _copy_data_root(source_data, cpp_data_root)
    _copy_data_root(source_data, rust_data_root)
    _install_runtime_files(manifest, package_artifact, source_data, cpp_data_root)
    _install_runtime_files(manifest, package_artifact, source_data, rust_data_root)
    runtime_files = {
        manifest.package_output: package_artifact,
        manifest.map_path: source_map,
    }
    records: dict[str, dict[str, str]] = {}
    for relative, source in runtime_files.items():
        cpp_copy = cpp_data_root / relative.as_posix()
        rust_copy = rust_data_root / relative.as_posix()
        source_digest = _sha256(source)
        cpp_digest = _sha256(cpp_copy)
        rust_digest = _sha256(rust_copy)
        if cpp_digest != source_digest or rust_digest != source_digest:
            raise LifecycleGateError(f"staged fixture copy changed {relative.as_posix()}")
        records[relative.as_posix()] = {
            "sha256": source_digest,
            "source_path": str(source),
            "cpp_path": str(cpp_copy),
            "rust_path": str(rust_copy),
        }
    fixture_record = {
        "version": TRACE_VERSION,
        "fixture": FIXTURE_NAME,
        "manifest_sha256": manifest.manifest_sha256,
        "package_artifact": {
            "source": manifest.package_source.as_posix(),
            "stage": manifest.package_output.as_posix(),
            "sha256": manifest.package_sha256,
            "provenance": manifest.package_provenance,
        },
        "files": records,
    }
    _write_json_atomic(stage_root / "fixture.json", fixture_record)
    return StagedFixture(
        cpp_data_root=cpp_data_root,
        rust_data_root=rust_data_root,
        files=records,
        package_artifact=fixture_record["package_artifact"],
    )


def _stable_actor_key(value: object, path: str) -> str:
    if isinstance(value, str) and value:
        return value
    if isinstance(value, dict):
        actor_class = value.get("class")
        actor_tag = value.get("tag")
        stable_key = value.get("stable_key")
        if (
            set(value) == {"class", "tag", "stable_key"}
            and isinstance(actor_class, str)
            and actor_class
            and isinstance(actor_tag, str)
            and actor_tag == "LifecycleGateProbe"
            and stable_key in ("ProbeA", "ProbeB")
        ):
            return f"{actor_class}:{stable_key}"
        if (
            set(value) == {"class", "tag"}
            and isinstance(actor_class, str)
            and actor_class
            and isinstance(actor_tag, str)
            and actor_tag
        ):
            return f"{actor_class}:{actor_tag}"
    raise LifecycleGateError(
        f"{path} must be a stable actor key string or approved actor identity object"
    )


def _json_value(value: object, path: str) -> object:
    if value is None or isinstance(value, (str, bool)):
        return value
    if isinstance(value, int) and not isinstance(value, bool):
        return value
    if isinstance(value, float):
        if not math.isfinite(value):
            raise LifecycleGateError(f"{path} must not contain a non-finite number")
        return value
    if isinstance(value, list):
        return [_json_value(item, f"{path}[{index}]") for index, item in enumerate(value)]
    if isinstance(value, dict):
        normalized: dict[str, object] = {}
        for key in sorted(value):
            if not isinstance(key, str):
                raise LifecycleGateError(f"{path} has a non-string object key")
            normalized[key] = _json_value(value[key], f"{path}.{key}")
        return normalized
    raise LifecycleGateError(f"{path} is not a JSON value")


def _required_keys(value: object, expected: set[str], path: str) -> Mapping[str, object]:
    mapping = _required_object(value, path)
    actual = set(mapping)
    if actual != expected:
        missing = sorted(expected - actual)
        extra = sorted(actual - expected)
        detail = []
        if missing:
            detail.append(f"missing {missing}")
        if extra:
            detail.append(f"unexpected {extra}")
        raise LifecycleGateError(f"{path} keys are invalid ({'; '.join(detail)})")
    return mapping


def _integer(value: object, path: str, *, minimum: int | None = None) -> int:
    if isinstance(value, bool) or not isinstance(value, int):
        raise LifecycleGateError(f"{path} must be an integer")
    if minimum is not None and value < minimum:
        raise LifecycleGateError(f"{path} must be an integer >= {minimum}")
    return value


def _string(value: object, path: str) -> str:
    if not isinstance(value, str) or not value:
        raise LifecycleGateError(f"{path} must be a non-empty string")
    return value


def _normalise_trace(document: object, source: str) -> dict[str, object]:
    root = _required_keys(
        document,
        {"version", "fixture", "fixed_dt", "requested_ticks", "actor", "checkpoints", "random_calls"},
        source,
    )
    if root["version"] != TRACE_VERSION:
        raise LifecycleGateError(f"{source}.version must be {TRACE_VERSION}")
    if root["fixture"] != FIXTURE_NAME:
        raise LifecycleGateError(f"{source}.fixture must be {FIXTURE_NAME!r}")
    fixed_dt = root["fixed_dt"]
    if isinstance(fixed_dt, bool) or not isinstance(fixed_dt, (int, float)) or not math.isfinite(fixed_dt):
        raise LifecycleGateError(f"{source}.fixed_dt must be a finite number")
    if not math.isclose(float(fixed_dt), EMITTED_FIXED_DT, rel_tol=0.0, abs_tol=1.0e-12):
        raise LifecycleGateError(f"{source}.fixed_dt must be {EMITTED_FIXED_DT:.10g}")
    if root["requested_ticks"] != REQUESTED_TICKS:
        raise LifecycleGateError(f"{source}.requested_ticks must be {REQUESTED_TICKS}")
    actor = _required_keys(root["actor"], {"class", "tag"}, f"{source}.actor")
    if actor != {"class": ACTOR_CLASS, "tag": ACTOR_TAG}:
        raise LifecycleGateError(f"{source}.actor must identify {ACTOR_CLASS}/{ACTOR_TAG}")
    checkpoints = root["checkpoints"]
    if not isinstance(checkpoints, list) or len(checkpoints) != len(CHECKPOINT_IDS):
        raise LifecycleGateError(
            f"{source}.checkpoints must contain exactly {list(CHECKPOINT_IDS)}"
        )
    normalized_checkpoints: list[dict[str, object]] = []
    checkpoint_ids: set[str] = set()
    for index, raw_checkpoint in enumerate(checkpoints):
        path = f"{source}.checkpoints[{index}]"
        checkpoint = _required_keys(
            raw_checkpoint,
            {"id", "tick", "events", "frame", "iterator", "properties", "live", "delete_marked"},
            path,
        )
        checkpoint_id = _string(checkpoint["id"], f"{path}.id")
        if checkpoint_id in checkpoint_ids:
            raise LifecycleGateError(f"{path}.id duplicates {checkpoint_id!r}")
        checkpoint_ids.add(checkpoint_id)
        expected_id = CHECKPOINT_IDS[index]
        if checkpoint_id != expected_id:
            raise LifecycleGateError(
                f"{path}.id must be {expected_id!r}; checkpoint order is fixed"
            )
        tick = _integer(checkpoint["tick"], f"{path}.tick")
        events = checkpoint["events"]
        if not isinstance(events, list) or not all(isinstance(event, str) and event for event in events):
            raise LifecycleGateError(f"{path}.events must be an array of non-empty strings")
        expected_events = list(CHECKPOINT_EVENTS[checkpoint_id])
        if events != expected_events:
            raise LifecycleGateError(
                f"{path}.events must be exactly {expected_events}"
            )
        frame = _required_keys(checkpoint["frame"], {"state", "pc_diagnostic", "latent"}, f"{path}.frame")
        state = _string(frame["state"], f"{path}.frame.state")
        pc_raw = frame["pc_diagnostic"]
        pc_diagnostic = None
        if pc_raw is not None:
            _integer(pc_raw, f"{path}.frame.pc_diagnostic", minimum=0)
            # Native FFrame pointers and Rust's decoded bytecode cursor use
            # different physical offset units. Presence is comparable; state,
            # stage, latent wake, and ordered effects carry the semantic cursor.
            pc_diagnostic = "present"
        latent_raw = frame["latent"]
        if latent_raw is None:
            latent: dict[str, object] | None = None
        else:
            latent_value = _required_keys(latent_raw, {"kind", "wake_tick"}, f"{path}.frame.latent")
            latent = {
                "kind": _string(latent_value["kind"], f"{path}.frame.latent.kind"),
                "wake_tick": _integer(
                    latent_value["wake_tick"], f"{path}.frame.latent.wake_tick", minimum=0,
                ),
            }
        expected_latent = (
            {"kind": "Sleep", "wake_tick": 1}
            if checkpoint_id == "await_resume"
            else None
        )
        if latent != expected_latent:
            raise LifecycleGateError(
                f"{path}.frame.latent must be {expected_latent!r}"
            )
        iterator_raw = checkpoint["iterator"]
        if iterator_raw is None:
            iterator: dict[str, object] | None = None
        else:
            iterator_value = _required_keys(
                iterator_raw,
                {"base_class", "tag", "yields", "exhausted", "output"},
                f"{path}.iterator",
            )
            yields = iterator_value["yields"]
            if not isinstance(yields, list):
                raise LifecycleGateError(f"{path}.iterator.yields must be an array")
            if not isinstance(iterator_value["exhausted"], bool):
                raise LifecycleGateError(f"{path}.iterator.exhausted must be a boolean")
            if iterator_value["output"] is not None:
                raise LifecycleGateError(f"{path}.iterator.output must be null at exhaustion")
            iterator = {
                "base_class": _string(iterator_value["base_class"], f"{path}.iterator.base_class"),
                "tag": _string(iterator_value["tag"], f"{path}.iterator.tag"),
                "yields": [_stable_actor_key(item, f"{path}.iterator.yields[{yield_index}]") for yield_index, item in enumerate(yields)],
                "exhausted": iterator_value["exhausted"],
                "output": None,
            }
        properties_value = _required_keys(
            checkpoint["properties"], set(PROPERTY_FIELDS), f"{path}.properties",
        )
        properties = {
            name: _integer(properties_value[name], f"{path}.properties.{name}", minimum=0)
            for name in PROPERTY_FIELDS
        }
        expected_properties = dict(zip(PROPERTY_FIELDS, EXPECTED_COUNTERS[checkpoint_id]))
        if properties != expected_properties:
            raise LifecycleGateError(
                f"{path}.properties must be exactly {expected_properties}"
            )
        if not isinstance(checkpoint["live"], bool):
            raise LifecycleGateError(f"{path}.live must be a boolean")
        if not isinstance(checkpoint["delete_marked"], bool):
            raise LifecycleGateError(f"{path}.delete_marked must be a boolean")
        if tick != EXPECTED_TICKS[checkpoint_id]:
            raise LifecycleGateError(
                f"{path}.tick must be {EXPECTED_TICKS[checkpoint_id]}"
            )
        if state != EXPECTED_STATES[checkpoint_id]:
            raise LifecycleGateError(
                f"{path}.frame.state must be {EXPECTED_STATES[checkpoint_id]!r}"
            )
        expected_iterator = checkpoint_id == "scan"
        if expected_iterator:
            if iterator != {
                "base_class": "Actor",
                "tag": "LifecycleGateProbe",
                "yields": ["LifecycleGateProbe:ProbeA", "LifecycleGateProbe:ProbeB"],
                "exhausted": True,
                "output": None,
            }:
                raise LifecycleGateError(
                    f"{path}.iterator must yield ProbeA then ProbeB and exhaust to null"
                )
        elif iterator is not None:
            raise LifecycleGateError(f"{path}.iterator must be null")
        expected_live = checkpoint_id != "destroy"
        if checkpoint["live"] is not expected_live:
            raise LifecycleGateError(f"{path}.live must be {expected_live}")
        if checkpoint["delete_marked"] is (checkpoint_id != "destroy"):
            raise LifecycleGateError(
                f"{path}.delete_marked must be {not expected_live}"
            )
        normalized_checkpoints.append({
            "id": checkpoint_id,
            "tick": tick,
            "events": list(events),
            "frame": {"state": state, "pc_diagnostic": pc_diagnostic, "latent": latent},
            "iterator": iterator,
            "properties": properties,
            "live": checkpoint["live"],
            "delete_marked": checkpoint["delete_marked"],
        })
    if root["random_calls"] != []:
        raise LifecycleGateError(f"{source}.random_calls must be []")
    return {
        "version": TRACE_VERSION,
        "fixture": FIXTURE_NAME,
        "fixed_dt": EMITTED_FIXED_DT,
        "requested_ticks": REQUESTED_TICKS,
        "actor": {"class": ACTOR_CLASS, "tag": ACTOR_TAG},
        "checkpoints": normalized_checkpoints,
        "random_calls": [],
    }


def read_atomic_trace(path: Path) -> dict[str, object]:
    """Read the trace only after its process has exited and its atomic rename completed."""
    trace_path = _require_file(path, "lifecycle trace")
    temporary = trace_path.with_suffix(trace_path.suffix + ".tmp")
    if temporary.exists():
        raise LifecycleGateError(f"lifecycle trace atomic temporary remains: {temporary}")
    try:
        raw = trace_path.read_bytes()
        document = json.loads(raw)
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise LifecycleGateError(f"lifecycle trace is not valid atomic JSON: {trace_path}: {error}") from error
    return _normalise_trace(document, str(trace_path))


def _first_difference(reference: object, candidate: object, path: str = "$") -> dict[str, object] | None:
    if type(reference) is not type(candidate):
        return {"path": path, "cpp": reference, "rust": candidate}
    if isinstance(reference, dict):
        for key in sorted(set(reference) | set(candidate)):
            key_path = f"{path}.{key}"
            if key not in reference:
                return {"path": key_path, "cpp": {"missing": True}, "rust": candidate[key]}
            if key not in candidate:
                return {"path": key_path, "cpp": reference[key], "rust": {"missing": True}}
            difference = _first_difference(reference[key], candidate[key], key_path)
            if difference is not None:
                return difference
        return None
    if isinstance(reference, list):
        for index, (left, right) in enumerate(zip(reference, candidate)):
            difference = _first_difference(left, right, f"{path}[{index}]")
            if difference is not None:
                return difference
        if len(reference) != len(candidate):
            return {"path": f"{path}.length", "cpp": len(reference), "rust": len(candidate)}
        return None
    if reference != candidate:
        return {"path": path, "cpp": reference, "rust": candidate}
    return None


def compare_traces(cpp_trace: Mapping[str, object], rust_trace: Mapping[str, object]) -> dict[str, object]:
    """Compare the whole normalized lifecycle contract, not authored checkpoints."""
    difference = _first_difference(cpp_trace, rust_trace)
    if difference is None:
        return {"status": "match", "first_difference": None}
    return {"status": "mismatch", "first_difference": difference}


def _executable(path: Path, label: str) -> Path:
    root = _resolve(path, label)
    candidate = root / "Contents" / "MacOS" / "HarryPotter2" if root.is_dir() else root
    return _require_file(candidate, label, executable=True)


def _map_token() -> str:
    return f"..\\Maps\\LifecycleGate.unr?game={GAME_CLASS}"


def _run_side(
    *,
    name: str,
    executable: Path,
    data_root: Path,
    artifact_dir: Path,
    renderer: str,
    timeout_seconds: float,
    start_gate: threading.Barrier,
) -> dict[str, object]:
    side_dir = artifact_dir / name
    side_dir.mkdir(parents=True, exist_ok=True)
    trace_path = side_dir / "lifecycle-gate.json"
    command = game_test.build_launch_command(
        executable,
        data_root,
        _map_token(),
        renderer,
        REQUESTED_TICKS,
        fixed_dt=FIXED_DT,
    )
    process = game_test.run_command(
        command,
        repo_root=_repo_root(),
        timeout_seconds=timeout_seconds,
        log_path=side_dir / "stdout-stderr.log",
        extra_env={TRACE_ENVIRONMENT: str(trace_path)},
        start_gate=start_gate,
        home_parent=side_dir,
    )
    result: dict[str, object] = {
        "process": process,
        "trace_path": str(trace_path),
        "trace": None,
        "trace_error": None,
    }
    try:
        result["trace"] = read_atomic_trace(trace_path)
    except LifecycleGateError as error:
        result["trace_error"] = str(error)
    _write_json_atomic(side_dir / "result.json", result)
    return result


def run_lifecycle_gate(
    *,
    app: Path,
    engine_bin: Path,
    data_root: Path,
    fixture_root: Path,
    output: Path,
    renderer: str = "xopengl",
    timeout_seconds: float = DEFAULT_TIMEOUT_SECONDS,
) -> dict[str, object]:
    """Stage the committed fixture and compare the C++ oracle trace to Rust."""
    if renderer not in ("xopengl", "vulkan"):
        raise LifecycleGateError(f"unsupported renderer: {renderer}")
    if isinstance(timeout_seconds, bool) or not isinstance(timeout_seconds, (int, float)) or timeout_seconds <= 0:
        raise LifecycleGateError("timeout must be positive")
    cpp_executable = _executable(app, "C++ application")
    rust_executable = _executable(engine_bin, "Rust engine binary")
    output_path = _resolve(output, "output", must_exist=False)
    artifact_dir = output_path.parent / f"{output_path.stem}-artifacts"
    staged = stage_fixture(
        data_root=data_root,
        fixture_root=fixture_root,
        artifact_dir=artifact_dir,
    )
    barrier = threading.Barrier(2)
    with ThreadPoolExecutor(max_workers=2, thread_name_prefix="lifecycle-gate") as executor:
        cpp_future = executor.submit(
            _run_side,
            name="cpp",
            executable=cpp_executable,
            data_root=staged.cpp_data_root,
            artifact_dir=artifact_dir,
            renderer=renderer,
            timeout_seconds=float(timeout_seconds),
            start_gate=barrier,
        )
        rust_future = executor.submit(
            _run_side,
            name="rust",
            executable=rust_executable,
            data_root=staged.rust_data_root,
            artifact_dir=artifact_dir,
            renderer=renderer,
            timeout_seconds=float(timeout_seconds),
            start_gate=barrier,
        )
        cpp = cpp_future.result()
        rust = rust_future.result()
    cpp_trace = cpp["trace"]
    rust_trace = rust["trace"]
    if cpp_trace is None or rust_trace is None:
        comparison: dict[str, object] = {
            "status": "invalid_trace",
            "first_difference": None,
            "diagnostic": {"cpp": cpp["trace_error"], "rust": rust["trace_error"]},
        }
    else:
        comparison = compare_traces(cpp_trace, rust_trace)
    passed = (
        bool(cpp["process"]["passed"])
        and bool(rust["process"]["passed"])
        and comparison["status"] == "match"
    )
    report = {
        "emitted_fixed_dt": EMITTED_FIXED_DT,
        "version": TRACE_VERSION,
        "fixture": FIXTURE_NAME,
        "fixed_dt": FIXED_DT,
        "requested_ticks": REQUESTED_TICKS,
        "actor": {"class": ACTOR_CLASS, "tag": ACTOR_TAG},
        "staging": {
            "cpp_data_root": str(staged.cpp_data_root),
            "rust_data_root": str(staged.rust_data_root),
            "files": staged.files,
            "package_artifact": staged.package_artifact,
        },
        "runs": {"cpp": cpp, "rust": rust},
        "comparison": comparison,
        "passed": passed,
    }
    _write_json_atomic(output_path, report)
    return report


def _positive_float(value: str) -> float:
    try:
        parsed = float(value)
    except ValueError as error:
        raise argparse.ArgumentTypeError("must be a number") from error
    if not math.isfinite(parsed) or parsed <= 0:
        raise argparse.ArgumentTypeError("must be a positive finite number")
    return parsed


def _arguments(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--app", required=True, type=Path, help="C++ oracle app bundle or executable")
    parser.add_argument("--engine-bin", required=True, type=Path, help="Rust engine executable")
    parser.add_argument("--data-root", required=True, type=Path, help="unmodified base data root")
    parser.add_argument(
        "--fixture-root",
        type=Path,
        default=_repo_root() / "Tests" / "Fixtures" / "lifecycle-gate",
        help="committed native fixture package plus manifest",
    )
    parser.add_argument("--output", required=True, type=Path, help="atomic lifecycle gate report")
    parser.add_argument("--renderer", choices=("xopengl", "vulkan"), default="xopengl")
    parser.add_argument("--timeout", type=_positive_float, default=DEFAULT_TIMEOUT_SECONDS)
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    arguments = _arguments(argv)
    try:
        report = run_lifecycle_gate(
            app=arguments.app,
            engine_bin=arguments.engine_bin,
            data_root=arguments.data_root,
            fixture_root=arguments.fixture_root,
            output=arguments.output,
            renderer=arguments.renderer,
            timeout_seconds=arguments.timeout,
        )
    except LifecycleGateError as error:
        print(f"lifecycle gate: {error}", file=sys.stderr)
        return 2
    return 0 if report["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
