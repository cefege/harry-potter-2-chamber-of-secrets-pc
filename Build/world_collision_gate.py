#!/usr/bin/env python3
"""Run the bounded TriggerTest2 world-collision C++/Rust parity gate.

The gate copies the existing retail map into separate data roots, gives both
engines one declarative, coordinate-free scenario, and compares their complete
atomic v1 traces.  It deliberately has no fixture compiler or UCC dependency.
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
FIXTURE_NAME = "TriggerTest2WorldCollision"
MAP_RELATIVE = PurePosixPath("Maps/Studies/TriggerTest2.unr")
MAP_SHA256 = "fffa53fee540d756b9ff6bbc872c64d3bc81eca47f366f3d4071196fae2546fc"
FIXED_DT = 0.016666667
EMITTED_FIXED_DT = 0.0166666675
REQUESTED_TICKS = 1
TRACE_ENVIRONMENT = "HP2_WORLD_COLLISION_GATE_TRACE"
SCENARIO_ENVIRONMENT = "HP2_WORLD_COLLISION_GATE_SCENARIO"
TRACE_FILENAME = "world-collision-gate.json"
SCENARIO_FILENAME = "scenario.json"
DEFAULT_TIMEOUT_SECONDS = 60.0
TOUCH_FIXTURE_NAME = "TriggerTest2WorldTouch"
TOUCH_TRACE_ENVIRONMENT = "HP2_WORLD_TOUCH_GATE_TRACE"
TOUCH_SCENARIO_ENVIRONMENT = "HP2_WORLD_TOUCH_GATE_SCENARIO"
TOUCH_TRACE_FILENAME = "world-touch-gate.json"
TOUCH_SCENARIO_FILENAME = "touch-scenario.json"
BUMP_FIXTURE_NAME = "TriggerTest2WorldBump"
BUMP_TRACE_ENVIRONMENT = "HP2_WORLD_BUMP_GATE_TRACE"
BUMP_SCENARIO_ENVIRONMENT = "HP2_WORLD_BUMP_GATE_SCENARIO"
BUMP_TRACE_FILENAME = "world-bump-gate.json"
BUMP_SCENARIO_FILENAME = "bump-scenario.json"
BUMP_SWEEP_DELTA = (256, 0, 0)




ROLE_VALUES = {
    "probe": "transient:Engine.Actor",
    "blocker": "Engine.Mover",
    "touch_target": "transient:Engine.Actor",
    "zone_trigger": "Engine.Trigger",
    "spawn_anchor": "Engine.PlayerStart",
}
ROLE_NAMES = frozenset(ROLE_VALUES)
SNAPSHOT_ROLES = ("probe", "blocker", "touch_target")
STEP_SPECS = (
    ("resolve_safe_origin", "FindSpot", "destination"),
    ("touch_sweep", "MoveActor", "delta"),
    ("prepare_block", "FarMoveActor", "destination"),
    ("block_sweep", "MoveActor", "delta"),
    ("far_move", "FarMoveActor", "destination"),
    ("set_base_and_carry", "MoveActor", "delta"),
)

TOUCH_ROLE_VALUES = {
    "probe": "transient:Engine.Actor",
    "touch_target": "transient:Engine.Actor",
    "spawn_anchor": "Engine.PlayerStart",
}
TOUCH_ROLE_NAMES = frozenset(TOUCH_ROLE_VALUES)
TOUCH_SNAPSHOT_ROLES = ("probe", "touch_target")
TOUCH_STEP_SPECS = (
    ("resolve_safe_origin", "FindSpot", "destination"),
    ("touch_sweep", "MoveActor", "delta"),
)

BUMP_ROLE_VALUES = {
    "probe": "transient:Engine.Actor",
    "blocker": "transient:Engine.Actor",
    "spawn_anchor": "Engine.PlayerStart",
}
BUMP_ROLE_NAMES = frozenset(BUMP_ROLE_VALUES)
BUMP_SNAPSHOT_ROLES = ("probe", "blocker")
BUMP_STEP_SPECS = (
    ("resolve_safe_origin", "FindSpot", "destination"),
    ("block_sweep", "MoveActor", "delta"),
)



class WorldCollisionGateError(RuntimeError):
    """The source map, scenario, or an engine trace violates the gate contract."""


@dataclass(frozen=True)
class StagedMap:
    cpp_data_root: Path
    rust_data_root: Path
    scenario_path: Path
    map_record: dict[str, str]
    scenario_sha256: str


@dataclass(frozen=True)
class GateProtocol:
    fixture: str
    role_values: Mapping[str, str]
    snapshot_roles: tuple[str, ...]
    step_specs: tuple[tuple[str, str, str], ...]
    trace_environment: str
    scenario_environment: str
    trace_filename: str
    scenario_filename: str


FULL_PROTOCOL = GateProtocol(
    fixture=FIXTURE_NAME,
    role_values=ROLE_VALUES,
    snapshot_roles=SNAPSHOT_ROLES,
    step_specs=STEP_SPECS,
    trace_environment=TRACE_ENVIRONMENT,
    scenario_environment=SCENARIO_ENVIRONMENT,
    trace_filename=TRACE_FILENAME,
    scenario_filename=SCENARIO_FILENAME,
)
TOUCH_PROTOCOL = GateProtocol(
    fixture=TOUCH_FIXTURE_NAME,
    role_values=TOUCH_ROLE_VALUES,
    snapshot_roles=TOUCH_SNAPSHOT_ROLES,
    step_specs=TOUCH_STEP_SPECS,
    trace_environment=TOUCH_TRACE_ENVIRONMENT,
    scenario_environment=TOUCH_SCENARIO_ENVIRONMENT,
    trace_filename=TOUCH_TRACE_FILENAME,
    scenario_filename=TOUCH_SCENARIO_FILENAME,
)
BUMP_PROTOCOL = GateProtocol(
    fixture=BUMP_FIXTURE_NAME,
    role_values=BUMP_ROLE_VALUES,
    snapshot_roles=BUMP_SNAPSHOT_ROLES,
    step_specs=BUMP_STEP_SPECS,
    trace_environment=BUMP_TRACE_ENVIRONMENT,
    scenario_environment=BUMP_SCENARIO_ENVIRONMENT,
    trace_filename=BUMP_TRACE_FILENAME,
    scenario_filename=BUMP_SCENARIO_FILENAME,
)





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
        raise WorldCollisionGateError(f"cannot resolve {label} {path}: {error}") from error


def _require_file(path: Path, label: str, *, executable: bool = False) -> Path:
    resolved = _resolve(path, label)
    if not resolved.is_file():
        raise WorldCollisionGateError(f"{label} is not a regular file: {resolved}")
    if executable and not os.access(resolved, os.X_OK):
        raise WorldCollisionGateError(f"{label} is not executable: {resolved}")
    return resolved


def _require_data_root(path: Path) -> Path:
    root = _resolve(path, "data root")
    if not root.is_dir():
        raise WorldCollisionGateError(f"data root is not a directory: {root}")
    if not (root / "System" / "Default.ini").is_file():
        raise WorldCollisionGateError(f"data root lacks System/Default.ini: {root}")
    return root


def _require_protocol(protocol: GateProtocol) -> GateProtocol:
    if protocol not in (FULL_PROTOCOL, TOUCH_PROTOCOL, BUMP_PROTOCOL):
        raise WorldCollisionGateError(f"unsupported gate protocol: {protocol.fixture}")
    return protocol


def scenario_document(protocol: GateProtocol = FULL_PROTOCOL) -> dict[str, object]:
    """Return the single coordinate-free scenario supplied unchanged to both engines."""
    protocol = _require_protocol(protocol)
    if protocol is BUMP_PROTOCOL:
        collision = {
            "radius": 16,
            "height": 24,
            "collide_actors": True,
            "collide_world": True,
            "block_actors": True,
            "block_players": True,
        }
        return {
            "version": TRACE_VERSION,
            "fixture": protocol.fixture,
            "map": {"relative": MAP_RELATIVE.as_posix(), "sha256": MAP_SHA256},
            "fixed_dt": FIXED_DT,
            "requested_ticks": REQUESTED_TICKS,
            "selectors": {
                "spawn_anchor": {"class": "Engine.PlayerStart", "cardinality": 1},
            },
            "transients": {
                "probe": {"class": "Engine.Actor", "collision": collision},
                "blocker": {"class": "Engine.Actor", "collision": collision.copy()},
            },
            "operations": [
                {
                    "id": "resolve_safe_origin",
                    "op": "FindSpot",
                    "actor": "probe",
                    "origin": {"relative_to": "spawn_anchor", "offset": [0, 0, 26]},
                },
                {
                    "id": "block_sweep",
                    "op": "MoveActor",
                    "actor": "probe",
                    "placement": {
                        "actor": "blocker",
                        "relative_to": "safe_origin",
                        "offset": [96, 0, 0],
                    },
                    "delta": list(BUMP_SWEEP_DELTA),
                },
            ],
            "random_calls": [],
        }
    selectors: dict[str, object] = {
        "spawn_anchor": {"class": "Engine.PlayerStart", "cardinality": 1},
    }
    if protocol is FULL_PROTOCOL:
        selectors = {
            "blocker": {"class": "Engine.Mover", "cardinality": 1},
            "zone_trigger": {"class": "Engine.Trigger", "cardinality": 1},
            **selectors,
        }
    operations: list[dict[str, object]] = [
        {
            "id": "resolve_safe_origin",
            "op": "FindSpot",
            "actor": "probe",
            "origin": {"relative_to": "spawn_anchor", "offset": [0, 0, 26]},
        },
        {
            "id": "touch_sweep",
            "op": "MoveActor",
            "actor": "probe",
            "placement": {
                "actor": "touch_target",
                "relative_to": "probe",
                "offset": [48, 0, 0],
            },
            "delta": [48, 0, 0],
        },
    ]
    if protocol is FULL_PROTOCOL:
        operations.extend((
            {
                "id": "prepare_block",
                "op": "FarMoveActor",
                "actor": "probe",
                "placement": {
                    "relative_to": "blocker",
                    "approach": "negative_x",
                    "clearance": 64,
                    "find_spot": True,
                },
            },
            {"id": "block_sweep", "op": "MoveActor", "actor": "probe", "delta": [256, 0, 0]},
            {
                "id": "far_move",
                "op": "FarMoveActor",
                "actor": "probe",
                "destination": "safe_origin",
                "find_spot": True,
            },
            {
                "id": "set_base_and_carry",
                "op": "MoveActor",
                "actor": "blocker",
                "prelude": {"op": "SetBase", "actor": "probe", "base": "blocker"},
                "delta": [0, 0, 16],
            },
        ))
    return {
        "version": TRACE_VERSION,
        "fixture": protocol.fixture,
        "map": {"relative": MAP_RELATIVE.as_posix(), "sha256": MAP_SHA256},
        "fixed_dt": FIXED_DT,
        "requested_ticks": REQUESTED_TICKS,
        "selectors": selectors,
        "transients": {
            "probe": {
                "class": "Engine.Actor",
                "collision": {
                    "radius": 16,
                    "height": 24,
                    "collide_actors": True,
                    "collide_world": True,
                    "block_actors": True,
                    "block_players": True,
                },
            },
            "touch_target": {
                "class": "Engine.Actor",
                "collision": {
                    "radius": 16,
                    "height": 24,
                    "collide_actors": True,
                    "collide_world": False,
                    "block_actors": False,
                    "block_players": False,
                },
            },
        },
        "operations": operations,
        "random_calls": [],
    }


def _copy_data_root(source: Path, destination: Path) -> None:
    try:
        shutil.copytree(source, destination, symlinks=True)
    except OSError as error:
        raise WorldCollisionGateError(f"cannot stage data root {source} to {destination}: {error}") from error


def stage_map(
    *,
    data_root: Path,
    artifact_dir: Path,
    protocol: GateProtocol = FULL_PROTOCOL,
) -> StagedMap:
    """Copy the unmodified retail map into independent engine data roots."""
    protocol = _require_protocol(protocol)
    source_data = _require_data_root(data_root)
    source_map = _require_file(source_data / MAP_RELATIVE.as_posix(), "TriggerTest2 map")
    source_sha256 = _sha256(source_map)
    if source_sha256 != MAP_SHA256:
        raise WorldCollisionGateError(
            "TriggerTest2 map SHA-256 mismatch: "
            f"expected {MAP_SHA256}, got {source_sha256}: {source_map}"
        )
    stage_root = _resolve(artifact_dir, "artifact directory", must_exist=False)
    if stage_root.exists():
        raise WorldCollisionGateError(f"artifact directory already exists: {stage_root}")
    stage_root.mkdir(parents=True, exist_ok=False)
    cpp_data_root = stage_root / "cpp-data"
    rust_data_root = stage_root / "rust-data"
    _copy_data_root(source_data, cpp_data_root)
    _copy_data_root(source_data, rust_data_root)
    cpp_map = cpp_data_root / MAP_RELATIVE.as_posix()
    rust_map = rust_data_root / MAP_RELATIVE.as_posix()
    cpp_sha256 = _sha256(_require_file(cpp_map, "staged C++ TriggerTest2 map"))
    rust_sha256 = _sha256(_require_file(rust_map, "staged Rust TriggerTest2 map"))
    if cpp_sha256 != source_sha256 or rust_sha256 != source_sha256:
        raise WorldCollisionGateError("staged TriggerTest2 bytes differ from the unmodified source map")
    scenario_path = stage_root / protocol.scenario_filename
    scenario = scenario_document(protocol)
    _write_json_atomic(scenario_path, scenario)
    return StagedMap(
        cpp_data_root=cpp_data_root,
        rust_data_root=rust_data_root,
        scenario_path=scenario_path,
        map_record={
            "relative": MAP_RELATIVE.as_posix(),
            "sha256": source_sha256,
            "source_path": str(source_map),
            "cpp_path": str(cpp_map),
            "rust_path": str(rust_map),
        },
        scenario_sha256=_sha256(scenario_path),
    )


def _required_object(value: object, path: str) -> Mapping[str, object]:
    if not isinstance(value, dict):
        raise WorldCollisionGateError(f"{path} must be a JSON object")
    return value


def _required_keys(value: object, expected: set[str], path: str) -> Mapping[str, object]:
    mapping = _required_object(value, path)
    actual = set(mapping)
    if actual != expected:
        detail: list[str] = []
        missing = sorted(expected - actual)
        extra = sorted(actual - expected)
        if missing:
            detail.append(f"missing {missing}")
        if extra:
            detail.append(f"unexpected {extra}")
        raise WorldCollisionGateError(f"{path} keys are invalid ({'; '.join(detail)})")
    return mapping


def _string(value: object, path: str) -> str:
    if not isinstance(value, str) or not value:
        raise WorldCollisionGateError(f"{path} must be a non-empty string")
    return value


def _integer(value: object, path: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int):
        raise WorldCollisionGateError(f"{path} must be an integer")
    return value


def _quantised_number(value: object, path: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(value):
        raise WorldCollisionGateError(f"{path} must be a finite number")
    quantised = round(float(value), 4)
    return 0.0 if quantised == 0.0 else quantised


def _vector(value: object, path: str) -> list[float]:
    if not isinstance(value, list) or len(value) != 3:
        raise WorldCollisionGateError(f"{path} must be a three-number vector")
    return [_quantised_number(component, f"{path}[{index}]") for index, component in enumerate(value)]


def _role(
    value: object,
    path: str,
    *,
    protocol: GateProtocol,
    nullable: bool = True,
) -> str | None:
    if value is None and nullable:
        return None
    role = _string(value, path)
    if role not in protocol.role_values:
        raise WorldCollisionGateError(f"{path} must name a declared role")
    return role


def _normalise_zone(value: object, path: str) -> dict[str, str] | None:
    if value is None:
        return None
    zone = _required_keys(value, {"class", "tag"}, path)
    return {
        "class": _string(zone["class"], f"{path}.class"),
        "tag": _string(zone["tag"], f"{path}.tag"),
    }


def _normalise_touching_roles(
    value: object,
    path: str,
    actor_role: str,
    *,
    protocol: GateProtocol,
) -> list[str]:
    if not isinstance(value, list) or not all(isinstance(role, str) for role in value):
        raise WorldCollisionGateError(f"{path} must be an array of snapshot role names")
    if any(role not in protocol.snapshot_roles or role == actor_role for role in value):
        raise WorldCollisionGateError(f"{path} must name other snapshot roles")
    if value != sorted(set(value)):
        raise WorldCollisionGateError(f"{path} must be unique and lexicographically sorted")
    return list(value)


def _normalise_actor(
    value: object,
    path: str,
    actor_role: str,
    *,
    protocol: GateProtocol,
) -> dict[str, object]:
    actor = _required_keys(
        value,
        {"location", "rotation", "base_role", "zone", "b_just_teleported", "delete_marked", "touching_roles"},
        path,
    )
    rotation = actor["rotation"]
    if not isinstance(rotation, list) or len(rotation) != 3:
        raise WorldCollisionGateError(f"{path}.rotation must be a three-integer rotator")
    if not isinstance(actor["b_just_teleported"], bool):
        raise WorldCollisionGateError(f"{path}.b_just_teleported must be a boolean")
    if not isinstance(actor["delete_marked"], bool):
        raise WorldCollisionGateError(f"{path}.delete_marked must be a boolean")
    base_role = _role(actor["base_role"], f"{path}.base_role", protocol=protocol)
    allowed_base_roles = (None, "blocker") if protocol is FULL_PROTOCOL else (None,)
    if base_role not in allowed_base_roles:
        raise WorldCollisionGateError(f"{path}.base_role is not admitted by this gate")
    return {
        "location": _vector(actor["location"], f"{path}.location"),
        "rotation": [_integer(component, f"{path}.rotation[{index}]") for index, component in enumerate(rotation)],
        "base_role": base_role,
        "zone": _normalise_zone(actor["zone"], f"{path}.zone"),
        "b_just_teleported": actor["b_just_teleported"],
        "delete_marked": actor["delete_marked"],
        "touching_roles": _normalise_touching_roles(
            actor["touching_roles"], f"{path}.touching_roles", actor_role, protocol=protocol,
        ),
    }


def _callback_event(kind: str, receiver: str, other: str) -> str:
    return f"{kind}:{receiver}:{other}"


def _same_displacement(left: list[float], right: list[float]) -> bool:
    return all(math.isclose(left[index], right[index], rel_tol=0.0, abs_tol=1.0e-4) for index in range(3))


def _validate_step_contract(
    step_id: str,
    result: Mapping[str, object],
    actors: Mapping[str, Mapping[str, object]],
    events: list[str],
    previous: Mapping[str, Mapping[str, object]] | None,
    path: str,
    *,
    protocol: GateProtocol,
) -> None:
    for role in protocol.snapshot_roles:
        touching_roles = actors[role]["touching_roles"]
        assert isinstance(touching_roles, list)
        for other in touching_roles:
            assert isinstance(other, str)
            other_touching = actors[other]["touching_roles"]
            assert isinstance(other_touching, list)
            if role not in other_touching:
                raise WorldCollisionGateError(f"{path}.actors touching_roles must be reciprocal")
    if protocol is BUMP_PROTOCOL:
        if step_id == "resolve_safe_origin":
            if events:
                raise WorldCollisionGateError(f"{path}.events must be [] before the Bump sweep")
            return
        assert step_id == "block_sweep"
        if not result["blocked"] or result["hit_role"] != "blocker":
            raise WorldCollisionGateError(f"{path} must be blocked by blocker")
        hit_time = result["hit_time"]
        assert isinstance(hit_time, float)
        if not 0.0 < hit_time < 1.0:
            raise WorldCollisionGateError(f"{path}.result.hit_time must be fractional for the Bump sweep")
        if actors["probe"]["b_just_teleported"]:
            raise WorldCollisionGateError(f"{path}.actors.probe.b_just_teleported must be false before the Bump sweep")
        if actors["probe"]["touching_roles"] or actors["blocker"]["touching_roles"]:
            raise WorldCollisionGateError(f"{path} must not retain a probe/blocker touching relation")
        expected_events = [
            _callback_event("Bump", "blocker", "probe"),
            _callback_event("Bump", "probe", "blocker"),
        ]
        if events != expected_events:
            raise WorldCollisionGateError(f"{path}.events must be the exact ordered Bump callbacks")
        return

    if step_id == "touch_sweep":
        if result["blocked"] or result["hit_role"] is not None or result["hit_time"] != 1.0:
            raise WorldCollisionGateError(f"{path} must be an unblocked touch sweep with a null hit at time 1")
        if actors["probe"]["touching_roles"] != ["touch_target"] or actors["touch_target"]["touching_roles"] != ["probe"]:
            raise WorldCollisionGateError(f"{path} must retain reciprocal probe/touch_target contact")
        if (
            _callback_event("Touch", "probe", "touch_target") not in events
            or _callback_event("Touch", "touch_target", "probe") not in events
        ):
            raise WorldCollisionGateError(f"{path}.events must contain reciprocal Touch callbacks")
    elif step_id == "prepare_block":
        if (
            "touch_target" in actors["probe"]["touching_roles"]
            or "probe" in actors["touch_target"]["touching_roles"]
        ):
            raise WorldCollisionGateError(f"{path} must clear probe/touch_target contact before the block sweep")
    elif step_id == "block_sweep":
        if not result["blocked"] or result["hit_role"] != "blocker":
            raise WorldCollisionGateError(f"{path} must be blocked by blocker")
        if (
            "blocker" in actors["probe"]["touching_roles"]
            or "probe" in actors["blocker"]["touching_roles"]
        ):
            raise WorldCollisionGateError(f"{path} must not retain probe/blocker contact")
        if not (
            _callback_event("Bump", "probe", "blocker") in events
            or _callback_event("Bump", "blocker", "probe") in events
        ):
            raise WorldCollisionGateError(f"{path}.events must contain a probe/blocker Bump callback")
    elif step_id == "far_move":
        callback_kinds = {"Touch", "UnTouch", "Bump"}
        if any(event.split(":", 1)[0] in callback_kinds for event in events):
            raise WorldCollisionGateError(f"{path}.events must not contain Touch, UnTouch, or Bump callbacks")
    elif step_id == "set_base_and_carry":
        if actors["probe"]["base_role"] != "blocker":
            raise WorldCollisionGateError(f"{path}.actors.probe.base_role must be 'blocker'")
        if previous is None:
            raise WorldCollisionGateError(f"{path} has no pre-carry snapshot")
        probe_before = previous["probe"]["location"]
        blocker_before = previous["blocker"]["location"]
        probe_after = actors["probe"]["location"]
        blocker_after = actors["blocker"]["location"]
        assert isinstance(probe_before, list) and isinstance(blocker_before, list)
        assert isinstance(probe_after, list) and isinstance(blocker_after, list)
        probe_displacement = [probe_after[index] - probe_before[index] for index in range(3)]
        blocker_displacement = [blocker_after[index] - blocker_before[index] for index in range(3)]
        if not _same_displacement(probe_displacement, blocker_displacement):
            raise WorldCollisionGateError(f"{path} must carry probe by blocker displacement")


def _raise_scenario_error(document: Mapping[str, object], source: str) -> None:
    if "scenario_error" not in document:
        return
    scenario_error = _required_keys(
        document["scenario_error"], {"code", "selectors"}, f"{source}.scenario_error",
    )
    code = _string(scenario_error["code"], f"{source}.scenario_error.code")
    selectors = scenario_error["selectors"]
    if not isinstance(selectors, list):
        raise WorldCollisionGateError(f"{source}.scenario_error.selectors must be an array")
    observations: list[str] = []
    for index, selector_value in enumerate(selectors):
        path = f"{source}.scenario_error.selectors[{index}]"
        selector = _required_keys(selector_value, {"id", "class", "path", "match_count"}, path)
        selector_id = _string(selector["id"], f"{path}.id")
        _string(selector["class"], f"{path}.class")
        raw_path = selector["path"]
        if raw_path is not None:
            _string(raw_path, f"{path}.path")
        match_count = _integer(selector["match_count"], f"{path}.match_count")
        if match_count < 0:
            raise WorldCollisionGateError(f"{path}.match_count must be non-negative")
        observations.append(f"{selector_id}={match_count}")
    detail = f" ({', '.join(observations)})" if observations else ""
    raise WorldCollisionGateError(f"{source}.scenario_error {code}{detail}")


def _normalise_trace(
    document: object,
    source: str,
    protocol: GateProtocol = FULL_PROTOCOL,
) -> dict[str, object]:
    protocol = _require_protocol(protocol)
    if isinstance(document, dict):
        _raise_scenario_error(document, source)
    root = _required_keys(
        document,
        {"version", "fixture", "map", "fixed_dt", "requested_ticks", "roles", "steps", "random_calls"},
        source,
    )
    if root["version"] != TRACE_VERSION:
        raise WorldCollisionGateError(f"{source}.version must be {TRACE_VERSION}")
    if root["fixture"] != protocol.fixture:
        raise WorldCollisionGateError(f"{source}.fixture must be {protocol.fixture!r}")
    map_value = _required_keys(root["map"], {"relative", "sha256"}, f"{source}.map")
    if map_value != {"relative": MAP_RELATIVE.as_posix(), "sha256": MAP_SHA256}:
        raise WorldCollisionGateError(f"{source}.map must identify the committed TriggerTest2 bytes")
    fixed_dt = root["fixed_dt"]
    if isinstance(fixed_dt, bool) or not isinstance(fixed_dt, (int, float)) or not math.isfinite(fixed_dt):
        raise WorldCollisionGateError(f"{source}.fixed_dt must be a finite number")
    if not math.isclose(float(fixed_dt), EMITTED_FIXED_DT, rel_tol=0.0, abs_tol=1.0e-9):
        raise WorldCollisionGateError(f"{source}.fixed_dt must be {EMITTED_FIXED_DT:.10g}")
    if root["requested_ticks"] != REQUESTED_TICKS:
        raise WorldCollisionGateError(f"{source}.requested_ticks must be {REQUESTED_TICKS}")
    roles = _required_keys(root["roles"], set(protocol.role_values), f"{source}.roles")
    if dict(roles) != dict(protocol.role_values):
        raise WorldCollisionGateError(f"{source}.roles must be the canonical role map")
    if root["random_calls"] != []:
        raise WorldCollisionGateError(f"{source}.random_calls must be []")
    raw_steps = root["steps"]
    if not isinstance(raw_steps, list) or len(raw_steps) != len(protocol.step_specs):
        raise WorldCollisionGateError(f"{source}.steps must contain the canonical bounded sequence")
    normalised_steps: list[dict[str, object]] = []
    for index, (step_id, operation, parameter) in enumerate(protocol.step_specs):
        path = f"{source}.steps[{index}]"
        step = _required_keys(raw_steps[index], {"id", "request", "result", "actors", "events"}, path)
        if step["id"] != step_id:
            raise WorldCollisionGateError(f"{path}.id must be {step_id!r}")
        request = _required_keys(step["request"], {"operation", parameter}, f"{path}.request")
        if request["operation"] != operation:
            raise WorldCollisionGateError(f"{path}.request.operation must be {operation!r}")
        request_value = _vector(request[parameter], f"{path}.request.{parameter}")
        if protocol is BUMP_PROTOCOL and step_id == "block_sweep" and request_value != list(BUMP_SWEEP_DELTA):
            raise WorldCollisionGateError(
                f"{path}.request.delta must be the canonical Bump sweep {list(BUMP_SWEEP_DELTA)}"
            )
        result = _required_keys(step["result"], {"moved", "hit_time", "blocked", "hit_role"}, f"{path}.result")
        if not isinstance(result["moved"], bool):
            raise WorldCollisionGateError(f"{path}.result.moved must be a boolean")
        if not isinstance(result["blocked"], bool):
            raise WorldCollisionGateError(f"{path}.result.blocked must be a boolean")
        actors = _required_keys(step["actors"], set(protocol.snapshot_roles), f"{path}.actors")
        events = step["events"]
        if not isinstance(events, list) or not all(isinstance(event, str) and event for event in events):
            raise WorldCollisionGateError(f"{path}.events must be an array of non-empty strings")
        normalised_result: dict[str, object] = {
            "moved": result["moved"],
            "hit_time": _quantised_number(result["hit_time"], f"{path}.result.hit_time"),
            "blocked": result["blocked"],
            "hit_role": _role(result["hit_role"], f"{path}.result.hit_role", protocol=protocol),
        }
        normalised_actors = {
            role: _normalise_actor(actors[role], f"{path}.actors.{role}", role, protocol=protocol)
            for role in protocol.snapshot_roles
        }
        normalised_events = list(events)
        previous_actors = normalised_steps[-1]["actors"] if normalised_steps else None
        assert previous_actors is None or isinstance(previous_actors, dict)
        _validate_step_contract(
            step_id,
            normalised_result,
            normalised_actors,
            normalised_events,
            previous_actors,
            path,
            protocol=protocol,
        )
        normalised_steps.append({
            "id": step_id,
            "request": {"operation": operation, parameter: request_value},
            "result": normalised_result,
            "actors": normalised_actors,
            "events": normalised_events,
        })
    return {
        "version": TRACE_VERSION,
        "fixture": protocol.fixture,
        "map": {"relative": MAP_RELATIVE.as_posix(), "sha256": MAP_SHA256},
        "fixed_dt": EMITTED_FIXED_DT,
        "requested_ticks": REQUESTED_TICKS,
        "roles": dict(protocol.role_values),
        "steps": normalised_steps,
        "random_calls": [],
    }


def read_atomic_trace(path: Path, protocol: GateProtocol = FULL_PROTOCOL) -> dict[str, object]:
    """Read a complete trace only after its writer's atomic rename."""
    protocol = _require_protocol(protocol)
    trace_path = _require_file(path, f"{protocol.fixture} trace")
    temporary = trace_path.with_suffix(trace_path.suffix + ".tmp")
    if temporary.exists():
        raise WorldCollisionGateError(f"{protocol.fixture} trace atomic temporary remains: {temporary}")
    try:
        document = json.loads(trace_path.read_bytes())
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise WorldCollisionGateError(
            f"{protocol.fixture} trace is not valid atomic JSON: {trace_path}: {error}"
        ) from error
    return _normalise_trace(document, str(trace_path), protocol)


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
    """Compare every normalized observation, including requests and callbacks."""
    difference = _first_difference(cpp_trace, rust_trace)
    if difference is None:
        return {"status": "match", "first_difference": None}
    return {"status": "mismatch", "first_difference": difference}


def _executable(path: Path, label: str) -> Path:
    root = _resolve(path, label)
    candidate = root / "Contents" / "MacOS" / "HarryPotter2" if root.is_dir() else root
    return _require_file(candidate, label, executable=True)


def _map_token() -> str:
    return "..\\Maps\\Studies\\TriggerTest2.unr"


def _run_side(
    *,
    name: str,
    executable: Path,
    data_root: Path,
    scenario_path: Path,
    artifact_dir: Path,
    renderer: str,
    timeout_seconds: float,
    start_gate: threading.Barrier,
    protocol: GateProtocol = FULL_PROTOCOL,
) -> dict[str, object]:
    protocol = _require_protocol(protocol)
    side_dir = artifact_dir / name
    side_dir.mkdir(parents=True, exist_ok=True)
    trace_path = side_dir / protocol.trace_filename
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
        extra_env={
            protocol.scenario_environment: str(scenario_path),
            protocol.trace_environment: str(trace_path),
        },
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
        result["trace"] = read_atomic_trace(trace_path, protocol)
    except WorldCollisionGateError as error:
        result["trace_error"] = str(error)
    _write_json_atomic(side_dir / "result.json", result)
    return result


def _run_gate(
    *,
    app: Path,
    engine_bin: Path,
    data_root: Path,
    output: Path,
    renderer: str,
    timeout_seconds: float,
    protocol: GateProtocol,
) -> dict[str, object]:
    protocol = _require_protocol(protocol)
    if renderer not in ("xopengl", "vulkan"):
        raise WorldCollisionGateError(f"unsupported renderer: {renderer}")
    if isinstance(timeout_seconds, bool) or not isinstance(timeout_seconds, (int, float)) or timeout_seconds <= 0:
        raise WorldCollisionGateError("timeout must be positive")
    cpp_executable = _executable(app, "C++ application")
    rust_executable = _executable(engine_bin, "Rust engine binary")
    output_path = _resolve(output, "output", must_exist=False)
    artifact_dir = output_path.parent / f"{output_path.stem}-artifacts"
    staged = stage_map(data_root=data_root, artifact_dir=artifact_dir, protocol=protocol)
    barrier = threading.Barrier(2)
    with ThreadPoolExecutor(max_workers=2, thread_name_prefix="world-collision-gate") as executor:
        cpp_future = executor.submit(
            _run_side,
            name="cpp",
            executable=cpp_executable,
            data_root=staged.cpp_data_root,
            scenario_path=staged.scenario_path,
            artifact_dir=artifact_dir,
            renderer=renderer,
            timeout_seconds=float(timeout_seconds),
            start_gate=barrier,
            protocol=protocol,
        )
        rust_future = executor.submit(
            _run_side,
            name="rust",
            executable=rust_executable,
            data_root=staged.rust_data_root,
            scenario_path=staged.scenario_path,
            artifact_dir=artifact_dir,
            renderer=renderer,
            timeout_seconds=float(timeout_seconds),
            start_gate=barrier,
            protocol=protocol,
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
    passed = bool(cpp["process"]["passed"]) and bool(rust["process"]["passed"]) and comparison["status"] == "match"
    report = {
        "version": TRACE_VERSION,
        "fixture": protocol.fixture,
        "map": {"relative": MAP_RELATIVE.as_posix(), "sha256": MAP_SHA256},
        "fixed_dt": FIXED_DT,
        "emitted_fixed_dt": EMITTED_FIXED_DT,
        "requested_ticks": REQUESTED_TICKS,
        "roles": dict(protocol.role_values),
        "scenario": {"path": str(staged.scenario_path), "sha256": staged.scenario_sha256},
        "staging": {
            "cpp_data_root": str(staged.cpp_data_root),
            "rust_data_root": str(staged.rust_data_root),
            "map": staged.map_record,
        },
        "runs": {"cpp": cpp, "rust": rust},
        "comparison": comparison,
        "passed": passed,
    }
    _write_json_atomic(output_path, report)
    return report


def run_world_collision_gate(
    *,
    app: Path,
    engine_bin: Path,
    data_root: Path,
    output: Path,
    renderer: str = "xopengl",
    timeout_seconds: float = DEFAULT_TIMEOUT_SECONDS,
) -> dict[str, object]:
    """Stage TriggerTest2 and compare the full C++/Rust collision trace."""
    return _run_gate(
        app=app,
        engine_bin=engine_bin,
        data_root=data_root,
        output=output,
        renderer=renderer,
        timeout_seconds=timeout_seconds,
        protocol=FULL_PROTOCOL,
    )


def run_world_touch_gate(
    *,
    app: Path,
    engine_bin: Path,
    data_root: Path,
    output: Path,
    renderer: str = "xopengl",
    timeout_seconds: float = DEFAULT_TIMEOUT_SECONDS,
) -> dict[str, object]:
    """Stage TriggerTest2 and compare the dedicated two-step touch trace."""
    return _run_gate(
        app=app,
        engine_bin=engine_bin,
        data_root=data_root,
        output=output,
        renderer=renderer,
        timeout_seconds=timeout_seconds,
        protocol=TOUCH_PROTOCOL,
    )

def run_world_bump_gate(
    *,
    app: Path,
    engine_bin: Path,
    data_root: Path,
    output: Path,
    renderer: str = "xopengl",
    timeout_seconds: float = DEFAULT_TIMEOUT_SECONDS,
) -> dict[str, object]:
    """Stage TriggerTest2 and compare the dedicated two-step Bump trace."""
    return _run_gate(
        app=app,
        engine_bin=engine_bin,
        data_root=data_root,
        output=output,
        renderer=renderer,
        timeout_seconds=timeout_seconds,
        protocol=BUMP_PROTOCOL,
    )





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
    parser.add_argument("--data-root", required=True, type=Path, help="unmodified HP2 data root")
    parser.add_argument("--output", required=True, type=Path, help="atomic world-collision gate report")
    parser.add_argument("--renderer", choices=("xopengl", "vulkan"), default="xopengl")
    parser.add_argument("--timeout", type=_positive_float, default=DEFAULT_TIMEOUT_SECONDS)
    route = parser.add_mutually_exclusive_group()
    route.add_argument("--touch-only", action="store_true", help="run only the dedicated two-step touch gate")
    route.add_argument("--bump-only", action="store_true", help="run only the dedicated two-step Bump gate")
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    arguments = _arguments(argv)
    try:
        if arguments.bump_only:
            run_gate = run_world_bump_gate
        elif arguments.touch_only:
            run_gate = run_world_touch_gate
        else:
            run_gate = run_world_collision_gate
        report = run_gate(
            app=arguments.app,
            engine_bin=arguments.engine_bin,
            data_root=arguments.data_root,
            output=arguments.output,
            renderer=arguments.renderer,
            timeout_seconds=arguments.timeout,
        )
    except WorldCollisionGateError as error:
        print(f"world collision gate: {error}", file=sys.stderr)
        return 2
    return 0 if report["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
