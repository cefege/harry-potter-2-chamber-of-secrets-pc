#!/usr/bin/env python3
"""Run the bounded TriggerTest2 static-BSP movement C++/Rust parity gate.

This gate stages the retail map into two isolated data roots, gives both engines the
same coordinate-free scenario, launches them together, and compares their complete
v1 reports.  It intentionally exercises only static world placement and movement:
there are no dynamic blockers, callbacks, bases, movers, or physics assertions.
"""

from __future__ import annotations

import argparse
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass
import hashlib
import json
import math
from pathlib import Path, PurePosixPath
import shutil
import sys
import tempfile
import threading
from typing import Mapping, Sequence

import game_test


TRACE_VERSION = 1
FIXTURE_NAME = "TriggerTest2StaticMovement"
MAP_RELATIVE = PurePosixPath("Maps/Studies/TriggerTest2.unr")
MAP_SHA256 = "fffa53fee540d756b9ff6bbc872c64d3bc81eca47f366f3d4071196fae2546fc"
FIXED_DT = 0.016666667
EMITTED_FIXED_DT = 0.0166666675
REQUESTED_TICKS = 1
SCENARIO_ENVIRONMENT = "HP2_STATIC_MOVEMENT_GATE_SCENARIO"
CPP_TRACE_ENVIRONMENT = "HP2_STATIC_BSP_PROBE"
RUST_TRACE_ENVIRONMENT = "HP2_STATIC_MOVEMENT_GATE_TRACE"
TRACE_FILENAME = "static-movement-gate.json"
SCENARIO_FILENAME = "scenario.json"
DEFAULT_TIMEOUT_SECONDS = 60.0
MAX_TIMEOUT_SECONDS = 300.0
VALIDATED_SWEEP_HIT_TIME = 0.7422
VALIDATED_SWEEP_CONTACT = [-3168.0, -7760.0, -231.5]
VALIDATED_SWEEP_LOCATION = [-3168.0, -7760.0, -229.5]


ROLE_VALUES = {
    "probe": "transient:Engine.Effects",
    "spawn_anchor": "Engine.PlayerStart",
}
STEP_SPECS = (
    ("resolve_origin", "FindSpot"),
    ("static_sweep", "MoveActor"),
    ("static_far_move", "FarMoveActor"),
)


class StaticMovementGateError(RuntimeError):
    """The static movement scenario, staged map, or a trace violates its contract."""


@dataclass(frozen=True)
class StagedMap:
    cpp_data_root: Path
    rust_data_root: Path
    scenario_path: Path
    scenario_sha256: str
    map_record: dict[str, str]


def _repo_root() -> Path:
    return Path(__file__).resolve().parent.parent


def _write_json_atomic(path: Path, document: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(path.suffix + ".tmp")
    try:
        with temporary.open("w", encoding="utf-8", newline="\n") as stream:
            json.dump(document, stream, ensure_ascii=False, indent=2, sort_keys=True)
            stream.write("\n")
        temporary.replace(path)
    except OSError as error:
        raise StaticMovementGateError(f"cannot atomically write {path}: {error}") from error


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    try:
        with path.open("rb") as stream:
            for block in iter(lambda: stream.read(1024 * 1024), b""):
                digest.update(block)
    except OSError as error:
        raise StaticMovementGateError(f"cannot hash {path}: {error}") from error
    return digest.hexdigest()


def _resolve(path: Path, label: str, *, must_exist: bool = True) -> Path:
    try:
        return path.expanduser().resolve(strict=must_exist)
    except OSError as error:
        raise StaticMovementGateError(f"cannot resolve {label} {path}: {error}") from error


def _require_file(path: Path, label: str, *, executable: bool = False) -> Path:
    resolved = _resolve(path, label)
    if not resolved.is_file():
        raise StaticMovementGateError(f"{label} is not a file: {resolved}")
    if executable and not resolved.stat().st_mode & 0o111:
        raise StaticMovementGateError(f"{label} is not executable: {resolved}")
    return resolved


def _require_data_root(path: Path) -> Path:
    root = _resolve(path, "data root")
    if not root.is_dir():
        raise StaticMovementGateError(f"data root is not a directory: {root}")
    return root


def scenario_document() -> dict[str, object]:
    """Return the sole coordinate-free static-movement input shared by both engines."""
    return {
        "version": TRACE_VERSION,
        "fixture": FIXTURE_NAME,
        "map": {"relative": MAP_RELATIVE.as_posix(), "sha256": MAP_SHA256},
        "fixed_dt": FIXED_DT,
        "requested_ticks": REQUESTED_TICKS,
        "roles": dict(ROLE_VALUES),
        "selectors": {
            "spawn_anchor": {"class": "Engine.PlayerStart", "cardinality": 1},
        },
        "transients": {
            "probe": {
                "class": "Engine.Effects",
                "collision_radius": 16,
                "collision_height": 24,
                "collide_world": True,
                "collide_actors": False,
                "block_actors": False,
                "block_players": False,
            },
        },
        "operations": [
            {
                "id": "resolve_origin",
                "operation": "FindSpot",
                "actor": "probe",
                "relative_to": "spawn_anchor",
                "offset": [0, 0, 26],
                "check_actors": False,
            },
            {
                "id": "static_sweep",
                "operation": "MoveActor",
                "actor": "probe",
                "delta": [0, 0, -64],
                "rotation": [10, 20, 30],
            },
            {
                "id": "static_far_move",
                "operation": "FarMoveActor",
                "actor": "probe",
                "destination": "resolved_origin",
                "rotation": [40, 50, 60],
            },
        ],
    }


def _copy_data_root(source: Path, destination: Path) -> None:
    try:
        shutil.copytree(source, destination, symlinks=True)
    except OSError as error:
        raise StaticMovementGateError(f"cannot stage data root {source} to {destination}: {error}") from error


def stage_map(*, data_root: Path, artifact_dir: Path) -> StagedMap:
    """Copy verified TriggerTest2 bytes into separate C++ and Rust data roots."""
    source_root = _require_data_root(data_root)
    source_map = _require_file(source_root / MAP_RELATIVE.as_posix(), "TriggerTest2 map")
    source_sha256 = _sha256(source_map)
    if source_sha256 != MAP_SHA256:
        raise StaticMovementGateError(
            f"TriggerTest2 map SHA-256 mismatch: expected {MAP_SHA256}, got {source_sha256}"
        )

    destination = _resolve(artifact_dir, "artifact directory", must_exist=False)
    cpp_root = destination / "cpp-data"
    rust_root = destination / "rust-data"
    if cpp_root.exists() or rust_root.exists():
        raise StaticMovementGateError(f"static movement staging already exists: {destination}")
    _copy_data_root(source_root, cpp_root)
    try:
        _copy_data_root(source_root, rust_root)
    except StaticMovementGateError:
        shutil.rmtree(cpp_root, ignore_errors=True)
        raise

    cpp_map = _require_file(cpp_root / MAP_RELATIVE.as_posix(), "staged C++ TriggerTest2 map")
    rust_map = _require_file(rust_root / MAP_RELATIVE.as_posix(), "staged Rust TriggerTest2 map")
    cpp_sha256 = _sha256(cpp_map)
    rust_sha256 = _sha256(rust_map)
    if cpp_sha256 != MAP_SHA256 or rust_sha256 != MAP_SHA256:
        raise StaticMovementGateError("staging changed the verified TriggerTest2 map bytes")

    scenario_path = destination / SCENARIO_FILENAME
    _write_json_atomic(scenario_path, scenario_document())
    return StagedMap(
        cpp_data_root=cpp_root,
        rust_data_root=rust_root,
        scenario_path=scenario_path,
        scenario_sha256=_sha256(scenario_path),
        map_record={
            "source_sha256": source_sha256,
            "cpp_sha256": cpp_sha256,
            "rust_sha256": rust_sha256,
        },
    )


def _required_object(value: object, path: str) -> Mapping[str, object]:
    if not isinstance(value, dict):
        raise StaticMovementGateError(f"{path} must be a JSON object")
    return value


def _required_keys(value: object, expected: set[str], path: str) -> Mapping[str, object]:
    mapping = _required_object(value, path)
    actual = set(mapping)
    if actual != expected:
        missing = sorted(expected - actual)
        extra = sorted(actual - expected)
        raise StaticMovementGateError(f"{path} keys are invalid: missing={missing}, extra={extra}")
    return mapping


def _string(value: object, path: str) -> str:
    if not isinstance(value, str) or not value:
        raise StaticMovementGateError(f"{path} must be a non-empty string")
    return value


def _integer(value: object, path: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int):
        raise StaticMovementGateError(f"{path} must be an integer")
    return value


def _quantised_number(value: object, path: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(value):
        raise StaticMovementGateError(f"{path} must be a finite number")
    quantised = round(float(value), 4)
    return 0.0 if quantised == 0.0 else quantised


def _vector(value: object, path: str) -> list[float]:
    if not isinstance(value, list) or len(value) != 3:
        raise StaticMovementGateError(f"{path} must be a three-number vector")
    return [_quantised_number(component, f"{path}[{index}]") for index, component in enumerate(value)]


def _zone(value: object, path: str) -> dict[str, str] | None:
    if value is None:
        return None
    zone = _required_keys(value, {"class", "tag"}, path)
    return {"class": _string(zone["class"], f"{path}.class"), "tag": _string(zone["tag"], f"{path}.tag")}


def _actor(value: object, path: str) -> dict[str, object]:
    actor = _required_keys(value, {"location", "rotation", "zone", "b_just_teleported"}, path)
    teleported = actor["b_just_teleported"]
    if not isinstance(teleported, bool):
        raise StaticMovementGateError(f"{path}.b_just_teleported must be a boolean")
    return {
        "location": _vector(actor["location"], f"{path}.location"),
        "rotation": _vector(actor["rotation"], f"{path}.rotation"),
        "zone": _zone(actor["zone"], f"{path}.zone"),
        "b_just_teleported": teleported,
    }


def _raise_scenario_error(document: Mapping[str, object], source: str) -> None:
    if "scenario_error" not in document:
        return
    if set(document) != {"scenario_error"}:
        raise StaticMovementGateError(f"{source}.scenario_error must be the sole error report field")
    error = _required_object(document["scenario_error"], f"{source}.scenario_error")
    code = _string(error.get("code"), f"{source}.scenario_error.code")
    detail = error.get("detail")
    if detail is not None and not isinstance(detail, str):
        raise StaticMovementGateError(f"{source}.scenario_error.detail must be a string when supplied")
    suffix = f": {detail}" if detail else ""
    raise StaticMovementGateError(f"{source}.scenario_error {code}{suffix}")


def _normalise_request(step_id: str, value: object, path: str) -> dict[str, object]:
    request = _required_object(value, path)
    if step_id == "resolve_origin":
        request = _required_keys(request, {"operation", "requested_origin", "extent", "check_actors"}, path)
        if request["operation"] != "FindSpot":
            raise StaticMovementGateError(f"{path}.operation must be 'FindSpot'")
        if request["check_actors"] is not False:
            raise StaticMovementGateError(f"{path}.check_actors must be false")
        extent = _vector(request["extent"], f"{path}.extent")
        if extent != [16.0, 16.0, 24.0]:
            raise StaticMovementGateError(f"{path}.extent must be [16, 16, 24]")
        return {
            "operation": "FindSpot",
            "requested_origin": _vector(request["requested_origin"], f"{path}.requested_origin"),
            "extent": extent,
            "check_actors": False,
        }
    if step_id == "static_sweep":
        request = _required_keys(request, {"operation", "delta", "rotation"}, path)
        if request["operation"] != "MoveActor":
            raise StaticMovementGateError(f"{path}.operation must be 'MoveActor'")
        delta = _vector(request["delta"], f"{path}.delta")
        rotation = _vector(request["rotation"], f"{path}.rotation")
        if delta != [0.0, 0.0, -64.0]:
            raise StaticMovementGateError(f"{path}.delta must be [0, 0, -64]")
        if rotation != [10.0, 20.0, 30.0]:
            raise StaticMovementGateError(f"{path}.rotation must be [10, 20, 30]")
        return {"operation": "MoveActor", "delta": delta, "rotation": rotation}
    if step_id == "static_far_move":
        request = _required_keys(request, {"operation", "destination", "rotation"}, path)
        if request["operation"] != "FarMoveActor":
            raise StaticMovementGateError(f"{path}.operation must be 'FarMoveActor'")
        rotation = _vector(request["rotation"], f"{path}.rotation")
        if rotation != [40.0, 50.0, 60.0]:
            raise StaticMovementGateError(f"{path}.rotation must be [40, 50, 60]")
        return {
            "operation": "FarMoveActor",
            "destination": _vector(request["destination"], f"{path}.destination"),
            "rotation": rotation,
        }
    raise AssertionError(f"unrecognized static movement step {step_id}")


def _normalise_result(step_id: str, value: object, path: str) -> dict[str, object]:
    result = _required_object(value, path)
    if step_id == "resolve_origin":
        result = _required_keys(result, {"found", "resolved_origin"}, path)
        if not isinstance(result["found"], bool):
            raise StaticMovementGateError(f"{path}.found must be a boolean")
        return {"found": result["found"], "resolved_origin": _vector(result["resolved_origin"], f"{path}.resolved_origin")}
    if step_id == "static_sweep":
        result = _required_keys(
            result,
            {"moved", "hit_time", "blocked", "hit_actor", "hit_primitive", "location", "normal", "item"},
            path,
        )
        if not isinstance(result["moved"], bool):
            raise StaticMovementGateError(f"{path}.moved must be a boolean")
        if not isinstance(result["blocked"], bool):
            raise StaticMovementGateError(f"{path}.blocked must be a boolean")
        if result["hit_actor"] is not None:
            raise StaticMovementGateError(f"{path}.hit_actor must be null for the static-only sweep")
        if result["hit_primitive"] != "static_bsp":
            raise StaticMovementGateError(f"{path}.hit_primitive must be 'static_bsp'")
        return {
            "moved": result["moved"],
            "hit_time": _quantised_number(result["hit_time"], f"{path}.hit_time"),
            "blocked": result["blocked"],
            "hit_actor": None,
            "hit_primitive": "static_bsp",
            "location": _vector(result["location"], f"{path}.location"),
            "normal": _vector(result["normal"], f"{path}.normal"),
            "item": _integer(result["item"], f"{path}.item"),
        }
    if step_id == "static_far_move":
        result = _required_keys(result, {"moved"}, path)
        if not isinstance(result["moved"], bool):
            raise StaticMovementGateError(f"{path}.moved must be a boolean")
        return {"moved": result["moved"]}
    raise AssertionError(f"unrecognized static movement step {step_id}")


def _validate_step_contract(steps: list[dict[str, object]], source: str) -> None:
    resolved = steps[0]
    sweep = steps[1]
    far_move = steps[2]
    resolved_request = resolved["request"]
    resolved_result = resolved["result"]
    sweep_request = sweep["request"]
    sweep_result = sweep["result"]
    far_request = far_move["request"]
    far_result = far_move["result"]
    assert isinstance(resolved_request, dict)
    assert isinstance(resolved_result, dict)
    assert isinstance(sweep_request, dict)
    assert isinstance(sweep_result, dict)
    assert isinstance(far_request, dict)
    assert isinstance(far_result, dict)

    if not resolved_result["found"]:
        raise StaticMovementGateError(f"{source}.steps[0].result.found must be true")
    if resolved_request["requested_origin"] != resolved_result["resolved_origin"]:
        raise StaticMovementGateError(f"{source}.steps[0] FindSpot must leave the static origin unchanged")
    if not sweep_result["moved"] or not sweep_result["blocked"]:
        raise StaticMovementGateError(f"{source}.steps[1] must commit a blocked static sweep")
    hit_time = sweep_result["hit_time"]
    if hit_time != VALIDATED_SWEEP_HIT_TIME:
        raise StaticMovementGateError(
            f"{source}.steps[1].result.hit_time must be the validated static BSP time "
            f"{VALIDATED_SWEEP_HIT_TIME:.4f}"
        )
    if sweep_result["location"] != VALIDATED_SWEEP_CONTACT:
        raise StaticMovementGateError(
            f"{source}.steps[1].result.location must be the validated static BSP contact "
            f"{VALIDATED_SWEEP_CONTACT}"
        )
    if sweep_result["normal"] != [0.0, 0.0, 1.0]:
        raise StaticMovementGateError(f"{source}.steps[1].result.normal must be +Z")
    if sweep_result["item"] != 26:
        raise StaticMovementGateError(f"{source}.steps[1].result.item must be the validated static BSP item 26")
    if not far_result["moved"]:
        raise StaticMovementGateError(f"{source}.steps[2].result.moved must be true")
    if far_request["destination"] != resolved_result["resolved_origin"]:
        raise StaticMovementGateError(f"{source}.steps[2] FarMoveActor must target the resolved origin")

    actors = [step["actor"] for step in steps]
    if not all(isinstance(actor, dict) for actor in actors):
        raise AssertionError("normalized static movement actors must be dictionaries")
    resolved_actor, sweep_actor, far_actor = actors
    assert isinstance(resolved_actor, dict)
    assert isinstance(sweep_actor, dict)
    assert isinstance(far_actor, dict)
    if resolved_actor["location"] != resolved_result["resolved_origin"]:
        raise StaticMovementGateError(f"{source}.steps[0].actor.location must be the resolved origin")
    if sweep_actor["rotation"] != sweep_request["rotation"]:
        raise StaticMovementGateError(f"{source}.steps[1].actor.rotation must be the requested MoveActor rotation")
    if far_actor["location"] != resolved_result["resolved_origin"]:
        raise StaticMovementGateError(f"{source}.steps[2].actor.location must return to the resolved origin")
    if far_actor["rotation"] != far_request["rotation"]:
        raise StaticMovementGateError(f"{source}.steps[2].actor.rotation must be the requested FarMoveActor rotation")
    if not resolved_actor["b_just_teleported"] or not far_actor["b_just_teleported"]:
        raise StaticMovementGateError(f"{source} FindSpot placement and FarMoveActor must mark the probe teleported")

    zones = [actor["zone"] for actor in actors]
    if sweep_actor["location"] != VALIDATED_SWEEP_LOCATION:
        raise StaticMovementGateError(
            f"{source}.steps[1].actor.location must be the validated static BSP location "
            f"{VALIDATED_SWEEP_LOCATION}"
        )
    if zones[0] is None or zones[1] is None or zones[2] is None or zones[0] != zones[1] or zones[0] != zones[2]:
        raise StaticMovementGateError(f"{source} probe zone must be non-null and stable across static movement")


def _normalise_trace(document: object, source: str) -> dict[str, object]:
    if isinstance(document, dict):
        _raise_scenario_error(document, source)
    root = _required_keys(
        document,
        {"version", "fixture", "map", "fixed_dt", "requested_ticks", "roles", "steps", "random_calls"},
        source,
    )
    if root["version"] != TRACE_VERSION:
        raise StaticMovementGateError(f"{source}.version must be {TRACE_VERSION}")
    if root["fixture"] != FIXTURE_NAME:
        raise StaticMovementGateError(f"{source}.fixture must be {FIXTURE_NAME!r}")
    map_value = _required_keys(root["map"], {"relative", "sha256"}, f"{source}.map")
    if dict(map_value) != {"relative": MAP_RELATIVE.as_posix(), "sha256": MAP_SHA256}:
        raise StaticMovementGateError(f"{source}.map must identify the verified TriggerTest2 bytes")
    fixed_dt = root["fixed_dt"]
    if isinstance(fixed_dt, bool) or not isinstance(fixed_dt, (int, float)) or not math.isfinite(fixed_dt):
        raise StaticMovementGateError(f"{source}.fixed_dt must be a finite number")
    if not math.isclose(float(fixed_dt), EMITTED_FIXED_DT, rel_tol=0.0, abs_tol=1.0e-9):
        raise StaticMovementGateError(f"{source}.fixed_dt must be {EMITTED_FIXED_DT:.10g}")
    if root["requested_ticks"] != REQUESTED_TICKS:
        raise StaticMovementGateError(f"{source}.requested_ticks must be {REQUESTED_TICKS}")
    roles = _required_keys(root["roles"], set(ROLE_VALUES), f"{source}.roles")
    if dict(roles) != ROLE_VALUES:
        raise StaticMovementGateError(f"{source}.roles must be the canonical static role map")
    if root["random_calls"] != []:
        raise StaticMovementGateError(f"{source}.random_calls must be []")

    raw_steps = root["steps"]
    if not isinstance(raw_steps, list) or len(raw_steps) != len(STEP_SPECS):
        raise StaticMovementGateError(f"{source}.steps must contain the canonical bounded sequence")
    steps: list[dict[str, object]] = []
    for index, (step_id, operation) in enumerate(STEP_SPECS):
        path = f"{source}.steps[{index}]"
        raw_step = _required_keys(raw_steps[index], {"id", "request", "result", "actor"}, path)
        if raw_step["id"] != step_id:
            raise StaticMovementGateError(f"{path}.id must be {step_id!r}")
        request = _normalise_request(step_id, raw_step["request"], f"{path}.request")
        if request["operation"] != operation:
            raise StaticMovementGateError(f"{path}.request.operation must be {operation!r}")
        steps.append({
            "id": step_id,
            "request": request,
            "result": _normalise_result(step_id, raw_step["result"], f"{path}.result"),
            "actor": _actor(raw_step["actor"], f"{path}.actor"),
        })
    _validate_step_contract(steps, source)
    return {
        "version": TRACE_VERSION,
        "fixture": FIXTURE_NAME,
        "map": {"relative": MAP_RELATIVE.as_posix(), "sha256": MAP_SHA256},
        "fixed_dt": EMITTED_FIXED_DT,
        "requested_ticks": REQUESTED_TICKS,
        "roles": dict(ROLE_VALUES),
        "steps": steps,
        "random_calls": [],
    }


def read_atomic_trace(path: Path) -> dict[str, object]:
    """Read a trace only after its writer has atomically replaced the final path."""
    trace_path = _require_file(path, "static movement trace")
    temporary = trace_path.with_suffix(trace_path.suffix + ".tmp")
    if temporary.exists():
        raise StaticMovementGateError(f"static movement trace atomic temporary remains: {temporary}")
    try:
        document = json.loads(trace_path.read_bytes())
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise StaticMovementGateError(f"static movement trace is not valid atomic JSON: {trace_path}: {error}") from error
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
    """Compare every normalized static movement observation without a dynamic shim."""
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
    trace_environment: str,
) -> dict[str, object]:
    side_dir = artifact_dir / name
    side_dir.mkdir(parents=True, exist_ok=True)
    trace_path = side_dir / TRACE_FILENAME
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
            SCENARIO_ENVIRONMENT: str(scenario_path),
            trace_environment: str(trace_path),
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
        result["trace"] = read_atomic_trace(trace_path)
    except StaticMovementGateError as error:
        result["trace_error"] = str(error)
    _write_json_atomic(side_dir / "result.json", result)
    return result


def _timeout(value: float) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(value):
        raise StaticMovementGateError("timeout must be a finite number")
    if not 0.0 < float(value) <= MAX_TIMEOUT_SECONDS:
        raise StaticMovementGateError(f"timeout must be within (0, {MAX_TIMEOUT_SECONDS:g}]")
    return float(value)


def run_static_movement_gate(
    *,
    app: Path,
    engine_bin: Path,
    data_root: Path,
    output: Path,
    renderer: str = "xopengl",
    timeout_seconds: float = DEFAULT_TIMEOUT_SECONDS,
) -> dict[str, object]:
    """Stage TriggerTest2 and compare C++ and Rust static-BSP movement traces."""
    if renderer not in ("xopengl", "vulkan"):
        raise StaticMovementGateError(f"unsupported renderer: {renderer}")
    timeout = _timeout(timeout_seconds)
    cpp_executable = _executable(app, "C++ application")
    rust_executable = _executable(engine_bin, "Rust engine binary")
    output_path = _resolve(output, "output", must_exist=False)
    artifact_dir = output_path.parent / f"{output_path.stem}-artifacts"
    staged = stage_map(data_root=data_root, artifact_dir=artifact_dir)
    barrier = threading.Barrier(2)
    with ThreadPoolExecutor(max_workers=2, thread_name_prefix="static-movement-gate") as executor:
        cpp_future = executor.submit(
            _run_side,
            name="cpp",
            executable=cpp_executable,
            data_root=staged.cpp_data_root,
            scenario_path=staged.scenario_path,
            artifact_dir=artifact_dir,
            renderer=renderer,
            timeout_seconds=timeout,
            start_gate=barrier,
            trace_environment=CPP_TRACE_ENVIRONMENT,
        )
        rust_future = executor.submit(
            _run_side,
            name="rust",
            executable=rust_executable,
            data_root=staged.rust_data_root,
            scenario_path=staged.scenario_path,
            artifact_dir=artifact_dir,
            renderer=renderer,
            timeout_seconds=timeout,
            start_gate=barrier,
            trace_environment=RUST_TRACE_ENVIRONMENT,
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
        assert isinstance(cpp_trace, Mapping)
        assert isinstance(rust_trace, Mapping)
        comparison = compare_traces(cpp_trace, rust_trace)
    passed = bool(cpp["process"]["passed"]) and bool(rust["process"]["passed"]) and comparison["status"] == "match"
    report = {
        "version": TRACE_VERSION,
        "fixture": FIXTURE_NAME,
        "map": {"relative": MAP_RELATIVE.as_posix(), "sha256": MAP_SHA256},
        "fixed_dt": FIXED_DT,
        "emitted_fixed_dt": EMITTED_FIXED_DT,
        "requested_ticks": REQUESTED_TICKS,
        "roles": dict(ROLE_VALUES),
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


def _positive_timeout(value: str) -> float:
    try:
        parsed = float(value)
    except ValueError as error:
        raise argparse.ArgumentTypeError("must be a number") from error
    try:
        return _timeout(parsed)
    except StaticMovementGateError as error:
        raise argparse.ArgumentTypeError(str(error)) from error


def _arguments(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--app", required=True, type=Path, help="C++ oracle app bundle or executable")
    parser.add_argument("--engine-bin", required=True, type=Path, help="Rust engine executable")
    parser.add_argument("--data-root", required=True, type=Path, help="unmodified HP2 data root")
    parser.add_argument("--output", required=True, type=Path, help="atomic static-movement gate report")
    parser.add_argument("--renderer", choices=("xopengl", "vulkan"), default="xopengl")
    parser.add_argument("--timeout", type=_positive_timeout, default=DEFAULT_TIMEOUT_SECONDS)
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    arguments = _arguments(argv)
    try:
        report = run_static_movement_gate(
            app=arguments.app,
            engine_bin=arguments.engine_bin,
            data_root=arguments.data_root,
            output=arguments.output,
            renderer=arguments.renderer,
            timeout_seconds=arguments.timeout,
        )
    except StaticMovementGateError as error:
        print(f"static movement gate: {error}", file=sys.stderr)
        return 2
    return 0 if report["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
