#!/usr/bin/env python3
"""Compare every smoke-classified playable map against the C++ oracle."""

from __future__ import annotations

import argparse
import json
import math
import sys
from pathlib import Path
from typing import Any

import game_test


def _positive_int(value: str) -> int:
    try:
        parsed = int(value, 10)
    except ValueError as error:
        raise argparse.ArgumentTypeError("must be an integer") from error
    if parsed <= 0:
        raise argparse.ArgumentTypeError("must be a positive integer")
    return parsed


def _positive_float(value: str) -> float:
    try:
        parsed = float(value)
    except ValueError as error:
        raise argparse.ArgumentTypeError("must be a number") from error
    if parsed <= 0 or not math.isfinite(parsed):
        raise argparse.ArgumentTypeError("must be a positive finite number")
    return parsed


def _arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--oracle-app", type=Path, required=True)
    parser.add_argument("--engine-bin", type=Path, required=True)
    parser.add_argument("--data-root", type=Path, required=True)
    parser.add_argument("--smoke-report", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--maps", default=None, help="comma-separated map stem allowlist")
    parser.add_argument("--ticks", type=_positive_int, default=300)
    parser.add_argument("--fixed-dt", type=_positive_float, default=0.02)
    parser.add_argument("--rng-seed", type=int, default=1)
    parser.add_argument("--timeout", type=_positive_float, default=180)
    return parser.parse_args()


def _selected_maps(smoke_report: Path, allowlist: str | None) -> list[dict[str, Any]]:
    document = json.loads(smoke_report.read_text(encoding="utf-8"))
    if not isinstance(document, dict) or not isinstance(document.get("maps"), list):
        raise ValueError(f"invalid smoke report map list: {smoke_report}")
    maps = []
    for record in document["maps"]:
        if not isinstance(record, dict):
            raise ValueError(f"invalid map record in smoke report: {smoke_report}")
        if record.get("classification") == "playable_level":
            relative = record.get("map_relative_to_data_root")
            if not isinstance(relative, str) or not relative:
                raise ValueError(f"playable map lacks map_relative_to_data_root: {record!r}")
            maps.append(record)

    if allowlist:
        wanted = {name.strip().casefold() for name in allowlist.split(",") if name.strip()}
        available = {Path(record["map_relative_to_data_root"]).stem.casefold() for record in maps}
        missing = sorted(wanted - available)
        if missing:
            raise ValueError("requested maps not found among playable smoke records: " + ", ".join(missing))
        maps = [
            record for record in maps
            if Path(record["map_relative_to_data_root"]).stem.casefold() in wanted
        ]
    if not maps:
        raise ValueError("smoke report contains no selected playable_level maps")

    stems = [Path(record["map_relative_to_data_root"]).stem.casefold() for record in maps]
    if len(stems) != len(set(stems)):
        raise ValueError("selected playable maps have duplicate stems; report filenames would collide")
    return maps


def main() -> int:
    arguments = _arguments()
    try:
        repo_root = game_test._repo_root()
        oracle_app = game_test._resolve_path(arguments.oracle_app, repo_root, strict=True)
        engine_bin = game_test._resolve_path(arguments.engine_bin, repo_root, strict=True)
        data_root = game_test._resolve_path(arguments.data_root, repo_root, strict=True)
        smoke_report = game_test._resolve_path(arguments.smoke_report, repo_root, strict=True)
        output = game_test._resolve_path(arguments.output, repo_root, strict=False)
        game_test._validate_data_root(data_root)
        if not output.name:
            raise ValueError(f"output path must name a JSON file: {arguments.output}")
        if not 0 <= arguments.rng_seed <= game_test.RNG_SEED_MAX:
            raise ValueError(f"RNG seed must be an integer from 0 through {game_test.RNG_SEED_MAX}")
        selected = _selected_maps(smoke_report, arguments.maps)
        maps_directory = output.with_name(f"{output.stem}-maps")
        rows = []
        for record in selected:
            relative = record["map_relative_to_data_root"]
            stem = Path(relative).stem
            report_path = maps_directory / f"{stem}.json"
            try:
                report = game_test.run_paired_game(
                    app=oracle_app,
                    engine_bin=engine_bin,
                    data_root=data_root,
                    selected_map=relative,
                    renderer="wgpu",
                    ticks=arguments.ticks,
                    fixed_dt=arguments.fixed_dt,
                    timeout_seconds=arguments.timeout,
                    output=report_path,
                    checkpoints="authored-or-tick",
                    rng_seed=arguments.rng_seed,
                )
            except game_test.GameTestError as error:
                rows.append({
                    "map": stem,
                    "passed": False,
                    "report": None,
                    "error": str(error),
                })
                print(f"parity_sweep.py: {stem}: {error}", file=sys.stderr)
                continue
            rows.append({
                "map": stem,
                "passed": report["passed"],
                "report": game_test._display_path(report_path, repo_root),
            })

        passed_count = sum(row["passed"] is True for row in rows)
        profile = game_test._profile_for_data_root(data_root, repo_root)
        aggregate = {
            "version": 1,
            "oracle_app": game_test._display_path(oracle_app, repo_root),
            "engine_bin": game_test._display_path(engine_bin, repo_root),
            "data_root": game_test._display_path(data_root, repo_root),
            "profile": profile,
            "verification_mode": "deterministic",
            "maps": rows,
            "passed_count": passed_count,
            "total": len(rows),
        }
        game_test._write_atomic(output, (
            json.dumps(aggregate, ensure_ascii=True, allow_nan=False, indent=2, sort_keys=True) + "\n"
        ).encode("utf-8"))
        print(f"{game_test._display_path(output, repo_root)}: {passed_count}/{len(rows)} maps passed")
        return 0 if passed_count == len(rows) else 1
    except (OSError, ValueError, json.JSONDecodeError, game_test.GameTestError) as error:
        print(f"parity_sweep.py: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
