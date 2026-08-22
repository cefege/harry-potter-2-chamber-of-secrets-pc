#!/usr/bin/env python3
"""Contract tests for Build/repair_save.py without launching HP2."""

from __future__ import annotations

from pathlib import Path
import sys
import tempfile
import unittest
from unittest import mock

REPOSITORY_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(REPOSITORY_ROOT / "Build"))
import repair_save  # noqa: E402


class RepairSaveContracts(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(prefix="hp2-repair-save-contract-")
        self.root = Path(self.temporary.name)
        self.engine = self.root / "hp2_game"
        self.engine.write_bytes(b"engine")
        self.engine.chmod(0o700)
        self.data_root = self.root / "Data"
        (self.data_root / "System").mkdir(parents=True)
        (self.data_root / "System" / "Default.ini").write_text("[Core.System]\n")
        (self.data_root / "System" / "DefUser.ini").write_text("[Engine.Input]\n")
        self.user_root = self.root / "User"
        (self.user_root / "Save").mkdir(parents=True)
        (self.user_root / "Game.ini").write_text("[Engine.Engine]\n")
        (self.user_root / "User.ini").write_text("[Engine.Input]\n")
        self.target = self.user_root / "Save" / "Save0.usa"
        self.target.write_bytes(b"original-save")

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def test_repair_replaces_only_after_isolated_success_and_keeps_backup(self) -> None:
        def rewrite_staged(
            engine: Path,
            data_root: Path,
            staged_user: Path,
            isolated_home: Path,
            slot: int,
            timeout_seconds: float,
        ) -> None:
            self.assertEqual(slot, 0)
            (staged_user / "Save" / "Save0.usa").write_bytes(b"repaired-save")

        with mock.patch.object(repair_save, "run_engine", side_effect=rewrite_staged):
            result = repair_save.repair_save(
                self.engine, self.data_root, self.user_root, self.target
            )

        self.assertEqual(self.target.read_bytes(), b"repaired-save")
        self.assertEqual(result.backup.read_bytes(), b"original-save")
        self.assertEqual(result.target, self.target.resolve())

    def test_engine_failure_leaves_original_untouched(self) -> None:
        def fail_engine(*args: object, **kwargs: object) -> None:
            raise repair_save.RepairError("engine failed")

        with mock.patch.object(repair_save, "run_engine", side_effect=fail_engine):
            with self.assertRaisesRegex(repair_save.RepairError, "engine failed"):
                repair_save.repair_save(
                    self.engine, self.data_root, self.user_root, self.target
                )

        self.assertEqual(self.target.read_bytes(), b"original-save")
        self.assertEqual(list(self.target.parent.glob("Save0.usa.backup-*")), [])

    def test_validation_rejects_wrong_name_and_outside_target(self) -> None:
        wrong_name = self.user_root / "Save" / "checkpoint.usa"
        wrong_name.write_bytes(b"save")
        with self.assertRaisesRegex(repair_save.RepairError, "SaveN\\.usa"):
            repair_save.validate_inputs(
                self.engine, self.data_root, self.user_root, wrong_name, 10.0
            )

        outside = self.root / "Save1.usa"
        outside.write_bytes(b"save")
        with self.assertRaisesRegex(repair_save.RepairError, "directly inside"):
            repair_save.validate_inputs(
                self.engine, self.data_root, self.user_root, outside, 10.0
            )

    def test_run_engine_requires_exact_marker(self) -> None:
        staged_user = self.root / "staged-user"
        staged_user.mkdir()
        isolated_home = self.root / "isolated-home"
        isolated_home.mkdir()

        def complete_without_marker(*args: object, **kwargs: object) -> object:
            (staged_user / "repair-save.log").write_text("Log: save returned\n")
            return mock.Mock(returncode=0, stdout=b"")

        with mock.patch.object(repair_save.subprocess, "run", side_effect=complete_without_marker):
            with self.assertRaisesRegex(repair_save.RepairError, "missing exact success marker"):
                repair_save.run_engine(
                    self.engine, self.data_root, staged_user, isolated_home, 0, 10.0
                )


if __name__ == "__main__":
    unittest.main()
