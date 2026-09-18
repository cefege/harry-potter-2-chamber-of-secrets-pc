#!/usr/bin/env python3
"""Focused contracts for Build/parity_gate.py without launching either runtime."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace

REPOSITORY_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(REPOSITORY_ROOT / "Build"))
import parity_gate  # noqa: E402


class ProfileIdentityContracts(unittest.TestCase):
    def _root(self, directory: Path) -> Path:
        root = directory / "data"
        (root / "System").mkdir(parents=True)
        (root / "System" / "Default.ini").write_text("[Core.System]\n", encoding="utf-8")
        return root

    def _overlay(
        self,
        root: Path,
        *,
        profile: str = "retail-only",
        payload: bytes = b"retail payload",
    ) -> Path:
        relative = "Maps/Test.unr"
        target = root / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(payload)
        digest = hashlib.sha256(payload).hexdigest()
        manifest = {
            "entries": [{
                "path": relative,
                "size": len(payload),
                "sha256": digest,
            }],
            "format": "hp2-retail-data-overlay",
            "profile": profile,
            "schema_version": 3 if profile == "retail-only" else 2,
        }
        (root / "overlay-manifest.json").write_text(
            json.dumps(manifest),
            encoding="utf-8",
        )
        (root / "overlay-checksums.txt").write_text(
            f"{relative}\t{len(payload)}\t{digest}\n",
            encoding="utf-8",
        )
        return target
    def test_retail_requires_qualified_overlay_profile(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = self._root(Path(temporary))
            self._overlay(root, profile="full")
            with self.assertRaisesRegex(
                parity_gate.GateError,
                "app.data_identity_profile",
            ):
                parity_gate._validate_profile(
                    "retail",
                    root.resolve(),
                    REPOSITORY_ROOT,
                )

            self._overlay(root)
            identity = parity_gate._validate_profile(
                "retail",
                root.resolve(),
                REPOSITORY_ROOT,
            )
            self.assertEqual(identity["qualification"], "retail-overlay")
            self.assertEqual(identity["overlay_profile"], "retail-only")
            self.assertRegex(
                str(identity["overlay_manifest_sha256"]),
                r"^[0-9a-f]{64}$",
            )
            self.assertRegex(
                str(identity["overlay_checksums_sha256"]),
                r"^[0-9a-f]{64}$",
            )
            self.assertEqual(
                identity["validator"],
                "hp-engine::data_identity::validate_retail_overlay_identity",
            )

    def test_retail_requires_manifest_checksum_pair(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = self._root(Path(temporary))
            (root / "overlay-manifest.json").write_text(
                json.dumps({
                    "entries": [],
                    "format": "hp2-retail-data-overlay",
                    "profile": "retail-only",
                    "schema_version": 3,
                }),
                encoding="utf-8",
            )
            with self.assertRaisesRegex(
                parity_gate.GateError,
                "app.data_identity_pair",
            ):
                parity_gate._validate_profile(
                    "retail",
                    root.resolve(),
                    REPOSITORY_ROOT,
                )

    def test_retail_rejects_checksum_mismatch_and_payload_drift(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = self._root(Path(temporary))
            target = self._overlay(root)
            (root / "overlay-checksums.txt").write_text(
                f"Maps/Other.unr\t1\t{'0' * 64}\n",
                encoding="utf-8",
            )
            with self.assertRaisesRegex(
                parity_gate.GateError,
                "app.data_identity_entries",
            ):
                parity_gate._validate_profile(
                    "retail",
                    root.resolve(),
                    REPOSITORY_ROOT,
                )

            target = self._overlay(root)
            target.write_bytes(b"drift")
            with self.assertRaisesRegex(
                parity_gate.GateError,
                "app.data_identity_size",
            ):
                parity_gate._validate_profile(
                    "retail",
                    root.resolve(),
                    REPOSITORY_ROOT,
                )

    def test_wrong_profile_fails_before_runtime_validation(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = self._root(Path(temporary))
            arguments = argparse.Namespace(
                profile="prototype",
                data_root=root,
                cpp_app=Path("does-not-exist.app"),
                rust_bin=Path("does-not-exist-bin"),
                artifact_dir=Path(temporary) / "artifacts",
            )
            with self.assertRaisesRegex(parity_gate.GateError, "prototype profile requires"):
                parity_gate._context(arguments)


class StageSelectionContracts(unittest.TestCase):
    def test_selects_exactly_requested_stage(self) -> None:
        self.assertEqual(parity_gate._selected_stages("format"), ("format",))
        self.assertEqual(parity_gate._selected_stages("all"), parity_gate.STAGES)

    def test_all_stage_order_is_sealed(self) -> None:
        self.assertEqual(
            parity_gate.STAGES,
            ("format", "vm", "paired", "world", "campaign", "product"),
        )

    def test_rust_app_is_conditional_on_product_selection(self) -> None:
        parity_gate._require_rust_app("paired", None)
        with self.assertRaisesRegex(parity_gate.GateError, "--rust-app is required"):
            parity_gate._require_rust_app("product", None)
        with self.assertRaisesRegex(parity_gate.GateError, "--rust-app is required"):
            parity_gate._require_rust_app("all", None)
        parity_gate._require_rust_app("product", Path("HarryPotter2.app"))


class VmStageContracts(unittest.TestCase):
    MAPS = ["..\\Maps\\A.unr", "..\\Maps\\B.unr", "..\\Maps\\C.unr"]

    def _census(self) -> dict[str, object]:
        return {
            "maps": [{
                "identity": {"map_token": map_name},
                "zero_streams": False,
                "token_aborts": [],
                "native_aborts": [],
                "actors_without_class": [],
                "class_chain_skips": [],
            } for map_name in self.MAPS]
        }

    def _ledger(self) -> dict[str, object]:
        maps = [{
            "identity": {"map_token": map_name},
            "map_outcome": "completed",
            "records": [{"outcome": "completed"}],
            "reason_census": {},
            "subject_census": {},
        } for map_name in self.MAPS]
        return {
            "maps": maps,
            "stream_count": len(maps),
            "completed_count": len(maps),
            "suspended_count": 0,
            "failed_count": 0,
            "aborted_count": 0,
            "deferred_count": 0,
            "script_deferred": 0,
            "reason_census": {},
            "subject_census": {},
        }

    def test_accepts_complete_all_map_reports(self) -> None:
        self.assertEqual(
            parity_gate._vm_report_violations(
                self._census(), self._ledger(), self.MAPS
            ),
            [],
        )

    def test_accepts_normal_suspended_execution_outcome(self) -> None:
        ledger = self._ledger()
        ledger["maps"][0]["records"][0]["outcome"] = "suspended"
        ledger["completed_count"] -= 1
        ledger["suspended_count"] = 1
        self.assertEqual(
            parity_gate._vm_report_violations(
                self._census(),
                ledger,
                self.MAPS,
            ),
            [],
        )

    def test_rejects_historical_three_map_subset_for_larger_profile(self) -> None:
        expected = [*self.MAPS, "..\\Maps\\D.unr"]
        violations = parity_gate._vm_report_violations(
            self._census(), self._ledger(), expected
        )
        self.assertTrue(any("map coverage mismatch" in item for item in violations))

    def test_rejects_structural_skips_and_aborts(self) -> None:
        census = self._census()
        census["maps"][0]["actors_without_class"] = [{"actor": "A"}]
        census["maps"][1]["class_chain_skips"] = [{"actor": "B"}]
        census["maps"][2]["token_aborts"] = [{"reason": "bad token"}]
        violations = parity_gate._vm_report_violations(
            census, self._ledger(), self.MAPS
        )
        self.assertTrue(any("actors_without_class" in item for item in violations))
        self.assertTrue(any("class_chain_skips" in item for item in violations))
        self.assertTrue(any("token_aborts" in item for item in violations))

    def test_rejects_noncompleted_and_nonempty_censuses(self) -> None:
        ledger = self._ledger()
        ledger["maps"][0]["map_outcome"] = "failed"
        ledger["maps"][0]["records"][0]["outcome"] = "deferred"
        ledger["maps"][0]["reason_census"] = {"native.body_deferred": 1}
        ledger["maps"][0]["subject_census"] = {"Engine.Actor.Foo": 1}
        ledger["completed_count"] -= 1
        ledger["deferred_count"] = 1
        ledger["script_deferred"] = 1
        ledger["reason_census"] = {"native.body_deferred": 1}
        ledger["subject_census"] = {"Engine.Actor.Foo": 1}
        violations = parity_gate._vm_report_violations(
            self._census(), ledger, self.MAPS
        )
        self.assertTrue(any("failed outcomes" in item for item in violations))
        self.assertTrue(any("reason_census" in item for item in violations))
        self.assertTrue(any("subject_census" in item for item in violations))


class SourceIdentityContracts(unittest.TestCase):
    def test_staged_only_source_is_included_in_worktree_digest(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            repo = Path(temporary)

            def git(*arguments: str) -> None:
                subprocess.run(
                    ("git", *arguments),
                    cwd=repo,
                    check=True,
                    stdout=subprocess.DEVNULL,
                    stderr=subprocess.DEVNULL,
                )

            git("init")
            source = repo / "Build" / "staged.py"
            source.parent.mkdir()
            source.write_text("value = 1\n", encoding="utf-8")
            git("add", "Build/staged.py")
            git(
                "-c", "user.name=Parity Gate Test",
                "-c", "user.email=parity-gate@example.invalid",
                "commit", "-m", "baseline",
            )
            source.write_text("value = 2\n", encoding="utf-8")
            git("add", "Build/staged.py")

            identity = parity_gate._source_identity(repo)

            self.assertEqual(
                identity["worktree"],
                [{
                    "path": "Build/staged.py",
                    "sha256": hashlib.sha256(source.read_bytes()).hexdigest(),
                }],
            )


class ArtifactContracts(unittest.TestCase):
    def test_artifact_directory_must_be_fresh_and_empty(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            new = root / "new"
            self.assertEqual(parity_gate._prepare_artifact_dir(new), new.resolve())
            (new / "foreign.txt").write_text("occupied", encoding="utf-8")
            with self.assertRaisesRegex(parity_gate.GateError, "new and empty"):
                parity_gate._prepare_artifact_dir(new)

    def test_inventory_is_sorted_deterministic_and_excludes_manifest(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "z").mkdir()
            (root / "z" / "last.bin").write_bytes(b"last")
            (root / "first.bin").write_bytes(b"first")
            (root / "manifest.json").write_text("changes freely", encoding="utf-8")
            first = parity_gate._generated_inventory(root)
            second = parity_gate._generated_inventory(root)
            self.assertEqual(first, second)
            self.assertEqual(
                [entry["path"] for entry in first],
                ["first.bin", "z/last.bin"],
            )
            self.assertTrue(all(len(str(entry["sha256"])) == 64 for entry in first))

    def test_stage_seal_rejects_undeclared_files(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            artifact_dir = Path(temporary)
            stage_dir = artifact_dir / "paired"
            stage_dir.mkdir()
            declared = stage_dir / "report.json"
            declared.write_text("{}\n", encoding="utf-8")
            (stage_dir / "foreign.txt").write_text("surprise", encoding="utf-8")
            context = SimpleNamespace(artifact_dir=artifact_dir)
            with self.assertRaisesRegex(parity_gate.GateError, "undeclared artifact"):
                parity_gate._seal_stage(
                    context,
                    stage_dir,
                    {"status": "passed"},
                    declared_files=(declared,),
                )

    def test_nested_stage_report_name_is_not_hidden_from_declarations(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            artifact_dir = Path(temporary)
            stage_dir = artifact_dir / "paired"
            nested = stage_dir / "nested"
            nested.mkdir(parents=True)
            declared = stage_dir / "report.json"
            declared.write_text("{}\n", encoding="utf-8")
            (nested / "stage.json").write_text("{}\n", encoding="utf-8")
            context = SimpleNamespace(artifact_dir=artifact_dir)
            with self.assertRaisesRegex(parity_gate.GateError, "undeclared artifact"):
                parity_gate._seal_stage(
                    context,
                    stage_dir,
                    {"status": "passed"},
                    declared_files=(declared,),
                )

    def test_stage_seal_records_declared_inventory(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            artifact_dir = Path(temporary)
            stage_dir = artifact_dir / "format"
            stage_dir.mkdir()
            report_path = stage_dir / "comparison.json"
            report_path.write_text('{"status":"match"}\n', encoding="utf-8")
            context = SimpleNamespace(artifact_dir=artifact_dir)
            report = parity_gate._seal_stage(
                context,
                stage_dir,
                {"status": "passed"},
                declared_files=(report_path,),
            )
            self.assertEqual(report["declarations"]["files"], ["format/comparison.json"])
            self.assertEqual(report["artifacts"][0]["path"], "format/comparison.json")
            self.assertTrue((stage_dir / "stage.json").is_file())

    def test_failed_stage_is_sealed_with_process_results_and_artifact_hashes(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            artifact_dir = Path(temporary)
            stage_dir = artifact_dir / "campaign"
            stage_dir.mkdir()
            retained = stage_dir / "prerequisites.json"
            retained.write_text('{"status":"failed"}\n', encoding="utf-8")
            process = {
                "command": ["campaign-runner"],
                "exit_status": 1,
                "log": "campaign/campaign-runner.log",
            }
            context = SimpleNamespace(
                artifact_dir=artifact_dir,
                data_identity={"qualification": "test"},
                process_records={"campaign": [process]},
            )

            report = parity_gate._seal_failed_stage(
                context,
                "campaign",
                parity_gate.GateError("campaign prerequisite failed"),
            )

            self.assertEqual(report["status"], "failed")
            self.assertEqual(report["commands"], [process])
            self.assertEqual(
                report["declarations"]["files"],
                ["campaign/prerequisites.json"],
            )
            self.assertEqual(
                report["artifacts"][0]["path"],
                "campaign/prerequisites.json",
            )
            persisted = json.loads(
                (stage_dir / "stage.json").read_text(encoding="utf-8")
            )
            self.assertEqual(persisted["error"], "campaign prerequisite failed")


class PairedReportContracts(unittest.TestCase):
    def _report(self) -> dict[str, object]:
        run = {
            "passed": True,
            "script_deferral_violation": None,
            "resources": {
                "script_deferred": 0,
                "script_deferral_reasons": {},
                "script_deferral_subjects": {},
            },
        }

        def actor_checkpoint(
            name: str, first_net: int | None, first_dynamic: int | None,
        ) -> dict[str, object]:
            order = [{"class": "Engine.Actor", "is_null": False}]
            boundaries = {
                "i_first_net_relevant_actor": first_net,
                "i_first_dynamic_actor": first_dynamic,
            }
            return {
                "checkpoint": name,
                "matches": True,
                "multiset_difference": [],
                "ambiguous_groups": [],
                "normalized_order": {"cpp": order, "rust": list(order)},
                "order_match": True,
                "null_positions": {"cpp": [], "rust": []},
                "null_positions_match": True,
                "boundaries": {
                    "cpp": boundaries,
                    "rust": dict(boundaries),
                },
                "boundaries_match": True,
            }

        actor_checkpoints = [
            actor_checkpoint("post_deserialize_raw", None, None),
            actor_checkpoint("post_startup_before_first_tick", 0, 0),
            actor_checkpoint("post_startup", 0, 0),
        ]
        return {
            "passed": True,
            "runs": {"cpp": dict(run), "rust": dict(run)},
            "reports": [{
                "cpp": {field: None for field in parity_gate.FIDELITY_FIELD_NAMES},
                "rust": {field: None for field in parity_gate.FIDELITY_FIELD_NAMES},
                "comparison": {"status": "match", "divergence": None},
            }],
            "actor_slot_semantic_join": {
                "status": "observed",
                "matches": True,
                "checkpoints": actor_checkpoints,
            },
            "rng_seed": {"status": "match"},
        }

    def test_accepts_complete_matching_report(self) -> None:
        parity_gate._validate_paired_report(self._report())

    def test_rejects_exit_success_without_checkpoint_comparison(self) -> None:
        report = self._report()
        report["reports"] = []
        with self.assertRaisesRegex(parity_gate.GateError, "checkpoint comparisons"):
            parity_gate._validate_paired_report(report)

    def test_rejects_missing_deferral_telemetry(self) -> None:
        report = self._report()
        report["runs"]["rust"]["resources"].pop("script_deferral_subjects")
        with self.assertRaisesRegex(parity_gate.GateError, "script_deferral_subjects"):
            parity_gate._validate_paired_report(report)

    def test_rejects_missing_requested_auxiliary_comparison(self) -> None:
        report = self._report()
        with self.assertRaisesRegex(parity_gate.GateError, "global_tick_trace"):
            parity_gate._validate_paired_report(
                report, required_auxiliaries=("global_tick_trace",)
            )


    def test_rejects_actor_slot_order_mismatch_even_with_equal_multiset(self) -> None:
        report = self._report()
        checkpoint = report["actor_slot_semantic_join"]["checkpoints"][2]
        checkpoint["normalized_order"]["rust"] = [
            {"class": "Engine.OtherActor", "is_null": False},
        ]
        with self.assertRaisesRegex(parity_gate.GateError, "order"):
            parity_gate._validate_paired_report(report)

    def test_rejects_actor_slot_null_position_mismatch(self) -> None:
        report = self._report()
        checkpoint = report["actor_slot_semantic_join"]["checkpoints"][2]
        checkpoint["null_positions"]["rust"] = [0]
        with self.assertRaisesRegex(parity_gate.GateError, "nulls"):
            parity_gate._validate_paired_report(report)

    def test_rejects_actor_slot_boundary_mismatch(self) -> None:
        report = self._report()
        checkpoint = report["actor_slot_semantic_join"]["checkpoints"][2]
        checkpoint["boundaries"]["rust"]["i_first_dynamic_actor"] = 1
        with self.assertRaisesRegex(parity_gate.GateError, "boundaries"):
            parity_gate._validate_paired_report(report)

class AuditComparisonContracts(unittest.TestCase):
    def test_reports_first_value_path_without_type_normalization(self) -> None:
        difference = parity_gate._first_json_difference(
            {"actors": [{"id": "Harry", "health": 1}]},
            {"actors": [{"id": "Harry", "health": 1.0}]},
        )
        self.assertEqual(
            difference,
            {"path": "$.actors[0].health", "reference": 1, "rust": 1.0},
        )

    def test_byte_mismatch_with_equal_json_is_not_silenced(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            reference = directory / "reference.json"
            rust = directory / "rust.json"
            report = directory / "diff.json"
            reference.write_bytes(b'{"a":1}\n')
            rust.write_bytes(b'{\n  "a": 1\n}\n')
            with self.assertRaisesRegex(parity_gate.GateError, "byte mismatch"):
                parity_gate._compare_audits(reference, rust, report)
            document = json.loads(report.read_text(encoding="utf-8"))
            self.assertFalse(document["byte_equal"])
            self.assertEqual(document["first_difference"]["path"], "$")


if __name__ == "__main__":
    raise SystemExit(unittest.main())
