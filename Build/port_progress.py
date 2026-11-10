#!/usr/bin/env python3
"""Report Rust port counters and ordered macOS acceptance gates; stdlib only."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys

SOURCES = frozenset({".c", ".cc", ".cpp", ".cxx", ".m", ".mm"})
HEADERS = frozenset({".h", ".hpp", ".inl", ".inc"})
NATIVE = SOURCES | HEADERS
ENGINE = Path("HarryPotter2/Unreal")
INVENTORY = Path("Build/engine-native-inventory.json")
CATALOG_ROW = re.compile(r"engine_native!\(\s*((?:native_opcode!\(\w+\))|-?\w+)\s*,\s*(\w+)\s*,\s*(\w+)")
SPEC_ROW = re.compile(r'class_name:\s*"Engine\.(\w+)",\s*function_name:\s*"(\w+)"')
REFERENCE = re.compile(r"crate::(\w+)::NATIVE_FUNCTIONS\[(\d+)\]")
NATIVE_ENTRY = re.compile(
    r"VmNativeSpec\s*\{(?P<spec>[^}]+)\}|"
    r"crate::(?P<module>\w+)::NATIVE_FUNCTIONS\[(?P<index>\d+)\]",
    re.DOTALL,
)
GATES = {
    "G1": ("Every prototype script package loads", ("canonical_engine_package_load",)),
    "G2": ("Rust Entry boot, 120 ticks with wgpu", ("canonical_game_boot_entry",)),
    "G3": ("Rust three-map renderer smoke", ("canonical_renderer_smoke_maps",)),
    "G4": ("Prototype load/run and observed functional gameplay", ()),
    "G5": ("Scripted input and cross-process persistence", ("canonical_input_script_smoke", "canonical_save_reload_smoke")),
    "G6": ("No startup launcher; settings through stock in-game menus", ("no_startup_launcher_contracts", "stock_render_scale_control")),
    "G7": ("Archived oracle, installed Rust products and native-free audits", ("native_free_game", "native_free_ucc", "native_free_import")),
    "G8": ("Installed user approval, then no first-party native code or hp2-ffi", ()),
    "G9": ("Full macOS functional/component suite", ()),
    "G10": ("Verified retail data and functional gameplay", ()),
    "G11": ("Human audio, input, visuals and continue acceptance", ()),
}

# Exact framebuffer parity remains diagnostic and never satisfies these checks.
FUNCTIONAL_REQUIREMENTS = {
    "G4": ("data-prototype", (
        "all_maps_load_run", "movement_collision", "projectile_hit",
        "world_interaction", "cutscene_return_control", "map_travel",
    )),
    "G5": ("data-prototype", ("cross_process_persistence",)),
    "G6": ("data-none", ("stock_menu_settings_delivery",)),
    "G7": ("data-prototype", ("oracle_archive", "installed_launch", "installed_persistence")),
    "G10": ("data-retail", (
        "data_provenance", "retail_launch", "movement_collision", "projectile_hit",
        "world_interaction", "cutscene_return_control", "map_travel", "cross_process_persistence",
    )),
}
HUMAN_ITEMS = frozenset({
    "audible_music_and_sfx", "physical_keyboard_mouse", "visual_review_three_maps",
    "save_quit_relaunch_continue", "installed_rust_gameplay",
})


def files(root: Path, suffixes: frozenset[str]):
    return sorted(p for p in root.rglob("*") if p.is_file() and p.suffix.lower() in suffixes)


def static_counters(root: Path) -> dict:
    sources = files(root / ENGINE, SOURCES)
    headers = files(root / ENGINE, HEADERS)
    harness_roots = sorted((root / "Build").glob("*_fixture_harness"))
    tracked = subprocess.run(["git", "ls-files", "-z"], cwd=root, capture_output=True, check=True).stdout
    other = []
    for raw in tracked.split(b"\0"):
        if not raw:
            continue
        path = Path(os.fsdecode(raw))
        if path.suffix.lower() not in NATIVE:
            continue
        if path.is_relative_to(ENGINE) or path.is_relative_to("Tests") or path.is_relative_to("ThirdParty"):
            continue
        if any(path.is_relative_to(p.relative_to(root)) for p in harness_roots):
            continue
        other.append(path)
    exports = {}
    for crate in sorted((root / "crates").glob("*")):
        if not (crate / "src").is_dir():
            continue
        count = sum(len(re.findall(r"#\[(unsafe\()?no_mangle", p.read_text())) for p in (crate / "src").rglob("*.rs"))
        exports[crate.name] = count
    return {
        "native_engine_sources": len(sources),
        "native_engine_headers": len(headers),
        "native_engine_nonblank_lines": sum(sum(bool(line.strip()) for line in p.read_bytes().splitlines()) for p in sources + headers),
        "native_test_files": len(files(root / "Tests", frozenset({".cpp", ".h", ".mm"}))),
        "native_harness_files": sum(len(files(p, NATIVE)) for p in harness_roots),
        "native_other_files": len(other),
        "native_binaries": len(files(root / ENGINE, frozenset({".lib", ".a", ".o"}))),
        "ffi_exports": exports,
        "ffi_crate_present": (root / "crates/hp2-ffi").exists(),
    }


def freeze_inventory(root: Path) -> dict:
    catalog = root / "crates/hp2-ffi/src"
    paths = sorted(catalog.glob("engine_natives_*.rs"))
    if not paths or not (catalog / "engine_native_catalog.rs").is_file():
        raise ValueError("legacy catalog absent; inventory is frozen")
    rows = []
    for path in paths + [catalog / "engine_native_catalog.rs"]:
        for index, cls, function in CATALOG_ROW.findall(path.read_text()):
            if cls[:1] in {"A", "U"}:
                cls = cls[1:]
            function = function.removeprefix("exec")
            if (cls, function) == ("Object", "Undefined"):
                continue
            rows.append({"class": cls, "function": function, "index": index, "legacy_file": path.name})
    if len(rows) != 173:
        raise ValueError(f"expected 173 legacy natives, found {len(rows)}")
    inventory = {"version": 1, "natives": rows}
    write_json(root / INVENTORY, inventory)
    return inventory


def array_body(text: str, name: str) -> str:
    """Select a named Rust array initializer, not type brackets or helper specs."""
    match = re.search(r"\b(?:static|const)\s+" + re.escape(name) + r"\s*:[^=]*=\s*&?\s*\[", text)
    if match is None:
        raise ValueError(f"native array {name} not found")
    start = match.end()
    depth = 1
    for position in range(start, len(text)):
        if text[position] == "[":
            depth += 1
        elif text[position] == "]":
            depth -= 1
            if depth == 0:
                return text[start:position]
    raise ValueError(f"unterminated native array {name}")


def native_coverage(root: Path, inventory: dict) -> dict:
    if inventory.get("version") != 1 or not isinstance(inventory.get("natives"), list) or not inventory["natives"]:
        raise ValueError("invalid frozen native inventory")
    source = root / "crates/hp2-engine/src"
    registry = array_body((source / "engine_vm_native.rs").read_text(), "ENGINE_VM_NATIVES")
    bound = {(cls.casefold(), fn.casefold()) for cls, fn in SPEC_ROW.findall(registry)}
    modules = {}
    resolved = {}
    active = set()

    def resolve(module: str, index: int) -> tuple[str, str]:
        key = (module, index)
        if key in resolved:
            return resolved[key]
        if key in active:
            raise ValueError(f"cyclic native reference: {module}[{index}]")
        if module not in modules:
            body = array_body((source / (module + ".rs")).read_text(), "NATIVE_FUNCTIONS")
            # References occupy array slots just like inline specs; preserve
            # their positions when a batch composes smaller handler modules.
            modules[module] = list(NATIVE_ENTRY.finditer(body))
        try:
            entry = modules[module][index]
        except IndexError as error:
            raise ValueError(f"native reference out of range: {module}[{index}]") from error
        active.add(key)
        try:
            if entry["spec"] is None:
                identity = resolve(entry["module"], int(entry["index"]))
            else:
                matches = SPEC_ROW.findall(entry["spec"])
                if len(matches) != 1:
                    raise ValueError(f"unresolved native reference: {module}[{index}]")
                identity = tuple(value.casefold() for value in matches[0])
            resolved[key] = identity
            return identity
        finally:
            active.remove(key)

    for module, index in REFERENCE.findall(registry):
        bound.add(resolve(module, int(index)))
    missing = {}
    seen = set()
    count = 0
    for row in inventory["natives"]:
        key = (row["class"].casefold(), row["function"].casefold())
        if key in seen:
            raise ValueError(f"duplicate frozen native: {row['class']}.{row['function']}")
        seen.add(key)
        if key in bound:
            count += 1
        else:
            missing.setdefault(row["legacy_file"], []).append(f"{row['class']}.{row['function']}")
    return {"engine_natives_bound": count, "engine_natives_total": len(seen),
            "engine_natives_missing": {name: sorted(rows) for name, rows in sorted(missing.items())}}


def ctest_status(root: Path, preset: str, name: str, runner=subprocess.run) -> str:
    base = ["ctest", "--preset", preset]
    expression = "^" + re.escape(name) + "$"
    listed = runner(base + ["-N", "-R", expression], cwd=root, capture_output=True, text=True)
    if not re.search(r"Test\s+#\d+:\s*" + re.escape(name) + r"\s*$", listed.stdout, re.MULTILINE):
        # A preset/configuration failure is not the same as an absent registration.
        if re.search(r"Total Tests:\s*0\b", listed.stdout + listed.stderr):
            return "not-registered"
        return "fail"
    if listed.returncode:
        return "fail"
    result = runner(base + ["-R", expression, "--output-on-failure"], cwd=root, capture_output=True, text=True)
    return "pass" if result.returncode == 0 else "fail"


def read_json(path: Path):
    return json.loads(path.read_text())


def write_json(path: Path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2) + "\n")


def parity_summary(path: Path) -> dict:
    report = read_json(path)
    result = {key: report[key] for key in ("data_root", "profile", "passed_count", "total")}
    passed, total = result["passed_count"], result["total"]
    if type(passed) is not int or type(total) is not int or not 0 <= passed <= total:
        raise ValueError(f"invalid parity counts: {path}")
    if result["profile"] not in {"data-prototype", "data-retail"}:
        raise ValueError(f"invalid parity profile: {path}")
    return result


def combined(statuses):
    for status in ("fail", "not-registered", "not-evaluated"):
        if status in statuses:
            return status
    return "pass"


def functional_summary(path: Path, root: Path) -> dict:
    report = read_json(path)
    if report.get("version") != 1 or report.get("profile") not in {
        "data-none", "data-prototype", "data-retail",
    } or not isinstance(report.get("checks"), dict):
        raise ValueError(f"invalid functional acceptance report: {path}")
    executable = root / report["executable"]
    digest = hashlib.sha256(executable.read_bytes()).hexdigest()
    if digest != report.get("executable_sha256"):
        raise ValueError(f"functional report executable identity mismatch: {path}")
    for name, check in report["checks"].items():
        if not isinstance(check, dict) or check.get("status") not in {"pass", "fail", "not-evaluated"}:
            raise ValueError(f"invalid functional check {name}: {path}")
        if check["status"] != "pass":
            continue
        command, observations, artifacts = (check.get(key) for key in ("command", "observations", "artifacts"))
        if not isinstance(command, list) or not command or not all(isinstance(x, str) and x for x in command):
            raise ValueError(f"functional check lacks an exercised command {name}: {path}")
        if not isinstance(observations, list) or not observations or not all(isinstance(x, str) and x for x in observations):
            raise ValueError(f"functional check lacks observed outcomes {name}: {path}")
        if not isinstance(artifacts, list) or not artifacts:
            raise ValueError(f"functional check lacks artifacts {name}: {path}")
        for artifact in artifacts:
            artifact_path = root / artifact["path"]
            if hashlib.sha256(artifact_path.read_bytes()).hexdigest() != artifact.get("sha256"):
                raise ValueError(f"functional artifact identity mismatch {name}: {artifact_path}")
    return {"path": str(path), **report}


def functional_status(reports: list, profile: str, name: str) -> str:
    statuses = [report["checks"][name]["status"] for report in reports
                if report["profile"] == profile and name in report["checks"]]
    return combined(statuses) if statuses else "not-evaluated"


def human_statuses(human: Path, root: Path) -> tuple[str, str]:
    if not human.is_file():
        return "not-evaluated", "not-evaluated"
    record = read_json(human)
    items = record.get("items", {})
    if record.get("version") != 1 or not isinstance(items, dict) or not HUMAN_ITEMS.issubset(items):
        raise ValueError(f"invalid human acceptance record: {human}")
    identified = all(isinstance(record.get(key), str) and record[key].strip() for key in ("observer", "date"))
    installed = "fail"
    if items["installed_rust_gameplay"] is True and identified:
        product = record.get("installed_product")
        if isinstance(product, dict) and isinstance(product.get("executable"), str):
            executable = root / product["executable"]
            if executable.is_file() and hashlib.sha256(executable.read_bytes()).hexdigest() == product.get("sha256"):
                installed = "pass"
    complete = "pass" if installed == "pass" and all(items[name] is True for name in HUMAN_ITEMS) else "fail"
    return complete, installed


def evaluate_gates(root: Path, counters: dict, parity: list, human: Path, preset: str, run: bool,
                   functional: list | None = None) -> dict:
    # `parity` is retained in reports solely as a historical diagnostic.
    functional = functional or []
    gates = {}
    for gate, (description, tests) in GATES.items():
        checks = {test: ctest_status(root, preset, test) if run else "not-evaluated" for test in tests}
        if gate in FUNCTIONAL_REQUIREMENTS:
            profile, names = FUNCTIONAL_REQUIREMENTS[gate]
            checks.update({name: functional_status(functional, profile, name) for name in names})
        gates[gate] = {"description": description, "checks": checks, "status": "not-evaluated"}
    complete_human, installed_human = human_statuses(human, root)
    gates["G11"]["checks"]["human_observations"] = complete_human
    gates["G8"]["checks"].update({
        "native_sources_removed": "pass" if (not counters["ffi_crate_present"] and
            all(value == 0 for key, value in counters.items() if key.startswith("native_")) and
            not any(counters["ffi_exports"].values())) else "fail",
        "installed_rust_gameplay_approved": installed_human,
    })
    for gate, command in (
        ("G7", [sys.executable, "Build/check_bundle.py"]),
        ("G9", ["ctest", "--preset", "macos-arm64", "-E", "hp2_canonical_game_boot_parity|parity_maps|pixel_capture"]),
    ):
        status = "not-evaluated"
        if run:
            result = subprocess.run(command, cwd=root, capture_output=True, text=True)
            status = "pass" if result.returncode == 0 else "fail"
        gates[gate]["checks"][" ".join(command)] = status
    for gate, prerequisites in (("G7", ("G1", "G2", "G3", "G4", "G5", "G6")), ("G8", ("G7",))):
        for prerequisite in prerequisites:
            checks = gates[prerequisite]["checks"]
            gates[gate]["checks"][f"prerequisite_{prerequisite}"] = combined(list(checks.values())) if checks else "not-evaluated"
    for value in gates.values():
        value["status"] = combined(list(value["checks"].values())) if value["checks"] else "not-evaluated"
    return gates


def print_tables(report: dict):
    print("| Counter | Now |\n| --- | ---: |")
    for key, value in report["counters"].items():
        if key == "ffi_exports":
            print(f"| ffi_exports (all crates) | {sum(value.values())} |")
            for crate, count in value.items():
                if count:
                    print(f"| ffi_exports: {crate} | {count} |")
        else:
            print(f"| {key} | {str(value).lower() if isinstance(value, bool) else value} |")
    for key in ("engine_natives_bound", "engine_natives_total"):
        print(f"| {key} | {report['coverage'][key]} |")
    for parity in report["parity"]:
        print(f"| exact-parity diagnostic ({parity['profile']}) | {parity['passed_count']} / {parity['total']} |")
    print("\n| Gate | Acceptance | Status |\n| --- | --- | --- |")
    for gate, result in report["gates"].items():
        print(f"| {gate} | {result['description']} | {result['status']} |")


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--preset", default="macos-arm64")
    parser.add_argument("--run-gates", action="store_true")
    parser.add_argument("--parity-report", type=Path, action="append", default=[])
    parser.add_argument("--functional-report", type=Path, action="append", default=[])
    parser.add_argument("--human-acceptance", type=Path)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--freeze-native-inventory", action="store_true")
    args = parser.parse_args(argv)
    if args.output is None:
        artifact = os.environ.get("HP2_ARTIFACT_DIR")
        if not artifact:
            print("port_progress: --output required (HP2_ARTIFACT_DIR unset)", file=sys.stderr)
            return 2
        args.output = Path(artifact) / "port-progress.json"
    try:
        root = args.repo_root.resolve()
        inventory = freeze_inventory(root) if args.freeze_native_inventory else read_json(root / INVENTORY)
        counters = static_counters(root)
        parity = [parity_summary(path) for path in args.parity_report]
        functional = [functional_summary(path, root) for path in args.functional_report]
        report = {
            "version": 2, "acceptance_policy": "functional-gameplay", "repo_root": str(root), "preset": args.preset,
            "counters": counters, "coverage": native_coverage(root, inventory), "parity": parity,
            "functional": functional,
            "gates": evaluate_gates(root, counters, parity, args.human_acceptance or root / "Build/human-acceptance.json", args.preset, args.run_gates, functional),
        }
        write_json(args.output, report)
        print_tables(report)
        return 0
    except (OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError) as error:
        print(f"port_progress: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
