#!/usr/bin/env python3
"""Byte-for-byte parity checks between the C++ importer and Python oracle."""

from __future__ import annotations

import importlib.util
import os
import shutil
import subprocess
import sys
import tarfile
import tempfile
import unittest
import zipfile
from pathlib import Path

IMPORTER_BINARY = Path(sys.argv[1]).resolve()
ORACLE_SCRIPT = Path(sys.argv[2]).resolve()
sys.argv = [sys.argv[0]]
ORACLE_SPEC = importlib.util.spec_from_file_location("prepare_retail_data", ORACLE_SCRIPT)
assert ORACLE_SPEC is not None and ORACLE_SPEC.loader is not None
ORACLE = importlib.util.module_from_spec(ORACLE_SPEC)
sys.modules[ORACLE_SPEC.name] = ORACLE
ORACLE_SPEC.loader.exec_module(ORACLE)


def _write(root: Path, relative: str, content: bytes | None = None) -> None:
    path = root / Path(relative)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(content if content is not None else relative.encode("utf-8"))


def _ini(sections: dict[str, dict[str, str]]) -> bytes:
    rows = []
    for section, values in sections.items():
        rows.append(f"[{section}]")
        rows.extend(f"{key}={value}" for key, value in values.items())
        rows.append("")
    return "\r\n".join(rows).encode("utf-8")


def _default_ini(retail: bool) -> bytes:
    return _ini({
        "Engine.Engine": {
            "GameRenderDevice": "D3D11Drv.D3D11RenderDevice",
            "WindowedRenderDevice": "D3D11Drv.D3D11RenderDevice",
            "RenderDevice": "D3D11Drv.D3D11RenderDevice",
            "ViewportManager": "WinDrv.WindowsClient",
            "AudioDevice": "ALAudio.ALAudioSubsystem",
            "Language": "usa" if retail else "int",
            "EASplashWaitTime": "1.0",
            "WBSplashWaitTime": "1.0",
        },
        "Engine.GameEngine": {
            "FrameRateLimit": "60.0",
            "ServerActors": "SomeUnavailable.Actor",
        },
        "Core.System": {
            "SavePath": "Save\\",
            "CachePath": "Cache\\",
            "Paths": "../System/*.u",
        },
        "SDLDrv.SDLClient": {"NativeText": "False"},
        "XOpenGLDrv.XOpenGLRenderDevice": {"OpenGLVersion": "Core"},
        "FrontEnd": {
            "EASplashWaitTime": "2.0",
            "WBSplashWaitTime": "3.0",
            "LaunchCode": "RetailLaunch",
            "UseSaveSlot": "True",
        },
        "ALAudio.ALAudioSubsystem": {"MusicVolume": "0.53"},
        "HGame.baseConsole": {"bDebugMode": "True"},
        "HGame.HPConsole": {"bDebugMode": "True"},
    })


def _defuser_ini(retail: bool) -> bytes:
    bindings = {
        "Aliases[8]": "\"Use Potion\"",
        "RightMouse": "Jump",
        "LeftMouse": "Fire",
        "Tab": "ShowObjectives",
        "Enter": "Use",
        "Ctrl": "Duck",
        "Alt": "Walk",
        "Space": "Jump",
        "Z": "CastSpell",
        "Slash": "Say",
        "A": "MoveLeft",
        "Joy1": "Fire",
        "Joy2": "Jump",
        "Joy3": "Use",
        "Joy4": "Duck",
        "JoyY": "Axis aBaseY speed=1.0",
        "JoyU": "Axis aBaseX speed=1.0",
    }
    return _ini({
        "Engine.Input": bindings,
        "Engine.PlayerPawn": {
            "bModernThirdPersonControls": "True" if not retail else "False",
            "ObjectDetail": "ObjectDetailMedium",
        },
        "HGame.Harry": {
            "fDamageMultiplier_Easy": "1.2",
            "fDamageMultiplier_Medium": "2.0",
            "fDamageMultiplier_Hard": "3.0",
        },
    })


def make_fixtures(base: Path) -> tuple[Path, Path]:
    retail = base / "retail"
    prototype = base / "prototype"
    for root in (retail, prototype):
        for directory in ORACLE.CANONICAL_ROOTS:
            (root / directory).mkdir(parents=True, exist_ok=True)

    critical = sorted(ORACLE.CRITICAL_RETAIL_PATHS | ORACLE.RETAIL_FINAL_LOCALIZATION)
    for relative in critical:
        _write(retail, relative)
    # The required HP_Menu package is deliberately located at its retail-only source path.
    _write(retail, "Textures/Old textures/HP_Menu.utx")
    for index in range(41):
        _write(retail, f"Maps/map-{index:02d}.unr")
    _write(retail, "Maps/Entry.unr")
    for index in range(214):
        _write(retail, f"System/cutscenes/scene-{index:03d}.int")
    for index in range(141):
        _write(retail, f"Music/track-{index:03d}.ogg")
    _write(retail, "Help/Splash.bmp")
    _write(retail, "System/default.ini", _default_ini(True) + b"; split\vform\frecord\x1cgroup\x1dunit\x1eNEL\xc2\x85LS\xe2\x80\xa8PS\xe2\x80\xa9")
    _write(retail, "System/DefUser.ini", _defuser_ini(True))

    for relative in critical:
        # Prototype needs the native-compatible system packages and INT localization.
        if (
            relative.casefold().endswith(".int")
            or relative.casefold().endswith(".usa")
            or relative.casefold() == "sounds/alldialog.usa_uax"
            or relative.casefold() == "textures/hp2_menu.utx"
        ):
            continue
        _write(prototype, relative)
    for index in range(100):
        name = f"map-{index:02d}.unr" if index < 41 else ("Entry.unr" if index == 41 else f"prototype-{index:03d}.unr")
        _write(prototype, f"Maps/{name}")
    for index in range(185):
        _write(prototype, f"System/cutscenes/scene-{index:03d}.int")
    for index in range(51):
        _write(prototype, f"Textures/prototype-{index:02d}.utx")
    _write(prototype, "Sounds/AllDialog.uax")
    _write(prototype, "System/AllDialog.int")
    _write(prototype, "System/default.ini", _default_ini(False) + b"; split\vform\frecord\x1cgroup\x1dunit\x1eNEL\xc2\x85LS\xe2\x80\xa8PS\xe2\x80\xa9")
    _write(prototype, "System/DefUser.ini", _defuser_ini(False))
    return retail, prototype


def tree_bytes(root: Path) -> dict[str, bytes]:
    return {
        path.relative_to(root).as_posix(): path.read_bytes()
        for path in sorted(root.rglob("*"))
        if path.is_file()
    }


class ImporterParity(unittest.TestCase):
    def setUp(self) -> None:
        artifact = Path(os.environ.get("HP2_ARTIFACT_DIR", tempfile.gettempdir()))
        artifact.mkdir(parents=True, exist_ok=True)
        self.temp = tempfile.TemporaryDirectory(prefix="importer-parity-", dir=artifact)
        self.root = Path(self.temp.name)
        self.retail, self.prototype = make_fixtures(self.root / "fixtures")
        self.importer = IMPORTER_BINARY
        self.oracle = ORACLE_SCRIPT
        self.python = sys.executable

    def tearDown(self) -> None:
        self.temp.cleanup()

    def _run(self, tool: str, source: Path, output: Path, profile: str,
             prototype: Path | None = None, *extra: str) -> subprocess.CompletedProcess[str]:
        if tool == "cpp":
            command = [str(self.importer)]
        else:
            command = [self.python, str(self.oracle)]
        command.extend(("--retail-root", str(source), "--output", str(output),
                        "--profile", profile, "--link-mode", "copy"))
        if prototype is not None:
            command.extend(("--prototype-root", str(prototype)))
        command.extend(extra)
        return subprocess.run(command, capture_output=True, encoding="utf-8", check=False)

    def _assert_success(self, py: subprocess.CompletedProcess[str], cpp: subprocess.CompletedProcess[str]) -> None:
        self.assertEqual(py.returncode, cpp.returncode, f"Python stderr: {py.stderr}\nC++ stderr: {cpp.stderr}")
        self.assertEqual(py.returncode, 0, f"Python stderr: {py.stderr}\nC++ stderr: {cpp.stderr}")

    def _assert_outputs_equal(self, py: Path, cpp: Path) -> None:
        py_tree = tree_bytes(py)
        cpp_tree = tree_bytes(cpp)
        self.assertEqual(set(py_tree), set(cpp_tree), "relative output file sets differ")
        for relative in sorted(py_tree):
            self.assertEqual(py_tree[relative], cpp_tree[relative], f"output bytes differ: {relative}")

    def test_profiles_check_idempotence_and_archives(self) -> None:
        for profile in ("safe", "full", "retail-only"):
            with self.subTest(profile=profile):
                py_out = self.root / f"{profile}-python"
                cpp_out = self.root / f"{profile}-cpp"
                prototype = None if profile == "retail-only" else self.prototype
                py = self._run("python", self.retail, py_out, profile, prototype)
                cpp = self._run("cpp", self.retail, cpp_out, profile, prototype)
                self._assert_success(py, cpp)
                self._assert_outputs_equal(py_out, cpp_out)
                if profile == "retail-only":
                    py_check = self._run("python", self.retail, py_out, profile, None, "--check")
                    cpp_check = self._run("cpp", self.retail, cpp_out, profile, None, "--check")
                    self._assert_success(py_check, cpp_check)
                    py_rerun = self._run("python", self.retail, py_out, profile)
                    cpp_rerun = self._run("cpp", self.retail, cpp_out, profile)
                    self._assert_success(py_rerun, cpp_rerun)
                    self._assert_outputs_equal(py_out, cpp_out)

        archives = self.root / "archives"
        archives.mkdir()
        zip_path = archives / "retail.zip"
        with zipfile.ZipFile(zip_path, "w", compression=zipfile.ZIP_STORED) as archive:
            for path in sorted(self.retail.rglob("*")):
                if path.is_file():
                    archive.write(path, (Path("Retail") / path.relative_to(self.retail)).as_posix())
        tar_path = archives / "retail.tar.gz"
        with tarfile.open(tar_path, "w:gz") as archive:
            archive.add(self.retail, arcname="Retail")
        archive_paths = [zip_path, tar_path]
        bsdtar = shutil.which("bsdtar")
        if bsdtar:
            seven_path = archives / "retail.7z"
            made = subprocess.run([bsdtar, "-a", "-cf", str(seven_path),
                                  "-C", str(self.retail.parent), self.retail.name],
                                  capture_output=True, text=True, check=False)
            self.assertEqual(made.returncode, 0, made.stderr)
            archive_paths.append(seven_path)
        for archive in archive_paths:
            with self.subTest(archive=archive.suffixes):
                py_out = self.root / f"{archive.name}-python"
                cpp_out = self.root / f"{archive.name}-cpp"
                py = self._run_archive("python", archive, py_out)
                cpp = self._run_archive("cpp", archive, cpp_out)
                self._assert_success(py, cpp)
                self._assert_outputs_equal(py_out, cpp_out)

    def _run_archive(self, tool: str, archive: Path, output: Path) -> subprocess.CompletedProcess[str]:
        command = ([str(self.importer)] if tool == "cpp" else [self.python, str(self.oracle)])
        command.extend(("--archive", str(archive), "--output", str(output),
                        "--profile", "retail-only", "--link-mode", "copy"))
        return subprocess.run(command, capture_output=True, encoding="utf-8", check=False)

    @staticmethod
    def _normalized_error(result: subprocess.CompletedProcess[str], prefix: str) -> str:
        lines = [line.strip() for line in result.stderr.splitlines() if line.strip()]
        if not lines:
            return ""
        line = lines[-1]
        marker = prefix + ": "
        return line[len(marker):] if line.startswith(marker) else line

    def _assert_failure_parity(self, py: subprocess.CompletedProcess[str],
                               cpp: subprocess.CompletedProcess[str]) -> None:
        self.assertNotEqual(py.returncode, 0, "Python oracle unexpectedly accepted invalid input")
        self.assertEqual(py.returncode, cpp.returncode, f"Python stderr: {py.stderr}\nC++ stderr: {cpp.stderr}")
        self.assertEqual(self._normalized_error(py, "prepare_retail_data.py"),
                         self._normalized_error(cpp, "hp2_import"))

    def test_invalid_inputs_report_same_errors(self) -> None:
        missing_maps = self.root / "missing-maps"
        shutil.copytree(self.retail, missing_maps)
        shutil.rmtree(missing_maps / "Maps")
        self._failure_root(missing_maps, "missing-maps")

        executable = self.root / "bad-executable"
        shutil.copytree(self.retail, executable)
        _write(executable, "System/unsafe.exe")
        self._failure_root(executable, "bad-executable")

        collision = self.root / "case-collision"
        shutil.copytree(self.retail, collision)
        _write(collision, "Maps/collision.unr")
        _write(collision, "Maps/COLLISION.unr")
        self._failure_root(collision, "case-collision")

        unicode_collision = self.root / "unicode-casefold-collision"
        shutil.copytree(self.retail, unicode_collision)
        _write(unicode_collision, "Maps/Straße.unr")
        _write(unicode_collision, "Maps/STRASSE.unr")
        self._failure_root(unicode_collision, "unicode-casefold-collision")

        bad_zip = self.root / "traversal.zip"
        with zipfile.ZipFile(bad_zip, "w") as archive:
            archive.write(self.retail / "System/Core.u", "Retail/System/Core.u")
            archive.writestr("../evil", b"not allowed")
        py = self._run_archive("python", bad_zip, self.root / "traversal-python")
        cpp = self._run_archive("cpp", bad_zip, self.root / "traversal-cpp")
        self._assert_failure_parity(py, cpp)

    def _failure_root(self, source: Path, name: str) -> None:
        py = self._run("python", source, self.root / f"{name}-python", "retail-only")
        cpp = self._run("cpp", source, self.root / f"{name}-cpp", "retail-only")
        self._assert_failure_parity(py, cpp)


if __name__ == "__main__":
    unittest.main()
