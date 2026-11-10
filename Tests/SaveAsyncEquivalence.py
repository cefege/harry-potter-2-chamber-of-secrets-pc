#!/usr/bin/env python3
"""A background (forked) save writes the same slot files as the blocking save.

Runs the save-gap-load replay once with the default background save and once with
HP2_SYNC_SAVE=1, then compares every file under the save directory. The only bytes allowed to
differ are the 16-byte random package GUID (offsets 36..51), which also differs between two
blocking runs. Also requires the save to stall the game for far less than the blocking save.
"""
import argparse, glob, os, re, subprocess, sys
from pathlib import Path
ROOT = Path(__file__).resolve().parents[1]


def run(engine, data, out, sync):
    env = {**os.environ, "HP2_FRAME_TIMING": "1"}
    env.pop("HP2_SYNC_SAVE", None)
    if sync:
        env["HP2_SYNC_SAVE"] = "1"
    cmd = [sys.executable, "-B", str(ROOT / "Tests/InputScriptSmoke.py"), "--smoke", "--data-root", str(data), "--engine-bin", str(engine),
           "--map=Adv1Willow", f"--fixture={ROOT}/Tests/Fixtures/input_script_save_gap_load.txt", "--bind", "F5=SAVEGAME 9", "--bind", "F9=LOADGAME 9",
           "--expect-user-file=Save9.usa", "--expect-map-load=Save9.usa", "--ticks=330", "--timeout=180", "--artifact-dir", str(out)]
    done = subprocess.run(cmd, capture_output=True, text=True, env=env)
    if done.returncode != 0:
        raise SystemExit(f"replay failed (sync={sync}):\n{done.stdout[-600:]}{done.stderr[-600:]}")
    log = (out / "replay-run.log").read_text(errors="replace")
    frames = [float(x) for x in re.findall(r"frame_ms=([\d.]+)", log)]
    files = {}
    for path in glob.glob(str(out / "home-*/Library/Application Support/Harry Potter 2/User/Save/**"), recursive=True):
        if os.path.isfile(path):
            files[os.path.relpath(path, glob.glob(str(out / "home-*/Library/Application Support/Harry Potter 2/User/Save"))[0])] = Path(path).read_bytes()
    return files, max(frames[85:100])


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--engine-bin", type=Path, required=True)
    p.add_argument("--data-root", type=Path, required=True)
    p.add_argument("--artifact-dir", type=Path, required=True)
    a = p.parse_args()
    a.artifact_dir.mkdir(parents=True, exist_ok=True)
    background, background_ms = run(a.engine_bin, a.data_root, a.artifact_dir / "background", False)
    blocking, blocking_ms = run(a.engine_bin, a.data_root, a.artifact_dir / "blocking", True)
    failures = []
    if sorted(background) != sorted(blocking):
        failures.append(f"file sets differ: {sorted(background)} vs {sorted(blocking)}")
    for name in sorted(set(background) & set(blocking)):
        x, y = background[name], blocking[name]
        if len(x) != len(y):
            failures.append(f"{name}: sizes {len(x)} vs {len(y)}")
            continue
        diff = [i for i in range(len(x)) if x[i] != y[i]]
        if diff and not (diff[0] >= 36 and diff[-1] <= 51):
            failures.append(f"{name}: {len(diff)} bytes differ outside the package GUID (first {diff[0]}, last {diff[-1]})")
    if "Save9.usa" not in background:
        failures.append("background save wrote no Save9.usa")
    print(f"save stall: background {background_ms:.0f} ms, blocking {blocking_ms:.0f} ms")
    if background_ms > blocking_ms * 0.5:
        failures.append(f"background save stalled {background_ms:.0f} ms, not well below the blocking {blocking_ms:.0f} ms")
    for failure in failures:
        print("FAIL:", failure)
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
