#!/usr/bin/env python3
"""Check Cargo references in the supported CI run syntax and exact CTest names.

This is deliberately not a YAML or shell parser. Cargo commands must be simple
standalone commands in run scalars or literal blocks. Unsupported Cargo syntax
is a finding. The web-cpp-freeze checkout is not the current workspace.
Without CTest options, only Cargo and skills are checked; CTest is not checked.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path, PureWindowsPath
import re
import shlex
import subprocess
import sys


def load_json(path):
    with open(path, encoding="utf-8") as stream:
        return json.load(stream)


def workspace_packages(metadata):
    if not isinstance(metadata, dict) or not isinstance(metadata.get("packages"), list) or not isinstance(metadata.get("workspace_members"), list):
        raise ValueError("metadata needs packages and workspace_members arrays")
    members = metadata["workspace_members"]
    if not members or any(not isinstance(x, str) or not x for x in members) or len(set(members)) != len(members):
        raise ValueError("invalid workspace_members")
    packages = {}
    ids = set()
    for package in metadata["packages"]:
        if not isinstance(package, dict) or not isinstance(package.get("id"), str) or not package["id"] or not isinstance(package.get("name"), str) or not package["name"] or not isinstance(package.get("features"), dict):
            raise ValueError("invalid package metadata")
        if any(not isinstance(k, str) or not isinstance(v, list) or any(not isinstance(x, str) for x in v) for k, v in package["features"].items()):
            raise ValueError("invalid package features")
        if package["id"] in ids:
            raise ValueError("duplicate package id")
        ids.add(package["id"])
        if package["id"] in members:
            if package["name"] in packages:
                raise ValueError("duplicate workspace package name")
            packages[package["name"]] = package["features"]
    if not set(members).issubset(ids):
        raise ValueError("workspace member missing from packages")
    return packages


def cargo_lines(workflow):
    """Extract simple run lines from the workflow's jobs/steps indentation.

    Reject Cargo text outside recognized run lines rather than guessing at YAML.
    """
    lines = workflow.splitlines()
    job = None
    jobs = {}
    in_jobs = False
    block_indent = None
    step = None
    for number, line in enumerate(lines, 1):
        stripped = line.strip()
        if not stripped or stripped.startswith("#"):
            continue
        indent = len(line) - len(line.lstrip(" "))
        if "\t" in line[:len(line) - len(line.lstrip())]:
            raise ValueError("tabs in workflow indentation")
        if indent == 0:
            in_jobs = stripped == "jobs:"
            job = None
        if in_jobs and indent == 2 and re.fullmatch(r"[A-Za-z0-9_-]+:", stripped):
            job = stripped[:-1]
            if job in jobs:
                raise ValueError("duplicate job " + job)
            jobs[job] = {"steps": [], "commands": []}
            step = None
        if block_indent is not None and indent <= block_indent:
            block_indent = None
        if not job:
            if re.search(r"\bcargo\b", stripped):
                raise ValueError("Cargo outside supported job run at line %d" % number)
            continue
        if indent == 6 and stripped.startswith("- "):
            step = {"uses": [], "refs": [], "uses_line": None}
            jobs[job]["steps"].append(step)
        uses = re.fullmatch(r"(?:-\s+)?uses:\s*(\S+)\s*(?:#.*)?", stripped)
        ref = re.fullmatch(r"ref:\s*(\S+)\s*(?:#.*)?", stripped)
        if uses:
            if step is None:
                raise ValueError("uses outside supported step at line %d" % number)
            step["uses"].append(uses.group(1).strip("'\""))
            step["uses_line"] = number
        if stripped.startswith("ref:"):
            if not ref or step is None:
                raise ValueError("unsupported ref at line %d" % number)
            step["refs"].append(ref.group(1).strip("'\""))
        match = re.fullmatch(r"(?:-\s+)?run:\s*(.*)", stripped)
        command = None
        if match:
            value = match.group(1)
            if value in ("|", "|-"):
                block_indent = indent
            elif value and not value.startswith((">", "|")):
                command = value
            else:
                raise ValueError("unsupported run syntax at line %d" % number)
        elif block_indent is not None:
            command = stripped
        if command is not None:
            jobs[job]["commands"].append((number, command))
        elif re.search(r"\bcargo\b", stripped):
            raise ValueError("Cargo outside supported run at line %d" % number)
    if not jobs:
        raise ValueError("no workflow jobs found")
    result = []
    for name, data in jobs.items():
        checkouts = []
        for step in data["steps"]:
            checkout = len(step["uses"]) == 1 and re.fullmatch(r"actions/checkout@[^\s]+", step["uses"][0])
            if len(step["uses"]) > 1:
                raise ValueError("ambiguous uses in job " + name)
            if step["refs"] and (not checkout or len(step["refs"]) != 1):
                raise ValueError("ref outside an unambiguous checkout in job " + name)
            if checkout:
                checkouts.append(step)
        explicit = [step for step in checkouts if step["refs"]]
        if explicit:
            if len(checkouts) == 1 and explicit[0]["refs"] == ["web-cpp-freeze"]:
                for number, command in data["commands"]:
                    if number < explicit[0]["uses_line"] and re.search(r"\bcargo\b", command):
                        raise ValueError("Cargo before frozen checkout at line %d" % number)
                continue
            raise ValueError("unsupported or ambiguous checkout ref in job " + name)
        result.extend(data["commands"])
    return result


def check_cargo(lines, packages):
    findings = []
    seen = 0
    for number, command in lines:
        try:
            tokens = shlex.split(command, comments=True)
        except ValueError as exc:
            if re.search(r"\bcargo\b", command):
                findings.append("line %d: %s" % (number, exc))
            continue
        if not any(token == "cargo" or Path(token).name == "cargo" for token in tokens):
            continue
        if not tokens or tokens[0] != "cargo" or len(tokens) < 2 or any(re.search(r"[;&|`$<>]", t) for t in tokens):
            findings.append("line %d: unsupported Cargo command" % number)
            continue
        seen += 1
        selected = []
        features = []
        workspace = False
        i = 2
        while i < len(tokens) and tokens[i] != "--":
            token = tokens[i]
            value = None
            if token in ("-p", "--package", "--features", "-F"):
                i += 1
                if i >= len(tokens) or tokens[i].startswith("-"):
                    findings.append("line %d: missing value for %s" % (number, token))
                    break
                value = tokens[i]
            elif token.startswith("--package=") or token.startswith("--features="):
                token, value = token.split("=", 1)
            elif token.startswith("-p") and len(token) > 2:
                token, value = "-p", token[2:]
            elif token.startswith("-F") and len(token) > 2:
                token, value = "-F", token[2:]
            if token in ("-p", "--package"):
                selected.append(value)
            elif token in ("-F", "--features"):
                features.extend(re.split(r"[,\s]+", value))
            elif token == "--workspace":
                workspace = True
            elif token in ("--manifest-path", "--exclude", "--all") or token.startswith(("--manifest-path=", "--exclude=")):
                findings.append("line %d: unsupported package selection %s" % (number, token))
            i += 1
        for name in selected:
            if name not in packages:
                findings.append("line %d: unknown workspace package %s" % (number, name))
        for feature in features:
            if not feature:
                findings.append("line %d: empty feature" % number)
                continue
            if "/" in feature:
                name, feat = feature.split("/", 1)
                valid = name in packages and feat in packages[name] and (workspace or not selected or name in selected)
            else:
                targets = list(packages) if workspace else selected
                valid = bool(targets) and any(feature in packages.get(name, {}) for name in targets)
            if not valid:
                findings.append("line %d: unknown or ambiguous feature %s" % (number, feature))
    if not seen:
        findings.append("no supported Cargo commands found")
    return findings


def check_skill_lock(lock_path):
    lock = load_json(lock_path)
    if not isinstance(lock, dict) or type(lock.get("schema_version")) is not int or lock["schema_version"] != 1:
        raise ValueError("invalid schema_version")
    if not isinstance(lock.get("commit"), str) or not re.fullmatch(r"[0-9a-fA-F]{40}", lock["commit"]):
        raise ValueError("invalid commit")
    if not isinstance(lock.get("files"), list) or not lock["files"]:
        raise ValueError("files must be a nonempty array")
    root = lock_path.parent.resolve()
    required = {
        "LICENSE.BeautifulRust",
        "upstream/beautiful-rust-lints.toml", "upstream/beautiful-rust-clippy.toml",
        "rust-best-practices/SKILL.md", "rust-review/SKILL.md", "rust-port/SKILL.md",
        "rust-best-practices/references/bevy.md",
        "rust-best-practices/references/effective-rust.md",
        "rust-best-practices/references/google-rust-style-guide.md",
        "rust-best-practices/references/microsoft-rust-guidelines.md",
        "rust-best-practices/references/rust-api-guidelines.md",
        "rust-best-practices/references/rust-design-patterns.md",
    }
    seen = set()
    findings = []
    for entry in lock["files"]:
        if not isinstance(entry, dict):
            raise ValueError("invalid file entry")
        name, digest = entry.get("path"), entry.get("sha256")
        if not isinstance(name, str) or not name or "\\" in name or any(part in ("", ".", "..") for part in name.split("/")) or Path(name).is_absolute() or PureWindowsPath(name).drive:
            raise ValueError("invalid relative file path: %r" % name)
        if name in seen:
            raise ValueError("duplicate file path: " + name)
        seen.add(name)
        if not isinstance(digest, str) or not re.fullmatch(r"[0-9a-fA-F]{64}", digest):
            raise ValueError("invalid SHA-256: " + name)
        path = (root / name).resolve()
        try:
            path.relative_to(root)
        except ValueError:
            raise ValueError("file escapes skill root: " + name)
        try:
            actual = hashlib.sha256(path.read_bytes()).hexdigest()
        except OSError as exc:
            findings.append("skill file %s: %s" % (name, exc))
            continue
        if actual != digest.lower():
            findings.append("skill hash mismatch: " + name)
    for name in sorted(required - seen):
        findings.append("required skill file missing from lock: " + name)
    return findings


def check_tests(listing, required, required_labels=()):
    if not isinstance(listing, dict) or not isinstance(listing.get("tests"), list):
        return ["malformed CTest listing: tests must be an array"]
    findings = []
    if not listing["tests"]:
        findings.append("empty CTest listing")
    names = {}
    for test in listing["tests"]:
        if not isinstance(test, dict) or not isinstance(test.get("name"), str) or not test["name"]:
            findings.append("malformed CTest test entry")
            continue
        names.setdefault(test["name"], []).append(test)
    for name, entries in names.items():
        if len(entries) > 1:
            findings.append("duplicate CTest name: " + name)
    for name in required:
        entries = names.get(name, [])
        if not entries:
            findings.append("required CTest missing: " + name)
        for entry in entries:
            command = entry.get("command")
            if not isinstance(command, list) or not command or any(not isinstance(x, str) for x in command) or not command[0].strip():
                findings.append("required CTest has empty or malformed command: " + name)
            properties = entry.get("properties", [])
            if not isinstance(properties, list):
                findings.append("required CTest has malformed properties: " + name)
                continue
            disabled_seen = False
            labels_seen = False
            labels = set()
            for prop in properties:
                if not isinstance(prop, dict) or not isinstance(prop.get("name"), str) or not prop["name"] or "value" not in prop:
                    findings.append("required CTest has malformed properties: " + name)
                    continue
                if prop["name"] == "LABELS":
                    if labels_seen:
                        findings.append("required CTest has duplicate LABELS property: " + name)
                    labels_seen = True
                    value = prop["value"]
                    if not isinstance(value, list) or any(not isinstance(label, str) or not label.strip() for label in value):
                        findings.append("required CTest has malformed LABELS property: " + name)
                    else:
                        labels.update(value)
                    continue
                if prop["name"] != "DISABLED":
                    continue
                value = prop["value"]
                if disabled_seen:
                    findings.append("required CTest has duplicate DISABLED property: " + name)
                disabled_seen = True
                if isinstance(value, bool):
                    disabled = value
                elif isinstance(value, str) and value.upper() in ("1", "ON", "YES", "TRUE", "Y", "0", "OFF", "NO", "FALSE", "N", "IGNORE", "NOTFOUND", ""):
                    disabled = value.upper() in ("1", "ON", "YES", "TRUE", "Y")
                else:
                    findings.append("required CTest has malformed DISABLED property: " + name)
                    continue
                if disabled:
                    findings.append("required CTest is DISABLED: " + name)
            for label in required_labels:
                if label not in labels:
                    findings.append("required CTest missing label %s: %s" % (label, name))
    return findings


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, default=Path(__file__).resolve().parent.parent)
    parser.add_argument("--metadata-json", type=Path)
    parser.add_argument("--ctest-json", type=Path)
    parser.add_argument("--require-test", action="append", default=[])
    parser.add_argument("--require-label", action="append", default=[], help="label every required test must have in the selected suite")
    parser.add_argument("--output", type=Path, help="report JSON file (default: HP2_ARTIFACT_DIR/agent-contracts.json)")
    args = parser.parse_args(argv)
    output = args.output or (Path(os.environ["HP2_ARTIFACT_DIR"]) / "agent-contracts.json" if os.environ.get("HP2_ARTIFACT_DIR") else None)
    if output is None:
        print("--output or HP2_ARTIFACT_DIR is required", file=sys.stderr)
        return 2
    findings = []
    workflow_path = (args.repo / ".github/workflows/ci.yml").resolve()
    lock_path = args.repo.resolve() / ".agents/skills/beautiful-rust.lock.json"
    metadata_command = ["cargo", "metadata", "--locked", "--offline", "--no-deps", "--format-version", "1"]
    start = len(findings)
    try:
        findings.extend(check_skill_lock(lock_path))
    except (OSError, ValueError, UnicodeError, RuntimeError) as exc:
        findings.append("skill lock: " + str(exc))
    skills_state = "fail" if len(findings) > start else "pass"
    start = len(findings)
    try:
        if args.metadata_json:
            metadata = load_json(args.metadata_json)
        else:
            result = subprocess.run(metadata_command, cwd=str(args.repo), capture_output=True, text=True, check=False)
            if result.returncode:
                raise ValueError("cargo metadata failed: " + result.stderr.strip())
            metadata = json.loads(result.stdout)
        packages = workspace_packages(metadata)
        workflow = workflow_path.read_text(encoding="utf-8")
        findings.extend(check_cargo(cargo_lines(workflow), packages))
    except (OSError, ValueError, UnicodeError) as exc:
        findings.append("Cargo/workflow: " + str(exc))
    cargo_state = "fail" if len(findings) > start else "pass"
    start = len(findings)
    ctest_state = "not_checked"
    if args.require_test or args.ctest_json or args.require_label:
        if args.ctest_json and not args.require_test:
            findings.append("--ctest-json needs at least one --require-test")
        if args.require_label and not args.require_test:
            findings.append("--require-label needs at least one --require-test")
        if not args.ctest_json:
            findings.append("required tests need --ctest-json")
        else:
            try:
                findings.extend(check_tests(load_json(args.ctest_json), args.require_test, args.require_label))
            except (OSError, ValueError, UnicodeError) as exc:
                findings.append("CTest listing: " + str(exc))
        ctest_state = "fail" if len(findings) > start else "pass"
    report = {"version": 1, "checks": {"cargo": cargo_state, "skills": skills_state, "ctest": ctest_state}, "ok": not findings, "findings": findings, "required_tests": args.require_test, "required_labels": args.require_label,
              "inputs": {"repo": str(args.repo.resolve()), "workflow": str(workflow_path), "skill_lock": str(lock_path),
                         "metadata_fixture": str(args.metadata_json.resolve()) if args.metadata_json else None,
                         "metadata_command": None if args.metadata_json else metadata_command,
                         "ctest_listing": str(args.ctest_json.resolve()) if args.ctest_json else None}}
    try:
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    except OSError as exc:
        print("cannot write report: " + str(exc), file=sys.stderr)
        return 2
    print("%s: %s" % ("PASS" if not findings else "FAIL", output))
    return 1 if findings else 0


if __name__ == "__main__":
    sys.exit(main())
