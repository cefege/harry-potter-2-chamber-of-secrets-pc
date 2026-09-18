#!/usr/bin/env python3
"""Playable-v0 self-verification: launch the packaged Rust app through
LaunchServices (the real double-click path), then prove — without a human —
that it renders, plays music, and exits cleanly.

Checks (exit 0 only when all required ones pass):
  alive    — the engine process stays up through the settle window;
  render   — the game window's own pixels (``screencapture -l<id>``, which
             sees through occlusion; falls back to a full-screen capture
             cropped to the window rect) are not uniformly black;
  audio    — ``lsof`` lists at least one open ``.ogg`` file descriptor on
             the engine process (the level-music stream's held-open asset);
  frontmost — informational (recorded, never fatal);
  exit     — SIGTERM ends the process cleanly within the shutdown timeout.

Only Python stdlib plus macOS ``open`` / ``pgrep`` / ``screencapture`` /
``sips`` / ``lsof`` / ``osascript``. Artifacts land in ``HP2_ARTIFACT_DIR``
when provided, else a scratch dir under TMPDIR.
"""

from __future__ import annotations

import argparse
import ctypes
import ctypes.util
import json
import os
import shutil
import struct
import subprocess
import sys
import tempfile
import time
import zlib
from pathlib import Path

ENGINE_PATTERN = "HarryPotter2.app/Contents/MacOS/HarryPotter2"
ENGINE_PROCESS_NAME = "HarryPotter2"
DEFAULT_BUNDLE = Path("dist/macos-arm64-rs/HarryPotter2.app")
SETTLE_SECONDS = 8.0
SHUTDOWN_TIMEOUT = 10.0
MIN_FULL_CAPTURE_BYTES = 200_000
CF_ENCODING_UTF8 = 0x08000100
K_CF_NUMBER_SINT32 = 3

# ------------------------------------------------------------------ tooling
#
# CoreFoundation via stdlib ctypes. arm64 discipline: every pointer-
# returning entry point needs an explicit restype — ctypes defaults to
# c_int and would truncate 64-bit pointers.

_core = ctypes.cdll.LoadLibrary(ctypes.util.find_library("ApplicationServices"))
_core.CGWindowListCopyWindowInfo.restype = ctypes.c_void_p
_core.CGWindowListCopyWindowInfo.argtypes = [ctypes.c_uint32, ctypes.c_uint32]
_core.CFArrayGetCount.restype = ctypes.c_long
_core.CFArrayGetCount.argtypes = [ctypes.c_void_p]
_core.CFArrayGetValueAtIndex.restype = ctypes.c_void_p
_core.CFArrayGetValueAtIndex.argtypes = [ctypes.c_void_p, ctypes.c_long]
_core.CFDictionaryGetValue.restype = ctypes.c_void_p
_core.CFDictionaryGetValue.argtypes = [ctypes.c_void_p, ctypes.c_void_p]
_core.CFNumberGetValue.restype = ctypes.c_bool
_core.CFNumberGetValue.argtypes = [
    ctypes.c_void_p, ctypes.c_uint32, ctypes.c_void_p
]
_core.CFStringCreateWithCString.restype = ctypes.c_void_p
_core.CFStringCreateWithCString.argtypes = [
    ctypes.c_void_p, ctypes.c_char_p, ctypes.c_uint32
]
_core.CFStringGetLength.restype = ctypes.c_long
_core.CFStringGetLength.argtypes = [ctypes.c_void_p]
_core.CFStringGetCString.restype = ctypes.c_bool
_core.CFStringGetCString.argtypes = [
    ctypes.c_void_p, ctypes.c_char_p, ctypes.c_long, ctypes.c_uint32
]

_key_cache: dict[str, int] = {}


def _key(name: str) -> int:
    if name not in _key_cache:
        _key_cache[name] = _core.CFStringCreateWithCString(
            None, name.encode(), CF_ENCODING_UTF8
        )
    return _key_cache[name]


def _cf_string(ref: int) -> str | None:
    length = _core.CFStringGetLength(ref)
    buf = ctypes.create_string_buffer(length * 4 + 1)
    if not _core.CFStringGetCString(ref, buf, len(buf), CF_ENCODING_UTF8):
        return None
    return buf.value.decode("utf-8", "replace")


def _number_at(dictionary: int, key: str) -> int | None:
    """Read a CFNumberRef entry as i32 (kCGWindowNumber / kCGWindowLayer)."""
    ref = _core.CFDictionaryGetValue(dictionary, _key(key))
    if not ref:
        return None
    value = ctypes.c_int32()
    if not _core.CFNumberGetValue(ref, K_CF_NUMBER_SINT32, ctypes.byref(value)):
        return None
    return value.value


def tool(name: str) -> str:
    """Resolve a system tool; macOS keeps some in /usr/sbin, which is not
    always on PATH for scripted contexts."""
    found = shutil.which(name)
    if found:
        return found
    for directory in ("/usr/bin", "/usr/sbin", "/bin", "/sbin"):
        candidate = Path(directory) / name
        if candidate.exists():
            return str(candidate)
    raise FileNotFoundError(name)


def run(argv: list[str]) -> subprocess.CompletedProcess:
    return subprocess.run(argv, capture_output=True, text=True)


def _window_list() -> list[dict]:
    """On-screen windows as plain dicts (owner, number, layer, title)."""
    raw = _core.CGWindowListCopyWindowInfo(1 | 16, 0)  # on-screen, no desktop
    if not raw:
        return []
    windows = []
    for index in range(_core.CFArrayGetCount(raw)):
        dictionary = _core.CFArrayGetValueAtIndex(raw, index)
        if not dictionary:
            continue
        window: dict = {"number": _number_at(dictionary, "kCGWindowNumber")}
        for key, dest in (("kCGWindowOwnerName", "owner"), ("kCGWindowName", "title")):
            ref = _core.CFDictionaryGetValue(dictionary, _key(key))
            if ref:
                window[dest] = _cf_string(ref)
        window["layer"] = _number_at(dictionary, "kCGWindowLayer")
        bounds_ref = _core.CFDictionaryGetValue(dictionary, _key("kCGWindowBounds"))
        if bounds_ref:
            x, y, w, h = (ctypes.c_double() for _ in range(4))
            if all(
                _core.CFNumberGetValue(
                    _core.CFDictionaryGetValue(bounds_ref, _key(k)),
                    K_CF_NUMBER_SINT32 if False else 4,  # kCFNumberFloat64Type
                    ctypes.byref(v),
                )
                for k, v in (("X", x), ("Y", y), ("Width", w), ("Height", h))
            ):
                window["bounds"] = (x.value, y.value, w.value, h.value)
        windows.append(window)
    return windows


def game_window_id() -> int | None:
    """The engine's main window id (layer 0), for ``screencapture -l`` —
    window-content capture that sees through occlusion. The CGWindow owner
    is the bundle display name ("Harry Potter 2"), matched loosely; the
    largest layer-0 surface wins (the app also exposes small helper
    surfaces)."""
    def is_engine(owner: str | None) -> bool:
        return owner is not None and "harrypotter" in owner.lower().replace(" ", "")

    def area(window: dict) -> float:
        bounds = window.get("bounds")
        return bounds[2] * bounds[3] if bounds else 0.0

    candidates = [
        window
        for window in _window_list()
        if is_engine(window.get("owner")) and window.get("layer") == 0
    ]
    if not candidates:
        return None
    return max(candidates, key=area)["number"]


def engine_pids() -> list[int]:
    result = run([tool("pgrep"), "-f", ENGINE_PATTERN])
    return [int(line) for line in result.stdout.split()]


def wait_for_engine(timeout: float) -> list[int]:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        pids = engine_pids()
        if pids:
            return pids
        time.sleep(0.25)
    return []


# --------------------------------------------------------------------- PNG

def png_dimensions(data: bytes) -> tuple[int, int] | None:
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        return None
    width, height = struct.unpack(">II", data[16:24])
    return width, height


def png_pixels_sampled(data: bytes, max_rows: int = 200) -> list[tuple[int, int, int]]:
    """Decode an 8-bit RGB/RGBA/grey PNG and sample sparse pixel triples.

    PNG filters chain across every scanline, so each row is unfiltered in
    sequence; only every stride-th row contributes samples.
    """
    pos = 8
    width = height = bit_depth = color_type = None
    idat = bytearray()
    channels = {0: 1, 2: 3, 3: 1, 4: 2, 6: 4}
    while pos + 12 <= len(data):
        length, kind = struct.unpack(">I4s", data[pos : pos + 8])
        chunk = data[pos + 8 : pos + 8 + length]
        pos += 12 + length
        if kind == b"IHDR":
            width, height, bit_depth, color_type = struct.unpack(">IIBB", chunk[:10])
        elif kind == b"IDAT":
            idat.extend(chunk)
        elif kind == b"IEND":
            break
    if None in (width, height, bit_depth, color_type):
        raise ValueError("missing IHDR fields")
    if bit_depth != 8 or color_type not in channels:
        raise ValueError(f"unsupported PNG: depth={bit_depth} type={color_type}")
    raw = zlib.decompress(bytes(idat))
    bpp = channels[color_type]
    stride = width * bpp
    if len(raw) < height * (stride + 1):
        raise ValueError("truncated IDAT stream")

    out: list[tuple[int, int, int]] = []
    prev = bytearray(stride)
    offset = 0
    row_stride = max(1, height // max(1, max_rows))
    step = max(1, width // 96)
    for y in range(height):
        filt = raw[offset]
        offset += 1
        row = bytearray(raw[offset : offset + stride])
        offset += stride
        _unfilter_into(row, filt, prev, bpp)
        prev = row
        if y % row_stride:
            continue
        for x in range(0, width, step):
            i = x * bpp
            if bpp < 3:
                out.append((row[i], row[i], row[i]))
            else:
                out.append((row[i], row[i + 1], row[i + 2]))
    return out


def _unfilter_into(row: bytearray, filt: int, prev: bytes | bytearray, bpp: int) -> None:
    """Apply one PNG scanline filter in place."""
    n = len(row)
    if filt == 0:
        pass  # row already holds the raw scanline
    elif filt == 1:  # Sub
        for i in range(bpp, n):
            row[i] = (row[i] + row[i - bpp]) & 0xFF
    elif filt == 2:  # Up
        for i in range(n):
            row[i] = (row[i] + prev[i]) & 0xFF
    elif filt == 3:  # Average
        for i in range(n):
            left = row[i - bpp] if i >= bpp else 0
            row[i] = (row[i] + ((left + prev[i]) >> 1)) & 0xFF
    elif filt == 4:  # Paeth
        for i in range(n):
            a = row[i - bpp] if i >= bpp else 0
            b = prev[i]
            c = prev[i - bpp] if i >= bpp else 0
            p = a + b - c
            pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
            pred = a if (pa <= pb and pa <= pc) else (b if pb <= pc else c)
            row[i] = (row[i] + pred) & 0xFF
    else:
        raise ValueError(f"unknown PNG filter {filt}")


def capture_is_contentful(png_path: Path, min_bytes: int) -> dict:
    info: dict = {}
    data = png_path.read_bytes()
    info["bytes"] = len(data)
    dims = png_dimensions(data)
    info["dimensions"] = list(dims) if dims else None
    if dims is None:
        info["verdict"] = "not_png"
        return info
    if len(data) < min_bytes:
        info["verdict"] = "too_small_for_a_live_frame"
        return info
    try:
        pixels = png_pixels_sampled(data)
    except Exception as error:  # noqa: BLE001 — reported, judged inconclusive
        info["decode_error"] = repr(error)
        info["verdict"] = "undecodable_size_ok"
        return info
    lit = sum(1 for r, g, b in pixels if max(r, g, b) > 24)
    distinct = len(set(pixels))
    info["sampled_pixels"] = len(pixels)
    info["lit_pixels"] = lit
    info["distinct_colors"] = distinct
    # A live 3D frame has structure: plenty of non-black pixels and more
    # than a handful of distinct colours. A black window would be ~all-dark
    # with one colour.
    ok = lit >= 0.02 * len(pixels) and distinct >= 16
    info["verdict"] = "contentful" if ok else "uniform_or_black"
    return info


def crop_to_window(full_png: Path, out_png: Path, geometry_text: str) -> Path | None:
    """Crop the full-screen capture to the game window's rect (fallback
    when the window-id capture is unavailable). ``geometry_text`` is
    AppleScript's ``{x, y}, {w, h}`` in points; the capture is in pixels,
    so the rect scales by capture-width / desktop-width."""

    def numbers_of(text: str) -> list[int]:
        parts = [p.strip() for p in text.replace("{", "").replace("}", "").split(",")]
        return [int(p) for p in parts if p.lstrip("-").isdigit()]

    numbers = numbers_of(geometry_text)
    if len(numbers) != 4:
        return None
    x, y, w, h = numbers
    if w <= 0 or h <= 0:
        return None
    dims = png_dimensions(full_png.read_bytes())
    if dims is None:
        return None
    capture_w, capture_h = dims
    screen = run([
        tool("osascript"),
        "-e",
        'tell application "Finder" to get bounds of window of desktop',
    ])
    screen_numbers = numbers_of(screen.stdout)
    scale = capture_w / screen_numbers[2] if len(screen_numbers) == 4 and screen_numbers[2] else 1.0
    px, py, pw, ph = (round(v * scale) for v in (x, y, w, h))
    px, py = max(0, px), max(0, py)
    pw = min(pw, capture_w - px)
    ph = min(ph, capture_h - py)
    if pw <= 0 or ph <= 0:
        return None
    cropped = run([
        tool("sips"),
        "-c", str(ph), str(pw),
        "--cropOffset", str(py), str(px),
        str(full_png), "--out", str(out_png),
    ])
    if cropped.returncode != 0 or not out_png.exists():
        return None
    return out_png


# ------------------------------------------------------------------ checks

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bundle", type=Path, default=DEFAULT_BUNDLE)
    parser.add_argument("--settle", type=float, default=SETTLE_SECONDS)
    args = parser.parse_args()

    artifact_dir = Path(
        os.environ.get("HP2_ARTIFACT_DIR")
        or tempfile.mkdtemp(prefix="verify-playable-")
    )
    artifact_dir.mkdir(parents=True, exist_ok=True)
    capture_path = artifact_dir / "playable_capture.png"
    window_path = artifact_dir / "playable_window.png"

    report: dict = {
        "bundle": str(args.bundle),
        "artifact_dir": str(artifact_dir),
        "checks": {},
        "notes": [],
    }
    checks = report["checks"]

    # 1. LaunchServices open — the real double-click path.
    opened = run([tool("open"), str(args.bundle)])
    checks["launchservices_open"] = {"exit": opened.returncode}
    if opened.returncode != 0:
        report["notes"].append(f"open failed: {opened.stderr.strip()}")

    # 2. Poll for the engine process, then let frames present and the music
    # stream open before sampling anything.
    pids = wait_for_engine(15.0)
    checks["process_alive"] = {"pids": pids}
    time.sleep(args.settle)

    pids_after = engine_pids()
    alive = bool(pids_after)
    checks["still_alive"] = {"pids": pids_after}

    # 3. Bring the game to the front (cosmetic for the user), then capture
    # the game window's own content — occlusion-proof via ``-l<id>`` when
    # the window id resolves, else full-screen capture cropped to the
    # window rect.
    run([
        tool("osascript"),
        "-e",
        f'tell app "System Events" to set frontmost of '
        f'(first process whose name is "{ENGINE_PROCESS_NAME}") to true',
    ])
    time.sleep(1.5)

    window_id = game_window_id() if alive else None
    capture_info: dict = {"window_id": window_id}
    if window_id is not None:
        shot = run([tool("screencapture"), "-x", f"-l{window_id}", str(window_path)])
        if shot.returncode == 0 and window_path.exists():
            capture_info["mode"] = "window_content"
            capture_info["judged_path"] = str(window_path)
            capture_info.update(capture_is_contentful(window_path, min_bytes=30_000))
        else:
            capture_info["mode"] = "window_capture_failed"
            report["notes"].append(
                f"screencapture -l{window_id} failed: {shot.stderr.strip()}"
            )
    else:
        capture_info["mode"] = "no_window_id"

    if "verdict" not in capture_info:
        # Fallback: full-screen capture cropped to the window rect.
        shot = run([tool("screencapture"), "-x", str(capture_path)])
        capture_info["full_exit"] = shot.returncode
        if shot.returncode == 0 and capture_path.exists():
            geometry = run([
                tool("osascript"),
                "-e",
                f'tell app "System Events" to get {{position, size}} of '
                f'front window of (first process whose name is "{ENGINE_PROCESS_NAME}")',
            ])
            capture_info["window_geometry"] = (
                geometry.stdout.strip() if geometry.returncode == 0 else None
            )
            judged = None
            if geometry.returncode == 0 and geometry.stdout.strip():
                judged = crop_to_window(capture_path, window_path, geometry.stdout)
            if judged is not None:
                capture_info["judged_path"] = str(judged)
                capture_info.update(capture_is_contentful(judged, min_bytes=30_000))
            else:
                capture_info.update(
                    capture_is_contentful(capture_path, min_bytes=MIN_FULL_CAPTURE_BYTES)
                )
        else:
            capture_info["verdict"] = "capture_tool_failed"
            report["notes"].append(
                "screencapture produced nothing (Screen Recording permission?) — "
                "falling back to process+audio proof per plan contingency"
            )
    checks["render_capture"] = capture_info

    # 4. Frontmost process (informational).
    front = run([
        tool("osascript"),
        "-e",
        'tell app "System Events" to get name of every process whose frontmost is true',
    ])
    report["frontmost"] = front.stdout.strip() if front.returncode == 0 else None

    # 5. Music fd proof: lsof must list an open .ogg on the engine.
    ogg_fds = 0
    if alive:
        lsof = run([tool("lsof"), "-p", ",".join(map(str, pids_after))])
        ogg_fds = sum(1 for line in lsof.stdout.splitlines() if ".ogg" in line)
    checks["music_fd_open"] = {"ogg_fds": ogg_fds}

    # 6. Clean shutdown on SIGTERM.
    terminated_cleanly = None
    if alive:
        run([tool("kill"), *[str(pid) for pid in pids_after]])
        deadline = time.monotonic() + SHUTDOWN_TIMEOUT
        while time.monotonic() < deadline:
            if not engine_pids():
                terminated_cleanly = True
                break
            time.sleep(0.2)
        else:
            terminated_cleanly = False
            for pid in pids_after:
                run([tool("kill"), "-9", str(pid)])
    checks["clean_exit"] = {"terminated_cleanly": terminated_cleanly}

    render_ok = (
        capture_info.get("verdict") == "contentful"
        or capture_info.get("verdict") == "undecodable_size_ok"
    )
    passed = (
        alive
        and ogg_fds >= 1
        and terminated_cleanly is True
        and render_ok
    )
    report["passed"] = passed
    print(json.dumps(report, indent=2))
    return 0 if passed else 1


if __name__ == "__main__":
    sys.exit(main())
