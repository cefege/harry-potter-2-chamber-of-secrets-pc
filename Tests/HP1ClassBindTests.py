#!/usr/bin/env python3
"""HP1 class-bind contract: Engine.Music resolves through the native registry.

HP1's Engine.u imports Engine.Music (Core/Class Music inside the Engine
package import) and derives two script classes from it, but no HP1 package
exports a Music class -- the original DLL-based engine satisfied it natively.
Our port mirrors that: UMusic is registered into the Engine package by
IMPLEMENT_CLASS(UMusic), and ULinkerLoad::VerifyImport's existing
RF_Public|RF_Native|RF_Transient fallback binds the import without weakening
validation (a genuinely missing class still throws FailedImport).

Layers checked here:
  data     - Engine.u really contains the two Music-derived script classes and
             the Engine.Music class-object import this contract exists for;
  registry - the native registration lines that create Engine.Music exist;
  strict   - VerifyImport's unconditional FailedImport rejection is intact,
             proven live: a byte-tampered Engine.u copy (one import's class
             name index flipped to a name that is not a class) must still
             hard-fail with the FailedImport family, never Broken import;
  runtime  - hp2_ucc against the HP1 root loads and verifies both Engine.u and
             Editor.u end to end (log shows both packages purged on exit);
  baseline - the same ucc invocation against the HP2 root stays exit-0 clean.

Pure stdlib plus one subprocess against the already-built hp2_ucc. When
HP2_ARTIFACT_DIR is set a report-schema-v1 JSON document is emitted there.
"""

from __future__ import annotations

import json
import os
import re
import shutil
import struct
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

REPOSITORY_ROOT = Path(__file__).resolve().parent.parent
HP1_ROOT = REPOSITORY_ROOT / "HarryPotter1" / "Unreal"
ENGINE_U = HP1_ROOT / "System" / "Engine.u"
UN_LINKER_CPP = REPOSITORY_ROOT / "HarryPotter2" / "Unreal" / "Core" / "Src" / "UnLinker.cpp"
UN_AUDIO_H = REPOSITORY_ROOT / "HarryPotter2" / "Unreal" / "Engine" / "Inc" / "UnAudio.h"
UN_AUDIO_CPP = REPOSITORY_ROOT / "HarryPotter2" / "Unreal" / "Engine" / "Src" / "UnAudio.cpp"
UN_ENGINE_NATIVE_H = REPOSITORY_ROOT / "HarryPotter2" / "Unreal" / "Engine" / "Inc" / "UnEngineNative.h"

MUSIC_DERIVED_CLASSES = ("JS_HP_Title_Screen_v2", "JS_StoryBook_v2_mx")

REPORT_NAME = "hp1-class-bind.json"


def read_compact_index(data: bytes | bytearray, pos: int) -> tuple[int, int]:
    first = data[pos]
    pos += 1
    magnitude = first & 0x3F
    shift = 6
    count = 1
    byte = first
    while byte & (0x40 if count == 1 else 0x80):
        if count == 5:
            raise ValueError("compact index exceeds five bytes")
        byte = data[pos]
        pos += 1
        count += 1
        magnitude |= (byte & 0x7F) << shift
        shift += 7
    value = -magnitude if first & 0x80 else magnitude
    return value, pos


class PackageTables:
    """Minimal name/import/export table reader (post-v68 layout, ANSI names)."""

    def __init__(self, path: Path):
        data = path.read_bytes()
        self.version = struct.unpack_from("<i", data, 4)[0] & 0xFFFF
        name_count, name_offset, export_count, export_offset, import_count, import_offset = (
            struct.unpack_from("<iiiiii", data, 12))
        self.names: list[str] = []
        pos = name_offset
        for _ in range(name_count):
            length, pos = read_compact_index(data, pos)
            if length < 0:
                raise ValueError(f"{path.name}: UTF-16 name entry not expected here")
            self.names.append(data[pos:pos + length - 1].decode("latin1"))
            pos += length + 4  # string bytes + u32 flags
        self.imports: list[tuple[int, int, int, int]] = []
        self.import_class_name_offsets: list[int] = []
        pos = import_offset
        for _ in range(import_count):
            cp, pos = read_compact_index(data, pos)
            class_name_offset = pos
            cn, pos = read_compact_index(data, pos)
            package_index = struct.unpack_from("<i", data, pos)[0]
            pos += 4
            on, pos = read_compact_index(data, pos)
            self.imports.append((cp, cn, package_index, on))
            self.import_class_name_offsets.append(class_name_offset)
        self.exports: list[tuple[int, int, int]] = []  # (class_index, name_index, outer)
        for _ in range(export_count):
            cls, pos = read_compact_index(data, pos)
            sup, pos = read_compact_index(data, pos)
            outer = struct.unpack_from("<i", data, pos)[0]
            pos += 4
            nm, pos = read_compact_index(data, pos)
            pos += 4
            serial_size, pos = read_compact_index(data, pos)
            if serial_size:
                _serial_offset, pos = read_compact_index(data, pos)
            self.exports.append((cls, nm, outer))

    def import_chain_top(self, index: int) -> str:
        idx = index
        while True:
            _, _, package_index, object_name = self.imports[idx]
            if package_index == 0:
                return self.names[object_name]
            idx = -package_index - 1


def find_music_import(tables: PackageTables) -> int:
    hits = [i for i, entry in enumerate(tables.imports)
            if tables.names[entry[3]] == "Music"]
    if len(hits) != 1:
        raise AssertionError(f"expected exactly one Engine.u Music import, got {hits}")
    return hits[0]


def music_derived_exports(tables: PackageTables, music_import: int) -> list[str]:
    derived = []
    for cls, name_index, _outer in tables.exports:
        if cls < 0 and -cls - 1 == music_import:
            derived.append(tables.names[name_index])
    return sorted(derived)


def compact_index_length(value: int) -> int:
    """Byte width FCompactIndex needs for a non-negative value."""
    if value < 0x40:
        return 1
    length = 1
    magnitude = value >> 6
    while magnitude >= 0x80:
        magnitude >>= 7
        length += 1
    return length + 1


def encode_compact_index(value: int) -> bytes:
    """Inverse of read_compact_index for non-negative values."""
    if value < 0 or value >= 0x8000000:
        raise ValueError(f"compact index out of range: {value}")
    if value < 0x40:
        return bytes([value])
    out = bytearray([(value & 0x3F) | 0x40])
    rest = value >> 6
    while rest >= 0x80:
        out.append((rest & 0x7F) | 0x80)
        rest >>= 7
    out.append(rest)
    return bytes(out)


def resolve_ucc_binary() -> Path | None:
    """HP2_UCC_BINARY wins; else out/macos-arm64/hp2_ucc, then any build."""
    from_env = os.environ.get("HP2_UCC_BINARY")
    candidates = [Path(from_env)] if from_env else []
    candidates.append(REPOSITORY_ROOT / "out" / "macos-arm64" / "hp2_ucc")
    out_root = REPOSITORY_ROOT / "out"
    if out_root.is_dir():
        candidates.extend(sorted(out_root.glob("*/hp2_ucc")))
    for candidate in candidates:
        if candidate.is_file() and os.access(candidate, os.X_OK):
            return candidate
    return None


def emit_report(name: str, payload: dict) -> None:
    artifact_dir = os.environ.get("HP2_ARTIFACT_DIR")
    if not artifact_dir:
        return
    path = Path(artifact_dir) / f"{name}.json"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=1, sort_keys=True))


class HP1ClassBindTests(unittest.TestCase):
    def test_engine_u_music_contract(self) -> None:
        """Data + registry + strictness layers; runtime layer runs separately."""
        if not ENGINE_U.is_file():
            self.skipTest("HP1 data root not present")
        tables = PackageTables(ENGINE_U)

        music_import = find_music_import(tables)
        cp, cn, package_index, on = tables.imports[music_import]
        self.assertEqual(tables.names[cp], "Core", "Music import class package")
        self.assertEqual(tables.names[cn], "Class", "Music import class name")
        self.assertLess(package_index, 0, "Music import must sit inside a parent import")
        self.assertEqual(tables.import_chain_top(music_import), "Engine",
                         "Music import must resolve against the Engine package")

        derived = music_derived_exports(tables, music_import)
        self.assertEqual(derived, sorted(MUSIC_DERIVED_CLASSES),
                         "Engine.u Music-derived script classes drifted")

        # Registry layer: the native registration that creates Engine.Music.
        native_header = UN_ENGINE_NATIVE_H.read_text()
        self.assertIn("UMusic::StaticClass();", native_header,
                      "UMusic missing from AUTO_INITIALIZE_REGISTRANTS_ENGINE")
        audio_impl = UN_AUDIO_CPP.read_text()
        self.assertIn("IMPLEMENT_CLASS(UMusic);", audio_impl,
                      "UMusic implementation not registered")
        audio_header = UN_AUDIO_H.read_text()
        self.assertRegex(audio_header,
                         r"class\s+ENGINE_API\s+UMusic\s*:\s*public\s+UObject")

        # Strictness layer: genuinely missing classes must still be rejected.
        linker_cpp = UN_LINKER_CPP.read_text()
        failed_import_throws = linker_cpp.count('appThrowf( LocalizeError("FailedImport")')
        self.assertEqual(failed_import_throws, 1,
                         "VerifyImport FailedImport rejection changed shape")

        payload = {
            "music_import_index": music_import,
            "music_import_path": "Engine.Music",
            "derived_classes": derived,
            "engine_u_version": tables.version,
        }
        emit_report("hp1-class-bind", {
            "schema": "report-schema-v1",
            "name": "hp1-class-bind",
            "status": "passed",
            "invariant": ("HP1 Engine.Music binds through the native registry "
                          "and strict import validation stays intact"),
            "reason_code": "",
            "data": {"profile": "data-hp1", **payload},
            "artifacts": [],
            "command": [],
            "exit_reason": "",
        })

    def test_runtime_linker_loads_hp1_base_packages(self) -> None:
        """Runtime layer: hp2_ucc must verify Engine.u and Editor.u cleanly."""
        binary = resolve_ucc_binary()
        if binary is None:
            self.skipTest("hp2_ucc binary not provided or not built")
        if not (HP1_ROOT / "System" / "Default.ini").is_file():
            self.skipTest("HP1 data root not present")

        with tempfile.TemporaryDirectory(prefix="hp1-class-bind.") as tmp:
            log_path = Path(tmp) / "UCC.log"
            # Listing run: bare `help` prints the commandlet registry (HP1's
            # Core.u exports HelloWorldCommandlet; Editor.MasterCommandlet is
            # HP2-only, so the HP1 listing is the honest "Editor commandlets
            # present" proof). No ABSLOG here: with no commandlet argument to
            # soak it up, the verb would try to resolve ABSLOG=... as a name.
            listed = subprocess.run(
                [str(binary), "-datadir=" + str(HP1_ROOT), "help"],
                capture_output=True, text=True, timeout=120)
            # Verification run: resolving a named commandlet forces the boot
            # to load every Paths package -- where VerifyImport runs -- and
            # the trailing ABSLOG token captures the engine log.
            verified = subprocess.run(
                [str(binary), "-datadir=" + str(HP1_ROOT), "help",
                 "Editor.MasterCommandlet", f"ABSLOG={log_path}"],
                capture_output=True, text=True, timeout=120)
            self.assertTrue(log_path.is_file(), "ucc engine log was not written")
            log_text = log_path.read_text(errors="replace").replace("\x00", "")

        self.assertEqual(listed.returncode, 0, "ucc help listing exited nonzero")
        self.assertIn('Commands for "ucc":', listed.stdout,
                      "commandlet registry listing missing")
        self.assertIn("HelloWorld", listed.stdout,
                      "HP1 HelloWorld commandlet not listed")
        self.assertIn("Unloading: Package Engine", log_text,
                      "Engine.u did not load and verify cleanly")
        self.assertIn("Unloading: Package Editor", log_text,
                      "Editor.u did not load and verify cleanly")
        self.assertNotIn("Failed import:", log_text,
                         "unexpected unresolved imports under the HP1 root")
        emit_report("hp1-class-bind-runtime", {
            "schema": "report-schema-v1",
            "name": "hp1-class-bind-runtime",
            "status": "passed",
            "invariant": "hp2_ucc verifies HP1 Engine.u and Editor.u end to end",
            "reason_code": "",
            "data": {"profile": "data-hp1"},
            "artifacts": [],
            "command": [],
            "exit_reason": "",
        })

    def test_tampered_engine_u_import_fails_strictly(self) -> None:
        """A corrupted import must still hard-fail; validation is not forgiving.

        Tamper design (offsets computed on the pristine file, bytes flipped
        only inside a TMPDIR copy): each FObjectImport is
        ClassPackage|ClassName|PackageIndex(s32)|ObjectName as compact
        indices except the s32. PackageTables records the byte offset of
        every import's ClassName field; for the Engine.Music import that
        field currently holds the name index of "Class". We overwrite
        exactly those bytes with a different valid name index chosen so
        that (a) its compact encoding has the SAME byte width -- every
        later byte in the file keeps its offset, so the package still
        parses -- and (b) the name is NOT a class exported by Core.u, so
        FindObject<UClass>(Core, <bogus>) misses and VerifyImport falls
        through to appThrowf(LocalizeError("FailedImport")). An
        out-of-range index would crash the name lookup instead of
        exercising strictness; an in-range non-class name cannot.
        """
        binary = resolve_ucc_binary()
        if binary is None:
            self.skipTest("hp2_ucc binary not provided or not built")
        if not ENGINE_U.is_file():
            self.skipTest("HP1 data root not present")

        pristine = ENGINE_U.read_bytes()
        tables = PackageTables(ENGINE_U)
        music_import = find_music_import(tables)
        class_name_offset = tables.import_class_name_offsets[music_import]
        _, end = read_compact_index(pristine, class_name_offset)
        width = end - class_name_offset

        core_tables = PackageTables(HP1_ROOT / "System" / "Core.u")
        core_class_names = {tables.names[nm] for _cls, nm, _outer in core_tables.exports}
        bogus = next(j for j, name in enumerate(tables.names)
                     if compact_index_length(j) == width
                     and name not in core_class_names
                     and name not in ("Class", "Core", "Music"))
        replacement = encode_compact_index(bogus)
        self.assertEqual(len(replacement), width,
                         "bogus name index must encode to the same width")
        self.assertEqual(read_compact_index(replacement, 0), (bogus, len(replacement)),
                         "replacement must round-trip through the reader")

        original_index = tables.imports[music_import][1]
        tampered = bytearray(pristine)
        tampered[class_name_offset:class_name_offset + width] = replacement
        self.assertNotEqual(tampered, pristine)
        # Everything after the flip must stay byte-identical.
        self.assertEqual(tampered[:class_name_offset], pristine[:class_name_offset])
        self.assertEqual(tampered[class_name_offset + width:], pristine[class_name_offset + width:])

        with tempfile.TemporaryDirectory(prefix="hp1-tamper.") as tmp:
            root = Path(tmp) / "Unreal"
            shutil.copytree(HP1_ROOT / "System", root / "System",
                            copy_function=shutil.copy2)
            for sub in ("Maps", "Textures", "Sounds", "Music", "Save", "Cache"):
                (root / sub).mkdir()
            target = root / "System" / "Engine.u"
            target.write_bytes(tampered)
            log_path = Path(tmp) / "UCC.log"
            result = subprocess.run(
                # Resolving a named commandlet forces the boot to load every
                # Paths package, which is where VerifyImport runs. The trailing
                # ABSLOG token is what FOutputDeviceFile picks up.
                [str(binary), "-datadir=" + str(root), "help",
                 "Editor.MasterCommandlet", f"ABSLOG={log_path}"],
                capture_output=True, text=True, timeout=120)
            self.assertTrue(log_path.is_file(), "ucc engine log was not written")
            log_text = log_path.read_text(errors="replace").replace("\x00", "")

        # Strictness signal is the VerifyImport rejection in the log, not the
        # process exit code: `ucc help` wraps each Paths package in stock
        # UObject::SafeLoadError (LOAD_Forgiving at the enumeration layer,
        # pre-existing engine behavior, untouched by the UMusic change), so a
        # failed package load degrades to "Commandlet ... not found" instead
        # of a nonzero exit. What must hold: VerifyImport emitted its
        # unconditional FailedImport rejection naming the bogus class, and
        # Engine.u never completed clean verification.
        bogus_name = tables.names[bogus]
        self.assertIn(f"Failed import: {bogus_name} {bogus_name} Engine.Music",
                      log_text, "loader did not emit the strict FailedImport rejection")
        self.assertIn(f"Can't find {bogus_name} in file '{bogus_name} Engine.Music'",
                      log_text, "unexpected FailedImport rendering")
        self.assertNotIn("Broken import:", log_text,
                         "VerifyImport's LOAD_Forgiving branch must not be taken")
        self.assertNotIn("Unloading: Package Engine", log_text,
                         "tampered Engine.u must not verify cleanly")
        emit_report("hp1-class-bind-tamper", {
            "schema": "report-schema-v1",
            "name": "hp1-class-bind-tamper",
            "status": "passed",
            "invariant": ("a corrupted Engine.u import hard-fails with "
                          "FailedImport; strict import validation intact"),
            "reason_code": "",
            "data": {
                "profile": "data-hp1",
                "music_import_index": music_import,
                "class_name_field_offset": class_name_offset,
                "original_name_index": original_index,
                "tampered_name_index": bogus,
                "tampered_name": tables.names[bogus],
                "encoded_width": width,
            },
            "artifacts": [],
            "command": [],
            "exit_reason": f"ucc exited {result.returncode} on tampered copy",
        })

    def test_hp2_baseline_help_unchanged(self) -> None:
        """Baseline: the same ucc invocation against the HP2 root stays clean."""
        binary = resolve_ucc_binary()
        if binary is None:
            self.skipTest("hp2_ucc binary not provided or not built")
        hp2_root = REPOSITORY_ROOT / "HarryPotter2" / "Unreal"
        if not (hp2_root / "System" / "Default.ini").is_file():
            self.skipTest("HP2 data root not present")

        with tempfile.TemporaryDirectory(prefix="hp2-baseline.") as tmp:
            result = subprocess.run(
                [str(binary), "-datadir=" + str(hp2_root), "help"],
                capture_output=True, text=True, timeout=120,
                env={**os.environ,
                     "HOME": tmp, "TMPDIR": tmp, "LC_ALL": "C"})

        combined = (result.stdout + result.stderr).lower()
        self.assertEqual(result.returncode, 0, "hp2 baseline help regressed")
        for marker in ('commands for "ucc":', "ucc help <command>"):
            self.assertIn(marker, combined, f"missing baseline marker {marker!r}")
        for marker in ("critical", "assert", "apperror", "access violation",
                       "pure virtual", "segmentation"):
            self.assertNotIn(marker, combined,
                             f"failure marker {marker!r} in baseline output")
        emit_report("hp1-class-bind-hp2-baseline", {
            "schema": "report-schema-v1",
            "name": "hp1-class-bind-hp2-baseline",
            "status": "passed",
            "invariant": "hp2_ucc help against the HP2 data root stays exit-0 clean",
            "reason_code": "",
            "data": {"profile": "data-prototype"},
            "artifacts": [],
            "command": [],
            "exit_reason": "",
        })


if __name__ == "__main__":
    unittest.main(module="__main__", argv=[sys.argv[0], "-v"], exit=False)
