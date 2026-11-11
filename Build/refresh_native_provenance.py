#!/usr/bin/env python3
"""Refresh CMake provenance without accepting changed generated headers."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile


REPO = Path(__file__).resolve().parents[1]


def inside(path, root):
    try:
        path.relative_to(root)
        return True
    except ValueError:
        return False


def safe_path(build, value):
    if not isinstance(value, str) or not value:
        raise ValueError("generated header policy contains an empty path")
    relative = Path(value)
    if relative.is_absolute() or ".." in relative.parts:
        raise ValueError("generated header policy contains an unsafe path: " + value)
    path = build
    for component in relative.parts:
        path = path / component
        if path.is_symlink():
            raise ValueError("generated header path contains a symlink: " + str(path))
    if not inside(path.resolve(), build):
        raise ValueError("generated header path escapes build directory: " + str(path))
    return path


def snapshot(build, policy, require_nonempty=True):
    headers = {}

    def add(path):
        safe_path(build, path.relative_to(build).as_posix())
        if not path.exists():
            return
        if not path.is_file():
            raise ValueError("generated header is not a file: " + str(path))
        digest = hashlib.sha256()
        with path.open("rb") as stream:
            for chunk in iter(lambda: stream.read(1024 * 1024), b""):
                digest.update(chunk)
        headers[path.relative_to(build).as_posix()] = digest.hexdigest()

    specs = policy.get("generated_native_headers")
    if not isinstance(specs, list) or not specs:
        raise ValueError("generated header policy is empty or invalid")
    for spec in specs:
        if not isinstance(spec, dict) or ("path" in spec) == ("path_prefix" in spec):
            raise ValueError("invalid generated header policy entry")
        if "path" in spec:
            add(safe_path(build, spec["path"]))
            continue
        prefix = safe_path(build, spec["path_prefix"])
        if not prefix.exists():
            continue
        if not prefix.is_dir():
            raise ValueError("generated header prefix is not a directory: " + str(prefix))

        def walk_error(error):
            raise error

        for directory, dirs, files in os.walk(prefix, onerror=walk_error, followlinks=False):
            base = Path(directory)
            for name in dirs:
                child = base / name
                if child.is_symlink():
                    raise ValueError("generated header prefix contains a symlink directory: " + str(child))
            for name in files:
                add(base / name)
    if require_nonempty and not headers:
        raise ValueError("no existing generated headers found")
    return dict(sorted(headers.items()))


def validate_preset(preset, build):
    presets = {}
    seen = set()
    inputs = {str(REPO / "CMakeUserPresets.json"): None}

    def load(path):
        path = path.resolve()
        if path in seen:
            return
        seen.add(path)
        content = path.read_bytes()
        inputs[str(path)] = hashlib.sha256(content).hexdigest()
        data = json.loads(content)
        for include in data.get("include", []):
            if not isinstance(include, str) or "$" in include:
                raise ValueError("unsupported preset include")
            load(path.parent / include)
        for item in data.get("configurePresets", []):
            if not isinstance(item, dict) or not isinstance(item.get("name"), str):
                raise ValueError("invalid configure preset")
            if item["name"] in presets:
                raise ValueError("duplicate configure preset: " + item["name"])
            presets[item["name"]] = item

    load(REPO / "CMakePresets.json")
    user_presets = REPO / "CMakeUserPresets.json"
    if user_presets.is_file():
        load(user_presets)

    def binary_dir(name, ancestors):
        if name in ancestors or name not in presets:
            raise ValueError("unknown or cyclic configure preset: " + name)
        item = presets[name]
        if "binaryDir" in item:
            return item["binaryDir"]
        parents = item.get("inherits", [])
        if isinstance(parents, str):
            parents = [parents]
        if not isinstance(parents, list):
            raise ValueError("invalid configure preset inheritance")
        for parent in parents:
            value = binary_dir(parent, ancestors | {name})
            if value is not None:
                return value
        return None

    value = binary_dir(preset, set())
    if not isinstance(value, str) or not value:
        raise ValueError("configure preset has no binaryDir: " + preset)
    variables = {"sourceDir": str(REPO), "sourceParentDir": str(REPO.parent),
                 "sourceDirName": REPO.name, "presetName": preset, "dollar": "$"}
    value = re.sub(r"\$\{([^}]+)\}", lambda m: variables.get(m.group(1), m.group(0)), value)
    value = re.sub(r"\$(?:p?env)\{([^}]+)\}", lambda m: os.environ.get(m.group(1), ""), value)
    if "$" in value:
        raise ValueError("unsupported binaryDir preset expansion: " + value)
    expected = Path(value)
    if not expected.is_absolute():
        expected = REPO / expected
    if expected.resolve() != build:
        raise ValueError("configure preset binaryDir does not match build directory")
    return dict(sorted(inputs.items()))


def cache_hashes(build):
    entries = {}
    for line in (build / "CMakeCache.txt").read_text(encoding="utf-8").splitlines():
        if not line or line.startswith(("#", "//")):
            continue
        name_type, separator, value = line.partition("=")
        name, type_separator, entry_type = name_type.rpartition(":")
        if not separator or not type_separator or not name or not entry_type or name in entries:
            raise ValueError("malformed or duplicate CMake cache entry")
        entries[name] = hashlib.sha256((entry_type + "\0" + value).encode("utf-8")).hexdigest()
    if not entries:
        raise ValueError("CMake cache is empty")
    return dict(sorted(entries.items()))


def differences(before, after):
    return {"added": sorted(after.keys() - before.keys()),
            "removed": sorted(before.keys() - after.keys()),
            "changed": sorted(key for key in before.keys() & after.keys() if before[key] != after[key])}


def validate_cache(build):
    cache = build / "CMakeCache.txt"
    entries = {}
    for line in cache.read_text(encoding="utf-8").splitlines():
        if line.startswith(("CMAKE_HOME_DIRECTORY:", "CMAKE_CACHEFILE_DIR:", "CMAKE_GENERATOR:")) and "=" in line:
            key, value = line.split("=", 1)
            entries[key.split(":", 1)[0]] = value
    if (Path(entries.get("CMAKE_HOME_DIRECTORY", "")).resolve() != REPO
            or Path(entries.get("CMAKE_CACHEFILE_DIR", "")).resolve() != build):
        raise ValueError("CMake cache source or build directory does not match")
    if entries.get("CMAKE_GENERATOR") != "Ninja":
        raise ValueError("native provenance refresh requires the Ninja generator")


def build_state(preset, build):
    presets = validate_preset(preset, build)
    validate_cache(build)
    result = subprocess.run(["ninja", "-C", str(build), "-t", "commands"], cwd=REPO,
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False)
    if result.returncode:
        raise ValueError("cannot inspect Ninja command graph: status " + str(result.returncode)
                         + ", stderr sha256 " + hashlib.sha256(result.stderr).hexdigest())
    if not result.stdout:
        raise ValueError("Ninja command graph is empty")
    policy = (REPO / "Build" / "native-free-allowlist.json").read_bytes()
    helper = Path(__file__).resolve().read_bytes()
    return {"schema_version": 1, "source_dir": str(REPO), "build_dir": str(build),
            "preset": preset, "cache": cache_hashes(build), "preset_inputs": presets,
            "policy_sha256": hashlib.sha256(policy).hexdigest(),
            "helper_sha256": hashlib.sha256(helper).hexdigest(),
            "ninja_commands_sha256": hashlib.sha256(result.stdout).hexdigest()}


def compare_state(expected, actual):
    if not isinstance(expected, dict):
        raise ValueError("invalid recorded native build state")
    changed = sorted(key for key in expected.keys() | actual.keys() if expected.get(key) != actual.get(key))
    if changed:
        raise ValueError("native build state differs: " + ", ".join(changed))


def record_state(path, state):
    data = (json.dumps(state, indent=2, sort_keys=True) + "\n").encode("utf-8")
    fd = os.open(str(path), os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(fd, "wb") as stream:
        stream.write(data)
        stream.flush()
        os.fsync(stream.fileno())


def write_json(path, value):
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def retain_manifest(build, artifacts, stage):
    manifest = safe_path(build, "hp2-native-free-manifest.json")
    if manifest.is_symlink() and not manifest.exists():
        raise ValueError("broken native-free manifest symlink")
    if not manifest.exists():
        return None
    if not manifest.is_file():
        raise ValueError("native-free manifest is not a file")
    data = manifest.read_bytes()
    (artifacts / ("manifest-" + stage + ".json")).write_bytes(data)
    return hashlib.sha256(data).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--preset", required=True)
    parser.add_argument("--build-dir", required=True, type=Path)
    parser.add_argument("--record-build-state", action="store_true")
    args = parser.parse_args()
    artifacts_value = os.environ.get("HP2_ARTIFACT_DIR")
    if not artifacts_value:
        print("refresh-native-provenance: HP2_ARTIFACT_DIR is required", file=sys.stderr)
        return 1
    try:
        Path(artifacts_value).mkdir(parents=True, exist_ok=True)
        artifacts = Path(tempfile.mkdtemp(prefix="refresh-native-provenance-", dir=artifacts_value)).resolve()
    except OSError as exc:
        print("refresh-native-provenance: cannot create evidence directory: " + str(exc), file=sys.stderr)
        return 1
    build = args.build_dir.resolve()
    command = ["cmake", "--preset", args.preset, "-B", str(build)]
    report = {"command": ["ninja", "-C", str(build), "-t", "commands"] if args.record_build_state else command,
              "cwd": str(REPO), "build_dir": str(build), "ok": False,
              "mode": "record-build-state" if args.record_build_state else "refresh"}
    try:
        state_path = Path(artifacts_value).resolve() / "native-build-state.json"
        current = build_state(args.preset, build)
        write_json(artifacts / "cache-before.json", current["cache"])
        write_json(artifacts / "preset-inputs-before.json", current["preset_inputs"])
        report["ninja_commands_before_sha256"] = current["ninja_commands_sha256"]
        if args.record_build_state:
            record_state(state_path, current)
            report["build_state"] = str(state_path)
            report["ok"] = True
        else:
            if state_path.is_symlink() or not state_path.is_file():
                raise ValueError("recorded native build state is missing or unsafe")
            state_bytes = state_path.read_bytes()
            recorded = json.loads(state_bytes)
            compare_state(recorded, current)
            report["build_state_sha256"] = hashlib.sha256(state_bytes).hexdigest()
            policy_path = REPO / "Build" / "native-free-allowlist.json"
            policy_bytes = policy_path.read_bytes()
            policy = json.loads(policy_bytes)
            report["policy_sha256"] = current["policy_sha256"]
            report["manifest_before_sha256"] = retain_manifest(build, artifacts, "before")
            before = snapshot(build, policy)
            write_json(artifacts / "before.json", before)
            with (artifacts / "configure.log").open("w", encoding="utf-8") as log:
                result = subprocess.run(command, cwd=REPO, stdout=log, stderr=subprocess.STDOUT, check=False)
            report["configure_status"] = result.returncode
            report["manifest_after_sha256"] = retain_manifest(build, artifacts, "after")
            after = snapshot(build, policy, require_nonempty=False)
            write_json(artifacts / "after.json", after)
            final = build_state(args.preset, build)
            write_json(artifacts / "cache-after.json", final["cache"])
            write_json(artifacts / "preset-inputs-after.json", final["preset_inputs"])
            report["ninja_commands_after_sha256"] = final["ninja_commands_sha256"]
            header_diff = differences(before, after)
            report.update(header_diff)
            report["cache"] = differences(current["cache"], final["cache"])
            report["preset_inputs"] = differences(current["preset_inputs"], final["preset_inputs"])
            if result.returncode:
                raise ValueError("configure failed with status " + str(result.returncode))
            if any(header_diff.values()):
                raise ValueError("generated headers changed: " + json.dumps(header_diff, sort_keys=True))
            compare_state(recorded, final)
            report["ok"] = True
    except (OSError, ValueError, TypeError, RuntimeError) as exc:
        report["error"] = str(exc)
    write_json(artifacts / "report.json", report)
    print("refresh-native-provenance: " + ("passed" if report["ok"] else "failed: " + report["error"]))
    print("refresh-native-provenance: evidence " + str(artifacts))
    return 0 if report["ok"] else 1


if __name__ == "__main__":
    sys.exit(main())
