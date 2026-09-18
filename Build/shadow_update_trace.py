#!/usr/bin/env python3
"""Compare bounded C++ and Rust ``ActorShadow.Update(None)`` trace artifacts.

This utility never launches a map or either runtime.  It consumes the side-specific
artifacts written by their renderer diagnostic hooks and reports the first semantic
mismatch without normalizing transforms or other values.
"""

from __future__ import annotations

import argparse
import json
import math
import re
import sys
from collections import defaultdict
from pathlib import Path
from typing import NoReturn

TRACE_VERSION = 1
ROOT_FIELDS = frozenset({"version", "enabled", "limit", "count", "truncated", "events"})
EVENT_FIELDS = frozenset(
    {
        "frame",
        "shadow_path",
        "owner_path",
        "pre",
        "post",
        "bone_name",
        "bone_pos",
        "owner",
        "mesh",
        "render_extent",
        "shadow",
        "decal",
        "attach_decal",
    }
)
TRANSFORM_FIELDS = frozenset({"location", "rotation"})
OWNER_FIELDS = TRANSFORM_FIELDS
MESH_FIELDS = frozenset({"path", "active_animation"})
ANIMATION_FIELDS = frozenset({"sequence", "frame", "rate", "finished"})
SHADOW_FIELDS = frozenset({"direction", "rotation"})
DECAL_FIELDS = frozenset({"trace_origin", "trace_direction", "trace_distance", "trace_hit"})
TRACE_HIT_FIELDS = frozenset({"kind", "surface_index"})
ATTACH_DECAL_FIELDS = frozenset({"outcome", "surface_count"})

_CPP_MAP_OWNER_PATH = re.compile(r"^Package[0-9]+\.(.+)$")


class TraceError(ValueError):
    """A supplied diagnostic artifact does not meet the shared trace schema."""


def _integer(value: object) -> bool:
    return isinstance(value, int) and not isinstance(value, bool)


def _number(value: object) -> bool:
    return isinstance(value, (int, float)) and not isinstance(value, bool) and math.isfinite(value)


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


def _path(value: object, path: str) -> None:
    if not isinstance(value, str) or not value:
        _problem(path, "must be a non-empty string")


def _vector(value: object, path: str) -> None:
    if not isinstance(value, list) or len(value) != 3:
        _problem(path, "must be a three-element JSON array")
    for index, component in enumerate(value):
        if not _number(component):
            _problem(f"{path}[{index}]", "must be a finite JSON number")


def _nullable_vector(value: object, path: str) -> None:
    if value is not None:
        _vector(value, path)


def _rotator(value: object, path: str) -> None:
    if not isinstance(value, list) or len(value) != 3:
        _problem(path, "must be a three-element JSON array")
    for index, component in enumerate(value):
        if not _integer(component):
            _problem(f"{path}[{index}]", "must be a JSON integer")


def _transform(value: object, path: str) -> None:
    transform = _object(value, path, TRANSFORM_FIELDS)
    _vector(transform["location"], f"{path}.location")
    _rotator(transform["rotation"], f"{path}.rotation")


def _animation(value: object, path: str) -> None:
    animation = _object(value, path, ANIMATION_FIELDS)
    _path(animation["sequence"], f"{path}.sequence")
    for field in ("frame", "rate"):
        if not _number(animation[field]):
            _problem(f"{path}.{field}", "must be a finite JSON number")
    if not isinstance(animation["finished"], bool):
        _problem(f"{path}.finished", "must be a boolean")
def _nullable_animation(value: object, path: str) -> None:
    if value is not None:
        _animation(value, path)




def _trace_hit(value: object, path: str) -> None:
    hit = _object(value, path, TRACE_HIT_FIELDS)
    if hit["kind"] not in {"surface", "miss", "not_attempted"}:
        _problem(f"{path}.kind", "must be 'surface', 'miss', or 'not_attempted'")
    surface_index = hit["surface_index"]
    if surface_index is not None and (not _integer(surface_index) or surface_index < 0):
        _problem(f"{path}.surface_index", "must be a non-negative JSON integer or null")
    if (hit["kind"] == "surface") != (surface_index is not None):
        _problem(f"{path}.surface_index", "must be present exactly for a surface hit")


def _attach_decal(value: object, path: str) -> None:
    outcome = _object(value, path, ATTACH_DECAL_FIELDS)
    if outcome["outcome"] not in {"attached", "miss", "skipped_owner", "skipped_threshold"}:
        _problem(f"{path}.outcome", "must be an AttachDecal outcome")
    if not _integer(outcome["surface_count"]) or outcome["surface_count"] < 0:
        _problem(f"{path}.surface_count", "must be a non-negative JSON integer")



def _event(event: object, index: int) -> None:
    path = f"$.events[{index}]"
    value = _object(event, path, EVENT_FIELDS)
    if not _integer(value["frame"]) or value["frame"] < 0:
        _problem(f"{path}.frame", "must be a non-negative JSON integer")
    _path(value["shadow_path"], f"{path}.shadow_path")
    if value["owner_path"] is not None:
        _path(value["owner_path"], f"{path}.owner_path")
    _transform(value["pre"], f"{path}.pre")
    _transform(value["post"], f"{path}.post")
    if value["bone_name"] is not None and not isinstance(value["bone_name"], str):
        _problem(f"{path}.bone_name", "must be a string or null")
    _nullable_vector(value["bone_pos"], f"{path}.bone_pos")
    if value["owner"] is None:
        if value["owner_path"] is not None:
            _problem(f"{path}.owner", "may be null only when owner_path is null")
    else:
        if value["owner_path"] is None:
            _problem(f"{path}.owner", "must be null when owner_path is null")
        owner = _object(value["owner"], f"{path}.owner", OWNER_FIELDS)
        _vector(owner["location"], f"{path}.owner.location")
        _rotator(owner["rotation"], f"{path}.owner.rotation")
    mesh = _object(value["mesh"], f"{path}.mesh", MESH_FIELDS)
    if mesh["path"] is not None:
        _path(mesh["path"], f"{path}.mesh.path")
    _nullable_animation(mesh["active_animation"], f"{path}.mesh.active_animation")
    if (mesh["active_animation"] is None) != (value["owner_path"] is None):
        _problem(f"{path}.mesh.active_animation", "is null exactly when owner_path is null")
    _nullable_vector(value["render_extent"], f"{path}.render_extent")
    if value["shadow"] is not None:
        shadow = _object(value["shadow"], f"{path}.shadow", SHADOW_FIELDS)
        _vector(shadow["direction"], f"{path}.shadow.direction")
        _rotator(shadow["rotation"], f"{path}.shadow.rotation")
    decal = _object(value["decal"], f"{path}.decal", DECAL_FIELDS)
    _nullable_vector(decal["trace_origin"], f"{path}.decal.trace_origin")
    _nullable_vector(decal["trace_direction"], f"{path}.decal.trace_direction")
    if decal["trace_distance"] is not None and not _number(decal["trace_distance"]):
        _problem(f"{path}.decal.trace_distance", "must be a finite JSON number or null")
    _trace_hit(decal["trace_hit"], f"{path}.decal.trace_hit")
    _attach_decal(value["attach_decal"], f"{path}.attach_decal")


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
    previous_frame = -1
    for index, event in enumerate(trace["events"]):
        _event(event, index)
        assert isinstance(event, dict)
        frame = event["frame"]
        assert isinstance(frame, int)
        if frame < previous_frame:
            _problem(f"$.events[{index}].frame", "must be in traversal frame order")
        previous_frame = frame
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


def _pairing_owner_path(
    owner_path: str | None, *, side: str, map_stem: str | None,
) -> str | None:
    """Return a map-root-neutral owner key without changing the artifact event."""
    if owner_path is None or map_stem is None:
        return owner_path
    prefix = f"{map_stem}."
    if owner_path.startswith(prefix):
        return f"<map>.{owner_path.removeprefix(prefix)}"
    if side == "cpp":
        match = _CPP_MAP_OWNER_PATH.fullmatch(owner_path)
        if match is not None:
            return f"<map>.{match.group(1)}"
    return owner_path


def _comparison_event(
    event: dict[str, object], *, side: str, map_stem: str | None,
) -> dict[str, object]:
    """Return a non-mutating projection with map-root object paths canonicalized."""
    normalized = event.copy()
    for field in ("owner_path", "shadow_path"):
        value = event[field]
        assert isinstance(value, str) or value is None
        normalized[field] = _pairing_owner_path(value, side=side, map_stem=map_stem)
    return normalized


def compare_traces(
    cpp_document: object, rust_document: object, *, map_stem: str | None = None,
) -> dict[str, object]:
    """Pair valid traces by (frame, owner path), preserving per-key traversal order.

    When supplied, ``map_stem`` normalizes selected-map C++
    ``Package<digits>.`` and Rust ``<map_stem>.`` object roots for pairing and
    field comparison. The input artifacts remain unmodified.
    """
    if map_stem is not None and (not isinstance(map_stem, str) or not map_stem):
        raise TraceError("map_stem must be a non-empty string or null")
    cpp = validate_trace(cpp_document)
    rust = validate_trace(rust_document)
    cpp_events = cpp["events"]
    rust_events = rust["events"]
    assert isinstance(cpp_events, list) and isinstance(rust_events, list)
    EventKey = tuple[int, str | None]
    rust_by_key: dict[EventKey, list[tuple[int, dict[str, object]]]] = defaultdict(list)
    for rust_index, event in enumerate(rust_events):
        assert isinstance(event, dict)
        frame = event["frame"]
        owner_path = event["owner_path"]
        assert isinstance(frame, int) and (isinstance(owner_path, str) or owner_path is None)
        key = (frame, _pairing_owner_path(owner_path, side="rust", map_stem=map_stem))
        rust_by_key[key].append((rust_index, event))
    consumed: dict[EventKey, int] = defaultdict(int)
    for cpp_index, cpp_event in enumerate(cpp_events):
        assert isinstance(cpp_event, dict)
        frame = cpp_event["frame"]
        owner_path = cpp_event["owner_path"]
        assert isinstance(frame, int) and (isinstance(owner_path, str) or owner_path is None)
        key = (frame, _pairing_owner_path(owner_path, side="cpp", map_stem=map_stem))
        rust_matches = rust_by_key.get(key, [])
        ordinal = consumed[key]
        if ordinal == len(rust_matches):
            return {
                "status": "mismatch",
                "first_difference": {
                    "path": "$.events",
                    "cpp": {"index": cpp_index, "key": list(key), "traversal": ordinal},
                    "rust": {"missing": True},
                },
            }
        rust_index, rust_event = rust_matches[ordinal]
        consumed[key] += 1
        difference = _first_difference(
            _comparison_event(cpp_event, side="cpp", map_stem=map_stem),
            _comparison_event(rust_event, side="rust", map_stem=map_stem),
            "$.event",
        )
        if difference is not None:
            return {
                "status": "mismatch",
                "first_difference": {
                    "cpp_index": cpp_index,
                    "rust_index": rust_index,
                    "key": {"frame": key[0], "owner_path": key[1], "traversal": ordinal},
                    **difference,
                },
            }
    seen: dict[EventKey, int] = defaultdict(int)
    for rust_index, rust_event in enumerate(rust_events):
        assert isinstance(rust_event, dict)
        frame = rust_event["frame"]
        owner_path = rust_event["owner_path"]
        assert isinstance(frame, int) and (isinstance(owner_path, str) or owner_path is None)
        key = (frame, _pairing_owner_path(owner_path, side="rust", map_stem=map_stem))
        ordinal = seen[key]
        seen[key] += 1
        if ordinal >= consumed[key]:
            return {
                "status": "mismatch",
                "first_difference": {
                    "path": "$.events",
                    "cpp": {"missing": True},
                    "rust": {"index": rust_index, "key": list(key), "traversal": ordinal},
                },
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
    parser.add_argument("--cpp-trace", type=Path, required=True, help="C++ shadow-update trace JSON")
    parser.add_argument("--rust-trace", type=Path, required=True, help="Rust shadow-update trace JSON")
    parser.add_argument(
        "--map-stem",
        help="opt in to selected-map owner-root pairing (for example, PrivetDr)",
    )
    parser.add_argument("--report", type=Path, help="write JSON comparison report to this path (stdout otherwise)")
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
