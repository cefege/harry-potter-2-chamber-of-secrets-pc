#!/usr/bin/env python3
"""Time save, load and level-switch hitches with the save/reload replay.

usage: saveload_bench.py ENGINE DATA_ROOT OUT_DIR [REPEATS] [MAP]
The replay (Tests/Fixtures/input_script_save_reload.txt, fixed 1/30 s step) presses F5 (SAVEGAME 9)
around tick 90 and F9 (LOADGAME 9) around tick 122. The tick that runs a save/load is one long
frame (the maximum tick in 85..100 and 118..140); its `frame_ms` is the time the game stalls. Also reports the engine's own `Load time` lines
and the largest of the first 40 ticks (startup).
"""
import os, re, statistics, subprocess, sys
from pathlib import Path
engine, data, out = (Path(a).resolve() for a in sys.argv[1:4])
repeats = int(sys.argv[4]) if len(sys.argv) > 4 else 5; map_name = sys.argv[5] if len(sys.argv) > 5 else "Adv1Willow"
root = Path(__file__).resolve().parents[1]; out.mkdir(parents=True, exist_ok=True)
rows = []
for i in range(repeats):
    run = out / f"run{i}"
    cmd = [sys.executable, "-B", str(root / "Tests/InputScriptSmoke.py"), "--smoke", "--data-root", str(data), "--engine-bin", str(engine),
           f"--map={map_name}", f"--fixture={root}/Tests/Fixtures/input_script_save_reload.txt", "--bind", "F5=SAVEGAME 9", "--bind", "F9=LOADGAME 9",
           "--expect-user-file=Save9.usa", "--expect-map-load=Save9.usa", "--ticks=300", "--timeout=180", "--artifact-dir", str(run)]
    done = subprocess.run(cmd, capture_output=True, text=True, env={**os.environ, "HP2_FRAME_TIMING": "1"})
    if done.returncode != 0: raise SystemExit(f"run {i} failed:\n{done.stdout[-400:]}{done.stderr[-400:]}")
    text = (run / "replay-run.log").read_text(errors="replace")
    frames = [float(x) for x in re.findall(r"frame_ms=([\d.]+)", text)]
    save_f = max(frames[85:100]); load_f = max(frames[118:140])
    window = [(frames.index(save_f), save_f), (frames.index(load_f), load_f)]
    loads = [float(x) for x in re.findall(r"Load time [^:]*: ([\d.]+) seconds", text)]
    rows.append({"save_ms": save_f, "load_ms": load_f, "ticks": [w[0] for w in window],
                 "startup_max_ms": max(frames[:40]), "engine_load_s": loads})
    print(rows[-1], flush=True)
med = lambda k: statistics.median(r[k] for r in rows if r[k] is not None)
print(f"median save {med('save_ms'):.0f} ms, load {med('load_ms'):.0f} ms, startup worst tick {med('startup_max_ms'):.0f} ms (n={repeats})")
