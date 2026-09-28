#!/usr/bin/env python3
"""Serve the staged WebAssembly build for local browser testing.

The page must be cross-origin isolated, so every response carries COOP/COEP
plus a no-store cache policy. ``--data-root`` additionally exposes the game
data under ``/dev-data/`` so the shell's dev import button can read it without
the player picking a folder by hand. That route is opt-in and path-checked: the
build never hosts game data by default.
"""
from __future__ import annotations


import argparse
import json
import mimetypes
import sys
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import unquote

DEV_DATA_PREFIX = "/dev-data/"


def build_handler(root: Path, data_root: Path | None):
    class Handler(SimpleHTTPRequestHandler):
        extensions_map = {
            **SimpleHTTPRequestHandler.extensions_map,
            ".wasm": "application/wasm",
            ".js": "text/javascript",
        }

        def end_headers(self) -> None:
            self.send_header("Cross-Origin-Opener-Policy", "same-origin")
            self.send_header("Cross-Origin-Embedder-Policy", "require-corp")
            self.send_header("Cross-Origin-Resource-Policy", "same-origin")
            self.send_header("Cache-Control", "no-store")
            super().end_headers()

        def do_GET(self) -> None:  # noqa: N802 - http.server API
            if self.path.startswith(DEV_DATA_PREFIX):
                if not self._serve_dev_data():
                    return
                return
            super().do_GET()

        def do_HEAD(self) -> None:  # noqa: N802 - http.server API
            if self.path.startswith(DEV_DATA_PREFIX):
                if not self._serve_dev_data(head_only=True):
                    return
                return
            super().do_HEAD()

        def _serve_dev_data(self, head_only: bool = False) -> bool:
            if data_root is None:
                self.send_error(404, "dev data is disabled; start with --data-root")
                return False
            if self.path == "/dev-data/manifest.json":
                self._send_json(self._manifest())
                return False
            relative = unquote(self.path[len(DEV_DATA_PREFIX):].split("?", 1)[0])
            target = self._resolve_dev_path(relative)
            if target is None:
                self.send_error(404, "not found")
                return False
            if not target.is_file():
                self.send_error(404, "not a file")
                return False
            try:
                self._send_file(target, head_only)
            except OSError as err:
                # A file that vanished between manifest and request is a 404,
                # not a dropped connection.
                self.send_error(404, "not readable: %s" % err)
            return False

        def _resolve_dev_path(self, relative: str) -> Path | None:
            if not relative:
                return None
            candidate = (data_root / relative).resolve()
            try:
                candidate.relative_to(data_root)
            except ValueError:
                return None
            return candidate

        def _manifest(self) -> list[str]:
            paths = []
            for path in sorted(data_root.rglob("*")):
                if path.is_file() and not path.is_symlink():
                    paths.append(path.relative_to(data_root).as_posix())
            return paths

        def _send_json(self, payload: object) -> None:
            body = json.dumps(payload, indent=1).encode("utf-8")
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def _send_file(self, path: Path, head_only: bool) -> None:
            content_type = mimetypes.guess_type(path.name)[0] or "application/octet-stream"
            size = path.stat().st_size
            self.send_response(200)
            self.send_header("Content-Type", content_type)
            self.send_header("Content-Length", str(size))
            self.end_headers()
            if not head_only:
                with path.open("rb") as handle:
                    self.copyfile(handle, self.wfile)

        def log_message(self, fmt: str, *args: object) -> None:
            sys.stderr.write("%s - %s\n" % (self.address_string(), fmt % args))

    return Handler


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", default="dist/web-wasm64", help="directory to serve")
    parser.add_argument("--port", type=int, default=8080)
    parser.add_argument("--bind", default="127.0.0.1")
    parser.add_argument(
        "--data-root",
        default=None,
        help="optional game data root exposed under /dev-data/ for local testing",
    )
    args = parser.parse_args()

    root = Path(args.root).resolve()
    if not root.is_dir():
        parser.error(f"--root is not a directory: {args.root}")

    data_root: Path | None = None
    if args.data_root:
        data_root = Path(args.data_root).resolve()
        if not data_root.is_dir():
            parser.error(f"--data-root is not a directory: {args.data_root}")

    handler = build_handler(root, data_root)
    server = ThreadingHTTPServer((args.bind, args.port), partial(handler, directory=str(root)))
    print(f"Serving {root} at http://{args.bind}:{args.port}/", flush=True)
    if data_root is not None:
        print(f"Dev data served from {data_root} under /dev-data/", flush=True)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
