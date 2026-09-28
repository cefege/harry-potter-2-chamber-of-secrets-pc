#!/usr/bin/env python3
"""Drive the staged WebAssembly build in headless Chrome and report what it did.

Emits a report-schema-v1 JSON document:
    {schema: 1, name, status: pass|fail|blocked, invariant, reason_code?,
     data: {profile}, artifacts: [{kind, path}], command, exit_reason, page?}

Two modes, both running the real staged directory in ``dist/web-wasm64``:

- ``shell`` serves only the staged page and waits for it to report that the
  wasm64 module instantiated with IDBFS mounted. Data profile ``data-none``:
  the page is served without a game data root, so nothing imports.
- ``boot`` additionally serves a prototype data root under ``/dev-data/`` and
  waits for the engine to enter its main loop and render 300 frames. Data
  profile ``data-prototype``.

The page POSTs one JSON document to ``/__hp2_smoke`` (a route this harness
adds to the same handler ``Build/serve_web.py`` uses, so the served page sees
the same COOP/COEP headers as a real session). Exit codes: 0 pass, 1 fail,
2 blocked (no staged build, no data, or no browser).
"""

from __future__ import annotations

import argparse
import json
import os
import pwd
import shutil
import subprocess
import sys
import tempfile
import threading
from functools import partial
from http.server import ThreadingHTTPServer
from pathlib import Path
from typing import Any
from urllib.parse import quote

sys.path.insert(0, str(Path(__file__).resolve().parent))

import serve_web  # noqa: E402  - the served page must use the real dev handler

SCHEMA_VERSION = 1
SMOKE_PATH = "/__hp2_smoke"

EXIT_PASS = 0
EXIT_FAIL = 1
EXIT_BLOCKED = 2

DEFAULT_TIMEOUT = {"shell": 120, "boot": 900}

INVARIANTS = {
    "shell": (
        "the staged page is cross-origin isolated and instantiates the wasm64 "
        "module with IDBFS mounted"
    ),
    "boot": (
        "the staged page imports prototype data, enters the engine main loop, "
        "and runs 300 frames"
    ),
}

# ctest gives every behavioral test an isolated, package-less HOME (see
# hp2_add_behavior_test). That is right for the engine and wrong for the
# browser: macOS Chrome never finishes coming up in an empty HOME, so the
# page never loads and the smoke times out. $HOME *is* that isolated value
# here, so the real home directory is read from the password database
# instead. Chrome still gets a private --user-data-dir, and nothing under
# test reads user state: the page's data lives in OPFS and IDBFS inside that
# throwaway profile.
def real_home() -> str | None:
    try:
        return pwd.getpwuid(os.getuid()).pw_dir or None
    except (KeyError, OSError):
        return None


_BROWSER_HOME = real_home()

# The stage a mode must reach to pass, mirroring the page's reportSmoke calls.
PASS_STAGE = {"shell": "ready", "boot": "running"}

MACOS_BROWSERS = (
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    "/Applications/Chromium.app/Contents/MacOS/Chromium",
)
LINUX_BROWSERS = ("google-chrome", "google-chrome-stable", "chromium", "chromium-browser")


def find_browser(explicit: str | None) -> str | None:
    """Resolve a Chrome/Chromium binary, or None.

    An explicit --chrome that does not resolve is reported as missing rather
    than falling through to discovery: the caller asked for that binary.
    """
    if explicit:
        return explicit if _usable(explicit) else None
    env = os.environ.get("HP2_CHROME")
    if env:
        return env if _usable(env) else None
    for name in LINUX_BROWSERS:
        found = shutil.which(name)
        if found:
            return found
    for candidate in MACOS_BROWSERS:
        if _usable(candidate):
            return candidate
    return None


def _usable(path: str) -> bool:
    if os.path.isabs(path):
        return os.path.isfile(path) and os.access(path, os.X_OK)
    return shutil.which(path) is not None


def build_report(mode: str, status: str, command: str, exit_reason: str) -> dict[str, Any]:
    return {
        "schema": SCHEMA_VERSION,
        "name": f"web_smoke_{mode}",
        "status": status,
        "invariant": INVARIANTS[mode],
        "data": {"profile": "data-prototype" if mode == "boot" else "none"},
        "artifacts": [],
        "command": command,
        "exit_reason": exit_reason,
    }


def artifact_dir() -> Path | None:
    root = os.environ.get("HP2_ARTIFACT_DIR")
    return Path(root) if root else None


def write_report(report: dict[str, Any], output: Path | None) -> None:
    """Write the report to --output and/or the ctest artifact directory."""
    encoded = json.dumps(report, indent=2, sort_keys=True) + "\n"
    if output is not None:
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(encoded)
    root = artifact_dir()
    if root is not None:
        root.mkdir(parents=True, exist_ok=True)
        path = root / f"{report['name']}.json"
        path.write_text(encoded)
        report["artifacts"].append({"kind": "report", "path": str(path)})
    sys.stdout.write(encoded)


def make_handler(root: Path, data_root: Path | None, received: dict[str, Any]):
    """serve_web's handler plus the single smoke-report route."""
    base = serve_web.build_handler(root, data_root)

    class Handler(base):  # type: ignore[misc, valid-type]
        def do_POST(self) -> None:  # noqa: N802 - http.server API
            if self.path.split("?", 1)[0] != SMOKE_PATH:
                self.send_error(404, "not found")
                return
            length = int(self.headers.get("Content-Length") or 0)
            raw = self.rfile.read(length) if length else b""
            try:
                received["payload"] = json.loads(raw.decode("utf-8"))
            except (UnicodeDecodeError, ValueError) as error:
                received["payload"] = {"stage": "failed", "detail": f"unreadable report: {error}"}
            received["event"].set()
            self.send_response(204)
            self.send_header("Content-Length", "0")
            self.end_headers()

    return Handler


def run_smoke(
    mode: str,
    root: Path,
    data_root: Path | None,
    chrome: str,
    timeout: float,
    map_name: str,
) -> dict[str, Any] | None:
    """Serve the staged build, open it in headless Chrome, wait for one POST."""
    received: dict[str, Any] = {"payload": None, "event": threading.Event()}
    handler = make_handler(root, data_root, received)
    server = ThreadingHTTPServer(("127.0.0.1", 0), partial(handler, directory=str(root)))
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    port = server.server_address[1]
    profile_dir = tempfile.mkdtemp(prefix="hp2-web-smoke-")
    url = page_url(port, mode, map_name)
    # Chrome gets a private profile directory, and the real home directory
    # rather than the isolated test HOME (see _BROWSER_HOME).
    environment = dict(os.environ)
    if _BROWSER_HOME is not None:
        environment["HOME"] = _BROWSER_HOME
    process = subprocess.Popen(
        [
            chrome,
            "--headless=new",
            "--no-first-run",
            "--no-default-browser-check",
            "--disable-extensions",
            "--user-data-dir=" + profile_dir,
            url,
        ],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        env=environment,
    )
    try:
        received["event"].wait(timeout)
    finally:
        process.terminate()
        try:
            process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()
        server.shutdown()
        server.server_close()
        shutil.rmtree(profile_dir, ignore_errors=True)
    return received["payload"]


def page_url(port: int, mode: str, map_name: str) -> str:
    """The URL under test. shell imports nothing; boot drives the dev import."""
    if mode == "boot":
        return f"http://127.0.0.1:{port}/?smoke=boot&devdata&map={quote(map_name)}"
    return f"http://127.0.0.1:{port}/?smoke=shell"


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--mode", choices=sorted(INVARIANTS), required=True)
    parser.add_argument("--root", type=Path, default=Path("dist/web-wasm64"))
    parser.add_argument("--data-root", type=Path, default=None, help="required for --mode=boot")
    parser.add_argument("--map", default="PrivetDr.unr", help="map to launch in boot mode")
    parser.add_argument("--chrome", default=None, help="explicit Chrome/Chromium binary")
    parser.add_argument("--timeout", type=float, default=None)
    parser.add_argument("--output", type=Path, default=None)
    args = parser.parse_args(argv)

    command = "python3 Build/web_smoke.py " + " ".join(argv if argv is not None else sys.argv[1:])
    root = args.root.resolve()
    mode = args.mode
    report = build_report(mode, "fail", command, "")

    def blocked(reason_code: str, detail: str) -> int:
        report["status"] = "blocked"
        report["reason_code"] = reason_code
        report["exit_reason"] = detail
        write_report(report, args.output)
        return EXIT_BLOCKED

    if not (root / "index.html").is_file() or not (root / "hp2_game.wasm").is_file():
        return blocked("web_smoke.dist_missing", f"{root} is not a staged web build")

    data_root: Path | None = None
    if mode == "boot":
        if args.data_root is None:
            return blocked("web_smoke.data_missing", "--mode=boot requires --data-root")
        data_root = args.data_root.resolve()
        if not (data_root / "System" / "Default.ini").is_file():
            return blocked(
                "web_smoke.data_missing",
                f"{data_root} does not look like a game data root (no System/Default.ini)",
            )

    chrome = find_browser(args.chrome)
    if chrome is None:
        return blocked(
            "web_smoke.browser_missing",
            f"no usable Chrome/Chromium binary (looked for {args.chrome or 'the usual locations'})",
        )

    timeout = args.timeout if args.timeout is not None else DEFAULT_TIMEOUT[mode]
    payload = run_smoke(mode, root, data_root, chrome, timeout, args.map)
    if payload is None:
        report["reason_code"] = "web_smoke.timeout"
        report["exit_reason"] = f"the page reported nothing within {timeout:g}s"
        write_report(report, args.output)
        return EXIT_FAIL

    page = {key: value for key, value in payload.items() if key != "log"}
    report["page"] = page
    log_text = payload.get("log") or ""
    root_artifacts = artifact_dir()
    if root_artifacts is not None:
        root_artifacts.mkdir(parents=True, exist_ok=True)
        log_path = root_artifacts / f"{report['name']}.log"
        log_path.write_text(log_text + ("\n" if log_text and not log_text.endswith("\n") else ""))
        report["artifacts"].append({"kind": "log", "path": str(log_path)})

    stage = payload.get("stage")
    if stage == PASS_STAGE[mode]:
        report["status"] = "pass"
        report["exit_reason"] = str(payload.get("detail") or stage)
    elif stage == "failed":
        report["reason_code"] = "web_smoke.page_failed"
        report["exit_reason"] = str(payload.get("detail") or "the page reported a failure")
    else:
        report["reason_code"] = "web_smoke.page_failed"
        report["exit_reason"] = f"unexpected stage {stage!r}"
    write_report(report, args.output)
    return EXIT_PASS if report["status"] == "pass" else EXIT_FAIL


if __name__ == "__main__":
    raise SystemExit(main())
