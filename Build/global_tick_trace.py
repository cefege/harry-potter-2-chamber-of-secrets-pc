#!/usr/bin/env python3
"""Validate and strictly compare bounded global actor Tick diagnostics.

The C++ oracle and hp2rs publish the same opt-in v2 JSON artifact. Events are
compared positionally, preserving actual recursive Tick method-entry order;
each entry carries its RNG interval and nested Tick/ProcessState observations.
This diagnostic never launches a game or evaluates actor code.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import NoReturn

TRACE_VERSION = 2
ROOT_FIELDS = frozenset({"version", "enabled", "limit", "count", "truncated", "events"})
EVENT_FIELDS = frozenset({
    "frame", "ordinal", "actor", "relation", "root_admission", "tick_stamp", "rng", "tick",
    "process_state",
})
ACTOR_FIELDS = frozenset({"path", "class"})
RNG_FIELDS = frozenset({"before", "after"})
TICK_FIELDS = frozenset({"identity", "outcome"})
PROCESS_STATE_FIELDS = frozenset({"identity", "outcome"})
RELATIONS = frozenset({"direct", "tick_parent", "tick_parent2", "owner"})
ROOT_ADMISSIONS = frozenset({"dynamic", "recursive"})
PROCESS_STATE_OUTCOMES = frozenset({
    "not_called", "dispatched", "skip_no_frame", "skip_no_code", "skip_role", "skip_pending_kill",
})
EVENT_COMPARISON_FIELDS = (
    "frame",
    "ordinal",
    "relation",
    "root_admission",
    "tick_stamp",
    "rng",
    "tick",
    "process_state",
)


class TraceError(ValueError):
    """A supplied diagnostic artifact does not meet the shared v2 schema."""


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


def _path(value: object, path: str) -> None:
    if not isinstance(value, str) or not value.startswith("<map>.") or len(value) == len("<map>."):
        _problem(path, "must be a canonical '<map>.' actor path")


def _event(value: object, index: int) -> tuple[int, int]:
    path = f"$.events[{index}]"
    event = _object(value, path, EVENT_FIELDS)
    for field in ("frame", "ordinal", "tick_stamp"):
        if not _integer(event[field]) or not 0 <= event[field] <= 0xFFFFFFFF:
            _problem(f"{path}.{field}", "must be an unsigned 32-bit JSON integer")
    actor = _object(event["actor"], f"{path}.actor", ACTOR_FIELDS)
    _path(actor["path"], f"{path}.actor.path")
    if not isinstance(actor["class"], str) or not actor["class"]:
        _problem(f"{path}.actor.class", "must be a non-empty class path")
    if event["relation"] not in RELATIONS:
        _problem(f"{path}.relation", "must be direct, tick_parent, tick_parent2, or owner")
    if event["root_admission"] not in ROOT_ADMISSIONS:
        _problem(f"{path}.root_admission", "must be dynamic or recursive")
    if event["relation"] == "direct":
        if event["root_admission"] != "dynamic":
            _problem(f"{path}.root_admission", "direct dispatch must use dynamic root admission")
    elif event["root_admission"] != "recursive":
        _problem(f"{path}.root_admission", "recursive dispatch must use recursive admission")
    rng = _object(event["rng"], f"{path}.rng", RNG_FIELDS)
    for field in ("before", "after"):
        if not _integer(rng[field]) or not 0 <= rng[field] <= 0xFFFFFFFFFFFFFFFF:
            _problem(f"{path}.rng.{field}", "must be an unsigned 64-bit JSON integer")
    if rng["after"] < rng["before"]:
        _problem(f"{path}.rng.after", "cannot precede rng.before")
    tick = _object(event["tick"], f"{path}.tick", TICK_FIELDS)
    if tick["identity"] != "Tick":
        _problem(f"{path}.tick.identity", "must equal 'Tick'")
    tick_outcome = tick["outcome"]
    if not isinstance(tick_outcome, str) or (
        tick_outcome != "dispatched" and not tick_outcome.startswith("skip_")
    ):
        _problem(
            f"{path}.tick.outcome",
            "must be 'dispatched' or a non-empty 'skip_' reason",
        )
    process_state = _object(event["process_state"], f"{path}.process_state", PROCESS_STATE_FIELDS)
    if not isinstance(process_state["identity"], str) or not process_state["identity"]:
        _problem(f"{path}.process_state.identity", "must be a non-empty state-node identity")
    if process_state["outcome"] not in PROCESS_STATE_OUTCOMES:
        _problem(
            f"{path}.process_state.outcome",
            "must be a supported ProcessState dispatch or skip outcome",
        )
    return event["frame"], event["ordinal"]  # type: ignore[return-value]


def validate_trace(document: object) -> dict[str, object]:
    """Return a validated trace or raise ``TraceError`` at its precise path."""
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
    events = trace["events"]
    if not isinstance(events, list):
        _problem("$.events", "must be an array")
    if trace["count"] != len(events):
        _problem("$.count", "must equal the events array length")
    if trace["count"] > trace["limit"]:
        _problem("$.count", "cannot exceed limit")
    if trace["truncated"] and trace["count"] != trace["limit"]:
        _problem("$.truncated", "requires count to equal limit")
    prior: tuple[int, int] | None = None
    for index, event in enumerate(events):
        key = _event(event, index)
        if prior is not None and key <= prior:
            _problem(f"$.events[{index}]", "must be in strictly increasing frame/ordinal dispatch order")
        prior = key
    return trace


def _first_difference(cpp: object, rust: object, path: str) -> dict[str, object] | None:
    if type(cpp) is not type(rust):
        return {"path": path, "cpp": cpp, "rust": rust}
    if isinstance(cpp, dict):
        assert isinstance(rust, dict)
        for field in cpp:
            difference = _first_difference(cpp[field], rust[field], f"{path}.{field}")
            if difference is not None:
                return difference
        return None
    if cpp != rust:
        return {"path": path, "cpp": cpp, "rust": rust}
    return None


def _actor_difference(cpp: object, rust: object, path: str) -> dict[str, object] | None:
    assert isinstance(cpp, dict) and isinstance(rust, dict)
    for field in ("path", "class"):
        difference = _first_difference(cpp[field], rust[field], f"{path}.{field}")
        if difference is not None:
            return difference
    return None


def _event_difference(
    cpp: dict[str, object], rust: dict[str, object], path: str,
) -> dict[str, object] | None:
    for field in EVENT_COMPARISON_FIELDS[:2]:
        difference = _first_difference(cpp[field], rust[field], f"{path}.{field}")
        if difference is not None:
            return difference
    difference = _actor_difference(cpp["actor"], rust["actor"], f"{path}.actor")
    if difference is not None:
        return difference
    for field in EVENT_COMPARISON_FIELDS[2:]:
        difference = _first_difference(cpp[field], rust[field], f"{path}.{field}")
        if difference is not None:
            return difference
    return None


def _event_key(event: dict[str, object], occurrence: int) -> tuple[int, str, str, int]:
    actor = event["actor"]
    assert isinstance(actor, dict)
    frame = event["frame"]
    actor_path = actor["path"]
    class_path = actor["class"]
    assert isinstance(frame, int) and isinstance(actor_path, str) and isinstance(class_path, str)
    return frame, actor_path, class_path, occurrence


def _render_key(key: tuple[int, str, str, int]) -> dict[str, object]:
    return {"frame": key[0], "actor": {"path": key[1], "class": key[2]}, "occurrence": key[3]}


def compare_traces(cpp_document: object, rust_document: object) -> dict[str, object]:
    """Compare every global dispatch position and report the first mismatch."""
    cpp = validate_trace(cpp_document)
    rust = validate_trace(rust_document)
    for field in ("version", "enabled", "limit", "truncated"):
        difference = _first_difference(cpp[field], rust[field], f"$.{field}")
        if difference is not None:
            return {"status": "mismatch", "first_difference": difference}
    cpp_events = cpp["events"]
    rust_events = rust["events"]
    assert isinstance(cpp_events, list) and isinstance(rust_events, list)
    occurrences: dict[tuple[int, str, str], int] = {}
    for index, (cpp_event, rust_event) in enumerate(zip(cpp_events, rust_events, strict=False)):
        assert isinstance(cpp_event, dict) and isinstance(rust_event, dict)
        actor = cpp_event["actor"]
        assert isinstance(actor, dict)
        frame = cpp_event["frame"]
        actor_path = actor["path"]
        class_path = actor["class"]
        assert isinstance(frame, int) and isinstance(actor_path, str) and isinstance(class_path, str)
        base = frame, actor_path, class_path
        occurrence = occurrences.get(base, 0)
        occurrences[base] = occurrence + 1
        difference = _event_difference(cpp_event, rust_event, f"$.events[{index}]")
        if difference is not None:
            return {
                "status": "mismatch",
                "first_difference": {
                    "event": index,
                    "key": _render_key(_event_key(cpp_event, occurrence)),
                    **difference,
                },
            }
    if len(cpp_events) != len(rust_events):
        index = min(len(cpp_events), len(rust_events))
        return {
            "status": "mismatch",
            "first_difference": {
                "event": index,
                "path": "$.events.length",
                "cpp": len(cpp_events),
                "rust": len(rust_events),
            },
        }
    return {"status": "match", "first_difference": None}


def _read_trace(path: Path, side: str) -> dict[str, object]:
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise TraceError(f"cannot read {side} trace {path}: {error}") from error
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
    parser.add_argument("--cpp-trace", type=Path, required=True, help="C++ global Tick v1 trace JSON")
    parser.add_argument("--rust-trace", type=Path, required=True, help="Rust global Tick v1 trace JSON")
    parser.add_argument("--report", type=Path, help="write JSON comparison report here (stdout otherwise)")
    arguments = parser.parse_args(argv)
    try:
        report = compare_traces(_read_trace(arguments.cpp_trace, "C++"), _read_trace(arguments.rust_trace, "Rust"))
        _write_report(arguments.report, report)
        return 0 if report["status"] == "match" else 1
    except TraceError as error:
        parser.error(str(error))
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
