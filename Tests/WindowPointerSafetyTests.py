#!/usr/bin/env python3
"""Compile-only check of the SDL window pointer's Rust safety contract.

No compiled probe is linked or executed. Run separately for each source
snapshot and preserve the resulting evidence directory.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--rustc", default="rustc")
    args = parser.parse_args()
    artifact_root = os.environ.get("HP2_ARTIFACT_DIR")
    if not artifact_root:
        parser.error("HP2_ARTIFACT_DIR is required")
    root = Path(artifact_root).resolve()
    root.mkdir(parents=True, exist_ok=True)
    if args.output is None:
        output = Path(tempfile.mkdtemp(prefix="window-pointer-safety-", dir=root))
    else:
        output = args.output.resolve()
        if output == root or root not in output.parents:
            parser.error("--output must be a new directory under HP2_ARTIFACT_DIR")
        output.mkdir(parents=True, exist_ok=False)
    source = args.source.resolve(strict=True)
    copied = output / "window_lifecycle.rs"
    shutil.copyfile(source, copied)
    digest = hashlib.sha256(copied.read_bytes()).hexdigest()
    (output / "source.sha256").write_text(f"{digest}  window_lifecycle.rs\n", encoding="utf-8")

    template = '''//! Compile-only window pointer contract probe.
pub mod viewport { pub const SDL_WINDOW_RESIZABLE: u32 = 0x20; }
pub mod window_lifecycle;
pub fn probe(window: *mut window_lifecycle::SdlWindow) -> u32 {
    CALL
}
'''
    call_line = next(index for index, line in enumerate(template.splitlines(), 1)
                     if "CALL" in line)
    calls = {
        "positive": "unsafe { window_lifecycle::launcher_window_flags(window) }",
        "negative": "window_lifecycle::launcher_window_flags(window)",
    }
    results = {}
    for name, call in calls.items():
        probe_dir = output / name
        probe_dir.mkdir()
        shutil.copyfile(copied, probe_dir / "window_lifecycle.rs")
        probe = probe_dir / "probe.rs"
        probe.write_text(template.replace("CALL", call), encoding="utf-8")
        command = [args.rustc, "--edition=2024", "--crate-type=lib", "--emit=metadata",
                   "--cfg", 'feature="sdl"', "--error-format=json", str(probe),
                   "-o", str(probe_dir / "probe.rmeta")]
        completed = subprocess.run(command, capture_output=True, text=True, check=False)
        (probe_dir / "stdout.txt").write_text(completed.stdout, encoding="utf-8")
        (probe_dir / "stderr.jsonl").write_text(completed.stderr, encoding="utf-8")
        (probe_dir / "command.json").write_text(json.dumps(command, indent=2) + "\n", encoding="utf-8")
        diagnostics = []
        for line in completed.stderr.splitlines():
            try:
                diagnostics.append(json.loads(line))
            except json.JSONDecodeError:
                pass
        def is_expected(item):
            return (item.get("level") == "error"
                    and (item.get("code") or {}).get("code") == "E0133"
                    and "launcher_window_flags" in item.get("message", "")
                    and any(span.get("is_primary")
                            and Path(span.get("file_name", "")).resolve() == probe
                            and span.get("line_start") == call_line
                            for span in item.get("spans", [])))

        coded_errors = [item for item in diagnostics if item.get("level") == "error"
                        and (item.get("code") or {}).get("code")]
        expected_error = any(is_expected(item) for item in coded_errors)
        unrelated_errors = [item for item in coded_errors if not is_expected(item)]
        results[name] = {"returncode": completed.returncode, "expected_e0133": expected_error,
                         "unrelated_coded_errors": len(unrelated_errors), "call_line": call_line}
    passed = (results["positive"]["returncode"] == 0
              and results["negative"]["returncode"] != 0
              and results["negative"]["expected_e0133"]
              and results["negative"]["unrelated_coded_errors"] == 0)
    report = {"source": str(source), "source_sha256": digest, "output": str(output),
              "results": results, "passed": passed}
    (output / "result.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(report, indent=2))
    return 0 if passed else 1


if __name__ == "__main__":
    sys.exit(main())
