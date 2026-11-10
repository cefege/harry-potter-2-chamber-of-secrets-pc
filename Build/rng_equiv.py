#!/usr/bin/env python3
"""Simulation-equivalence check: compare the per-call RNG trace of two engine builds.

The RNG is consumed by gameplay, script and effects code in a fixed order, so equal
traces over many fixed-step ticks are strong (not complete) evidence that an
optimization did not change simulation logic. It says nothing about rendering.

  rng_equiv.py --a BIN --b BIN --data-root D --out DIR [--maps a,b] [--ticks N]
"""
import argparse, hashlib, sys
from pathlib import Path
sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))
import game_test, perf_baseline

p = argparse.ArgumentParser()
p.add_argument("--a", type=Path, required=True); p.add_argument("--b", type=Path, required=True)
p.add_argument("--data-root", type=Path, required=True); p.add_argument("--out", type=Path, required=True)
p.add_argument("--maps", default="PrivetDr,Entry,Ch2Skurge,Adv1Willow,Entryhall_hub,Grounds_hub,GreatHall_G,Quidditch,Duel01")
p.add_argument("--ticks", type=int, default=400)
a = p.parse_args()
root = a.data_root.resolve(strict=True); a.out.mkdir(parents=True, exist_ok=True)
bad = 0
for name in a.maps.split(","):
    digests = []
    for label, binary in (("a", a.a), ("b", a.b)):
        trace = (a.out / f"{name}-{label}.rng").resolve()
        trace.unlink(missing_ok=True)
        ini = perf_baseline.uncapped_ini(root, a.out / f"{name}-{label}.ini", "wgpu")
        r = game_test.run_game(app=binary, engine_bin=binary, data_root=root, selected_map=f"Maps/{name}.unr", renderer="wgpu",
                               ticks=a.ticks, timeout_seconds=300, log_path=a.out / f"{name}-{label}.log", no_sound=True,
                               extra_env={"HP2_RNG_TRACE": str(trace), "HP2_RNG_TRACE_LIMIT": "5000000"}, ini_file=ini, fixed_dt=1 / 60)
        if r.get("exit_status") != 0 or r.get("failure_markers"):
            raise SystemExit(f"{name}/{label}: unclean run")
        data = trace.read_bytes()
        digests.append((hashlib.sha256(data).hexdigest(), len(data)))
    same = digests[0] == digests[1]
    bad += not same
    print(("SAME " if same else "DIFF ") + f"{name} rng trace bytes={digests[0][1]}/{digests[1][1]}", flush=True)
sys.exit(1 if bad else 0)
