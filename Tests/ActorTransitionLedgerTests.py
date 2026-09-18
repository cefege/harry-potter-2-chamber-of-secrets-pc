#!/usr/bin/env python3
"""Synthetic contracts for the actor lifecycle transition ledger; no map launch."""

from __future__ import annotations

import json
import subprocess
import sys
import tempfile
import unittest
from copy import deepcopy
from pathlib import Path

REPOSITORY_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(REPOSITORY_ROOT / "Build"))
import actor_transition_ledger  # noqa: E402


def _slot(path: str | None = "PrivetDr.GFemSly5", cls: str | None = "HGFemSly") -> dict[str, object]:
    return {"path": path, "class": cls}


def _snapshot(slot: dict[str, object] | None = None) -> dict[str, object]:
    return {
        "active_slot": _slot() if slot is None else slot,
        "delete_marked": False,
        "pending_kill": False,
        "effective_shadow_class": "Engine.ActorShadow",
        "shadow_class_cdo": "Engine.Default__ActorShadow",
    }


def _owner(identity: str = "<map>.GFemSly5") -> dict[str, object]:
    return {
        "identity": identity,
        "first_observed_actor": _slot(),
        "post_init_execution": _snapshot(),
        "pre_begin_before": _snapshot(),
        "pre_begin_after": _snapshot(),
        "spawn": {
            "request": {
                "class": "Engine.ActorShadow",
                "name": None,
                "owner": "PrivetDr.GFemSly5",
            },
            "result": _slot("Transient.ActorShadow0", "Engine.ActorShadow"),
            "published_child_slot": _slot("Transient.ActorShadow0", "Engine.ActorShadow"),
        },
        "shadow_post_continuation": "Transient.ActorShadow0",
        "first_renderer_candidate": "Transient.ActorShadow0",
    }


def _trace(owners: list[dict[str, object]], *, limit: int = 16, truncated: bool = False) -> dict[str, object]:
    return {
        "version": 2,
        "enabled": True,
        "limit": limit,
        "count": len(owners),
        "truncated": truncated,
        "owners": owners,
    }


class ActorTransitionLedgerSchemaContracts(unittest.TestCase):
    def test_accepts_complete_shared_schema(self) -> None:
        trace = _trace([_owner()])
        self.assertIs(actor_transition_ledger.validate_trace(trace), trace)

    def test_rejects_legacy_raw_slot_schema_even_at_new_version(self) -> None:
        trace = _trace([_owner()])
        owner = trace["owners"][0]
        assert isinstance(owner, dict)
        owner["raw_slot"] = owner.pop("first_observed_actor")
        with self.assertRaisesRegex(
            actor_transition_ledger.TraceError,
            r"owners\[0\]: missing required field 'first_observed_actor'",
        ):
            actor_transition_ledger.validate_trace(trace)

    def test_rejects_legacy_version_before_owner_schema_is_considered(self) -> None:
        trace = _trace([_owner()])
        trace["version"] = 1
        with self.assertRaisesRegex(actor_transition_ledger.TraceError, r"\$\.version: must equal 2"):
            actor_transition_ledger.validate_trace(trace)

    def test_rejects_non_normalized_identity(self) -> None:
        trace = _trace([_owner("PrivetDr.GFemSly5")])
        with self.assertRaisesRegex(
            actor_transition_ledger.TraceError,
            r"owners\[0\]\.identity: must be a normalized '<map>\.' actor identity",
        ):
            actor_transition_ledger.validate_trace(trace)

    def test_rejects_missing_transition_without_implicit_defaults(self) -> None:
        trace = _trace([_owner()])
        del trace["owners"][0]["pre_begin_after"]  # type: ignore[index]
        with self.assertRaisesRegex(
            actor_transition_ledger.TraceError,
            r"owners\[0\]: missing required field 'pre_begin_after'",
        ):
            actor_transition_ledger.validate_trace(trace)

    def test_rejects_duplicate_identity_and_inconsistent_bound(self) -> None:
        with self.assertRaisesRegex(actor_transition_ledger.TraceError, r"identity: must be unique"):
            actor_transition_ledger.validate_trace(_trace([_owner(), _owner()]))
        with self.assertRaisesRegex(actor_transition_ledger.TraceError, r"truncated: requires count to equal limit"):
            actor_transition_ledger.validate_trace(_trace([_owner()], limit=2, truncated=True))


class ActorTransitionLedgerComparisonContracts(unittest.TestCase):
    def test_joins_normalized_owner_identity_instead_of_capture_order(self) -> None:
        cpp = _trace([_owner("<map>.GFemSly5"), _owner("<map>.Ron")])
        rust = _trace([_owner("<map>.Ron"), _owner("<map>.GFemSly5")])
        self.assertEqual(
            actor_transition_ledger.compare_traces(cpp, rust),
            {"status": "match", "first_difference": None},
        )

    def test_canonicalizes_only_proven_observed_actor_map_roots(self) -> None:
        cpp = _trace([_owner()])
        rust = deepcopy(cpp)
        cpp_owner = cpp["owners"][0]
        assert isinstance(cpp_owner, dict)
        cpp_owner["first_observed_actor"]["path"] = "Package2.GFemSly5"  # type: ignore[index]
        for phase in ("post_init_execution", "pre_begin_before", "pre_begin_after"):
            cpp_owner[phase]["active_slot"]["path"] = "Package2.GFemSly5"  # type: ignore[index]
        cpp_owner["spawn"]["request"]["owner"] = "Package2.GFemSly5"  # type: ignore[index]

        self.assertEqual(
            actor_transition_ledger.compare_traces(cpp, rust),
            {"status": "match", "first_difference": None},
        )

    def test_preserves_external_class_paths_in_comparison(self) -> None:
        cpp = _trace([_owner()])
        rust = deepcopy(cpp)
        rust["owners"][0]["pre_begin_after"]["effective_shadow_class"] = "External.ActorShadow"  # type: ignore[index]

        result = actor_transition_ledger.compare_traces(cpp, rust)

        self.assertEqual(result["status"], "mismatch")
        self.assertEqual(
            result["first_difference"]["path"],  # type: ignore[index]
            "$.owner.pre_begin_after.effective_shadow_class",
        )

    def test_reports_first_lifecycle_transition_difference(self) -> None:
        cpp = _trace([_owner()])
        rust = deepcopy(cpp)
        rust["owners"][0]["pre_begin_after"]["pending_kill"] = True  # type: ignore[index]

        result = actor_transition_ledger.compare_traces(cpp, rust)

        self.assertEqual(result["status"], "mismatch")
        self.assertEqual(result["first_difference"]["identity"], "<map>.GFemSly5")  # type: ignore[index]
        self.assertEqual(result["first_difference"]["path"], "$.owner.pre_begin_after.pending_kill")  # type: ignore[index]
        self.assertFalse(result["first_difference"]["cpp"])  # type: ignore[index]
        self.assertTrue(result["first_difference"]["rust"])  # type: ignore[index]

    def test_rejects_differently_bounded_ledgers_before_owner_join(self) -> None:
        result = actor_transition_ledger.compare_traces(
            _trace([_owner()], limit=16),
            _trace([_owner()], limit=32),
        )
        self.assertEqual(
            result,
            {
                "status": "mismatch",
                "first_difference": {"path": "$.limit", "cpp": 16, "rust": 32},
            },
        )

    def test_reports_first_absent_cpp_owner_and_rust_only_owner(self) -> None:
        cpp = _trace([_owner("<map>.GFemSly5"), _owner("<map>.Ron")])
        rust = _trace([_owner("<map>.Ron")])
        missing_rust = actor_transition_ledger.compare_traces(cpp, rust)
        self.assertEqual(missing_rust["first_difference"]["cpp"], {"index": 0, "identity": "<map>.GFemSly5"})  # type: ignore[index]
        self.assertEqual(missing_rust["first_difference"]["rust"], {"missing": True})  # type: ignore[index]

        rust_only = actor_transition_ledger.compare_traces(_trace([]), _trace([_owner()]))
        self.assertEqual(rust_only["first_difference"]["cpp"], {"missing": True})  # type: ignore[index]
        self.assertEqual(rust_only["first_difference"]["rust"], {"index": 0, "identity": "<map>.GFemSly5"})  # type: ignore[index]


class ActorTransitionLedgerCliContracts(unittest.TestCase):
    def test_cli_writes_synthetic_first_difference(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            cpp_path = directory / "cpp.json"
            rust_path = directory / "rust.json"
            report_path = directory / "report.json"
            cpp_path.write_text(json.dumps(_trace([_owner()])), encoding="utf-8")
            rust = _trace([_owner()])
            rust["owners"][0]["spawn"]["published_child_slot"]["path"] = "Transient.ActorShadow1"  # type: ignore[index]
            rust_path.write_text(json.dumps(rust), encoding="utf-8")

            completed = subprocess.run(
                [
                    sys.executable,
                    str(REPOSITORY_ROOT / "Build" / "actor_transition_ledger.py"),
                    "--cpp-trace", str(cpp_path),
                    "--rust-trace", str(rust_path),
                    "--report", str(report_path),
                ],
                check=False,
                capture_output=True,
                text=True,
            )

            self.assertEqual(completed.returncode, 1, completed.stderr)
            report = json.loads(report_path.read_text(encoding="utf-8"))
            self.assertEqual(
                report["first_difference"]["path"],
                "$.owner.spawn.published_child_slot.path",
            )


if __name__ == "__main__":
    raise SystemExit(unittest.main())
