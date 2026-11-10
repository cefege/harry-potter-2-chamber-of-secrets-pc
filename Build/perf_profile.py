#!/usr/bin/env python3
"""Sample the running engine with macOS ``sample`` on one map (uncapped, fixed step).

Usage: perf_profile.py --engine-bin BIN --data-root ROOT --map GreatHall_G --out DIR
Writes DIR/<map>.sample.txt (call tree, heaviest-stack summary) and the game log.
The sample is wall-clock stack sampling of all threads at 1 ms; it shows where
threads spend time (including waits), not GPU execution time.
"""
import argparse, subprocess, sys, time
from pathlib import Path
sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))
import game_test, perf_baseline

p = argparse.ArgumentParser()
p.add_argument("--engine-bin", type=Path, required=True)
p.add_argument("--data-root", type=Path, required=True)
p.add_argument("--map", required=True)
p.add_argument("--out", type=Path, required=True)
p.add_argument("--ticks", type=int, default=4000)
p.add_argument("--delay", type=float, default=6.0)
p.add_argument("--seconds", type=int, default=10)
a = p.parse_args()
a.out.mkdir(parents=True, exist_ok=True)
root = a.data_root.resolve(strict=True)
ini = perf_baseline.uncapped_ini(root, a.out / f"{a.map}.ini", "wgpu")
cmd = game_test.build_launch_command(a.engine_bin.resolve(strict=True), root, "..\\Maps\\%s.unr" % a.map, "wgpu", a.ticks,
                                     no_sound=True, ini_file=ini, fixed_dt=1 / 60)
log = open(a.out / f"{a.map}.log", "wb")
# Isolated HOME/XDG: the game writes its user directory (logs, ini copies, saves).
iso = a.out / f"{a.map}-home"
for sub in ("home", "tmp", "xdg-config", "xdg-cache", "xdg-data"):
    (iso / sub).mkdir(parents=True, exist_ok=True)
env = {**__import__("os").environ, "HP2_FRAME_TIMING": "1", "HOME": str(iso / "home"), "TMPDIR": str(iso / "tmp"),
       "XDG_CONFIG_HOME": str(iso / "xdg-config"), "XDG_CACHE_HOME": str(iso / "xdg-cache"), "XDG_DATA_HOME": str(iso / "xdg-data")}
proc = subprocess.Popen(cmd, stdout=log, stderr=subprocess.STDOUT, env=env)
time.sleep(a.delay)
sample = subprocess.run(["sample", str(proc.pid), str(a.seconds), "1", "-file", str(a.out / f"{a.map}.sample.txt")], capture_output=True, text=True)
proc.wait(timeout=600)
print("game exit", proc.returncode, "sample exit", sample.returncode, sample.stderr[-200:])
