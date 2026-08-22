#!/usr/bin/env python3
"""Cutscene localization contract tests for the prototype data tree.

Verifies that every cutscene localization file under
``<prototype-root>/System/CUTSCENES/*.int`` parses the way the engine's INI
reader parses it and satisfies the naming convention its consumers rely on.

Engine ground truth (read-only references):
- ``Core/Src/UnMisc.cpp`` ``appLoadFileToString``: UTF-16LE (FF FE BOM),
  UTF-16BE (FE FF BOM), or ANSI bytes zero-extended per byte via ``FromAnsi``
  (Latin-1 semantics on the Apple/UNICODE TCHAR build).
- ``Core/Inc/FConfigCacheIni.h`` ``FConfigFile::Read``: lines split on CR/LF;
  a line is a section header when it starts with ``[`` and ends with ``]``;
  otherwise the first ``=`` splits key/value (verbatim, no whitespace trim);
  a value is unquoted only when fully quoted. Comments are NOT skipped.
  Section and key lookups are case-insensitive (``FString::operator==`` uses
  ``appStricmp`` and ``GetTypeHash(FString)`` uses ``appStrihash``); duplicate
  keys within a section are appended to the TMultiMap but only the first is
  ever returned, so later duplicates are unreachable data.
- ``HGame/Classes/CutScene.uc``: threads are probed as ``Localize("thread_"$t,
  "line_0", "Cutscenes\\<FileName>")`` for t in [0, MAX_THREADS=20); a missing
  lookup yields the ``<?...?>`` marker, which the caller rejects.
- ``HGame/Classes/CutScriptDisk.uc``: lines load as ``line_0``, ``line_1``, ...
  until a lookup is empty or contains ``<?``; any gap or ``<?`` inside a value
  makes every later line unreachable. ``lineArray[4096]`` bounds lines.

Inventory constants are pinned to the prototype (data-prototype profile):
185 cutscene localization files. The 214 figure quoted by the audit is the
retail-only cutscene payload contract enforced by ``Build/prepare_retail_data.py``
(full/retail-only profiles); the prototype tree ships 185, matching the
safe-profile cutscene contract in the same script.

Pure stdlib. Deterministic: sorted iteration everywhere, no network, no
wall-clock dependence. When ``HP2_ARTIFACT_DIR`` is set, a report-schema-v1
JSON document is emitted there; a missing prototype tree yields status
"blocked" with reason_code "data.prototype_missing".
"""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import re
import sys
import tempfile
import unittest

REPOSITORY_ROOT = Path(__file__).resolve().parent.parent
DEFAULT_DATA_ROOT = REPOSITORY_ROOT / "HarryPotter2" / "Unreal"

# Prototype cutscene localization inventory (System/CUTSCENES/*.int).
EXPECTED_PROTOTYPE_CUTSCENE_COUNT = 185
# Retail-only cutscene payload count (Build/prepare_retail_data.py full and
# retail-only profiles). Documented here so the audit figure has a home; the
# prototype tree intentionally ships fewer files.
RETAIL_ONLY_CUTSCENE_COUNT = 214

# HGame/Classes/CutScene.uc and CutScriptDisk.uc bounds.
MAX_THREADS = 20
MAX_DISK_LINES = 4096

THREAD_RE = re.compile(r"thread_(\d+)\Z", re.IGNORECASE)
LINE_RE = re.compile(r"line_(\d+)\Z", re.IGNORECASE)

REPORT_SCHEMA = 1
REPORT_INVARIANT = (
    "prototype cutscene localization parses under the engine INI reader "
    "and matches the pinned prototype inventory"
)


def resolve_data_root() -> Path:
    """Data root: $HP2_UNREAL_ROOT when set, else the repository tree."""
    env = os.environ.get("HP2_UNREAL_ROOT")
    if env:
        return Path(env)
    return DEFAULT_DATA_ROOT


def enumerate_cutscene_files(data_root: Path) -> list[Path]:
    """Sorted *.int files under System/CUTSCENES (deterministic order)."""
    directory = data_root / "System" / "CUTSCENES"
    if not directory.is_dir():
        return []
    return sorted(directory.glob("*.int"), key=lambda path: path.name)


def enumerate_reference_stems(data_root: Path) -> list[str]:
    """Sorted <name>.int names implied by CutScenes/*.txt script references."""
    directory = data_root / "CutScenes"
    if not directory.is_dir():
        return []
    return sorted(
        (path.stem + ".int") for path in directory.glob("*.txt")
    )


def decode_ini(data: bytes) -> tuple[str, str, list[str]]:
    """Decode bytes exactly like appLoadFileToString (Core/Src/UnMisc.cpp).

    Returns (encoding_name, text, issues). ANSI bytes are decoded as Latin-1,
    matching FromAnsi's unsigned zero-extend on the Apple UNICODE build.
    """
    if data[:2] == b"\xff\xfe":
        body = data[2:]
        if len(body) % 2:
            return "utf-16le", "", ["encoding.utf16_odd_byte_length"]
        return "utf-16le", body.decode("utf-16-le"), []
    if data[:2] == b"\xfe\xff":
        body = data[2:]
        if len(body) % 2:
            return "utf-16be", "", ["encoding.utf16_odd_byte_length"]
        return "utf-16be", body.decode("utf-16-be"), []
    return "ansi", data.decode("latin-1"), []


def split_engine_lines(text: str) -> list[str]:
    """FConfigFile::Read treats every CR or LF as a line terminator."""
    return re.split(r"\r\n|\r|\n", text)


def classify_line_endings(text: str) -> tuple[str, list[str]]:
    """Per-file EOL class: lf, crlf, or mixed (with issue codes)."""
    crlf = text.count("\r\n")
    lone_lf = text.count("\n") - crlf
    lone_cr = text.count("\r") - crlf
    if lone_cr:
        return "mixed", ["line_endings.bare_cr"]
    if crlf and lone_lf:
        return "mixed", ["line_endings.mixed_crlf_lf"]
    if crlf:
        return "crlf", []
    return "lf", []


def parse_ini(text: str) -> list[tuple[str, list[tuple[str, str]]]]:
    """Parse exactly like FConfigFile::Read (Core/Inc/FConfigCacheIni.h).

    Returns an ordered list of (section_name, [(key, value), ...]). Keys are
    verbatim (no trim); values are unquoted only when fully quoted; comment
    lines are not special-cased, matching the engine.
    """
    sections: list[tuple[str, list[tuple[str, str]]]] = []
    current: list[tuple[str, str]] | None = None
    for line in split_engine_lines(text):
        if line.startswith("[") and line.endswith("]"):
            current = []
            sections.append((line[1:-1], current))
            continue
        if current is None or not line or "=" not in line:
            continue
        key, _, value = line.partition("=")
        if len(value) >= 1 and value.startswith('"') and value.endswith('"'):
            value = value[1:-1]
        current.append((key, value))
    return sections


def audit_cutscene_file(path: Path) -> dict[str, object]:
    """Run every contract check on one cutscene localization file.

    Issue codes are dotted lowercase; each is prefixed with the file name in
    aggregated diagnostics so failures point at exact offenders.
    """
    issues: list[str] = []
    data = path.read_bytes()
    if b"\x00" in data:
        issues.append("encoding.embedded_nul")
    encoding, text, decode_issues = decode_ini(data)
    issues.extend(decode_issues)
    if "\x00" in text:
        issues.append("encoding.nul_after_decode")

    eol, eol_issues = classify_line_endings(text)
    issues.extend(eol_issues)

    sections = parse_ini(text)
    if not sections:
        issues.append("sections.none")
    thread_lines: dict[int, dict[int, str]] = {}
    for name, pairs in sections:
        seen: set[str] = set()
        for key, value in pairs:
            folded = key.casefold()
            if folded in seen:
                issues.append(f"keys.duplicate[{name}.{key}]")
            seen.add(folded)
            if "<?" in value:
                issues.append(f"values.localization_marker[{name}.{key}]")
        thread_match = THREAD_RE.fullmatch(name)
        if thread_match is None:
            issues.append(f"sections.unexpected[{name}]")
            continue
        index = int(thread_match.group(1))
        if index >= MAX_THREADS:
            issues.append(f"threads.unreachable_index[{name}]")
        line_numbers: dict[int, str] = {}
        for key, value in pairs:
            line_match = LINE_RE.fullmatch(key)
            if line_match is None:
                issues.append(f"keys.unexpected[{name}.{key}]")
                continue
            line_numbers[int(line_match.group(1))] = value
        if not line_numbers:
            # A Thread section without line_N keys is a placeholder: the
            # engine's line_0 probe misses and the thread never spawns.
            continue
        if max(line_numbers) >= MAX_DISK_LINES:
            issues.append(f"keys.unreachable_line_index[{name}]")
        expected = range(max(line_numbers) + 1)
        missing = [n for n in expected if n not in line_numbers]
        if missing:
            issues.append(f"keys.line_gap[{name}:{missing[:8]}]")
        thread_lines[index] = line_numbers

    # Note: absent or empty threads are deliberate here. CutScene.uc probes
    # every thread index independently, so a missing Thread_N never breaks
    # sibling threads; only unreachable keys within a present thread matter.

    return {
        "file": path.name,
        "bytes": len(data),
        "encoding": encoding,
        "line_endings": eol,
        "sections": len(sections),
        "keys": sum(len(pairs) for _, pairs in sections),
        "issues": issues,
    }

def audit_inventory(data_root: Path) -> dict[str, object]:
    """Audit the whole cutscene localization inventory under data_root."""
    files = enumerate_cutscene_files(data_root)
    results = [audit_cutscene_file(path) for path in files]

    inventory_issues: list[str] = []
    if len(files) != EXPECTED_PROTOTYPE_CUTSCENE_COUNT:
        inventory_issues.append(
            f"inventory.count: expected {EXPECTED_PROTOTYPE_CUTSCENE_COUNT} "
            f"cutscene localization files, found {len(files)}"
        )
    references = enumerate_reference_stems(data_root)
    runtime_names = [path.name for path in files]
    if references != runtime_names:
        only_references = sorted(set(references) - set(runtime_names))
        only_runtime = sorted(set(runtime_names) - set(references))
        if only_references:
            inventory_issues.append(
                f"inventory.references_without_runtime: {only_references}"
            )
        if only_runtime:
            inventory_issues.append(
                f"inventory.runtime_without_references: {only_runtime}"
            )

    return {
        "data_root": str(data_root),
        "files": results,
        "count": len(files),
        "inventory_issues": inventory_issues,
    }


def report_status(audit: dict[str, object] | None, data_root: Path) -> tuple[str, str]:
    """(status, reason_code) per report schema v1."""
    if audit is None:
        return "blocked", "data.prototype_missing"
    violations = len(audit["inventory_issues"]) + sum(
        len(result["issues"]) for result in audit["files"]
    )
    if violations:
        return "fail", "localization.contract_violation"
    return "pass", ""


def build_report(
    audit: dict[str, object] | None,
    data_root: Path,
    command: list[str],
    artifact_path: Path | None,
) -> dict[str, object]:
    """Report schema v1 document for this runner."""
    status, reason_code = report_status(audit, data_root)
    report: dict[str, object] = {
        "schema": REPORT_SCHEMA,
        "name": "localization_contract",
        "status": status,
        "invariant": REPORT_INVARIANT,
        "data": {"profile": "prototype"},
        "artifacts": [],
        "command": " ".join(command),
        "exit_reason": "",
    }
    if reason_code:
        report["reason_code"] = reason_code
    if audit is None:
        report["exit_reason"] = f"prototype data root absent: {data_root}"
    else:
        violations = len(audit["inventory_issues"]) + sum(
            len(result["issues"]) for result in audit["files"]
        )
        report["exit_reason"] = (
            f"{violations} contract violation(s) across {audit['count']} file(s)"
            if violations
            else f"all {audit['count']} cutscene localization files satisfy the contract"
        )
        report["details"] = {
            "expected_count": EXPECTED_PROTOTYPE_CUTSCENE_COUNT,
            "retail_only_count": RETAIL_ONLY_CUTSCENE_COUNT,
            "count": audit["count"],
            "inventory_issues": audit["inventory_issues"],
            "files": audit["files"],
        }
    if artifact_path is not None:
        report["artifacts"].append({"kind": "report", "path": str(artifact_path)})
    return report


def artifact_report_path() -> Path | None:
    """Deterministic report destination inside HP2_ARTIFACT_DIR when set."""
    artifact_dir = os.environ.get("HP2_ARTIFACT_DIR")
    if not artifact_dir:
        return None
    directory = Path(artifact_dir)
    directory.mkdir(parents=True, exist_ok=True)
    test_name = os.environ.get("HP2_TEST_NAME", "localization_contract")
    return directory / f"{test_name}-report.json"


def write_report(report: dict[str, object], path: Path) -> None:
    """Serialize a report-schema-v1 document deterministically."""
    path.write_text(
        json.dumps(report, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def run_audit(data_root: Path) -> dict[str, object] | None:
    """Full audit, or None when the prototype tree is absent."""
    if not (data_root / "System" / "CUTSCENES").is_dir():
        return None
    return audit_inventory(data_root)


def format_diagnostics(audit: dict[str, object]) -> str:
    """One line per offending file, deterministic order."""
    lines = list(audit["inventory_issues"])
    for result in audit["files"]:
        for issue in result["issues"]:
            lines.append(f"{result['file']}: {issue}")
    return "\n".join(sorted(lines))


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument(
        "--data-root",
        help="prototype data root (default: $HP2_UNREAL_ROOT or repo tree)",
    )
    args, _ = parser.parse_known_args(argv)
    data_root = Path(args.data_root) if args.data_root else resolve_data_root()

    audit = run_audit(data_root)
    status, _ = report_status(audit, data_root)
    artifact_path = artifact_report_path()
    report = build_report(
        audit, data_root, [sys.executable, str(Path(__file__))], artifact_path
    )
    if artifact_path is not None:
        write_report(report, artifact_path)

    if audit is None:
        print(f"BLOCKED data.prototype_missing: {data_root}")
        return 2
    diagnostics = format_diagnostics(audit)
    if diagnostics:
        print(f"FAIL {status}: cutscene localization contract violations\n{diagnostics}")
        return 1
    print(f"PASS: {audit['count']} cutscene localization files satisfy the contract")
    return 0


class IniDecoderContracts(unittest.TestCase):
    """Engine-faithful decoding and parsing behavior on synthetic bytes."""

    def test_ansi_bytes_decode_as_latin1_without_bom(self) -> None:
        encoding, text, issues = decode_ini(b"[Thread_0]\nline_0=caf\xe9\n")
        self.assertEqual((encoding, text, issues), ("ansi", "[Thread_0]\nline_0=caf\xe9\n", []))

    def test_utf16le_and_be_boms_are_honored(self) -> None:
        for bom, codec, name in ((b"\xff\xfe", "utf-16-le", "utf-16le"), (b"\xfe\xff", "utf-16-be", "utf-16be")):
            encoding, text, issues = decode_ini(bom + "[Thread_0]".encode(codec))
            self.assertEqual((encoding, text, issues), (name, "[Thread_0]", []))

    def test_odd_utf16_body_is_rejected(self) -> None:
        _, text, issues = decode_ini(b"\xff\xfe[\x00a")
        self.assertEqual(text, "")
        self.assertEqual(issues, ["encoding.utf16_odd_byte_length"])

    def test_embedded_nul_is_flagged(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "Nul.int"
            path.write_bytes(b"[Thread_0]\nline_0=a\x00b\n")
            result = audit_cutscene_file(path)
        self.assertIn("encoding.embedded_nul", result["issues"])
        self.assertIn("encoding.nul_after_decode", result["issues"])

    def test_mixed_line_endings_are_flagged(self) -> None:
        eol, issues = classify_line_endings("a\nb\r\n")
        self.assertEqual((eol, issues), ("mixed", ["line_endings.mixed_crlf_lf"]))
        eol, issues = classify_line_endings("a\rb")
        self.assertEqual((eol, issues), ("mixed", ["line_endings.bare_cr"]))

    def test_duplicate_keys_within_section_are_flagged(self) -> None:
        sections = parse_ini("[Thread_0]\nline_0=a\nLINE_0=b\n")
        self.assertEqual(sections, [("Thread_0", [("line_0", "a"), ("LINE_0", "b")])])
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "Dupe.int"
            path.write_bytes(b"[Thread_0]\nline_0=a\nLINE_0=b\n")
            result = audit_cutscene_file(path)
        self.assertIn("keys.duplicate[Thread_0.LINE_0]", result["issues"])

    def test_line_gap_makes_later_lines_unreachable(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "Gap.int"
            path.write_bytes(b"[Thread_0]\nline_0=a\nline_2=c\n")
            result = audit_cutscene_file(path)
        self.assertIn("keys.line_gap[Thread_0:[1]]", result["issues"])

    def test_missing_line_0_is_flagged(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "NoStart.int"
            path.write_bytes(b"[Thread_0]\nline_1=a\n")
            result = audit_cutscene_file(path)
        self.assertIn("keys.line_gap[Thread_0:[0]]", result["issues"])

    def test_localization_marker_in_value_is_flagged(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "Marker.int"
            path.write_bytes(b"[Thread_0]\nline_0=hello <?world?>\n")
            result = audit_cutscene_file(path)
        self.assertIn("values.localization_marker[Thread_0.line_0]", result["issues"])

    def test_thread_and_line_bounds_are_enforced(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "Bounds.int"
            content = b"[Thread_20]\nline_0=a\n[Thread_0]\nline_4096=a\n"
            path.write_bytes(content)
            result = audit_cutscene_file(path)
        self.assertIn("threads.unreachable_index[Thread_20]", result["issues"])
        self.assertIn("keys.unreachable_line_index[Thread_0]", result["issues"])

    def test_non_thread_section_and_key_are_flagged(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "Odd.int"
            path.write_bytes(b"[Strings]\nline_0=a\n[Thread_0]\nkey=b\n")
            result = audit_cutscene_file(path)
        self.assertIn("sections.unexpected[Strings]", result["issues"])
        self.assertIn("keys.unexpected[Thread_0.key]", result["issues"])

    def test_empty_thread_section_is_benign(self) -> None:
        # CutScene.uc probes each thread index independently, so an empty
        # Thread section (or a missing one) simply means "thread absent".
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "Empty.int"
            path.write_bytes(b"[Thread_0]\nline_0=a\n[Thread_1]\n[Thread_2]\nline_0=b\n")
            result = audit_cutscene_file(path)
        self.assertEqual(result["issues"], [])

    def test_quoted_values_unquote_like_the_engine(self) -> None:
        sections = parse_ini('[Thread_0]\nline_0="quoted"\nline_1=par"\n')
        self.assertEqual(
            sections,
            [("Thread_0", [("line_0", "quoted"), ("line_1", 'par"')])],
        )

    def test_comment_lines_parse_like_the_engine(self) -> None:
        # FConfigFile::Read has no comment syntax: "; x=1" is key "; x".
        sections = parse_ini("[Thread_0]\n; note=1\nline_0=a\n")
        self.assertEqual(
            sections,
            [("Thread_0", [("; note", "1"), ("line_0", "a")])],
        )


class PrototypeCutsceneInventory(unittest.TestCase):
    """Contracts over the real prototype cutscene localization inventory."""

    def setUp(self) -> None:
        self.data_root = resolve_data_root()
        if not (self.data_root / "System" / "CUTSCENES").is_dir():
            self.skipTest(f"data.prototype_missing: {self.data_root}")

    def test_inventory_count_matches_pinned_constant(self) -> None:
        files = enumerate_cutscene_files(self.data_root)
        self.assertEqual(
            len(files),
            EXPECTED_PROTOTYPE_CUTSCENE_COUNT,
            f"expected {EXPECTED_PROTOTYPE_CUTSCENE_COUNT} files in "
            f"{self.data_root / 'System' / 'CUTSCENES'}, found {len(files)}: "
            f"{[path.name for path in files][:10]}...",
        )

    def test_runtime_files_match_cutscene_script_references(self) -> None:
        runtime = [path.name for path in enumerate_cutscene_files(self.data_root)]
        references = enumerate_reference_stems(self.data_root)
        self.assertEqual(
            references,
            runtime,
            "CutScenes/*.txt script references and System/CUTSCENES/*.int "
            "runtime files must correspond one-to-one",
        )

    def test_every_file_satisfies_engine_contracts(self) -> None:
        audit = audit_inventory(self.data_root)
        diagnostics = format_diagnostics(audit)
        self.assertEqual(
            diagnostics,
            "",
            "cutscene localization contract violations:\n" + diagnostics,
        )
        encodings = {result["encoding"] for result in audit["files"]}
        eols = {result["line_endings"] for result in audit["files"]}
        self.assertEqual(encodings, {"ansi"}, f"unexpected encodings: {encodings}")
        self.assertEqual(eols, {"lf"}, f"inconsistent line endings: {eols}")

    def test_report_schema_v1_is_emitted(self) -> None:
        audit = audit_inventory(self.data_root)
        report = build_report(
            audit, self.data_root, [sys.executable, str(Path(__file__))], None
        )
        self.assertEqual(report["schema"], REPORT_SCHEMA)
        self.assertEqual(report["name"], "localization_contract")
        self.assertEqual(report["data"], {"profile": "prototype"})
        self.assertIn(report["status"], {"pass", "fail"})
        self.assertTrue(report["invariant"])
        self.assertTrue(report["command"])
        self.assertTrue(report["exit_reason"])


class BlockedInventoryContracts(unittest.TestCase):
    """Blocked-path behavior when the prototype tree is absent."""

    def test_missing_tree_reports_blocked_with_reason(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            empty = Path(tmp)
            self.assertIsNone(run_audit(empty))
            report = build_report(None, empty, ["cmd"], None)
        self.assertEqual(report["status"], "blocked")
        self.assertEqual(report["reason_code"], "data.prototype_missing")
        self.assertEqual(report["data"], {"profile": "prototype"})


if __name__ == "__main__":
    raise SystemExit(main())
