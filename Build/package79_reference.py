#!/usr/bin/env python3
"""Generate a deterministic reference manifest for the six stock HP2 UE1 packages."""

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
        guid_words = [cursor.u32() for _ in range(4)]
        generation_count_offset = cursor.pos
        generation_count = cursor.i32()

        if tag != PACKAGE_TAG:
            fail(self.relative_path, 0, f"bad package tag 0x{tag:08x}; expected 0x{PACKAGE_TAG:08x}")
        version = version_word & 0xFFFF
        licensee = (version_word >> 16) & 0xFFFF
        if version != PACKAGE_VERSION or licensee != LICENSEE_VERSION:
            fail(
                self.relative_path,
                4,
                f"unsupported version/licensee {version}/{licensee}; expected {PACKAGE_VERSION}/{LICENSEE_VERSION}",
            )

        self.validate_count(name_count, name_count_offset, "name count")
        self.validate_count(export_count, export_count_offset, "export count")
        self.validate_count(import_count, import_count_offset, "import count")
        self.validate_count(generation_count, generation_count_offset, "generation count")
        if generation_count == 0:
            fail(self.relative_path, generation_count_offset, "package has no generations")
        if generation_count > (len(self.data) - cursor.pos) // 8:
            fail(self.relative_path, generation_count_offset, "generation table is truncated")

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

        if generations[-1]["export_count"] != export_count or generations[-1]["name_count"] != name_count:
            fail(self.relative_path, generation_count_offset, "latest generation counts disagree with summary")

        self.validate_offset(name_offset, name_offset_position, "name offset")
        self.validate_offset(export_offset, export_offset_position, "export offset")
        self.validate_offset(import_offset, import_offset_position, "import offset")
        if name_offset != cursor.pos:
            fail(
                self.relative_path,
                name_offset_position,
                f"name table begins at {name_offset}, not immediately after summary at {cursor.pos}",
            )

        return {
            "export_count": export_count,
            "export_offset": export_offset,
            "generations": generations,
            "guid": {
                "a": guid_words[0],
                "b": guid_words[1],
                "c": guid_words[2],
                "d": guid_words[3],
                "text": "-".join(f"{word:08x}" for word in guid_words),
            },
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

    def parse_names(self, summary: dict[str, Any]) -> tuple[list[NameEntry], int]:
        cursor = Cursor(self.data, self.relative_path, summary["name_offset"], "name table")
        names: list[NameEntry] = []
        for _ in range(summary["name_count"]):
            record_offset = cursor.pos
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

    def read(self) -> dict[str, Any]:
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


def canonical_json(reference: dict[str, Any]) -> bytes:
    return (json.dumps(reference, ensure_ascii=False, indent=2, sort_keys=True) + "\n").encode("utf-8")


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    script_repo_root = Path(__file__).resolve().parent.parent
    parser = argparse.ArgumentParser(
        description="Generate the fixed-width UE1 package-79 reference for the six untouched HP2 assets."
    )
    parser.add_argument(
        "--repo-root",
        type=Path,
        default=script_repo_root,
        help="repository root containing HarryPotter2/Unreal (default: script parent)",
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=Path(DEFAULT_OUTPUT),
        help=f"output path, relative to repo root unless absolute (default: {DEFAULT_OUTPUT})",
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
    output = args.output if args.output.is_absolute() else repo_root / args.output
    try:
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
