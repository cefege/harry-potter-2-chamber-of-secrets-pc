#!/usr/bin/env python3
"""Opt-in final-binary evidence. Unsupported configurations fail closed.

This records content hashes before and after production and rechecks them during
an audit. It is not a sealed toolchain or protection against hostile transient
replacement while a process runs.
"""
from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile


class Unsupported(RuntimeError):
    """The invocation cannot be qualified by this narrow capture."""


WRAPPER_KEYS = {
    "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER",
    "CARGO_BUILD_RUSTC_WRAPPER", "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER",
}


def reject_wrappers(env: dict, config: dict) -> None:
    selected = sorted(key for key in WRAPPER_KEYS if key in env)
    selected.extend(sorted(key for key in env if key.startswith("HP2_FINAL_BIN_")))
    build = config.get("build", {})
    if not isinstance(build, dict):
        raise Unsupported("Cargo build configuration is malformed")
    for key in build:
        if key.replace("_", "-") in {"rustc-wrapper", "rustc-workspace-wrapper"}:
            selected.append("build." + key)
    for key in config:
        if key.replace("_", "-") in {"build.rustc-wrapper", "build.rustc-workspace-wrapper"}:
            selected.append(key)
    configured_env = config.get("env", {})
    if not isinstance(configured_env, dict):
        raise Unsupported("Cargo env configuration is malformed")
    selected.extend("env." + key for key in configured_env
                    if key.upper() in WRAPPER_KEYS or key.upper().startswith("HP2_FINAL_BIN_"))
    if selected:
        raise Unsupported("pre-existing Rust wrapper selector is unsupported: " + ", ".join(selected))


def digest(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            h.update(block)
    return h.hexdigest()


def identity(paths: list[str], trees: list[str]) -> dict:
    """Hash regular files and symlinks; reject escapes and special files."""
    result = {}
    for raw in paths:
        path = Path(raw)
        if not path.is_absolute() or not path.is_file():
            raise Unsupported(f"identity file is missing: {path}")
        result[str(path)] = {"sha256": digest(path), "resolved": str(path.resolve(strict=True))}
    for raw in trees:
        root = Path(raw).resolve(strict=True)
        if not root.is_dir():
            raise Unsupported(f"identity tree is missing: {root}")
        def onerror(error):
            raise Unsupported(f"cannot enumerate identity tree: {error}") from error
        for base, directories, files in os.walk(root, followlinks=False, onerror=onerror):
            directories.sort()
            files.sort()
            for name in directories + files:
                path = Path(base) / name
                if path.is_symlink():
                    destination = path.resolve(strict=True)
                    if not destination.is_relative_to(root):
                        raise Unsupported(f"identity tree symlink escapes: {path}")
                    result[str(path)] = {"symlink": os.readlink(path)}
                elif path.is_file():
                    result[str(path)] = {"sha256": digest(path)}
                elif not path.is_dir():
                    raise Unsupported(f"identity tree has special file: {path}")
    return result


def config_arguments(argv: list[str]) -> list[str]:
    flags = []
    tokens = argv[2:]
    end = tokens.index("--") if "--" in tokens else len(tokens)
    i = 0
    while i < end:
        token = tokens[i]
        if token == "--config":
            if i + 1 >= end or not tokens[i + 1] or tokens[i + 1].startswith("--"):
                raise Unsupported("missing Cargo --config value")
            flags.extend((token, tokens[i + 1]))
            i += 2
        else:
            if token.startswith("--config="):
                if not token[len("--config="):]:
                    raise Unsupported("missing Cargo --config value")
                flags.append(token)
            i += 1
    return flags


def config_query(cargo: str, argv: list[str], cwd: Path, env: dict) -> dict:
    query_env = dict(env)
    query_env["RUSTC_BOOTSTRAP"] = "1"
    try:
        result = subprocess.run([cargo, "-Z", "unstable-options", *config_arguments(argv),
                                 "config", "get", "--format", "json"], cwd=cwd, env=query_env,
                                text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=True)
        config = json.loads(result.stdout)
    except (OSError, subprocess.SubprocessError, ValueError) as exc:
        raise Unsupported(f"cannot inspect actual Cargo configuration: {exc}") from exc
    if not isinstance(config, dict):
        raise Unsupported("Cargo configuration is not an object")
    return config


def require_direct_cargo(cargo: str, sysroot: Path) -> Path:
    actual = Path(cargo).resolve(strict=True)
    candidate = sysroot / "bin" / "cargo"
    if candidate.is_symlink() or not candidate.is_file():
        raise Unsupported("Cargo must be the direct executable from the selected compiler sysroot")
    expected = candidate.resolve(strict=True)
    if actual != expected or not expected.is_relative_to(sysroot.resolve(strict=True)):
        raise Unsupported("Cargo must be the direct executable from the selected compiler sysroot")
    return actual


def prepare(cargo_argv: list[str], cwd: Path, env: dict, artifact_dir: Path, target: str) -> tuple[dict, dict]:
    if sys.platform != "darwin":
        raise Unsupported("final-binary qualification currently supports macOS only")
    config = config_query(cargo_argv[0], cargo_argv, cwd, env)
    reject_wrappers(env, config)
    configured_env = config.get("env", {})
    unsafe_env = {key for key in configured_env if key.upper() in {"SDKROOT", "RUSTC", "RUSTUP_TOOLCHAIN", "RUSTUP_HOME", "PATH", "CARGO_BUILD_RUSTC"}}
    if unsafe_env:
        raise Unsupported("Cargo [env] compiler, SDK or PATH overrides are unsupported: " + ", ".join(sorted(unsafe_env)))
    selected = env.get("RUSTC") or env.get("CARGO_BUILD_RUSTC") or config.get("build", {}).get("rustc") or "rustc"
    if not isinstance(selected, str):
        raise Unsupported("Cargo compiler selector is not a path")
    compiler = shutil.which(selected, path=env.get("PATH")) if not Path(selected).is_absolute() else selected
    if not compiler:
        raise Unsupported("selected Cargo compiler not found")
    compiler = str(Path(compiler).resolve(strict=True))
    try:
        root = subprocess.run([compiler, "--print", "sysroot"], cwd=cwd, env=env, text=True,
                              stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=True).stdout.strip()
    except (OSError, subprocess.SubprocessError) as exc:
        raise Unsupported(f"cannot identify selected compiler sysroot: {exc}") from exc
    sysroot = Path(root).resolve(strict=True)
    if Path(compiler) != (sysroot / "bin" / "rustc").resolve(strict=True):
        raise Unsupported("compiler must be the direct sysroot rustc, not a selector or wrapper")
    cargo = require_direct_cargo(cargo_argv[0], sysroot)
    sdk = env.get("SDKROOT")
    if not sdk or not Path(sdk).is_absolute():
        raise Unsupported("qualification requires explicit absolute SDKROOT")
    run_dir = Path(tempfile.mkdtemp(prefix="final-bin-", dir=artifact_dir)).resolve()
    python = str(Path(sys.executable).resolve(strict=True))
    files = [str(cargo), compiler, str(Path(__file__).resolve()), python]
    trees = [str(sysroot), str(Path(sdk).resolve(strict=True))]
    before = identity(files, trees)
    config_sha = hashlib.sha256(json.dumps(config, sort_keys=True).encode()).hexdigest()
    context = {"target": target, "compiler": compiler, "python": python, "files": files, "trees": trees,
               "identity": before, "run_dir": str(run_dir), "cargo_argv": cargo_argv,
               "cwd": str(cwd), "config_sha256": config_sha}
    (run_dir / "context.json").write_text(json.dumps(context, sort_keys=True), encoding="utf-8")
    child = dict(env)
    child["RUSTC_WRAPPER"] = str(Path(__file__).resolve())
    child["HP2_FINAL_BIN_CONTEXT"] = str(run_dir / "context.json")
    return context, child


def _compiler_linker(args: list[str]) -> tuple[int, str]:
    options = []
    for index, token in enumerate(args):
        if token == "-C" and index + 1 < len(args) and args[index + 1].startswith("linker="):
            options.append((index + 1, args[index + 1][len("linker="):]))
        elif token.startswith("-Clinker="):
            options.append((index, token[len("-Clinker="):]))
    if len(options) != 1 or not Path(options[0][1]).is_absolute():
        raise Unsupported("qualification requires exactly one explicit absolute Cargo-selected linker")
    return options[0]


def _native_program(path: Path) -> bool:
    if not path.is_file() or not os.access(path, os.X_OK):
        return False
    with path.open("rb") as stream:
        magic = stream.read(4)
    return magic in {b"\x7fELF", b"\xcf\xfa\xed\xfe", b"\xfe\xed\xfa\xcf", b"\xca\xfe\xba\xbe", b"\xbe\xba\xfe\xca"}


def rustc_wrapper(context: dict, argv: list[str]) -> int:
    if not argv:
        raise Unsupported("missing rustc executable")
    compiler, args = argv[0], argv[1:]
    if str(Path(compiler).resolve(strict=True)) != context["compiler"]:
        raise Unsupported("Cargo selected a different compiler")
    selected = ("--crate-name" in args and args[args.index("--crate-name") + 1] == context["target"].replace("-", "_")
                and "--crate-type" in args and args[args.index("--crate-type") + 1] == "bin")
    if not selected:
        return subprocess.run(argv, check=False).returncode
    index, linker = _compiler_linker(args)
    linker_path = Path(linker).resolve(strict=True)
    if (linker_path != Path("/Library/Developer/CommandLineTools/usr/bin/clang")
            or not _native_program(linker_path)):
        raise Unsupported("configured linker driver must be the direct Command Line Tools clang")
    modified = list(args)
    prefix = "-Clinker=" if args[index].startswith("-Clinker=") else "linker="
    modified[index] = prefix + str(Path(__file__).resolve())
    env = os.environ.copy()
    env["HP2_FINAL_BIN_LINKER_STAGE"] = "1"
    env["HP2_FINAL_BIN_REAL_LINKER"] = str(linker_path)
    run_dir = Path(context["run_dir"])
    (run_dir / "rustc-argv.json").write_text(json.dumps({"original": argv, "delegated": [compiler, *modified]}), encoding="utf-8")
    result = subprocess.run([compiler, *modified], env=env, check=False)
    if result.returncode:
        return result.returncode
    link_path = run_dir / "link.json"
    link = json.loads(link_path.read_text(encoding="utf-8"))
    output = Path(link["output"])
    link["post_rustc_sha256"] = digest(output)
    link["rustc_argv_sha256"] = digest(run_dir / "rustc-argv.json")
    link_path.write_text(json.dumps(link, sort_keys=True), encoding="utf-8")
    return 0


def linker_file_arguments(args: list[str]) -> list[tuple[int, str]]:
    """Classify the supported clang driver grammar, never skip unknown options."""
    names = re.compile(r"[A-Za-z0-9_+.-]+\Z")
    # Avoid clang options such as -load and -lto_library, which also start
    # with -l but do not mean a direct library-name argument.
    libraries = {"System", "c", "m", "c++", "objc", "iconv", "SDL2", "openal",
                 "archive", "z", "hp2_bzip2", "lzma", "resolv", "pthread"}
    files = []
    index = 0
    outputs = 0
    while index < len(args):
        arg = args[index]
        if not arg or "@" in arg or "filelist" in arg.lower():
            raise Unsupported("response or filelist linker argument is unsupported")
        if arg in {"-o", "-L", "-F", "-arch", "-framework", "-weak_framework", "-l"}:
            if index + 1 >= len(args):
                raise Unsupported(f"missing value for linker option {arg}")
            value = args[index + 1]
            if not value or value.startswith("-") or "@" in value:
                raise Unsupported(f"invalid value for linker option {arg}")
            if arg == "-o":
                outputs += 1
            elif arg in {"-L", "-F"} and not Path(value).is_absolute():
                raise Unsupported("relative linker search path is unsupported")
            elif arg == "-arch" and value not in {"arm64", "x86_64"}:
                raise Unsupported("unsupported linker architecture")
            elif arg == "-l" and value not in libraries:
                raise Unsupported(f"unsupported linker library name: {value}")
            elif arg in {"-framework", "-weak_framework"} and not names.fullmatch(value):
                raise Unsupported(f"unsupported framework name: {value}")
            index += 2
            continue
        if arg.startswith("-L") or arg.startswith("-F"):
            if len(arg) <= 2 or not Path(arg[2:]).is_absolute():
                raise Unsupported("missing or relative linker search path")
        elif arg.startswith("-l"):
            if arg[2:] not in libraries:
                raise Unsupported(f"unsupported linker library name or option: {arg}")
        elif arg in {"-nodefaultlibs", "-nostdlib", "-Wl,-dead_strip", "-Wl,-no_compact_unwind",
                     "-Wl,-no_warn_duplicate_libraries"}:
            pass
        elif arg.startswith("-mmacosx-version-min="):
            if not re.fullmatch(r"-mmacosx-version-min=[0-9]+(?:\.[0-9]+)*", arg):
                raise Unsupported("invalid macOS deployment target")
        elif arg.startswith("-fuse-ld="):
            if not Path(arg[len("-fuse-ld="):]).is_absolute():
                raise Unsupported("final linker path is not absolute")
        elif arg.startswith("-Wl,"):
            parts = arg.split(",")
            if (len(parts) != 3 or parts[1] not in {"-framework", "-weak_framework", "-rpath"}
                    or not parts[2] or parts[2].startswith("-")):
                raise Unsupported(f"unsupported embedded linker option: {arg}")
            if parts[1] != "-rpath" and not names.fullmatch(parts[2]):
                raise Unsupported(f"unsupported framework name: {parts[2]}")
            if parts[1] == "-rpath" and not Path(parts[2]).is_absolute():
                raise Unsupported("relative runtime search path is unsupported")
        elif arg.startswith("-"):
            raise Unsupported(f"unrecognized linker option: {arg}")
        else:
            files.append((index, arg))
        index += 1
    if outputs != 1:
        raise Unsupported("link output is ambiguous")
    return files


def linker_input_kind(path: Path) -> str:
    with path.open("rb") as stream:
        header = stream.read(32)
    if header.startswith(b"!<arch>\n"):
        return "archive"
    if header.startswith((b"BC\xc0\xde", b"\xde\xc0\x17\x0b")):
        return "bitcode"
    byte_orders = {b"\xfe\xed\xfa\xce": "big", b"\xce\xfa\xed\xfe": "little",
                   b"\xfe\xed\xfa\xcf": "big", b"\xcf\xfa\xed\xfe": "little"}
    order = byte_orders.get(header[:4])
    if order and len(header) >= 16 and int.from_bytes(header[12:16], order) == 1:
        return "object"
    if header.startswith(b"\x7fELF") and len(header) >= 18 and header[5] in {1, 2}:
        order = "little" if header[5] == 1 else "big"
        if int.from_bytes(header[16:18], order) == 1:
            return "object"
    raise Unsupported(f"unrecognized or non-object direct linker input: {path}")


def linker_wrapper(context: dict, args: list[str]) -> int:
    run_dir = Path(context["run_dir"])
    if (run_dir / "link.json").exists():
        raise Unsupported("multiple final link invocations")
    inputs = linker_file_arguments(args)
    ld = [arg.split("=", 1)[1] for arg in args if arg.startswith("-fuse-ld=")]
    if len(ld) != 1 or not Path(ld[0]).is_absolute():
        raise Unsupported("qualification requires an explicit absolute -fuse-ld linker")
    linker = Path(os.environ["HP2_FINAL_BIN_REAL_LINKER"]).resolve(strict=True)
    ld_path = Path(ld[0]).resolve(strict=True)
    if ld_path != linker.parent / "ld" or not _native_program(ld_path):
        raise Unsupported("selected final linker must be the matching direct Command Line Tools ld")
    output = Path(args[args.index("-o") + 1]).resolve(strict=False)
    objects = []
    for index, arg in inputs:
        original = Path(arg)
        source = original.resolve(strict=True)
        kind = linker_input_kind(source)
        original_hash = digest(source)
        copy = run_dir / f"object-{index}.o"
        shutil.copyfile(source, copy)
        retained_before = digest(copy)
        if original.resolve(strict=True) != source or digest(source) != original_hash or retained_before != original_hash:
            raise Unsupported(f"link input changed while retaining it: {original}")
        objects.append({"argument": index, "source_argument": arg, "source": str(source),
                        "kind": kind, "path": str(copy), "sha256": original_hash,
                        "source_sha256_before": original_hash, "retained_sha256_before": retained_before})
    if not any(item["kind"] in {"object", "bitcode"} for item in objects):
        raise Unsupported("final link has no direct object or bitcode input")
    files = [str(linker), str(ld_path)]
    before = identity(files, [])
    result = subprocess.run([str(linker), *args], check=False)
    for item in objects:
        original = Path(item["source_argument"])
        item["source_sha256_after"] = digest(original)
        item["retained_sha256_after"] = digest(Path(item["path"]))
        if original.resolve(strict=True) != Path(item["source"]) or item["source_sha256_after"] != item["sha256"]:
            raise Unsupported(f"link input changed during delegation: {original}")
        if item["retained_sha256_after"] != item["sha256"]:
            raise Unsupported(f"retained link input changed during delegation: {item['path']}")
    if before != identity(files, []):
        raise Unsupported("linker identity changed during link")
    if result.returncode:
        return result.returncode
    record = {"argv": args, "linker": str(linker), "ld": str(ld_path), "tool_identity": before,
              "objects": objects, "output": str(output), "link_output_sha256": digest(output)}
    (run_dir / "link.json").write_text(json.dumps(record, sort_keys=True), encoding="utf-8")
    return 0


def finish(context: dict, messages: list[dict], env: dict) -> dict:
    selected = [m for m in messages if m.get("reason") == "compiler-artifact"
                and m.get("target", {}).get("name") == context["target"]
                and "bin" in m.get("target", {}).get("kind", [])]
    if len(selected) != 1 or selected[0].get("fresh") is not False:
        raise Unsupported("selected binary must be rebuilt; fresh or ambiguous Cargo artifact")
    run_dir = Path(context["run_dir"])
    link = json.loads((run_dir / "link.json").read_text(encoding="utf-8"))
    executable = Path(selected[0]["executable"]).resolve(strict=True)
    if digest(executable) != link["post_rustc_sha256"]:
        raise Unsupported("Cargo executable differs from post-rustc output")
    if identity(context["files"], context["trees"]) != context["identity"]:
        raise Unsupported("toolchain or SDK changed during Cargo invocation")
    config = config_query(context["cargo_argv"][0], context["cargo_argv"], Path(context["cwd"]), env)
    reject_wrappers(env, config)
    if hashlib.sha256(json.dumps(config, sort_keys=True).encode()).hexdigest() != context["config_sha256"]:
        raise Unsupported("Cargo configuration changed during producer invocation")
    return {"schema_version": 1, "context": context, "link": link, "executable": str(executable)}


def verify(evidence: dict, artifact_dir: Path, env: dict) -> list[Path]:
    if not isinstance(evidence, dict) or evidence.get("schema_version") != 1:
        raise Unsupported("missing or malformed final-binary evidence")
    context = evidence["context"]
    if (not isinstance(context.get("files"), list) or not isinstance(context.get("trees"), list)
            or len(context["trees"]) != 2 or context.get("compiler") not in context["files"]
            or context.get("python") not in context["files"]
            or str(Path(__file__).resolve()) not in context["files"]):
        raise Unsupported("incomplete compiler, helper, sysroot or SDK identity inventory")
    sysroot = Path(context["trees"][0]).resolve(strict=True)
    if Path(context["compiler"]).resolve(strict=True) != (sysroot / "bin" / "rustc").resolve(strict=True):
        raise Unsupported("bound compiler does not belong to the bound sysroot")
    cargo = require_direct_cargo(context["cargo_argv"][0], sysroot)
    if str(cargo) not in context["files"]:
        raise Unsupported("direct Cargo executable is absent from identity inventory")
    run_dir = Path(context["run_dir"]).resolve(strict=True)
    if not run_dir.is_relative_to(artifact_dir.resolve(strict=True)):
        raise Unsupported("final-binary evidence escapes producer artifacts")
    if identity(context["files"], context["trees"]) != context["identity"]:
        raise Unsupported("compiler, sysroot, SDK or capture tool identity changed")
    if not env.get("SDKROOT") or Path(env["SDKROOT"]).resolve(strict=True) != Path(context["trees"][1]).resolve(strict=True):
        raise Unsupported("selected SDK differs from bound SDK")
    config = config_query(context["cargo_argv"][0], context["cargo_argv"], Path(context["cwd"]), env)
    reject_wrappers(env, config)
    if hashlib.sha256(json.dumps(config, sort_keys=True).encode()).hexdigest() != context["config_sha256"]:
        raise Unsupported("Cargo configuration differs from the producer")
    link = evidence["link"]
    if json.loads((run_dir / "context.json").read_text(encoding="utf-8")) != context:
        raise Unsupported("final-binary capture context differs")
    if json.loads((run_dir / "link.json").read_text(encoding="utf-8")) != link:
        raise Unsupported("final-binary link record differs")
    rustc_args = run_dir / "rustc-argv.json"
    if digest(rustc_args) != link["rustc_argv_sha256"]:
        raise Unsupported("compiler invocation evidence changed")
    invocation = json.loads(rustc_args.read_text(encoding="utf-8"))
    original = invocation["original"]
    if Path(original[0]).resolve(strict=True) != Path(context["compiler"]).resolve(strict=True):
        raise Unsupported("compiler invocation does not use the selected compiler")
    index, original_linker = _compiler_linker(original[1:])
    if Path(original_linker).resolve(strict=True) != Path(link["linker"]).resolve(strict=True):
        raise Unsupported("configured linker differs from the delegated linker")
    delegated = list(original)
    token = delegated[index + 1]
    delegated[index + 1] = ("-Clinker=" if token.startswith("-Clinker=") else "linker=") + str(Path(__file__).resolve())
    if invocation["delegated"] != delegated:
        raise Unsupported("compiler arguments differ beyond linker interception")
    if [arg for arg in link["argv"] if arg.startswith("-fuse-ld=")] != ["-fuse-ld=" + link["ld"]]:
        raise Unsupported("actual final-linker argument differs from bound ld")
    if identity([link["linker"], link["ld"]], []) != link["tool_identity"]:
        raise Unsupported("linker identity changed")
    executable = Path(evidence["executable"])
    if digest(executable) != link["post_rustc_sha256"]:
        raise Unsupported("final executable changed")
    objects = []
    input_arguments = linker_file_arguments(link["argv"])
    if [(item["argument"], item["source_argument"]) for item in link["objects"]] != input_arguments:
        raise Unsupported("final-link input inventory differs from linker arguments")
    for item in link["objects"]:
        path = Path(item["path"]).resolve(strict=True)
        if (not path.is_relative_to(run_dir) or digest(path) != item["sha256"]
                or any(item[key] != item["sha256"] for key in (
                    "source_sha256_before", "source_sha256_after", "retained_sha256_before", "retained_sha256_after"))
                or linker_input_kind(path) != item["kind"]):
            raise Unsupported("missing or stale final-link input")
        objects.append(path)
    if not any(item["kind"] in {"object", "bitcode"} for item in link["objects"]):
        raise Unsupported("missing final-link object or bitcode")
    return objects


def main() -> int:
    try:
        context = json.loads(Path(os.environ["HP2_FINAL_BIN_CONTEXT"]).read_text(encoding="utf-8"))
        if str(Path(sys.executable).resolve(strict=True)) != context["python"]:
            raise Unsupported("capture wrapper Python differs from the bound interpreter")
        if os.environ.get("HP2_FINAL_BIN_LINKER_STAGE") == "1":
            return linker_wrapper(context, sys.argv[1:])
        return rustc_wrapper(context, sys.argv[1:])
    except (KeyError, IndexError, OSError, ValueError, Unsupported) as exc:
        print(f"final-bin capture unsupported: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
