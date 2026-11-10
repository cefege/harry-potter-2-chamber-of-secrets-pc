#!/usr/bin/env python3
"""Pixel-equivalence check between two engine builds (or two runs of one build).

Runs each map with a fixed step, captures PNG frames at fixed indices, and records
SHA-256 + size of every PNG. ``compare`` lists differing frames. Equal hashes mean
byte-identical captures; they prove nothing about frames that were not captured.
By default any differing decoded pixel fails; ``--max-delta``/``--max-fraction`` accept a
stated tolerance (reported as WITHIN-TOLERANCE, never as identical).

  frame_equiv.py run  --engine-bin B --data-root D --out DIR [--maps a,b] [--frames 30,120,240]
  frame_equiv.py compare A.json B.json
"""
import argparse, hashlib, json, os, sys
from pathlib import Path
sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))
import game_test, perf_baseline

DEFAULT_MAPS = "PrivetDr,Entry,Ch2Skurge,Adv1Willow,Entryhall_hub,Grounds_hub,GreatHall_G,Quidditch,Duel01"

def run(a):
    root = a.data_root.resolve(strict=True); out = a.out; out.mkdir(parents=True, exist_ok=True)
    frames = [int(x) for x in a.frames.split(",")]
    result = {"engine": str(a.engine_bin), "frames": frames, "maps": {}}
    for name in a.maps.split(","):
        fdir = out / name
        ini = perf_baseline.uncapped_ini(root, out / f"{name}.ini", "wgpu")
        env = {"HP2_CAPTURE_FRAMES": str(fdir), "HP2_CAPTURE_FRAME_INDICES": ",".join(map(str, frames)),
               "HP2_CAPTURE_MAP": name, "HP2_CAPTURE_TICKS": str(max(frames) + 1)}
        r = game_test.run_game(app=a.engine_bin, engine_bin=a.engine_bin, data_root=root, selected_map=f"Maps/{name}.unr",
                               renderer="wgpu", ticks=max(frames) + 2, timeout_seconds=300, log_path=out / f"{name}.log",
                               no_sound=True, extra_env=env, ini_file=ini, fixed_dt=1 / 60)
        if r.get("exit_status") != 0 or r.get("failure_markers"):
            raise SystemExit(f"{name}: unclean run {r.get('exit_status')} {r.get('failure_markers')}")
        shots = {}
        for f in frames:
            p = fdir / f"frame_{f:06}.png"
            shots[str(f)] = {"sha256": hashlib.sha256(p.read_bytes()).hexdigest(), "bytes": p.stat().st_size} if p.is_file() else None
        result["maps"][name] = shots
        print(name, {k: (v["sha256"][:10] if v else None) for k, v in shots.items()}, flush=True)
    (out / "frames.json").write_text(json.dumps(result, indent=2) + "\n")

def pixel_diff(a_path, b_path):
    """(differing pixels, total pixels, max channel delta); needs PIL."""
    from PIL import Image, ImageChops
    x = Image.open(a_path).convert("RGB"); y = Image.open(b_path).convert("RGB")
    if x.size != y.size: return (-1, 0, 255)
    diff = ImageChops.difference(x, y)
    data = list(diff.getdata())
    count = sum(1 for px in data if px != (0, 0, 0))
    if count == 0: return (0, x.size[0] * x.size[1], 0)
    return (count, x.size[0] * x.size[1], max(max(px) for px in data))


def compare(a):
    fa, fb = Path(a.first), Path(a.second)
    x = json.loads(fa.read_text()); y = json.loads(fb.read_text())
    bad = 0
    for name, shots in x["maps"].items():
        for f, s in shots.items():
            t = y["maps"].get(name, {}).get(f)
            same = s is not None and t is not None and s["sha256"] == t["sha256"]
            line = ("SAME " if same else "DIFF ") + f"{name} frame {f}"
            if not same:
                try:
                    n, total, mx = pixel_diff(fa.parent / name / f"frame_{int(f):06}.png", fb.parent / name / f"frame_{int(f):06}.png")
                    line += f"  pixels differing {n}/{total} ({100.0 * n / max(total, 1):.4f}%), max channel delta {mx}"
                    if n == 0:
                        # PNG bytes differ but decoded pixels are identical (encoder metadata only).
                        line = line.replace("DIFF ", "SAME-PIXELS ", 1)
                    elif n <= a.max_fraction * total and mx <= a.max_delta:
                        line = line.replace("DIFF ", "WITHIN-TOLERANCE ", 1)
                    else:
                        bad += 1
                except Exception as error:  # a frame that cannot be compared is a failure
                    bad += 1
                    line += f"  (pixel diff unavailable: {error})"
            print(line)
    print("differences:", bad)
    return 1 if bad else 0

p = argparse.ArgumentParser(); sub = p.add_subparsers(dest="cmd", required=True)
r = sub.add_parser("run"); r.add_argument("--engine-bin", type=Path, required=True); r.add_argument("--data-root", type=Path, required=True)
r.add_argument("--out", type=Path, required=True); r.add_argument("--maps", default=DEFAULT_MAPS); r.add_argument("--frames", default="30,120,240")
c = sub.add_parser("compare"); c.add_argument("first"); c.add_argument("second")
c.add_argument("--max-delta", type=int, default=0, help="largest per-channel difference accepted for a differing pixel")
c.add_argument("--max-fraction", type=float, default=0.0, help="largest fraction of differing pixels accepted per frame")
a = p.parse_args()
sys.exit(run(a) or 0 if a.cmd == "run" else compare(a))
