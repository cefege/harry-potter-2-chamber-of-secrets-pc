#!/usr/bin/env python3
"""Real-time (no --fixed-dt) simulation-speed probe: game seconds per wall second.

Launches one map at several engine frame-rate limits and reports the engine's own
`<HP2_PERF> game_speed` summary. A window without keyboard focus is clamped to <=10 fps
by the engine's frame cap, so the probe tries to focus the game window first.
"""
import argparse, os, re, subprocess, sys, time
from pathlib import Path
sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))
import game_test, perf_baseline

p = argparse.ArgumentParser()
p.add_argument("--engine-bin", type=Path, required=True); p.add_argument("--data-root", type=Path, required=True)
p.add_argument("--out", type=Path, required=True); p.add_argument("--map", default="PrivetDr")
p.add_argument("--limits", default="30,60,144,1000"); p.add_argument("--seconds", type=float, default=12.0)
a = p.parse_args()
root = a.data_root.resolve(strict=True); a.out.mkdir(parents=True, exist_ok=True)
for limit in [float(x) for x in a.limits.split(",")]:
    ini = perf_baseline.uncapped_ini(root, a.out / f"{a.map}-{limit:g}.ini", "wgpu")
    text = ini.read_text(); text = perf_baseline.set_ini_value(text, "Engine.GameEngine", "FrameRateLimit", f"{limit:.6f}"); ini.write_text(text)
    ticks = int(min(limit, 400) * a.seconds)
    cmd = game_test.build_launch_command(a.engine_bin.resolve(strict=True), root, f"..\\Maps\\{a.map}.unr", "wgpu", ticks, no_sound=True, ini_file=ini, fixed_dt=None)
    iso = a.out / f"home-{limit:g}"
    for sub in ("home", "tmp"): (iso / sub).mkdir(parents=True, exist_ok=True)
    env = {**os.environ, "HP2_FRAME_TIMING": "1", "HOME": str(iso / "home"), "TMPDIR": str(iso / "tmp")}
    log = a.out / f"{a.map}-{limit:g}.log"
    with log.open("wb") as handle:
        proc = subprocess.Popen(cmd, stdout=handle, stderr=subprocess.STDOUT, env=env)
        time.sleep(2.0)
        subprocess.run(["osascript", "-e", f'tell application "System Events" to set frontmost of (first process whose unix id is {proc.pid}) to true'], capture_output=True)
        proc.wait(timeout=600)
    t = log.read_text(errors="replace")
    speed = re.findall(r"<HP2_PERF> game_speed [^\n]*", t); tick = re.findall(r"<HP2_PERF> interval [^\n]*", t)
    print(f"limit {limit:g}: {speed[-1] if speed else 'no speed line'}\n   {tick[-1] if tick else ''}", flush=True)
