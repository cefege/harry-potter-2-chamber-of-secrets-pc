#!/usr/bin/env python3
"""Contract tests for Build/game_test.py without opening the HP2 application."""

from __future__ import annotations

import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import unittest

REPOSITORY_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(REPOSITORY_ROOT / "Build"))
import game_test  # noqa: E402


class GameTestContracts(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(prefix="hp2-game-test-contract-")
        self.root = Path(self.temporary.name)
        self.data_root = self.root / "Unreal"
        (self.data_root / "System").mkdir(parents=True)
        (self.data_root / "System" / "Default.ini").write_text("[Core.System]\n")

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def _map(self, relative: str) -> Path:
        path = self.data_root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(b"map")
        return path

    def test_maps_are_bytewise_sorted_and_tokens_are_derived(self) -> None:
        self._map("Maps/zeta.unr")
        self._map("Maps/Alpha.unr")
        self._map("Other/level.UNR")
        (self.data_root / "Maps" / "ignored.txt").write_text("not a map")

        self.assertEqual(
            game_test.list_maps(self.data_root),
            [
                {"map": "Maps/Alpha.unr", "map_token": "..\\Maps\\Alpha.unr"},
                {"map": "Maps/zeta.unr", "map_token": "..\\Maps\\zeta.unr"},
                {"map": "Other/level.UNR", "map_token": "..\\Other\\level.UNR"},
            ],
        )
        selected_path, token = game_test._resolve_map(self.data_root, "Maps/Alpha.unr")
        self.assertEqual(selected_path, self.data_root / "Maps" / "Alpha.unr")
        self.assertEqual(token, "..\\Maps\\Alpha.unr")
        with self.assertRaisesRegex(game_test.GameTestError, "exact listed map"):
            game_test._resolve_map(self.data_root, "maps/Alpha.unr")

    def test_symlinked_maps_are_rejected(self) -> None:
        target = self._map("Maps/target.unr")
        (self.data_root / "Maps" / "linked.unr").symlink_to(target)
        with self.assertRaisesRegex(game_test.GameTestError, "refusing symlinked map"):
            game_test.list_maps(self.data_root)

    def test_maps_cli_emits_only_versioned_json(self) -> None:
        self._map("Maps/Entryhall_hub.unr")
        completed = subprocess.run(
            [sys.executable, str(REPOSITORY_ROOT / "Build" / "game_test.py"), "maps", "--data-root", str(self.data_root)],
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )
        self.assertEqual(
            json.loads(completed.stdout),
            {"version": 1, "maps": [{"map": "Maps/Entryhall_hub.unr", "map_token": "..\\Maps\\Entryhall_hub.unr"}]},
        )
        self.assertEqual(completed.stderr, "")

    def test_validation_rejects_invalid_inputs_before_spawn(self) -> None:
        with self.assertRaisesRegex(game_test.GameTestError, "System/Default.ini"):
            game_test.list_maps(self.root)
        self._map("Maps/Entryhall_hub.unr")
        with self.assertRaisesRegex(game_test.GameTestError, "unsupported renderer"):
            game_test.build_launch_command(Path("app"), self.data_root, "..\\Maps\\Entryhall_hub.unr", "metal", 1)
        with self.assertRaisesRegex(game_test.GameTestError, "positive integer"):
            game_test.build_launch_command(Path("app"), self.data_root, "..\\Maps\\Entryhall_hub.unr", "xopengl", 0)
        with self.assertRaisesRegex(game_test.GameTestError, "positive finite"):
            game_test.run_command([sys.executable, "-c", "raise SystemExit(0)"], repo_root=self.root, timeout_seconds=float("inf"), log_path=self.root / "never.log")

    def test_launch_command_preserves_sound_by_default(self) -> None:
        command = game_test.build_launch_command(
            Path("/application"), self.data_root, "..\\Maps\\Entryhall_hub.unr", "xopengl", 42
        )
        self.assertEqual(
            command,
            [
                "/application", f"-datadir={self.data_root}", "..\\Maps\\Entryhall_hub.unr",
                "-xopengl", "-NOFRONTEND", "-window", "-testticks=42", "-log",
            ],
        )
        self.assertNotIn("-nosound", command)
        self.assertIn(
            "-nosound",
            game_test.build_launch_command(
                Path("/application"), self.data_root, "..\\Maps\\Entryhall_hub.unr", "vulkan", 1, no_sound=True
            ),
        )

    def test_supervised_command_captures_output_to_log(self) -> None:
        log_path = self.root / "logs" / "child.log"
        command = [sys.executable, "-c", "print('controlled child output')"]
        result = game_test.run_command(command, repo_root=self.root, timeout_seconds=5.0, log_path=log_path)
        self.assertTrue(result["passed"])
        self.assertEqual(result["command"], command)
        self.assertEqual(log_path.read_text(), "controlled child output\n")
        self.assertEqual(result["captured_output_bytes"], len(b"controlled child output\n"))

    def test_timeout_kills_the_complete_process_group(self) -> None:
        child_pid_path = self.root / "worker.pid"
        log_path = self.root / "timeout.log"
        code = (
            "import pathlib, subprocess, sys, time; "
            "worker = subprocess.Popen([sys.executable, '-c', 'import time; time.sleep(60)']); "
            f"pathlib.Path({str(child_pid_path)!r}).write_text(str(worker.pid)); "
            "time.sleep(60)"
        )
        result = game_test.run_command(
            [sys.executable, "-c", code], repo_root=self.root, timeout_seconds=0.2, log_path=log_path
        )
        self.assertTrue(result["timed_out"])
        self.assertFalse(result["passed"])
        self.assertIn(result["process_group_cleanup"], ("sigterm", "sigkill"))
        worker_pid = int(child_pid_path.read_text())
        deadline = time.monotonic() + 2.0
        while True:
            try:
                os.kill(worker_pid, 0)
            except ProcessLookupError:
                break
            if time.monotonic() >= deadline:
                self.fail("timed-out child process group left its worker alive")
            time.sleep(0.05)


if __name__ == "__main__":
    unittest.main()
