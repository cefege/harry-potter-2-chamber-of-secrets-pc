#!/usr/bin/env python3
"""Validate and strictly compare bounded CreatureGenerator Tick diagnostics.

The C++ oracle and hp2rs publish the same opt-in v1 JSON artifact. Records
are joined by normalized generator identity, frame, and per-generator occurrence;
Tick order is compared as diagnostic event data. Map-owned object-reference paths
are normalized while class and external-package paths remain exact. This utility
never launches either runtime or resolves a runtime reference.
"""

from __future__ import annotations

import argparse
import json
import math
from pathlib import Path
from typing import NoReturn

TRACE_VERSION = 1
ROOT_FIELDS = frozenset({"version", "enabled", "limit", "count", "truncated", "events"})
EVENT_FIELDS = frozenset({
    "identity", "frame", "tick", "timing", "b_off", "trigger", "active",
    "first_pp", "camera_visible", "base_creature_count", "gates", "rng", "spawn",
})
TICK_FIELDS = frozenset({"eligible", "order"})
TIMING_FIELDS = frozenset({"curr_time", "wait_time", "delta_seconds"})
TRIGGER_FIELDS = frozenset({"waiting_time", "generate_creature", "tag"})
ACTIVE_FIELDS = frozenset({"count", "max"})
GATE_FIELDS = frozenset({"name", "passed"})
RNG_RANGE_FIELDS = frozenset({"kind", "raw", "index", "resolved_class", "min", "max", "result"})
RNG_SELECTION_FIELDS = frozenset({"kind", "raw", "index", "resolved_class", "bound"})
SPAWN_FIELDS = frozenset({"request", "result", "published_child"})
REFERENCE_FIELDS = frozenset({"path", "class"})
GATE_ORDER = (
    "tick_wait",
    "b_off",
    "trigger_waiting",
    "active_at_capacity",
    "missing_first_pp",
    "camera_visible",
    "empty_base_creatures",
    "class_resolution",
)

DIAGNOSTIC_FLOAT_TOLERANCE = 0.000001
DIAGNOSTIC_FLOAT_PATH_SUFFIXES = (
    ".timing.curr_time",
    ".timing.wait_time",
    ".timing.delta_seconds",
    ".trigger.waiting_time",
)


class TraceError(ValueError):
    """A supplied diagnostic artifact does not meet the shared v1 schema."""


def _problem(path: str, detail: str) -> NoReturn:
    raise TraceError(f"{path}: {detail}")


def _integer(value: object) -> bool:
    return isinstance(value, int) and not isinstance(value, bool)


def _finite_number(value: object) -> bool:
    return isinstance(value, (int, float)) and not isinstance(value, bool) and math.isfinite(value)


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


def _nullable_path(value: object, path: str) -> None:
    if value is not None and (not isinstance(value, str) or not value):
        _problem(path, "must be a non-empty string or null")


def _reference(value: object, path: str) -> None:
    if value is None:
        return
    reference = _object(value, path, REFERENCE_FIELDS)
    _nullable_path(reference["path"], f"{path}.path")
    _nullable_path(reference["class"], f"{path}.class")


def _event(value: object, index: int) -> tuple[int, int, str]:
    path = f"$.events[{index}]"
    event = _object(value, path, EVENT_FIELDS)
    identity = event["identity"]
    if not isinstance(identity, str) or not identity.startswith("<map>.") or len(identity) == len("<map>."):
        _problem(f"{path}.identity", "must be a normalized '<map>.' generator identity")
    if not _integer(event["frame"]) or event["frame"] < 0:
        _problem(f"{path}.frame", "must be a non-negative JSON integer")
    tick = _object(event["tick"], f"{path}.tick", TICK_FIELDS)
    if not isinstance(tick["eligible"], bool):
        _problem(f"{path}.tick.eligible", "must be a boolean")
    if not _integer(tick["order"]) or tick["order"] < 0:
        _problem(f"{path}.tick.order", "must be a non-negative JSON integer")
    timing = _object(event["timing"], f"{path}.timing", TIMING_FIELDS)
    for field in TIMING_FIELDS:
        if not _finite_number(timing[field]):
            _problem(f"{path}.timing.{field}", "must be a finite JSON number")
    if not isinstance(event["b_off"], bool):
        _problem(f"{path}.b_off", "must be a boolean")
    trigger = _object(event["trigger"], f"{path}.trigger", TRIGGER_FIELDS)
    if not _finite_number(trigger["waiting_time"]):
        _problem(f"{path}.trigger.waiting_time", "must be a finite JSON number")
    if not isinstance(trigger["generate_creature"], bool):
        _problem(f"{path}.trigger.generate_creature", "must be a boolean")
    _nullable_path(trigger["tag"], f"{path}.trigger.tag")
    active = _object(event["active"], f"{path}.active", ACTIVE_FIELDS)
    for field in ACTIVE_FIELDS:
        if not _integer(active[field]) or active[field] < 0:
            _problem(f"{path}.active.{field}", "must be a non-negative JSON integer")
    _reference(event["first_pp"], f"{path}.first_pp")
    if event["camera_visible"] is not None and not isinstance(event["camera_visible"], bool):
        _problem(f"{path}.camera_visible", "must be a boolean or null")
    if not _integer(event["base_creature_count"]) or not 0 <= event["base_creature_count"] <= 16:
        _problem(f"{path}.base_creature_count", "must be a JSON integer from 0 through 16")
    gates = event["gates"]
    if not isinstance(gates, list) or not gates:
        _problem(f"{path}.gates", "must be a non-empty array")
    if len(gates) > len(GATE_ORDER):
        _problem(f"{path}.gates", "cannot contain more source gates than v1 defines")
    stopped = False
    terminal_gate: str | None = None
    for gate_index, gate_value in enumerate(gates):
        gate_path = f"{path}.gates[{gate_index}]"
        gate = _object(gate_value, gate_path, GATE_FIELDS)
        if gate["name"] != GATE_ORDER[gate_index]:
            _problem(f"{gate_path}.name", f"must equal {GATE_ORDER[gate_index]!r} in C++ source order")
        if not isinstance(gate["passed"], bool):
            _problem(f"{gate_path}.passed", "must be a boolean")
        if stopped:
            _problem(gate_path, "cannot follow an early-return gate")
        stopped = not gate["passed"]
        if stopped:
            terminal_gate = gate["name"]
    if not stopped and len(gates) != len(GATE_ORDER):
        _problem(f"{path}.gates", "must include every source gate when no gate returns")
    rng = event["rng"]
    if not isinstance(rng, list):
        _problem(f"{path}.rng", "must be an array")
    selection_count = 0
    selection_resolved_class: object = None
    saw_selection = False
    for rng_index, selection_value in enumerate(rng):
        selection_path = f"{path}.rng[{rng_index}]"
        if not isinstance(selection_value, dict):
            _problem(selection_path, "must be an object")
        kind = selection_value.get("kind")
        if kind == "rand_range":
            selection = _object(selection_value, selection_path, RNG_RANGE_FIELDS)
            for field in ("min", "max", "result"):
                if not _finite_number(selection[field]):
                    _problem(f"{selection_path}.{field}", "must be a finite JSON number")
            if selection["index"] is not None or selection["resolved_class"] is not None:
                _problem(selection_path, "rand_range index and resolved_class must be null")
        elif kind == "rand":
            selection = _object(selection_value, selection_path, RNG_SELECTION_FIELDS)
            if not _integer(selection["bound"]) or not 2 <= selection["bound"] <= 16:
                _problem(f"{selection_path}.bound", "must be a JSON integer from 2 through 16")
            if not _integer(selection["index"]) or not 0 <= selection["index"] < selection["bound"]:
                _problem(f"{selection_path}.index", "must be an in-range selection JSON integer")
            _nullable_path(selection["resolved_class"], f"{selection_path}.resolved_class")
            if selection["bound"] != event["base_creature_count"]:
                _problem(
                    f"{selection_path}.bound",
                    "must equal base_creature_count for CreatureGenerator selection",
                )
            selection_count += 1
            selection_resolved_class = selection["resolved_class"]
            saw_selection = True
        else:
            _problem(f"{selection_path}.kind", "must be 'rand_range' or 'rand'")
        if not _integer(selection["raw"]) or not 0 <= selection["raw"] <= 0xFFFFFFFF:
            _problem(f"{selection_path}.raw", "must be an unsigned 32-bit JSON integer")
        if saw_selection and kind != "rand":
            _problem(selection_path, "rand_range cannot follow a creature selection")
    if (
        stopped
        and terminal_gate != "class_resolution"
        and any(isinstance(item, dict) and item.get("kind") == "rand" for item in rng)
    ):
        _problem(f"{path}.rng", "cannot select a creature before an earlier early-return gate")
    if not tick["eligible"] and rng:
        _problem(f"{path}.rng", "must be empty when Tick returns for fCurrTime < fWaitTime")
    spawn = _object(event["spawn"], f"{path}.spawn", SPAWN_FIELDS)
    _reference(spawn["request"], f"{path}.spawn.request")
    _reference(spawn["result"], f"{path}.spawn.result")
    _reference(spawn["published_child"], f"{path}.spawn.published_child")
    if stopped and any(spawn[field] is not None for field in SPAWN_FIELDS):
        _problem(f"{path}.spawn", "must be entirely null after an early-return gate")
    if spawn["published_child"] is not None and spawn["result"] is None:
        _problem(f"{path}.spawn.published_child", "requires a spawn result")
    source_gates_passed = (
        len(gates) == len(GATE_ORDER)
        and all(
            isinstance(gate, dict) and gate["passed"] is True
            for gate in gates[:-1]
        )
    )
    if source_gates_passed:
        base_count = event["base_creature_count"]
        assert isinstance(base_count, int)
        if base_count == 0:
            _problem(f"{path}.base_creature_count", "cannot be zero when class resolution is reached")
        if base_count == 1 and selection_count:
            _problem(f"{path}.rng", "must not select randomly for one base creature")
        if base_count > 1 and selection_count == 0:
            _problem(f"{path}.rng", "must include a creature selection when more than one base creature exists")
        if selection_count and bool(gates[-1]["passed"]) != (selection_resolved_class is not None):
            _problem(
                f"{path}.gates[-1].passed",
                "must equal whether the final selected class resolves",
            )
    if not stopped and spawn["request"] is None:
        _problem(f"{path}.spawn.request", "is required when every source gate passes")
    return event["frame"], tick["order"], identity  # type: ignore[return-value]


def validate_trace(document: object) -> dict[str, object]:
    """Return a validated artifact or raise ``TraceError`` with a precise path."""
    trace = _object(document, "$", ROOT_FIELDS)
    if trace["version"] != TRACE_VERSION or isinstance(trace["version"], bool):
        _problem("$.version", f"must equal {TRACE_VERSION}")
    if trace["enabled"] is not True:
        _problem("$.enabled", "must be true")
    for field in ("limit", "count"):
        if not _integer(trace[field]) or trace[field] < 0:
            _problem(f"$.{field}", "must be a non-negative JSON integer")
    if not isinstance(trace["truncated"], bool):
        _problem("$.truncated", "must be a boolean")
    if not isinstance(trace["events"], list):
        _problem("$.events", "must be an array")
    if trace["count"] != len(trace["events"]):
        _problem("$.count", "must equal the events array length")
    if trace["count"] > trace["limit"]:
        _problem("$.count", "cannot exceed limit")
    if trace["truncated"] and trace["count"] != trace["limit"]:
        _problem("$.truncated", "requires count to equal limit")
    prior: tuple[int, int, str] | None = None
    for index, event in enumerate(trace["events"]):
        key = _event(event, index)
        if prior is not None and key <= prior:
            _problem(f"$.events[{index}]", "must be in unique frame/tick/identity traversal order")
        prior = key
    return trace


def _first_difference(cpp: object, rust: object, path: str = "$") -> dict[str, object] | None:
    if (
        path.endswith(DIAGNOSTIC_FLOAT_PATH_SUFFIXES)
        and _finite_number(cpp)
        and _finite_number(rust)
        and math.isclose(cpp, rust, rel_tol=0.0, abs_tol=DIAGNOSTIC_FLOAT_TOLERANCE)
    ):
        return None
    if type(cpp) is not type(rust):
        return {"path": path, "cpp": cpp, "rust": rust}
    if isinstance(cpp, dict):
        for key in cpp:
            difference = _first_difference(cpp[key], rust[key], f"{path}.{key}")
            if difference is not None:
                return difference
        return None
    if isinstance(cpp, list):
        for index, (left, right) in enumerate(zip(cpp, rust, strict=False)):
            difference = _first_difference(left, right, f"{path}[{index}]")
            if difference is not None:
                return difference
        if len(cpp) != len(rust):
            return {"path": f"{path}.length", "cpp": len(cpp), "rust": len(rust)}
        return None
    if cpp != rust:
        return {"path": path, "cpp": cpp, "rust": rust}
    return None


def _canonical_events(
    events: list[object],
) -> list[tuple[tuple[str, int, int], int, dict[str, object]]]:
    occurrences: dict[tuple[str, int], int] = {}
    canonical: list[tuple[tuple[str, int, int], int, dict[str, object]]] = []
    for index, event in enumerate(events):
        assert isinstance(event, dict)
        identity = event["identity"]
        frame = event["frame"]
        assert isinstance(identity, str) and isinstance(frame, int)
        owner_key = (identity, frame)
        occurrence = occurrences.get(owner_key, 0)
        occurrences[owner_key] = occurrence + 1
        canonical.append(((identity, frame, occurrence), index, event))
    return canonical


def _reference_map_root(reference: object) -> str | None:
    if not isinstance(reference, dict):
        return None
    path = reference.get("path")
    if not isinstance(path, str):
        return None
    root, separator, _ = path.partition(".")
    return root if separator and root else None


def _canonical_reference_path(reference: object, map_root: str) -> object:
    if not isinstance(reference, dict):
        return reference
    path = reference.get("path")
    prefix = f"{map_root}."
    if not isinstance(path, str) or not path.startswith(prefix):
        return reference
    canonical = dict(reference)
    canonical["path"] = f"<map>{path[len(map_root):]}"
    return canonical


def _canonicalize_event_references(event: dict[str, object]) -> dict[str, object]:
    map_root = _reference_map_root(event["first_pp"])
    if map_root is None:
        return event
    canonical = dict(event)
    canonical["first_pp"] = _canonical_reference_path(event["first_pp"], map_root)
    spawn = event["spawn"]
    assert isinstance(spawn, dict)
    canonical["spawn"] = {
        field: _canonical_reference_path(spawn[field], map_root)
        for field in SPAWN_FIELDS
    }
    return canonical


def compare_traces(cpp_document: object, rust_document: object) -> dict[str, object]:
    """Report the first differing event key, C++ gate/RNG/value, or a match."""
    cpp = validate_trace(cpp_document)
    rust = validate_trace(rust_document)
    for field in ("version", "enabled", "limit", "truncated"):
        difference = _first_difference(cpp[field], rust[field], f"$.{field}")
        if difference is not None:
            return {"status": "mismatch", "first_difference": difference}
    cpp_events = cpp["events"]
    rust_events = rust["events"]
    assert isinstance(cpp_events, list) and isinstance(rust_events, list)
    cpp_canonical = _canonical_events([
        _canonicalize_event_references(event)
        for event in cpp_events
        if isinstance(event, dict)
    ])
    rust_canonical = {
        key: event
        for key, _index, event in _canonical_events([
            _canonicalize_event_references(event)
            for event in rust_events
            if isinstance(event, dict)
        ])
    }
    for key, index, cpp_event in cpp_canonical:
        rust_event = rust_canonical.pop(key, None)
        rendered_key = {
            "identity": key[0],
            "frame": key[1],
            "occurrence": key[2],
        }
        if rust_event is None:
            return {
                "status": "mismatch",
                "first_difference": {
                    "event": index,
                    "key": rendered_key,
                    "path": "$.events.key",
                    "cpp": rendered_key,
                    "rust": None,
                },
            }
        difference = _first_difference(cpp_event, rust_event, f"$.events[{index}]")
        if difference is not None:
            return {
                "status": "mismatch",
                "first_difference": {
                    "event": index,
                    "key": rendered_key,
                    **difference,
                },
            }
    if rust_canonical:
        key = next(iter(rust_canonical))
        rendered_key = {
            "identity": key[0],
            "frame": key[1],
            "occurrence": key[2],
        }
        return {
            "status": "mismatch",
            "first_difference": {
                "path": "$.events.key",
                "cpp": None,
                "rust": rendered_key,
            },
        }
    return {"status": "match", "first_difference": None}


def _read_trace(path: Path, side: str) -> dict[str, object]:
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise TraceError(f"cannot read {side} trace {path}: {error}") from error
    return validate_trace(document)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cpp-trace", type=Path, required=True, help="C++ CreatureGenerator v1 trace JSON")
    parser.add_argument("--rust-trace", type=Path, required=True, help="Rust CreatureGenerator v1 trace JSON")
    parser.add_argument("--report", type=Path, help="write JSON comparison report here (stdout otherwise)")
    arguments = parser.parse_args(argv)
    try:
        report = compare_traces(_read_trace(arguments.cpp_trace, "C++"), _read_trace(arguments.rust_trace, "Rust"))
        rendered = json.dumps(report, ensure_ascii=True, allow_nan=False, sort_keys=True, indent=2) + "\n"
        if arguments.report is None:
            print(rendered, end="")
        else:
            arguments.report.parent.mkdir(parents=True, exist_ok=True)
            temporary = arguments.report.with_suffix(arguments.report.suffix + ".tmp")
            temporary.write_text(rendered, encoding="utf-8")
            temporary.replace(arguments.report)
        return 0 if report["status"] == "match" else 1
    except TraceError as error:
        parser.error(str(error))
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
