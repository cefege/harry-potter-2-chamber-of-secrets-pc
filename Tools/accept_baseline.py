#!/usr/bin/env python3
"""Accept a golden/baseline candidate into Tests/Fixtures/baselines/.

Golden updates must be reviewable artifacts instead of silent diffs (see
AGENTS.md forbidden actions). This tool is the single entry point for
landing a candidate baseline: it validates the payload, copies it under a
content-derived deterministic name (never wall-clock), records an explicit
acceptance row in ACCEPTANCE.jsonl, and prints the exact git-add line plus
the reminder that golden changes require their own acceptance commit.

Usage:
    python3 Tools/accept_baseline.py --kind framing --candidate out/framing.json \
        --reason "text pipeline rework" --ticket HP2-1234

    python3 Tools/accept_baseline.py --check

Kinds:
    framing  captured text/frame JSON snapshot
    smoke    smoke-run JSON report used as the expected output
    assets   captured frame PNG compared by Tools/baseline_compare.py

The ledger and stored names contain no timestamps; two acceptances of the
same content produce byte-identical state.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import sys
from pathlib import Path

LEDGER_NAME = "ACCEPTANCE.jsonl"
COMMITTED_BY = "accept_baseline"
SCHEMA_VERSION = 1

_KIND_EXTENSIONS = {"framing": ".json", "smoke": ".json", "assets": ".png"}


class AcceptBaselineError(Exception):
    pass


def _sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def _validate_payload(kind: str, data: bytes) -> None:
    if not data:
        raise AcceptBaselineError(f"candidate is empty ({kind})")
    if kind == "assets":
        if not data.startswith(b"\x89PNG\r\n\x1a\n"):
            raise AcceptBaselineError("assets candidate is not a PNG image")
        return
    try:
        json.loads(data.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise AcceptBaselineError(f"{kind} candidate is not valid JSON: {error}") from error


def _load_ledger(ledger: Path) -> list[dict[str, object]]:
    if not ledger.is_file():
        return []
    rows: list[dict[str, object]] = []
    for number, line in enumerate(ledger.read_text(encoding="utf-8").splitlines(), start=1):
        line = line.strip()
        if not line:
            continue
        try:
            rows.append(json.loads(line))
        except json.JSONDecodeError as error:
            raise AcceptBaselineError(f"{ledger}:{number}: malformed ledger row: {error}") from error
    return rows


def accept(repo: Path, kind: str, candidate: Path, reason: str, ticket: str) -> int:
    if not candidate.is_file():
        raise AcceptBaselineError(f"candidate not found: {candidate}")
    payload = candidate.read_bytes()
    _validate_payload(kind, payload)

    candidate_sha = _sha256(payload)
    baseline_dir = repo / "Tests" / "Fixtures" / "baselines" / kind
    baseline_dir.mkdir(parents=True, exist_ok=True)

    # Deterministic, date-free name derived purely from content.
    baseline_path = baseline_dir / f"{candidate_sha}{_KIND_EXTENSIONS[kind]}"
    already_stored = baseline_path.is_file()
    if not already_stored:
        tmp = baseline_path.with_suffix(baseline_path.suffix + ".tmp")
        shutil.copyfile(candidate, tmp)
        if _sha256(tmp.read_bytes()) != candidate_sha:
            tmp.unlink(missing_ok=True)
            raise AcceptBaselineError("copy verification failed; refusing to store")
        shutil.move(str(tmp), str(baseline_path))

    ledger = repo / "Tests" / "Fixtures" / "baselines" / LEDGER_NAME
    rows = _load_ledger(ledger)
    record: dict[str, object] = {
        "baseline_sha256": candidate_sha,
        "candidate_sha256": candidate_sha,
        "committed_by": COMMITTED_BY,
        "kind": kind,
        "reason": reason,
        "schema_version": SCHEMA_VERSION,
        "ticket": ticket,
        "path": str(baseline_path.relative_to(repo)),
    }
    duplicate = any(
        row.get("kind") == kind and row.get("baseline_sha256") == candidate_sha for row in rows
    )
    if not duplicate:
        rows.append(record)
        # sorted-stable append: keys sorted, rows kept stable across rewrites.
        lines = [json.dumps(row, sort_keys=True, separators=(",", ":")) for row in rows]
        ledger.write_text("\n".join(lines) + "\n", encoding="utf-8")

    print(f"accepted {kind} baseline: {baseline_path.relative_to(repo)}")
    if already_stored or duplicate:
        print("(identical content and ledger row already present; nothing changed)")
    print()
    print("Record to stage:")
    print(f"    git add {baseline_path.relative_to(repo)} {ledger.relative_to(repo)}")
    print()
    print(
        "REMINDER: golden changes require a separate acceptance commit "
        "(AGENTS.md forbidden action: never bundle a golden bump silently "
        "into a feature commit)."
    )
    return 0


def check(repo: Path) -> int:
    ledger = repo / "Tests" / "Fixtures" / "baselines" / LEDGER_NAME
    rows = _load_ledger(ledger)
    if not rows:
        print(f"no acceptance rows in {ledger}")
        return 0
    failures = 0
    for index, row in enumerate(rows, start=1):
        label = f"{ledger}:{index}"
        path_value = row.get("path")
        expected = row.get("baseline_sha256")
        if not isinstance(path_value, str) or not isinstance(expected, str):
            print(f"DRIFT {label}: row missing path/baseline_sha256")
            failures += 1
            continue
        baseline = repo / path_value
        if not baseline.is_file():
            print(f"DRIFT {label}: baseline missing: {path_value}")
            failures += 1
            continue
        actual = _sha256(baseline.read_bytes())
        if actual != expected:
            print(f"DRIFT {label}: {path_value} sha256 {actual} != recorded {expected}")
            failures += 1
        else:
            print(f"ok {label}: {path_value} matches {expected[:12]}…")
    if failures:
        print(f"\n{failures} drift failure(s) detected", file=sys.stderr)
        return 1
    print(f"\nall {len(rows)} acceptance row(s) verified")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", type=Path, default=None)
    parser.add_argument("--kind", choices=sorted(_KIND_EXTENSIONS))
    parser.add_argument("--candidate", type=Path, default=None)
    parser.add_argument("--reason", default="")
    parser.add_argument("--ticket", default="")
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    if not args.check:
        missing = [
            flag
            for flag, value in (
                ("--kind", args.kind),
                ("--candidate", args.candidate),
            )
            if value is None
        ]
        if missing:
            parser.error("accept mode requires " + " and ".join(missing))

    try:
        repo = (args.repo_root or Path(__file__).resolve().parent.parent).resolve()
        if args.check:
            return check(repo)
        return accept(repo, args.kind, args.candidate.resolve(), args.reason, args.ticket)
    except (AcceptBaselineError, OSError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
