#!/usr/bin/env python3
"""Focused contracts for Build/lifecycle_gate.py without launching an engine."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
import sys
import threading
import tempfile
import unittest
from unittest.mock import patch

REPOSITORY_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(REPOSITORY_ROOT / "Build"))
import lifecycle_gate  # noqa: E402


class LifecycleGateContracts(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(prefix="hp2-lifecycle-gate-contract-")
        self.root = Path(self.temporary.name)

    def tearDown(self) -> None:
        self.temporary.cleanup()

    @staticmethod
    def _trace(*, probe_identities: list[object] | None = None) -> dict[str, object]:
        identities = probe_identities or [
            "LifecycleGateProbe:ProbeA",
            "LifecycleGateProbe:ProbeB",
        ]

        def properties(values: tuple[int, ...]) -> dict[str, int]:
            return dict(zip(lifecycle_gate.PROPERTY_FIELDS, values))

        return {
            "version": 1,
            "fixture": "LifecycleGate",
            "fixed_dt": 0.0166666675,
            "requested_ticks": 2,
            "actor": {"class": "LifecycleGateActor", "tag": "LifecycleGate"},
            "checkpoints": [
                {
                    "id": "spawn",
                    "tick": 0,
                    "events": list(lifecycle_gate.LIFECYCLE_PHASES),
                    "frame": {"state": "Scan", "pc_diagnostic": None, "latent": None},
                    "iterator": None,
                    "properties": properties(lifecycle_gate.EXPECTED_COUNTERS["spawn"]),
                    "live": True,
                    "delete_marked": False,
                },
                {
                    "id": "scan",
                    "tick": 0,
                    "events": ["AllActorsYield", "AllActorsYield", "AllActorsExhausted"],
                    "frame": {"state": "Scan", "pc_diagnostic": 13, "latent": None},
                    "iterator": {
                        "base_class": "Actor",
                        "tag": "LifecycleGateProbe",
                        "yields": identities,
                        "exhausted": True,
                        "output": None,
                    },
                    "properties": properties(lifecycle_gate.EXPECTED_COUNTERS["scan"]),
                    "live": True,
                    "delete_marked": False,
                },
                {
                    "id": "await_resume",
                    "tick": 0,
                    "events": ["EndState", "BeginState", "Sleep"],
                    "frame": {
                        "state": "AwaitResume",
                        "pc_diagnostic": 21,
                        "latent": {"kind": "Sleep", "wake_tick": 1},
                    },
                    "iterator": None,
                    "properties": properties(lifecycle_gate.EXPECTED_COUNTERS["await_resume"]),
                    "live": True,
                    "delete_marked": False,
                },
                {
                    "id": "destroy",
                    "tick": 1,
                    "events": ["EndState", "Destroyed", "DestroyActor"],
                    "frame": {
                        "state": "AwaitResume",
                        "pc_diagnostic": None,
                        "latent": None,
                    },
                    "iterator": None,
                    "properties": properties(lifecycle_gate.EXPECTED_COUNTERS["destroy"]),
                    "live": False,
                    "delete_marked": True,
                },
            ],
            "random_calls": [],
        }

    @staticmethod
    def _manifest(package_sha256: str) -> dict[str, object]:
        return {
            "version": 1,
            "fixture": "LifecycleGate",
            "package": "LifecycleGate",
            "package_artifact": {
                "source": "Package/LifecycleGate.u",
                "stage": "System/LifecycleGate.u",
                "sha256": package_sha256,
                "provenance": {
                    "producer": "native fixture package export",
                    "source": "LifecycleGate UnrealScript sources",
                },
            },
            "map": "Maps/LifecycleGate.unr",
            "map_source": "Maps/Entry.unr",
            "game_class": "LifecycleGate.LifecycleGateGame",
            "launch_url": "Maps/LifecycleGate.unr?game=LifecycleGate.LifecycleGateGame",
            "actor": {"class": "LifecycleGateActor", "tag": "LifecycleGate"},
            "probes": ["LifecycleGateProbe:ProbeA", "LifecycleGateProbe:ProbeB"],
            "lifecycle_phases": list(lifecycle_gate.LIFECYCLE_PHASES),
            "fixed_dt": 0.016666667,
            "emitted_fixed_dt": 0.0166666675,
            "requested_ticks": 2,
        }

    def _fixture(self) -> Path:
        fixture = self.root / "fixture"
        package = fixture / "Package" / "LifecycleGate.u"
        package.parent.mkdir(parents=True)
        package.write_bytes(b"committed-native-package")
        manifest = self._manifest(hashlib.sha256(package.read_bytes()).hexdigest())
        (fixture / "manifest.json").write_text(json.dumps(manifest), encoding="utf-8")
        return fixture

    def _data_root(self) -> Path:
        data_root = self.root / "data"
        (data_root / "System").mkdir(parents=True)
        (data_root / "System" / "Default.ini").write_text(
            "[Engine.Engine]\n", encoding="utf-8",
        )
        (data_root / "Maps").mkdir()
        (data_root / "Maps" / "Entry.unr").write_bytes(b"entry-map")
        return data_root

    def test_normalizes_probe_identity_without_runtime_object_ids(self) -> None:
        cpp = lifecycle_gate._normalise_trace(
            self._trace(probe_identities=[
                {"class": "LifecycleGateProbe", "tag": "LifecycleGateProbe", "stable_key": "ProbeA"},
                {"class": "LifecycleGateProbe", "tag": "LifecycleGateProbe", "stable_key": "ProbeB"},
            ]),
            "cpp",
        )
        rust = lifecycle_gate._normalise_trace(self._trace(), "rust")

        self.assertEqual(
            cpp["checkpoints"][1]["iterator"]["yields"],  # type: ignore[index]
            ["LifecycleGateProbe:ProbeA", "LifecycleGateProbe:ProbeB"],
        )
        self.assertEqual(lifecycle_gate.compare_traces(cpp, rust), {
            "status": "match",
            "first_difference": None,
        })

    def test_comparator_normalizes_physical_pc_units(self) -> None:
        cpp = lifecycle_gate._normalise_trace(self._trace(), "cpp")
        rust_document = self._trace()
        rust_document["checkpoints"][1]["frame"]["pc_diagnostic"] = 14  # type: ignore[index]
        rust = lifecycle_gate._normalise_trace(rust_document, "rust")

        self.assertEqual(
            lifecycle_gate.compare_traces(cpp, rust),
            {"status": "match", "first_difference": None},
        )

    def test_rejects_missing_duplicate_or_reordered_startup_phase(self) -> None:
        mutations = {
            "missing": ["Spawned", "PreBeginPlay", "BeginPlay", "PostBeginPlay"],
            "duplicate": [
                "Spawned",
                "PreBeginPlay",
                "BeginPlay",
                "BeginPlay",
                "PostBeginPlay",
                "SetInitialState",
            ],
            "reordered": [
                "Spawned",
                "BeginPlay",
                "PreBeginPlay",
                "PostBeginPlay",
                "SetInitialState",
            ],
        }
        for label, events in mutations.items():
            with self.subTest(label=label):
                document = self._trace()
                document["checkpoints"][0]["events"] = events  # type: ignore[index]
                with self.assertRaisesRegex(
                    lifecycle_gate.LifecycleGateError,
                    r"\.events must be exactly",
                ):
                    lifecycle_gate._normalise_trace(document, label)

    def test_rejects_missing_or_duplicated_phase_count(self) -> None:
        for label, value in (("missing", None), ("duplicated", 2)):
            with self.subTest(label=label):
                document = self._trace()
                properties = document["checkpoints"][0]["properties"]  # type: ignore[index]
                if value is None:
                    del properties["PreBeginPlayCount"]  # type: ignore[index]
                else:
                    properties["PreBeginPlayCount"] = value  # type: ignore[index]
                with self.assertRaisesRegex(
                    lifecycle_gate.LifecycleGateError,
                    r"\.properties",
                ):
                    lifecycle_gate._normalise_trace(document, label)

    def test_atomic_trace_rejects_leftover_temporary(self) -> None:
        trace = self.root / "trace.json"
        trace.write_text(json.dumps(self._trace()), encoding="utf-8")
        trace.with_suffix(".json.tmp").write_text("partial", encoding="utf-8")

        with self.assertRaisesRegex(lifecycle_gate.LifecycleGateError, "atomic temporary"):
            lifecycle_gate.read_atomic_trace(trace)

    def test_side_launch_uses_bounded_url_and_separate_trace_path(self) -> None:
        calls: list[dict[str, object]] = []

        def fake_run_command(command: list[str], **kwargs: object) -> dict[str, object]:
            calls.append({"command": command, **kwargs})
            trace = Path(kwargs["extra_env"][lifecycle_gate.TRACE_ENVIRONMENT])  # type: ignore[index]
            trace.write_text(json.dumps(self._trace()), encoding="utf-8")
            return {"passed": True}

        with patch.object(lifecycle_gate.game_test, "run_command", side_effect=fake_run_command):
            result = lifecycle_gate._run_side(
                name="cpp",
                executable=self.root / "cpp-engine",
                data_root=self.root / "cpp-data",
                artifact_dir=self.root / "artifacts",
                renderer="xopengl",
                timeout_seconds=5.0,
                start_gate=threading.Barrier(1),
            )

        self.assertIsNone(result["trace_error"])
        command = calls[0]["command"]
        self.assertIn("..\\Maps\\LifecycleGate.unr?game=LifecycleGate.LifecycleGateGame", command)
        self.assertIn("-testticks=2", command)
        self.assertIn("--fixed-dt=0.016666667", command)
        self.assertEqual(
            calls[0]["extra_env"],
            {lifecycle_gate.TRACE_ENVIRONMENT: str(self.root / "artifacts" / "cpp" / "lifecycle-gate.json")},
        )

    def test_fixture_manifest_requires_committed_package_entry_map_and_game_class(self) -> None:
        fixture = self._fixture()

        parsed = lifecycle_gate._read_fixture_manifest(fixture)

        self.assertEqual(parsed.package_source.as_posix(), "Package/LifecycleGate.u")
        self.assertEqual(parsed.package_output.as_posix(), "System/LifecycleGate.u")
        self.assertEqual(parsed.map_source.as_posix(), "Maps/Entry.unr")
        self.assertEqual(parsed.map_path.as_posix(), "Maps/LifecycleGate.unr")
        self.assertEqual(parsed.package_provenance["producer"], "native fixture package export")

    def test_staging_copies_provenance_checked_package_and_map_to_both_roots(self) -> None:
        fixture = self._fixture()
        data_root = self._data_root()

        staged = lifecycle_gate.stage_fixture(
            data_root=data_root,
            fixture_root=fixture,
            artifact_dir=self.root / "artifacts",
        )

        package = fixture / "Package" / "LifecycleGate.u"
        self.assertEqual(
            (staged.cpp_data_root / "System" / "LifecycleGate.u").read_bytes(),
            package.read_bytes(),
        )
        self.assertEqual(
            (staged.rust_data_root / "System" / "LifecycleGate.u").read_bytes(),
            package.read_bytes(),
        )
        self.assertEqual(
            (staged.cpp_data_root / "Maps" / "LifecycleGate.unr").read_bytes(),
            b"entry-map",
        )
        self.assertEqual(
            (staged.rust_data_root / "Maps" / "LifecycleGate.unr").read_bytes(),
            b"entry-map",
        )
        self.assertNotIn("EditPackages=LifecycleGate", (staged.cpp_data_root / "System" / "Default.ini").read_text())
        fixture_record = json.loads((self.root / "artifacts" / "fixture.json").read_text())
        self.assertEqual(
            fixture_record["package_artifact"]["sha256"],
            hashlib.sha256(package.read_bytes()).hexdigest(),
        )

    def test_staging_rejects_missing_or_mismatched_package_artifact(self) -> None:
        data_root = self._data_root()
        fixture = self._fixture()
        package = fixture / "Package" / "LifecycleGate.u"
        package.unlink()

        with self.assertRaisesRegex(lifecycle_gate.LifecycleGateError, "artifact is missing"):
            lifecycle_gate.stage_fixture(
                data_root=data_root,
                fixture_root=fixture,
                artifact_dir=self.root / "missing-artifacts",
            )

        package.write_bytes(b"tampered-package")
        with self.assertRaisesRegex(lifecycle_gate.LifecycleGateError, "SHA-256 mismatch"):
            lifecycle_gate.stage_fixture(
                data_root=data_root,
                fixture_root=fixture,
                artifact_dir=self.root / "tampered-artifacts",
            )

    def test_cli_has_no_ucc_binary_argument(self) -> None:
        arguments = lifecycle_gate._arguments([
            "--app", "cpp-app",
            "--engine-bin", "rust-engine",
            "--data-root", "data",
            "--output", "report.json",
        ])

        self.assertFalse(hasattr(arguments, "ucc_bin"))
        with self.assertRaises(SystemExit):
            lifecycle_gate._arguments([
                "--app", "cpp-app",
                "--engine-bin", "rust-engine",
                "--data-root", "data",
                "--output", "report.json",
                "--ucc-bin", "ucc",
            ])


if __name__ == "__main__":
    raise SystemExit(unittest.main())
