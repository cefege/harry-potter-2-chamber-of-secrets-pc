#!/usr/bin/env python3
"""Validate and compare bounded C++ and Rust actor-transition ledgers.

The reporters are diagnostic-only. Each record is keyed by a normalized map
actor identity and its ``first_observed_actor`` captures lifecycle, spawn, or
renderer observation—not serialized ``ULevel::Actors`` membership—so comparison
joins owners by identity rather than incidental allocation or traversal order.
It canonicalizes only map roots proven by matching observed actor paths;
external package paths and classes remain literal. The first absent owner or
differing lifecycle transition is reported.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import NoReturn

TRACE_VERSION = 2
ROOT_FIELDS = ("version", "enabled", "limit", "count", "truncated", "owners")
OWNER_FIELDS = (
    "identity",
    "first_observed_actor",
    "post_init_execution",
    "pre_begin_before",
    "pre_begin_after",
    "spawn",
    "shadow_post_continuation",
    "first_renderer_candidate",
)
SLOT_FIELDS = ("path", "class")
SNAPSHOT_FIELDS = (
    "active_slot",
    "delete_marked",
    "pending_kill",
    "effective_shadow_class",
    "shadow_class_cdo",
)
SPAWN_FIELDS = ("request", "result", "published_child_slot")
SPAWN_REQUEST_FIELDS = ("class", "name", "owner")


class TraceError(ValueError):
    """A supplied artifact does not meet the shared transition-ledger schema."""


def _problem(path: str, detail: str) -> NoReturn:
    raise TraceError(f"{path}: {detail}")


def _integer(value: object) -> bool:
    return isinstance(value, int) and not isinstance(value, bool)


def _object(value: object, path: str, fields: tuple[str, ...]) -> dict[str, object]:
    if not isinstance(value, dict):
        _problem(path, "must be an object")
    actual = set(value)
    expected = set(fields)
    if actual != expected:
        missing = [field for field in fields if field not in actual]
        unknown = sorted(actual - expected)
        if missing:
            _problem(path, f"missing required field {missing[0]!r}")
        _problem(path, f"contains unknown field {unknown[0]!r}")
    return value


def _nullable_path(value: object, path: str) -> None:
    if value is not None and (not isinstance(value, str) or not value):
        _problem(path, "must be a non-empty string or null")


def _slot(value: object, path: str) -> None:
    slot = _object(value, path, SLOT_FIELDS)
    _nullable_path(slot["path"], f"{path}.path")
    _nullable_path(slot["class"], f"{path}.class")


def _nullable_slot(value: object, path: str) -> None:
    if value is not None:
        _slot(value, path)


def _snapshot(value: object, path: str) -> None:
    snapshot = _object(value, path, SNAPSHOT_FIELDS)
    _slot(snapshot["active_slot"], f"{path}.active_slot")
    for field in ("delete_marked", "pending_kill"):
        if not isinstance(snapshot[field], bool):
            _problem(f"{path}.{field}", "must be a boolean")
    for field in ("effective_shadow_class", "shadow_class_cdo"):
        _nullable_path(snapshot[field], f"{path}.{field}")


def _spawn_request(value: object, path: str) -> None:
    request = _object(value, path, SPAWN_REQUEST_FIELDS)
    for field in SPAWN_REQUEST_FIELDS:
        _nullable_path(request[field], f"{path}.{field}")


def _spawn(value: object, path: str) -> None:
    spawn = _object(value, path, SPAWN_FIELDS)
    if spawn["request"] is not None:
        _spawn_request(spawn["request"], f"{path}.request")
    _nullable_slot(spawn["result"], f"{path}.result")
    _nullable_slot(spawn["published_child_slot"], f"{path}.published_child_slot")


def _owner(value: object, index: int) -> None:
    path = f"$.owners[{index}]"
    owner = _object(value, path, OWNER_FIELDS)
    identity = owner["identity"]
    if not isinstance(identity, str) or not identity.startswith("<map>.") or len(identity) == len("<map>."):
        _problem(f"{path}.identity", "must be a normalized '<map>.' actor identity")
    _slot(owner["first_observed_actor"], f"{path}.first_observed_actor")
    for field in ("post_init_execution", "pre_begin_before", "pre_begin_after"):
        _snapshot(owner[field], f"{path}.{field}")
    _spawn(owner["spawn"], f"{path}.spawn")
    for field in ("shadow_post_continuation", "first_renderer_candidate"):
        _nullable_path(owner[field], f"{path}.{field}")


def validate_trace(document: object) -> dict[str, object]:
    """Return a validated ledger or raise ``TraceError`` at the precise field."""
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
    owners = trace["owners"]
    if not isinstance(owners, list):
        _problem("$.owners", "must be an array")
    if trace["count"] != len(owners):
        _problem("$.count", "must equal the owners array length")
    if trace["count"] > trace["limit"]:
        _problem("$.count", "cannot exceed limit")
    if trace["truncated"] and trace["count"] != trace["limit"]:
        _problem("$.truncated", "requires count to equal limit")
    identities: set[str] = set()
    for index, owner in enumerate(owners):
        _owner(owner, index)
        assert isinstance(owner, dict)
        identity = owner["identity"]
        assert isinstance(identity, str)
        if identity in identities:
            _problem(f"$.owners[{index}].identity", "must be unique")
        identities.add(identity)
    return trace


def _first_difference(cpp: object, rust: object, path: str, fields: tuple[str, ...] | None = None) -> dict[str, object] | None:
    if type(cpp) is not type(rust):
        return {"path": path, "cpp": cpp, "rust": rust}
    if isinstance(cpp, dict):
        assert isinstance(rust, dict)
        keys = fields if fields is not None else tuple(cpp)
        for key in keys:
            difference = _first_difference(cpp[key], rust[key], f"{path}.{key}")
            if difference is not None:
                return difference
        return None
    if cpp != rust:
        return {"path": path, "cpp": cpp, "rust": rust}
    return None


def _map_roots(owners: list[object]) -> frozenset[str]:
    """Infer package roots only from matching observed actor paths."""
    roots: set[str] = set()
    for owner in owners:
        assert isinstance(owner, dict)
        identity = owner["identity"]
        first_observed_actor = owner["first_observed_actor"]
        assert isinstance(identity, str) and isinstance(first_observed_actor, dict)
        path = first_observed_actor["path"]
        assert isinstance(path, str) or path is None
        if path is None or "." not in path:
            continue
        root, suffix = path.split(".", 1)
        if suffix == identity.removeprefix("<map>."):
            roots.add(root)
    return frozenset(roots)


def _canonical_map_path(value: object, roots: frozenset[str]) -> object:
    if not isinstance(value, str) or "." not in value:
        return value
    root, suffix = value.split(".", 1)
    return f"<map>.{suffix}" if root in roots else value


def _comparison_slot(slot: object, roots: frozenset[str]) -> object:
    if slot is None:
        return None
    assert isinstance(slot, dict)
    projection = slot.copy()
    projection["path"] = _canonical_map_path(slot["path"], roots)
    return projection


def _comparison_snapshot(snapshot: object, roots: frozenset[str]) -> dict[str, object]:
    assert isinstance(snapshot, dict)
    projection = snapshot.copy()
    projection["active_slot"] = _comparison_slot(snapshot["active_slot"], roots)
    return projection


def _comparison_owner(owner: dict[str, object], roots: frozenset[str]) -> dict[str, object]:
    """Normalize only selected-map object paths; classes/package references stay raw."""
    projection = owner.copy()
    projection["first_observed_actor"] = _comparison_slot(owner["first_observed_actor"], roots)
    for field in ("post_init_execution", "pre_begin_before", "pre_begin_after"):
        projection[field] = _comparison_snapshot(owner[field], roots)
    spawn = owner["spawn"]
    assert isinstance(spawn, dict)
    spawn_projection = spawn.copy()
    request = spawn["request"]
    if request is not None:
        assert isinstance(request, dict)
        request_projection = request.copy()
        request_projection["owner"] = _canonical_map_path(request["owner"], roots)
        spawn_projection["request"] = request_projection
    for field in ("result", "published_child_slot"):
        spawn_projection[field] = _comparison_slot(spawn[field], roots)
    projection["spawn"] = spawn_projection
    for field in ("shadow_post_continuation", "first_renderer_candidate"):
        projection[field] = _canonical_map_path(owner[field], roots)
    return projection


def _slot_difference(cpp: object, rust: object, path: str) -> dict[str, object] | None:
    return _first_difference(cpp, rust, path, SLOT_FIELDS)


def _nullable_slot_difference(cpp: object, rust: object, path: str) -> dict[str, object] | None:
    if cpp is None or rust is None:
        return _first_difference(cpp, rust, path)
    return _slot_difference(cpp, rust, path)


def _snapshot_difference(cpp: dict[str, object], rust: dict[str, object], path: str) -> dict[str, object] | None:
    for field in SNAPSHOT_FIELDS:
        field_path = f"{path}.{field}"
        difference = (
            _slot_difference(cpp[field], rust[field], field_path)
            if field == "active_slot"
            else _first_difference(cpp[field], rust[field], field_path)
        )
        if difference is not None:
            return difference
    return None


def _spawn_difference(cpp: dict[str, object], rust: dict[str, object], path: str) -> dict[str, object] | None:
    cpp_request = cpp["request"]
    rust_request = rust["request"]
    if cpp_request is None or rust_request is None:
        difference = _first_difference(cpp_request, rust_request, f"{path}.request")
    else:
        difference = _first_difference(
            cpp_request, rust_request, f"{path}.request", SPAWN_REQUEST_FIELDS,
        )
    if difference is not None:
        return difference
    for field in ("result", "published_child_slot"):
        difference = _nullable_slot_difference(cpp[field], rust[field], f"{path}.{field}")
        if difference is not None:
            return difference
    return None


def _owner_difference(cpp: dict[str, object], rust: dict[str, object]) -> dict[str, object] | None:
    for field in OWNER_FIELDS:
        if field == "identity":
            continue
        path = f"$.owner.{field}"
        if field == "first_observed_actor":
            difference = _slot_difference(cpp[field], rust[field], path)
        elif field in ("post_init_execution", "pre_begin_before", "pre_begin_after"):
            assert isinstance(cpp[field], dict) and isinstance(rust[field], dict)
            difference = _snapshot_difference(cpp[field], rust[field], path)
        elif field == "spawn":
            assert isinstance(cpp[field], dict) and isinstance(rust[field], dict)
            difference = _spawn_difference(cpp[field], rust[field], path)
        else:
            difference = _first_difference(cpp[field], rust[field], path)
        if difference is not None:
            return difference
    return None


def compare_traces(cpp_document: object, rust_document: object) -> dict[str, object]:
    """Join validated ledgers by normalized owner identity and find the first loss."""
    cpp = validate_trace(cpp_document)
    rust = validate_trace(rust_document)
    for field in ("limit", "truncated"):
        if cpp[field] != rust[field]:
            return {
                "status": "mismatch",
                "first_difference": {"path": f"$.{field}", "cpp": cpp[field], "rust": rust[field]},
            }
    cpp_owners = cpp["owners"]
    rust_owners = rust["owners"]
    assert isinstance(cpp_owners, list) and isinstance(rust_owners, list)
    cpp_map_roots = _map_roots(cpp_owners)
    rust_map_roots = _map_roots(rust_owners)
    rust_by_identity = {
        owner["identity"]: (index, owner)
        for index, owner in enumerate(rust_owners)
        if isinstance(owner, dict)
    }
    joined: set[str] = set()
    for cpp_index, cpp_owner in enumerate(cpp_owners):
        assert isinstance(cpp_owner, dict)
        identity = cpp_owner["identity"]
        assert isinstance(identity, str)
        rust_match = rust_by_identity.get(identity)
        if rust_match is None:
            return {
                "status": "mismatch",
                "first_difference": {
                    "path": "$.owners",
                    "cpp": {"index": cpp_index, "identity": identity},
                    "rust": {"missing": True},
                },
            }
        rust_index, rust_owner = rust_match
        assert isinstance(rust_owner, dict)
        joined.add(identity)
        difference = _owner_difference(
            _comparison_owner(cpp_owner, cpp_map_roots),
            _comparison_owner(rust_owner, rust_map_roots),
        )
        if difference is not None:
            return {
                "status": "mismatch",
                "first_difference": {
                    "cpp_index": cpp_index,
                    "rust_index": rust_index,
                    "identity": identity,
                    **difference,
                },
            }
    for rust_index, rust_owner in enumerate(rust_owners):
        assert isinstance(rust_owner, dict)
        identity = rust_owner["identity"]
        assert isinstance(identity, str)
        if identity not in joined:
            return {
                "status": "mismatch",
                "first_difference": {
                    "path": "$.owners",
                    "cpp": {"missing": True},
                    "rust": {"index": rust_index, "identity": identity},
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
    rendered = json.dumps(report, ensure_ascii=True, allow_nan=False, indent=2, sort_keys=True) + "\n"
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
    parser.add_argument("--cpp-trace", type=Path, required=True, help="C++ transition-ledger JSON")
    parser.add_argument("--rust-trace", type=Path, required=True, help="Rust transition-ledger JSON")
    parser.add_argument("--report", type=Path, help="write JSON comparison report to this path (stdout otherwise)")
    arguments = parser.parse_args(argv)
    try:
        report = compare_traces(
            _read_trace(arguments.cpp_trace, "C++"),
            _read_trace(arguments.rust_trace, "Rust"),
        )
        _write_report(arguments.report, report)
    except TraceError as error:
        parser.error(str(error))
    return 0 if report["status"] == "match" else 1


if __name__ == "__main__":
    raise SystemExit(main())
