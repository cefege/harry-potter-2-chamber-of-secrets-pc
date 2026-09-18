#!/usr/bin/env python3
"""Validate and compare opt-in Fire1 runtime trace artifacts."""
from __future__ import annotations

import argparse
import json
import re
from pathlib import Path
from typing import NoReturn

TRACE_FORMAT = "hp2-fire-texture-trace"
TRACE_VERSION = 1
ARCHIVE_SHA256 = "78dde55622484c313adb88b87d54b8aa2f502982d7200909b1b7edd1d475845b"
ROOT_FIELDS = frozenset({"format", "version", "enabled", "target", "init_tables", "first_speed_rand", "burn_mip0_writes", "lifecycle"})
TARGET_FIELDS = frozenset({"archive", "export", "class", "dimensions", "active_spark_count"})
ARCHIVE_FIELDS = frozenset({"path", "sha256"})
EXPORT_FIELDS = frozenset({"index", "path"})
DIMENSION_FIELDS = frozenset({"u", "v"})
INIT_FIELDS = frozenset({"rng", "fingerprint"})
RNG_FIELDS = frozenset({"before", "after"})
SPEED_FIELDS = frozenset({"index", "value"})
BURN_FIELDS = frozenset({"spark_index", "offset", "before", "value"})
STEP_FIELDS = frozenset({"step", "realtime", "dirty"})
TRANSITION_FIELDS = frozenset({"before", "after"})
DIRTY_FIELDS = frozenset({"before", "after", "lock_reported"})
STEPS = ("lock", "update", "tick", "constant_time_tick")
HEX64 = re.compile(r"^[0-9a-f]{16}$")


class TraceError(ValueError):
    """Artifact violates the Fire1 oracle contract."""


def _fail(path: str, detail: str) -> NoReturn:
    raise TraceError(f"{path}: {detail}")


def _integer(value: object) -> bool:
    return isinstance(value, int) and not isinstance(value, bool)


def _object(value: object, path: str, fields: frozenset[str]) -> dict[str, object]:
    if not isinstance(value, dict):
        _fail(path, "must be an object")
    actual = frozenset(value)
    if actual != fields:
        missing = sorted(fields - actual)
        unknown = sorted(actual - fields)
        if missing:
            _fail(path, f"missing required field {missing[0]!r}")
        _fail(path, f"contains unknown field {unknown[0]!r}")
    return value


def _u(value: object, path: str, maximum: int) -> int:
    if not _integer(value) or not 0 <= value <= maximum:
        _fail(path, f"must be an unsigned integer in [0, {maximum}]")
    return value


def _transitions(value: object, path: str, fields: frozenset[str]) -> dict[str, object]:
    transition = _object(value, path, fields)
    for field in fields:
        if not isinstance(transition[field], bool):
            _fail(f"{path}.{field}", "must be a boolean")
    return transition


def validate_trace(document: object) -> dict[str, object]:
    trace = _object(document, "$", ROOT_FIELDS)
    if trace["format"] != TRACE_FORMAT:
        _fail("$.format", f"must equal {TRACE_FORMAT!r}")
    if trace["version"] != TRACE_VERSION or isinstance(trace["version"], bool):
        _fail("$.version", f"must equal {TRACE_VERSION}")
    if trace["enabled"] is not True:
        _fail("$.enabled", "must be true")

    target = _object(trace["target"], "$.target", TARGET_FIELDS)
    archive = _object(target["archive"], "$.target.archive", ARCHIVE_FIELDS)
    if not isinstance(archive["path"], str) or not archive["path"]:
        _fail("$.target.archive.path", "must be a non-empty loaded archive path")
    if archive["sha256"] != ARCHIVE_SHA256:
        _fail("$.target.archive.sha256", "must be the pinned HPParticle.u SHA-256")
    export = _object(target["export"], "$.target.export", EXPORT_FIELDS)
    if export["index"] != 886:
        _fail("$.target.export.index", "must equal pinned export index 886")
    if export["path"] != "HPParticle.hp_fx.Fire1":
        _fail("$.target.export.path", "must equal 'HPParticle.hp_fx.Fire1'")
    if target["class"] != "Fire.FireTexture":
        _fail("$.target.class", "must equal 'Fire.FireTexture'")
    dimensions = _object(target["dimensions"], "$.target.dimensions", DIMENSION_FIELDS)
    if dimensions["u"] != 128 or dimensions["v"] != 128:
        _fail("$.target.dimensions", "must equal 128x128")
    if target["active_spark_count"] != 105:
        _fail("$.target.active_spark_count", "must equal 105")

    init = _object(trace["init_tables"], "$.init_tables", INIT_FIELDS)
    rng = _object(init["rng"], "$.init_tables.rng", RNG_FIELDS)
    before = _u(rng["before"], "$.init_tables.rng.before", 0xFFFFFFFFFFFFFFFF)
    after = _u(rng["after"], "$.init_tables.rng.after", 0xFFFFFFFFFFFFFFFF)
    if after - before != 512:
        _fail("$.init_tables.rng", "must contain InitTables' 512 appRand draws")
    if not isinstance(init["fingerprint"], str) or not HEX64.fullmatch(init["fingerprint"]):
        _fail("$.init_tables.fingerprint", "must be a lowercase 16-hex-byte table fingerprint")

    speed = _object(trace["first_speed_rand"], "$.first_speed_rand", SPEED_FIELDS)
    _u(speed["index"], "$.first_speed_rand.index", 63)
    _u(speed["value"], "$.first_speed_rand.value", 255)

    burns = trace["burn_mip0_writes"]
    if not isinstance(burns, list) or not burns:
        _fail("$.burn_mip0_writes", "must contain first normal-tick SPARK_Burn writes")
    prior_spark = -1
    for index, value in enumerate(burns):
        burn = _object(value, f"$.burn_mip0_writes[{index}]", BURN_FIELDS)
        spark = _u(burn["spark_index"], f"$.burn_mip0_writes[{index}].spark_index", 104)
        if spark <= prior_spark:
            _fail(f"$.burn_mip0_writes[{index}].spark_index", "must be strictly increasing native spark order")
        prior_spark = spark
        _u(burn["offset"], f"$.burn_mip0_writes[{index}].offset", 128 * 128 - 1)
        _u(burn["before"], f"$.burn_mip0_writes[{index}].before", 255)
        _u(burn["value"], f"$.burn_mip0_writes[{index}].value", 255)

    lifecycle = trace["lifecycle"]
    if not isinstance(lifecycle, list) or len(lifecycle) != len(STEPS):
        _fail("$.lifecycle", "must contain exactly Lock, Update, Tick, ConstantTimeTick observations")
    for index, expected in enumerate(STEPS):
        step = _object(lifecycle[index], f"$.lifecycle[{index}]", STEP_FIELDS)
        if step["step"] != expected:
            _fail(f"$.lifecycle[{index}].step", f"must equal {expected!r}")
        _transitions(step["realtime"], f"$.lifecycle[{index}].realtime", TRANSITION_FIELDS)
        dirty = _transitions(step["dirty"], f"$.lifecycle[{index}].dirty", DIRTY_FIELDS)
        if step["realtime"]["before"] is not True or step["realtime"]["after"] is not True:  # type: ignore[index]
            _fail(f"$.lifecycle[{index}].realtime", "must retain the FireTexture realtime flag")
        if expected == "update" and dirty["after"] is not True:
            _fail(f"$.lifecycle[{index}].dirty.after", "must show UTexture::Update setting dirty")
    return trace


def _first_difference(cpp: object, rust: object, path: str = "$") -> dict[str, object] | None:
    """Return the first deterministic structural difference between valid traces."""
    if type(cpp) is not type(rust):
        return {"path": path, "cpp": cpp, "rust": rust}
    if isinstance(cpp, dict):
        for key in cpp:
            if key not in rust:
                return {"path": f"{path}.{key}", "cpp": cpp[key], "rust": {"missing": True}}
            difference = _first_difference(cpp[key], rust[key], f"{path}.{key}")
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


def compare_traces(cpp_document: object, rust_document: object) -> dict[str, object]:
    """Strictly compare C++ and Rust FireTexture v1 artifacts."""
    cpp = validate_trace(cpp_document)
    rust = validate_trace(rust_document)
    difference = _first_difference(cpp, rust)
    if difference is not None:
        return {"status": "mismatch", "first_difference": difference}
    return {"status": "match", "first_difference": None}


def _synthetic_trace() -> dict[str, object]:
    return {
        "format": TRACE_FORMAT, "version": 1, "enabled": True,
        "target": {"archive": {"path": "/fixture/System/HPParticle.u", "sha256": ARCHIVE_SHA256}, "export": {"index": 886, "path": "HPParticle.hp_fx.Fire1"}, "class": "Fire.FireTexture", "dimensions": {"u": 128, "v": 128}, "active_spark_count": 105},
        "init_tables": {"rng": {"before": 7, "after": 519}, "fingerprint": "0123456789abcdef"},
        "first_speed_rand": {"index": 1, "value": 42},
        "burn_mip0_writes": [{"spark_index": 0, "offset": 0, "before": 0, "value": 42}],
        "lifecycle": [
            {"step": "lock", "realtime": {"before": True, "after": True}, "dirty": {"before": False, "after": False, "lock_reported": False}},
            {"step": "update", "realtime": {"before": True, "after": True}, "dirty": {"before": False, "after": True, "lock_reported": False}},
            {"step": "tick", "realtime": {"before": True, "after": True}, "dirty": {"before": True, "after": True, "lock_reported": False}},
            {"step": "constant_time_tick", "realtime": {"before": True, "after": True}, "dirty": {"before": True, "after": True, "lock_reported": False}},
        ],
    }


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
    parser.add_argument("--trace", type=Path, help="FireTexture v1 JSON artifact to validate")
    parser.add_argument("--cpp-trace", type=Path, help="C++ FireTexture v1 JSON artifact to compare")
    parser.add_argument("--rust-trace", type=Path, help="Rust FireTexture v1 JSON artifact to compare")
    parser.add_argument("--report", type=Path, help="write JSON comparison report here (stdout otherwise)")
    parser.add_argument("--self-test", action="store_true", help="validate synthetic schema and comparator cases")
    args = parser.parse_args(argv)
    comparing = args.cpp_trace is not None or args.rust_trace is not None
    if comparing and (args.cpp_trace is None or args.rust_trace is None):
        parser.error("--cpp-trace and --rust-trace must be supplied together")
    if args.trace is not None and comparing:
        parser.error("--trace cannot be combined with --cpp-trace or --rust-trace")
    if args.report is not None and not comparing:
        parser.error("--report requires --cpp-trace and --rust-trace")
    if not args.trace and not comparing and not args.self_test:
        parser.error("one of --trace, --cpp-trace/--rust-trace, or --self-test is required")
    try:
        if args.self_test:
            valid = _synthetic_trace()
            validate_trace(valid)
            invalid = _synthetic_trace()
            invalid["target"]["export"]["index"] = 885  # type: ignore[index]
            try:
                validate_trace(invalid)
            except TraceError:
                pass
            else:
                raise TraceError("synthetic negative export-index case was accepted")
            if compare_traces(valid, valid)["status"] != "match":
                raise TraceError("synthetic matching comparison was rejected")
        if args.trace:
            _read_trace(args.trace, "FireTexture")
        if comparing:
            assert args.cpp_trace is not None and args.rust_trace is not None
            report = compare_traces(
                _read_trace(args.cpp_trace, "C++"),
                _read_trace(args.rust_trace, "Rust"),
            )
            _write_report(args.report, report)
            return 0 if report["status"] == "match" else 1
    except TraceError as error:
        parser.error(str(error))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
