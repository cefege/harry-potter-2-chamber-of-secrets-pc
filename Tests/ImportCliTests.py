#!/usr/bin/env python3
"""Importer argv boundaries and real filesystem behavior through the executable."""

from __future__ import annotations

import argparse
import os
import subprocess
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path

IMPORTER_BINARY: Path


def write(root: Path, relative: str, content: bytes = b"asset") -> None:
    path = root / relative
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(content)


def retail_fixture(root: Path) -> None:
    # Matches output.rs's complete retail-only fixture, using real imported files.
    for category in ("System", "Maps", "Textures", "Sounds", "Music", "Help"):
        (root / category).mkdir(parents=True, exist_ok=True)
    for index in range(41):
        write(root, f"Maps/Map{index:03}.unr", b"map")
    write(root, "Maps/Entry.unr", b"entry map")
    for index in range(141):
        write(root, f"Music/Track{index:03}.ogg", b"music")
    for index in range(213):
        write(root, f"System/Cutscenes/Scene{index:03}.int", b"cutscene")
    for relative in (
        "System/Core.u", "System/Editor.u", "System/Engine.u", "System/Fire.u",
        "System/HGame.u", "System/HPModels.u", "System/HPParticle.u", "System/HProps.u",
        "System/HPSounds.u", "System/UnrealShare.u", "System/UWindow.u",
        "Textures/HP2_Master.utx", "Textures/HP2_Menu.utx", "Sounds/AllDialog.USA_uax",
        "System/BumpDialog.int", "System/BumpSet.int", "System/Credits.int",
        "System/Duelset.int", "System/hpcredits.int", "System/hpdialog.int",
        "System/HPMenu.int", "System/QuidSet.int", "System/BumpDialog.usa",
        "System/hpcredits.usa", "System/HpDialog.usa", "System/HpMenu.usa",
        "Textures/Old textures/HP_Menu.utx",
    ):
        write(root, relative)
    write(root, "System/Default.ini", (
        b"[Core.System]\n[FrontEnd]\nEASplashWaitTime=1\nWBSplashWaitTime=1\n"
        b"LaunchCode=1\nUseSaveSlot=1\n[ALAudio.ALAudioSubsystem]\nMusicVolume=0.53\n"
    ))
    write(root, "System/DefUser.ini", (
        b"[Engine.Input]\nAliases[8]=x\nRightMouse=x\nLeftMouse=x\nTab=x\nEnter=x\n"
        b"Ctrl=x\nAlt=x\nSpace=x\nZ=x\nSlash=x\nA=x\n[HGame.Harry]\n"
        b"fDamageMultiplier_Easy=1.2\nfDamageMultiplier_Medium=2.0\nfDamageMultiplier_Hard=3.0\n"
    ))


class ImportCliTests(unittest.TestCase):
    def setUp(self) -> None:
        temporary = tempfile.TemporaryDirectory(
            prefix="import-cli-", dir=os.environ.get("HP2_ARTIFACT_DIR")
        )
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()

    def cli(self, *args: str | bytes | Path) -> subprocess.CompletedProcess:
        return subprocess.run(
            [os.fspath(IMPORTER_BINARY), *args], cwd=self.root,
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False,
        )

    def failure(self, result: subprocess.CompletedProcess, error: str | bytes) -> None:
        if isinstance(error, str):
            error = error.encode("utf-8")
        self.assertEqual(result.returncode, 2)
        self.assertEqual(result.stdout, b"")
        self.assertEqual(result.stderr, b"hp2_import: " + error + b"\n")

    def success(self, result: subprocess.CompletedProcess) -> None:
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stderr, b"")
        self.assertEqual(result.stdout, b"Import completed successfully.\n")

    def test_first_error_without_help_or_option_normalization(self) -> None:
        cases = (
            ([], "one of the arguments --retail-root --archive is required"),
            (["--output", "out"], "one of the arguments --retail-root --archive is required"),
            (["--retail-root", "r", "--archive", "a"], "one of the arguments --retail-root --archive is required"),
            (["--retail-root", "r"], "the following arguments are required: --output"),
            (["--retail-root", "r", "--output", ""], "the following arguments are required: --output"),
            (["--help"], "unrecognized arguments: --help"),
            (["--"], "unrecognized arguments: --"),
            (["--output=out"], "unrecognized arguments: --output=out"),
            (["", "--profile", "bad"], "unrecognized arguments: "),
            (["first", "second"], "unrecognized arguments: first"),
            (["--profile", "SAFE", "--output"], "argument --profile: invalid choice: 'SAFE' (choose from 'safe', 'full', 'retail-only')"),
            (["--profile", "", "--profile", "safe"], "argument --profile: invalid choice: '' (choose from 'safe', 'full', 'retail-only')"),
            (["--link-mode", "COPY"], "argument --link-mode: invalid choice: 'COPY' (choose from 'auto', 'copy', 'hardlink')"),
            (["--check", "false"], "unrecognized arguments: false"),
        )
        for args, expected in cases:
            with self.subTest(args=args):
                self.failure(self.cli(*args), expected)
        for flag in ("--prototype-root", "--retail-root", "--archive", "--output", "--profile", "--link-mode"):
            with self.subTest(flag=flag):
                self.failure(self.cli(flag), f"argument {flag}: expected one argument")

    def test_values_consume_option_tokens_literally(self) -> None:
        for flag in ("--prototype-root", "--retail-root", "--archive", "--output"):
            with self.subTest(flag=flag):
                self.failure(self.cli(flag, "--check", "unknown"), "unrecognized arguments: unknown")
        self.failure(
            self.cli("--profile", "--check"),
            "argument --profile: invalid choice: '--check' (choose from 'safe', 'full', 'retail-only')",
        )
        self.failure(
            self.cli("--link-mode", "--output"),
            "argument --link-mode: invalid choice: '--output' (choose from 'auto', 'copy', 'hardlink')",
        )
        self.failure(
            self.cli("--retail-root", "--archive", "--output", "out"),
            "--prototype-root is required for safe and full profiles",
        )

    def test_presence_xor_precedes_output_and_import_nonempty_validation(self) -> None:
        for source in ("--retail-root", "--archive"):
            self.failure(self.cli(source, "", "--output", "out"), "exactly one of --retail-root and --archive is required")
            self.failure(self.cli(source, "", "--output", ""), "the following arguments are required: --output")
        for retail, archive in (("", "a"), ("r", ""), ("", "")):
            self.failure(
                self.cli("--retail-root", retail, "--archive", archive, "--output", "out"),
                "one of the arguments --retail-root --archive is required",
            )
        self.failure(
            self.cli("--retail-root", "r", "--output", "out", "--output", ""),
            "the following arguments are required: --output",
        )

    def test_default_safe_and_last_profile_require_prototype(self) -> None:
        args = ["--retail-root", "r", "--output", "out"]
        for profile_args in ([], ["--profile", "retail-only", "--profile", "full"], ["--profile", "safe"]):
            self.failure(self.cli(*args, *profile_args), "--prototype-root is required for safe and full profiles")
        self.assertFalse((self.root / "out").exists())

    @unittest.skipUnless(os.name == "posix", "raw argv bytes require POSIX")
    def test_raw_invalid_utf8_is_not_lossily_decoded(self) -> None:
        bad = b"\xff\xfe"
        self.failure(self.cli(bad), b"unrecognized arguments: \xff\xfe")
        self.failure(
            self.cli("--profile", bad),
            b"argument --profile: invalid choice: '\xff\xfe' (choose from 'safe', 'full', 'retail-only')",
        )
        self.failure(
            self.cli("--link-mode", bad),
            b"argument --link-mode: invalid choice: '\xff\xfe' (choose from 'auto', 'copy', 'hardlink')",
        )
        for source, error in (("--retail-root", "retail root path is not valid UTF-8"), ("--archive", "archive path is not valid UTF-8")):
            # Reverse argv order: conversion precedence must still be source,
            # prototype, output, after all parser checks have succeeded.
            args = ["--output", bad, "--prototype-root", bad, source, bad]
            self.failure(self.cli(*args), error)
            args[5] = ""
            self.failure(self.cli(*args), "prototype root path is not valid UTF-8")
            args[3] = ""
            self.failure(self.cli(*args), "output path is not valid UTF-8")
            args[1], args[5] = "", bad
            self.failure(self.cli(*args), "the following arguments are required: --output")
            self.failure(self.cli(*args, "unknown"), "unrecognized arguments: unknown")
        self.failure(
            self.cli("--retail-root", bad, "--archive", bad),
            "one of the arguments --retail-root --archive is required",
        )
        self.failure(self.cli("--retail-root", bad, "--output"), "argument --output: expected one argument")
        self.failure(
            self.cli("--retail-root", bad, "--output", "out", "--retail-root", ""),
            "exactly one of --retail-root and --archive is required",
        )

    def test_unicode_import_default_auto_existing_output_and_check(self) -> None:
        retail, output = self.root / "retail 雪 é", self.root / "overlay 魔法"
        retail_fixture(retail)
        args = ["--retail-root", retail, "--output", output, "--profile", "retail-only"]
        self.success(self.cli(*args))
        self.assertEqual((output / "Maps/Entry.unr").read_bytes(), b"entry map")
        self.assertTrue((output / "overlay-manifest.json").is_file())
        self.assertTrue((output / "overlay-checksums.txt").is_file())
        self.success(self.cli(*args))
        self.success(self.cli(*args, "--check", "--check"))
        # Damage an identity file, not an auto-hardlinked source asset.
        manifest = output / "overlay-manifest.json"
        manifest.write_bytes(b"corrupt")
        self.failure(self.cli(*args, "--check"), f"manifest is out of date or non-deterministic: {manifest}")
        self.assertEqual(manifest.read_bytes(), b"corrupt")

    def test_last_values_and_real_copy_and_hardlink(self) -> None:
        retail = self.root / "retail"
        retail_fixture(retail)
        for mode in ("copy", "hardlink"):
            with self.subTest(mode=mode):
                output = self.root / mode
                self.success(self.cli(
                    "--retail-root", "missing", "--retail-root", retail,
                    "--prototype-root", "missing", "--prototype-root", "",
                    "--output", "", "--output", output,
                    "--profile", "full", "--profile", "safe", "--profile", "retail-only",
                    "--link-mode", "auto", "--link-mode", mode,
                ))
                source, destination = retail / "Maps/Entry.unr", output / "Maps/Entry.unr"
                self.assertEqual(destination.read_bytes(), b"entry map")
                self.assertEqual(os.path.samefile(source, destination), mode == "hardlink")

    def test_archive_import_uses_last_archive_and_cleans_staging(self) -> None:
        retail, output = self.root / "retail", self.root / "archive-output"
        retail_fixture(retail)
        archive = self.root / "retail.zip"
        with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_DEFLATED) as packed:
            for entry in sorted(retail.rglob("*")):
                packed.write(entry, (Path("retail") / entry.relative_to(retail)).as_posix())
        args = ["--archive", "missing.zip", "--archive", archive, "--output", output, "--profile", "retail-only"]
        self.success(self.cli(*args))
        self.assertEqual((output / "Maps/Entry.unr").read_bytes(), b"entry map")
        self.success(self.cli(*args, "--check"))
        self.assertEqual(list(self.root.glob(".hp2-import-archive-*")), [])
        self.assertEqual(list(self.root.glob(".archive-output.tmp-*")), [])

    def test_check_precedes_source_access_without_creating_parent(self) -> None:
        parent = self.root / "missing-parent"
        self.failure(
            self.cli("--retail-root", "missing-source", "--output", parent / "out", "--profile", "retail-only", "--check"),
            f"output parent does not exist for --check: {parent}",
        )
        self.assertFalse(parent.exists())


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--importer", required=True, type=Path)
    arguments, unittest_args = parser.parse_known_args()
    IMPORTER_BINARY = arguments.importer.resolve()
    unittest.main(argv=[sys.argv[0], *unittest_args])
