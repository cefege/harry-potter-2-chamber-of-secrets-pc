#!/usr/bin/env python3
"""Does the simulation depend on the step size? Capture the same game time at several fixed steps.

For each step dt the map runs for `--game-seconds` of game time (ticks = seconds/dt) and the
last frame is captured. If scenes at equal game time match across dt, per-tick effects are
small; large differences point at behavior tied to the tick rate. Not proof either way.
"""
import argparse, sys
from pathlib import Path
sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))
import game_test, perf_baseline

p = argparse.ArgumentParser()
p.add_argument("--engine-bin", type=Path, required=True); p.add_argument("--data-root", type=Path, required=True)
p.add_argument("--out", type=Path, required=True); p.add_argument("--map", default="PrivetDr")
p.add_argument("--fps", default="30,60,144"); p.add_argument("--game-seconds", type=float, default=5.0)
a = p.parse_args()
root = a.data_root.resolve(strict=True); a.out.mkdir(parents=True, exist_ok=True)
for fps in [int(x) for x in a.fps.split(",")]:
    frame = round(a.game_seconds * fps)
    d = a.out / f"fps{fps}"
    ini = perf_baseline.uncapped_ini(root, a.out / f"fps{fps}.ini", "wgpu")
    env = {"HP2_CAPTURE_FRAMES": str(d), "HP2_CAPTURE_FRAME_INDICES": str(frame), "HP2_CAPTURE_MAP": a.map, "HP2_CAPTURE_TICKS": str(frame + 1)}
    r = game_test.run_game(app=a.engine_bin, engine_bin=a.engine_bin, data_root=root, selected_map=f"Maps/{a.map}.unr", renderer="wgpu",
                           ticks=frame + 2, timeout_seconds=300, log_path=a.out / f"fps{fps}.log", no_sound=True, extra_env=env, ini_file=ini, fixed_dt=1.0 / fps)
    print(fps, "fps ->", d / f"frame_{frame:06}.png", "exit", r.get("exit_status"), flush=True)
