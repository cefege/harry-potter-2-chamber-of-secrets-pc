#!/usr/bin/env python3
"""Stamp a release version and checksum into the Homebrew cask.

The cask is the only install path the README advertises, and Homebrew refuses
an install whose ``sha256`` does not match the download. Keeping the checksum
by hand invites exactly the drift that makes `brew install` fail for users, so
the release workflow computes it and rewrites the two generated lines here.

Only ``version`` and the ``sha256`` literal are rewritten. The download URL is
built from ``version`` by the cask itself, so it never needs stamping.

Exit codes: 0 stamped (or already correct), 1 nothing matched or bad input.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

VERSION_PATTERN = re.compile(r'^(\s*version\s+")([^"]*)("\s*)$')
SHA256_PATTERN = re.compile(r'^(\s*sha256\s+)(:[a-z_]+|"[0-9a-fA-F]{64}")(\s*)$')

EXIT_PASS = 0
EXIT_FAIL = 1


def stamp(cask: Path, version: str, sha256: str) -> bool:
    """Rewrite version and sha256 in place. Returns True if the file changed."""
    original = cask.read_text()
    lines = original.splitlines(keepends=True)

    stamped_version = False
    stamped_sha = False
    for index, line in enumerate(lines):
        match = VERSION_PATTERN.match(line.rstrip("\n"))
        if match and not stamped_version:
            lines[index] = f"{match.group(1)}{version}{match.group(3)}\n"
            stamped_version = True
            continue
        match = SHA256_PATTERN.match(line.rstrip("\n"))
        if match and not stamped_sha:
            lines[index] = f'{match.group(1)}"{sha256}"{match.group(3)}\n'
            stamped_sha = True

    if not stamped_version:
        raise SystemExit(f"{cask}: no `version` line to stamp")
    if not stamped_sha:
        raise SystemExit(f"{cask}: no `sha256` line to stamp")

    updated = "".join(lines)
    if updated == original:
        return False
    cask.write_text(updated)
    return True


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--cask", required=True, type=Path)
    parser.add_argument("--version", required=True)
    parser.add_argument("--sha256", required=True)
    arguments = parser.parse_args(argv)

    if not re.fullmatch(r"[0-9]+(\.[0-9]+)*", arguments.version):
        print(f"error: --version {arguments.version!r} is not numeric dotted", file=sys.stderr)
        return EXIT_FAIL
    if not re.fullmatch(r"[0-9a-fA-F]{64}", arguments.sha256):
        print("error: --sha256 must be 64 hex characters", file=sys.stderr)
        return EXIT_FAIL
    if not arguments.cask.is_file():
        print(f"error: {arguments.cask} does not exist", file=sys.stderr)
        return EXIT_FAIL

    changed = stamp(arguments.cask, arguments.version, arguments.sha256)
    print(f"{arguments.cask}: {'stamped' if changed else 'already current'}")
    return EXIT_PASS


if __name__ == "__main__":
    raise SystemExit(main())
