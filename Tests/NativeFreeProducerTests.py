#!/usr/bin/env python3
"""Regression tests for native-free Cargo and compiler dependency evidence."""
import hashlib
import importlib.util
import json
from pathlib import Path
import shlex
import subprocess
import tempfile
import unittest
import io
import tarfile
from unittest import mock


ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("check_native_free", ROOT / "Build" / "check_native_free.py")
check_native_free = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(check_native_free)
CAPTURE_SPEC = importlib.util.spec_from_file_location(
    "capture_cargo_provenance", ROOT / "Build" / "capture_cargo_provenance.py"
)
capture_cargo_provenance = importlib.util.module_from_spec(CAPTURE_SPEC)
assert CAPTURE_SPEC.loader is not None
CAPTURE_SPEC.loader.exec_module(capture_cargo_provenance)


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


class CargoProvenanceTests(unittest.TestCase):
    def make_record(self, root: Path) -> tuple[Path, Path, Path, list[str]]:
        root = root.resolve()
        build = root / "build"
        artifact_dir = build / "Testing" / "HP2" / "native_free_core" / "Artifacts"
        artifact_dir.mkdir(parents=True)
        command_dir = root / "workspace" / "crates" / "hp2-ucc"
        command_dir.mkdir(parents=True)
        real_cargo = root / "toolchain" / "bin" / "cargo-real"
        real_cargo.parent.mkdir(parents=True)
        real_cargo.write_bytes(b"cargo executable")
        cargo = real_cargo.parent / "cargo"
        cargo.symlink_to(real_cargo)
        output = build / "cargo" / "aarch64-apple-darwin" / "release" / "hp2_ucc_rust"
        output.parent.mkdir(parents=True)
        output.write_bytes(b"selected rust executable")
        args = [
            "--bin=hp2_ucc_rust",
            "--target=aarch64-apple-darwin",
            "--package=hp2-ucc",
            "--manifest-path",
            str(command_dir / "Cargo.toml"),
            "--target-dir",
            str(build / "cargo"),
            "--release",
        ]
        build_out = (
            build / "cargo" / "aarch64-apple-darwin" / "release"
            / "build" / "hp2-core-hash" / "out"
        )
        build_out.parent.mkdir(parents=True)
        build_out.mkdir()
        build_output = build_out / "generated.h"
        build_output.write_bytes(b"generated output")
        cargo_output = build_out.parent / "output"
        cargo_output.write_bytes(b"cargo:rustc-cfg=fixture\n")
        messages = [
            {
                "reason": "compiler-artifact",
                "package_id": "path+file:///workspace/crates/hp2-ucc#hp2-ucc@0.1.0",
                "target": {"name": "hp2_ucc_rust"},
                "filenames": [str(output)],
                "executable": str(output),
            },
            {
                "reason": "build-script-executed",
                "package_id": "path+file:///workspace/crates/hp2-core#hp2-core@0.1.0",
                "out_dir": str(build_out.resolve()),
                "linked_paths": [],
                "linked_libs": [],
            },
            {"reason": "build-finished", "success": True},
        ]
        stdout = "".join(json.dumps(message, separators=(",", ":")) + "\n" for message in messages).encode()
        stderr = b""
        (artifact_dir / "cargo.stdout").write_bytes(stdout)
        (artifact_dir / "cargo.stderr").write_bytes(stderr)
        record = {
            "schema_version": 1,
            "selected_target": "hp2_ucc_rust",
            "cargo_argv": [str(cargo), "rustc", *args],
            "cwd": str(command_dir.resolve()),
            "environment": {
                "RUSTFLAGS": "-Cdebuginfo=0",
                "CORROSION_BUILD_DIR": str(build.resolve()),
            },
            "status": 0,
            "stdout_sha256": sha256(stdout),
            "stderr_sha256": sha256(stderr),
            "messages": messages,
            "artifact_sha256": {str(output.resolve()): sha256(output.read_bytes())},
            "build_script_output_sha256": {
                str(build_output.resolve()): sha256(build_output.read_bytes()),
                str(cargo_output.resolve()): sha256(cargo_output.read_bytes()),
            },
        }
        record_path = artifact_dir / "cargo-provenance.json"
        record_path.write_text(json.dumps(record), encoding="utf-8")
        return build, record_path, command_dir, args


    def test_record_rejects_tampered_artifact_bytes(self):
        with tempfile.TemporaryDirectory() as directory:
            build, record_path, command_dir, args = self.make_record(Path(directory))
            record = json.loads(record_path.read_text())
            artifact = Path(next(iter(record["artifact_sha256"])))
            artifact.write_bytes(b"tampered")
            with self.assertRaises(check_native_free.GuardError):
                check_native_free.cargo_provenance_record(
                    str(record_path), "hp2_ucc_rust", build,
                    Path(record["cargo_argv"][0]), command_dir, args,
                )

    def test_record_rejects_tampered_build_script_output(self):
        with tempfile.TemporaryDirectory() as directory:
            build, record_path, command_dir, args = self.make_record(Path(directory))
            record = json.loads(record_path.read_text())
            output = Path(next(iter(record["build_script_output_sha256"])))
            output.write_bytes(b"tampered generated output")
            with self.assertRaises(check_native_free.GuardError):
                check_native_free.cargo_provenance_record(
                    str(record_path), "hp2_ucc_rust", build,
                    Path(record["cargo_argv"][0]), command_dir, args,
                )
    def test_record_binds_cargo_build_script_output_sibling(self):
        with tempfile.TemporaryDirectory() as directory:
            build, record_path, command_dir, args = self.make_record(Path(directory))
            record = json.loads(record_path.read_text())
            output = Path(next(
                path for path in record["build_script_output_sha256"]
                if Path(path).name == "output"
            ))
            self.assertEqual(output.parent.name, "hp2-core-hash")
            output.write_bytes(b"tampered Cargo directives")
            with self.assertRaises(check_native_free.GuardError):
                check_native_free.cargo_provenance_record(
                    str(record_path), "hp2_ucc_rust", build,
                    Path(record["cargo_argv"][0]), command_dir, args,
                )


    def test_product_copy_requires_matching_selected_executable_bytes(self):
        with tempfile.TemporaryDirectory() as directory:
            build = Path(directory).resolve() / "build"
            expected = build / "cargo" / "release" / "hp2_game"
            target = build / "app" / "MacOS" / "hp2_game"
            expected.parent.mkdir(parents=True)
            target.parent.mkdir(parents=True)
            executable = bytearray(32)
            executable[:4] = b"\xcf\xfa\xed\xfe"
            executable[12:16] = (2).to_bytes(4, "little")
            expected.write_bytes(executable)
            target.write_bytes(executable)
            command = f"cmake -E copy {expected} {target.parent}"
            self.assertTrue(
                check_native_free.cargo_binary_copy_provenance(
                    command, expected, target, build,
                )
            )
            target.write_bytes(b"substituted")
            self.assertFalse(
                check_native_free.cargo_binary_copy_provenance(
                    command, expected, target, build,
                )
            )

    def test_record_rejects_failed_producer_and_unsuccessful_build_event(self):
        for mutation in ("status", "build-finished"):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as directory:
                build, record_path, command_dir, args = self.make_record(Path(directory))
                record = json.loads(record_path.read_text())
                if mutation == "status":
                    record["status"] = 101
                else:
                    record["messages"][-1]["success"] = False
                    stdout = "".join(
                        json.dumps(message, separators=(",", ":")) + "\n"
                        for message in record["messages"]
                    ).encode()
                    (record_path.parent / "cargo.stdout").write_bytes(stdout)
                    record["stdout_sha256"] = sha256(stdout)
                record_path.write_text(json.dumps(record), encoding="utf-8")
                with self.assertRaises(check_native_free.GuardError):
                    check_native_free.cargo_provenance_record(
                        str(record_path), "hp2_ucc_rust", build,
                        Path(record["cargo_argv"][0]), command_dir, args,
                    )

    def test_record_rejects_selected_build_environment_mismatch(self):
        with tempfile.TemporaryDirectory() as directory:
            build, record_path, command_dir, args = self.make_record(Path(directory))
            record = json.loads(record_path.read_text())
            with self.assertRaises(check_native_free.GuardError):
                check_native_free.cargo_provenance_record(
                    str(record_path), "hp2_ucc_rust", build,
                    Path(record["cargo_argv"][0]), command_dir, args,
                    configured_environment={"RUSTFLAGS": "-Cdebuginfo=2"},
                )


    def test_record_binds_corrosion_directory_and_rejects_unclassified_env(self):
        with tempfile.TemporaryDirectory() as directory:
            build, record_path, command_dir, args = self.make_record(Path(directory))
            record = json.loads(record_path.read_text())
            cargo = Path(record["cargo_argv"][0])
            for selected_env in (
                {"CORROSION_BUILD_DIR": str(build.parent)},
                {"UNCLASSIFIED_BUILD_FLAG": "unexpected"},
            ):
                with self.subTest(selected_env=selected_env), self.assertRaises(
                    check_native_free.GuardError,
                ):
                    check_native_free.cargo_provenance_record(
                        str(record_path), "hp2_ucc_rust", build, cargo, command_dir, args,
                        configured_environment=selected_env,
                    )
    def test_record_rejects_wrong_target_argv_and_retained_messages(self):
        for mutation in ("target", "argv", "messages"):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as directory:
                build, record_path, command_dir, args = self.make_record(Path(directory))
                record = json.loads(record_path.read_text())
                if mutation == "target":
                    record["selected_target"] = "hp2_import"
                elif mutation == "argv":
                    record["cargo_argv"][-1] = "--debug"
                else:
                    record["messages"][0]["package_id"] = "unrelated-package"
                record_path.write_text(json.dumps(record), encoding="utf-8")
                with self.assertRaises(check_native_free.GuardError):
                    check_native_free.cargo_provenance_record(
                        str(record_path), "hp2_ucc_rust", build,
                        Path(record["cargo_argv"][0]), command_dir, args,
                    )
    def test_record_rejects_missing_hash_inventory(self):
        with tempfile.TemporaryDirectory() as directory:
            build, record_path, command_dir, args = self.make_record(Path(directory))
            record = json.loads(record_path.read_text())
            record["artifact_sha256"] = {}
            record_path.write_text(json.dumps(record), encoding="utf-8")
            with self.assertRaises(check_native_free.GuardError):
                check_native_free.cargo_provenance_record(
                    str(record_path), "hp2_ucc_rust", build,
                    Path(json.loads(record_path.read_text())["cargo_argv"][0]), command_dir, args,
                )
                
class LockedRegistryPackageTests(unittest.TestCase):
    def test_registry_package_requires_matching_locked_archive(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            cargo_home = root / "cargo"
            source = cargo_home / "registry/src/index-hash/libc-0.2.190"
            source.mkdir(parents=True)
            manifest = source / "Cargo.toml"
            manifest_contents = b'[package]\nname = "libc"\nversion = "0.2.190"\n'
            manifest.write_bytes(manifest_contents)
            archive = cargo_home / "registry/cache/index-hash/libc-0.2.190.crate"
            archive.parent.mkdir(parents=True)
            with tarfile.open(archive, "w:gz") as crate:
                entry = tarfile.TarInfo("libc-0.2.190/Cargo.toml")
                entry.size = len(manifest_contents)
                crate.addfile(entry, io.BytesIO(manifest_contents))
            checksum = sha256(archive.read_bytes())
            workspace = root / "workspace"
            workspace.mkdir()
            (workspace / "Cargo.lock").write_text(
                '[[package]]\nname = "libc"\nversion = "0.2.190"\n'
                'source = "registry+https://github.com/rust-lang/crates.io-index"\n'
                f'checksum = "{checksum}"\n'
            )
            package = {
                "name": "libc",
                "version": "0.2.190",
                "source": "registry+https://github.com/rust-lang/crates.io-index",
                "manifest_path": str(manifest),
            }
            metadata = {"workspace_root": str(workspace)}
            allow = {"cargo_registry_root": "${CARGO_HOME}/registry/src"}
            with mock.patch.dict("os.environ", {"CARGO_HOME": str(cargo_home)}):
                self.assertEqual(
                    check_native_free.locked_package_provenance(package, metadata, allow, root),
                    f"registry:{checksum}",
                )
                manifest.write_bytes(b"tampered source")
                self.assertIsNone(
                    check_native_free.locked_package_provenance(package, metadata, allow, root)
                )
                manifest.write_bytes(manifest_contents)
                archive.write_bytes(b"tampered archive")
                self.assertIsNone(
                    check_native_free.locked_package_provenance(package, metadata, allow, root)
                )





class CargoCaptureTests(unittest.TestCase):
    def run_capture(self, root: Path, include_output: bool) -> tuple[int, dict, Path]:
        root = root.resolve()
        build_dir = root / "build-hash"
        out_dir = build_dir / "out"
        out_dir.mkdir(parents=True)
        generated = out_dir / "generated.rs"
        generated.write_bytes(b"generated source")
        output = build_dir / "output"
        if include_output:
            output.write_bytes(b"cargo:rustc-cfg=fixture\n")
        artifact = root / "artifact.rlib"
        artifact.write_bytes(b"compiled artifact")
        messages = [
            {
                "reason": "compiler-artifact",
                "filenames": [str(artifact)],
            },
            {
                "reason": "build-script-executed",
                "out_dir": str(out_dir),
            },
            {"reason": "build-finished", "success": True},
        ]
        stream = "".join(json.dumps(message) + "\n" for message in messages).encode()

        class FakeCargo:
            def __init__(self):
                self.stdout = capture_cargo_provenance.BytesIO(stream)
                self.stderr = capture_cargo_provenance.BytesIO()

            @staticmethod
            def wait():
                return 0

        record_path = root / "Artifacts" / "cargo-provenance.json"
        args = type("Args", (), {"record": str(record_path), "target": "hp2_ucc_rust"})()
        with (
            mock.patch.object(capture_cargo_provenance.Path, "cwd", return_value=root),
            mock.patch.object(capture_cargo_provenance.subprocess, "Popen", return_value=FakeCargo()),
            mock.patch.object(capture_cargo_provenance.sys, "stdout", mock.Mock(buffer=capture_cargo_provenance.BytesIO())),
            mock.patch.object(capture_cargo_provenance.sys, "stderr", mock.Mock(buffer=capture_cargo_provenance.BytesIO())),
        ):
            status = capture_cargo_provenance._capture(args, ["cargo", "rustc"], {})
        return status, json.loads(record_path.read_text()), output

    def test_capture_hashes_cargo_output_sibling(self):
        with tempfile.TemporaryDirectory() as directory:
            status, record, output = self.run_capture(Path(directory), include_output=True)
            self.assertEqual(status, 0)
            self.assertEqual(
                record["build_script_output_sha256"][str(output.resolve())],
                sha256(output.read_bytes()),
            )

    def test_capture_rejects_missing_cargo_output_sibling(self):
        with tempfile.TemporaryDirectory() as directory:
            status, record, _ = self.run_capture(Path(directory), include_output=False)
            self.assertEqual(status, 1)
            self.assertIn("output file is missing or unsafe", record["capture_error"])


class CargoArtifactClosureTests(unittest.TestCase):
    def make_closure(self, root: Path) -> tuple[dict, set[str], dict[str, set[str]], dict, Path, dict]:
        root = root.resolve()
        workspace = root / "workspace"
        root_dir = workspace / "crates" / "hp2-ucc"
        core_dir = workspace / "crates" / "hp2-core"
        root_dir.mkdir(parents=True)
        core_dir.mkdir(parents=True)
        root_source = root_dir / "src" / "main.rs"
        core_source = core_dir / "src" / "lib.rs"
        root_source.parent.mkdir()
        core_source.parent.mkdir()
        root_source.write_text("fn main() {}\n", encoding="utf-8")
        core_source.write_text("pub fn core() {}\n", encoding="utf-8")
        target_dir = root / "cargo-target"
        profile_dir = target_dir / "aarch64-apple-darwin" / "release"
        deps_dir = profile_dir / "deps"
        deps_dir.mkdir(parents=True)
        executable = profile_dir / "hp2_ucc_rust"
        library = deps_dir / "libhp2_core-abc.rlib"
        executable.write_bytes(b"binary")
        library.write_bytes(b"rlib")
        host_deps_dir = target_dir / "release" / "deps"
        host_deps_dir.mkdir(parents=True)
        host_library = host_deps_dir / library.name
        host_library.write_bytes(b"host rlib")
        root_id, core_id = "workspace#hp2-ucc@0.1.0", "workspace#hp2-core@0.1.0"
        root_target = {
            "name": "hp2_ucc_rust", "kind": ["bin"], "crate_types": ["bin"],
            "src_path": str(root_source), "edition": "2021",
            "doc": True, "doctest": False, "test": True,
        }
        core_target = {
            "name": "hp2_core", "kind": ["lib"], "crate_types": ["lib"],
            "src_path": str(core_source), "edition": "2021",
            "doc": True, "doctest": True, "test": True,
        }
        packages = [
            {
                "id": root_id, "name": "hp2-ucc", "version": "0.1.0", "source": None,
                "manifest_path": str(root_dir / "Cargo.toml"), "targets": [root_target],
            },
            {
                "id": core_id, "name": "hp2-core", "version": "0.1.0", "source": None,
                "manifest_path": str(core_dir / "Cargo.toml"), "targets": [core_target],
                "features": {"default": [], "std": []}, "dependencies": [],
            },
        ]
        metadata = {
            "workspace_root": str(workspace),
            "target_directory": str(target_dir),
            "packages": packages,
            "resolve": {
                "nodes": [
                    {"id": root_id, "features": []},
                    {"id": core_id, "features": ["default", "std"]},
                ],
            },
        }
        options = {
            "target_dir": target_dir, "target": "aarch64-apple-darwin",
            "profile": "release", "bin": "hp2_ucc_rust", "package": "hp2-ucc",
            "unit_graph": {"version": 1, "units": [
                {
                    "pkg_id": root_id, "target": root_target, "platform": "aarch64-apple-darwin",
                    "mode": "build", "features": [], "dependencies": [{"index": 1}, {"index": 2}],
                },
                {
                    "pkg_id": core_id, "target": core_target, "platform": "aarch64-apple-darwin",
                    "mode": "build", "features": ["default"], "dependencies": [],
                },
                {
                    "pkg_id": core_id, "target": core_target, "platform": None,
                    "mode": "build", "features": ["std"], "dependencies": [],
                },
            ]},
        }
        events = [
            {
                "reason": "compiler-artifact", "package_id": root_id,
                "target": root_target, "features": [], "fresh": True,
                "profile": {"test": False}, "filenames": [str(executable)],
                "executable": str(executable),
            },
            {
                "reason": "compiler-artifact", "package_id": core_id,
                "target": core_target, "features": ["default"], "fresh": True,
                "profile": {"test": False}, "filenames": [str(library)],
                "executable": None,
            },
            {
                "reason": "compiler-artifact", "package_id": core_id,
                "target": core_target, "features": ["std"], "fresh": True,
                "profile": {"test": False}, "filenames": [str(host_library)],
                "executable": None,
            },
        ]
        record = {"messages": events}
        roles = {root_id: {"target"}, core_id: {"target", "host"}}
        return metadata, {root_id, core_id}, roles, options, root_dir, record

    def inspect_closure(self, fixture):
        metadata, package_ids, roles, options, command_dir, record = fixture
        with mock.patch.object(
            check_native_free, "locked_package_provenance", return_value="workspace",
        ):
            return check_native_free.rust_objects_for_closure(
                metadata, package_ids, roles, options, command_dir,
                record, {}, command_dir.parents[1],
            )

    def test_artifact_events_select_exact_target_and_host_units(self):
        with tempfile.TemporaryDirectory() as directory:
            outputs, symbol_outputs, findings = self.inspect_closure(
                self.make_closure(Path(directory))
            )
        self.assertEqual(findings, [])
        self.assertTrue(any(path.name == "hp2_ucc_rust" for path in outputs))
        self.assertEqual(
            [path.name for path in symbol_outputs],
            ["libhp2_core-abc.rlib", "libhp2_core-abc.rlib"],
        )

    def test_artifact_events_reject_declared_features_selected_for_another_role(self):
        for mutation in ("declared_other_role_feature", "host_path"):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as directory:
                fixture = self.make_closure(Path(directory))
                record = fixture[-1]
                if mutation == "declared_other_role_feature":
                    record["messages"][1]["features"] = ["std"]
                else:
                    target_dir = fixture[0]["target_directory"]
                    event = record["messages"][1]
                    old_library = Path(event["filenames"][0])
                    host_library = Path(target_dir) / "release" / "deps" / old_library.name
                    event["filenames"] = [str(host_library)]
                _, _, findings = self.inspect_closure(fixture)
                self.assertTrue(
                    any("features differ from selected unit graph" in finding for finding in findings),
                    findings,
                )

    def test_missing_artifact_diagnostic_includes_full_unit_identity(self):
        with tempfile.TemporaryDirectory() as directory:
            fixture = self.make_closure(Path(directory))
            fixture[-1]["messages"].pop(1)
            _, _, findings = self.inspect_closure(fixture)
        self.assertIn(
            "selected Cargo compiler-artifact records missing for targets/roles: "
            "hp2-core (workspace#hp2-core@0.1.0):hp2_core:('lib',):target",
            findings,
        )

    def test_metadata_union_cannot_replace_selected_unit_graph(self):
        target = {"name": "game", "kind": ["bin"]}
        metadata = {
            "packages": [{
                "id": "root", "name": "game", "version": "1", "targets": [target],
            }],
            "resolve": {"nodes": [{"id": "root", "features": ["declared-but-unselected"]}]},
        }
        findings, selected, roles = check_native_free.cargo_closure_findings(
            "game", metadata, {"package": "game", "target": "aarch64-apple-darwin"},
        )
        self.assertEqual(selected, set())
        self.assertEqual(roles, {})
        self.assertTrue(any("unit graph is missing" in finding for finding in findings), findings)
    def test_unit_graph_closure_selects_platform_and_host_units(self):
        root_target = {"name": "game", "kind": ["bin"]}
        target_target = {"name": "target_dep", "kind": ["lib"]}
        host_target = {"name": "build_dep", "kind": ["lib"]}
        proc_target = {"name": "proc_dep", "kind": ["proc-macro"]}
        packages = [
            {"id": "root", "name": "game", "version": "1", "targets": [root_target]},
            {"id": "target", "name": "target-dep", "version": "1", "targets": [target_target]},
            {"id": "host", "name": "build-dep", "version": "1", "targets": [host_target]},
            {"id": "proc", "name": "proc-dep", "version": "1", "targets": [proc_target]},
            {"id": "excluded", "name": "windows-only", "version": "1", "targets": [{"kind": ["lib"]}]},
        ]
        metadata = {"packages": packages}
        target_platform = "aarch64-apple-darwin"
        graph = {"version": 1, "units": [
            {
                "pkg_id": "root", "target": root_target, "platform": target_platform,
                "mode": "build", "profile": {"name": "release"}, "features": [],
                "dependencies": [{"index": 1}, {"index": 2}, {"index": 3}],
            },
            {
                "pkg_id": "target", "target": target_target, "platform": target_platform,
                "mode": "build", "profile": {"name": "release"}, "features": [],
                "dependencies": [],
            },
            {
                "pkg_id": "host", "target": host_target, "platform": None,
                "mode": "build", "profile": {"name": "release"}, "features": [],
                "dependencies": [],
            },
            {
                "pkg_id": "proc", "target": proc_target, "platform": None,
                "mode": "build", "profile": {"name": "release"}, "features": [],
                "dependencies": [],
            },
        ]}
        findings, selected, roles = check_native_free.cargo_closure_findings(
            "game", metadata,
            {"package": "game", "target": target_platform, "profile": "release", "unit_graph": graph},
        )
        self.assertEqual(findings, [])
        self.assertEqual(selected, {"root", "target", "host", "proc"})
        self.assertEqual(roles, {
            "root": {"target"}, "target": {"target"},
            "host": {"host"}, "proc": {"host"},
        })


class SelectedCompilerDependencyTests(unittest.TestCase):
    def make_compile_fixture(self, root: Path) -> tuple[Path, Path, Path, Path, str]:
        root = root.resolve()
        build = root / "build"
        build.mkdir()
        source = root / "repo" / "src" / "owner.cpp"
        header = root / "repo" / "include" / "owner.hpp"
        source.parent.mkdir(parents=True)
        header.parent.mkdir(parents=True)
        source.write_text("int owner;\n", encoding="utf-8")
        header.write_text("struct Owner {};\n", encoding="utf-8")
        obj = build / "CMakeFiles" / "owner.dir" / "owner.cpp.o"
        obj.parent.mkdir(parents=True)
        obj.write_bytes(b"object")
        depfile = obj.with_suffix(".d")
        depfile.write_text(f"{obj}: {source} {header}\n{header}:\n", encoding="utf-8")
        command = f"clang++ -c {source} -o {obj} -MD -MF {depfile}"
        (build / "compile_commands.json").write_text(json.dumps([{
            "directory": str(build),
            "file": str(source),
            "output": str(obj),
            "command": command,
        }]), encoding="utf-8")
        ninja_deps = f"{obj.relative_to(build)}: #deps 2, deps mtime 100\n  {source}\n  {header}\n"
        return build, source, header, obj, command + "\n" + ninja_deps

    def run_compile_check(self, build: Path, source: Path, ninja_text: str, ninja_output: str):
        result = subprocess.CompletedProcess(
            ["ninja", "-t", "deps"], 0, stdout=ninja_output, stderr="",
        )
        with mock.patch.object(check_native_free, "native_input_proven", return_value=True), \
                mock.patch.object(check_native_free.subprocess, "run", return_value=result):
            return check_native_free.compile_commands(
                build, build.parent, {source}, {}, {}, set(), None, ninja_text,
            )

    def test_selected_compile_uses_fresh_ninja_dependencies_and_checks_depfile(self):
        with tempfile.TemporaryDirectory() as directory:
            build, source, header, obj, payload = self.make_compile_fixture(Path(directory))
            ninja_text, ninja_deps = payload.split("\n", 1)
            findings = self.run_compile_check(build, source, ninja_text + "\n", ninja_deps)
        self.assertEqual(findings, [])

    def test_selected_compile_rejects_stale_ninja_dependencies(self):
        with tempfile.TemporaryDirectory() as directory:
            build, source, _, _, payload = self.make_compile_fixture(Path(directory))
            ninja_text, ninja_deps = payload.split("\n", 1)
            ninja_deps = ninja_deps.replace("deps mtime 100", "deps mtime 100 (STALE)")
            findings = self.run_compile_check(build, source, ninja_text + "\n", ninja_deps)
        self.assertTrue(any("stale" in finding.lower() for finding in findings), findings)

    def test_selected_compile_rejects_depfile_disagreeing_with_ninja_record(self):
        with tempfile.TemporaryDirectory() as directory:
            build, source, header, _, payload = self.make_compile_fixture(Path(directory))
            (header.parent / "different.hpp").write_text("struct Other {};\n", encoding="utf-8")
            ninja_text, ninja_deps = payload.split("\n", 1)
            ninja_deps = ninja_deps.replace("owner.hpp", "different.hpp")
            findings = self.run_compile_check(build, source, ninja_text + "\n", ninja_deps)
        self.assertTrue(any("differs from retained Ninja dependencies" in finding for finding in findings), findings)

    def test_selected_compile_accepts_valid_retained_ninja_record_without_depfile(self):
        with tempfile.TemporaryDirectory() as directory:
            build, source, _, obj, payload = self.make_compile_fixture(Path(directory))
            obj.with_suffix(".d").unlink()
            ninja_text, ninja_deps = payload.split("\n", 1)
            findings = self.run_compile_check(build, source, ninja_text + "\n", ninja_deps)
        self.assertEqual(findings, [])

    def test_selected_compile_rejects_missing_retained_header(self):
        with tempfile.TemporaryDirectory() as directory:
            build, source, header, _, payload = self.make_compile_fixture(Path(directory))
            header.unlink()
            ninja_text, ninja_deps = payload.split("\n", 1)
            findings = self.run_compile_check(build, source, ninja_text + "\n", ninja_deps)
        self.assertTrue(any("missing inputs" in finding for finding in findings), findings)


class NativeFreePlatformProvenanceTests(unittest.TestCase):
    def test_compiler_resource_headers_come_from_configured_compiler(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            build = root / "build"
            build.mkdir()
            resource = root / "toolchain" / "lib" / "clang" / "21"
            (resource / "include").mkdir(parents=True)
            header = resource / "include" / "stddef.h"
            header.write_text("compiler header\n", encoding="utf-8")
            unrelated = root / "include" / "stddef.h"
            unrelated.parent.mkdir()
            unrelated.write_text("not compiler-owned\n", encoding="utf-8")
            compiler = root / "clang"
            (build / "CMakeCache.txt").write_text(
                f"CMAKE_C_COMPILER:FILEPATH={compiler}\n"
                "CMAKE_CXX_COMPILER:FILEPATH=\n",
                encoding="utf-8",
            )
            result = subprocess.CompletedProcess(
                [str(compiler), "-print-resource-dir"], 0, str(resource), "",
            )
            check_native_free.clang_resource_roots.cache_clear()
            try:
                with mock.patch.object(check_native_free.subprocess, "run", return_value=result) as run:
                    self.assertTrue(check_native_free.clang_resource_header(header, build))
                    self.assertFalse(check_native_free.clang_resource_header(unrelated, build))
                run.assert_called_once_with(
                    [str(compiler), "-print-resource-dir"],
                    text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                )
            finally:
                check_native_free.clang_resource_roots.cache_clear()

    def test_cmake_sysroot_resolves_only_its_apple_frameworks(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            sdk = root / "SDK"
            (sdk / "System/Library/Frameworks/AppKit.framework").mkdir(parents=True)
            (sdk / "usr/lib").mkdir(parents=True)
            build = root / "build"
            build.mkdir()
            (build / "CMakeCache.txt").write_text(
                f"CMAKE_OSX_SYSROOT:PATH={sdk}\n", encoding="utf-8",
            )
            with mock.patch.dict(check_native_free.os.environ, {"SDKROOT": ""}):
                resolved_sdk = check_native_free.apple_sdk_root(build, {})
            common = (
                root, "hp2_game_rust", set(), {}, {}, set(), set(),
                root, build, {}, {}, set(), resolved_sdk,
            )
            self.assertEqual(
                check_native_free.audit_link_inputs(["-lframework=AppKit"], *common),
                [],
            )
            findings = check_native_free.audit_link_inputs(
                ["-lframework=UnlistedFramework"], *common,
            )
        self.assertTrue(any("unresolved Apple framework input" in finding for finding in findings), findings)

    def test_generated_native_header_requires_pinned_producer_inputs_and_output_hash(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            build, repo = root / "build", root / "repo"
            source_root = build / "_deps" / "hp2_openal_soft-src"
            output = build / "_deps" / "hp2_openal_soft-build" / "config.h"
            template = source_root / "config.h.in"
            producer = source_root / "CMakeLists.txt"
            source_root.mkdir(parents=True)
            output.parent.mkdir(parents=True)
            template.write_text("#define OPENAL_CONFIG 1\n", encoding="utf-8")
            producer.write_text(
                'configure_file("${OpenAL_SOURCE_DIR}/config.h.in" "${OpenAL_BINARY_DIR}/config.h")\n',
                encoding="utf-8",
            )
            output.write_text("#define OPENAL_CONFIG 1\n", encoding="utf-8")
            digest = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
            spec = {
                "target": "OpenAL",
                "path": "_deps/hp2_openal_soft-build/config.h",
                "template_path": "_deps/hp2_openal_soft-src/config.h.in",
                "source": "OpenAL Soft",
                "producer_cmake": "CMakeLists.txt",
                "input": "config.h.in",
            }
            allow = {
                "pinned_source_roots": [{
                    "root": "_deps/hp2_openal_soft-src", "source": "OpenAL Soft",
                }],
                "generated_native_headers": [spec],
            }
            record = {
                "target": "OpenAL",
                "path": spec["path"],
                "template_path": str(template),
                "source_dir": str(source_root),
                "cmake_source": str(producer),
                "input_source": str(template),
                "cmake_sha256": digest(producer),
                "input_sha256": digest(template),
                "template_sha256": digest(template),
                "sha256": digest(output),
            }
            alias = root / "build-alias"
            alias.symlink_to(build, target_is_directory=True)
            aliased_output = alias / output.relative_to(build)
            with mock.patch.object(check_native_free, "pinned_generated_source", return_value=True):
                self.assertTrue(check_native_free.generated_native_header(
                    output.resolve(), build, repo, allow, [record], {"OpenAL"},
                ))
                self.assertTrue(check_native_free.generated_native_header(
                    aliased_output, alias, repo, allow, [record], {"OpenAL"},
                ))
                output.write_text("tampered header\n", encoding="utf-8")
                self.assertFalse(check_native_free.generated_native_header(
                    output.resolve(), build, repo, allow, [record], {"OpenAL"},
                ))


class CMakeObjectProducerTests(unittest.TestCase):
    def make_closure(self, root: Path, *, reachable=True, duplicate=False, source=True, owner=True):
        root = root.resolve()
        repo = root / "checkout"
        build = repo / "out" / "macos-arm64"
        reply = build / ".cmake/api/v1/reply"
        reply.mkdir(parents=True)
        cpp = build / "_deps/fmt-src/src/format.cc"
        cpp.parent.mkdir(parents=True)
        cpp.write_text("int external_format() { return 1; }\n", encoding="utf-8")
        obj = build / "_deps/fmt-build/CMakeFiles/fmt.dir/src/format.cc.o"
        obj.parent.mkdir(parents=True)
        obj.write_bytes(b"compiled external object")
        targets = {
            "product": {"name": "game", "type": "UTILITY", "dependencies": [{"id": "consumer"}]},
            "consumer": {
                "name": "external", "type": "STATIC_LIBRARY",
                "dependencies": [{"id": "fmt"}] if reachable else [],
                "sources": [{"path": str(obj.relative_to(repo)), "isGenerated": True}],
            },
            "fmt": {
                "name": "fmt", "type": "OBJECT_LIBRARY",
                "sources": [{"path": str(cpp.relative_to(repo))}] if source else [],
                "artifacts": [{"path": str(obj.relative_to(build))}] if owner else [],
            },
        }
        if duplicate:
            targets["other"] = {**targets["fmt"], "name": "other"}
        records = []
        for target_id, details in targets.items():
            filename = target_id + ".json"
            (reply / filename).write_text(json.dumps(details), encoding="utf-8")
            records.append({"id": target_id, "name": details["name"], "jsonFile": filename})
        (reply / "codemodel.json").write_text(json.dumps({
            "paths": {"source": str(repo), "build": str(build)},
            "configurations": [{"targets": records}],
        }), encoding="utf-8")
        (reply / "index-fixture.json").write_text(json.dumps({
            "reply": {"codemodel-v2": {"kind": "codemodel", "jsonFile": "codemodel.json"}},
        }), encoding="utf-8")
        return build, cpp, obj

    def test_generated_object_alias_retains_its_reachable_producer_sources(self):
        with tempfile.TemporaryDirectory() as directory:
            build, cpp, obj = self.make_closure(Path(directory))
            findings, sources, artifacts, targets = check_native_free.file_api_closure(build, "game", [])
            self.assertEqual(findings, [])
            self.assertEqual(sources, {cpp})
            self.assertIn(obj, artifacts)
            self.assertIn("fmt", targets)

    def test_symlinked_build_root_keeps_canonical_producer_identity(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            build, cpp, obj = self.make_closure(root)
            alias = root / "build-alias"
            alias.symlink_to(build, target_is_directory=True)
            findings, sources, artifacts, targets = check_native_free.file_api_closure(alias, "game", [])
            self.assertEqual(findings, [])
            self.assertEqual(sources, {cpp})
            self.assertIn(obj, artifacts)
            self.assertIn("fmt", targets)

    def test_object_without_unique_reachable_compilation_owner_remains_unproven(self):
        for options in [
            {"reachable": False}, {"duplicate": True}, {"source": False}, {"owner": False},
        ]:
            with self.subTest(options=options), tempfile.TemporaryDirectory() as directory:
                build, _, obj = self.make_closure(Path(directory), **options)
                _, sources, _, _ = check_native_free.file_api_closure(build, "game", [])
                self.assertIn(obj, sources)


class NativeHeaderCopyProofTests(unittest.TestCase):
    def test_copied_header_requires_selected_target_and_unchanged_pinned_inputs(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            build, repo = root / "build", root / "repo"
            source = build / "_deps/sdl-src"
            output_root = build / "_deps/sdl-build/include/SDL2"
            (source / "include").mkdir(parents=True)
            output_root.mkdir(parents=True)
            cmake = source / "CMakeLists.txt"
            cmake.write_text("configure_file(input output COPYONLY)\n", encoding="utf-8")
            original = source / "include/SDL_events.h"
            original.write_bytes(b"pinned SDL event declarations\n")
            output = output_root / original.name
            output.write_bytes(original.read_bytes())
            trusted = {path: sha256(path.read_bytes()) for path in (cmake, original)}
            def pinned(path, *_):
                return path in trusted and sha256(path.read_bytes()) == trusted[path]
            allow = {
                "pinned_source_roots": [{"root": str(source), "source": "SDL2"}],
                "generated_native_headers": [{
                    "target": "SDL2-static", "mode": "copy", "source": "SDL2",
                    "path_prefix": str(output_root), "input_prefix": "include",
                    "producer_cmake": "CMakeLists.txt",
                }],
            }
            def proven(targets={"SDL2-static"}):
                return check_native_free.generated_native_header(output, build, repo, allow, [], targets)
            with mock.patch.object(check_native_free, "pinned_generated_source", side_effect=pinned):
                self.assertTrue(proven())
                self.assertFalse(proven(set()))
                output.write_bytes(b"modified copied header\n")
                self.assertFalse(proven())
                original.write_bytes(output.read_bytes())
                self.assertFalse(proven(), "equal modified bytes cannot replace pinned source identity")
                original.write_bytes(b"pinned SDL event declarations\n")
                output.write_bytes(original.read_bytes())
                cmake.write_text("unrelated producer\n", encoding="utf-8")
                self.assertFalse(proven())

    def test_configuration_and_revision_headers_cannot_use_copy_proof(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            source, output_root = root / "source", root / "output"
            (source / "include").mkdir(parents=True)
            output_root.mkdir()
            allow = {
                "pinned_source_roots": [{"root": str(source), "source": "SDL2"}],
                "generated_native_headers": [{
                    "target": "SDL2-static", "mode": "copy", "source": "SDL2",
                    "path_prefix": str(output_root), "input_prefix": "include",
                    "producer_cmake": "CMakeLists.txt",
                }],
            }
            (source / "CMakeLists.txt").write_bytes(b"pinned producer")
            with mock.patch.object(check_native_free, "pinned_generated_source", return_value=True):
                for name in ["SDL_config.h", "SDL_revision.h"]:
                    with self.subTest(name=name):
                        (source / "include" / name).write_bytes(b"configuration output")
                        output = output_root / name
                        output.write_bytes(b"configuration output")
                        self.assertFalse(check_native_free.generated_native_header(
                            output, root, root, allow, [], {"SDL2-static"},
                        ))


if __name__ == "__main__":
    unittest.main()
