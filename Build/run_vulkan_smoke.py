#!/usr/bin/env python3
"""Deterministic single-map Vulkan renderer smoke for HP2.

Wraps a gate-on (HP2_ENABLE_VULKAN_DRIVER=ON) hp2_game build with:
  - a generated Engine.ini selecting VulkanDrv.VulkanRenderDevice
    (ApplyPortableConfig preserves valid custom selections),
  - MoltenVK loader environment (VK_DRIVER_FILES + DYLD_LIBRARY_PATH),
  - fixed ticks/timeout, isolated HOME, structured schema-v1 report.

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
from pathlib import Path

def blocked(name: str, reason: str) -> int:
    print(json.dumps({"schema": 1, "name": name, "status": "blocked",
                      "reason_code": reason, "invariant":
                      "vulkan_renderer_boots_and_ticks_a_map",
                      "data": {"profile": "prototype"}, "artifacts": [],
                      "exit_reason": "prerequisite_missing"}))
    return 2

def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--build-dir", required=True, help="gate-on build dir containing hp2_game")
    ap.add_argument("--data-root", default="HarryPotter2/Unreal")
    ap.add_argument("--map", default="Entry")
    ap.add_argument("--ticks", type=int, default=60)
    ap.add_argument("--timeout", type=float, default=180)
    ap.add_argument("--icd", default="/opt/homebrew/etc/vulkan/icd.d/MoltenVK_icd.json")
    ap.add_argument("--name", default="renderer_smoke_vulkan")
    args = ap.parse_args()

    repo = Path(__file__).resolve().parent.parent
    binary = (Path(args.build_dir) / "hp2_game").resolve()
    data_root = (repo / args.data_root).resolve()
    icd = Path(args.icd)

    if not binary.is_file():
        return blocked(args.name, "renderer.binary_missing")
    if not icd.is_file():
        return blocked(args.name, "renderer.icd_missing")
    if not (data_root / "System" / "Default.ini").is_file():
        return blocked(args.name, "data.prototype_missing")

    work = Path(os.environ.get("HP2_ARTIFACT_DIR") or (Path.cwd() / "vk-smoke"))
    home = work / "Home"
    shutil.rmtree(work, ignore_errors=True)
    (home / "Library/Application Support/Harry Potter 2/User").mkdir(parents=True)

    engine_ini = (data_root / "System" / "Default.ini").read_text(errors="replace")
    for key in ("GameRenderDevice", "WindowedRenderDevice", "RenderDevice"):
        engine_ini = re.sub(rf"{key}=.*", f"{key}=VulkanDrv.VulkanRenderDevice", engine_ini)
    ini_path = work / "Engine.ini"
    ini_path.write_text(engine_ini)

    env = dict(os.environ)
    env.update({
        "HOME": str(home), "TMPDIR": str(work),
        "LC_ALL": "C", "TZ": "UTC",
        "DYLD_LIBRARY_PATH": "/opt/homebrew/lib",
        "VK_DRIVER_FILES": str(icd),
    })
    cmd = [str(binary), f"-datadir={data_root}", f"-INI={ini_path}",
           args.map, "-NOFRONTEND", "-window", "-nosound",
           f"-testticks={args.ticks}", "-log"]
    try:
        proc = subprocess.run(cmd, cwd=work, env=env,
                              timeout=args.timeout,
                              stdout=subprocess.PIPE,
                              stderr=subprocess.STDOUT)
        log_bytes = proc.stdout or b""
        rc, timed_out = proc.returncode, False
    except subprocess.TimeoutExpired as e:
        log_bytes = e.stdout or b""
        rc, timed_out = None, True

    log_path = work / "vulkan-smoke.log"
    log_path.write_bytes(log_bytes)
    text = log_bytes.decode("utf-8", errors="replace")

    failure = re.search(r"History:|General protection|Assertion|Access violation|"
                        r"Could not create vulkan renderer|Failed to initialize", text, re.I)
    device = re.search(r"Vulkan device:\s*(.+)", text)
    passed = (rc == 0) and (not timed_out) and bool(device) and not failure

    report = {
        "schema": 1, "name": args.name,
        "status": "pass" if passed else "fail",
        "invariant": "vulkan_renderer_boots_and_ticks_a_map",
        "reason_code": None if passed else (
            "process.timeout" if timed_out else
            ("engine.vulkan_device_absent" if not device else
             ("process.exit_status" if rc else "marker.failure"))),
        "data": {"profile": "prototype"},
        "artifacts": [{"kind": "log", "path": str(log_path)}],
        "command": cmd,
        "exit_reason": ("timeout" if timed_out else
                        f"exit_{rc}" if rc is not None else "unknown"),
        "device": device.group(1).strip() if device else None,
        "ticks_requested": args.ticks,
    }
    print(json.dumps(report))
    (work / "vulkan-smoke-report.json").write_text(json.dumps(report, indent=2))
    return 0 if passed else 1

if __name__ == "__main__":
    sys.exit(main())
