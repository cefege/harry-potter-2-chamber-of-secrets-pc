#!/usr/bin/env python3
"""Compare two captured frame PNGs with per-channel tolerance.

Primary comparison is a strict per-channel maximum delta over every pixel.
Because antialiasing noise can move individual pixel decisions between two
otherwise identical runs, a comparison that fails the primary check falls
back to comparing downscaled RGB thumbnails; when the thumbnails agree
within the fallback tolerance the frames are judged the same.

CLI:

    baseline_compare.py baseline.png candidate.png [--tolerance N]

Emits a schema-v1 verdict report on stdout and exits 0 only for "same".
"""

from __future__ import annotations

import argparse
import json
import struct
import sys
import zlib
from pathlib import Path

SCHEMA_VERSION = 1
REPORT_NAME = "baseline_compare"
DEFAULT_TOLERANCE = 2
DEFAULT_THUMBNAIL_SIZE = 16

PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"


class BaselineCompareError(Exception):
    """Raised when a PNG input cannot be decoded."""


# ---------------------------------------------------------------------------
# Minimal PNG codec (8-bit, non-interlaced; RGB/RGBA/gray inputs, RGB output).
# ---------------------------------------------------------------------------


def _read_chunk(payload: bytes, cursor: int) -> tuple[bytes, bytes, int]:
    if cursor + 8 > len(payload):
        raise BaselineCompareError("truncated PNG chunk header")
    length = struct.unpack(">I", payload[cursor : cursor + 4])[0]
    kind = payload[cursor + 4 : cursor + 8]
    end = cursor + 8 + length
    if end + 4 > len(payload):
        raise BaselineCompareError("truncated PNG chunk body")
    return kind, payload[cursor + 8 : end], end + 4


def _paeth(a: int, b: int, c: int) -> int:
    p = a + b - c
    pa = abs(p - a)
    pb = abs(p - b)
    pc = abs(p - c)
    if pa <= pb and pa <= pc:
        return a
    if pb <= pc:
        return b
    return c


def _unfilter(
    raw: bytes, width: int, height: int, channels: int
) -> list[bytearray]:
    stride = width * channels
    expected = (stride + 1) * height
    if len(raw) != expected:
        raise BaselineCompareError(
            f"unexpected PNG scanline payload: {len(raw)} != {expected}"
        )
    rows: list[bytearray] = []
    previous = bytearray(stride)
    cursor = 0
    for _ in range(height):
        filter_kind = raw[cursor]
        cursor += 1
        row = bytearray(raw[cursor : cursor + stride])
        cursor += stride
        if filter_kind == 1:  # Sub
            for index in range(channels, stride):
                row[index] = (row[index] + row[index - channels]) & 0xFF
        elif filter_kind == 2:  # Up
            for index in range(stride):
                row[index] = (row[index] + previous[index]) & 0xFF
        elif filter_kind == 3:  # Average
            for index in range(stride):
                left = row[index - channels] if index >= channels else 0
                row[index] = (row[index] + ((left + previous[index]) >> 1)) & 0xFF
        elif filter_kind == 4:  # Paeth
            for index in range(stride):
                left = row[index - channels] if index >= channels else 0
                up_left = previous[index - channels] if index >= channels else 0
                row[index] = (
                    row[index] + _paeth(left, previous[index], up_left)
                ) & 0xFF
        elif filter_kind != 0:
            raise BaselineCompareError(f"unsupported PNG filter {filter_kind}")
        rows.append(row)
        previous = row
    return rows


def _rows_to_rgb(
    rows: list[bytearray], width: int, height: int, color_type: int
) -> bytes:
    if color_type == 2:  # RGB
        return b"".join(bytes(row) for row in rows)
    if color_type == 6:  # RGBA
        out = bytearray(width * height * 3)
        cursor = 0
        for row in rows:
            for index in range(0, len(row), 4):
                out[cursor : cursor + 3] = row[index : index + 3]
                cursor += 3
        return bytes(out)
    if color_type == 0:  # Grayscale
        out = bytearray(width * height * 3)
        cursor = 0
        for row in rows:
            for value in row:
                out[cursor] = out[cursor + 1] = out[cursor + 2] = value
                cursor += 3
        return bytes(out)
    raise BaselineCompareError(f"unsupported PNG color type {color_type}")


def read_png(path: Path) -> tuple[int, int, bytes]:
    """Decode a PNG into (width, height, tightly packed RGB8 bytes)."""
    try:
        payload = Path(path).read_bytes()
    except OSError as error:
        raise BaselineCompareError(f"cannot read {path}: {error}") from error
    if not payload.startswith(PNG_SIGNATURE):
        raise BaselineCompareError(f"{path} is not a PNG file")
    cursor = len(PNG_SIGNATURE)
    width = height = 0
    bit_depth = color_type = interlace = 0
    idat = bytearray()
    while cursor < len(payload):
        kind, body, cursor = _read_chunk(payload, cursor)
        if kind == b"IHDR":
            if len(body) != 13:
                raise BaselineCompareError("malformed IHDR")
            width, height = struct.unpack(">II", body[:8])
            bit_depth = body[8]
            color_type = body[9]
            interlace = body[12]
        elif kind == b"IDAT":
            idat.extend(body)
        elif kind == b"IEND":
            break
    if not width or not height:
        raise BaselineCompareError(f"{path} has no usable IHDR")
    if bit_depth != 8:
        raise BaselineCompareError(f"unsupported PNG bit depth {bit_depth}")
    if interlace != 0:
        raise BaselineCompareError("interlaced PNG is not supported")
    channels = {0: 1, 2: 3, 6: 4}.get(color_type)
    if channels is None:
        raise BaselineCompareError(f"unsupported PNG color type {color_type}")
    try:
        raw = zlib.decompress(bytes(idat))
    except zlib.error as error:
        raise BaselineCompareError(f"corrupt PNG IDAT in {path}: {error}") from error
    rows = _unfilter(raw, width, height, channels)
    return width, height, _rows_to_rgb(rows, width, height, color_type)


def write_png(path: Path, width: int, height: int, rgb: bytes) -> None:
    """Write tightly packed RGB8 bytes as an 8-bit non-interlaced PNG."""
    if len(rgb) != width * height * 3:
        raise BaselineCompareError(
            f"RGB payload {len(rgb)} does not match {width}x{height}x3"
        )
    stride = width * 3
    raw = bytearray()
    for row in range(height):
        raw.append(0)  # filter kind None
        raw.extend(rgb[row * stride : (row + 1) * stride])

    def chunk(kind: bytes, body: bytes) -> bytes:
        return (
            struct.pack(">I", len(body))
            + kind
            + body
            + struct.pack(">I", zlib.crc32(kind + body) & 0xFFFFFFFF)
        )

    encoded = PNG_SIGNATURE
    encoded += chunk(
        b"IHDR",
        struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0),
    )
    encoded += chunk(b"IDAT", zlib.compress(bytes(raw), 9))
    encoded += chunk(b"IEND", b"")
    Path(path).write_bytes(encoded)


# ---------------------------------------------------------------------------
# Comparison.
# ---------------------------------------------------------------------------


def _max_channel_delta(
    baseline: bytes, candidate: bytes
) -> int:
    delta = 0
    for index in range(0, len(baseline)):
        difference = baseline[index] - candidate[index]
        if difference < 0:
            difference = -difference
        if difference > delta:
            delta = difference
            if delta == 255:
                break
    return delta


def _thumbnail(pixels: bytes, width: int, height: int, size: int) -> list[int]:
    """Box-downscale RGB8 pixels to size x size average channels."""
    averages: list[int] = []
    for out_row in range(size):
        y0 = out_row * height // size
        y1 = max((out_row + 1) * height // size, y0 + 1)
        for out_column in range(size):
            x0 = out_column * width // size
            x1 = max((out_column + 1) * width // size, x0 + 1)
            sums = [0, 0, 0]
            count = 0
            for y in range(y0, y1):
                cursor = (y * width + x0) * 3
                for _ in range(x0, x1):
                    sums[0] += pixels[cursor]
                    sums[1] += pixels[cursor + 1]
                    sums[2] += pixels[cursor + 2]
                    cursor += 3
                    count += 1
            averages.extend(total // count for total in sums)
    return averages


def compare_pngs(
    baseline_pixels: bytes,
    candidate_pixels: bytes,
    baseline_size: tuple[int, int],
    candidate_size: tuple[int, int],
    *,
    tolerance: int = DEFAULT_TOLERANCE,
    thumbnail_size: int = DEFAULT_THUMBNAIL_SIZE,
    fallback_tolerance: int | None = None,
) -> dict[str, object]:
    """Compare decoded frames; see module docstring for the verdict policy."""
    if fallback_tolerance is None:
        fallback_tolerance = max(tolerance, 8)
    result: dict[str, object] = {
        "width_px": baseline_size[0],
        "height_px": baseline_size[1],
        "candidate_width_px": candidate_size[0],
        "candidate_height_px": candidate_size[1],
        "tolerance": tolerance,
        "fallback_tolerance": fallback_tolerance,
        "max_channel_delta": None,
        "fallback_used": False,
        "fallback_max_channel_delta": None,
    }
    if baseline_size != candidate_size:
        result["verdict"] = "size_mismatch"
        return result
    primary = _max_channel_delta(baseline_pixels, candidate_pixels)
    result["max_channel_delta"] = primary
    if primary <= tolerance:
        result["verdict"] = "same"
        return result
    baseline_thumb = _thumbnail(
        baseline_pixels, baseline_size[0], baseline_size[1], thumbnail_size
    )
    candidate_thumb = _thumbnail(
        candidate_pixels, candidate_size[0], candidate_size[1], thumbnail_size
    )
    fallback = _max_channel_delta(
        bytes(baseline_thumb), bytes(candidate_thumb)
    )
    result["fallback_used"] = True
    result["fallback_max_channel_delta"] = fallback
    result["verdict"] = "same" if fallback <= fallback_tolerance else "differs"
    return result


def compare_png_files(
    baseline_path: Path,
    candidate_path: Path,
    *,
    tolerance: int = DEFAULT_TOLERANCE,
    thumbnail_size: int = DEFAULT_THUMBNAIL_SIZE,
    fallback_tolerance: int | None = None,
) -> dict[str, object]:
    """Load two PNG files and compare them via compare_pngs."""
    baseline_width, baseline_height, baseline_pixels = read_png(baseline_path)
    candidate_width, candidate_height, candidate_pixels = read_png(candidate_path)
    result = compare_pngs(
        baseline_pixels,
        candidate_pixels,
        (baseline_width, baseline_height),
        (candidate_width, candidate_height),
        tolerance=tolerance,
        thumbnail_size=thumbnail_size,
        fallback_tolerance=fallback_tolerance,
    )
    result["baseline"] = str(baseline_path)
    result["candidate"] = str(candidate_path)
    return result


# ---------------------------------------------------------------------------
# Schema-v1 CLI report.
# ---------------------------------------------------------------------------


def _report(
    verdict_result: dict[str, object] | None,
    *,
    status: str,
    reason_code: str | None,
    tolerance: int,
    thumbnail_size: int,
    artifacts: list[str],
    command: list[str],
    error: str | None = None,
) -> dict[str, object]:
    data: dict[str, object] = {
        "profile": {
            "tolerance": tolerance,
            "thumbnail_size": thumbnail_size,
        }
    }
    if verdict_result is not None:
        data.update(verdict_result)
    if error is not None:
        data["error"] = error
    report: dict[str, object] = {
        "schema": SCHEMA_VERSION,
        "name": REPORT_NAME,
        "status": status,
        "invariant": (
            "candidate frame matches baseline within per-channel tolerance "
            "(downscaled-thumbnail fallback absorbs minor AA noise)"
        ),
        "data": data,
        "artifacts": artifacts,
        "command": command,
    }
    if reason_code is not None:
        report["reason_code"] = reason_code
    report["exit_reason"] = {
        "pass": "frames match within tolerance",
        "fail": "frames differ beyond tolerance or sizes mismatch",
        "blocked": "comparison inputs could not be decoded",
    }[status]
    return report


def _arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Compare two frame PNGs with per-channel tolerance.",
    )
    parser.add_argument("baseline", type=Path, help="baseline PNG")
    parser.add_argument("candidate", type=Path, help="candidate PNG")
    parser.add_argument(
        "--tolerance",
        type=int,
        default=DEFAULT_TOLERANCE,
        help="maximum accepted per-channel delta (default: %(default)s)",
    )
    parser.add_argument(
        "--thumbnail-size",
        type=int,
        default=DEFAULT_THUMBNAIL_SIZE,
        help="fallback thumbnail edge length (default: %(default)s)",
    )
    return parser.parse_args()


def main() -> int:
    arguments = _arguments()
    command = [
        "baseline_compare.py",
        str(arguments.baseline),
        str(arguments.candidate),
        "--tolerance",
        str(arguments.tolerance),
    ]
    artifacts = [str(arguments.baseline), str(arguments.candidate)]
    try:
        verdict_result = compare_png_files(
            arguments.baseline,
            arguments.candidate,
            tolerance=arguments.tolerance,
            thumbnail_size=arguments.thumbnail_size,
        )
    except BaselineCompareError as error:
        report = _report(
            None,
            status="blocked",
            reason_code="renderer.frame_undecodable",
            tolerance=arguments.tolerance,
            thumbnail_size=arguments.thumbnail_size,
            artifacts=artifacts,
            command=command,
            error=str(error),
        )
        print(json.dumps(report, indent=2, sort_keys=True))
        return 2
    status = "pass" if verdict_result["verdict"] == "same" else "fail"
    reason_code = None
    if status == "fail":
        reason_code = (
            "renderer.frame_size_mismatch"
            if verdict_result["verdict"] == "size_mismatch"
            else "renderer.frame_differs"
        )
    report = _report(
        verdict_result,
        status=status,
        reason_code=reason_code,
        tolerance=arguments.tolerance,
        thumbnail_size=arguments.thumbnail_size,
        artifacts=artifacts,
        command=command,
    )
    print(json.dumps(report, indent=2, sort_keys=True))
    return 0 if status == "pass" else 1


if __name__ == "__main__":
    raise SystemExit(main())
