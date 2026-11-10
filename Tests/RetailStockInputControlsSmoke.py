#!/usr/bin/env python3
"""Opt-in retail replay for the Controls page additions (exact mouse speed box, smoothing toggle).

Needs retail data (the page layout is retail's), a built `hp2_game_rust`, and the display. It opens
the Controls page, clicks the "Mouse Smoothing" checkbox, feeds the number box the value a typed
edit would (`HP2_INPUT_SELFTEST`, because replays cannot type text) and checks the saved User.ini.
It also keeps a screenshot of the page; review it for clipping and overlap.
"""
import argparse, glob, os, shutil, subprocess, sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = """tick 0.033333335
idle 60
press Enter
idle 40
press Escape
idle 80
mouse 120 472
idle 5
press LeftMouse
idle 60
mouse 170 229
idle 5
press LeftMouse
idle 30
"""


def main() -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--engine-bin", type=Path, required=True)
    p.add_argument("--data-root", type=Path, required=True)
    p.add_argument("--artifact-dir", type=Path, required=True)
    a = p.parse_args()
    shutil.rmtree(a.artifact_dir, ignore_errors=True)
    a.artifact_dir.mkdir(parents=True)
    fixture = a.artifact_dir / "controls.txt"
    fixture.write_text(FIXTURE)
    env = {**os.environ, "HP2_INPUT_SELFTEST": "7.5", "HP2_CAPTURE_FRAMES": str(a.artifact_dir / "frames"), "HP2_CAPTURE_FRAME_INDICES": "300"}
    run = subprocess.run([sys.executable, "-B", str(ROOT / "Tests/InputScriptSmoke.py"), "--smoke", "--data-root", str(a.data_root),
                          "--engine-bin", str(a.engine_bin), "--map=Startup", f"--fixture={fixture}", "--ticks=420", "--timeout=180",
                          "--artifact-dir", str(a.artifact_dir / "run")], capture_output=True, text=True, env=env)
    failures = []
    if run.returncode != 0:
        failures.append(f"replay failed: {run.stdout[-400:]}{run.stderr[-400:]}")
    ini = next(iter(glob.glob(str(a.artifact_dir / "run/home-*/Library/Application Support/Harry Potter 2/User/User.ini"))), None)
    text = Path(ini).read_text(errors="replace") if ini else ""
    for want in ("MouseSensitivity=7.5", "bMaxMouseSmoothing=False"):
        if want not in text:
            failures.append(f"User.ini lacks {want}")
    if not list((a.artifact_dir / "frames").glob("frame_000300.png")):
        failures.append("no screenshot captured")
    print("\n".join(failures) or "controls page additions OK (review the screenshot)")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
