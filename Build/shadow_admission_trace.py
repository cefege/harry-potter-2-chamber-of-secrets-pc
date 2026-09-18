#!/usr/bin/env python3
"""Compare bounded C++ and Rust ActorShadow admission-decision trace artifacts.

This utility never launches a map or either runtime. It validates the opt-in,
side-specific JSON artifacts and reports the first semantic decision difference.
"""

from __future__ import annotations

import argparse
import json
import math
import re
from collections import defaultdict
from pathlib import Path
from typing import NoReturn

TRACE_VERSION = 1
ROOT_FIELDS = frozenset({"version", "enabled", "limit", "count", "truncated", "events"})
EVENT_FIELDS = frozenset(
    {
        "frame",
        "pass",
        "shadow_path",
        "owner_path",
        "candidate_status",
        "raw_facts",
        "update_eligible",
    },
)
RAW_FACT_FIELDS = frozenset(
    {
        "viewport_actor_path",
        "view_target_path",
        "behind_view",
        "recursion_parent_path",
        "perspective",
        "world_dynamics",
    },
)
CANDIDATE_STATUSES = frozenset(
    {
        "missing_shadow",
        "rejected_missing_location",
        "rejected_ortho",
        "rejected_behind_projection",
        "rejected_bounds",
        "admitted",
    },
)
_PAIRING_EXCLUDED_STATUSES = frozenset({"missing_shadow", "rejected_missing_location"})
_CPP_MAP_PATH = re.compile(r"^(Package[0-9]+)\.(.+)$")


class TraceError(ValueError):
    """A supplied diagnostic artifact does not meet the shared trace schema."""


def _integer(value: object) -> bool:
    return isinstance(value, int) and not isinstance(value, bool)


def _problem(path: str, detail: str) -> NoReturn:
    raise TraceError(f"{path}: {detail}")


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


def _path(value: object, path: str, *, nullable: bool = False) -> None:
    if nullable and value is None:
        return
    if not isinstance(value, str) or not value:
        _problem(path, "must be a non-empty string" + (" or null" if nullable else ""))


def _raw_facts(value: object, path: str) -> None:
    facts = _object(value, path, RAW_FACT_FIELDS)
    for field in ("viewport_actor_path", "view_target_path", "recursion_parent_path"):
        _path(facts[field], f"{path}.{field}", nullable=True)
    for field in ("behind_view", "perspective", "world_dynamics"):
        if not isinstance(facts[field], bool):
            _problem(f"{path}.{field}", "must be a boolean")


def _event(event: object, index: int) -> None:
    path = f"$.events[{index}]"
    value = _object(event, path, EVENT_FIELDS)
    for field in ("frame", "pass"):
        if not _integer(value[field]) or value[field] < 0:
            _problem(f"{path}.{field}", "must be a non-negative JSON integer")
    _path(value["shadow_path"], f"{path}.shadow_path", nullable=True)
    _path(value["owner_path"], f"{path}.owner_path")
    if value["candidate_status"] not in CANDIDATE_STATUSES:
        _problem(
            f"{path}.candidate_status",
            "must be one of " + ", ".join(sorted(repr(status) for status in CANDIDATE_STATUSES)),
        )
    _raw_facts(value["raw_facts"], f"{path}.raw_facts")
    if not isinstance(value["update_eligible"], bool):
        _problem(f"{path}.update_eligible", "must be a boolean")


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
    previous_identity = (-1, -1)
    for index, event in enumerate(trace["events"]):
        _event(event, index)
        assert isinstance(event, dict)
        identity = (event["frame"], event["pass"])
        assert isinstance(identity[0], int) and isinstance(identity[1], int)
        if identity < previous_identity:
            _problem(f"$.events[{index}]", "must be in frame/pass traversal order")
        previous_identity = identity
    return trace


def _first_difference(cpp: object, rust: object, path: str = "$") -> dict[str, object] | None:
    if type(cpp) is not type(rust):
        return {"path": path, "cpp": cpp, "rust": rust}
    if isinstance(cpp, dict):
        for key in cpp:
            child = f"{path}.{key}"
            if key not in rust:
                return {"path": child, "cpp": cpp[key], "rust": {"missing": True}}
            difference = _first_difference(cpp[key], rust[key], child)
            if difference is not None:
                return difference
        for key in rust:
            if key not in cpp:
                return {"path": f"{path}.{key}", "cpp": {"missing": True}, "rust": rust[key]}
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


def _canonical_map_path(
    path: str | None,
    *,
    side: str,
    map_stem: str | None,
    cpp_map_roots: frozenset[str] = frozenset(),
) -> str | None:
    if path is None or map_stem is None:
        return path
    prefix = f"{map_stem}."
    if path.startswith(prefix):
        return f"<map>.{path.removeprefix(prefix)}"
    if side == "cpp":
        match = _CPP_MAP_PATH.fullmatch(path)
        if match is not None and match.group(1) in cpp_map_roots:
            return f"<map>.{match.group(2)}"
    return path


def _map_root_packages(
    cpp_events: list[object], rust_events: list[object], map_stem: str | None,
) -> frozenset[str]:
    """Identify only C++ package roots proven to be the selected map."""
    if map_stem is None:
        return frozenset()
    rust_prefix = f"{map_stem}."
    rust_suffixes = {
        owner_path.removeprefix(rust_prefix)
        for event in rust_events
        if isinstance(event, dict)
        for owner_path in (event["owner_path"],)
        if isinstance(owner_path, str) and owner_path.startswith(rust_prefix)
    }
    if not rust_suffixes:
        return frozenset(
            match.group(1)
            for event in cpp_events
            if isinstance(event, dict)
            for owner_path in (event["owner_path"],)
            if isinstance(owner_path, str)
            for match in (_CPP_MAP_PATH.fullmatch(owner_path),)
            if match is not None
        )
    return frozenset(
        match.group(1)
        for event in cpp_events
        if isinstance(event, dict)
        for owner_path in (event["owner_path"],)
        if isinstance(owner_path, str)
        for match in (_CPP_MAP_PATH.fullmatch(owner_path),)
        if match is not None and match.group(2) in rust_suffixes
    )


def _comparison_event(
    event: dict[str, object],
    *,
    side: str,
    map_stem: str | None,
    cpp_map_roots: frozenset[str],
    rust_frame_offset: int,
) -> dict[str, object]:
    """Return a non-mutating diagnostic projection for an already paired event."""
    normalized = event.copy()
    if side == "rust":
        frame = event["frame"]
        assert isinstance(frame, int)
        normalized["frame"] = frame - rust_frame_offset
    for field in ("shadow_path", "owner_path"):
        value = event[field]
        assert isinstance(value, str) or value is None
        normalized[field] = _canonical_map_path(
            value, side=side, map_stem=map_stem, cpp_map_roots=cpp_map_roots,
        )
    raw_facts = event["raw_facts"]
    assert isinstance(raw_facts, dict)
    normalized_facts = raw_facts.copy()
    for field in ("viewport_actor_path", "view_target_path", "recursion_parent_path"):
        value = raw_facts[field]
        assert isinstance(value, str) or value is None
        normalized_facts[field] = _canonical_map_path(
            value, side=side, map_stem=map_stem, cpp_map_roots=cpp_map_roots,
        )
    normalized["raw_facts"] = normalized_facts
    return normalized


def _candidate_projection(
    events: list[object],
    *,
    side: str,
    map_stem: str | None,
    cpp_map_roots: frozenset[str],
) -> list[tuple[int, dict[str, object], tuple[str, int]]]:
    """Select comparable shadow decisions and ordinal each canonical owner."""
    occurrences: dict[str, int] = defaultdict(int)
    projection: list[tuple[int, dict[str, object], tuple[str, int]]] = []
    for index, event in enumerate(events):
        assert isinstance(event, dict)
        status = event["candidate_status"]
        owner_path = event["owner_path"]
        assert isinstance(status, str) and isinstance(owner_path, str)
        if status in _PAIRING_EXCLUDED_STATUSES:
            continue
        owner = _canonical_map_path(
            owner_path, side=side, map_stem=map_stem, cpp_map_roots=cpp_map_roots,
        )
        assert isinstance(owner, str)
        occurrence = occurrences[owner]
        occurrences[owner] += 1
        projection.append((index, event, (owner, occurrence)))
    return projection


def _legacy_event_key(event: dict[str, object], owner_path: str) -> list[object]:
    frame, render_pass = event["frame"], event["pass"]
    assert isinstance(frame, int) and isinstance(render_pass, int)
    return [frame, render_pass, owner_path]


def _legacy_matched_key(event: dict[str, object], key: tuple[str, int]) -> dict[str, object]:
    frame, render_pass = event["frame"], event["pass"]
    assert isinstance(frame, int) and isinstance(render_pass, int)
    return {
        "frame": frame,
        "pass": render_pass,
        "owner_path": key[0],
        "traversal": key[1],
    }


def compare_traces(
    cpp_document: object, rust_document: object, *, map_stem: str | None = None,
) -> dict[str, object]:
    """Compare real shadow candidates by canonical owner and occurrence ordinal.

    ``map_stem`` enables selected-map path canonicalization. A C++ package root is
    canonicalized only when its owner suffix proves it corresponds to the selected
    Rust map; external package paths remain literal. ``missing_shadow`` and
    ``rejected_missing_location`` records stay in the artifacts but do not affect
    pairing. If every joined candidate has one Rust-minus-C++ frame delta, that
    delta is applied only to the comparison projection.
    """
    if map_stem is not None and (not isinstance(map_stem, str) or not map_stem):
        raise TraceError("map_stem must be a non-empty string or null")
    cpp = validate_trace(cpp_document)
    rust = validate_trace(rust_document)
    cpp_events = cpp["events"]
    rust_events = rust["events"]
    assert isinstance(cpp_events, list) and isinstance(rust_events, list)
    cpp_map_roots = _map_root_packages(cpp_events, rust_events, map_stem)
    cpp_candidates = _candidate_projection(
        cpp_events, side="cpp", map_stem=map_stem, cpp_map_roots=cpp_map_roots,
    )
    rust_candidates = _candidate_projection(
        rust_events, side="rust", map_stem=map_stem, cpp_map_roots=cpp_map_roots,
    )
    rust_by_key = {key: (index, event) for index, event, key in rust_candidates}
    pairs: list[tuple[int, dict[str, object], int, dict[str, object], tuple[str, int]]] = []
    matched_keys: set[tuple[str, int]] = set()
    for cpp_index, cpp_event, key in cpp_candidates:
        rust_match = rust_by_key.get(key)
        if rust_match is None:
            return {
                "status": "mismatch",
                "first_difference": {
                    "path": "$.events",
                    "cpp": {
                        "index": cpp_index,
                        "key": _legacy_event_key(cpp_event, key[0]),
                        "traversal": key[1],
                    },
                    "rust": {"missing": True},
                    "occurrence": key[1],
                },
            }
        rust_index, rust_event = rust_match
        matched_keys.add(key)
        pairs.append((cpp_index, cpp_event, rust_index, rust_event, key))
    for rust_index, rust_event, key in rust_candidates:
        if key not in matched_keys:
            return {
                "status": "mismatch",
                "first_difference": {
                    "path": "$.events",
                    "cpp": {"missing": True},
                    "rust": {
                        "index": rust_index,
                        "key": _legacy_event_key(rust_event, key[0]),
                        "traversal": key[1],
                    },
                    "occurrence": key[1],
                },
            }
    frame_offsets = {
        rust_event["frame"] - cpp_event["frame"]
        for _cpp_index, cpp_event, _rust_index, rust_event, _key in pairs
        if isinstance(cpp_event["frame"], int) and isinstance(rust_event["frame"], int)
    }
    inferred_offset = len(frame_offsets) == 1
    rust_frame_offset = next(iter(frame_offsets)) if inferred_offset else 0
    for cpp_index, cpp_event, rust_index, rust_event, key in pairs:
        difference = _first_difference(
            _comparison_event(
                cpp_event,
                side="cpp",
                map_stem=map_stem,
                cpp_map_roots=cpp_map_roots,
                rust_frame_offset=rust_frame_offset,
            ),
            _comparison_event(
                rust_event,
                side="rust",
                map_stem=map_stem,
                cpp_map_roots=cpp_map_roots,
                rust_frame_offset=rust_frame_offset,
            ),
            "$.event",
        )
        if difference is not None:
            first_difference = {
                "cpp_index": cpp_index,
                "rust_index": rust_index,
                "key": _legacy_matched_key(cpp_event, key),
                "occurrence": key[1],
                **difference,
            }
            if inferred_offset:
                first_difference["rust_frame_offset"] = rust_frame_offset
            return {"status": "mismatch", "first_difference": first_difference}
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
    rendered = json.dumps(report, indent=2, sort_keys=True) + "\n"
    if path is None:
        print(rendered, end="")
        return
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(rendered, encoding="utf-8")
    except OSError as error:
        raise TraceError(f"cannot write report {path}: {error}") from error


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cpp-trace", type=Path, required=True, help="C++ shadow-admission trace JSON")
    parser.add_argument("--rust-trace", type=Path, required=True, help="Rust shadow-admission trace JSON")
    parser.add_argument("--map-stem", help="opt in to selected-map root canonicalization (for example, PrivetDr)")
    parser.add_argument("--report", type=Path, help="write JSON comparison report here (stdout otherwise)")
    arguments = parser.parse_args(argv)
    try:
        report = compare_traces(
            _read_trace(arguments.cpp_trace, "C++"),
            _read_trace(arguments.rust_trace, "Rust"),
            map_stem=arguments.map_stem,
        )
        _write_report(arguments.report, report)
    except TraceError as error:
        parser.error(str(error))
    return 0 if report["status"] == "match" else 1


if __name__ == "__main__":
    raise SystemExit(main())
