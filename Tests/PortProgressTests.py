#!/usr/bin/env python3
"""Behavioral contracts for the Rust port scoreboard (no game data)."""
from __future__ import annotations

import hashlib
import json
import importlib.util
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location("port_progress", Path(__file__).resolve().parents[1] / "Build/port_progress.py")
progress = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(progress)


class PortProgressTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        subprocess.run(["git", "init", "-q", str(self.root)], check=True)

    def put(self, path, text="x\n\n y\n"):
        target = self.root / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(text)

    def test_suffixes_roots_nonblank_and_tracked_other_files(self):
        for name in ["a.c", "b.cc", "c.cpp", "d.cxx", "e.m", "f.mm", "a.h", "b.hpp", "c.inl", "d.inc", "x.lib", "y.a", "z.o", "ignore.uc"]:
            self.put("HarryPotter2/Unreal/Engine/" + name)
        for name in ["a.cpp", "b.h", "c.mm", "ignore.c"]:
            self.put("Tests/" + name)
        self.put("Build/demo_fixture_harness/nested/test.cxx")
        self.put("Build/demo_fixture_harness/test.h")
        self.put("Build/not_a_harness/test.c")
        self.put("ThirdParty/vendor/test.c")
        self.put("crates/tools/test.c")
        self.put("crates/hp2-ffi/src/lib.rs", "#[unsafe(no_mangle)]\npub fn a() {}\n#[no_mangle]\npub fn b() {}\n")
        subprocess.run(["git", "-C", str(self.root), "add", "."], check=True)
        self.put("untracked.c")
        counts = progress.static_counters(self.root)
        self.assertEqual(counts["native_engine_sources"], 6)
        self.assertEqual(counts["native_engine_headers"], 4)
        self.assertEqual(counts["native_engine_nonblank_lines"], 20)
        self.assertEqual(counts["native_test_files"], 3)
        self.assertEqual(counts["native_harness_files"], 2)
        self.assertEqual(counts["native_other_files"], 2)
        self.assertEqual(counts["native_binaries"], 3)
        self.assertEqual(counts["ffi_exports"], {"hp2-ffi": 2})
        self.assertIs(counts["ffi_crate_present"], True)

    def test_indexed_registry_resolution_and_grouped_missing(self):
        self.put("crates/hp2-engine/src/engine_vm_native.rs", '''
const UNRELATED: VmNativeSpec = VmNativeSpec { class_name: "Engine.Pawn", function_name: "Missing" };
pub static ENGINE_VM_NATIVES: &[VmNativeSpec] = &[
 VmNativeSpec { class_name: "Engine.actor", function_name: "playsound" },
 crate::canvas_vm_native::NATIVE_FUNCTIONS[1],
];
''')
        self.put("crates/hp2-engine/src/canvas_vm_native.rs", '''
const UNRELATED: VmNativeSpec = VmNativeSpec { class_name: "Engine.Canvas", function_name: "Wrong" };
pub const NATIVE_FUNCTIONS: [VmNativeSpec; 2] = [
 VmNativeSpec { class_name: "Engine.Canvas", function_name: "NotReferenced" },
 VmNativeSpec { class_name: "Engine.Canvas", function_name: "DrawText" },
];
''')
        rows = [
            ("Actor", "PlaySound", "actor.rs"),
            ("Canvas", "DrawText", "render.rs"),
            ("Canvas", "NotReferenced", "render.rs"),
            ("Pawn", "Missing", "pawn.rs"),
            ("Pawn", "Another", "pawn.rs"),
        ]
        inventory = {"version": 1, "natives": [dict(class_=c, function=f, legacy_file=source) for c, f, source in rows]}
        for row in inventory["natives"]:
            row["class"] = row.pop("class_")
        coverage = progress.native_coverage(self.root, inventory)
        self.assertEqual(coverage["engine_natives_bound"], 2)
        self.assertEqual(coverage["engine_natives_total"], 5)
        self.assertEqual(coverage["engine_natives_missing"], {
            "render.rs": ["Canvas.NotReferenced"],
            "pawn.rs": ["Pawn.Another", "Pawn.Missing"],
        })

    def test_nested_references_preserve_interleaved_array_positions(self):
        self.put("crates/hp2-engine/src/engine_vm_native.rs", '''
pub static ENGINE_VM_NATIVES: &[VmNativeSpec] = &[
 crate::console_vm_native::NATIVE_FUNCTIONS[0],
 crate::console_vm_native::NATIVE_FUNCTIONS[2],
];
''')
        self.put("crates/hp2-engine/src/console_vm_native.rs", '''
pub const NATIVE_FUNCTIONS: [VmNativeSpec; 3] = [
 crate::texture_vm_native::NATIVE_FUNCTIONS[1],
 VmNativeSpec { class_name: "Engine.Actor", function_name: "NotReferenced" },
 crate::texture_vm_native::NATIVE_FUNCTIONS[0],
];
''')
        self.put("crates/hp2-engine/src/texture_vm_native.rs", '''
pub const NATIVE_FUNCTIONS: [VmNativeSpec; 2] = [
 VmNativeSpec { class_name: "Engine.Actor", function_name: "TraceTexture" },
 VmNativeSpec { class_name: "Engine.ScriptedTexture", function_name: "DrawText" },
];
''')
        inventory = {"version": 1, "natives": [
            {"class": cls, "function": name, "legacy_file": "render.rs"}
            for cls, name in [("Actor", "TraceTexture"), ("ScriptedTexture", "DrawText"), ("Actor", "NotReferenced")]
        ]}
        coverage = progress.native_coverage(self.root, inventory)
        self.assertEqual(coverage["engine_natives_bound"], 2)
        self.assertEqual(coverage["engine_natives_missing"], {"render.rs": ["Actor.NotReferenced"]})

    def test_referenced_native_cycle_fails_instead_of_reporting_coverage(self):
        self.put("crates/hp2-engine/src/engine_vm_native.rs", '''
pub static ENGINE_VM_NATIVES: &[VmNativeSpec] = &[crate::cycle::NATIVE_FUNCTIONS[0]];
''')
        self.put("crates/hp2-engine/src/cycle.rs", '''
pub const NATIVE_FUNCTIONS: [VmNativeSpec; 1] = [crate::cycle::NATIVE_FUNCTIONS[0]];
''')
        inventory = {"version": 1, "natives": [{"class": "Actor", "function": "TraceTexture", "legacy_file": "render.rs"}]}
        with self.assertRaisesRegex(ValueError, "cyclic native reference"):
            progress.native_coverage(self.root, inventory)

    def test_absent_ctest_is_not_run_or_reported_green(self):
        calls = []
        def runner(args, **kwargs):
            calls.append(args)
            return subprocess.CompletedProcess(args, 8, "Total Tests: 0\n", "No tests were found!!!")
        self.assertEqual(progress.ctest_status(self.root, "macos-arm64", "missing", runner=runner), "not-registered")
        self.assertEqual(len(calls), 1)
        self.assertIn("-N", calls[0])

    def test_registered_ctest_failure_is_retained(self):
        calls = []
        def runner(args, **kwargs):
            calls.append(args)
            if "-N" in args:
                return subprocess.CompletedProcess(args, 0, "  Test #42: actual\nTotal Tests: 1\n", "")
            return subprocess.CompletedProcess(args, 8, "actual failed", "")
        self.assertEqual(progress.ctest_status(self.root, "macos-arm64", "actual", runner=runner), "fail")
        self.assertEqual(len(calls), 2)

    def evaluate(self, functional=None, parity=None, run=False, human=None):
        counters = {"ffi_crate_present": False, "native_engine_sources": 0, "ffi_exports": {}}
        with patch.object(progress, "ctest_status", return_value="pass"), patch.object(
            progress.subprocess, "run", return_value=subprocess.CompletedProcess([], 0, "", ""),
        ):
            return progress.evaluate_gates(
                self.root, counters, parity or [], human or self.root / "human.json",
                "macos-arm64", run, functional or [],
            )

    def functional_records(self):
        return [{"profile": profile, "checks": {name: {"status": "pass"} for name in names}}
                for profile, names in progress.FUNCTIONAL_REQUIREMENTS.values()]

    def test_exact_parity_cannot_promote_or_block_functional_gate(self):
        for passed in (0, 88):
            parity = [{"profile": "data-prototype", "passed_count": passed, "total": 88}]
            self.assertEqual(self.evaluate(parity=parity)["G4"]["status"], "not-evaluated")
            self.assertEqual(self.evaluate(self.functional_records(), parity)["G4"]["status"], "pass")

    def test_missing_projectile_or_cross_process_outcome_prevents_acceptance(self):
        records = self.functional_records()
        records[0]["checks"].pop("projectile_hit")
        records[1]["checks"].pop("cross_process_persistence")
        gates = self.evaluate(records, run=True)
        self.assertEqual(gates["G4"]["status"], "not-evaluated")
        self.assertEqual(gates["G5"]["status"], "not-evaluated")
        self.assertEqual(gates["G7"]["status"], "not-evaluated")
        records[0]["checks"]["projectile_hit"] = {"status": "fail"}
        self.assertEqual(self.evaluate(records, run=True)["G7"]["status"], "fail")

    def test_native_deletion_requires_identified_installed_product_human_check(self):
        self.put("installed-game", "real product identity\n")
        executable = self.root / "installed-game"
        human = self.root / "human.json"
        record = {"version": 1, "items": {name: True for name in progress.HUMAN_ITEMS},
                  "observer": "tester", "date": "2026-11-05",
                  "installed_product": {"executable": "installed-game", "sha256": hashlib.sha256(executable.read_bytes()).hexdigest()}}
        human.write_text(json.dumps(record))
        self.assertEqual(self.evaluate(self.functional_records(), run=True)["G8"]["status"], "pass")
        record["items"]["installed_rust_gameplay"] = False
        human.write_text(json.dumps(record))
        self.assertEqual(self.evaluate(self.functional_records(), run=True)["G8"]["status"], "fail")
        record["items"]["installed_rust_gameplay"] = True
        record["observer"] = None
        human.write_text(json.dumps(record))
        self.assertEqual(self.evaluate(self.functional_records(), run=True)["G8"]["status"], "fail")
        record["observer"] = "tester"
        human.write_text(json.dumps(record))
        executable.write_text("different installed binary\n")
        self.assertEqual(self.evaluate(self.functional_records(), run=True)["G8"]["status"], "fail")

    def test_retail_evidence_does_not_substitute_for_prototype(self):
        retail = [{"profile": "data-retail", "checks": {name: {"status": "pass"}
                    for name in progress.FUNCTIONAL_REQUIREMENTS["G10"][1]}}]
        gates = self.evaluate(retail)
        self.assertEqual(gates["G10"]["status"], "pass")
        self.assertEqual(gates["G4"]["status"], "not-evaluated")

    def test_functional_evidence_requires_unchanged_product_and_real_artifacts(self):
        self.put("game", "product\n")
        self.put("observation.json", '{"projectile_hit":true}\n')
        report = {"version": 1, "profile": "data-prototype", "executable": "game",
                  "executable_sha256": hashlib.sha256((self.root / "game").read_bytes()).hexdigest(),
                  "checks": {"projectile_hit": {"status": "pass", "command": ["game", "scenario"],
                    "observations": ["projectile struck the target; target health decreased"],
                    "artifacts": [{"path": "observation.json", "sha256": hashlib.sha256((self.root / "observation.json").read_bytes()).hexdigest()}]}}}
        path = self.root / "functional.json"
        path.write_text(json.dumps(report))
        self.assertEqual(progress.functional_summary(path, self.root)["checks"]["projectile_hit"]["status"], "pass")
        report["checks"]["projectile_hit"]["observations"] = []
        path.write_text(json.dumps(report))
        with self.assertRaisesRegex(ValueError, "observed outcomes"):
            progress.functional_summary(path, self.root)
        report["checks"]["projectile_hit"]["observations"] = ["observed outcome"]
        path.write_text(json.dumps(report))
        (self.root / "observation.json").write_text("changed evidence")
        with self.assertRaisesRegex(ValueError, "artifact identity mismatch"):
            progress.functional_summary(path, self.root)
        (self.root / "game").write_text("changed product")
        with self.assertRaisesRegex(ValueError, "executable identity mismatch"):
            progress.functional_summary(path, self.root)


if __name__ == "__main__":
    unittest.main()
