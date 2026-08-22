#!/usr/bin/env python3
"""Tie the HP2 prototype dev-data identity together.

Cross-checks ``prototype-data.7z`` (a BagIt bag stored as a 7z archive)
against the unpacked ``bag/`` checkout and its manifests:

* streams the archive member list with /usr/bin/bsdtar -tf (never extracts),
* parses bag manifest-*.txt / tagmanifest-*.txt files (BagIt 1.0 layout),
* verifies the local payload against the recorded hashes and Payload-Oxum,
* reports every archive member that no recorded hash can vouch for instead
  of failing blindly.

The output is report-schema-v1 JSON with deterministic ordering and no
timestamps. Exit code: 0 for pass/blocked, 1 for fail.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import subprocess
import sys
from pathlib import Path
from typing import Iterator

SCHEMA_VERSION = 1
REPORT_NAME = "prototype_archive"
BSDTAR = "/usr/bin/bsdtar"

BAGIT_ALGORITHMS = {
    "md5": 32,
    "sha1": 40,
    "sha224": 56,
    "sha256": 64,
    "sha384": 96,
    "sha512": 128,
}

# The bag in this repository ships "manifest-sha3512.txt", which is not a
# BagIt algorithm name; unknown names are parsed, recorded, and reported as
# unverifiable rather than crashing or being silently ignored.
MANIFEST_FILE_RE = re.compile(r"^manifest-([A-Za-z0-9_-]+)\.txt$")
TAGMANIFEST_FILE_RE = re.compile(r"^tagmanifest-([A-Za-z0-9_-]+)\.txt$")

INVARIANT = (
    "prototype-data.7z member set matches the unpacked bag/ checkout, the "
    "local payload hashes to its manifest values and Payload-Oxum, and every "
    "archive member is covered by a recorded hash"
)


class VerificationError(Exception):
    """Internal fatal error; converted into a fail report."""


def repo_root() -> Path:
    return Path(__file__).resolve(strict=True).parent.parent


def artifact_dir() -> Path | None:
    value = os.environ.get("HP2_ARTIFACT_DIR")
    return Path(value) if value else None


# ---------------------------------------------------------------------------
# Bag parsing


def parse_manifest_file(path: Path) -> tuple[dict[str, str], list[str]]:
    """Return {relative path: checksum} plus a list of malformed-line notes."""
    entries: dict[str, str] = {}
    problems: list[str] = []
    text = path.read_text(encoding="utf-8", errors="replace")
    for number, raw in enumerate(text.splitlines(), start=1):
        line = raw.strip()
        if not line:
            continue
        parts = line.split(None, 1)
        if len(parts) != 2:
            problems.append(f"{path.name}:{number}: not '<checksum> <path>'")
            continue
        checksum, member = parts[0].lower(), parts[1].strip()
        entries[member] = checksum
    return entries, problems


class Bag:
    """Parsed view of one BagIt bag directory."""

    def __init__(self, root: Path) -> None:
        self.root = root
        self.manifests: dict[str, dict[str, str]] = {}  # algorithm -> entries
        self.tagmanifests: dict[str, dict[str, str]] = {}
        self.unsupported_algorithms: list[str] = []
        self.problems: list[str] = []
        self.payload_oxum_octets: int | None = None
        self.payload_oxum_count: int | None = None

    def load(self) -> None:
        if not (self.root / "bagit.txt").is_file():
            raise VerificationError(f"{self.root}: missing bagit.txt")
        info = self.root / "bag-info.txt"
        if info.is_file():
            for line in info.read_text(encoding="utf-8", errors="replace").splitlines():
                name, _, value = line.partition(":")
                if name.strip() == "Payload-Oxum":
                    octets, _, count = value.strip().partition(".")
                    try:
                        self.payload_oxum_octets = int(octets)
                        self.payload_oxum_count = int(count)
                    except ValueError:
                        self.problems.append("bag-info.txt: unparsable Payload-Oxum")
        else:
            self.problems.append("missing bag-info.txt")

        found_manifest = False
        for entry in sorted(self.root.iterdir(), key=lambda item: item.name):
            match = MANIFEST_FILE_RE.match(entry.name)
            tag_match = TAGMANIFEST_FILE_RE.match(entry.name)
            if match:
                found_manifest = True
                algorithm = match.group(1).lower()
                if algorithm not in BAGIT_ALGORITHMS:
                    self.unsupported_algorithms.append(algorithm)
                    continue
                entries, problems = parse_manifest_file(entry)
                self.problems.extend(problems)
                self._check_lengths(algorithm, entry.name, entries)
                self.manifests[algorithm] = entries
            elif tag_match:
                algorithm = tag_match.group(1).lower()
                if algorithm not in BAGIT_ALGORITHMS:
                    self.unsupported_algorithms.append(algorithm)
                    continue
                entries, problems = parse_manifest_file(entry)
                self.problems.extend(problems)
                self.tagmanifests[algorithm] = entries
        if not found_manifest:
            raise VerificationError(f"{self.root}: no manifest-*.txt present")

    def _check_lengths(self, algorithm: str, filename: str, entries: dict[str, str]) -> None:
        expected = BAGIT_ALGORITHMS[algorithm]
        for member, checksum in entries.items():
            if len(checksum) != expected or not re.fullmatch(r"[0-9a-f]+", checksum):
                self.problems.append(
                    f"{filename}: checksum for {member!r} is not {expected} hex digits"
                )

    def payload_entries(self) -> set[str]:
        covered: set[str] = set()
        for entries in self.manifests.values():
            covered.update(entries)
        return covered


def file_hash(path: Path, algorithm: str) -> str:
    digest = hashlib.new(algorithm)
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def verify_local_payload(bag: Bag, problems: list[str]) -> dict[str, int]:
    """Hash every local payload file and compare against all manifests."""
    counts = {"verified": 0, "mismatched": 0, "missing": 0}
    payload_files = {
        str(path.relative_to(bag.root))
        for path in (bag.root / "data").rglob("*")
        if path.is_file()
    } if (bag.root / "data").is_dir() else set()

    for member in sorted(bag.payload_entries()):
        path = bag.root / member
        if not path.is_file():
            problems.append(f"manifest references missing payload file {member}")
            counts["missing"] += 1
            continue
        matched = False
        mismatched = False
        for algorithm, entries in sorted(bag.manifests.items()):
            if member not in entries:
                continue
            if file_hash(path, algorithm) == entries[member]:
                matched = True
            else:
                mismatched = True
                problems.append(f"{member}: {algorithm} checksum mismatch")
        if mismatched:
            counts["mismatched"] += 1
        elif matched:
            counts["verified"] += 1
        else:
            # Present locally but only named by unsupported-algorithm manifests.
            problems.append(f"{member}: no supported-algorithm checksum recorded")
            counts["mismatched"] += 1
    return counts


def check_payload_oxum(bag: Bag, problems: list[str]) -> bool:
    data_dir = bag.root / "data"
    if not data_dir.is_dir():
        problems.append("bag has no data/ directory")
        return False
    files = [path for path in data_dir.rglob("*") if path.is_file()]
    octets = sum(path.stat().st_size for path in files)
    if bag.payload_oxum_octets is None:
        problems.append("bag-info.txt records no Payload-Oxum")
        return False
    if octets != bag.payload_oxum_octets or len(files) != bag.payload_oxum_count:
        problems.append(
            f"Payload-Oxum {bag.payload_oxum_octets}.{bag.payload_oxum_count} does not "
            f"match observed {octets}.{len(files)}"
        )
        return False
    return True


# ---------------------------------------------------------------------------
# Archive side


def list_archive_members(archive: Path) -> list[str]:
    """Stream `bsdtar -tf` without buffering the archive itself."""
    completed = subprocess.run(
        [BSDTAR, "-tf", str(archive)],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if completed.returncode != 0:
        detail = completed.stderr.decode("utf-8", "replace").strip().splitlines()
        message = detail[-1] if detail else f"{BSDTAR} exited {completed.returncode}"
        raise VerificationError(f"archive listing failed: {message}")
    archived_files: list[str] = []
    for line in completed.stdout.decode("utf-8", "replace").splitlines():
        if not line:
            continue
        # The archive stores the whole bag under a top-level bag/ directory;
        # normalize to bag-relative paths so bag/ and the archive are comparable.
        name = line.rstrip("/")
        if name.startswith("bag/"):
            name = name[len("bag/"):]
        elif name == "bag":
            continue  # the container directory itself
        if not name or line.endswith("/"):
            continue  # directory entries carry no bytes of their own
        archived_files.append(name)
    return sorted(archived_files)


def classify_members(
    members: list[str], bag: Bag
) -> tuple[list[dict[str, object]], dict[str, int]]:
    records: list[dict[str, object]] = []
    counts = {
        "directory": 0,
        "payload_hash_recorded": 0,
        "tag_hash_recorded": 0,
        "tag_self_referential": 0,
        "unverifiable_payload": 0,
        "unverifiable_tag": 0,
    }
    tag_covered: set[str] = set()
    for entries in bag.tagmanifests.values():
        tag_covered.update(entries)

    for member in members:
        record: dict[str, object] = {"member": f"bag/{member}"}
        algorithms = sorted(
            algorithm
            for algorithm, entries in bag.manifests.items()
            if member in entries
        )
        if member.startswith("data/"):
            if algorithms:
                record["verdict"] = "hash-recorded"
                record["algorithms"] = algorithms
                counts["payload_hash_recorded"] += 1
            else:
                record["verdict"] = "unverifiable"
                record["note"] = "no manifest-*.txt entry records this payload member"
                counts["unverifiable_payload"] += 1
        elif member in ("bagit.txt", "bag-info.txt"):
            if member in tag_covered:
                record["verdict"] = "tag-hash-recorded"
                counts["tag_hash_recorded"] += 1
            else:
                record["verdict"] = "unverifiable"
                record["note"] = "no tagmanifest-*.txt entry covers this tag file"
                counts["unverifiable_tag"] += 1
        elif TAGMANIFEST_FILE_RE.match(Path(member).name):
            record["verdict"] = "self-referential"
            record["note"] = (
                "tagmanifests cannot vouch for themselves; their own bytes are "
                "only provable against an external reference"
            )
            counts["tag_self_referential"] += 1
        else:
            if member in tag_covered:
                record["verdict"] = "tag-hash-recorded"
                counts["tag_hash_recorded"] += 1
            else:
                record["verdict"] = "unverifiable"
                record["note"] = "no tagmanifest-*.txt entry covers this tag file"
                counts["unverifiable_tag"] += 1
        records.append(record)
    return records, counts


def compare_member_sets(members: list[str], bag: Bag) -> tuple[list[str], list[str]]:
    """Archive members vs. local bag tree; returns (archive_only, bag_only)."""
    local: set[str] = set()
    for path in bag.root.rglob("*"):
        if path.is_file() or path.is_symlink():
            relative = path.relative_to(bag.root).as_posix()
            local.add(relative)
    archived = set(members)
    return sorted(archived - local), sorted(local - archived)


def manifest_content_matches_archive(archive: Path, bag: Bag) -> list[str]:
    """Byte-compare each small manifest/tag file between bag/ and the archive."""
    differences: list[str] = []
    candidates = sorted(
        path
        for path in bag.root.iterdir()
        if path.is_file()
        and (MANIFEST_FILE_RE.match(path.name) or path.name in ("bagit.txt", "bag-info.txt"))
    )
    for path in candidates:
        member = f"bag/{path.name}"
        completed = subprocess.run(
            [BSDTAR, "-x", "-O", "-f", str(archive), member],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )
        if completed.returncode != 0:
            differences.append(f"{path.name}: absent from archive")
            continue
        if completed.stdout != path.read_bytes():
            differences.append(f"{path.name}: bag/ copy differs from archived bytes")
    return differences


# ---------------------------------------------------------------------------
# Reporting


def build_report(
    status: str,
    reason_code: str | None,
    exit_reason: str,
    command: list[str],
    extra: dict[str, object] | None = None,
) -> dict[str, object]:
    report: dict[str, object] = {
        "schema": SCHEMA_VERSION,
        "name": REPORT_NAME,
        "status": status,
        "invariant": INVARIANT,
        "reason_code": reason_code,
        "data": {"profile": "data-none"},
        "artifacts": [],
        "command": command,
        "exit_reason": exit_reason,
    }
    if extra:
        report["findings"] = extra
    return report


def emit(report: dict[str, object], output: Path | None, artifacts: list[dict[str, str]]) -> dict[str, object]:
    report["artifacts"] = artifacts
    text = json.dumps(report, ensure_ascii=False, indent=2, sort_keys=True) + "\n"
    if output is not None:
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(text, encoding="utf-8")
    sys.stdout.write(text)
    return report


def run(argv: list[str] | None = None) -> tuple[int, dict[str, object]]:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    default_root = repo_root()
    parser.add_argument("--repo-root", type=Path, default=default_root)
    parser.add_argument("--archive", type=Path, default=None)
    parser.add_argument("--bag", type=Path, default=None)
    parser.add_argument("--output", type=Path, default=None)
    args = parser.parse_args(argv)

    archive = args.archive or (args.repo_root / "prototype-data.7z")
    bag_root = args.bag or (args.repo_root / "bag")
    command = [
        "Build/verify_prototype_archive.py",
        "--repo-root", str(args.repo_root),
        "--archive", str(archive),
        "--bag", str(bag_root),
    ]
    output_path = args.output
    if output_path is None:
        directory = artifact_dir()
        if directory is not None:
            output_path = directory / "prototype-archive.json"

    findings: dict[str, object] = {}

    def finish(status: str, reason_code: str | None, exit_reason: str,
               extra: dict[str, object] | None = None) -> tuple[int, dict[str, object]]:
        report = build_report(status, reason_code, exit_reason, command, extra)
        artifacts = []
        if output_path is not None:
            artifacts.append({"kind": "json-report", "path": str(output_path)})
        emit(report, output_path, artifacts)
        return (0 if status in ("pass", "blocked") else 1), report

    if not archive.is_file():
        return finish("blocked", "data.archive_missing",
                      f"{archive.name} is not present on this machine")
    if not bag_root.is_dir():
        return finish("blocked", "data.bag_incomplete",
                      f"bag directory {bag_root.name}/ is missing")

    try:
        bag = Bag(bag_root)
        bag.load()
    except VerificationError as error:
        return finish("blocked", "data.bag_incomplete", str(error))

    problems: list[str] = list(bag.problems)

    try:
        members = list_archive_members(archive)
    except VerificationError as error:
        return finish("fail", "data.archive_listing_failed", str(error),
                      {"problems": problems})
    findings["archive_member_count"] = len(members)
    findings["unsupported_bag_algorithms"] = sorted(set(bag.unsupported_algorithms))

    payload_counts = verify_local_payload(bag, problems)
    oxum_ok = check_payload_oxum(bag, problems)

    archive_only, bag_only = compare_member_sets(members, bag)
    if archive_only:
        problems.extend(f"in archive but not in bag/: {member}" for member in archive_only)
    if bag_only:
        problems.extend(f"in bag/ but not in archive: {member}" for member in bag_only)
    differences = manifest_content_matches_archive(archive, bag)
    problems.extend(differences)

    records, verdict_counts = classify_members(members, bag)
    gaps = [record for record in records if record["verdict"] == "unverifiable"]

    findings["local_verification"] = {
        **payload_counts,
        "payload_oxum_matches": oxum_ok,
    }
    findings["member_sets"] = {"archive_only": archive_only, "bag_only": bag_only}
    findings["manifest_vs_archive_differences"] = differences
    findings["members"] = records
    findings["verdict_counts"] = verdict_counts
    findings["gaps"] = gaps
    findings["limits"] = [
        "recorded hashes prove intent; proving the bytes inside the 7z match "
        "them would require extraction, which this check deliberately skips "
        "(small tag/manifest members are byte-compared against bag/ instead)",
        "BagIt records per-member checksums but only aggregate sizes "
        "(Payload-Oxum); per-member sizes inside the archive are unchecked",
        "each tagmanifest cannot cover itself, so its own integrity is "
        "outward-facing only",
        "unknown manifest algorithm names are listed under "
        "unsupported_bag_algorithms and contribute no verification",
    ]

    status = "pass" if not problems else "fail"
    reason_code = None if not problems else "data.identity_gap"
    exit_reason = "dev-data identity holds" if not problems else "; ".join(problems[:10])
    if len(problems) > 10:
        exit_reason += f"; (+{len(problems) - 10} more)"
    return finish(status, reason_code, exit_reason, findings)


def main(argv: list[str] | None = None) -> int:
    try:
        code, _report = run(argv)
    except VerificationError as error:  # defensive: never traceback into ctest logs
        print(f"verify_prototype_archive: {error}", file=sys.stderr)
        return 1
    return code


if __name__ == "__main__":
    raise SystemExit(main())
