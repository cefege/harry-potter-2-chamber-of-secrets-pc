#!/usr/bin/env python3
"""Validate, stage, and compare a requested map's startup RNG generation.

Both runtimes publish this opt-in artifact before the first actor Tick. Events
retain every startup map generation for bootstrap diagnosis, but comparison is
restricted to the requested map token so C++ Entry startup cannot be folded
into a direct Rust map launch.
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Iterable, NoReturn

TRACE_VERSION = 2
ROOT_FIELDS = frozenset({"version", "enabled", "phases"})
PHASE_FIELDS = frozenset({
    "phase",
    "map_token",
    "map_url",
    "load_ordinal",
    "lifecycle_boundary",
    "ordinal_before",
    "ordinal_after",
    "delta",
})
LEAF_PHASES = (
    "fire_init_tables",
    "collision_hash",
)
PRE_FIRST_LEVEL_TICK_PHASE = "pre_first_level_tick"
OPTIONAL_PHASES = ("pre_render_fglobalrandoms",)
PHASE_ORDER = (
    *LEAF_PHASES,
    "pre_load_map",
    "post_load_map",
    PRE_FIRST_LEVEL_TICK_PHASE,
    *OPTIONAL_PHASES,
)
PHASE_RANK = {name: index for index, name in enumerate(PHASE_ORDER)}
LIFECYCLE_BOUNDARIES = frozenset({
    "map_loaded",
    "bring_up_for_play_complete",
    "before_actor_tick",
})
MAX_ORDINAL = 0xFFFFFFFFFFFFFFFF


class TraceError(ValueError):
    """A startup RNG phase artifact violates the shared schema."""


def _problem(path: str, detail: str) -> NoReturn:
    raise TraceError(f"{path}: {detail}")


def _integer(value: object) -> bool:
    return isinstance(value, int) and not isinstance(value, bool)


def _object(value: object, path: str, fields: frozenset[str]) -> dict[str, object]:
    if not isinstance(value, dict):
        _problem(path, "must be an object")
    actual = frozenset(value)
    if actual != fields:
        missing = sorted(fields - actual)
        unknown = sorted(actual - fields)
        if missing:
            _problem(path, f"missing required field {missing[0]!r}")
        _problem(path, f"contains unknown field {unknown[0]!r}")
    return value


def _phase(value: object, index: int) -> dict[str, object]:
    path = f"$.phases[{index}]"
    phase = _object(value, path, PHASE_FIELDS)
    name = phase["phase"]
    if name not in PHASE_RANK:
        _problem(f"{path}.phase", "must be a supported startup RNG phase")
    for field in ("map_token", "map_url", "lifecycle_boundary"):
        if not isinstance(phase[field], str):
            _problem(f"{path}.{field}", "must be a string")
    boundary = phase["lifecycle_boundary"]
    assert isinstance(boundary, str)
    if boundary not in LIFECYCLE_BOUNDARIES:
        _problem(f"{path}.lifecycle_boundary", "must be a supported lifecycle boundary")
    for field in ("load_ordinal", "ordinal_before", "ordinal_after", "delta"):
        if not _integer(phase[field]) or not 0 <= phase[field] <= MAX_ORDINAL:
            _problem(f"{path}.{field}", "must be an unsigned 64-bit JSON integer")
    load_ordinal = phase["load_ordinal"]
    map_token = phase["map_token"]
    map_url = phase["map_url"]
    before = phase["ordinal_before"]
    after = phase["ordinal_after"]
    delta = phase["delta"]
    assert isinstance(load_ordinal, int)
    assert isinstance(map_token, str) and isinstance(map_url, str)
    assert isinstance(before, int) and isinstance(after, int) and isinstance(delta, int)
    if load_ordinal == 0:
        if map_token or map_url:
            _problem(path, "bootstrap load_ordinal 0 must have empty map_token and map_url")
    elif not map_token or not map_url:
        _problem(path, "map generation must have non-empty map_token and map_url")
    if after < before:
        _problem(f"{path}.ordinal_after", "cannot precede ordinal_before")
    if delta != after - before:
        _problem(f"{path}.delta", "must equal ordinal_after - ordinal_before")
    return phase


def validate_trace(document: object) -> dict[str, object]:
    """Return a validated startup phase artifact or raise ``TraceError``."""
    trace = _object(document, "$", ROOT_FIELDS)
    if trace["version"] != TRACE_VERSION or isinstance(trace["version"], bool):
        _problem("$.version", f"must equal {TRACE_VERSION}")
    if trace["enabled"] is not True:
        _problem("$.enabled", "must be true")
    phases = trace["phases"]
    if not isinstance(phases, list):
        _problem("$.phases", "must be an array")

    generations: dict[int, tuple[str, str]] = {}
    for index, value in enumerate(phases):
        phase = _phase(value, index)
        load_ordinal = phase["load_ordinal"]
        map_token = phase["map_token"]
        map_url = phase["map_url"]
        assert isinstance(load_ordinal, int)
        assert isinstance(map_token, str) and isinstance(map_url, str)
        if load_ordinal:
            identity = (map_token, map_url)
            previous = generations.setdefault(load_ordinal, identity)
            if previous != identity:
                _problem(
                    f"$.phases[{index}]",
                    f"load_ordinal {load_ordinal} must retain one map token and URL identity",
                )
    return trace


def phase_totals(trace: dict[str, object]) -> dict[str, int]:
    """Aggregate raw interval deltas by phase without discarding generations."""
    phases = trace["phases"]
    assert isinstance(phases, list)
    return _phase_totals(phases)


def _phase_totals(phases: Iterable[object]) -> dict[str, int]:
    totals = {phase: 0 for phase in PHASE_ORDER}
    for value in phases:
        assert isinstance(value, dict)
        name = value["phase"]
        delta = value["delta"]
        assert isinstance(name, str) and isinstance(delta, int)
        totals[name] += delta
    return totals


def _target_generation(trace: dict[str, object], requested_map_token: str) -> tuple[int, list[dict[str, object]]]:
    if not isinstance(requested_map_token, str) or not requested_map_token:
        _problem("$.requested_map_token", "must be a non-empty map token")
    phases = trace["phases"]
    assert isinstance(phases, list)
    matching_loads = {
        value["load_ordinal"]
        for value in phases
        if isinstance(value, dict)
        and value["map_token"] == requested_map_token
        and value["load_ordinal"] > 0
    }
    if not matching_loads:
        _problem("$.phases", f"does not contain requested map token {requested_map_token!r}")
    load_ordinal = max(matching_loads)
    selected = [
        value for value in phases
        if isinstance(value, dict) and value["load_ordinal"] == load_ordinal
    ]
    assert all(isinstance(value, dict) for value in selected)
    return load_ordinal, selected


def _target_before_actor_tick(phases: Iterable[dict[str, object]], path: str) -> int:
    markers = [
        phase["ordinal_before"]
        for phase in phases
        if phase["phase"] == PRE_FIRST_LEVEL_TICK_PHASE
        and phase["lifecycle_boundary"] == "before_actor_tick"
    ]
    if not markers:
        _problem(path, "missing target before_actor_tick marker")
    assert all(isinstance(marker, int) for marker in markers)
    return max(markers)


def _target_start(phases: Iterable[dict[str, object]], path: str) -> int:
    starts = [phase["ordinal_before"] for phase in phases]
    if not starts:
        _problem(path, "contains no events")
    assert all(isinstance(start, int) for start in starts)
    return min(starts)


def _difference(cpp: object, rust: object, path: str) -> dict[str, object] | None:
    if type(cpp) is not type(rust) or cpp != rust:
        return {"path": path, "cpp": cpp, "rust": rust}
    return None


def compare_traces(
    cpp_document: object,
    rust_document: object,
    *,
    requested_map_token: str,
) -> dict[str, object]:
    """Compare process-global leaves and one requested map's residual.

    Fire tables and the collision-hash basis are process-global initializers:
    C++ may initialize them while loading Entry before travelling to the
    requested map, while Rust may initialize them during a direct requested-map
    launch. Compare their full-trace totals.  The remaining requested-map
    interval is then compared after subtracting only leaves physically scoped
    to that generation, preserving nested Entry/target map-load telemetry.
    """
    cpp = validate_trace(cpp_document)
    rust = validate_trace(rust_document)
    cpp_load, cpp_phases = _target_generation(cpp, requested_map_token)
    rust_load, rust_phases = _target_generation(rust, requested_map_token)
    del cpp_load, rust_load  # Raw ordinals intentionally differ across launch paths.

    cpp_global_totals = phase_totals(cpp)
    rust_global_totals = phase_totals(rust)
    for phase in LEAF_PHASES:
        difference = _difference(
            cpp_global_totals[phase],
            rust_global_totals[phase],
            f"$.process_phase_totals.{phase}.delta",
        )
        if difference is not None:
            return {
                "status": "mismatch",
                "first_difference": {"phase": phase, **difference},
            }

    cpp_target_totals = _phase_totals(cpp_phases)
    rust_target_totals = _phase_totals(rust_phases)
    cpp_residual = (
        _target_before_actor_tick(cpp_phases, "$.cpp.target_generation")
        - _target_start(cpp_phases, "$.cpp.target_generation")
        - sum(cpp_target_totals[phase] for phase in LEAF_PHASES)
    )
    rust_residual = (
        _target_before_actor_tick(rust_phases, "$.rust.target_generation")
        - _target_start(rust_phases, "$.rust.target_generation")
        - sum(rust_target_totals[phase] for phase in LEAF_PHASES)
    )
    if cpp_residual < 0 or rust_residual < 0:
        _problem("$.target_generation", "before_actor_tick total cannot be smaller than leaf draw totals")
    difference = _difference(cpp_residual, rust_residual, "$.target_startup_residual.delta")
    if difference is not None:
        return {
            "status": "mismatch",
            "first_difference": {"phase": "startup_residual", **difference},
        }
    return {"status": "match", "first_difference": None}


def _read_trace(path: Path, side: str) -> dict[str, object]:
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as error:
        raise TraceError(f"{side} trace {path}: file does not exist") from error
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise TraceError(f"{side} trace {path}: unreadable JSON: {error}") from error
    return validate_trace(document)


def _write_report(path: Path | None, report: dict[str, object]) -> None:
    rendered = json.dumps(report, ensure_ascii=True, allow_nan=False, sort_keys=True, indent=2) + "\n"
    if path is None:
        print(rendered, end="")
        return
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
        temporary = path.with_suffix(path.suffix + ".tmp")
        temporary.write_text(rendered, encoding="utf-8")
        temporary.replace(path)
    except OSError as error:
        raise TraceError(f"cannot write report {path}: {error}") from error


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cpp-trace", type=Path, required=True, help="C++ startup RNG phase JSON")
    parser.add_argument("--rust-trace", type=Path, required=True, help="Rust startup RNG phase JSON")
    parser.add_argument("--map-token", required=True, help="requested target map token")
    parser.add_argument("--report", type=Path, help="write JSON comparison report here (stdout otherwise)")
    args = parser.parse_args(argv)
    try:
        report = compare_traces(
            _read_trace(args.cpp_trace, "C++"),
            _read_trace(args.rust_trace, "Rust"),
            requested_map_token=args.map_token,
        )
        _write_report(args.report, report)
        return 0 if report["status"] == "match" else 1
    except TraceError as error:
        parser.error(str(error))
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
