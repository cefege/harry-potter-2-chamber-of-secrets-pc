#!/usr/bin/env python3
"""Reject packaging paths that could overwrite a configured reference bundle.

This is not an oracle identity or provenance check. Paths can change after this
check; callers must also protect the filesystem from concurrent replacement.
"""
import argparse
import errno
import os
from pathlib import Path
import sys


def inode(path):
    try:
        stat = path.stat()
    except OSError as exc:
        if exc.errno in (errno.ENOENT, errno.ENOTDIR):
            return None
        raise
    return stat.st_dev, stat.st_ino


def overlaps(left, right):
    # Case-folding is conservative on case-sensitive volumes and protects
    # missing path components on the usual case-insensitive macOS volumes.
    a = tuple(part.casefold() for part in left.parts)
    b = tuple(part.casefold() for part in right.parts)
    if a[:len(b)] == b or b[:len(a)] == a:
        return True
    # Detect existing directory aliases even where spelling differs.
    for path, other in ((left, right), (right, left)):
        identity = inode(path)
        if identity is not None and any(inode(p) == identity for p in other.parents):
            return True
    return inode(left) is not None and inode(left) == inode(right)


def check(oracle, destination):
    if not destination or not Path(destination).is_absolute():
        raise ValueError("destination must be a nonempty absolute path")
    dest = Path(destination).resolve(strict=False)
    if not oracle:
        return
    if not Path(oracle).is_absolute():
        raise ValueError("configured oracle must be an absolute path")
    ref = Path(oracle).resolve(strict=False)
    if overlaps(ref, dest):
        raise ValueError("configured oracle overlaps the packaging destination")
    # A bundle executable can be hard-linked into the bundle to be removed.
    # Also check a directly supplied executable path conservatively.
    executable = Path("Contents/MacOS/HarryPotter2")
    dest_exe = inode(dest / executable)
    if dest_exe is not None and any(
        inode(candidate) == dest_exe for candidate in (ref, ref / executable)
    ):
        raise ValueError("configured oracle executable is hard-linked to the packaging executable")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--oracle", default="")
    parser.add_argument("--destination", required=True)
    args = parser.parse_args()
    try:
        check(args.oracle, args.destination)
    except (OSError, RuntimeError, ValueError) as exc:
        print(f"Oracle packaging safety check failed: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
