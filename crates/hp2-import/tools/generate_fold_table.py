#!/usr/bin/env python3
"""Turns dump_utf8proc_casefold's output into crates/hp2-import/src/
fold_table.rs: a sparse (codepoint, folded string) Rust table listing
only the codepoints whose full case fold differs from the codepoint
itself, sorted by codepoint for lower()'s binary search.

Usage: python3 generate_fold_table.py cpp_table.txt > ../src/fold_table.rs

See dump_utf8proc_casefold.c's header for how to produce cpp_table.txt
against the pinned utf8proc version.
"""

import sys

HEADER = """\
//! Generated from the pinned `utf8proc` 2.12.0 (Unicode 18.0) via
//! `utf8proc_map(..., UTF8PROC_CASEFOLD)` run over every Unicode scalar
//! value (0..=0x10FFFF, excluding surrogates). Lists only codepoints whose
//! full case fold differs from the codepoint itself -- 1,606 of 1,112,064,
//! the same sparse-table shape as the Unicode `CaseFolding.txt` C+F set
//! this was generated from, but read directly from utf8proc's own output
//! rather than a separately fetched copy of the file, so there is no
//! version-matching step that could drift from the actual pinned library.
//!
//! Regeneration: `tools/dump_utf8proc_casefold.c` against the utf8proc
//! commit pinned in Build/CMake/HP2Dependencies.cmake's hp2_utf8proc
//! FetchContent declaration (79cdd5ab40c79afa559f689ffc13b76812dee1ac at
//! the time this was generated), piped through
//! `tools/generate_fold_table.py`. Only touch this file if that pinned
//! commit changes.
//!
//! Sorted by codepoint; `lower()` binary-searches it.
pub static FOLD_TABLE: &[(u32, &str)] = &[
"""

FOOTER = "];\n"


def rust_escape(s: str) -> str:
    return s.replace("\\", "\\\\").replace('"', '\\"')


def main() -> None:
    if len(sys.argv) != 2:
        print(f"usage: {sys.argv[0]} cpp_table.txt", file=sys.stderr)
        sys.exit(2)

    entries: list[tuple[int, str]] = []
    with open(sys.argv[1]) as f:
        for line in f:
            parts = line.split()
            cp = int(parts[0], 16)
            folded_hex = parts[1] if len(parts) > 1 else ""
            if folded_hex == "ERR":
                continue
            folded_str = bytes.fromhex(folded_hex).decode("utf-8")
            if folded_str != chr(cp):
                entries.append((cp, folded_str))
    entries.sort(key=lambda e: e[0])

    sys.stdout.write(HEADER)
    for cp, folded in entries:
        sys.stdout.write(f'    (0x{cp:06x}, "{rust_escape(folded)}"),\n')
    sys.stdout.write(FOOTER)
    print(f"{len(entries)} entries", file=sys.stderr)


if __name__ == "__main__":
    main()
