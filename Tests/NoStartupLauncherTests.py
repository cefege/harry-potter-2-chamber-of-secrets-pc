#!/usr/bin/env python3
"""Process and source contracts: the game never opens a startup launcher UI.

The process cases use only an empty temporary HOME/cwd outside the repository
(`data-none`). They prove the no-data and invalid-root exits and that removed UI
source/dependencies stay removed. They do not prove stock-menu delivery or any
physical window behavior; boot into data is covered by separate process tests.
"""
from __future__ import annotations

import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
GAME = Path(sys.argv.pop(1)) if len(sys.argv) > 1 and not sys.argv[1].startswith("-") else None


def run_game(*args: str, timeout: float = 60.0) -> subprocess.CompletedProcess:
    if GAME is None:
        raise unittest.SkipTest("game binary argument required")
    # Data discovery walks cwd ancestors. The harness TMPDIR can be a symlink into
    # this checkout, so use a fixed system temporary root outside the repository.
    with tempfile.TemporaryDirectory(prefix="hp2-bare-", dir="/tmp") as raw:
        base = Path(raw).resolve()
        if ROOT in base.parents:
            raise AssertionError(f"fixture must be outside the checkout: {base}")
        for name in ("home", "cwd", "tmp"):
            (base / name).mkdir()
        env = {"PATH": "/usr/bin:/bin", "LC_ALL": "C", "TZ": "UTC",
               "HOME": str(base / "home"), "TMPDIR": str(base / "tmp")}
        return subprocess.run([str(GAME), *args], cwd=base / "cwd", env=env,
                              capture_output=True, text=True, timeout=timeout, check=False)


class NoStartupLauncher(unittest.TestCase):
    def test_missing_data_exits_with_importer_guidance(self):
        result = run_game()
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertIn("hp2_import", result.stderr)
        self.assertNotIn("launcher", result.stderr.lower())

    def test_invalid_explicit_root_exits_without_ui(self):
        result = run_game("-datadir=/nonexistent-hp2-root")
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertIn("invalid -datadir", result.stderr)
        self.assertNotIn("launcher", result.stderr.lower())

    def test_removed_startup_ui_stays_removed(self):
        for removed in ("crates/hp2-game/src/launcher_ui.rs", "crates/hp2-game/src/launcher_window.rs",
                        "crates/hp2-game/tests/launcher_ui_contracts.rs"):
            self.assertFalse((ROOT / removed).exists(), removed)
        manifest = (ROOT / "crates/hp2-game/Cargo.toml").read_text(encoding="utf-8")
        for dependency in ("egui", "egui-wgpu"):
            self.assertNotIn(dependency, manifest)
        boot = (ROOT / "crates/hp2-launcher/src/boot.rs").read_text(encoding="utf-8")
        for removed in ("RunLauncherRound", "ShouldRunNativeLauncher"):
            self.assertNotIn(removed, boot)


if __name__ == "__main__":
    unittest.main()
