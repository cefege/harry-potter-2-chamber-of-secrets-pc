#!/usr/bin/env python3
"""Repeatable runtime performance baseline for the shipping engine binary.

Launches each selected map for a fixed number of ticks, several times, with
``HP2_FRAME_TIMING=1`` and reports per-map medians (and min/max across
repeats) of tick cost, frame interval, map load time, peak RSS and wall time.
Every sample runs in a fresh helper process so ``ru_maxrss`` of its single
child is that run's own peak.

This measures one binary, one data root, one machine. Compare two runs only if
they used equivalent fixtures, RNG inputs, ticks, maps, renderer and machine
state. The optional deterministic clock requires input/audio/visual markers;
it does not prove RNG equivalence or a valid campaign state. CutError messages
reject a sample. This does not measure GPU time separately, audio quality,
input latency or physical display presentation. Check ``spread`` and retain
fixture/parity evidence before interpreting a difference.

Caps: every sample is forced uncapped and verified. The engine ini gets
``FrameRateLimit=1000`` (a 1000 fps ceiling; SleepFrameCap never binds) and ``UseVSync=Off`` (Immediate present),
and the run uses ``--fixed-dt`` so the unfocused-window 10 fps clamp (applied only
when no fixed step is set) cannot engage. A sample fails when more than
``--max-idle-fraction`` of the frame interval lies outside the engine tick, which
is how an unfocused clamp (102 ms frames around a 22 ms tick) was caught.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import math
import platform
import re
import resource
import statistics
import subprocess
import sys
import tempfile
import time
from pathlib import Path

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import game_test  # noqa: E402

DEFAULT_MAPS = (
    "PrivetDr,Entry,Ch2Skurge,Adv1Willow,Entryhall_hub,Grandstaircase_hub,"
    "Grounds_hub,GreatHall_G,Quidditch,Duel01"
)
PERF = re.compile(
    r"<HP2_PERF> (tick|interval) n=(\d+) mean_ms=([\d.]+) p50_ms=([\d.]+) "
    r"p95_ms=([\d.]+) p99_ms=([\d.]+) max_ms=([\d.]+)"
)
LOAD = re.compile(r"Load time (\S+): ([\d.]+) seconds total, ([\d.]+) app")
PRESENTED = re.compile(r"WgpuDrv: frames presented=(\d+) occluded=(\d+) outdated=(\d+) other_failure=(\d+)")
PRESENTATION_KEYS = ("presented", "occluded", "outdated", "other_failure")
VERIFY = re.compile(r"<HP2_VERIFY>\s+(input|audio|visuals)=(\S+)")
EXPECTED_VERIFY = {"input": "isolated", "audio": "loopback", "visuals": "tick"}


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def set_ini_value(text: str, section: str, key: str, value: str) -> str:
    """Set ``key`` inside ``[section]``; the key must already exist exactly once."""
    match = re.search(rf"^\[{re.escape(section)}\]\s*$", text, re.MULTILINE)
    if match is None:
        raise SystemExit(f"perf ini: missing section [{section}]")
    end = re.search(r"^\[", text[match.end():], re.MULTILINE)
    stop = match.end() + (end.start() if end else len(text) - match.end())
    body, count = re.subn(rf"^{re.escape(key)}=.*$", f"{key}={value}", text[match.end():stop], flags=re.MULTILINE)
    if count != 1:
        raise SystemExit(f"perf ini: expected exactly one {section}/{key}, found {count}")
    return text[:match.end()] + body + text[stop:]


def uncapped_ini(data_root: Path, destination: Path, renderer: str) -> Path:
    """Engine ini with the frame-rate limit and vsync forced off.

    ``FrameRateLimit=1000`` is a ceiling no map approaches (SleepFrameCap only
    sleeps for the part of a 1 ms frame that is left) and ``UseVSync=Off``
    selects the Immediate present mode.
    """
    base = game_test.render_device_engine_ini(data_root, destination, renderer)
    text = base.read_text(encoding="utf-8")
    text = set_ini_value(text, "Engine.GameEngine", "FrameRateLimit", "1000.000000")
    text = set_ini_value(text, "WgpuDrv.WgpuRenderDevice", "UseVSync", "Off")
    base.write_text(text, encoding="utf-8")
    return base


def one(args: argparse.Namespace) -> int:
    """Run one sample and print a single JSON line."""
    log = Path(args.log)
    ini = uncapped_ini(Path(args.data_root).resolve(strict=True), log.with_suffix(".ini"), args.renderer)
    started = time.monotonic()
    extra_env = {"HP2_FRAME_TIMING": "1"}
    if args.deterministic_clock:
        extra_env["HP2_DETERMINISTIC_VERIFICATION"] = "1"
    result = game_test.run_game(
        app=args.engine_bin, engine_bin=args.engine_bin, data_root=args.data_root,
        selected_map=f"Maps/{args.one}.unr", renderer=args.renderer, ticks=args.ticks,
        timeout_seconds=args.timeout, log_path=log, no_sound=not args.deterministic_clock,
        extra_env=extra_env, ini_file=ini, fixed_dt=1.0 / 60.0,
    )
    wall = time.monotonic() - started
    text = log.read_text(encoding="utf-8", errors="replace") if log.exists() else ""
    # Each tally describes one device; sum all tallies so a clean device cannot
    # hide presentation failures reported by another device.
    tallies = [tuple(map(int, m.groups())) for m in PRESENTED.finditer(text)]
    presentation = {key: sum(row[i] for row in tallies) for i, key in enumerate(PRESENTATION_KEYS)}
    sample = {"map": args.one, "exit_status": result.get("exit_status"),
              "timed_out": result.get("timed_out"), "wall_seconds": wall,
              "failure_markers": result.get("failure_markers", []),
              "run_passed": result.get("passed", False), "presentation": presentation,
              "cut_error_count": text.count("CutError:")}
    if args.deterministic_clock:
        markers = {key: [] for key in EXPECTED_VERIFY}
        for key, value in VERIFY.findall(text):
            markers[key].append(value)
        sample["verification"] = markers
    for match in PERF.finditer(text):
        label, n, mean, p50, p95, p99, mx = match.groups()
        sample[label] = {"n": int(n), "mean": float(mean), "p50": float(p50),
                         "p95": float(p95), "p99": float(p99), "max": float(mx)}
    loads = [float(m.group(2)) for m in LOAD.finditer(text)]
    sample["load_seconds"] = loads[-1] if loads else None
    sample["ini"] = str(ini)
    sample["frame_rate_limit"] = re.search(r"^FrameRateLimit=(.*)$", ini.read_text(), re.MULTILINE).group(1)
    # macOS reports bytes; Linux kilobytes.
    peak = resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss
    sample["peak_rss_mb"] = peak / (1024 * 1024 if sys.platform == "darwin" else 1024)
    print("PERF_SAMPLE " + json.dumps(sample))
    return 0 if valid_sample(sample, args.deterministic_clock) else 1


def valid_sample(sample: dict, deterministic_clock: bool) -> bool:
    presentation = sample.get("presentation", {})
    cut_errors = sample.get("cut_error_count")
    if not isinstance(cut_errors, int) or isinstance(cut_errors, bool) or cut_errors != 0:
        return False
    if (not sample.get("run_passed") or sample.get("exit_status") != 0
            or sample.get("timed_out") or sample.get("failure_markers")
            or presentation.get("presented", 0) <= 0
            or any(presentation.get(key) != 0 for key in PRESENTATION_KEYS[1:])):
        return False
    for name in ("tick", "interval"):
        timing = sample.get(name, {})
        if not isinstance(timing.get("n"), int) or timing["n"] <= 0:
            return False
        for key in ("mean", "p50", "p95", "p99", "max"):
            value = timing.get(key)
            if not isinstance(value, (int, float)) or not math.isfinite(value) or value <= 0:
                return False
    if deterministic_clock:
        markers = sample.get("verification", {})
        if any(not markers.get(key) or any(value != expected for value in markers[key])
               for key, expected in EXPECTED_VERIFY.items()):
            return False
    return True


TARGET_FPS = 60.0


def evaluate(summary: dict, target_fps: float = TARGET_FPS) -> list[dict]:
    """Per-map verdict against a frame-time budget.

    A map meets the target when its median p95 tick time is within the budget
    (1000 / target_fps ms), i.e. 95% of frames reach the target rate. ``gap`` is
    the speedup still needed at p95. p99 and max are reported, not gated.
    """
    budget = 1000.0 / target_fps
    rows = []
    for name, row in summary.items():
        p95 = row["tick_p95"]["median"]
        rows.append({"map": name, "budget_ms": budget, "p50_ms": row["tick_p50"]["median"], "p95_ms": p95,
                     "p99_ms": row["tick_p99"]["median"], "fps_p50": 1000.0 / row["tick_p50"]["median"],
                     "meets_target": p95 <= budget, "speedup_needed": max(1.0, p95 / budget)})
    return rows


def spread(values: list[float]) -> dict[str, float]:
    return {"median": statistics.median(values), "min": min(values), "max": max(values)}


def print_table(rows: list[dict], target: float) -> None:
    print(f"target {target:.0f} fps (p95 tick <= {1000.0 / target:.2f} ms)")
    print(f"{'map':22}{'fps@p50':>9}{'p50 ms':>8}{'p95 ms':>8}{'p99 ms':>8}{'need x':>8}  verdict")
    for r in rows:
        print(f"{r['map']:22}{r['fps_p50']:9.0f}{r['p50_ms']:8.2f}{r['p95_ms']:8.2f}{r['p99_ms']:8.2f}"
              f"{r['speedup_needed']:8.2f}  {'MEETS' if r['meets_target'] else 'MISSES'}")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--engine-bin", type=Path)
    parser.add_argument("--data-root", type=Path)
    parser.add_argument("--maps", default=DEFAULT_MAPS)
    parser.add_argument("--ticks", type=int, default=600)
    parser.add_argument("--repeats", type=int, default=5)
    parser.add_argument("--renderer", default="wgpu")
    parser.add_argument("--timeout", type=float, default=240.0)
    parser.add_argument("--label", default="baseline")
    parser.add_argument("--output", type=Path)
    parser.add_argument("--max-idle-fraction", type=float, default=0.25,
                        help="fail a sample when more of the frame than this is outside the engine tick")
    parser.add_argument("--target-fps", type=float, default=TARGET_FPS)
    parser.add_argument("--evaluate", type=Path, help="print the target table for an existing report and exit")
    parser.add_argument("--deterministic-clock", action="store_true",
                        help="require isolated input, loopback audio and tick-driven visuals")
    parser.add_argument("--one")
    parser.add_argument("--log")
    args = parser.parse_args()
    if args.evaluate:
        data = json.loads(args.evaluate.read_text())
        print_table(evaluate(data["summary"], args.target_fps), args.target_fps)
        return 0
    if args.one:
        if args.engine_bin is None or args.data_root is None:
            parser.error("--one needs --engine-bin and --data-root")
        return one(args)
    if args.output is None or args.engine_bin is None or args.data_root is None:
        parser.error("--engine-bin, --data-root and --output are required")
    out_dir = args.output.parent
    out_dir.mkdir(parents=True, exist_ok=True)
    engine = args.engine_bin.resolve(strict=True)
    samples: dict[str, list[dict]] = {}
    for name in [m.strip() for m in args.maps.split(",") if m.strip()]:
        for repeat in range(args.repeats):
            log = out_dir / f"{args.label}-{name}-{repeat}.log"
            command = [sys.executable, "-B", str(Path(__file__).resolve()), "--one", name,
                       "--engine-bin", str(engine), "--data-root", str(args.data_root),
                       "--ticks", str(args.ticks), "--renderer", args.renderer,
                       "--timeout", str(args.timeout), "--log", str(log)]
            if args.deterministic_clock:
                command.append("--deterministic-clock")
            done = subprocess.run(command, capture_output=True, text=True)
            lines = [l for l in done.stdout.splitlines() if l.startswith("PERF_SAMPLE ")]
            if done.returncode != 0 or not lines:
                (out_dir / f"{args.label}-{name}-{repeat}.error").write_text(done.stdout + done.stderr)
                print(f"FAILED {name} #{repeat}: see .error", file=sys.stderr)
                return 1
            sample = json.loads(lines[-1][len("PERF_SAMPLE "):])
            if not valid_sample(sample, args.deterministic_clock):
                (out_dir / f"{args.label}-{name}-{repeat}.error").write_text(json.dumps(sample, indent=2))
                print(f"FAILED {name} #{repeat}: run was not clean", file=sys.stderr)
                return 1
            # Cap detector: an uncapped fixed-step loop spends nearly all of each
            # frame inside the engine tick. A large unexplained gap means a sleep
            # or present/vsync wait outside it.
            gap = 1.0 - sample["tick"]["mean"] / max(sample["interval"]["mean"], 1e-9)
            sample["idle_fraction"] = gap
            if gap > args.max_idle_fraction:
                (out_dir / f"{args.label}-{name}-{repeat}.error").write_text(json.dumps(sample, indent=2))
                print(f"FAILED {name} #{repeat}: {gap:.0%} of each frame is outside the tick; "
                      "the run looks capped (frame limit, vsync or present wait)", file=sys.stderr)
                return 1
            samples.setdefault(name, []).append(sample)
            print(f"{name} #{repeat}: tick p50 {sample['tick']['p50']} p99 {sample['tick']['p99']} "
                  f"max {sample['tick']['max']} load {sample['load_seconds']} rss {sample['peak_rss_mb']:.0f}MB", flush=True)
    summary = {}
    for name, rows in samples.items():
        summary[name] = {
            "repeats": len(rows),
            **{f"tick_{k}": spread([r["tick"][k] for r in rows]) for k in ("mean", "p50", "p95", "p99", "max")},
            **{f"interval_{k}": spread([r["interval"][k] for r in rows]) for k in ("mean", "p50", "p99", "max")},
            "load_seconds": spread([r["load_seconds"] for r in rows if r["load_seconds"] is not None] or [0.0]),
            "peak_rss_mb": spread([r["peak_rss_mb"] for r in rows]),
            "wall_seconds": spread([r["wall_seconds"] for r in rows]),
        }
    report = {
        "label": args.label, "engine": str(engine), "engine_sha256": sha256(engine),
        "data_root": str(args.data_root), "ticks": args.ticks, "repeats": args.repeats,
        "renderer": args.renderer, "deterministic_clock": args.deterministic_clock,
        "machine": platform.platform(), "processor": platform.processor(),
        "cpu": subprocess.run(["sysctl", "-n", "machdep.cpu.brand_string"], capture_output=True, text=True).stdout.strip(),
        "summary": summary, "target": evaluate(summary, args.target_fps), "samples": samples,
        "caps": "FrameRateLimit=1000 and UseVSync=Off forced via the engine ini; idle-fraction detector enforced",
        "limits": ["single machine and data root", "tick cost includes any GPU/present waits inside the engine tick",
                   "no physical display or input latency",
                   "loopback audio only" if args.deterministic_clock else "no audio"],
    }
    args.output.write_text(json.dumps(report, indent=2) + "\n")
    print_table(report["target"], args.target_fps)
    print(f"wrote {args.output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
