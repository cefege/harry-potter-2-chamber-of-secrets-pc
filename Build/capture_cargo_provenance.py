#!/usr/bin/env python3
"""Capture the real Cargo invocation launched by a Corrosion custom target."""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import threading
from io import BytesIO
from typing import Any
from typing import BinaryIO

ENVIRONMENT_KEYS = {
    "AR", "CC", "CFLAGS", "CMAKE_PREFIX_PATH", "CPATH", "CPPFLAGS", "CXX", "HOME",
    "CXXFLAGS", "DYLD_LIBRARY_PATH", "LDFLAGS", "LIBRARY_PATH",
    "MACOSX_DEPLOYMENT_TARGET", "OBJCFLAGS", "OBJCXXFLAGS", "PATH",
    "PKG_CONFIG_LIBDIR", "PKG_CONFIG_PATH", "SDKROOT", "TARGET", "RUSTC",
    "RUSTC_WRAPPER", "RUSTFLAGS", "RUSTDOC", "RUSTUP_HOME", "RUSTUP_TOOLCHAIN", "CARGO_BUILD_TARGET",
    "CARGO_BUILD_RUSTC", "CARGO_BUILD_RUSTC_WRAPPER", "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER",
    "CARGO_HOME", "CARGO_ENCODED_RUSTFLAGS",
    "CARGO_NET_OFFLINE", "CARGO_INCREMENTAL", "BINDGEN_EXTRA_CLANG_ARGS", "CORROSION_BUILD_DIR",
}


def _recorded_environment(environment: dict[str, str]) -> dict[str, str]:
    return {
        key: value for key, value in sorted(environment.items())
        if key in ENVIRONMENT_KEYS
        or key.startswith((
            "AR_", "CC_", "CFLAGS_", "CXX_", "CXXFLAGS_", "CARGO_TARGET_", "CARGO_PROFILE_", "RUSTC_",
        ))
    }





def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def _absolute(path: str, cwd: Path) -> Path:
    candidate = Path(path)
    if not candidate.is_absolute():
        candidate = cwd / candidate
    return candidate.resolve(strict=True)


def _digest_tree(out_dirs: set[Path]) -> dict[str, str]:
    hashes: dict[str, str] = {}
    for out_dir in sorted(out_dirs):
        real_out_dir = out_dir.resolve(strict=True)
        if not real_out_dir.is_dir():
            raise RuntimeError(f"Cargo build-script output directory is missing: {out_dir}")
        for path in sorted(real_out_dir.rglob("*")):
            if path.is_file():
                real_path = path.resolve(strict=True)
                if not real_path.is_relative_to(real_out_dir):
                    raise RuntimeError(f"Cargo build-script output escapes its observed out_dir: {path}")
                hashes[str(real_path)] = _sha256(real_path)
    return hashes


def _cargo_messages(stdout: bytes) -> list[dict[str, Any]]:
    messages: list[dict[str, Any]] = []
    for number, line in enumerate(stdout.splitlines(), 1):
        if not line.strip():
            continue
        try:
            message = json.loads(line)
        except (UnicodeDecodeError, json.JSONDecodeError) as error:
            raise RuntimeError(f"Cargo JSON stream contains non-JSON stdout on line {number}") from error
        if isinstance(message, dict) and isinstance(message.get("reason"), str):
            messages.append(message)
        elif isinstance(message, dict):
            # Cargo's --message-format=json-render-diagnostics protocol is JSON
            # per line; retain every object, not a synthesized event subset.
            messages.append(message)
        else:
            raise RuntimeError(f"Cargo emitted non-object JSON on stdout line {number}")
    return messages


def _final_bin_module():
    spec = importlib.util.spec_from_file_location("capture_final_bin", Path(__file__).with_name("capture_final_bin.py"))
    if spec is None or spec.loader is None:
        raise RuntimeError("cannot load final-binary capture helper")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _capture(args: argparse.Namespace, cargo_argv: list[str], child_env: dict[str, str]) -> int:
    cwd = Path.cwd().resolve()
    artifact_dir = Path(args.record).resolve().parent
    artifact_dir.mkdir(parents=True, exist_ok=True)
    child_env["HP2_ARTIFACT_DIR"] = str(artifact_dir)
    selected_environment = _recorded_environment(child_env)
    original_environment = dict(child_env)
    mode = getattr(args, "mode", "schema3")
    if mode == "final-bin-v1":
        # Cargo's merged config includes build settings such as CARGO_BUILD_JOBS.
        # Preserve them for the qualification replay without changing schema 3.
        selected_environment.update({key: value for key, value in child_env.items()
                                     if key.startswith("CARGO_BUILD_")})
    if mode not in {"schema3", "final-bin-v1"}:
        print(f"unsupported Cargo provenance mode: {mode}", file=sys.stderr)
        return 1
    final_bin = None
    context = None
    if mode == "final-bin-v1":
        # Never leave an earlier successful qualification record on failure.
        Path(args.record).write_text(json.dumps({"schema_version": 4, "status": 1,
                                                "capture_error": "capture did not complete"}) + "\n", encoding="utf-8")
        try:
            final_bin = _final_bin_module()
            context, child_env = final_bin.prepare(cargo_argv, cwd, child_env, artifact_dir, args.target)
        except (OSError, ValueError, RuntimeError) as error:
            # Invalidate any earlier successful producer record before returning.
            failure = {"schema_version": 4, "selected_target": args.target, "status": 1,
                       "capture_error": f"final-binary qualification unsupported: {error}"}
            Path(args.record).write_text(json.dumps(failure, indent=2) + "\n", encoding="utf-8")
            print(f"final-binary qualification unsupported: {error}", file=sys.stderr)
            return 1
    stdout_capture = BytesIO()
    stderr_capture = BytesIO()
    try:
        popen = subprocess.Popen(cargo_argv, cwd=cwd, env=child_env, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    except OSError as error:
        empty = b""
        (artifact_dir / "cargo.stdout").write_bytes(empty)
        (artifact_dir / "cargo.stderr").write_bytes(empty)
        record = {
            "schema_version": 4 if final_bin else 3,
            "selected_target": args.target,
            "cargo_argv": cargo_argv,
            "cwd": str(cwd),
            "environment": selected_environment,
            "status": 127,
            "stdout_sha256": hashlib.sha256(empty).hexdigest(),
            "stderr_sha256": hashlib.sha256(empty).hexdigest(),
            "messages": [],
            "artifact_sha256": {},
            "build_script_output_sha256": {},
            "capture_error": f"unable to start Cargo: {error}",
        }
        Path(args.record).write_text(json.dumps(record, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        print(f"Cargo provenance capture failed: {error}", file=sys.stderr)
        return 127

    def pump(source: BinaryIO, destination: BinaryIO, forward: BinaryIO, capture: BytesIO) -> None:
        while True:
            chunk = source.read(64 * 1024)
            if not chunk:
                break
            destination.write(chunk)
            destination.flush()
            forward.write(chunk)
            forward.flush()
            capture.write(chunk)

    with (artifact_dir / "cargo.stdout").open("wb") as stdout_file, (artifact_dir / "cargo.stderr").open("wb") as stderr_file:
        stdout_thread = threading.Thread(target=pump, args=(popen.stdout, stdout_file, sys.stdout.buffer, stdout_capture))
        stderr_thread = threading.Thread(target=pump, args=(popen.stderr, stderr_file, sys.stderr.buffer, stderr_capture))
        stdout_thread.start()
        stderr_thread.start()
        stdout_thread.join()
        stderr_thread.join()
    status = popen.wait()
    result = subprocess.CompletedProcess(cargo_argv, status, stdout_capture.getvalue(), stderr_capture.getvalue())

    messages: list[dict[str, Any]] = []
    artifacts: dict[str, str] = {}
    build_script_outputs: dict[str, str] = {}
    parse_error: str | None = None
    try:
        messages = _cargo_messages(result.stdout)
        if result.returncode == 0:
            for message in messages:
                reason = message.get("reason")
                if reason == "compiler-artifact":
                    for filename in message.get("filenames", []):
                        path = _absolute(filename, cwd)
                        if path.is_file():
                            artifacts[str(path)] = _sha256(path)
                        else:
                            raise RuntimeError(f"Cargo reported missing compiler artifact: {path}")
                elif reason == "build-script-executed":
                    out_dir = _absolute(message["out_dir"], cwd)
                    build_script_outputs.update(_digest_tree({out_dir}))
                    # Cargo writes the build script's emitted directives to
                    # output beside (not inside) its OUT_DIR.
                    output = out_dir.parent / "output"
                    if output.is_symlink() or not output.is_file():
                        raise RuntimeError(f"Cargo build-script output file is missing or unsafe: {output}")
                    real_output = output.resolve(strict=True)
                    if not real_output.is_relative_to(out_dir.parent.resolve(strict=True)):
                        raise RuntimeError(f"Cargo build-script output escapes its build directory: {output}")
                    build_script_outputs[str(real_output)] = _sha256(real_output)
        if not any(message.get("reason") == "build-finished" for message in messages):
            raise RuntimeError("Cargo JSON stream is missing build-finished")
    except (KeyError, OSError, RuntimeError, TypeError, ValueError) as error:
        parse_error = str(error)

    final_evidence = None
    if final_bin and parse_error is None and result.returncode == 0:
        try:
            final_evidence = final_bin.finish(context, messages, original_environment)
        except (KeyError, OSError, ValueError, final_bin.Unsupported) as error:
            parse_error = f"final-binary qualification incomplete: {error}"

    record = {
        "schema_version": 4 if final_bin else 3,
        "selected_target": args.target,
        "cargo_argv": cargo_argv,
        "cwd": str(cwd),
        "environment": selected_environment,
        "status": result.returncode,
        "stdout_sha256": hashlib.sha256(result.stdout).hexdigest(),
        "stderr_sha256": hashlib.sha256(result.stderr).hexdigest(),
        "messages": messages,
        "artifact_sha256": artifacts,
        "build_script_output_sha256": build_script_outputs,
    }
    if final_bin:
        record["final_bin"] = final_evidence
    if parse_error is not None:
        record["capture_error"] = parse_error
    record_path = Path(args.record)
    record_path.parent.mkdir(parents=True, exist_ok=True)
    record_path.write_text(json.dumps(record, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    if parse_error is not None:
        print(f"Cargo provenance capture incomplete: {parse_error}", file=sys.stderr)
        return result.returncode if result.returncode else 1
    return result.returncode


def main() -> int:
    target = os.environ.pop("HP2_CARGO_PROVENANCE_TARGET", None)
    record = os.environ.pop("HP2_CARGO_PROVENANCE_RECORD", None)
    cargo_executable = os.environ.pop("HP2_CARGO_PROVENANCE_EXECUTABLE", None)
    mode = os.environ.pop("HP2_CARGO_PROVENANCE_MODE", "schema3")
    if not target or not record or not cargo_executable:
        print("Cargo provenance proxy was invoked without Corrosion target configuration", file=sys.stderr)
        return 2
    child_env = os.environ.copy()
    cargo_argv = [cargo_executable, *sys.argv[1:]]
    if not sys.argv[1:] or sys.argv[1] != "rustc":
        return subprocess.run(cargo_argv, env=child_env, check=False).returncode
    args = argparse.Namespace(target=target, record=record, mode=mode)
    return _capture(args, cargo_argv, child_env)


if __name__ == "__main__":
    raise SystemExit(main())
