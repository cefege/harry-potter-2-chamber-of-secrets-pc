#!/usr/bin/env python3
"""Deterministic single-map wgpu renderer smoke for HP2.

Wraps a gate-on (HP2_ENABLE_WGPU_DRIVER=ON) hp2_game build with:
  - a generated Engine.ini selecting WgpuDrv.WgpuRenderDevice
    (ApplyPortableConfig preserves valid custom selections),
  - fixed ticks/timeout, isolated HOME, structured schema-v1 report.

No ICD/loader environment needed unlike run_vulkan_smoke.py: wgpu's
Metal backend talks to the system Metal framework directly, nothing
to point at. Mirrors run_vulkan_smoke.py's structure closely since
the two scripts solve the same problem for two different experimental
render devices.

Exit codes: 0 pass, 1 fail, 2 blocked(reason).
"""
from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path


def blocked(name: str, reason: str) -> int:
    print(json.dumps({"schema": 1, "name": name, "status": "blocked",
                      "invariant": "wgpu_renderer_boots_and_ticks_a_map",
                      "reason_code": reason, "data": {}, "artifacts": [],
                      "exit_reason": "prerequisite_missing"}))
    return 2


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--build-dir", required=True, help="gate-on build dir containing hp2_game")
    ap.add_argument("--data-root", default="HarryPotter2/Unreal")
    ap.add_argument("--map", default="Entry")
    ap.add_argument("--ticks", type=int, default=60)
    ap.add_argument("--timeout", type=float, default=180)
    ap.add_argument("--name", default="renderer_smoke_wgpu")
    args = ap.parse_args()

    repo = Path(__file__).resolve().parent.parent
    binary = (Path(args.build_dir) / "hp2_game").resolve()
    data_root = (repo / args.data_root).resolve()

    if not binary.is_file():
        return blocked(args.name, "renderer.binary_missing")
    if not (data_root / "System" / "Default.ini").is_file():
        return blocked(args.name, "data.prototype_missing")

    # `base` is the pre-created directory owned by the caller (ctest's
    # per-test HP2_ARTIFACT_DIR, or a fresh tempdir for a manual run --
    # never cwd, which would write into the repo working tree for a
    # manual run from the repo root). `work` is always a subdirectory
    # this script creates and owns itself, never `base` directly: an
    # earlier version of this script used `base` itself as `work` and
    # called shutil.rmtree(work) on it, which would have deleted
    # ctest's own pre-created artifact directory out from under it
    # (caught on review, not self-caught).
    base = Path(os.environ.get("HP2_ARTIFACT_DIR") or tempfile.mkdtemp(prefix="hp2-wgpu-smoke-"))
    work = base / "wgpu-smoke"
    home = work / "Home"
    shutil.rmtree(work, ignore_errors=True)
    (home / "Library/Application Support/Harry Potter 2/User").mkdir(parents=True)

    engine_ini = (data_root / "System" / "Default.ini").read_text(errors="replace")
    for key in ("GameRenderDevice", "WindowedRenderDevice", "RenderDevice"):
        engine_ini = re.sub(rf"{key}=.*", f"{key}=WgpuDrv.WgpuRenderDevice", engine_ini)
    ini_path = work / "Engine.ini"
    ini_path.write_text(engine_ini)

    env = dict(os.environ)
    env["HOME"] = str(home)
    env["TMPDIR"] = str(work)

    cmd = [str(binary), f"-datadir={data_root}", f"-INI={ini_path}",
           args.map, "-NOFRONTEND", "-window", "-nosound",
           f"-testticks={args.ticks}", "-log"]
    try:
        proc = subprocess.run(cmd, cwd=work, env=env,
                               capture_output=True, timeout=args.timeout)
        log_bytes = proc.stdout + proc.stderr
        rc, timed_out = proc.returncode, False
    except subprocess.TimeoutExpired as e:
        log_bytes = e.stdout or b""
        rc, timed_out = None, True

    log_path = work / "wgpu-smoke.log"
    log_path.write_bytes(log_bytes)
    text = log_bytes.decode("utf-8", errors="replace")

    failure = re.search(r"History:|General protection|Assertion|Access violation|"
                        r"no compatible GPU adapter|Failed to initialize", text, re.I)
    adapter = re.search(r'WgpuDrv: adapter "(.+?)" backend (\S+)', text)

    # Last "WgpuDrv: Lock outcome=... tally presented=N occluded=M
    # outdated=K other_failure=J" line, if any: the running cumulative
    # tally as of the final tick, logged by WgpuDrv.cpp's own Lock on
    # every call (not just Exit, which is confirmed NOT reached in a
    # -testticks run -- WgpuDrv.h's own doc on why). A real -window
    # run with zero presented frames is not the same claim as "the
    # renderer works": this script hardcodes -window above, so every
    # run here is exactly the scenario that distinction matters for
    # (caught on review: an earlier version of this script reported
    # "pass" on a run that, per this same tally, presented zero frames
    # the entire time).
    tally_matches = list(re.finditer(
        r"WgpuDrv: Lock outcome=(\d+) tally presented=(\d+) occluded=(\d+) "
        r"outdated=(\d+) other_failure=(\d+)", text))
    tally = None
    if tally_matches:
        last = tally_matches[-1]
        tally = {
            "last_outcome": int(last.group(1)),
            "presented": int(last.group(2)),
            "occluded": int(last.group(3)),
            "outdated": int(last.group(4)),
            "other_failure": int(last.group(5)),
            "ticks_logged": len(tally_matches),
        }

    booted = (rc == 0) and (not timed_out) and bool(adapter) and not failure
    presented_frames = bool(tally and tally["presented"] > 0)
    # "pass" needs more than "did not crash while trying": a real
    # -window run (hardcoded above) that booted cleanly but never
    # actually presented a frame is a known, honestly-reported
    # condition (WgpuDrv.h's own doc on a real such run on this
    # project's own dev machine), not silently treated as success.
    passed = booted and presented_frames

    if booted and not presented_frames:
        status = "blocked"
        reason_code = "renderer.zero_frames_presented"
    elif not booted:
        status = "fail"
        reason_code = (
            "process.timeout" if timed_out else
            ("engine.wgpu_adapter_absent" if not adapter else
             ("process.exit_status" if rc else "marker.failure")))
    else:
        status = "pass"
        reason_code = None

    report = {
        "schema": 1, "name": args.name,
        "status": status,
        "invariant": "wgpu_renderer_boots_and_ticks_a_map",
        "reason_code": reason_code,
        "data": {"profile": "prototype", "frame_tally": tally},
        "artifacts": [{"kind": "log", "path": str(log_path)}],
        "command": cmd,
        "exit_reason": ("timeout" if timed_out else
                        f"exit_{rc}" if rc is not None else "unknown"),
        "adapter": adapter.group(1).strip() if adapter else None,
        "backend": adapter.group(2).strip() if adapter else None,
        "ticks_requested": args.ticks,
    }
    print(json.dumps(report))
    (work / "wgpu-smoke-report.json").write_text(json.dumps(report, indent=2))
    return 0 if passed else (2 if status == "blocked" else 1)


if __name__ == "__main__":
    sys.exit(main())
