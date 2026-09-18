#!/usr/bin/env python3
"""UE1 UnrealScript package reader: HP2 v79 reference goldens + data-root audits.

Naming note (historical misnomer): the file is called ``package79_reference.py``
because it started life as a version-79-only reader. It now parameterizes the
package FileVersion over [PACKAGE_MIN_VERSION, PACKAGE_MAX_VERSION] = [60, 79]
with LicenseeVersion 0, covering both HP2 (v79) and the HP1 disc set
{61, 68, 69, 72, 73, 75, 76}. The name is kept because CMake registrations,
docs, and sibling tools reference it.

Per-feature decoding is keyed on the parsed FileVersion exactly as the native
loader branches do:
  * summary: >= 68 uses Guid + generation table (unchanged v79 layout);
    < 68 uses the pre-68 heritage shape (HeritageCount/HeritageOffset i32 pair
    where the GUID would be, no generation table).
  * name entries: >= 64 uses the FString form (compact-index char count incl.
    NUL, ANSI or UTF-16LE payload, u32 flags); < 64 uses the pre-64 form
    (NUL-terminated ANSI bytes without length prefix, u32 flags) -- mirrors
    UnName.cpp:236 (Ar.Ver() < 64).
  * imports, exports, and property tags are byte-compatible across the whole
    range (probe-verified on the HP1 ISO; see local://hp1-v76-format-facts.md).

Modes
-----
reference (default, no ``--data-root``):
    Regenerate -- or with ``--check``, byte-verify -- the six-package HP2
    golden ``Tests/Fixtures/package79-reference.json``. Output is frozen;
    any change to it requires its own acceptance record.

audit (``--data-root ROOT``):
    Walk ROOT recursively, decode EVERY UE1 package found (any supported
    version), classify every other file as a non-package entry, and emit an
    audit JSON (default ``Tests/Fixtures/hp1-package-audit.json``). Top-level
    ``*.json`` provenance manifests directly inside ROOT are excluded from the
    walk: they are build artifacts of prepare scripts, not game data. Reuses
    the exact same PackageReader parsers as reference mode -- there is no
    second reader implementation.

Audit JSON schema (``schema_version`` 2), keys sorted by canonical_json.  The
audit retains the structural package census and adds byte-stable payload
coverage: every export serial span and every non-package file has its size and
SHA-256 recorded.  Export payloads are classified by resolved class (mesh,
font, sound, texture, replay, save, or object).  The existing tagged-property
reader projects every reachable prefix; mesh/font dispatch and standalone
PSA/DDS-DXT1/raw-EA-XA projections mirror hp-format's existing readers.
Families without enough wire metadata for those readers remain explicit
``hash-only`` projections.  Top-level ``payload_family_census`` accounts for
both package exports and non-package config/localization/PSA/sound/texture/
replay files.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import struct
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any, NoReturn, Sequence

PACKAGE_TAG = 0x9E2A83C1
PACKAGE_VERSION = 79
LICENSEE_VERSION = 0
MAX_NAME_UNITS = 128
MAX_NATIVE_INDEX = 0x1000
FUNCTION_FLAG_NET = 0x00000040
FUNCTION_FLAG_NET_RELIABLE = 0x00000080
FUNCTION_FLAG_NATIVE = 0x00000400
FUNCTION_FLAG_MASK = 0x0001FFFF
PACKAGE_MIN_VERSION = 60
PACKAGE_MAX_VERSION = 79
AUDIT_SCHEMA_VERSION = 2

# Family names are deliberately wire-format concepts, not profile concepts.
# Keep this table in lockstep with hp-format's package79::audit module.
NON_PACKAGE_FAMILIES = {
    ".ini": "config",
    ".int": "localization",
    ".psa": "psa",
    ".dem": "replay",
    ".rpl": "replay",
    ".replay": "replay",
    ".uax": "sound",
    ".wav": "sound",
    ".ogg": "sound",
    ".xa": "sound",
    ".utx": "texture",
    ".dds": "texture",
    ".png": "texture",
    ".bmp": "texture",
    ".tga": "texture",
}

def non_package_family(path: str) -> str:
    return NON_PACKAGE_FAMILIES.get(Path(path).suffix.lower(), "binary")


def export_payload_family(path: str, class_path: str | None) -> str:
    if Path(path).suffix.lower() == ".usa":
        return "save"
    leaf = (class_path or "").rsplit(".", 1)[-1].lower()
    if leaf in {"mesh", "lodmesh", "skeletalmesh"}:
        return "mesh"
    if leaf == "font" or leaf.endswith("font"):
        return "font"
    if "replay" in leaf:
        return "replay"
    if leaf in {"sound", "music"} or leaf.endswith("sound"):
        return "sound"
    if "texture" in leaf or leaf == "palette":
        return "texture"
    return "object"


def add_family_census(
    census: dict[str, dict[str, int]], family: str, size: int
) -> None:
    entry = census.setdefault(family, {"bytes": 0, "payload_count": 0})
    entry["bytes"] += size
    entry["payload_count"] += 1

def _decode_dxt1(blocks: bytes, width: int, height: int) -> bytes:
    if width <= 0 or height <= 0:
        raise ValueError("empty DXT1 dimensions")
    blocks_x = (width + 3) // 4
    blocks_y = (height + 3) // 4
    if len(blocks) < blocks_x * blocks_y * 8:
        raise ValueError("truncated DXT1 payload")
    output = bytearray(width * height * 4)
    for block_y in range(blocks_y):
        for block_x in range(blocks_x):
            base = (block_y * blocks_x + block_x) * 8
            color0, color1, indices = struct.unpack_from("<HHI", blocks, base)
            def color(value: int) -> list[int]:
                r5, g6, b5 = (value >> 11) & 31, (value >> 5) & 63, value & 31
                return [(r5 << 3) | (r5 >> 2), (g6 << 2) | (g6 >> 4),
                        (b5 << 3) | (b5 >> 2), 255]
            c0, c1 = color(color0), color(color1)
            palette = [c0, c1, [0, 0, 0, 255], [0, 0, 0, 0]]
            if color0 > color1:
                palette[2] = [(2 * c0[i] + c1[i]) // 3 for i in range(3)] + [255]
                palette[3] = [(c0[i] + 2 * c1[i]) // 3 for i in range(3)] + [255]
            else:
                palette[2] = [(c0[i] + c1[i]) // 2 for i in range(3)] + [255]
            for texel in range(16):
                x, y = block_x * 4 + texel % 4, block_y * 4 + texel // 4
                if x >= width or y >= height:
                    continue
                rgba = palette[(indices >> (texel * 2)) & 3]
                at = (y * width + x) * 4
                output[at : at + 4] = bytes((rgba[2], rgba[1], rgba[0], rgba[3]))
    return bytes(output)


_EAXA_COEFFICIENTS = (
    (0, 0), (240, 0), (460, -208), (392, -220), (488, -240), (328, -208),
    (440, -168), (420, -188), (432, -176), (240, -16), (416, -192),
    (424, -160), (288, -8), (436, -188), (224, -1), (272, -16),
)


def _decode_eaxa(data: bytes) -> bytes:
    if len(data) % 15:
        raise ValueError("EA-XA payload is not block-aligned")
    history1 = history2 = 0
    output = bytearray()
    for base in range(0, len(data), 15):
        block = data[base : base + 15]
        predictor, shift = block[0] >> 4, block[0] & 15
        c0, c1 = _EAXA_COEFFICIENTS[predictor]
        for index in range(28):
            packed = block[1 + index // 2]
            nibble = (packed >> 4) if index & 1 else (packed & 15)
            if nibble >= 8:
                nibble -= 16
            scaled = nibble << (12 - shift) if shift <= 12 else nibble // (1 << (shift - 12))
            sample = (scaled * 256 + c0 * history1 + c1 * history2) // 256
            sample = max(-32768, min(32767, sample))
            output.extend(struct.pack("<h", sample))
            history2, history1 = history1, sample
    return bytes(output)


def non_package_projection(path: str, family: str, data: bytes) -> dict[str, Any]:
    suffix = Path(path).suffix.lower()
    try:
        if family == "psa":
            pos = 0
            chunks: dict[str, tuple[int, int, bytes]] = {}
            for expected in ("ANIMHEAD", "BONENAMES", "ANIMINFO", "ANIMKEYS"):
                if pos + 32 > len(data):
                    raise ValueError("truncated PSA header")
                identifier = data[pos : pos + 20].split(b"\0", 1)[0].decode("ascii")
                total_frames, size, count = struct.unpack_from("<iii", data, pos + 20)
                if identifier != expected or min(total_frames, size, count) < 0:
                    raise ValueError("invalid PSA chunk")
                end = pos + 32 + size * count
                if end > len(data):
                    raise ValueError("truncated PSA chunk")
                chunks[identifier] = (total_frames, count, data[pos + 32 : end])
                pos = end
            if pos != len(data):
                raise ValueError("trailing PSA bytes")
            total_frames, _, _ = chunks["ANIMHEAD"]
            bone_count = chunks["BONENAMES"][1]
            sequence_count = chunks["ANIMINFO"][1]
            key_count = chunks["ANIMKEYS"][1]
            return {"bone_count": bone_count, "key_count": key_count,
                    "reader": "hp-format.psa", "sequence_count": sequence_count,
                    "status": "projected", "total_frames": total_frames}
        if family == "texture" and suffix == ".dds":
            if (
                len(data) < 128
                or data[:4] != b"DDS "
                or struct.unpack_from("<I", data, 4)[0] != 124
                or struct.unpack_from("<I", data, 76)[0] != 32
                or data[84:88] != b"DXT1"
            ):
                return {"reader": "hp-format.dxt1", "status": "hash-only"}
            height, width = struct.unpack_from("<II", data, 12)
            blocks_x = (width + 3) // 4
            blocks_y = (height + 3) // 4
            encoded_size = blocks_x * blocks_y * 8
            if encoded_size > len(data) - 128:
                return {"reader": "hp-format.dxt1", "status": "hash-only"}
            decoded = _decode_dxt1(data[128:128 + encoded_size], width, height)
            return {"decoded_sha256": hashlib.sha256(decoded).hexdigest(),
                    "height": height, "reader": "hp-format.dxt1",
                    "status": "decoded", "width": width}
        if family == "sound" and suffix == ".xa":
            decoded = _decode_eaxa(data)
            return {"decoded_sha256": hashlib.sha256(decoded).hexdigest(),
                    "reader": "hp-format.eaxa", "sample_count": len(decoded) // 2,
                    "status": "decoded"}
    except (UnicodeDecodeError, ValueError, struct.error):
        reader = {"psa": "hp-format.psa", "sound": "hp-format.eaxa",
                  "texture": "hp-format.dxt1"}.get(family, "bytes")
        return {"reader": reader, "status": "hash-only"}
    return {"reader": "bytes", "status": "hash-only"}

DEFAULT_AUDIT_OUTPUT = "Tests/Fixtures/hp1-package-audit.json"

PACKAGE_PATHS = (
    "HarryPotter2/Unreal/System/Core.u",
    "HarryPotter2/Unreal/System/Engine.u",
    "HarryPotter2/Unreal/System/HGame.u",
    "HarryPotter2/Unreal/Maps/startup.unr",
    "HarryPotter2/Unreal/Textures/HP2_Master.utx",
    "HarryPotter2/Unreal/Sounds/General.uax",
)
DEFAULT_OUTPUT = "Tests/Fixtures/package79-reference.json"


class PackageFormatError(ValueError):
    """A package is truncated, malformed, or incompatible with this reader."""


def fail(path: str, offset: int, message: str) -> NoReturn:
    raise PackageFormatError(f"{path}: offset {offset}: {message}")


def sha256_span(data: bytes, offset: int, size: int) -> str:
    return hashlib.sha256(memoryview(data)[offset : offset + size]).hexdigest()


def encode_compact_index(value: int) -> bytes:
    """Encode a signed UE1 compact index in its unique shortest form."""
    if not -(1 << 31) <= value <= (1 << 31) - 1:
        raise ValueError(f"compact index {value} is outside int32")

    negative = value < 0
    magnitude = -value if negative else value
    output = bytearray()
    first_payload = magnitude & 0x3F
    magnitude >>= 6
    output.append((0x80 if negative else 0) | first_payload | (0x40 if magnitude else 0))

    while magnitude:
        payload = magnitude & 0x7F
        magnitude >>= 7
        if len(output) == 4:
            output.append(payload)
            if magnitude:
                raise ValueError(f"compact index {value} needs more than five bytes")
            break
        output.append(payload | (0x80 if magnitude else 0))
    return bytes(output)


class Cursor:
    def __init__(self, data: bytes, path: str, offset: int, context: str) -> None:
        self.data = data
        self.path = path
        self.pos = offset
        self.context = context

    def require(self, size: int) -> None:
        if size < 0 or self.pos < 0 or self.pos + size > len(self.data):
            fail(
                self.path,
                self.pos,
                f"truncated {self.context}: need {size} byte(s), file size is {len(self.data)}",
            )

    def take(self, size: int) -> bytes:
        self.require(size)
        start = self.pos
        self.pos += size
        return self.data[start : self.pos]

    def u8(self) -> int:
        return self.take(1)[0]

    def u16(self) -> int:
        self.require(2)
        value = struct.unpack_from("<H", self.data, self.pos)[0]
        self.pos += 2
        return value

    def u32(self) -> int:
        self.require(4)
        value = struct.unpack_from("<I", self.data, self.pos)[0]
        self.pos += 4
        return value

    def i32(self) -> int:
        self.require(4)
        value = struct.unpack_from("<i", self.data, self.pos)[0]
        self.pos += 4
        return value

    def compact_index(self) -> int:
        start = self.pos
        first = self.u8()
        magnitude = first & 0x3F
        shift = 6
        byte = first
        byte_count = 1

        while byte & (0x40 if byte_count == 1 else 0x80):
            if byte_count == 5:
                fail(self.path, start, f"{self.context} compact index exceeds five bytes")
            byte = self.u8()
            byte_count += 1
            magnitude |= (byte & 0x7F) << shift
            shift += 7
            if byte_count == 5:
                break

        value = -magnitude if first & 0x80 else magnitude
        if value < -(1 << 31) or value > (1 << 31) - 1:
            fail(self.path, start, f"{self.context} compact index is outside int32")
        encoded = self.data[start : self.pos]
        if encoded != encode_compact_index(value):
            fail(self.path, start, f"non-canonical {self.context} compact index")
        return value

    def name_string(self) -> tuple[str, str, int]:
        start = self.pos
        count = self.compact_index()
        units = abs(count)
        if units == 0:
            fail(self.path, start, "name FString has zero serialized characters")
        if units > MAX_NAME_UNITS:
            fail(self.path, start, f"name FString has {units} units; maximum is {MAX_NAME_UNITS}")

        if count > 0:
            raw = self.take(units)
            if raw[-1] != 0 or b"\0" in raw[:-1]:
                fail(self.path, start, "ANSI name FString is not singly NUL-terminated")
            text = raw[:-1].decode("latin-1")
            encoding = "ansi"
        else:
            raw = self.take(units * 2)
            code_units = struct.unpack(f"<{units}H", raw)
            if code_units[-1] != 0 or 0 in code_units[:-1]:
                fail(self.path, start, "UTF-16 name FString is not singly NUL-terminated")
            try:
                text = raw[:-2].decode("utf-16le", errors="strict")
            except UnicodeDecodeError as exc:
                fail(self.path, start, f"invalid UTF-16LE name FString: {exc}")
            encoding = "utf-16le"
        if not text:
            fail(self.path, start, "empty package name entry")
        return text, encoding, count

    def ansi_cstring(self) -> str:
        """Pre-64 name entry: NUL-terminated ANSI, no length prefix (UnName.cpp:236)."""
        start = self.pos
        end = self.data.find(b"\0", start)
        if end < 0:
            fail(self.path, start, "pre-64 ANSI name entry has no NUL terminator")
        if end - start + 1 > MAX_NAME_UNITS:
            fail(self.path, start, f"pre-64 ANSI name entry exceeds {MAX_NAME_UNITS} units")
        raw = self.take(end - start + 1)
        text = raw[:-1].decode("latin-1")
        if not text:
            fail(self.path, start, "empty package name entry")
        return text


@dataclass(frozen=True)
class NameEntry:
    text: str
    flags: int
    encoding: str
    serialized_character_count: int
    record_offset: int
    record_size: int


@dataclass(frozen=True)
class ImportEntry:
    class_package_index: int
    class_name_index: int
    outer_ref: int
    object_name_index: int
    record_offset: int
    record_size: int


@dataclass(frozen=True)
class ExportEntry:
    class_ref: int
    super_ref: int
    outer_ref: int
    object_name_index: int
    object_flags: int
    serial_size: int
    serial_offset: int | None
    record_offset: int
    record_size: int


class ObjectPathResolver:
    def __init__(
        self,
        path: str,
        package_name: str,
        names: Sequence[NameEntry],
        imports: Sequence[ImportEntry],
        exports: Sequence[ExportEntry],
    ) -> None:
        self.path = path
        self.package_name = package_name
        self.names = names
        self.imports = imports
        self.exports = exports
        self.cache: dict[tuple[str, int], str] = {}

    def ref_path(self, ref: int, stack: tuple[tuple[str, int], ...] = ()) -> str | None:
        if ref == 0:
            return None
        if ref > 0:
            return self.export_path(ref - 1, stack)
        return self.import_path(-ref - 1, stack)

    def import_path(self, index: int, stack: tuple[tuple[str, int], ...] = ()) -> str:
        key = ("import", index)
        if key in self.cache:
            return self.cache[key]
        if key in stack:
            fail(self.path, self.imports[index].record_offset, "cycle in import/export outer references")
        entry = self.imports[index]
        outer = self.ref_path(entry.outer_ref, stack + (key,))
        name = self.names[entry.object_name_index].text
        result = f"{outer}.{name}" if outer else name
        self.cache[key] = result
        return result

    def export_path(self, index: int, stack: tuple[tuple[str, int], ...] = ()) -> str:
        key = ("export", index)
        if key in self.cache:
            return self.cache[key]
        if key in stack:
            fail(self.path, self.exports[index].record_offset, "cycle in import/export outer references")
        entry = self.exports[index]
        outer = self.ref_path(entry.outer_ref, stack + (key,))
        if outer is None:
            outer = self.package_name
        name = self.names[entry.object_name_index].text
        result = f"{outer}.{name}"
        self.cache[key] = result
        return result


@dataclass(frozen=True)
class ParsedPackage:
    """Structural parse result shared by reference rendering and audit mode."""

    summary: dict[str, Any]
    names: list[NameEntry]
    imports: list[ImportEntry]
    exports: list[ExportEntry]
    names_end: int
    imports_end: int
    exports_end: int
    resolver: ObjectPathResolver


class PackageReader:
    def __init__(self, relative_path: str, data: bytes) -> None:
        self.relative_path = relative_path
        self.data = data
        self.package_name = Path(relative_path).stem

    def validate_count(self, value: int, offset: int, label: str) -> None:
        if value < 0:
            fail(self.relative_path, offset, f"negative {label}: {value}")
        if value > len(self.data):
            fail(self.relative_path, offset, f"implausible {label}: {value} exceeds file size")

    def validate_offset(self, value: int, offset: int, label: str) -> None:
        if value < 0 or value > len(self.data):
            fail(
                self.relative_path,
                offset,
                f"{label} {value} is outside file size {len(self.data)}",
            )

    def validate_name_index(self, value: int, offset: int, label: str, names: Sequence[NameEntry]) -> None:
        if not 0 <= value < len(names):
            fail(self.relative_path, offset, f"{label} {value} is outside name table count {len(names)}")

    def validate_object_ref(
        self,
        value: int,
        offset: int,
        label: str,
        imports: Sequence[ImportEntry],
        exports: Sequence[ExportEntry],
    ) -> None:
        if value > 0 and value > len(exports):
            fail(self.relative_path, offset, f"{label} export reference {value} is out of range")
        if value < 0 and -value > len(imports):
            fail(self.relative_path, offset, f"{label} import reference {value} is out of range")

    def parse_summary(self) -> dict[str, Any]:
        cursor = Cursor(self.data, self.relative_path, 0, "package summary")
        tag = cursor.u32()
        version_word = cursor.u32()
        package_flags = cursor.u32()
        name_count_offset = cursor.pos
        name_count = cursor.i32()
        name_offset_position = cursor.pos
        name_offset = cursor.i32()
        export_count_offset = cursor.pos
        export_count = cursor.i32()
        export_offset_position = cursor.pos
        export_offset = cursor.i32()
        import_count_offset = cursor.pos
        import_count = cursor.i32()
        import_offset_position = cursor.pos
        import_offset = cursor.i32()
        version = version_word & 0xFFFF
        licensee = (version_word >> 16) & 0xFFFF
        guid_words = [cursor.u32() for _ in range(4)] if version >= 68 else []
        generation_count_offset = cursor.pos
        generation_count = cursor.i32() if version >= 68 else 0
        heritage_count_offset = cursor.pos
        heritage_count = cursor.i32() if version < 68 else 0
        heritage_offset_position = cursor.pos
        heritage_offset = cursor.i32() if version < 68 else 0

        if tag != PACKAGE_TAG:
            fail(self.relative_path, 0, f"bad package tag 0x{tag:08x}; expected 0x{PACKAGE_TAG:08x}")
        if not PACKAGE_MIN_VERSION <= version <= PACKAGE_MAX_VERSION or licensee != LICENSEE_VERSION:
            fail(
                self.relative_path,
                4,
                f"unsupported version/licensee {version}/{licensee}; supported: FileVersion in "
                f"[{PACKAGE_MIN_VERSION}, {PACKAGE_MAX_VERSION}] with LicenseeVersion {LICENSEE_VERSION}",
            )
        if version >= 68:
            self.validate_count(generation_count, generation_count_offset, "generation count")
            if generation_count == 0:
                fail(self.relative_path, generation_count_offset, "package has no generations")
            if generation_count > (len(self.data) - cursor.pos) // 8:
                fail(self.relative_path, generation_count_offset, "generation table is truncated")
        else:
            self.validate_count(heritage_count, heritage_count_offset, "heritage count")
            self.validate_offset(heritage_offset, heritage_offset_position, "heritage offset")

        self.validate_count(name_count, name_count_offset, "name count")
        self.validate_count(export_count, export_count_offset, "export count")
        self.validate_count(import_count, import_count_offset, "import count")
        generations = []
        for index in range(generation_count):
            record_offset = cursor.pos
            generation_export_count = cursor.i32()
            generation_name_count = cursor.i32()
            self.validate_count(generation_export_count, record_offset, "generation export count")
            self.validate_count(generation_name_count, record_offset + 4, "generation name count")
            generations.append(
                {
                    "export_count": generation_export_count,
                    "index": index,
                    "name_count": generation_name_count,
                }
            )

        if version >= 68 and (
            generations[-1]["export_count"] != export_count or generations[-1]["name_count"] != name_count
        ):
            fail(self.relative_path, generation_count_offset, "latest generation counts disagree with summary")

        self.validate_offset(name_offset, name_offset_position, "name offset")
        self.validate_offset(export_offset, export_offset_position, "export offset")
        self.validate_offset(import_offset, import_offset_position, "import offset")
        if version >= 68 and name_offset != cursor.pos:
            fail(
                self.relative_path,
                name_offset_position,
                f"name table begins at {name_offset}, not immediately after summary at {cursor.pos}",
            )
        if version < 68 and name_offset < cursor.pos:
            fail(
                self.relative_path,
                name_offset_position,
                f"name table begins at {name_offset}, before end of heritage summary at {cursor.pos}",
            )

        summary: dict[str, Any] = {
            "export_count": export_count,
            "export_offset": export_offset,
            "header_size": cursor.pos,
            "import_count": import_count,
            "import_offset": import_offset,
            "licensee": licensee,
            "name_count": name_count,
            "name_offset": name_offset,
            "package_flags": package_flags,
            "tag": tag,
            "tag_hex": f"0x{tag:08x}",
            "version": version,
            "version_word": version_word,
        }
        if version >= 68:
            summary["generations"] = generations
            summary["guid"] = {
                "a": guid_words[0],
                "b": guid_words[1],
                "c": guid_words[2],
                "d": guid_words[3],
                "text": "-".join(f"{word:08x}" for word in guid_words),
            }
        else:
            summary["heritage"] = {"count": heritage_count, "offset": heritage_offset}
        return summary

    def parse_names(self, summary: dict[str, Any]) -> tuple[list[NameEntry], int]:
        cursor = Cursor(self.data, self.relative_path, summary["name_offset"], "name table")
        names: list[NameEntry] = []
        pre_64 = summary["version"] < 64
        for _ in range(summary["name_count"]):
            record_offset = cursor.pos
            if pre_64:
                text = cursor.ansi_cstring()
                encoding = "ansi"
                serialized_character_count = len(text) + 1
            else:
                text, encoding, serialized_character_count = cursor.name_string()
            flags = cursor.u32()
            names.append(
                NameEntry(
                    text=text,
                    flags=flags,
                    encoding=encoding,
                    serialized_character_count=serialized_character_count,
                    record_offset=record_offset,
                    record_size=cursor.pos - record_offset,
                )
            )
        return names, cursor.pos

    def parse_imports(
        self, summary: dict[str, Any], names: Sequence[NameEntry]
    ) -> tuple[list[ImportEntry], int]:
        cursor = Cursor(self.data, self.relative_path, summary["import_offset"], "import table")
        imports: list[ImportEntry] = []
        for _ in range(summary["import_count"]):
            record_offset = cursor.pos
            class_package_index = cursor.compact_index()
            class_name_index = cursor.compact_index()
            outer_ref = cursor.i32()
            object_name_index = cursor.compact_index()
            self.validate_name_index(class_package_index, record_offset, "import class-package name", names)
            self.validate_name_index(class_name_index, record_offset, "import class name", names)
            self.validate_name_index(object_name_index, record_offset, "import object name", names)
            imports.append(
                ImportEntry(
                    class_package_index=class_package_index,
                    class_name_index=class_name_index,
                    outer_ref=outer_ref,
                    object_name_index=object_name_index,
                    record_offset=record_offset,
                    record_size=cursor.pos - record_offset,
                )
            )
        return imports, cursor.pos

    def parse_exports(
        self, summary: dict[str, Any], names: Sequence[NameEntry]
    ) -> tuple[list[ExportEntry], int]:
        cursor = Cursor(self.data, self.relative_path, summary["export_offset"], "export table")
        exports: list[ExportEntry] = []
        for _ in range(summary["export_count"]):
            record_offset = cursor.pos
            class_ref = cursor.compact_index()
            super_ref = cursor.compact_index()
            outer_ref = cursor.i32()
            object_name_index = cursor.compact_index()
            object_flags = cursor.u32()
            serial_size = cursor.compact_index()
            if serial_size < 0:
                fail(self.relative_path, cursor.pos, f"negative export serial size {serial_size}")
            serial_offset: int | None = None
            if serial_size:
                serial_offset = cursor.compact_index()
                if serial_offset < 0:
                    fail(self.relative_path, cursor.pos, f"negative export serial offset {serial_offset}")
            self.validate_name_index(object_name_index, record_offset, "export object name", names)
            exports.append(
                ExportEntry(
                    class_ref=class_ref,
                    super_ref=super_ref,
                    outer_ref=outer_ref,
                    object_name_index=object_name_index,
                    object_flags=object_flags,
                    serial_size=serial_size,
                    serial_offset=serial_offset,
                    record_offset=record_offset,
                    record_size=cursor.pos - record_offset,
                )
            )
        return exports, cursor.pos

    def validate_regions(
        self,
        summary: dict[str, Any],
        names_end: int,
        imports: Sequence[ImportEntry],
        imports_end: int,
        exports: Sequence[ExportEntry],
        exports_end: int,
    ) -> None:
        if not names_end <= summary["import_offset"] <= summary["export_offset"]:
            fail(self.relative_path, names_end, "name/payload/import/export regions are out of order")
        if imports_end != summary["export_offset"]:
            fail(
                self.relative_path,
                imports_end,
                f"import table ends at {imports_end}, expected export offset {summary['export_offset']}",
            )
        if exports_end != len(self.data):
            fail(
                self.relative_path,
                exports_end,
                f"export table ends at {exports_end}, expected file size {len(self.data)}",
            )

        spans: list[tuple[int, int, int]] = []
        for index, entry in enumerate(exports):
            if not entry.serial_size:
                if entry.serial_offset is not None:
                    fail(self.relative_path, entry.record_offset, "zero-size export unexpectedly has an offset")
                continue
            assert entry.serial_offset is not None
            end = entry.serial_offset + entry.serial_size
            if entry.serial_offset < names_end or end > summary["import_offset"]:
                fail(
                    self.relative_path,
                    entry.record_offset,
                    f"export {index} serial span [{entry.serial_offset}, {end}) is outside payload region "
                    f"[{names_end}, {summary['import_offset']})",
                )
            spans.append((entry.serial_offset, end, index))

        spans.sort()
        expected = names_end
        for start, end, index in spans:
            if start != expected:
                relation = "overlaps" if start < expected else "leaves a gap before"
                fail(
                    self.relative_path,
                    start,
                    f"export {index} serial span {relation} payload offset {expected}",
                )
            expected = end
        if expected != summary["import_offset"]:
            fail(
                self.relative_path,
                expected,
                f"export serial spans end at {expected}, expected import offset {summary['import_offset']}",
            )


    def parse_function_terminal(self, entry: ExportEntry) -> dict[str, Any]:
        # In version 79, UFunction::Serialize ends with iNative, OperPrecedence,
        # FunctionFlags, and (only for FUNC_Net) RepOffset. The export serial
        # span therefore gives the exact end boundary for these fixed-width fields.
        if entry.serial_offset is None or entry.serial_size < 7:
            fail(self.relative_path, entry.record_offset, "Function export is too small for its terminal fields")
        payload_end = entry.serial_offset + entry.serial_size
        candidates: list[dict[str, Any]] = []

        def consider(is_net: bool) -> None:
            trailer_size = 9 if is_net else 7
            trailer_offset = payload_end - trailer_size
            if trailer_offset < entry.serial_offset:
                return
            native_index = struct.unpack_from("<H", self.data, trailer_offset)[0]
            operator_precedence = self.data[trailer_offset + 2]
            function_flags = struct.unpack_from("<I", self.data, trailer_offset + 3)[0]
            if native_index >= MAX_NATIVE_INDEX or function_flags & ~FUNCTION_FLAG_MASK:
                return
            if bool(function_flags & FUNCTION_FLAG_NET) != is_net:
                return
            if function_flags & FUNCTION_FLAG_NET_RELIABLE and not is_net:
                return
            if native_index and not function_flags & FUNCTION_FLAG_NATIVE:
                return

            candidate: dict[str, Any] = {
                "function_flags": function_flags,
                "native_index": native_index,
                "operator_precedence": operator_precedence,
                "payload_end": payload_end,
                "payload_offset": entry.serial_offset,
                "payload_size": entry.serial_size,
                "trailer_offset": trailer_offset,
            }
            if is_net:
                candidate["rep_offset"] = struct.unpack_from("<H", self.data, trailer_offset + 7)[0]
            candidates.append(candidate)

        consider(False)
        consider(True)
        if len(candidates) != 1:
            fail(
                self.relative_path,
                entry.serial_offset,
                f"Function export has {len(candidates)} structurally consistent terminal layouts; expected one",
            )
        return candidates[0]

    def parse(self) -> ParsedPackage:
        summary = self.parse_summary()
        names, names_end = self.parse_names(summary)
        imports, imports_end = self.parse_imports(summary, names)
        exports, exports_end = self.parse_exports(summary, names)

        for entry in imports:
            self.validate_object_ref(entry.outer_ref, entry.record_offset, "import outer", imports, exports)
        for entry in exports:
            self.validate_object_ref(entry.class_ref, entry.record_offset, "export class", imports, exports)
            self.validate_object_ref(entry.super_ref, entry.record_offset, "export super", imports, exports)
            self.validate_object_ref(entry.outer_ref, entry.record_offset, "export outer", imports, exports)

        self.validate_regions(summary, names_end, imports, imports_end, exports, exports_end)
        resolver = ObjectPathResolver(self.relative_path, self.package_name, names, imports, exports)
        for index in range(len(imports)):
            resolver.import_path(index)
        for index in range(len(exports)):
            resolver.export_path(index)
        return ParsedPackage(
            summary=summary,
            names=names,
            imports=imports,
            exports=exports,
            names_end=names_end,
            imports_end=imports_end,
            exports_end=exports_end,
            resolver=resolver,
        )

    def read(self) -> dict[str, Any]:
        parsed = self.parse()
        summary = parsed.summary
        names = parsed.names
        imports = parsed.imports
        exports = parsed.exports
        names_end = parsed.names_end
        resolver = parsed.resolver
        imports_end = parsed.imports_end
        exports_end = parsed.exports_end

        names_json = [
            {
                "encoding": entry.encoding,
                "flags": entry.flags,
                "index": index,
                "record_offset": entry.record_offset,
                "record_size": entry.record_size,
                "serialized_character_count": entry.serialized_character_count,
                "text": entry.text,
            }
            for index, entry in enumerate(names)
        ]

        imports_json = []
        for index, entry in enumerate(imports):
            outer_path = resolver.ref_path(entry.outer_ref)
            imports_json.append(
                {
                    "class_name": {
                        "index": entry.class_name_index,
                        "text": names[entry.class_name_index].text,
                    },
                    "class_package": {
                        "index": entry.class_package_index,
                        "text": names[entry.class_package_index].text,
                    },
                    "class_path": (
                        f"{names[entry.class_package_index].text}.{names[entry.class_name_index].text}"
                    ),
                    "index": index,
                    "object_name": {
                        "index": entry.object_name_index,
                        "text": names[entry.object_name_index].text,
                    },
                    "object_path": resolver.import_path(index),
                    "outer_path": outer_path,
                    "outer_ref": entry.outer_ref,
                    "record_offset": entry.record_offset,
                    "record_size": entry.record_size,
                }
            )

        exports_json = []
        native_entries = []
        for index, entry in enumerate(exports):
            class_path = "Core.Class" if entry.class_ref == 0 else resolver.ref_path(entry.class_ref)
            object_path = resolver.export_path(index)
            serial: dict[str, Any] | None = None
            if entry.serial_size:
                assert entry.serial_offset is not None
                serial = {
                    "end": entry.serial_offset + entry.serial_size,
                    "offset": entry.serial_offset,
                    "sha256": sha256_span(self.data, entry.serial_offset, entry.serial_size),
                    "size": entry.serial_size,
                }
            export_json: dict[str, Any] = {
                "class_path": class_path,
                "class_ref": entry.class_ref,
                "index": index,
                "object_flags": entry.object_flags,
                "object_name": {
                    "index": entry.object_name_index,
                    "text": names[entry.object_name_index].text,
                },
                "object_path": object_path,
                "outer_path": resolver.ref_path(entry.outer_ref) or self.package_name,
                "outer_ref": entry.outer_ref,
                "record_offset": entry.record_offset,
                "record_size": entry.record_size,
                "serial": serial,
                "super_path": resolver.ref_path(entry.super_ref),
                "super_ref": entry.super_ref,
            }
            if class_path is not None and (class_path == "Core.Function" or class_path.endswith(".Function")):
                function_entry = self.parse_function_terminal(entry)
                function_entry["export_index"] = index
                function_entry["object_path"] = object_path
                native_entries.append(function_entry)
            exports_json.append(export_json)

        regions = {
            "export_table": self.region(summary["export_offset"], exports_end),
            "import_table": self.region(summary["import_offset"], imports_end),
            "name_table": self.region(summary["name_offset"], names_end),
            "serial_payloads": self.region(names_end, summary["import_offset"]),
            "summary": self.region(0, summary["header_size"]),
        }
        return {
            "exports": exports_json,
            "file_sha256": hashlib.sha256(self.data).hexdigest(),
            "file_size": len(self.data),
            "imports": imports_json,
            "names": names_json,
            "native_indices": {
                "entries": native_entries,
                "status": "derived-from-version-79-ufunction-terminal-fields",
            },
            "package_name": self.package_name,
            "path": self.relative_path,
            "regions": regions,
            "summary": summary,
        }

    def region(self, start: int, end: int) -> dict[str, Any]:
        return {
            "end": end,
            "offset": start,
            "sha256": sha256_span(self.data, start, end - start),
            "size": end - start,
        }
    def property_projection(
        self, payload: bytes, names: Sequence[NameEntry]
    ) -> dict[str, Any]:
        """Conservatively project the existing tagged-property prefix.

        UE1 payload families with an object-stack prologue do not begin with a
        property stream.  Those remain explicit hash-only entries rather than
        guessing an alignment or dropping the payload.
        """
        cursor = Cursor(payload, self.relative_path, 0, "export tagged properties")
        property_count = 0
        try:
            while True:
                name_index = cursor.compact_index()
                if not 0 <= name_index < len(names):
                    raise PackageFormatError("tagged property name index is out of range")
                if names[name_index].text == "None":
                    return {
                        "property_bytes": cursor.pos,
                        "property_count": property_count,
                        "reader": "package79.properties",
                        "status": "decoded",
                        "trailing_bytes": len(payload) - cursor.pos,
                    }
                info = cursor.u8()
                kind = info & 0x0F
                size_code = info & 0x70
                array_flag = bool(info & 0x80)
                if kind == 10:
                    struct_name = cursor.compact_index()
                    if not 0 <= struct_name < len(names):
                        raise PackageFormatError("tagged property struct name is out of range")
                if size_code == 0x00:
                    size = 1
                elif size_code == 0x10:
                    size = 2
                elif size_code == 0x20:
                    size = 4
                elif size_code == 0x30:
                    size = 12
                elif size_code == 0x40:
                    size = 16
                elif size_code == 0x50:
                    size = cursor.u8()
                elif size_code == 0x60:
                    size = cursor.u16()
                else:
                    size = cursor.i32()
                    if size < 0:
                        raise PackageFormatError("negative tagged property size")
                if array_flag and kind != 3:
                    cursor.compact_index()
                cursor.take(size)
                property_count += 1
        except (PackageFormatError, IndexError, struct.error):
            return {
                "reader": "package79.properties",
                "status": "hash-only",
            }
    def font_projection(self, payload: bytes) -> dict[str, Any]:
        cursor = Cursor(payload, self.relative_path, 0, "UFont payload")
        try:
            if cursor.compact_index() != 0:
                raise PackageFormatError("UFont payload has tagged properties")
            page_count = cursor.compact_index()
            if page_count < 0:
                raise PackageFormatError("negative UFont page count")
            glyph_count = 0
            for _ in range(page_count):
                cursor.compact_index()
                character_count = cursor.compact_index()
                if character_count < 0:
                    raise PackageFormatError("negative UFont character count")
                glyph_count += character_count
                cursor.take(character_count * 16)
            characters_per_page = cursor.i32()
            remap_count = cursor.compact_index()
            if remap_count < 0:
                raise PackageFormatError("negative UFont remap count")
            cursor.take(remap_count * 8)
            cursor.u32()
            return {
                "characters_per_page": characters_per_page,
                "glyph_count": glyph_count,
                "page_count": page_count,
                "properties": self.property_projection(payload, []),
                "reader": "hp-format.font",
                "remap_count": remap_count,
                "status": "decoded",
            }
        except (PackageFormatError, IndexError, struct.error):
            return {
                "properties": self.property_projection(payload, []),
                "reader": "hp-format.font",
                "status": "hash-only",
            }

    def family_projection(
        self,
        family: str,
        class_path: str | None,
        payload: bytes,
        names: Sequence[NameEntry],
    ) -> dict[str, Any]:
        properties = self.property_projection(payload, names)
        if family == "font":
            projection = self.font_projection(payload)
            projection["properties"] = properties
            return projection
        if family == "mesh":
            # The Rust leg dispatches hp-format::mesh::decode_export.  The
            # canonical projection stays on the shared property/count surface;
            # malformed prefixes are therefore explicit hash-only payloads.
            return {
                "properties": properties,
                "reader": "hp-format.mesh",
                "status": (
                    "projected" if properties["status"] == "decoded" else "hash-only"
                ),
            }
        return properties


    def audit_entry(self, parsed: ParsedPackage) -> dict[str, Any]:
        """Canonical structural and payload projection for one package."""
        exports_summary = []
        native_functions = []
        family_census: dict[str, dict[str, int]] = {}
        for index, entry in enumerate(parsed.exports):
            class_path = (
                "Core.Class"
                if entry.class_ref == 0
                else parsed.resolver.ref_path(entry.class_ref)
            )
            object_path = parsed.resolver.export_path(index)
            family = export_payload_family(self.relative_path, class_path)
            payload: dict[str, Any] | None = None
            if entry.serial_size:
                assert entry.serial_offset is not None
                payload_bytes = self.data[
                    entry.serial_offset : entry.serial_offset + entry.serial_size
                ]
                payload = {
                    "end": entry.serial_offset + entry.serial_size,
                    "family": family,
                    "offset": entry.serial_offset,
                    "projection": self.family_projection(
                        family, class_path, payload_bytes, parsed.names
                    ),
                    "sha256": hashlib.sha256(payload_bytes).hexdigest(),
                    "size": entry.serial_size,
                }
                add_family_census(family_census, family, entry.serial_size)
            exports_summary.append(
                {
                    "class_path": class_path,
                    "object_path": object_path,
                    "payload": payload,
                }
            )
            if class_path is not None and (
                class_path == "Core.Function" or class_path.endswith(".Function")
            ):
                function_entry = self.parse_function_terminal(entry)
                if function_entry["function_flags"] & FUNCTION_FLAG_NATIVE:
                    native_functions.append(
                        {
                            "native_index": function_entry["native_index"],
                            "object_path": object_path,
                        }
                    )
        summary = parsed.summary
        return {
            "export_count": summary["export_count"],
            "exports": exports_summary,
            "file_sha256": hashlib.sha256(self.data).hexdigest(),
            "file_size": len(self.data),
            "import_count": summary["import_count"],
            "licensee": summary["licensee"],
            "name_count": summary["name_count"],
            "native_functions": native_functions,
            "package_flags": summary["package_flags"],
            "path": self.relative_path,
            "payload_families": family_census,
            "payload_region": self.region(
                parsed.names_end, summary["import_offset"]
            ),
            "version": summary["version"],
        }


def generate_reference(repo_root: Path) -> dict[str, Any]:
    packages = []
    for relative_path in PACKAGE_PATHS:
        source = repo_root / Path(relative_path)
        try:
            data = source.read_bytes()
        except OSError as exc:
            raise PackageFormatError(f"{relative_path}: cannot read package: {exc}") from exc
        packages.append(PackageReader(relative_path, data).read())
    return {
        "format": "hp2-ue1-package-reference",
        "licensee_version": LICENSEE_VERSION,
        "package_version": PACKAGE_VERSION,
        "packages": packages,
        "schema_version": 1,
    }


def generate_audit(data_root: Path, repo_root: Path) -> dict[str, Any]:
    """Decode every UE1 package under data_root; classify everything else.

    Top-level *.json provenance manifests directly inside the root (written by
    prepare scripts) are excluded: they are build artifacts, not game data.
    Transient engine crash reports (crash-report.json, written into System/ by
    the crash handler at runtime) are excluded for the same reason: they are
    debris that would otherwise make audits nondeterministic across runs.
    """
    root = data_root.resolve()
    if not root.is_dir():
        raise PackageFormatError(f"{data_root}: data root is not a directory")
    files = sorted(
        source.relative_to(root).as_posix()
        for source in root.rglob("*")
        if source.is_file()
        and not (source.parent == root and source.suffix == ".json")
        and source.name != "crash-report.json"
    )

    packages = []
    non_package_files: list[str] = []
    non_package_payloads: list[dict[str, Any]] = []
    family_census: dict[str, dict[str, int]] = {}
    profiles: dict[str, int] = {}
    for relative_path in files:
        try:
            data = (root / relative_path).read_bytes()
        except OSError as exc:
            raise PackageFormatError(f"{relative_path}: cannot read file: {exc}") from exc
        if len(data) < 4 or struct.unpack_from("<I", data, 0)[0] != PACKAGE_TAG:
            family = non_package_family(relative_path)
            non_package_files.append(relative_path)
            non_package_payloads.append(
                {
                    "family": family,
                    "path": relative_path,
                    "projection": non_package_projection(relative_path, family, data),
                    "sha256": hashlib.sha256(data).hexdigest(),
                    "size": len(data),
                }
            )
            add_family_census(family_census, family, len(data))
            continue
        reader = PackageReader(relative_path, data)
        entry = reader.audit_entry(reader.parse())
        packages.append(entry)
        for family, counts in entry["payload_families"].items():
            aggregate = family_census.setdefault(
                family, {"bytes": 0, "payload_count": 0}
            )
            aggregate["bytes"] += counts["bytes"]
            aggregate["payload_count"] += counts["payload_count"]
        profile = f"v{entry['version']}/licensee{entry['licensee']}/flags{entry['package_flags']}"
        profiles[profile] = profiles.get(profile, 0) + 1

    try:
        data_root_display = str(root.relative_to(repo_root))
    except ValueError:
        data_root_display = str(root)
    return {
        "accepted_versions": {"max": PACKAGE_MAX_VERSION, "min": PACKAGE_MIN_VERSION},
        "census": {
            "file_count": len(files),
            "format_profiles": profiles,
            "non_package_count": len(non_package_files),
            "package_count": len(packages),
        },
        "data_root": data_root_display,
        "format": "hp1-ue1-package-audit",
        "non_package_files": non_package_files,
        "non_package_payloads": non_package_payloads,
        "packages": packages,
        "payload_family_census": family_census,
        "schema_version": AUDIT_SCHEMA_VERSION,
    }


def canonical_json(reference: dict[str, Any]) -> bytes:
    return (json.dumps(reference, ensure_ascii=False, indent=2, sort_keys=True) + "\n").encode("utf-8")


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    script_repo_root = Path(__file__).resolve().parent.parent
    parser = argparse.ArgumentParser(
        description=(
            "UE1 package reader: generate/verify the HP2 v79 reference golden (default) "
            "or audit every package under a data root (--data-root)."
        )
    )
    parser.add_argument(
        "--repo-root",
        type=Path,
        default=script_repo_root,
        help="repository root containing HarryPotter2/Unreal (default: script parent)",
    )
    parser.add_argument(
        "--data-root",
        type=Path,
        default=None,
        help=(
            "audit every UE1 package under this data root (e.g. HarryPotter1/Unreal) "
            "instead of generating the six-package HP2 v79 reference"
        ),
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=None,
        help=(
            f"output path, relative to repo root unless absolute "
            f"(default: {DEFAULT_OUTPUT}, or {DEFAULT_AUDIT_OUTPUT} with --data-root)"
        ),
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="verify that the existing output is byte-identical instead of writing it",
    )
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(argv)
    repo_root = args.repo_root.resolve()
    if args.output is not None:
        output_path = args.output
    else:
        output_path = Path(DEFAULT_AUDIT_OUTPUT if args.data_root is not None else DEFAULT_OUTPUT)
    output = output_path if output_path.is_absolute() else repo_root / output_path
    try:
        if args.data_root is not None:
            encoded = canonical_json(generate_audit(args.data_root, repo_root))
        else:
            encoded = canonical_json(generate_reference(repo_root))
        if args.check:
            try:
                existing = output.read_bytes()
            except OSError as exc:
                raise PackageFormatError(f"{output}: cannot read existing fixture: {exc}") from exc
            if existing != encoded:
                raise PackageFormatError(f"{output}: fixture differs; regenerate without --check")
            print(f"verified {output}")
        else:
            output.parent.mkdir(parents=True, exist_ok=True)
            output.write_bytes(encoded)
            print(f"wrote {output} ({len(encoded)} bytes)")
    except (OSError, PackageFormatError, ValueError) as exc:
        print(f"package79_reference.py: error: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
