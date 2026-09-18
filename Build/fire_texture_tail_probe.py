#!/usr/bin/env python3
"""Read the serialized ``UFireTexture`` spark tail without instantiating it.

The generic PackageReader owns UE1 package/name/import/export/property parsing.
This tool only follows the native portion defined by UTexture::Serialize and
UFireTexture::Serialize: primary mips, optional compressed mips, then the
compact ``TArray<FSpark>`` count and its eight serialized bytes per spark.

The installed package exposes the resident fire textures through HPParticle.u;
the default is its SPARK_Burn-bearing Fire1 export. The probe performs no
package fallback and can inspect any explicitly supplied UFireTexture path.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path
from typing import Any, Sequence

from package79_reference import Cursor, PackageFormatError, PackageReader, ParsedPackage, fail

REPORT_FORMAT = "hp2-fire-texture-tail-probe"
SCHEMA_VERSION = 1
TARGET_EXPORT_PATH = "hpparticle.hp_fx.fire1"
TARGET_EXPORT_INDEX = 886
DEFAULT_ARCHIVE = Path(__file__).resolve().parent.parent / "HarryPotter2" / "Unreal" / "System" / "HPParticle.u"
SPARK_BURN = 0
SPARK_FIELD_NAMES = ("type", "heat", "x", "y", "byte_a", "byte_b", "byte_c", "byte_d")
MIP_MIN_SERIALIZED_BYTES = 15  # i32 skip, compact zero length, i32/i32 dimensions, two bits fields.


def _property(parsed: ParsedPackage, reader: PackageReader, export_index: int, name: str) -> dict[str, Any] | None:
    stream = reader.parse_property_stream(parsed, export_index)
    matches = [tag for tag in stream["tags"] if tag["name"]["text"].casefold() == name.casefold()]
    if len(matches) > 1:
        entry = parsed.exports[export_index]
        fail(reader.relative_path, entry.serial_offset or entry.record_offset, f"duplicate {name} property")
    return matches[0] if matches else None


def _bool_property(parsed: ParsedPackage, reader: PackageReader, export_index: int, name: str) -> bool:
    tag = _property(parsed, reader, export_index, name)
    if tag is None:
        return False
    value = tag["value"]
    if tag["type"] != "BoolProperty" or value.get("kind") != "bool":
        fail(reader.relative_path, tag["offset"], f"{name} property is not a BoolProperty")
    return bool(value["value"])


def _palette_reference(parsed: ParsedPackage, reader: PackageReader, export_index: int) -> dict[str, Any] | None:
    tag = _property(parsed, reader, export_index, "Palette")
    if tag is None:
        return None
    value = tag["value"]
    if tag["type"] != "ObjectProperty" or value.get("kind") != "object":
        fail(reader.relative_path, tag["offset"], "Palette property is not an object reference")
    return {"path": value["path"], "ref": value["ref"]}


def _read_mips(cursor: Cursor, label: str) -> list[dict[str, int]]:
    count_offset = cursor.pos
    count = cursor.compact_index()
    if count < 0:
        fail(cursor.path, count_offset, f"negative {label} count: {count}")
    if count > (cursor.limit - cursor.pos) // MIP_MIN_SERIALIZED_BYTES:
        fail(cursor.path, count_offset, f"{label} count {count} cannot fit in export payload")

    mips: list[dict[str, int]] = []
    for index in range(count):
        mip_offset = cursor.pos
        lazy_skip = cursor.i32()
        data_count_offset = cursor.pos
        data_count = cursor.compact_index()
        if data_count < 0:
            fail(cursor.path, data_count_offset, f"negative {label}[{index}] byte count: {data_count}")
        data_start = cursor.pos
        cursor.require(data_count)
        cursor.pos += data_count
        if lazy_skip >= 0 and data_count:
            lower = data_start
            if lazy_skip < lower or lazy_skip > cursor.limit:
                fail(
                    cursor.path,
                    mip_offset,
                    f"{label}[{index}] lazy-array skip position {lazy_skip} is outside [{lower}, {cursor.limit}]",
                )
            cursor.pos = lazy_skip
        u_size_offset = cursor.pos
        u_size = cursor.i32()
        v_size = cursor.i32()
        u_bits = cursor.u8()
        v_bits = cursor.u8()
        if u_size <= 0 or v_size <= 0:
            fail(cursor.path, u_size_offset, f"{label}[{index}] has non-positive dimensions {u_size}x{v_size}")
        mips.append(
            {
                "data_bytes": data_count,
                "u_bits": u_bits,
                "u_size": u_size,
                "v_bits": v_bits,
                "v_size": v_size,
            }
        )
    return mips


def decode_fire_texture_tail(
    reader: PackageReader,
    parsed: ParsedPackage,
    export_index: int,
    property_stream_end: int,
    has_comp_mips: bool,
) -> dict[str, Any]:
    """Decode UTexture native payload and the immediately following FSpark tail."""
    entry = parsed.exports[export_index]
    if entry.serial_offset is None or not entry.serial_size:
        fail(reader.relative_path, entry.record_offset, "FireTexture export has no serial payload")
    serial_end = entry.serial_offset + entry.serial_size
    cursor = Cursor(
        reader.data,
        reader.relative_path,
        property_stream_end,
        "UTexture/UFireTexture native payload",
        serial_end,
    )
    primary_mips = _read_mips(cursor, "primary mip")
    compressed_mips = _read_mips(cursor, "compressed mip") if has_comp_mips else []

    count_offset = cursor.pos
    spark_count = cursor.compact_index()
    if spark_count < 0:
        fail(reader.relative_path, count_offset, f"negative FSpark count: {spark_count}")
    remaining = cursor.limit - cursor.pos
    if spark_count > remaining // len(SPARK_FIELD_NAMES):
        fail(cursor.path, count_offset, f"FSpark count {spark_count} exceeds remaining tail bytes {remaining}")

    sparks: list[dict[str, int]] = []
    for _ in range(spark_count):
        values = cursor.take(len(SPARK_FIELD_NAMES))
        sparks.append(dict(zip(SPARK_FIELD_NAMES, values, strict=True)))
    if cursor.pos != serial_end:
        fail(cursor.path, cursor.pos, f"UFireTexture native tail leaves {serial_end - cursor.pos} byte(s) unread")

    type_counts: dict[int, int] = {}
    for spark in sparks:
        type_counts[spark["type"]] = type_counts.get(spark["type"], 0) + 1
    return {
        "compressed_mip_count": len(compressed_mips),
        "dimensions": (
            {"u_size": primary_mips[0]["u_size"], "v_size": primary_mips[0]["v_size"]}
            if primary_mips
            else None
        ),
        "has_spark_burn": any(spark["type"] == SPARK_BURN for spark in sparks),
        "mip_count": len(primary_mips),
        "spark_count": spark_count,
        "spark_list": sparks,
        "spark_types": [
            {
                "count": count,
                "name": "SPARK_Burn" if spark_type == SPARK_BURN else None,
                "value": spark_type,
            }
            for spark_type, count in sorted(type_counts.items())
        ],
    }


def probe_archive(
    archive_path: Path,
    target_export_path: str = TARGET_EXPORT_PATH,
    target_export_index: int | None = None,
) -> dict[str, Any]:
    try:
        data = archive_path.read_bytes()
    except OSError as exc:
        raise PackageFormatError(f"{archive_path}: cannot read archive: {exc}") from exc

    reader = PackageReader(archive_path.name, data)
    parsed = reader.parse()
    if target_export_index is None and target_export_path.casefold() == TARGET_EXPORT_PATH:
        target_export_index = TARGET_EXPORT_INDEX
    if target_export_index is None:
        target = target_export_path.casefold()
        matches = [
            index
            for index in range(len(parsed.exports))
            if parsed.resolver.export_path(index).casefold() == target
        ]
        if len(matches) != 1:
            raise PackageFormatError(
                f"{archive_path}: expected exactly one export {target_export_path!r}, found {len(matches)}"
            )
        export_index = matches[0]
    else:
        if not 0 <= target_export_index < len(parsed.exports):
            raise PackageFormatError(
                f"{archive_path}: export index {target_export_index} is outside export table count {len(parsed.exports)}"
            )
        export_index = target_export_index
        resolved_path = parsed.resolver.export_path(export_index)
        if target_export_path and resolved_path.casefold() != target_export_path.casefold():
            raise PackageFormatError(
                f"{archive_path}: export index {export_index} resolves to {resolved_path!r}, "
                f"not {target_export_path!r}"
            )
    entry = parsed.exports[export_index]
    class_path = parsed.resolver.ref_path(entry.class_ref)
    if class_path is None or class_path.casefold() != "fire.firetexture":
        fail(reader.relative_path, entry.record_offset, f"target export class is {class_path!r}, expected Fire.FireTexture")

    property_stream = reader.parse_property_stream(parsed, export_index)
    tail = decode_fire_texture_tail(
        reader,
        parsed,
        export_index,
        property_stream["stream_end"],
        _bool_property(parsed, reader, export_index, "bHasComp"),
    )
    entry_offset = entry.serial_offset
    if entry_offset is None:
        fail(reader.relative_path, entry.record_offset, "target export has no serial offset")
    tail["palette_ref"] = _palette_reference(parsed, reader, export_index)
    return {
        "archive": {"path": str(archive_path.resolve()), "sha256": hashlib.sha256(data).hexdigest()},
        "class": class_path,
        "export": {
            "index": export_index,
            "path": parsed.resolver.export_path(export_index),
            "serial_span": {"end": entry_offset + entry.serial_size, "offset": entry_offset, "size": entry.serial_size},
        },
        "format": REPORT_FORMAT,
        "schema_version": SCHEMA_VERSION,
        **tail,
    }


def canonical_json(report: dict[str, Any]) -> bytes:
    return (json.dumps(report, indent=2, sort_keys=True) + "\n").encode("utf-8")


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--archive", type=Path, default=DEFAULT_ARCHIVE, help="package archive to inspect")
    parser.add_argument("--export", default=TARGET_EXPORT_PATH, help="fully qualified UFireTexture export path")
    parser.add_argument(
        "--export-index",
        type=int,
        help="require this export-table index (disambiguates duplicate paths)",
    )
    parser.add_argument("--output", type=Path, help="write canonical JSON here instead of stdout")
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(argv)
    try:
        report = probe_archive(args.archive, args.export, args.export_index)
        rendered = canonical_json(report)
        if args.output is None:
            sys.stdout.buffer.write(rendered)
        else:
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_bytes(rendered)
    except (OSError, PackageFormatError, ValueError) as exc:
        print(f"fire_texture_tail_probe.py: error: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
