#!/usr/bin/env python3
"""Fail-closed provenance check for selected shipping product closures."""
from __future__ import annotations

import argparse
from collections import Counter
import functools
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shlex
import re
import subprocess
import tarfile
import sys

NATIVE_SUFFIXES = {".c", ".cc", ".cpp", ".cxx", ".C", ".c++", ".cppm", ".cxxm", ".ixx", ".mpp", ".m", ".mm", ".s", ".S", ".asm", ".o", ".obj", ".a", ".lib", ".h", ".hh", ".hpp", ".hxx", ".h++", ".inc", ".ipp", ".tpp", ".tcc", ".inl"}
FORBIDDEN = re.compile(r"(?:hp2[-_]native[-_]exception(?![\w-])|hp2[-_]core[-_]runtime(?![\w-])|hp2[-_]ffi(?=$|[\s./\\\\])|gen_engine_abi|HP2EngineAbiAsserts|cbindgen)", re.I)
LEGACY_NATIVE_DIR = re.compile(r"(?:^|[/\\])(?:Core|FFI|NativeException)(?:[/\\]|$)", re.I)
ABI_SYMBOLS = re.compile(r"(?:__cxa_(?:throw|rethrow|begin_catch|end_catch)|__gxx_personality|_ZTV|_ZTI|_ZTT|typeinfo for|vtable for)")
NATIVE_COMPILE_SUFFIXES = {".c", ".cc", ".cpp", ".cxx", ".C", ".c++", ".cppm", ".cxxm", ".ixx", ".mpp", ".m", ".mm", ".s", ".S", ".asm"}


def rust_llvm_nm(cargo: Path, command_dir: Path, environment: dict, cargo_args: list[str]) -> Path:
    """Find llvm-nm in the sysroot of the compiler selected by Cargo."""
    env = cargo_replay_environment(environment)
    env["RUSTC_BOOTSTRAP"] = "1"

    def query(command: list[str]) -> str:
        try:
            result = subprocess.run(command, cwd=command_dir, env=env, text=True,
                                    stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        except OSError as exc:
            raise GuardError(f"cannot execute {command[0]}: {exc}") from exc
        if result.returncode:
            raise GuardError(f"cannot query {command[0]}: {result.stderr.strip()}")
        return result.stdout.strip()

    try:
        config = json.loads(query([str(cargo), "-Z", "unstable-options", *cargo_config_args(cargo_args),
                                   "config", "get", "--format", "json"]))
        rustc = env.get("RUSTC") or env.get("CARGO_BUILD_RUSTC") or config.get("build", {}).get("rustc") or "rustc"
        if not isinstance(rustc, str) or not rustc:
            raise ValueError("invalid build.rustc")
        version = query([rustc, "-vV"])
        host = re.search(r"^host: (\S+)$", version, re.M)
        llvm = re.search(r"^LLVM version: (\S+)$", version, re.M)
        release = re.search(r"^release: (\S+)$", version, re.M)
        if not host or not llvm or not release:
            raise ValueError("missing rustc host, release, or LLVM version")
        sysroot = Path(query([rustc, "--print", "sysroot"]))
        tool = sysroot / "lib" / "rustlib" / host.group(1) / "bin" / ("llvm-nm.exe" if os.name == "nt" else "llvm-nm")
        tool_version = query([str(tool), "--version"])
        tool_llvm = re.search(r"^\s*LLVM version (\S+)\s*$", tool_version, re.M)
        allowed_versions = {llvm.group(1), f"{llvm.group(1)}-rust-{release.group(1)}-stable"}
        if not tool_llvm or tool_llvm.group(1) not in allowed_versions:
            raise ValueError(f"llvm-nm does not match rustc LLVM {llvm.group(1)}")
        return tool
    except (json.JSONDecodeError, AttributeError, TypeError, ValueError) as exc:
        raise GuardError(f"cannot identify matching Rust LLVM tools: {exc}") from exc


def inspect_rust_abi(obj: Path, nm: Path) -> list[str]:
    if not obj.is_file():
        return [f"missing selected Rust compiler-artifact for ABI inspection: {obj}"]
    try:
        result = subprocess.run([str(nm), "-a", str(obj)], text=True,
                                stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    except OSError as exc:
        return [f"cannot inspect selected Rust artifact {obj}: {exc}"]
    # llvm-nm can silently skip unrecognized archive members while exiting 0.
    # Require an output header for every member reported by the matching llvm-ar.
    no_symbols_warnings = set()
    if is_archive(obj):
        try:
            listing = subprocess.run([str(nm.with_name("llvm-ar")), "t", str(obj)], text=True,
                                     stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        except OSError as exc:
            return [f"cannot list selected Rust archive {obj}: {exc}"]
        if listing.returncode or listing.stderr:
            return [f"cannot list selected Rust archive {obj}: {listing.stderr.strip()}"]
        members = listing.stdout.splitlines()
        if not members or any(not member or member.endswith(":") for member in members):
            return [f"invalid or empty selected Rust archive {obj}"]
        expected = Counter(members)
        headers = Counter(line[:-1] for line in result.stdout.splitlines()
                          if line.endswith(":") and line[:-1] in expected)
        if headers != expected:
            return [f"cannot inspect every member of selected Rust archive {obj}"]
        no_symbols_warnings = {f"{obj}:{member}: no symbols" for member in members}
    diagnostics = [line for line in result.stderr.splitlines() if line not in no_symbols_warnings]
    if result.returncode or diagnostics:
        return [f"cannot inspect selected Rust artifact {obj}: {result.stderr.strip()}"]
    if ABI_SYMBOLS.search(result.stdout):
        return [f"first-party Rust compiler-artifact contains C++ ABI dependency: {obj}"]
    return []


def is_archive(path: Path) -> bool:
    try:
        with path.open("rb") as stream:
            return stream.read(8) in {b"!<arch>\n", b"!<thin>\n"}
    except OSError:
        return False


def native_image_type(path: Path) -> str | None:
    try:
        with path.open("rb") as stream:
            data = stream.read(32)
    except OSError:
        return None
    if data.startswith(b"\x7fELF") and len(data) >= 18:
        order = "little" if data[5] == 1 else "big" if data[5] == 2 else ""
        if order:
            kind = int.from_bytes(data[16:18], order)
            return "object" if kind == 1 else "executable" if kind in {2, 3} else "elf-other"
    macho = {
        b"\xfe\xed\xfa\xce": "big",
        b"\xce\xfa\xed\xfe": "little",
        b"\xfe\xed\xfa\xcf": "big",
        b"\xcf\xfa\xed\xfe": "little",
    }
    order = macho.get(data[:4])
    if order and len(data) >= 16:
        kind = int.from_bytes(data[12:16], order)
        return "object" if kind == 1 else "executable" if kind == 2 else "macho-other"
    return None


def is_executable_image(path: Path) -> bool:
    return native_image_type(path) == "executable"


def is_native_object(path: Path) -> bool:
    return native_image_type(path) == "object"


def cargo_registry(allow: dict, repo: Path) -> Path | None:
    raw = allow.get("cargo_registry_root")
    if not raw:
        return None
    cargo_home = Path(os.environ.get("CARGO_HOME", Path.home() / ".cargo")).expanduser()
    return resolved(str(raw).replace("${CARGO_HOME}", str(cargo_home)), repo)


class GuardError(Exception):
    pass


def resolved(value: str | Path, base: Path) -> Path:
    p = Path(value)
    return (p if p.is_absolute() else base / p).resolve(strict=False)


def beneath(path: Path, root: Path) -> bool:
    try:
        path.relative_to(root)
        return True
    except ValueError:
        return False


def load_json(path: Path):
    try:
        return json.loads(path.read_text())
    except (OSError, json.JSONDecodeError) as exc:
        raise GuardError(f"cannot read {path}: {exc}") from exc


def run(args: list[str], cwd: Path) -> str:
    try:
        proc = subprocess.run(args, cwd=cwd, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    except OSError as exc:
        raise GuardError(f"cannot execute {' '.join(args)}: {exc}") from exc
    if proc.returncode:
        raise GuardError(f"{' '.join(args)} failed: {proc.stderr.strip()}")
    return proc.stdout


def ninja_product_commands(build: Path, target: str) -> str:
    candidates = [target, f"_cargo-build_{target}", f"cargo-build_{target}"]
    errors = []
    for candidate in candidates:
        result = subprocess.run(
            ["ninja", "-C", str(build), "-t", "commands", candidate],
            cwd=build, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        )
        if result.returncode == 0 and result.stdout.strip():
            return result.stdout
        errors.append(result.stderr.strip())
    raise GuardError(f"no Ninja product/Cargo build commands for {target}: {'; '.join(errors)}")



def is_generated_rule_label(value: Path) -> bool:
    text = value.as_posix()
    return bool(
        re.search(r"(?:^|/)CMakeFiles/_cargo-build_[^/]+(?:\.rule)?$", text)
        or re.search(r"(?:^|/)CMakeFiles/native_input(?:\.rule)?$", text)
        or re.search(r"(?:^|/)CMakeFiles/[^/]+\.o\.rule$", text)
    )



def cargo_invocation(
    text: str, build: Path, repo: Path,
) -> tuple[Path, Path, list[str], list[str], dict[str, str]]:
    proxy = (repo / "Build" / "capture_cargo_provenance.py").resolve()
    required = {
        "HP2_CARGO_PROVENANCE_TARGET",
        "HP2_CARGO_PROVENANCE_RECORD",
        "HP2_CARGO_PROVENANCE_EXECUTABLE",
    }
    matches = []
    for line in text.splitlines():
        try:
            tokens = shlex.split(line)
        except ValueError as exc:
            raise GuardError(f"cannot parse Ninja Cargo command: {exc}") from exc
        command_dir = build
        if len(tokens) > 1 and tokens[0] == "cd":
            command_dir = resolved(tokens[1], build)
            if "&&" not in tokens:
                continue
            tokens = tokens[tokens.index("&&") + 1:]
        segments, segment = [], []
        for token in tokens:
            if token in {"&&", ";"}:
                if segment:
                    segments.append(segment)
                segment = []
            else:
                segment.append(token)
        if segment:
            segments.append(segment)
        for command in segments:
            if (
                len(command) < 5
                or Path(command[0]).name != "cmake"
                or command[1:3] != ["-E", "env"]
            ):
                continue
            proxy_indexes = [
                index for index, token in enumerate(command[3:], 3)
                if Path(token).is_absolute()
                and Path(os.path.abspath(token)) == proxy
            ]
            for proxy_index in proxy_indexes:
                if Path(command[proxy_index]).resolve() != proxy:
                    continue
                prefix = command[3:proxy_index]
                values: dict[str, str] = {}
                required_counts = {key: 0 for key in required}
                cursor = 0
                while cursor < len(prefix):
                    token = prefix[cursor]
                    cursor += 1
                    if token == "--unset" and cursor < len(prefix):
                        key = prefix[cursor]
                        cursor += 1
                        if key in required:
                            raise GuardError(f"Cargo proxy environment unsets required value {key}")
                        continue
                    if token.startswith("--unset="):
                        key = token.split("=", 1)[1]
                        if key in required:
                            raise GuardError(f"Cargo proxy environment unsets required value {key}")
                        continue
                    if token == "--modify" and cursor < len(prefix):
                        modification = prefix[cursor]
                        cursor += 1
                        key = modification.split("=", 1)[0]
                        if key in required:
                            raise GuardError(f"Cargo proxy environment modifies required value {key}")
                        continue
                    if token.startswith("--modify="):
                        key = token.split("=", 1)[1].split("=", 1)[0]
                        if key in required:
                            raise GuardError(f"Cargo proxy environment modifies required value {key}")
                        continue
                    if "=" not in token or token.startswith("-"):
                        raise GuardError(f"unexpected Cargo proxy environment token: {token}")
                    key, value = token.split("=", 1)
                    values[key] = value
                    if key in required:
                        required_counts[key] += 1
                if any(count != 1 for count in required_counts.values()):
                    missing = sorted(key for key, count in required_counts.items() if count != 1)
                    raise GuardError(f"Cargo proxy command has missing/duplicate environment bindings: {missing}")
                cargo_token = values["HP2_CARGO_PROVENANCE_EXECUTABLE"]
                cargo = Path(cargo_token)
                if not cargo.is_absolute() or not cargo.is_file():
                    raise GuardError(f"Cargo proxy has an invalid configured Cargo executable: {cargo_token}")
                cargo_args = command[proxy_index + 1:]
                if not cargo_args or cargo_args[0] != "rustc":
                    raise GuardError("Cargo capture proxy is not invoked with the selected rustc command")
                matches.append(
                    (cargo, command_dir, cargo_args[1:], prefix, values)
                )
    unique = {
        (str(cargo), str(directory), tuple(args), tuple(prefix), tuple(sorted(values.items())))
        for cargo, directory, args, prefix, values in matches
    }
    if len(unique) != 1:
        raise GuardError(
            f"expected one selected Cargo capture-proxy invocation, found {len(unique)}"
        )
    cargo_raw, directory_raw, args_raw, prefix_raw, values_raw = next(iter(unique))
    return (
        Path(cargo_raw), Path(directory_raw), list(args_raw), list(prefix_raw), dict(values_raw),
    )


def cargo_option(args: list[str], name: str, default: str | None = None) -> str | None:
    prefix = f"{name}="
    for index, token in enumerate(args):
        if token.startswith(prefix):
            return token[len(prefix):]
        if token == name and index + 1 < len(args):
            return args[index + 1]
    return default


def cargo_config_args(args: list[str]) -> list[str]:
    result = []
    index = 0
    while index < len(args) and args[index] != "--":
        token = args[index]
        if token == "--config":
            index += 1
            if index >= len(args) or args[index] == "--" or not args[index]:
                raise GuardError("Cargo --config has no value")
            result.extend(["--config", args[index]])
        elif token.startswith("--config="):
            if not token[len("--config="):]:
                raise GuardError("Cargo --config has no value")
            result.append(token)
        index += 1
    return result


def cargo_build_options(args: list[str], command_dir: Path) -> dict:
    cargo_args = args[:args.index("--")] if "--" in args else args
    features = []
    for index, token in enumerate(cargo_args):
        if token.startswith("--features="):
            features.extend(token.split("=", 1)[1].split(","))
        elif token == "--features" and index + 1 < len(cargo_args):
            features.extend(cargo_args[index + 1].split(","))
    target = cargo_option(cargo_args, "--target")
    target_dir = cargo_option(cargo_args, "--target-dir")
    profile = cargo_option(cargo_args, "--profile")
    if profile is None:
        profile = "release" if "--release" in cargo_args else "debug"
    return {
        "args": cargo_args,
        "target": target,
        "target_dir": resolved(target_dir, command_dir) if target_dir else None,
        "profile": profile,
        "bin": cargo_option(cargo_args, "--bin"),
        "package": cargo_option(cargo_args, "--package"),
        "features": sorted(set(feature for feature in features if feature)),
        "all_features": "--all-features" in cargo_args,
        "no_default_features": "--no-default-features" in cargo_args,
        "manifest": cargo_option(cargo_args, "--manifest-path"),
    }

def cargo_rustflags(prefix: list[str], recorded: dict[str, str]) -> list[str]:
    selected_env = dict(recorded)
    cursor = 0
    while cursor < len(prefix):
        token = prefix[cursor]
        cursor += 1
        if token == "--":
            break
        if token == "--modify" and cursor < len(prefix):
            modification = prefix[cursor]
            cursor += 1
            key = modification.split("=", 1)[0]
            if key in {"RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS"}:
                raise GuardError(f"cannot prove Cargo flags modified by cmake -E env: {modification}")
            continue
        if token.startswith("--modify="):
            modification = token.split("=", 1)[1]
            key = modification.split("=", 1)[0]
            if key in {"RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS"}:
                raise GuardError(f"cannot prove Cargo flags modified by cmake -E env: {modification}")
        elif token == "--unset" and cursor < len(prefix):
            selected_env.pop(prefix[cursor], None)
            cursor += 1
            continue
        elif token.startswith("--unset="):
            selected_env.pop(token.split("=", 1)[1], None)
        elif "=" in token and not token.startswith("-"):
            key, value = token.split("=", 1)
            selected_env[key] = value
    encoded = selected_env.get("CARGO_ENCODED_RUSTFLAGS")
    if encoded:
        return [flag for flag in encoded.split("\x1f") if flag]
    flags = selected_env.get("RUSTFLAGS", "")
    try:
        return shlex.split(flags)
    except ValueError as exc:
        raise GuardError(f"cannot parse selected Cargo RUSTFLAGS: {exc}") from exc


def selected_cargo_binary(options: dict, metadata: dict) -> tuple[dict, dict] | None:
    package_name = options.get("package")
    binary_name = options.get("bin")
    matches = [
        (package, target)
        for package in metadata["packages"]
        if package_name is None or package["name"] == package_name
        for target in package.get("targets", [])
        if target.get("kind") == ["bin"] and (binary_name is None or target["name"] == binary_name)
    ]
    return matches[0] if len(matches) == 1 else None


def expected_cargo_binary(options: dict, metadata: dict, command_dir: Path) -> Path:
    target_dir = options.get("target_dir")
    if target_dir is None:
        target_dir = Path(metadata["target_directory"]).resolve()
    triple = options.get("target")
    profile = options["profile"]
    selected = selected_cargo_binary(options, metadata)
    if selected is None:
        raise GuardError("Cargo invocation does not select exactly one binary target")
    _, target = selected
    parts = [target_dir]
    if triple:
        parts.append(Path(triple))
    parts.extend((Path(profile), Path(target["name"])))
    return Path(*parts).resolve()


def cargo_binary_copy_provenance(text: str, expected: Path, target: Path, build: Path) -> bool:
    if not expected.is_file() or not target.is_file() or not is_executable_image(expected) or not is_executable_image(target):
        return False
    for line in text.splitlines():
        try:
            tokens = shlex.split(line)
        except ValueError:
            continue
        command_dir = build
        if len(tokens) > 1 and tokens[0] == "cd":
            command_dir = resolved(tokens[1], build)
            if "&&" in tokens:
                tokens = tokens[tokens.index("&&") + 1:]
        segments, segment = [], []
        for token in tokens:
            if token in {"&&", ";"}:
                if segment:
                    segments.append(segment)
                segment = []
            else:
                segment.append(token)
        if segment:
            segments.append(segment)
        for command in segments:
            if (
                len(command) != 5
                or Path(command[0]).name != "cmake"
                or command[1:3] not in (["-E", "copy"], ["-E", "copy_if_different"])
            ):
                continue
            source = resolved(command[3], command_dir)
            destination = resolved(command[4], command_dir)
            if source == expected and destination == target.parent and source.name == target.name:
                return source.read_bytes() == target.read_bytes()
    return False
SHA256_RE = re.compile(r"[0-9a-f]{64}\Z")


def file_sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def registry_source_matches_archive(package_root: Path, archive: Path) -> bool:
    expected: dict[Path, str] = {}
    try:
        with tarfile.open(archive, "r:gz") as crate:
            for member in crate.getmembers():
                member_path = Path(member.name)
                if member_path.is_absolute() or ".." in member_path.parts:
                    return False
                if not member_path.parts or member_path.parts[0] != package_root.name:
                    return False
                relative = Path(*member_path.parts[1:])
                if not relative.parts:
                    continue
                if member.isdir():
                    continue
                if not member.isfile() or relative in expected:
                    return False
                stream = crate.extractfile(member)
                if stream is None:
                    return False
                digest = hashlib.sha256()
                with stream:
                    for block in iter(lambda: stream.read(1024 * 1024), b""):
                        digest.update(block)
                expected[relative] = digest.hexdigest()

        observed: dict[Path, str] = {}
        for root, directories, files in os.walk(package_root, followlinks=False):
            root_path = Path(root)
            for name in directories:
                if (root_path / name).is_symlink():
                    return False
            for name in files:
                path = root_path / name
                if path.is_symlink():
                    return False
                relative = path.relative_to(package_root)
                if relative in {Path(".cargo-ok"), Path(".cargo-checksum.json")}:
                    continue
                observed[relative] = file_sha256(path)
        return observed == expected
    except (OSError, tarfile.TarError, EOFError):
        return False


def absolute_record_path(raw: object, label: str) -> Path:
    if not isinstance(raw, str) or not raw:
        raise GuardError(f"{label} is not a nonempty path")
    path = Path(raw)
    if not path.is_absolute():
        raise GuardError(f"{label} is not absolute: {raw}")
    normalized = path.resolve(strict=False)
    if str(normalized) != raw:
        raise GuardError(f"{label} is not normalized: {raw}")
    return normalized


def cargo_reported_path(raw: object, label: str) -> Path:
    if not isinstance(raw, str) or not raw or not Path(raw).is_absolute():
        raise GuardError(f"{label} is not an absolute path: {raw!r}")
    return Path(raw).resolve(strict=False)


PROVENANCE_ENV_KEYS = {
    "AR", "CC", "CFLAGS", "CMAKE_PREFIX_PATH", "CPATH", "CPPFLAGS", "CXX", "HOME",
    "CXXFLAGS", "DYLD_LIBRARY_PATH", "LDFLAGS", "LIBRARY_PATH",
    "MACOSX_DEPLOYMENT_TARGET", "OBJCFLAGS", "OBJCXXFLAGS", "PATH",
    "PKG_CONFIG_LIBDIR", "PKG_CONFIG_PATH", "SDKROOT", "TARGET", "RUSTC",
    "RUSTC_WRAPPER", "RUSTFLAGS", "RUSTDOC", "RUSTUP_HOME", "RUSTUP_TOOLCHAIN", "CARGO_BUILD_TARGET",
    "CARGO_BUILD_RUSTC", "CARGO_BUILD_RUSTC_WRAPPER", "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER",
    "CARGO_HOME", "CARGO_ENCODED_RUSTFLAGS",
    "CARGO_NET_OFFLINE", "CARGO_INCREMENTAL", "BINDGEN_EXTRA_CLANG_ARGS", "CORROSION_BUILD_DIR",
}


def provenance_environment_key(key: str) -> bool:
    return key in PROVENANCE_ENV_KEYS or key.startswith((
        "AR_", "CC_", "CFLAGS_", "CXX_", "CXXFLAGS_", "CARGO_TARGET_", "CARGO_PROFILE_", "RUSTC_",
    ))


def cargo_replay_environment(recorded: dict[str, str]) -> dict[str, str]:
    env = {key: value for key, value in os.environ.items() if not provenance_environment_key(key)}
    env.update(recorded)
    return env


def cargo_provenance_record(
    raw_path: object, product: str, build: Path,
    cargo: Path, command_dir: Path, cargo_args: list[str],
    configured_cargo: str | None = None,
    configured_environment: dict[str, str] | None = None,
) -> dict:
    if not isinstance(raw_path, str) or not raw_path:
        raise GuardError(f"product {product} has no retained Cargo provenance record")
    record_path = resolved(raw_path, build)
    if not beneath(record_path, build / "Testing/HP2"):
        raise GuardError(f"Cargo provenance record is outside Testing/HP2 artifacts: {record_path}")
    record = load_json(record_path)
    if not isinstance(record, dict) or record.get("schema_version") not in {3, 4}:
        raise GuardError(f"invalid Cargo provenance schema: {record_path}")
    configured_mode = (configured_environment or {}).get("HP2_CARGO_PROVENANCE_MODE", "schema3")
    if configured_mode not in {"schema3", "final-bin-v1"} or record["schema_version"] != (4 if configured_mode == "final-bin-v1" else 3):
        raise GuardError(f"Cargo provenance schema differs from configured qualification mode: {record_path}")
    if record.get("capture_error"):
        raise GuardError(f"Cargo provenance capture is incomplete: {record['capture_error']}")
    if record.get("selected_target") != product:
        raise GuardError(
            f"Cargo provenance target differs for {product}: {record.get('selected_target')!r}"
        )
    argv = record.get("cargo_argv")
    if not isinstance(argv, list) or not argv or any(not isinstance(token, str) for token in argv):
        raise GuardError(f"Cargo provenance argv is missing or malformed: {record_path}")
    cargo_token = argv[0]
    if not Path(cargo_token).is_absolute():
        raise GuardError(f"Cargo provenance executable is not absolute: {cargo_token}")
    expected_cargo_token = configured_cargo if configured_cargo is not None else str(cargo)
    if (
        argv[1:] != ["rustc", *cargo_args]
        or cargo_token != expected_cargo_token
        or Path(cargo_token).resolve(strict=False) != cargo.resolve(strict=False)
    ):
        raise GuardError(
            f"retained Cargo argv differs from selected Ninja command for {product}"
        )
    environment = record.get("environment")
    if not isinstance(environment, dict) or any(
        not isinstance(key, str) or not isinstance(value, str)
        for key, value in environment.items()
    ):
        raise GuardError(f"Cargo provenance environment is missing or malformed for {product}")
    if configured_environment is not None:
        proxy_keys = {
            "HP2_CARGO_PROVENANCE_TARGET",
            "HP2_CARGO_PROVENANCE_RECORD",
            "HP2_CARGO_PROVENANCE_EXECUTABLE",
            "HP2_CARGO_PROVENANCE_MODE",
        }
        unclassified = sorted(
            key for key in configured_environment
            if key not in proxy_keys and not provenance_environment_key(key)
        )
        if unclassified:
            raise GuardError(
                f"selected Ninja command has unclassified Cargo environment bindings for "
                f"{product}: {unclassified}"
            )
        selected_environment = {
            key: value for key, value in configured_environment.items()
            if provenance_environment_key(key)
        }
        if any(environment.get(key) != value for key, value in selected_environment.items()):
            raise GuardError(
                f"retained Cargo environment differs from selected Ninja command for {product}"
            )
    recorded_cwd = absolute_record_path(record.get("cwd"), "Cargo provenance cwd")
    if recorded_cwd != command_dir.resolve(strict=False):
        raise GuardError(
            f"retained Cargo cwd differs from selected Ninja command for {product}: "
            f"{recorded_cwd} != {command_dir}"
        )
    if type(record.get("status")) is not int or record["status"] != 0:
        raise GuardError(f"retained Cargo invocation did not succeed for {product}")

    for stream_name, hash_name in (("stdout", "stdout_sha256"), ("stderr", "stderr_sha256")):
        expected_hash = record.get(hash_name)
        stream_path = record_path.parent / f"cargo.{stream_name}"
        if not isinstance(expected_hash, str) or not SHA256_RE.fullmatch(expected_hash):
            raise GuardError(f"Cargo provenance {hash_name} is missing or malformed")
        if not stream_path.is_file() or file_sha256(stream_path) != expected_hash:
            raise GuardError(f"retained Cargo {stream_name} hash does not match: {stream_path}")

    try:
        stdout_events = [
            json.loads(line)
            for line in (record_path.parent / "cargo.stdout").read_text(encoding="utf-8").splitlines()
            if line.strip()
        ]
    except (OSError, UnicodeError, json.JSONDecodeError) as exc:
        raise GuardError(f"retained Cargo stdout is not complete JSON event output: {exc}") from exc
    messages = record.get("messages")
    if not isinstance(messages, list) or messages != stdout_events:
        raise GuardError(f"retained Cargo JSON messages differ from stdout bytes for {product}")
    if any(not isinstance(message, dict) or not isinstance(message.get("reason"), str) for message in messages):
        raise GuardError(f"Cargo provenance contains a malformed message event for {product}")
    finished = [message for message in messages if message.get("reason") == "build-finished"]
    if len(finished) != 1 or finished[0].get("success") is not True or messages[-1] is not finished[0]:
        raise GuardError(f"Cargo provenance has no unique successful final build-finished event for {product}")

    artifact_hashes = record.get("artifact_sha256")
    if not isinstance(artifact_hashes, dict) or any(
        not isinstance(key, str) or not isinstance(value, str) or not SHA256_RE.fullmatch(value)
        for key, value in artifact_hashes.items()
    ):
        raise GuardError(f"Cargo provenance artifact hashes are missing or malformed for {product}")
    for key in artifact_hashes:
        absolute_record_path(key, "Cargo artifact hash key")
    observed_artifacts = set()
    build_script_dirs = set()
    for message in messages:
        if message.get("reason") == "compiler-artifact":
            filenames = message.get("filenames")
            if not isinstance(filenames, list) or not filenames:
                raise GuardError("Cargo compiler-artifact event has no filenames")
            observed_artifacts.update(
                cargo_reported_path(filename, "Cargo compiler-artifact filename")
                for filename in filenames
            )
            executable = message.get("executable")
            if executable:
                observed_artifacts.add(
                    cargo_reported_path(executable, "Cargo compiler-artifact executable")
                )
        elif message.get("reason") == "build-script-executed":
            build_script_dirs.add(
                cargo_reported_path(message.get("out_dir"), "Cargo build-script out_dir")
            )
    if not observed_artifacts:
        raise GuardError(f"Cargo provenance has no compiler-artifact outputs for {product}")
    if set(artifact_hashes) != {str(path) for path in observed_artifacts}:
        missing = sorted(str(path) for path in observed_artifacts - {Path(key) for key in artifact_hashes})
        extra = sorted(key for key in artifact_hashes if key not in {str(path) for path in observed_artifacts})
        raise GuardError(
            f"Cargo compiler-artifact hash inventory differs for {product}; "
            f"missing={missing}, extra={extra}"
        )
    for path in sorted(observed_artifacts):
        expected_hash = artifact_hashes[str(path)]
        if not isinstance(expected_hash, str) or not SHA256_RE.fullmatch(expected_hash):
            raise GuardError(f"Cargo compiler-artifact hash is malformed: {path}")
        if not path.is_file() or file_sha256(path) != expected_hash:
            raise GuardError(f"Cargo compiler-artifact hash does not match: {path}")

    output_hashes = record.get("build_script_output_sha256")
    if not isinstance(output_hashes, dict) or any(
        not isinstance(key, str) or not isinstance(value, str) or not SHA256_RE.fullmatch(value)
        for key, value in output_hashes.items()
    ):
        raise GuardError(f"Cargo build-script output hashes are missing or malformed for {product}")
    for key in output_hashes:
        absolute_record_path(key, "Cargo build-script output hash key")
    observed_outputs = set()
    for out_dir in build_script_dirs:
        if not out_dir.is_dir():
            raise GuardError(f"Cargo build-script output directory is missing: {out_dir}")
        def output_walk_error(error: OSError) -> None:
            raise GuardError(f"cannot enumerate Cargo build-script output: {error}") from error
        for root, directories, files in os.walk(out_dir, followlinks=False, onerror=output_walk_error):
            root_path = Path(root)
            for name in directories:
                candidate = root_path / name
                if candidate.is_symlink():
                    raise GuardError(f"Cargo build-script output contains a directory symlink: {candidate}")
            for name in files:
                candidate = root_path / name
                if candidate.is_symlink():
                    raise GuardError(f"Cargo build-script output contains a file symlink: {candidate}")
                if not candidate.is_file():
                    raise GuardError(f"Cargo build-script output is not a regular file: {candidate}")
                path = candidate.resolve(strict=False)
                if not beneath(path, out_dir):
                    raise GuardError(f"Cargo build-script output escapes its out_dir: {candidate}")
                observed_outputs.add(path)
        output = out_dir.parent / "output"
        if output.is_symlink() or not output.is_file():
            raise GuardError(f"Cargo build-script output file is missing or unsafe: {output}")
        observed_outputs.add(output.resolve(strict=True))
    if set(output_hashes) != {str(path) for path in observed_outputs}:
        missing = sorted(str(path) for path in observed_outputs - {Path(key) for key in output_hashes})
        extra = sorted(key for key in output_hashes if key not in {str(path) for path in observed_outputs})
        raise GuardError(
            f"Cargo build-script output hash inventory differs for {product}; "
            f"missing={missing}, extra={extra}"
        )
    for path in sorted(observed_outputs):
        expected_hash = output_hashes[str(path)]
        if not isinstance(expected_hash, str) or not SHA256_RE.fullmatch(expected_hash):
            raise GuardError(f"Cargo build-script output hash is malformed: {path}")
        if file_sha256(path) != expected_hash:
            raise GuardError(f"Cargo build-script output hash does not match: {path}")
    if record["schema_version"] == 4:
        spec = importlib.util.spec_from_file_location("capture_final_bin", Path(__file__).with_name("capture_final_bin.py"))
        if spec is None or spec.loader is None:
            raise GuardError("cannot load final-binary evidence verifier")
        helper = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(helper)
        try:
            evidence = record["final_bin"]
            selected = [message for message in messages if message.get("reason") == "compiler-artifact"
                        and message.get("target", {}).get("name") == product
                        and "bin" in message.get("target", {}).get("kind", [])]
            if (len(selected) != 1 or selected[0].get("fresh") is not False
                    or evidence["context"]["target"] != product
                    or evidence["context"]["cargo_argv"] != argv
                    or evidence["context"]["cwd"] != str(recorded_cwd)
                    or str(Path(cargo_token).resolve(strict=True)) not in evidence["context"]["files"]
                    or cargo_reported_path(selected[0].get("executable"), "selected binary")
                    != Path(evidence["executable"]).resolve(strict=True)):
                raise helper.Unsupported("selected Cargo binary event does not match final-link evidence")
            record["_final_bin_objects"] = helper.verify(evidence, record_path.parent,
                                                         cargo_replay_environment(environment))
        except (KeyError, OSError, TypeError, ValueError, helper.Unsupported) as exc:
            raise GuardError(f"invalid final-binary qualification evidence: {exc}") from exc
    return record


def locked_package_provenance(
    package: dict, metadata: dict, allow: dict, repo: Path,
) -> str | None:
    package_source = package.get("source")
    manifest = Path(package.get("manifest_path", "")).resolve(strict=False)
    workspace_root = Path(metadata.get("workspace_root", "")).resolve(strict=False)
    lock_path = workspace_root / "Cargo.lock"
    try:
        lock_packages = parse_cargo_lock(lock_path.read_text())
    except OSError:
        return None
    lock_matches = [
        locked for locked in lock_packages
        if locked.get("name") == package.get("name")
        and locked.get("version") == package.get("version")
        and locked.get("source") == package_source
    ]
    if len(lock_matches) != 1:
        return None
    locked = lock_matches[0]
    if package_source is None:
        if not beneath(manifest, workspace_root) or not manifest.is_file():
            return None
        return "workspace"
    if package_source.startswith("registry+"):
        checksum = locked.get("checksum")
        if not isinstance(checksum, str) or not SHA256_RE.fullmatch(checksum):
            return None
        if package.get("checksum") is not None and package["checksum"] != checksum:
            return None
        cargo_root = cargo_registry(allow, repo)
        if cargo_root is None or not beneath(manifest, cargo_root):
            return None
        package_root = manifest.parent
        if package_root.name != f"{package['name']}-{package['version']}":
            return None
        # Current Cargo registry unpack trees no longer consistently contain
        # .cargo-checksum.json. Bind the selected package to the exact locked
        # crate archive instead; accepting a name/version directory alone
        # would let an arbitrary directory impersonate a registry package.
        cache_roots = sorted((cargo_root.parent / "cache").glob("*/"))
        matching_archives = []
        for cache_root in cache_roots:
            archive = cache_root / f"{package['name']}-{package['version']}.crate"
            if archive.is_file() and file_sha256(archive) == checksum:
                matching_archives.append(archive.resolve())
        if len(matching_archives) != 1:
            return None
        if not registry_source_matches_archive(package_root, matching_archives[0]):
            return None
        return f"registry:{checksum}"


def locked_external(path: Path, repo: Path, allow: dict) -> str | None:
    # Source checkouts use pinned_generated_source() so dirty files cannot inherit
    # provenance from their enclosing directory.
    cargo_root = cargo_registry(allow, repo)
    lock = Path(allow.get("cargo_lock", repo / "Cargo.lock"))
    if not (lock.is_file() and cargo_root and beneath(path, cargo_root)):
        return None
    packages = {
        f"{pkg.get('name')}-{pkg.get('version')}": pkg
        for pkg in parse_cargo_lock(lock.read_text())
        if pkg.get("source", "").startswith("registry+") and pkg.get("checksum")
    }
    for package_root in path.parents:
        if package_root == cargo_root:
            break
        package = packages.get(package_root.name)
        if not package:
            continue
        checksum_path = package_root / ".cargo-checksum.json"
        try:
            checksum = json.loads(checksum_path.read_text())
            relative = path.relative_to(package_root).as_posix()
            expected = checksum.get("files", {}).get(relative)
            actual = hashlib.sha256(path.read_bytes()).hexdigest()
        except (OSError, ValueError):
            return None
        if checksum.get("package") == package["checksum"] and expected == actual:
            return f"locked-cargo:{package_root.name}"
    return None
def cmake_cache_values(build: Path) -> dict[str, str]:
    cache = build / "CMakeCache.txt"
    values = {}
    try:
        for line in cache.read_text(errors="replace").splitlines():
            if line and not line.startswith(("//", "#")) and ":" in line and "=" in line:
                key, value = line.split("=", 1)
                name, _, _kind = key.partition(":")
                values[name] = value
    except OSError:
        pass
    return values


@functools.lru_cache(maxsize=16)
def clang_resource_roots(build: str) -> tuple[Path, ...]:
    values = cmake_cache_values(Path(build))
    roots = set()
    for key in ("CMAKE_C_COMPILER", "CMAKE_CXX_COMPILER"):
        compiler = values.get(key)
        if not compiler:
            continue
        try:
            proc = subprocess.run(
                [compiler, "-print-resource-dir"],
                text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            )
        except OSError:
            continue
        if proc.returncode == 0 and proc.stdout.strip():
            root = Path(proc.stdout.strip()).resolve(strict=False)
            if (root / "include").is_dir():
                roots.add(root)
    return tuple(sorted(roots))


def clang_resource_header(path: Path, build: Path) -> bool:
    path = path.resolve(strict=False)
    return any(beneath(path, root / "include") for root in clang_resource_roots(str(build.resolve())))


def apple_sdk_root(build: Path, allow: dict) -> Path | None:
    cache_sysroot = cmake_cache_values(build).get("CMAKE_OSX_SYSROOT")
    configured = os.environ.get("SDKROOT")
    approved = []
    for raw in allow.get("sdk_roots", []):
        root = Path(os.path.expandvars(raw)).expanduser().resolve(strict=False)
        if (root / "System/Library/Frameworks").is_dir() and (root / "usr/lib").is_dir():
            approved.append(root)
    sdk = None
    if cache_sysroot:
        candidate = Path(os.path.expandvars(cache_sysroot)).expanduser().resolve(strict=False)
        if (
            (candidate / "System/Library/Frameworks").is_dir()
            and (candidate / "usr/lib").is_dir()
        ):
            sdk = candidate
    if sdk is None:
        try:
            result = subprocess.run(
                ["xcrun", "--sdk", "macosx", "--show-sdk-path"],
                text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            )
        except OSError:
            result = None
        candidate = (
            Path(result.stdout.strip()).resolve(strict=False)
            if result is not None and result.returncode == 0 and result.stdout.strip()
            else None
        )
        if candidate is not None and (
            (candidate / "System/Library/Frameworks").is_dir()
            and (candidate / "usr/lib").is_dir()
        ):
            sdk = candidate
    if configured:
        root = Path(os.path.expandvars(configured)).expanduser().resolve(strict=False)
        if root == sdk or root in approved:
            return root
        return None
    return sdk or (approved[0] if approved else None)


def sdk_framework(name: str, sdk: Path | None) -> bool:
    return bool(sdk and (sdk / "System/Library/Frameworks" / f"{name}.framework").is_dir())




def pinned_generated_source(path: Path, build: Path, repo: Path, allow: dict) -> bool:
    entries = {item.get("name"): item for item in load_json(repo / "ThirdParty/sources.json").get("sources", [])}
    for root_entry in allow.get("pinned_source_roots", []):
        item = entries.get(root_entry.get("source"))
        root = resolved(root_entry["root"], build)
        if (
            not item
            or item.get("source_type") != "git"
            or not item.get("commit")
            or not beneath(path, root)
        ):
            continue
        relative = path.relative_to(root).as_posix()
        try:
            head = subprocess.run(
                ["git", "-C", str(root), "rev-parse", "HEAD"],
                text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            )
            if head.returncode or head.stdout.strip() != item["commit"]:
                continue
            blob = subprocess.run(
                ["git", "-C", str(root), "rev-parse", f"{item['commit']}:{relative}"],
                text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            )
            actual = subprocess.run(
                ["git", "-C", str(root), "hash-object", str(path)],
                text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            )
        except OSError:
            continue
        if blob.returncode == 0 and actual.returncode == 0 and blob.stdout.strip() == actual.stdout.strip():
            return True
    return False


def parse_cargo_lock(text: str) -> list[dict]:
    packages, current = [], None
    for line in text.splitlines():
        if line == "[[package]]":
            if current is not None:
                packages.append(current)
            current = {}
        elif current is not None and " = " in line:
            key, value = line.split(" = ", 1)
            if key in {"name", "version", "source", "checksum"}:
                current[key] = value.strip().strip('"')
    if current is not None:
        packages.append(current)
    return packages

def file_api_closure(build: Path, requested: str, seeds: list[str]) -> tuple[list[str], set[Path], set[Path], set[str]]:
    reply = build / ".cmake/api/v1/reply"
    indexes = sorted(reply.glob("index-*.json"))
    if not indexes:
        raise GuardError("missing CMake File API codemodel reply")
    index = load_json(indexes[-1])
    entry = next((item for item in index.get("reply", {}).values() if item.get("kind") == "codemodel"), None)
    if not entry:
        raise GuardError("File API reply has no codemodel")
    codemodel = load_json(reply / entry["jsonFile"])
    configs = codemodel.get("configurations", [])
    if not configs:
        raise GuardError("File API codemodel has no configurations")
    records, by_name = {}, {}
    for record in configs[0].get("targets", []):
        details = load_json(reply / record["jsonFile"])
        records[record["id"]] = (record, details)
        by_name.setdefault(record["name"], []).append(record["id"])
    roots = by_name.get(requested, [])
    if not roots:
        roots = by_name.get(f"_cargo-build_{requested}", []) or by_name.get(f"cargo-build_{requested}", [])
    for seed in seeds:
        roots.extend(by_name.get(seed, []))
    if not roots:
        return [f"File API codemodel has no product/Cargo/link target for {requested}"], set(), set(), set()
    source_root = resolved(codemodel.get("paths", {}).get("source", "."), build)
    build_root = resolved(codemodel.get("paths", {}).get("build", "."), build)
    reachable, pending_targets = set(), list(roots)
    while pending_targets:
        target_id = pending_targets.pop()
        if target_id in reachable:
            continue
        pair = records.get(target_id)
        if not pair:
            raise GuardError(f"File API dependency target missing: {target_id}")
        reachable.add(target_id)
        pending_targets.extend(
            dep["id"] for dep in pair[1].get("dependencies", []) if dep.get("id")
        )
    object_library_artifact_owners = {}
    object_library_producer_sources = set()
    for target_id, (target, details) in records.items():
        if details.get("type") != "OBJECT_LIBRARY":
            continue
        if any(
            Path(source.get("path", "")).suffix in NATIVE_COMPILE_SUFFIXES
            for source in details.get("sources", [])
            if source.get("path")
        ):
            object_library_producer_sources.add(target_id)
        for artifact in details.get("artifacts", []):
            raw_artifact = artifact.get("path")
            if raw_artifact:
                path = resolved(raw_artifact, build_root)
                object_library_artifact_owners.setdefault(path, set()).add(target_id)
    visited, pending, sources, artifacts, target_names = set(), list(roots), set(), set(), set()
    # File API source paths are rooted at the codemodel source tree, while
    # artifacts are rooted at its build tree. Some CMake integrations emit
    # a build-tree-relative source path (including the build directory name);
    # resolving that spelling against build_root would duplicate the prefix.
    while pending:
        target_id = pending.pop()
        if target_id in visited:
            continue
        visited.add(target_id)
        pair = records.get(target_id)
        if not pair:
            raise GuardError(f"File API dependency target missing: {target_id}")
        target, details = pair
        target_names.add(target["name"])
        for source in details.get("sources", []):
            raw_path = source.get("path")
            if not raw_path:
                continue
            raw = Path(raw_path)
            source_path = resolved(raw, source_root)
            build_path = resolved(raw, build_root)
            if (
                raw.suffix == ".o"
                and source.get("isGenerated")
                and source_path in object_library_artifact_owners
                and len(object_library_artifact_owners[source_path]) == 1
                and next(iter(object_library_artifact_owners[source_path])) in reachable
                and next(iter(object_library_artifact_owners[source_path])) in object_library_producer_sources
            ):
                # CMake can repeat an OBJECT_LIBRARY's object outputs as
                # generated sources on consuming targets. The producer target
                # is separately traversed through File API dependencies; its
                # own source records and artifacts retain the producer/source
                # relationship. Never treat the object as a C++ source.
                continue
            if source.get("isGenerated"):
                path = build_path if build_path.exists() else source_path
            else:
                path = source_path if source_path.exists() or not build_path.exists() else build_path
            if not (is_generated_rule_label(path) and not path.exists()):
                sources.add(path)
        for artifact in details.get("artifacts", []):
            if artifact.get("path"):
                artifacts.add(resolved(artifact["path"], build_root))
        pending.extend(dep["id"] for dep in details.get("dependencies", []) if dep.get("id"))
    return [], sources, artifacts, target_names


def generated_native_header(
    path: Path, build: Path, repo: Path, allow: dict, records: list[dict], targets: set[str]
) -> bool:
    path = path.resolve(strict=False)
    for spec in allow.get("generated_native_headers", []):
        source_root = next(
            (
                resolved(item["root"], build)
                for item in allow.get("pinned_source_roots", [])
                if item.get("source") == spec.get("source")
            ),
            None,
        )
        if source_root is None:
            continue
        if spec.get("mode") == "copy":
            output_root = resolved(spec.get("path_prefix", ""), build)
            if not beneath(path, output_root):
                continue
            if path.name.startswith(("SDL_config", "SDL_revision")):
                continue
            relative = path.relative_to(output_root)
            input_path = source_root / spec.get("input_prefix", "") / relative
            cmake_source = source_root / spec.get("producer_cmake", "")
            try:
                same_bytes = hashlib.sha256(path.read_bytes()).digest() == hashlib.sha256(input_path.read_bytes()).digest()
            except OSError:
                continue
            if (
                spec.get("target") in targets
                and path.is_file()
                and input_path.is_file()
                and same_bytes
                and pinned_generated_source(input_path, build, repo, allow)
                and pinned_generated_source(cmake_source, build, repo, allow)
            ):
                return True
            continue
        expected_path = resolved(spec.get("path", ""), build)
        template_path = resolved(spec.get("template_path", ""), build)
        if path != expected_path:
            continue
        expected = {
            "target": spec.get("target"),
            "path": spec.get("path"),
            "source_dir": str(source_root),
            "cmake_source": str(source_root / spec.get("producer_cmake", "")),
            "input_source": str(source_root / spec.get("input", "")),
        }
        for record in records:
            if (
                not isinstance(record, dict)
                or any(record.get(key) != value for key, value in expected.items())
                or not isinstance(record.get("template_path"), str)
                or resolved(record["template_path"], build) != template_path
            ):
                continue
            if expected["target"] not in targets or not path.is_file():
                continue
            cmake_source = Path(expected["cmake_source"])
            input_source = Path(expected["input_source"])
            try:
                cmake_hash = hashlib.sha256(cmake_source.read_bytes()).hexdigest()
                input_hash = hashlib.sha256(input_source.read_bytes()).hexdigest()
                template_hash = hashlib.sha256(template_path.read_bytes()).hexdigest()
                output_hash = hashlib.sha256(path.read_bytes()).hexdigest()
            except OSError:
                continue
            template_proven = (
                (
                    spec.get("template_sha256") is not None
                    and template_hash == spec["template_sha256"]
                )
                or (
                    beneath(template_path, source_root)
                    and pinned_generated_source(template_path, build, repo, allow)
                )
            )
            if (
                record.get("cmake_sha256") == cmake_hash
                and record.get("input_sha256") == input_hash
                and record.get("template_sha256") == template_hash
                and template_proven
                and record.get("sha256") == output_hash
                and pinned_generated_source(cmake_source, build, repo, allow)
                and pinned_generated_source(input_source, build, repo, allow)
            ):
                return True
    return False


def check_cargo_metadata(
    manifest_path: Path, cargo: Path, options: dict, environment: dict, command_dir: Path,
) -> tuple[list[str], dict]:
    manifest_path = manifest_path.resolve()
    command = [
        str(cargo), *cargo_config_args(options.get("args", [])), "metadata", "--locked", "--format-version", "1",
        "--manifest-path", str(manifest_path),
    ]
    if options["all_features"]:
        command.append("--all-features")
    if options["features"]:
        command.extend(["--features", ",".join(options["features"])])
    if options["no_default_features"]:
        command.append("--no-default-features")
    if options["target"]:
        command.extend(["--filter-platform", options["target"]])
    try:
        result = subprocess.run(
            command, cwd=command_dir, env=cargo_replay_environment(environment), text=True,
            stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        )
    except OSError as exc:
        raise GuardError(f"cannot run configured Cargo {cargo}: {exc}") from exc
    if result.returncode:
        return [f"cargo metadata failed for {manifest_path}: {result.stderr.strip()}"], {}
    try:
        data = json.loads(result.stdout)
    except json.JSONDecodeError as exc:
        return [f"cargo metadata returned invalid JSON: {exc}"], {}
    lock_path = Path(data.get("workspace_root", manifest_path.parent)) / "Cargo.lock"
    if not lock_path.is_file() or not data.get("packages") or not data.get("resolve"):
        return [f"Cargo.lock or resolved Cargo metadata missing for {manifest_path}"], {}
    return [], data

def cargo_unit_graph(
    manifest_path: Path, cargo: Path, options: dict, metadata: dict,
    environment: dict, command_dir: Path,
) -> tuple[list[str], dict]:
    selected = selected_cargo_binary(options, metadata)
    if selected is None:
        return ["Cargo unit graph has no unique selected binary target"], {}
    if (
        not options.get("manifest")
        or resolved(options["manifest"], command_dir) != manifest_path.resolve()
    ):
        return ["Cargo unit graph manifest differs from selected invocation"], {}
    # Cargo metadata merges package feature unions; ask the same Cargo executable
    # for its selected unit graph, using the captured invocation args and environment.
    raw_args = options.get("args", [])
    command = [
        str(cargo), "rustc", "-Z", "unstable-options", "--unit-graph",
    ]
    index = 0
    while index < len(raw_args):
        token = raw_args[index]
        if token == "--message-format" and index + 1 < len(raw_args):
            index += 2
        elif token.startswith("--message-format="):
            index += 1
        else:
            command.append(token)
            index += 1
    if "--locked" not in raw_args and not any(arg.startswith("--locked=") for arg in raw_args):
        command.append("--locked")
    child_environment = cargo_replay_environment(environment)
    child_environment["RUSTC_BOOTSTRAP"] = "1"
    try:
        result = subprocess.run(
            command, cwd=command_dir, env=child_environment,
            text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        )
    except OSError as exc:
        return [f"cannot query selected Cargo unit graph with {cargo}: {exc}"], {}
    if result.returncode:
        return [
            f"selected Cargo unit graph query failed: {result.stderr.strip()}"
        ], {}
    try:
        graph = json.loads(result.stdout)
    except json.JSONDecodeError as exc:
        return [f"Cargo unit graph returned invalid JSON: {exc}"], {}
    if (
        not isinstance(graph, dict)
        or graph.get("version") != 1
        or not isinstance(graph.get("units"), list)
    ):
        return ["Cargo unit graph has an unsupported or missing schema"], {}
    return [], graph


def cargo_unit_graph_closure(
    product: str, metadata: dict, options: dict, graph: dict,
) -> tuple[list[str], set[str], dict[str, set[str]]]:
    packages = {package["id"]: package for package in metadata["packages"]}
    selected = selected_cargo_binary(options, metadata)
    if selected is None:
        return ["Cargo unit graph has no unique selected binary target"], set(), {}
    root_package, root_target = selected
    if options.get("bin") is None and root_target["name"] != product:
        return [
            f"Cargo binary target {root_target['name']} does not match product {product}"
        ], set(), {}
    units = graph.get("units")
    if not isinstance(units, list) or not units:
        return ["Cargo unit graph has no selected units"], set(), {}
    failures = []
    for index, unit in enumerate(units):
        if not isinstance(unit, dict) or unit.get("pkg_id") not in packages:
            failures.append(f"Cargo unit graph contains unknown package at unit {index}")
            continue
        target = unit.get("target")
        if not isinstance(target, dict) or len([
            candidate for candidate in packages[unit["pkg_id"]].get("targets", [])
            if cargo_target_matches(target, candidate)
        ]) != 1:
            failures.append(f"Cargo unit graph target does not match metadata at unit {index}")
        features = unit.get("features")
        if (
            not isinstance(features, list)
            or any(not isinstance(feature, str) for feature in features)
            or len(features) != len(set(features))
            or features != sorted(features)
        ):
            failures.append(f"Cargo unit graph features are malformed at unit {index}")
        if unit.get("mode") not in {"build", "run-custom-build"}:
            failures.append(f"Cargo unit graph contains an unselected mode at unit {index}")
        profile = unit.get("profile")
        if not isinstance(profile, dict) or profile.get("name") != options.get("profile"):
            failures.append(f"Cargo unit graph profile differs from selected invocation at unit {index}")
        dependencies = unit.get("dependencies")
        if not isinstance(dependencies, list) or any(
            not isinstance(dependency, dict)
            or type(dependency.get("index")) is not int
            or not 0 <= dependency["index"] < len(units)
            for dependency in dependencies
        ):
            failures.append(f"Cargo unit graph dependencies are malformed at unit {index}")

    def unit_role(unit: dict) -> str | None:
        platform = unit.get("platform")
        if options.get("target"):
            if platform == options["target"]:
                return "target"
            if platform is None:
                return "host"
            return None
        return "host"

    roots = [
        index for index, unit in enumerate(units)
        if isinstance(unit, dict)
        and unit.get("pkg_id") == root_package["id"]
        and unit.get("mode") == "build"
        and cargo_target_matches(unit.get("target", {}), root_target)
        and unit_role(unit) == ("target" if options.get("target") else "host")
    ]
    if len(roots) != 1:
        return failures + [
            f"Cargo unit graph has no unique selected binary unit for {product}"
        ], set(), {}
    reachable, pending = set(), list(roots)
    while pending:
        index = pending.pop()
        if index in reachable or index >= len(units):
            continue
        reachable.add(index)
        unit = units[index]
        if not isinstance(unit, dict):
            continue
        for dependency in unit.get("dependencies", []):
            if isinstance(dependency, dict) and type(dependency.get("index")) is int:
                pending.append(dependency["index"])
    if reachable != set(range(len(units))):
        failures.append(
            f"Cargo unit graph contains unreachable units: "
            f"{sorted(set(range(len(units))) - reachable)}"
        )
    roles: dict[str, set[str]] = {}
    for index in sorted(reachable):
        unit = units[index]
        if not isinstance(unit, dict) or unit.get("pkg_id") not in packages:
            continue
        role = unit_role(unit)
        if role is None:
            failures.append(f"Cargo unit graph has an unsupported platform at unit {index}")
            continue
        package_id = unit["pkg_id"]
        roles.setdefault(package_id, set()).add(role)
        if FORBIDDEN.search(packages[package_id]["name"]):
            failures.append(
                f"forbidden Cargo dependency in {product}: "
                f"{packages[package_id]['name']} {packages[package_id]['version']}"
            )
    return failures, set(roles), roles


def cargo_closure_findings(
    product: str, metadata: dict, options: dict
) -> tuple[list[str], set[str], dict[str, set[str]]]:
    graph = options.get("unit_graph")
    if not isinstance(graph, dict):
        return ["selected Cargo unit graph is missing"], set(), {}
    return cargo_unit_graph_closure(product, metadata, options, graph)


def cargo_target_matches(event_target: dict, package_target: dict) -> bool:
    for key in ("name", "kind", "crate_types", "src_path", "edition", "doc", "doctest", "test"):
        if key in package_target and event_target.get(key) != package_target.get(key):
            return False
    return True


def cargo_output_role(
    path: Path, target_dir: Path, options: dict, package_target: dict,
) -> str | None:
    profile_root = target_dir / options["profile"]
    target = options.get("target")
    target_profile = target_dir / target / options["profile"] if target else None
    if package_target.get("kind") == ["custom-build"]:
        return "host" if beneath(path, profile_root) else None
    if "proc-macro" in package_target.get("kind", []):
        return "host" if beneath(path, profile_root) else None
    if target_profile is not None and beneath(path, target_profile):
        return "target"
    if beneath(path, profile_root):
        return "host"
    return None


def rust_objects_for_closure(
    metadata: dict, package_ids: set[str], package_roles: dict[str, set[str]],
    options: dict, command_dir: Path, record: dict, allow: dict, repo: Path,
) -> tuple[list[Path], list[Path], list[str]]:
    packages = {pkg["id"]: pkg for pkg in metadata["packages"]}
    nodes = {node["id"]: node for node in metadata["resolve"]["nodes"]}
    selected = selected_cargo_binary(options, metadata)
    if selected is None:
        return [], [], ["Cargo invocation does not select exactly one binary target"]
    selected_package, selected_target = selected
    selected_package_id, selected_bin_name = selected_package["id"], selected_target["name"]
    target_dir = options.get("target_dir") or Path(metadata["target_directory"]).resolve()
    root_package_ids = {selected_package_id}
    output_package_ids = root_package_ids | {
        package_id for package_id in package_ids
        if any(
            target.get("kind") in (["lib"], ["rlib"], ["dylib"], ["proc-macro"])
            for target in packages[package_id].get("targets", [])
        )
    }
    failures = []
    outputs, symbol_outputs = set(), set()
    artifact_messages = [
        message for message in record["messages"]
        if message.get("reason") == "compiler-artifact"
    ]
    observed_targets: set[tuple[str, str, tuple[str, ...], str]] = set()
    selected_executables = []
    for event in artifact_messages:
        package_id = event.get("package_id")
        package = packages.get(package_id)
        if package is None or package_id not in package_ids:
            failures.append(f"Cargo compiler-artifact belongs to an unselected package: {package_id!r}")
            continue
        provenance = locked_package_provenance(package, metadata, allow, repo)
        if provenance is None:
            failures.append(
                f"Cargo compiler-artifact package has no lock/source provenance: "
                f"{package['name']} {package['version']} ({package.get('source')})"
            )
        event_target = event.get("target")
        if not isinstance(event_target, dict):
            failures.append(f"Cargo compiler-artifact target is missing for {package['name']}")
            continue
        target_matches = [
            target for target in package.get("targets", [])
            if cargo_target_matches(event_target, target)
        ]
        if len(target_matches) != 1:
            failures.append(
                f"Cargo compiler-artifact target does not uniquely match metadata for "
                f"{package['name']}: {event_target.get('name')!r}"
            )
            continue
        package_target = target_matches[0]
        if package_id not in output_package_ids:
            if package_target.get("kind") != ["custom-build"]:
                failures.append(
                    f"unexpected Cargo compiler-artifact target in selected closure: "
                    f"{package['name']}:{package_target['name']}"
                )
        if not isinstance(event.get("fresh"), bool):
            failures.append(f"Cargo compiler-artifact freshness is missing for {package['name']}")
        if not isinstance(event.get("profile"), dict) or event["profile"].get("test") is not False:
            failures.append(f"Cargo compiler-artifact profile is not a non-test product profile: {package['name']}")
        filenames = event.get("filenames")
        if not isinstance(filenames, list) or not filenames:
            failures.append(f"Cargo compiler-artifact has no output filenames: {package['name']}")
            continue
        event_paths = [
            cargo_reported_path(filename, "Cargo compiler-artifact filename")
            for filename in filenames
        ]
        executable = event.get("executable")
        if executable:
            executable_path = cargo_reported_path(executable, "Cargo compiler-artifact executable")
            if executable_path not in event_paths:
                failures.append(
                    f"Cargo executable is not listed in its compiler-artifact record: {executable_path}"
                )
            if (
                package_id == selected_package_id
                and package_target.get("kind") == ["bin"]
                and package_target["name"] == selected_bin_name
            ):
                selected_executables.append(executable_path)
        roles = {
            cargo_output_role(path, target_dir, options, package_target)
            for path in event_paths
        }
        roles.discard(None)
        if len(roles) != 1:
            failures.append(
                f"Cargo compiler-artifact output has ambiguous host/target role for "
                f"{package['name']}:{package_target['name']}: {[str(path) for path in event_paths]}"
            )
            continue
        role = next(iter(roles))
        features = event.get("features")
        graph = options.get("unit_graph", {})
        matching_units = [
            unit for unit in graph.get("units", [])
            if isinstance(unit, dict)
            and unit.get("pkg_id") == package_id
            and unit.get("mode") == "build"
            and cargo_target_matches(unit.get("target", {}), package_target)
            and (
                (unit.get("platform") == options.get("target"))
                if role == "target" and options.get("target")
                else (
                    unit.get("platform") is None
                    if options.get("target")
                    else True
                )
            )
        ]
        if (
            not isinstance(features, list)
            or any(not isinstance(feature, str) for feature in features)
            or len(features) != len(set(features))
            or features != sorted(features)
            or len(matching_units) != 1
            or features != matching_units[0].get("features")
        ):
            failures.append(
                f"Cargo compiler-artifact features differ from selected unit graph for "
                f"{package['name']}:{package_target['name']}:{role}: "
                f"{features!r} != "
                f"{[unit.get('features') for unit in matching_units]!r}"
            )
        if role not in package_roles.get(package_id, set()) and not (
            package_target.get("kind") == ["custom-build"]
            and bool(package_roles.get(package_id))
            and role == "host"
        ):
            failures.append(
                f"Cargo compiler-artifact role is outside selected closure for "
                f"{package['name']}:{package_target['name']}: {role}"
            )
        unit_key = (package_id, package_target["name"], tuple(package_target.get("kind", [])), role)
        if unit_key in observed_targets:
            failures.append(
                f"duplicate Cargo compiler-artifact unit for "
                f"{package['name']}:{package_target['name']}:{role}"
            )
        observed_targets.add(unit_key)
        outputs.update(event_paths)
        is_first_party = package.get("source") is None and beneath(
            Path(package["manifest_path"]).resolve().parent, Path(metadata["workspace_root"]).resolve()
        )
        if is_first_party and package_target.get("kind") in (["lib"], ["rlib"], ["dylib"], ["proc-macro"]):
            symbol_outputs.update(
                path for path in event_paths if path.suffix.lower() == ".rlib"
            )

    expected_targets = set()
    for unit in options.get("unit_graph", {}).get("units", []):
        if not isinstance(unit, dict) or unit.get("mode") != "build":
            continue
        package_id = unit.get("pkg_id")
        if package_id not in package_ids or not isinstance(unit.get("target"), dict):
            continue
        role = (
            "target"
            if options.get("target") and unit.get("platform") == options["target"]
            else "host"
            if unit.get("platform") is None or not options.get("target")
            else None
        )
        if role is None:
            failures.append(
                f"Cargo unit graph has unsupported compiler platform for "
                f"{packages[package_id]['name']}:{unit['target'].get('name')}"
            )
            continue
        expected_targets.add((package_id, unit["target"]["name"], tuple(unit["target"].get("kind", [])), role))
    missing_targets = expected_targets - observed_targets
    if missing_targets:
        failures.append(
            "selected Cargo compiler-artifact records missing for targets/roles: "
            + ", ".join(
                f"{packages[package_id]['name']} ({package_id}):{target_name}:{target_kind!r}:{role}"
                for package_id, target_name, target_kind, role in sorted(missing_targets)
            )
        )
    expected = expected_cargo_binary(options, metadata, command_dir)
    if not expected.is_file():
        failures.append(f"selected Cargo binary artifact is missing: {expected}")
    if selected_executables != [expected]:
        failures.append(
            f"selected Cargo binary has no unique compiler-artifact executable record: "
            f"{[str(path) for path in selected_executables]} != {expected}"
        )
    outputs.add(expected)
    if not symbol_outputs:
        failures.append("no first-party Rust compiler-artifact archives available for ABI inspection")
    return sorted(outputs), sorted(symbol_outputs), failures
def get_manifest(build: Path) -> dict:
    path = build / "hp2-native-free-manifest.json"
    if not path.is_file():
        raise GuardError(f"missing CMake product manifest: {path}")
    data = load_json(path)
    if data.get("schema_version") != 1 or not isinstance(data.get("products"), dict):
        raise GuardError("invalid native-free manifest schema")
    return data



def make_depfile_record(path: Path) -> tuple[list[Path], list[Path]]:
    text = path.read_text(errors="replace").replace("\\\r\n", " ").replace("\\\n", " ")
    text = text.splitlines()[0] if text.splitlines() else ""
    escaped = False
    separator = None
    for index, char in enumerate(text):
        if escaped:
            escaped = False
        elif char == "\\":
            escaped = True
        elif char == ":":
            separator = index
            break
    if separator is None:
        raise GuardError(f"invalid compiler dependency file (missing target separator): {path}")

    def parse_values(body: str) -> list[Path]:
        values, current = [], []
        index = 0
        while index < len(body):
            char = body[index]
            if char == "\\" and index + 1 < len(body):
                index += 1
                current.append(body[index])
            elif char.isspace():
                if current:
                    values.append("".join(current))
                    current.clear()
            else:
                current.append(char)
            index += 1
        if current:
            values.append("".join(current))
        return [Path(value) for value in values]

    return parse_values(text[:separator]), parse_values(text[separator + 1:])


def make_depfile_inputs(path: Path) -> list[Path]:
    return make_depfile_record(path)[1]


def native_input_proven(
    path: Path, build: Path, repo: Path, allow: dict, item: dict,
    targets: set[str], sdk: Path | None = None,
) -> bool:
    return bool(
        clang_resource_header(path, build)
        or (sdk and beneath(path.resolve(), sdk))
        or locked_external(path, repo, allow)
        or pinned_generated_source(path, build, repo, allow)
        or generated_native_header(path, build, repo, allow, item.get("generated_headers", []), targets)
    )


def normalized_compile_tokens(tokens: list[str]) -> list[str]:
    normalized, index = [], 0
    depfile_options = {"-MF", "-MT", "-MQ"}
    while index < len(tokens):
        token = tokens[index]
        index += 1
        if token in {"-MD", "-MMD", "-MP", "-M"}:
            continue
        if token in depfile_options:
            index += 1
            continue
        normalized.append(token)
    return normalized


def compile_command_details(tokens: list[str], directory: Path) -> tuple[Path | None, Path | None]:
    source = None
    output = None
    for index, token in enumerate(tokens):
        if token == "-c" and index + 1 < len(tokens):
            source = resolved(tokens[index + 1], directory)
        elif token == "-o" and index + 1 < len(tokens):
            output = resolved(tokens[index + 1], directory)
        elif token.startswith("-o") and len(token) > 2:
            output = resolved(token[2:], directory)
    return source, output


def selected_ninja_compile_commands(text: str, build: Path) -> set[tuple[str, ...]]:
    commands = set()
    for line in text.splitlines():
        try:
            tokens = shlex.split(line)
        except ValueError as exc:
            raise GuardError(f"cannot parse selected Ninja command: {exc}") from exc
        if len(tokens) > 1 and tokens[0] == "cd":
            if "&&" not in tokens:
                continue
            tokens = tokens[tokens.index("&&") + 1:]
        segments, segment = [], []
        for token in tokens:
            if token in {"&&", ";"}:
                if segment:
                    segments.append(segment)
                segment = []
            else:
                segment.append(token)
        if segment:
            segments.append(segment)
        for segment in segments:
            if (
                len(segment) > 3 and Path(segment[0]).name == "cmake"
                and segment[1:3] == ["-E", "env"]
            ):
                index = 3
                while index < len(segment) and "=" in segment[index] and not segment[index].startswith("-"):
                    index += 1
                segment = segment[index:]
            if any(token == "-c" for token in segment) and any(
                token == "-o" or (token.startswith("-o") and len(token) > 2)
                for token in segment
            ):
                commands.add(tuple(normalized_compile_tokens(segment)))
    return commands


def ninja_deps_inputs(build: Path, object_path: Path) -> tuple[list[Path], str]:
    if not beneath(object_path, build):
        raise GuardError(f"selected compiler object is outside Ninja build directory: {object_path}")
    relative = object_path.relative_to(build).as_posix()
    try:
        result = subprocess.run(
            ["ninja", "-C", str(build), "-t", "deps", relative],
            cwd=build, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        )
    except OSError as exc:
        raise GuardError(f"cannot query retained Ninja dependencies for {object_path}: {exc}") from exc
    if result.returncode:
        raise GuardError(
            f"Ninja has no retained dependency record for {object_path}: {result.stderr.strip()}"
        )
    lines = result.stdout.splitlines()
    if not lines:
        raise GuardError(f"Ninja returned an empty dependency record for {object_path}")
    match = re.fullmatch(r"(.+): #deps (\d+), deps mtime \d+(?: \(([^)]*)\))?", lines[0].strip())
    if not match:
        raise GuardError(f"invalid Ninja dependency record for {object_path}: {lines[0]!r}")
    reported_object = resolved(match.group(1), build)
    if reported_object != object_path:
        raise GuardError(
            f"Ninja dependency record names the wrong object: {reported_object} != {object_path}"
        )
    if "STALE" in (match.group(3) or "").upper():
        raise GuardError(f"Ninja dependency record is stale for selected object: {object_path}")
    dependencies = [
        resolved(line.strip(), build)
        for line in lines[1:]
        if line.startswith((" ", "\t")) and line.strip()
    ]
    if int(match.group(2)) == 0 or len(dependencies) != int(match.group(2)):
        raise GuardError(
            f"Ninja dependency count does not match retained inputs for {object_path}: "
            f"{match.group(2)} != {len(dependencies)}"
        )
    if any(not dependency.is_file() for dependency in dependencies):
        missing = [str(path) for path in dependencies if not path.is_file()]
        raise GuardError(f"Ninja dependency record references missing inputs for {object_path}: {missing}")
    return dependencies, result.stdout


def compile_commands(
    build: Path, repo: Path, source_paths: set[Path], allow: dict,
    item: dict, targets: set[str], sdk: Path | None, ninja_text: str,
) -> list[str]:
    path = build / "compile_commands.json"
    if not path.is_file():
        if not any(source.suffix in NATIVE_COMPILE_SUFFIXES for source in source_paths):
            return []
        raise GuardError(f"missing compile_commands.json for native source closure: {path}")
    entries = load_json(path)
    expected_sources = {
        source for source in source_paths
        if source.suffix in NATIVE_COMPILE_SUFFIXES and source.is_file()
    }
    ninja_commands = selected_ninja_compile_commands(ninja_text, build)
    compiled_sources = set()
    failures = []
    for entry in entries:
        directory = resolved(entry.get("directory", "."), build)
        source = resolved(entry.get("file", ""), directory)
        command = entry.get("command") or " ".join(entry.get("arguments", []))
        if source not in source_paths and not any(str(p) in command for p in source_paths):
            continue
        if source in expected_sources:
            compiled_sources.add(source)
        if source in source_paths and source.suffix not in NATIVE_SUFFIXES:
            failures.append(f"unclassified native compilation source: {source}")
        if FORBIDDEN.search(command):
            failures.append(f"forbidden compile dependency: {source}: {command}")
        if source.suffix in NATIVE_SUFFIXES and not native_input_proven(source, build, repo, allow, item, targets, sdk):
            failures.append(f"unproven native compilation input: {source}")
        try:
            tokens = shlex.split(command)
        except ValueError as exc:
            failures.append(f"cannot parse compile command for {source}: {exc}")
            continue
        command_source, command_object = compile_command_details(tokens, directory)
        if command_source != source or command_object is None:
            failures.append(f"compile command source/object is not explicit for {source}")
            continue
        declared_output = entry.get("output")
        if declared_output and resolved(declared_output, directory) != command_object:
            failures.append(f"compile_commands output differs from command for {source}")
        normalized = tuple(normalized_compile_tokens(tokens))
        if normalized not in ninja_commands:
            failures.append(
                f"compile command is not the selected Ninja producer for {source}: {command_object}"
            )
        if FORBIDDEN.search(str(command_object)):
            failures.append(f"forbidden native object path in {source}: {command_object}")
        depfile = next((
            resolved(tokens[index + 1], directory)
            for index, token in enumerate(tokens[:-1])
            if token == "-MF"
        ), None)
        try:
            dependencies, _ = ninja_deps_inputs(build, command_object)
            if source not in dependencies:
                failures.append(f"Ninja dependency record omits selected source {source}")
            dependency_source = f"Ninja deps for {command_object}"
            if depfile is not None and depfile.is_file():
                raw_targets, raw_inputs = make_depfile_record(depfile)
                dep_targets = [resolved(raw, directory) for raw in raw_targets]
                depfile_dependencies = [resolved(raw, directory) for raw in raw_inputs]
                if command_object not in dep_targets:
                    failures.append(
                        f"compiler depfile targets do not include selected object {command_object}: {depfile}"
                    )
                if source not in depfile_dependencies:
                    failures.append(f"compiler depfile omits selected source {source}: {depfile}")
                if any(not dependency.is_file() for dependency in depfile_dependencies):
                    missing = [
                        str(dependency) for dependency in depfile_dependencies
                        if not dependency.is_file()
                    ]
                    failures.append(
                        f"compiler depfile references missing inputs for {source}: {missing}"
                    )
                if set(depfile_dependencies) != set(dependencies):
                    failures.append(
                        f"compiler depfile differs from retained Ninja dependencies for "
                        f"{command_object}: {depfile}"
                    )
            for dependency in dependencies:
                if dependency.suffix in NATIVE_SUFFIXES and not native_input_proven(
                    dependency, build, repo, allow, item, targets, sdk
                ):
                    failures.append(
                        f"unproven compiler-discovered native dependency: {dependency} "
                        f"(from {dependency_source})"
                    )
        except (OSError, GuardError) as exc:
            failures.append(str(exc))
    for source in sorted(expected_sources - compiled_sources):
        failures.append(f"native source has no selected compile command: {source}")
    return failures


def compile_object_sources(build: Path) -> dict[Path, Path]:
    path = build / "compile_commands.json"
    if not path.is_file():
        return {}
    output_sources = {}
    for entry in load_json(path):
        directory = resolved(entry.get("directory", "."), build)
        source = resolved(entry.get("file", ""), directory)
        command = entry.get("command") or " ".join(entry.get("arguments", []))
        try:
            tokens = shlex.split(command)
        except ValueError as exc:
            raise GuardError(f"cannot parse compile command for {source}: {exc}") from exc
        for index, token in enumerate(tokens):
            if token == "-o" and index + 1 < len(tokens):
                output_sources[resolved(tokens[index + 1], directory)] = source
            elif token.startswith("-o") and len(token) > 2:
                output_sources[resolved(token[2:], directory)] = source
    return output_sources


def archive_producers(text: str, build: Path) -> dict[Path, set[Path]]:
    producers = {}
    lines = [(line, build) for line in text.splitlines()]
    line_index = 0
    ar_tools = {"ar", "llvm-ar", "gcc-ar"}
    while line_index < len(lines):
        line, command_dir = lines[line_index]
        line_index += 1
        try:
            tokens = shlex.split(line)
        except ValueError as exc:
            raise GuardError(f"cannot parse Ninja command: {exc}") from exc
        if len(tokens) > 1 and tokens[0] == "cd":
            command_dir = resolved(tokens[1], command_dir)
            if "&&" in tokens:
                tokens = tokens[tokens.index("&&") + 1:]
        if len(tokens) >= 4 and Path(tokens[0]).name == "cmake" and tokens[1:3] == ["-E", "cmake_link_script"]:
            script = resolved(tokens[3], command_dir)
            if script.is_file():
                lines.extend(
                    (script_line, command_dir)
                    for script_line in script.read_text(errors="replace").splitlines()
                )
        segments, segment = [], []
        for token in tokens:
            if token in {"&&", ";"}:
                if segment:
                    segments.append(segment)
                segment = []
            else:
                segment.append(token)
        if segment:
            segments.append(segment)
        for command in segments:
            if not command:
                continue
            tool = Path(command[0]).name.lower()
            archive_token = None
            object_tokens = []
            if tool in ar_tools:
                args = command[1:]
                index = 0
                while index < len(args) and args[index] in {"--plugin", "--target"}:
                    index += 2
                if index >= len(args):
                    continue
                operation = args[index].lstrip("-")
                if not operation or operation[0] not in "dmpqrstx":
                    continue
                index += 1
                if index >= len(args) or args[index].startswith("-"):
                    continue
                archive_token = args[index]
                object_tokens = args[index + 1:]
            elif tool == "libtool" and "-static" in command:
                output_index = command.index("-o") if "-o" in command else -1
                if output_index >= 0 and output_index + 1 < len(command):
                    archive_token = command[output_index + 1]
                    object_tokens = command[output_index + 2:]
            elif tool == "llvm-lib":
                output = next((value.split(":", 1)[1] for value in command[1:] if value.upper().startswith("/OUT:")), None)
                if output:
                    archive_token = output
                    object_tokens = command[1:]
            if not archive_token:
                continue
            archive = resolved(archive_token, command_dir)
            if not is_archive(archive):
                continue
            objects = {
                resolved(value, command_dir)
                for value in object_tokens
                if Path(value).suffix.lower() in {".o", ".obj"}
            }
            if not objects:
                continue
            previous = producers.get(archive)
            if previous is not None and previous != objects:
                raise GuardError(f"conflicting observed archive producers for {archive}")
            producers[archive] = objects
    return producers


def archive_members(path: Path) -> list[str]:
    result = subprocess.run(["ar", "t", str(path)], text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    if result.returncode:
        raise GuardError(f"cannot inspect archive members in {path}: {result.stderr.strip()}")
    return [name for name in result.stdout.splitlines() if not name.startswith("__.SYMDEF")]



def linker_arguments(tokens: list[str], cwd: Path, findings: list[str], stack: set[Path] | None = None) -> list[str]:
    stack = set() if stack is None else stack
    result = []
    index = 0
    while index < len(tokens):
        token = tokens[index]
        index += 1
        if token == "-Xlinker" and index < len(tokens):
            result.extend(linker_arguments([tokens[index]], cwd, findings, stack))
            index += 1
        elif token in {"-C", "-Clink-arg", "link-arg"} and index < len(tokens):
            value = tokens[index]
            index += 1
            if token == "-C" and value.startswith("link-arg="):
                value = value.split("=", 1)[1]
            result.extend(linker_arguments([value], cwd, findings, stack))
        elif token.startswith(("-Clink-arg=", "link-arg=")):
            result.extend(linker_arguments([token.split("=", 1)[1]], cwd, findings, stack))
        elif token.startswith("-Wl,"):
            result.extend(linker_arguments(token[4:].split(","), cwd, findings, stack))
        elif token.startswith("@"):
            response = resolved(token[1:], cwd)
            if response in stack:
                findings.append(f"recursive linker response file: {response}")
                continue
            if not response.is_file():
                findings.append(f"missing linker response file: {response}")
                continue
            try:
                nested = shlex.split(response.read_text(errors="replace"))
            except ValueError as exc:
                findings.append(f"cannot parse linker response file {response}: {exc}")
                continue
            result.extend(linker_arguments(nested, response.parent, findings, stack | {response}))
        else:
            result.append(token)
    return result


def sdk_library_path(name: str, sdk: Path | None) -> Path | None:
    if sdk is None:
        return None
    for suffix in (".tbd", ".dylib", ".a"):
        path = sdk / "usr/lib" / f"lib{name}{suffix}"
        if path.is_file():
            return path.resolve()
    return None


def resolve_link_library(name: str, search: list[Path], sdk: Path | None) -> Path | None:
    for directory in search:
        for suffix in (".dylib", ".tbd", ".a", ".so", ".lib"):
            for basename in (f"lib{name}{suffix}", f"{name}{suffix}"):
                candidate = directory / basename
                if candidate.is_file():
                    return candidate.resolve()
    return sdk_library_path(name, sdk)


def audit_link_file(
    candidate: Path, product: str, api_artifacts: set[Path],
    producers: dict[Path, set[Path]], object_sources: dict[Path, Path],
    rust_outputs: set[Path], cargo_archives: set[Path],
    repo: Path, build: Path, allow: dict, item: dict, targets: set[str], sdk: Path | None,
) -> list[str]:
    candidate = candidate.resolve()
    if is_archive(candidate):
        if candidate in cargo_archives or candidate in rust_outputs:
            return []
        if candidate in api_artifacts:
            if not producers.get(candidate):
                return [f"linked CMake archive has no observed producer in {product}: {candidate}"]
            return []
        if locked_external(candidate, repo, allow) or (sdk and beneath(candidate, sdk)):
            return []
        return [f"unapproved archive input in {product}: {candidate}"]
    if is_native_object(candidate) or candidate.suffix.lower() in {".o", ".obj"}:
        if candidate in rust_outputs or candidate in cargo_archives:
            return []
        source = object_sources.get(candidate)
        if source and native_input_proven(source, build, repo, allow, item, targets, sdk):
            return []
        if locked_external(candidate, repo, allow) or pinned_generated_source(candidate, build, repo, allow):
            return []
        return [f"unproven native object input in {product}: {candidate}"]
    if candidate in api_artifacts:
        return []
    if sdk and beneath(candidate, sdk):
        return []
    if locked_external(candidate, repo, allow):
        return []
    return [f"unclassified linker file input in {product}: {candidate}"]


def audit_link_inputs(
    args: list[str], cwd: Path, product: str, api_artifacts: set[Path],
    producers: dict[Path, set[Path]], object_sources: dict[Path, Path],
    rust_outputs: set[Path], cargo_archives: set[Path],
    repo: Path, build: Path, allow: dict, item: dict, targets: set[str], sdk: Path | None,
) -> list[str]:
    failures = []
    tokens = linker_arguments(args, cwd, failures)
    search_dirs, framework_dirs = [], []

    def search_path(value: str) -> Path:
        _, separator, path = value.partition("=")
        return resolved(path if separator else value, cwd)

    index = 0
    while index < len(tokens):
        token = tokens[index]
        index += 1
        if token in {"-L", "-F"} and index < len(tokens):
            value = tokens[index]
            index += 1
            (framework_dirs if token == "-F" else search_dirs).append(search_path(value))
        elif token.startswith("-L") and len(token) > 2:
            search_dirs.append(search_path(token[2:]))
        elif token.startswith("-F") and len(token) > 2:
            framework_dirs.append(search_path(token[2:]))
    index = 0
    value_options = {
        "-Xlinker", "-o", "-isysroot", "-syslibroot", "-rpath", "-rpath-link",
        "-install_name", "-macosx_version_min", "-platform_version", "-map",
        "-order_file", "-exported_symbols_list", "-unexported_symbols_list",
        "-u", "-e", "-arch",
    }
    while index < len(tokens):
        token = tokens[index]
        index += 1
        if token in {"-L", "-F", *value_options}:
            index += 1
            continue
        if token in {"-framework", "-weak_framework", "-reexport_framework"} and index < len(tokens):
            framework = tokens[index]
            index += 1
            framework_path = next(
                (directory / f"{framework}.framework" for directory in framework_dirs
                 if (directory / f"{framework}.framework").is_dir()),
                None,
            )
            if framework_path is None:
                if not sdk_framework(framework, sdk):
                    failures.append(f"unresolved Apple framework input in {product}: {framework}")
            elif sdk is None or not beneath(framework_path.resolve(), sdk):
                binary_candidates = (
                    framework_path / "Versions/Current" / framework,
                    framework_path / framework,
                )
                binary = next((path.resolve() for path in binary_candidates if path.is_file()), None)
                if binary is None:
                    failures.append(f"framework binary missing in {product}: {framework_path}")
                else:
                    failures.extend(audit_link_file(
                        binary, product, api_artifacts, producers, object_sources,
                        rust_outputs, cargo_archives, repo, build, allow, item, targets, sdk,
                    ))
            continue
        if token == "-l" and index < len(tokens):
            library = tokens[index]
            index += 1
        elif token.startswith("-l") and not token.startswith("--") and len(token) > 2:
            library = token[2:]
        elif token.startswith("-weak-l") and len(token) > len("-weak-l"):
            library = token[len("-weak-l"):]
        else:
            library = None
        if library:
            if library.startswith("framework="):
                framework = library.partition("=")[2]
                if not framework or not sdk_framework(framework, sdk):
                    failures.append(f"unresolved Apple framework input in {product}: {framework}")
                continue
            candidate = resolve_link_library(library, search_dirs, sdk)
            if candidate is None:
                failures.append(f"unresolved linker library in {product}: -l{library}")
            else:
                failures.extend(audit_link_file(
                    candidate, product, api_artifacts, producers, object_sources,
                    rust_outputs, cargo_archives, repo, build, allow, item, targets, sdk,
                ))
            continue
        if token in {"-filelist", "-force_load"} and index < len(tokens):
            value = tokens[index]
            index += 1
            if token == "-filelist":
                filelist = resolved(value, cwd)
                if not filelist.is_file():
                    failures.append(f"missing linker file list in {product}: {filelist}")
                else:
                    for entry in shlex.split(filelist.read_text(errors="replace")):
                        failures.extend(audit_link_file(
                            resolved(entry, filelist.parent), product, api_artifacts, producers,
                            object_sources, rust_outputs, cargo_archives, repo, build, allow,
                            item, targets, sdk,
                        ))
            else:
                failures.extend(audit_link_file(
                    resolved(value, cwd), product, api_artifacts, producers,
                    object_sources, rust_outputs, cargo_archives, repo, build, allow,
                    item, targets, sdk,
                ))
            continue
        if token.startswith("-"):
            continue
        candidate = resolved(token, cwd)
        if candidate.exists() and (
            is_archive(candidate) or is_native_object(candidate)
            or candidate.suffix.lower() in {".o", ".obj", ".a", ".lib"}
        ):
            failures.extend(audit_link_file(
                candidate, product, api_artifacts, producers, object_sources,
                rust_outputs, cargo_archives, repo, build, allow, item, targets, sdk,
            ))
        elif candidate.exists():
            failures.append(f"unclassified file-valued linker input in {product}: {candidate}")
        elif candidate.suffix.lower() in {".o", ".obj", ".a", ".lib"}:
            failures.append(f"missing native linker input in {product}: {candidate}")
    return failures




def cargo_manifests_from_commands(text: str, fallback: str, build: Path) -> list[Path]:
    manifests = set()
    for line in text.splitlines():
        try:
            tokens = shlex.split(line)
        except ValueError as exc:
            raise GuardError(f"cannot parse Ninja command: {exc}") from exc
        command_dir = build
        if len(tokens) > 1 and tokens[0] == "cd":
            command_dir = resolved(tokens[1], build)
        for index, token in enumerate(tokens):
            if token.startswith("--manifest-path="):
                manifests.add(resolved(token.split("=", 1)[1], command_dir))
            elif token == "--manifest-path" and index + 1 < len(tokens):
                manifests.add(resolved(tokens[index + 1], command_dir))
    if fallback:
        manifests.add(resolved(fallback, build))
    return sorted(manifests)


def audit_cargo_build_scripts(
    metadata: dict, package_ids: set[str], package_roles: dict[str, set[str]],
    options: dict, build: Path, repo: Path, allow: dict, item: dict, targets: set[str],
    api_artifacts: set[Path], producers: dict[Path, set[Path]],
    object_sources: dict[Path, Path], sdk: Path | None, record: dict,
) -> tuple[list[str], set[Path], list[tuple[list[str], Path]]]:
    packages = {package["id"]: package for package in metadata["packages"]}
    selected = selected_cargo_binary(options, metadata)
    if selected is None:
        return ["Cargo build-script closure has no unique selected binary"], set(), []
    root_package_id, selected_bin_name = selected[0]["id"], selected[1]["name"]
    target_dir = options.get("target_dir") or Path(metadata["target_directory"]).resolve()
    profile_root = target_dir / options["profile"]
    target_root = (
        target_dir / options["target"] / options["profile"]
        if options.get("target") else profile_root
    )
    output_hashes = record["build_script_output_sha256"]
    events = [
        message for message in record["messages"]
        if message.get("reason") == "build-script-executed"
    ]
    failures, trusted_archives, all_script_link_args = [], set(), []
    selected_events: dict[tuple[str, str], tuple[dict, Path]] = {}

    for event in events:
        package_id = event.get("package_id")
        package = packages.get(package_id)
        if package is None or package_id not in package_ids:
            failures.append(f"Cargo build-script event belongs to an unselected package: {package_id!r}")
            continue
        if not any("custom-build" in target.get("kind", []) for target in package.get("targets", [])):
            failures.append(f"Cargo build-script event has no metadata custom-build target: {package['name']}")
            continue
        if locked_package_provenance(package, metadata, allow, repo) is None:
            failures.append(
                f"Cargo build-script package has no lock/source provenance: "
                f"{package['name']} {package['version']}"
            )
        try:
            out_dir = cargo_reported_path(event.get("out_dir"), "Cargo build-script out_dir")
        except GuardError as exc:
            failures.append(str(exc))
            continue
        if out_dir.name != "out" or not out_dir.is_dir():
            failures.append(f"Cargo build-script event has an invalid or missing out_dir: {out_dir}")
            continue
        if beneath(out_dir, target_root / "build"):
            role = "target" if options.get("target") else "host"
            build_root = target_root / "build"
        elif options.get("target") and beneath(out_dir, profile_root / "build"):
            role, build_root = "host", profile_root / "build"
        else:
            failures.append(f"Cargo build-script out_dir has no selected host/target role: {out_dir}")
            continue
        build_dir = out_dir.parent
        prefixes = (package["name"] + "-", package["name"].replace("-", "_") + "-")
        if build_dir.parent != build_root or not any(build_dir.name.startswith(prefix) for prefix in prefixes):
            failures.append(
                f"Cargo build-script out_dir is not the package's direct selected output: {out_dir}"
            )
            continue
        if role not in package_roles.get(package_id, set()):
            failures.append(f"Cargo build-script role is outside selected closure: {package['name']}:{role}")
            continue
        key = (package_id, role)
        if key in selected_events:
            failures.append(f"duplicate Cargo build-script event for {package['name']}:{role}")
            continue
        selected_events[key] = (event, out_dir)

    expected_events = set()
    for unit in options.get("unit_graph", {}).get("units", []):
        if (
            not isinstance(unit, dict)
            or unit.get("mode") != "run-custom-build"
            or unit.get("pkg_id") not in package_ids
        ):
            continue
        role = (
            "target"
            if options.get("target") and unit.get("platform") == options["target"]
            else "host"
            if unit.get("platform") is None or not options.get("target")
            else None
        )
        if role is None:
            failures.append(
                f"Cargo unit graph has unsupported build-script platform for "
                f"{packages[unit['pkg_id']]['name']}"
            )
            continue
        expected_events.add((unit["pkg_id"], role))
    missing_events = expected_events - set(selected_events)
    if missing_events:
        failures.append(
            "selected Cargo build-script-executed records missing for: "
            + ", ".join(
                f"{packages[package_id]['name']}:{role}"
                for package_id, role in sorted(missing_events)
            )
        )

    selected_out_dirs = {out_dir for _, out_dir in selected_events.values()}
    generated_outputs = {
        Path(raw_path): digest
        for raw_path, digest in output_hashes.items()
        if any(
            beneath(Path(raw_path), out_dir)
            or Path(raw_path) == out_dir.parent / "output"
            for out_dir in selected_out_dirs
        )
    }

    def generated_output(path: Path) -> bool:
        path = path.resolve(strict=False)
        expected_hash = generated_outputs.get(path)
        return bool(
            expected_hash
            and path.is_file()
            and file_sha256(path) == expected_hash
            and any(
                beneath(path, out_dir) or path == out_dir.parent / "output"
                for out_dir in selected_out_dirs
            )
        )

    def source_proven(path: Path) -> bool:
        return native_input_proven(path, build, repo, allow, item, targets, sdk) or generated_output(path)

    def depfile_path(raw: Path, depfile: Path, package_root: Path) -> Path:
        if raw.is_absolute():
            return raw.resolve(strict=False)
        candidates = {
            (depfile.parent / raw).resolve(strict=False),
            (package_root / raw).resolve(strict=False),
        }
        existing = {candidate for candidate in candidates if candidate.exists()}
        if len(existing) != 1:
            raise GuardError(
                f"ambiguous relative compiler dependency in {depfile}: {raw} "
                f"resolves to {sorted(str(path) for path in existing)}"
            )
        return next(iter(existing))

    for (package_id, role), (event, out_dir) in sorted(selected_events.items()):
        package = packages[package_id]
        package_root = Path(package["manifest_path"]).parent.resolve()
        output = out_dir.parent / "output"
        if not generated_output(output):
            failures.append(
                f"selected Cargo build-script output file is not hash-bound: {package['name']} {output}"
            )
            continue
        native_inputs, search_dirs, libraries, script_link_args = set(), [], [], []
        event_search, event_libraries = [], []
        for line in output.read_text(errors="replace").splitlines():
            line = line.removeprefix("cargo::").removeprefix("cargo:")
            if line.startswith("rerun-if-changed="):
                path = resolved(line.split("=", 1)[1], package_root)
                candidates = path.rglob("*") if path.is_dir() else (path,)
                for candidate in candidates:
                    if candidate.is_file() and candidate.suffix in NATIVE_SUFFIXES:
                        native_inputs.add(candidate.resolve())
            elif line.startswith("rustc-link-search="):
                value = line.split("=", 1)[1]
                kind, separator, location = value.partition("=")
                if not separator:
                    failures.append(f"malformed Cargo native link-search directive: {value}")
                    continue
                event_search.append(value)
                if kind == "native":
                    raw_location = Path(location)
                    if not raw_location.is_absolute():
                        failures.append(
                            f"relative Cargo native link-search path is ambiguous for "
                            f"{package['name']}: {location}"
                        )
                        continue
                    search_dir = raw_location.resolve(strict=False)
                    if not (
                        any(beneath(search_dir, out_dir) for out_dir in selected_out_dirs)
                        or (sdk and beneath(search_dir, sdk))
                        or locked_external(search_dir, repo, allow)
                        or pinned_generated_source(search_dir, build, repo, allow)
                    ):
                        failures.append(
                            f"unapproved Cargo native link-search path for {package['name']}: {search_dir}"
                        )
                    search_dirs.append(search_dir)
            elif line.startswith("rustc-link-lib="):
                value = line.split("=", 1)[1]
                event_libraries.append(value)
                libraries.append(value)
            elif line.startswith("rustc-link-arg="):
                if package_id == root_package_id:
                    script_link_args.append(line.split("=", 1)[1])
            elif line.startswith("rustc-link-arg-bins="):
                if package_id == root_package_id:
                    script_link_args.append(line.split("=", 1)[1])
            elif line.startswith("rustc-link-arg-bin="):
                binary, separator, linker_arg = line.split("=", 1)[1].partition("=")
                if package_id == root_package_id and separator and binary == selected_bin_name:
                    script_link_args.append(linker_arg)

        linked_paths = event.get("linked_paths")
        linked_libs = event.get("linked_libs")
        if not isinstance(linked_paths, list) or sorted(event_search) != sorted(linked_paths):
            failures.append(
                f"Cargo build-script linked_paths differ from retained output: "
                f"{package['name']} ({role})"
            )
        if not isinstance(linked_libs, list) or sorted(event_libraries) != sorted(linked_libs):
            failures.append(
                f"Cargo build-script linked_libs differ from retained output: "
                f"{package['name']} ({role})"
            )
        for source in sorted(native_inputs):
            if not source_proven(source):
                failures.append(f"unproven Cargo build-script native input for {package['name']}: {source}")
        if script_link_args:
            link_args = [arg for directory in search_dirs for arg in ("-L", f"native={directory}")]
            all_script_link_args.append((link_args + script_link_args, package_root))

        objects_by_name: dict[str, list[Path]] = {}
        object_sources_by_path: dict[Path, Path] = {}
        output_files = sorted(
            path for path in generated_outputs if beneath(path, out_dir)
        )
        for depfile in (path for path in output_files if path.suffix == ".d"):
            try:
                raw_targets, raw_inputs = make_depfile_record(depfile)
                dependency_targets = [depfile_path(raw, depfile, package_root) for raw in raw_targets]
                dependencies = [depfile_path(raw, depfile, package_root) for raw in raw_inputs]
                if not dependencies or any(not dependency.is_file() for dependency in dependencies):
                    raise GuardError(f"compiler depfile has missing inputs: {depfile}")
                source_inputs = {
                    dependency for dependency in dependencies
                    if dependency.suffix in NATIVE_COMPILE_SUFFIXES
                }
                object_targets = {
                    target for target in dependency_targets
                    if target.suffix.lower() in {".o", ".obj"}
                }
                if len(object_targets) != 1 or len(source_inputs) != 1:
                    raise GuardError(
                        f"Cargo compiler depfile does not identify one object and one source: {depfile}"
                    )
                source = next(iter(source_inputs))
                object_target = next(iter(object_targets))
                if (
                    not beneath(object_target, out_dir)
                    or object_target not in output_files
                    or not object_target.is_file()
                ):
                    raise GuardError(
                        f"Cargo compiler depfile object target is missing/outside observed out_dir: "
                        f"{object_target}"
                    )
                previous = object_sources_by_path.setdefault(object_target, source)
                if previous != source:
                    raise GuardError(f"conflicting compiler sources for object target: {object_target}")
                objects_by_name.setdefault(object_target.name, []).append(object_target)
                for dependency in dependencies:
                    if dependency.suffix in NATIVE_SUFFIXES and not source_proven(dependency):
                        failures.append(
                            f"unproven Cargo compiler-discovered input for {package['name']}: {dependency}"
                        )
            except (OSError, GuardError) as exc:
                failures.append(str(exc))

        for artifact in output_files:
            if is_archive(artifact):
                members = archive_members(artifact)
                if not members:
                    failures.append(f"empty Cargo build-script archive cannot be provenance-checked: {artifact}")
                    continue
                if artifact in api_artifacts:
                    objects = producers.get(artifact, set())
                    if not objects or sorted(members) != sorted(obj.name for obj in objects):
                        failures.append(f"Cargo build-script archive producer mismatch: {artifact}")
                        continue
                    for obj in objects:
                        source = object_sources.get(obj)
                        if source is None or not source_proven(source):
                            failures.append(f"unproven Cargo archive member source: {obj}")
                    trusted_archives.add(artifact)
                    continue
                member_ok = True
                for member in members:
                    objects = objects_by_name.get(Path(member).name, [])
                    if len(objects) != 1:
                        member_ok = False
                        break
                    object_path = objects[0]
                    source = object_sources_by_path.get(object_path)
                    if source is None or not source_proven(source):
                        member_ok = False
                        break
                    extracted = subprocess.run(
                        ["ar", "p", str(artifact), member],
                        stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                    )
                    try:
                        object_bytes = object_path.read_bytes()
                    except OSError:
                        member_ok = False
                        break
                    if extracted.returncode or extracted.stdout != object_bytes:
                        member_ok = False
                        break
                if member_ok:
                    trusted_archives.add(artifact)
                else:
                    failures.append(
                        f"Cargo build-script archive members lack matching produced objects/sources: {artifact}"
                    )
            elif is_native_object(artifact) or artifact.suffix.lower() in {".o", ".obj"}:
                source = object_sources_by_path.get(artifact)
                if source is None or not source_proven(source):
                    failures.append(f"Cargo build-script object has no unique proven source: {artifact}")
                else:
                    trusted_archives.add(artifact)
            elif artifact.suffix.lower() in {".dylib", ".so", ".dll"}:
                if not (
                    locked_external(artifact, repo, allow)
                    or pinned_generated_source(artifact, build, repo, allow)
                ):
                    failures.append(f"Cargo build-script produced unproven dynamic library: {artifact}")
                else:
                    trusted_archives.add(artifact)

        for entry in libraries:
            kind, separator, library = entry.partition("=")
            if not separator:
                kind, library = "dylib", kind
            kind = kind.split(":", 1)[0]
            candidates = [
                directory / f"lib{library}{suffix}"
                for directory in search_dirs
                for suffix in (".a", ".dylib", ".so", ".tbd")
            ]
            resolved_library = next((candidate.resolve() for candidate in candidates if candidate.is_file()), None)
            if kind == "static":
                if resolved_library is None:
                    failures.append(
                        f"unresolved Cargo build-script static library for {package['name']}: {library}"
                    )
                elif (
                    resolved_library in api_artifacts
                    or resolved_library in trusted_archives
                    or locked_external(resolved_library, repo, allow)
                    or pinned_generated_source(resolved_library, build, repo, allow)
                    or (sdk and beneath(resolved_library, sdk))
                ):
                    trusted_archives.add(resolved_library)
                else:
                    failures.append(
                        f"Cargo build-script static library lacks selected output/member proof: {resolved_library}"
                    )
            elif resolved_library is not None:
                if not (
                    resolved_library in api_artifacts
                    or resolved_library in trusted_archives
                    or locked_external(resolved_library, repo, allow)
                    or (sdk and beneath(resolved_library, sdk))
                ):
                    failures.append(
                        f"Cargo build-script dynamic library lacks approved provenance: {resolved_library}"
                    )
            elif kind != "framework" or not sdk_framework(library, sdk):
                if not sdk_library_path(library, sdk):
                    failures.append(
                        f"unresolved Cargo build-script library for {package['name']}: {entry}"
                    )
    return failures, trusted_archives, all_script_link_args
def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--build-dir", required=True, type=Path)
    parser.add_argument("--product", required=True, action="append")
    parser.add_argument("--report", required=True, type=Path)
    parser.add_argument("--require-final-bin", action="store_true", help="require schema-4 final-binary evidence")
    args = parser.parse_args()
    build = args.build_dir.resolve()
    repo = Path(__file__).resolve().parents[1]
    artifacts_env = os.environ.get("HP2_ARTIFACT_DIR")
    findings: list[str] = []
    report = {"schema_version": 1, "build_dir": str(build), "products": {}, "ok": False, "findings": findings}
    try:
        if not artifacts_env:
            raise GuardError("HP2_ARTIFACT_DIR is required")
        artifacts = Path(artifacts_env).resolve()
        report_path = resolved(args.report, Path.cwd())
        if not beneath(report_path, artifacts) or report_path == artifacts:
            raise GuardError("--report must resolve beneath HP2_ARTIFACT_DIR")
        if not report_path.parent.is_dir():
            raise GuardError(f"report parent must already exist: {report_path.parent}")
        if not build.is_dir():
            raise GuardError(f"build directory does not exist: {build}")
        manifest = get_manifest(build)
        allow_path = repo / "Build/native-free-allowlist.json"
        allow = load_json(allow_path)
        products = manifest["products"]
        if not (build / "build.ninja").is_file():
            raise GuardError("Ninja build graph is required for product-command inspection")
        for name in args.product:
            if name not in products:
                raise GuardError(f"product absent from manifest: {name}")
            item = products[name]
            if not isinstance(item, dict) or not item.get("target_file"):
                raise GuardError(f"product {name} has no target_file")
            inputs = item.get("inputs", [])
            seeds = [raw for raw in inputs if isinstance(raw, str) and raw and "$<" not in raw]
            api_findings, api_sources, api_artifacts, api_targets = file_api_closure(build, name, seeds)
            findings.extend(api_findings)
            text = ninja_product_commands(build, name)
            target = resolved(item["target_file"], build)
            if not target.exists():
                findings.append(f"product target_file is missing for {name}: {target}")
            cargo_manifests = cargo_manifests_from_commands(text, item.get("cargo_manifest", ""), build)
            if not cargo_manifests:
                findings.append(f"no Cargo manifest found in configured command for {name}")
            cargo_closures = []
            cargo = cargo_dir = cargo_args = cargo_options = cargo_record = None
            cargo_env_prefix = cargo_proxy_env = None
            if cargo_manifests:
                (
                    cargo, cargo_dir, cargo_args, cargo_env_prefix, cargo_proxy_env,
                ) = cargo_invocation(text, build, repo)
                if cargo_proxy_env["HP2_CARGO_PROVENANCE_TARGET"] != name:
                    findings.append(
                        f"Cargo capture proxy target differs for {name}: "
                        f"{cargo_proxy_env['HP2_CARGO_PROVENANCE_TARGET']!r}"
                    )
                proxy_record = absolute_record_path(
                    cargo_proxy_env["HP2_CARGO_PROVENANCE_RECORD"],
                    "Cargo capture proxy record",
                )
                manifest_record = resolved(item.get("cargo_provenance", ""), build)
                if proxy_record != manifest_record:
                    findings.append(
                        f"Cargo capture proxy record differs from product manifest for {name}: "
                        f"{proxy_record} != {manifest_record}"
                    )
                cargo_options = cargo_build_options(cargo_args, cargo_dir)
                try:
                    cargo_record = cargo_provenance_record(
                        item.get("cargo_provenance"), name, build,
                        cargo, cargo_dir, cargo_args,
                        configured_cargo=cargo_proxy_env["HP2_CARGO_PROVENANCE_EXECUTABLE"],
                        configured_environment=cargo_proxy_env,
                    )
                except GuardError as exc:
                    findings.append(str(exc))
                if args.require_final_bin and cargo_record and cargo_record.get("schema_version") != 4:
                    findings.append(f"final-binary qualification evidence is required for {name}")
                if cargo_record and not cargo_options.get("target"):
                    cargo_options["target"] = cargo_record["environment"].get("CARGO_BUILD_TARGET")
                selected_manifest = (
                    resolved(cargo_options["manifest"], cargo_dir)
                    if cargo_options.get("manifest") else None
                )
                if selected_manifest is None:
                    findings.append(f"selected Cargo invocation has no manifest path for {name}")
                if selected_manifest is None or cargo_manifests != [selected_manifest]:
                    findings.append(f"Cargo manifest inventory differs from selected invocation for {name}")
                selected_manifests = [selected_manifest] if selected_manifest else []
                for cargo_manifest in selected_manifests:
                    allow["cargo_manifest"] = str(cargo_manifest)
                    metadata_findings, cargo_metadata = check_cargo_metadata(
                        cargo_manifest, cargo, cargo_options,
                        cargo_record.get("environment", {}) if cargo_record else {}, cargo_dir,
                    )
                    findings.extend(metadata_findings)
                    if cargo_metadata:
                        allow["cargo_lock"] = str(Path(cargo_metadata["workspace_root"]) / "Cargo.lock")
                        graph_findings, unit_graph = cargo_unit_graph(
                            cargo_manifest, cargo, cargo_options, cargo_metadata,
                            cargo_record.get("environment", {}) if cargo_record else {},
                            cargo_dir,
                        )
                        findings.extend(graph_findings)
                        if unit_graph:
                            cargo_options["unit_graph"] = unit_graph
                        closure_findings, package_ids, package_roles = cargo_closure_findings(
                            name, cargo_metadata, cargo_options
                        )
                        findings.extend(closure_findings)
                        cargo_closures.append(
                            (cargo_metadata, package_ids, package_roles, cargo_options, cargo_dir, cargo_record)
                        )
                        expected = expected_cargo_binary(cargo_options, cargo_metadata, cargo_dir)
                        if not expected.is_file() or not cargo_binary_copy_provenance(text, expected, target, build):
                            findings.append(
                                f"Cargo output/copy producer does not prove declared target_file for {name}: "
                                f"expected {expected}, target {target}"
                            )
            sources = {resolved(p, build) for p in item.get("sources", [])} | api_sources
            object_sources = compile_object_sources(build)
            producers = archive_producers(text, build)
            rust_outputs, rust_symbol_outputs = set(), set()
            for cargo_metadata, package_ids, package_roles, options, command_dir, producer_record in cargo_closures:
                if producer_record is None:
                    continue
                outputs, symbol_outputs, artifact_findings = rust_objects_for_closure(
                    cargo_metadata, package_ids, package_roles, options, command_dir,
                    producer_record, allow, repo,
                )
                findings.extend(artifact_findings)
                rust_outputs.update(path.resolve() for path in outputs)
                rust_symbol_outputs.update(path.resolve() for path in symbol_outputs)
                rust_symbol_outputs.update(producer_record.get("_final_bin_objects", []))
            sdk = apple_sdk_root(build, allow)
            for value in sources:
                if is_generated_rule_label(value) and not value.exists():
                    continue
                proven_input = native_input_proven(value, build, repo, allow, item, api_targets, sdk)
                if value.suffix in NATIVE_SUFFIXES:
                    if not value.exists():
                        findings.append(f"missing native target source: {value}")
                    if not proven_input:
                        findings.append(f"unproven native source in {name}: {value}")
                if FORBIDDEN.search(str(value)):
                    findings.append(f"forbidden legacy dependency in {name}: {value}")
                if (
                    not proven_input
                    and beneath(value, repo)
                    and not beneath(value, repo / "ThirdParty")
                    and LEGACY_NATIVE_DIR.search(value.relative_to(repo).as_posix())
                ):
                    findings.append(f"first-party legacy native directory in {name}: {value}")
            findings.extend(compile_commands(build, repo, sources, allow, item, api_targets, sdk, text))
            for artifact in api_artifacts:
                if not is_archive(artifact):
                    continue
                objects = producers.get(artifact)
                if not objects:
                    findings.append(f"CMake archive has no Ninja producer inputs for {name}: {artifact}")
                    continue
                if not artifact.is_file():
                    findings.append(f"missing CMake archive artifact for {name}: {artifact}")
                    continue
                members = archive_members(artifact)
                if sorted(members) != sorted(obj.name for obj in objects):
                    findings.append(f"archive member list does not match producer inputs: {artifact}")
                for obj in objects:
                    source = object_sources.get(obj)
                    if source is None or not native_input_proven(source, build, repo, allow, item, api_targets, sdk):
                        findings.append(f"unproven archive member object in {name}: {obj}")
            cargo_archives, build_script_link_args = set(), []
            for cargo_metadata, package_ids, package_roles, options, command_dir, producer_record in cargo_closures:
                if producer_record is None:
                    continue
                build_findings, build_archives, script_args = audit_cargo_build_scripts(
                    cargo_metadata, package_ids, package_roles, options, build, repo, allow, item,
                    api_targets, api_artifacts, producers, object_sources, sdk, producer_record,
                )
                findings.extend(build_findings)
                cargo_archives.update(build_archives)
                build_script_link_args.extend(script_args)
            if cargo_args is not None:
                separator = cargo_args.index("--") if "--" in cargo_args else len(cargo_args)
                linker_args = cargo_rustflags(
                    cargo_env_prefix or [], cargo_record.get("environment", {}) if cargo_record else {},
                ) + cargo_args[separator + 1:]
                findings.extend(audit_link_inputs(
                    linker_args, cargo_dir, name, api_artifacts, producers,
                    object_sources, rust_outputs, cargo_archives, repo, build,
                    allow, item, api_targets, sdk,
                ))
            for script_args, script_cwd in build_script_link_args:
                findings.extend(audit_link_inputs(
                    script_args, script_cwd, name, api_artifacts, producers,
                    object_sources, rust_outputs, cargo_archives, repo, build,
                    allow, item, api_targets, sdk,
                ))
            target = resolved(item["target_file"], build)
            if not target.is_file():
                findings.append(f"missing product artifact: {target}")
            rust_objects = rust_symbol_outputs
            if cargo is None or cargo_record is None:
                findings.append(f"cannot inspect Rust ABI without Cargo provenance for {name}")
            elif not rust_objects:
                findings.append(f"no selected Rust artifacts for ABI inspection for {name}")
            else:
                try:
                    nm = rust_llvm_nm(cargo, cargo_dir, cargo_record.get("environment", {}), cargo_args)
                except GuardError as exc:
                    findings.append(str(exc))
                else:
                    if cargo_record.get("schema_version") == 4:
                        sysroot = Path(cargo_record["final_bin"]["context"]["trees"][0]).resolve()
                        if not nm.resolve().is_relative_to(sysroot):
                            findings.append("selected LLVM nm is outside the bound compiler sysroot")
                    for obj in sorted(rust_objects):
                        findings.extend(inspect_rust_abi(obj, nm))
            installed = item.get("installed_file")
            if installed:
                installed_path = resolved(installed, build)
                if not installed_path.is_file() or not target.is_file() or installed_path.read_bytes() != target.read_bytes():
                    findings.append(f"installed executable differs from guarded artifact for {name}: {installed_path}")
            cargo_evidence = None
            if cargo_record is not None:
                cargo_evidence = {
                    "record": str(resolved(item["cargo_provenance"], build)),
                    "capture_proxy": str((repo / "Build" / "capture_cargo_provenance.py").resolve()),
                    "configured_cargo": cargo_proxy_env["HP2_CARGO_PROVENANCE_EXECUTABLE"],
                    "proxy_target": cargo_proxy_env["HP2_CARGO_PROVENANCE_TARGET"],
                    "proxy_record": cargo_proxy_env["HP2_CARGO_PROVENANCE_RECORD"],
                    "selected_target": cargo_record["selected_target"],
                    "cargo_argv": cargo_record["cargo_argv"],
                    "cwd": cargo_record["cwd"],
                    "status": cargo_record["status"],
                    "compiler_artifact_count": sum(
                        message.get("reason") == "compiler-artifact"
                        for message in cargo_record["messages"]
                    ),
                    "build_script_count": sum(
                        message.get("reason") == "build-script-executed"
                        for message in cargo_record["messages"]
                    ),
                    "artifact_sha256": cargo_record["artifact_sha256"],
                    "build_script_output_sha256": cargo_record["build_script_output_sha256"],
                    "product_sha256": file_sha256(target) if target.is_file() else None,
                    "rust_abi_inputs": sorted(str(path) for path in rust_objects),
                }
            report["products"][name] = {
                "target_file": str(target),
                "source_count": len(sources),
                "input_count": len(inputs),
                "cargo_provenance": cargo_evidence,
            }
        report["ok"] = not findings
    except GuardError as exc:
        findings.append(str(exc))
    except (OSError, ValueError, TypeError) as exc:
        findings.append(f"invalid guard input: {exc}")
    if artifacts_env:
        report_path = resolved(args.report, Path.cwd())
        if beneath(report_path, Path(artifacts_env).resolve()) and report_path.parent.is_dir():
            report_path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    for finding in findings:
        print(f"native-free: {finding}", file=sys.stderr)
    if report.get("ok"):
        print("native-free: selected product closure passed")
        return 0
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
