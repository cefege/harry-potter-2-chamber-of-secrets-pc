#!/usr/bin/env python3
"""Run the save/reload replay under `sample` so save, load and map-switch costs can be attributed.

usage: saveload_profile.py ENGINE DATA_ROOT OUT_DIR [MAP]
Writes OUT_DIR/sample.txt (macOS sample of the game) and the replay artifacts. The replay presses
F5 (SAVEGAME 9) at about tick 89 and F9 (LOADGAME 9) at about tick 122 on a fixed 1/30 s step.
"""
import os, subprocess, sys, time
from pathlib import Path
engine, data, out = (Path(a).resolve() for a in sys.argv[1:4]); map_name = sys.argv[4] if len(sys.argv) > 4 else "Adv1Willow"
out.mkdir(parents=True, exist_ok=True)
root = Path(__file__).resolve().parents[1]
cmd = [sys.executable, "-B", str(root / "Tests/InputScriptSmoke.py"), "--smoke", "--data-root", str(data), "--engine-bin", str(engine),
       f"--map={map_name}", f"--fixture={root}/Tests/Fixtures/input_script_save_reload.txt", "--bind", "F5=SAVEGAME 9", "--bind", "F9=LOADGAME 9",
       "--expect-user-file=Save9.usa", "--expect-map-load=Save9.usa", "--ticks=300", "--timeout=180", "--artifact-dir", str(out)]
env = {**os.environ, "HP2_FRAME_TIMING": "1"}
smoke = subprocess.Popen(cmd, stdout=(out / "smoke.json").open("w"), stderr=subprocess.STDOUT, env=env)
pid = None
for _ in range(200):
    r = subprocess.run(["pgrep", "-f", "hp2_game_rust"], capture_output=True, text=True)
    pids = [int(x) for x in r.stdout.split() if int(x) != os.getpid()]
    pids = [p for p in pids if "hp2_game_rust" in subprocess.run(["ps", "-o", "comm=", "-p", str(p)], capture_output=True, text=True).stdout]
    if pids: pid = pids[0]; break
    time.sleep(0.1)
if pid is None: raise SystemExit("game process not found")
res = subprocess.run(["sample", str(pid), "20", "1", "-file", str(out / "sample.txt")], capture_output=True, text=True)
print("sampled pid", pid, res.returncode, res.stderr[-200:], res.stdout[-200:])
smoke.wait(timeout=300)
print("smoke exit", smoke.returncode)
