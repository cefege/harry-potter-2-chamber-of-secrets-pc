#!/usr/bin/env python3
"""Isolated tests for the agent contract checker; no Cargo or CTest execution."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

CHECKER = Path(__file__).resolve().parents[1] / "Build/check_agent_contracts.py"


class AgentContractTests(unittest.TestCase):
    def setUp(self):
        artifact_dir = os.environ.get("HP2_ARTIFACT_DIR")
        if not artifact_dir:
            self.fail("HP2_ARTIFACT_DIR must name the fixture parent")
        Path(artifact_dir).mkdir(parents=True, exist_ok=True)
        self.temp = tempfile.TemporaryDirectory(prefix="agent-contract-", dir=artifact_dir)
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.repo = self.root / "repo"
        workflow = self.repo / ".github/workflows/ci.yml"
        workflow.parent.mkdir(parents=True)
        workflow.write_text("""name: ci
jobs:
  build:
    steps:
      - run: cargo test --workspace
      - run: cargo clippy -p hp2-ffi --features renderer-wgpu --all-targets -- -D warnings
  web:
    steps:
      - uses: actions/checkout@v4
        with:
          ref: web-cpp-freeze
      - run: cargo test -p old-frozen-package
""", encoding="utf-8")
        self.metadata = {"packages": [
            {"id": "path+file:///repo/crates/hp2-ffi#0.1.0", "name": "hp2-ffi", "features": {"renderer-wgpu": ["dep:hp2-wgpu"]}},
            {"id": "path+file:///repo/crates/hp2-core#0.1.0", "name": "hp2-core", "features": {}}
        ], "workspace_members": ["path+file:///repo/crates/hp2-ffi#0.1.0", "path+file:///repo/crates/hp2-core#0.1.0"]}
        self.skill_root = self.repo / ".agents/skills"
        self.lock_path = self.skill_root / "beautiful-rust.lock.json"
        self.lock = {"schema_version": 1, "commit": "a" * 40, "files": []}
        for name in ("LICENSE.BeautifulRust", "rust-best-practices/SKILL.md", "rust-review/SKILL.md", "rust-port/SKILL.md",
                     "upstream/beautiful-rust-lints.toml", "upstream/beautiful-rust-clippy.toml",
                     "rust-best-practices/references/bevy.md", "rust-best-practices/references/effective-rust.md",
                     "rust-best-practices/references/google-rust-style-guide.md", "rust-best-practices/references/microsoft-rust-guidelines.md",
                     "rust-best-practices/references/rust-api-guidelines.md", "rust-best-practices/references/rust-design-patterns.md"):
            path = self.skill_root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            data = ("fixture " + name).encode()
            path.write_bytes(data)
            self.lock["files"].append({"path": name, "sha256": hashlib.sha256(data).hexdigest()})
        self.save_lock()
        self.listing = {"kind": "ctestInfo", "version": {"major": 1, "minor": 0}, "tests": [
            {"name": "alpha", "command": ["/bin/alpha"]},
            {"name": "beta", "command": ["/bin/beta", "arg"]}]}

    def save_lock(self):
        self.lock_path.write_text(json.dumps(self.lock), encoding="utf-8")

    def run_checker(self, required=("alpha", "beta"), metadata=None, listing=None, include_listing=True, labels=()):
        meta = self.root / "metadata.json"
        ctest = self.root / "ctest.json"
        report = self.root / "report.json"
        meta.write_text(json.dumps(self.metadata if metadata is None else metadata), encoding="utf-8")
        ctest.write_text(json.dumps(self.listing if listing is None else listing), encoding="utf-8")
        command = [sys.executable, str(CHECKER), "--repo", str(self.repo), "--metadata-json", str(meta), "--output", str(report)]
        if include_listing:
            command += ["--ctest-json", str(ctest)]
        for name in required:
            command += ["--require-test", name]
        for label in labels:
            command += ["--require-label", label]
        result = subprocess.run(command, capture_output=True, text=True, check=False)
        self.assertTrue(report.exists(), result.stderr)
        data = json.loads(report.read_text(encoding="utf-8"))
        self.assertEqual(data["version"], 1)
        self.assertEqual(data["ok"], result.returncode == 0)
        return result, data

    def assert_failure(self, data, phrase):
        self.assertFalse(data["ok"])
        self.assertTrue(any(phrase in item for item in data["findings"]), data)

    def test_valid_current_workspace_metadata_shape(self):
        result, report = self.run_checker()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(report["findings"], [])
        self.assertEqual(report["checks"], {"cargo": "pass", "skills": "pass", "ctest": "pass"})
        self.assertEqual(report["inputs"]["metadata_fixture"], str(self.root / "metadata.json"))
        self.assertEqual(report["inputs"]["workflow"], str(self.repo / ".github/workflows/ci.yml"))
        self.assertEqual(report["inputs"]["skill_lock"], str(self.lock_path))

    def test_missing_package(self):
        metadata = {"packages": self.metadata["packages"][1:], "workspace_members": self.metadata["workspace_members"][1:]}
        result, report = self.run_checker(metadata=metadata)
        self.assertEqual(result.returncode, 1)
        self.assert_failure(report, "unknown workspace package hp2-ffi")

    def test_bad_feature(self):
        self.metadata["packages"][0]["features"] = {}
        result, report = self.run_checker()
        self.assertEqual(result.returncode, 1)
        self.assert_failure(report, "unknown or ambiguous feature renderer-wgpu")

    def test_malformed_metadata(self):
        for metadata in ({}, {"packages": [], "workspace_members": ["absent"]}, {"packages": "bad", "workspace_members": []}):
            with self.subTest(metadata=metadata):
                result, report = self.run_checker(metadata=metadata)
                self.assertEqual(result.returncode, 1)
                self.assert_failure(report, "Cargo/workflow")

    def test_malformed_and_missing_listings(self):
        for listing in ({}, {"tests": "bad"}, {"tests": [None]}):
            with self.subTest(listing=listing):
                result, report = self.run_checker(listing=listing)
                self.assertEqual(result.returncode, 1)
                self.assertFalse(report["ok"])
        result, report = self.run_checker(include_listing=False)
        self.assertEqual(result.returncode, 1)
        self.assert_failure(report, "need --ctest-json")

    def test_partial_match_omission(self):
        result, report = self.run_checker(listing={"tests": [{"name": "alpha", "command": ["a"]}, {"name": "beta-extra", "command": ["b"]}]})
        self.assertEqual(result.returncode, 1)
        self.assert_failure(report, "required CTest missing: beta")

    def test_empty_selection_and_command(self):
        for listing, phrase in (({"tests": []}, "required CTest missing"),
                                ({"tests": [{"name": "alpha", "command": []}]}, "empty or malformed command"),
                                ({"tests": [{"name": "alpha", "command": [" "]}]}, "empty or malformed command")):
            with self.subTest(listing=listing):
                result, report = self.run_checker(required=("alpha",), listing=listing)
                self.assertEqual(result.returncode, 1)
                self.assert_failure(report, phrase)

    def test_duplicate_tests(self):
        self.listing["tests"].append({"name": "alpha", "command": ["other"]})
        result, report = self.run_checker()
        self.assertEqual(result.returncode, 1)
        self.assert_failure(report, "duplicate CTest name: alpha")

    def test_disabled_and_malformed_properties(self):
        cases = [([{"name": "DISABLED", "value": True}], "is DISABLED"),
                 ([{"name": "DISABLED", "value": "ON"}], "is DISABLED"),
                 (None, "malformed properties"),
                 ([{}], "malformed properties"),
                 ([{"name": "DISABLED", "value": []}], "malformed DISABLED"),
                 ([{"name": "DISABLED", "value": False}, {"name": "DISABLED", "value": False}], "duplicate DISABLED")]
        for properties, phrase in cases:
            with self.subTest(properties=properties):
                listing = {"tests": [{"name": "alpha", "command": ["a"], "properties": properties}]}
                result, report = self.run_checker(required=("alpha",), listing=listing)
                self.assertEqual(result.returncode, 1)
                self.assert_failure(report, phrase)
        result, report = self.run_checker(required=("alpha",), listing={"tests": [{"name": "alpha", "command": ["a"], "properties": [{"name": "DISABLED", "value": False}]}]})
        self.assertEqual(result.returncode, 0, report)

    def test_required_tests_remain_in_labeled_selection(self):
        for test in self.listing["tests"]:
            test["properties"] = [{"name": "LABELS", "value": ["fast", "data-none"]}]
        result, report = self.run_checker(labels=("data-none",))
        self.assertEqual(result.returncode, 0, report)
        self.assertEqual(report["required_labels"], ["data-none"])
        self.listing["tests"][1]["properties"][0]["value"] = ["fast"]
        result, report = self.run_checker(labels=("data-none",))
        self.assertEqual(result.returncode, 1)
        self.assert_failure(report, "required CTest missing label data-none: beta")
        self.listing["tests"][1].pop("properties")
        result, report = self.run_checker(labels=("data-none",))
        self.assertEqual(result.returncode, 1)
        self.assert_failure(report, "required CTest missing label data-none: beta")

    def test_malformed_label_selection_fails_closed(self):
        for value in ("data-none", [None], [""], [" "]):
            with self.subTest(value=value):
                listing = {"tests": [{"name": "alpha", "command": ["a"],
                                      "properties": [{"name": "LABELS", "value": value}]}]}
                result, report = self.run_checker(required=("alpha",), listing=listing, labels=("data-none",))
                self.assertEqual(result.returncode, 1)
                self.assert_failure(report, "malformed LABELS property")
        prop = {"name": "LABELS", "value": ["data-none"]}
        listing = {"tests": [{"name": "alpha", "command": ["a"], "properties": [prop, prop]}]}
        result, report = self.run_checker(required=("alpha",), listing=listing, labels=("data-none",))
        self.assertEqual(result.returncode, 1)
        self.assert_failure(report, "duplicate LABELS property")
        result, report = self.run_checker(required=(), labels=("data-none",), include_listing=False)
        self.assertEqual(result.returncode, 1)
        self.assert_failure(report, "--require-label needs at least one --require-test")
        self.assertEqual(report["checks"]["ctest"], "fail")

    def test_lock_invalid_or_missing(self):
        mutations = [lambda lock: lock.update(schema_version=True),
                     lambda lock: lock.update(commit="bad"),
                     lambda lock: lock.update(files=[]),
                     lambda lock: lock["files"][0].update(sha256="bad"),
                     lambda lock: lock["files"].append(dict(lock["files"][0])),
                     lambda lock: lock["files"].pop(),
                     lambda lock: lock["files"][0].update(path="../escape"),
                     lambda lock: lock["files"][0].update(path="/absolute"),
                     lambda lock: lock["files"][0].update(path="missing")]
        original = json.loads(json.dumps(self.lock))
        for mutate in mutations:
            with self.subTest(mutation=mutate):
                self.lock = json.loads(json.dumps(original))
                mutate(self.lock)
                self.save_lock()
                result, report = self.run_checker()
                self.assertEqual(result.returncode, 1, report)
                self.assertEqual(report["checks"]["skills"], "fail")
        self.lock_path.write_text("{bad", encoding="utf-8")
        result, report = self.run_checker()
        self.assertEqual(result.returncode, 1)
        self.assert_failure(report, "skill lock:")
        self.lock_path.unlink()
        result, report = self.run_checker()
        self.assertEqual(result.returncode, 1)
        self.assert_failure(report, "skill lock:")

    def test_lock_requires_reference_and_lint_inventory(self):
        original = list(self.lock["files"])
        for name in ("rust-best-practices/references/bevy.md", "upstream/beautiful-rust-lints.toml"):
            with self.subTest(missing=name):
                self.lock["files"] = [entry for entry in original if entry["path"] != name]
                self.save_lock()
                result, report = self.run_checker()
                self.assertEqual(result.returncode, 1)
                self.assert_failure(report, "required skill file missing from lock: " + name)

    def test_lock_hash_mismatch_and_symlink_escape(self):
        target = self.skill_root / "LICENSE.BeautifulRust"
        target.write_text("changed", encoding="utf-8")
        result, report = self.run_checker()
        self.assertEqual(result.returncode, 1)
        self.assert_failure(report, "skill hash mismatch")
        target.unlink()
        outside = self.root / "outside"
        outside.write_text("outside", encoding="utf-8")
        target.symlink_to(outside)
        result, report = self.run_checker()
        self.assertEqual(result.returncode, 1)
        self.assert_failure(report, "escapes skill root")

    def test_empty_ctest_selection_rejected_and_unchecked_state(self):
        result, report = self.run_checker(required=(), listing={"tests": []})
        self.assertEqual(result.returncode, 1)
        self.assert_failure(report, "at least one --require-test")
        self.assert_failure(report, "empty CTest listing")
        result, report = self.run_checker(required=(), listing=self.listing)
        self.assertEqual(result.returncode, 1)
        self.assert_failure(report, "at least one --require-test")
        result, report = self.run_checker(required=(), include_listing=False)
        self.assertEqual(result.returncode, 0, report)
        self.assertEqual(report["checks"]["ctest"], "not_checked")

    def test_checkout_exemption_is_unambiguous(self):
        workflow = self.repo / ".github/workflows/ci.yml"
        templates = ["      - uses: other/action@v1\n        with:\n          ref: web-cpp-freeze\n",
                     "      - uses: actions/checkout@v4\n        with:\n          ref: web-cpp-freeze\n      - uses: actions/checkout@v4\n"]
        for steps in templates:
            with self.subTest(steps=steps):
                workflow.write_text("jobs:\n  build:\n    steps:\n" + steps + "      - run: cargo test -p missing\n", encoding="utf-8")
                result, report = self.run_checker()
                self.assertEqual(result.returncode, 1, report)
                self.assertEqual(report["checks"]["cargo"], "fail")

    def test_cargo_before_frozen_checkout_rejected(self):
        workflow = self.repo / ".github/workflows/ci.yml"
        workflow.write_text("jobs:\n  build:\n    steps:\n      - run: cargo test --workspace\n  web:\n    steps:\n      - run: cargo test -p missing\n      - uses: actions/checkout@v4\n        with:\n          ref: web-cpp-freeze\n      - run: cargo test -p frozen-only\n", encoding="utf-8")
        result, report = self.run_checker()
        self.assertEqual(result.returncode, 1)
        self.assert_failure(report, "Cargo before frozen checkout")

    def test_multiline_non_cargo_identifier_is_not_a_cargo_invocation(self):
        workflow = self.repo / ".github/workflows/ci.yml"
        lines = [
            "jobs:", "  build:", "    steps:", "      - run: |",
            "          cargo test --workspace",
            "          python3 Build/check_agent_contracts.py " + "\\",
            "            --require-test cargo_test_cache_contracts " + "\\",
            "            --require-label data-none",
        ]
        workflow.write_text("\n".join(lines) + "\n", encoding="utf-8")
        result, report = self.run_checker()
        self.assertEqual(result.returncode, 0, report)
        workflow.write_text("jobs:\n  build:\n    steps:\n      - run: |\n          cargo test --workspace " + "\\" + "\n            --lib\n", encoding="utf-8")
        result, report = self.run_checker()
        self.assertEqual(result.returncode, 1)
        self.assert_failure(report, "No escaped character")

    def test_cargo_home_setup_is_not_a_cargo_invocation(self):
        workflow = self.repo / ".github/workflows/ci.yml"
        workflow.write_text('jobs:\n  build:\n    steps:\n      - run: |\n          export CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}"\n          export RUSTUP_HOME="${RUSTUP_HOME:-$HOME/.rustup}"\n          cargo test --workspace\n', encoding="utf-8")
        result, report = self.run_checker()
        self.assertEqual(result.returncode, 0, report)
        workflow.write_text("jobs:\n  build:\n    steps:\n      - run: /some/bin/cargo test --workspace\n", encoding="utf-8")
        result, report = self.run_checker()
        self.assertEqual(result.returncode, 1)
        self.assert_failure(report, "unsupported Cargo command")

    def test_unsupported_cargo_command_fails_closed(self):
        workflow = self.repo / ".github/workflows/ci.yml"
        workflow.write_text("jobs:\n  build:\n    steps:\n      - run: echo hi && cargo test -p hp2-ffi\n", encoding="utf-8")
        result, report = self.run_checker()
        self.assertEqual(result.returncode, 1)
        self.assert_failure(report, "unsupported Cargo command")

    def test_default_report_location_and_missing_output(self):
        metadata = self.root / "metadata.json"
        metadata.write_text(json.dumps(self.metadata), encoding="utf-8")
        command = [sys.executable, str(CHECKER), "--repo", str(self.repo), "--metadata-json", str(metadata)]
        env = os.environ.copy()
        env["HP2_ARTIFACT_DIR"] = str(self.root / "reports")
        result = subprocess.run(command, env=env, capture_output=True, text=True, check=False)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(json.loads((self.root / "reports/agent-contracts.json").read_text())["ok"])
        del env["HP2_ARTIFACT_DIR"]
        result = subprocess.run(command, env=env, capture_output=True, text=True, check=False)
        self.assertEqual(result.returncode, 2)


if __name__ == "__main__":
    unittest.main()
