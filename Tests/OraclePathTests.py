#!/usr/bin/env python3
"""Isolated path-policy and real CMake packaging-target rejection fixtures."""
import importlib.util
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("oracle_paths", ROOT / "Build/check_oracle_paths.py")
guard = importlib.util.module_from_spec(spec)
spec.loader.exec_module(guard)


class OraclePathTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(dir=os.environ.get("TMPDIR"))
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        self.dest = self.root / "dist/HarryPotter2.app"
        self.dest.mkdir(parents=True)

    def reject(self, path):
        with self.assertRaises((ValueError, OSError, RuntimeError)):
            guard.check(str(path), str(self.dest))

    def test_empty_and_unrelated(self):
        guard.check("", str(self.dest))
        guard.check(str(self.root / "other/missing.app"), str(self.dest))
        guard.check(str(self.root / "other/missing.app"), str(self.root / "missing/dest.app"))

    def test_overlap_and_relative(self):
        for path in (self.dest, self.dest.parent, self.dest / "missing", "relative", str(self.dest).upper()):
            with self.subTest(path=path):
                self.reject(path)
        with self.assertRaises(ValueError):
            guard.check("", "relative")

    def test_symlinks_and_dangling(self):
        alias = self.root / "alias"
        alias.symlink_to(self.dest, target_is_directory=True)
        self.reject(alias)
        self.reject(alias / "missing")
        dangling = self.root / "dangling"
        dangling.symlink_to(self.dest / "missing", target_is_directory=True)
        self.reject(dangling)
        loop = self.root / "loop"
        loop.symlink_to(loop)
        self.reject(loop)

    def test_filesystem_errors_fail_closed(self):
        with mock.patch.object(guard.Path, "stat", side_effect=PermissionError("denied")):
            with self.assertRaises(PermissionError):
                guard.check(str(self.root / "oracle"), str(self.dest))

    def test_dangling_destination_alias(self):
        alias = self.root / "future-destination"
        alias.symlink_to(self.root / "future/target", target_is_directory=True)
        with self.assertRaises(ValueError):
            guard.check(str(self.root / "future/target/child"), str(alias))

    def test_hardlinked_executable_and_unrelated_executable(self):
        exe = self.dest / "Contents/MacOS/HarryPotter2"
        exe.parent.mkdir(parents=True)
        exe.write_text("fixture")
        other = self.root / "other.app/Contents/MacOS/HarryPotter2"
        other.parent.mkdir(parents=True)
        os.link(exe, other)
        self.reject(other.parents[2])
        self.reject(other)
        other.unlink()
        other.write_text("fixture")
        guard.check(str(other.parents[2]), str(self.dest))

    @unittest.skipUnless(sys.platform == "darwin", "macOS packaging fixture")
    def test_cmake_default_and_dmg_reject_before_deletion(self):
        source = self.root / "project"
        (source / "Build").mkdir(parents=True)
        shutil.copy(ROOT / "Build/check_oracle_paths.py", source / "Build")
        (source / "dummy.c").write_text("int main(void) { return 0; }\n")
        (source / "CMakeLists.txt").write_text(f'''cmake_minimum_required(VERSION 3.20)
project(GuardFixture VERSION 1.0 LANGUAGES C)
set(Python3_EXECUTABLE "{sys.executable}")
set(HP2_CPP_ORACLE_APP "${{PROJECT_SOURCE_DIR}}/dist/macos-arm64/HarryPotter2.app" CACHE PATH "Synthetic reference")
add_library(OpenAL SHARED dummy.c)
add_executable(hp2_game_rust dummy.c)
add_executable(hp2_import IMPORTED GLOBAL)
set_target_properties(hp2_import PROPERTIES IMPORTED_LOCATION "{sys.executable}")
include("{ROOT / 'Build/CMake/HP2Install.cmake'}")
''')
        target = source / "dist/macos-arm64/HarryPotter2.app"
        target.mkdir(parents=True)
        sentinel = target / "reference-sentinel"
        sentinel.write_text("must survive")
        for index, candidate in enumerate([str(target), "OFF", "FALSE", "0", "NOTFOUND"]):
            with self.subTest(candidate=candidate):
                build = self.root / f"build-{index}"
                configure = subprocess.run([
                    "cmake", "-S", str(source), "-B", str(build),
                    f"-DHP2_CPP_ORACLE_APP:PATH={candidate}",
                ], text=True, capture_output=True)
                self.assertEqual(configure.returncode, 0, configure.stdout + configure.stderr)
                warning = " ".join(configure.stderr.split())
                self.assertIn("the value is preserved and packaging remains blocked", warning)
                detail = "overlaps the packaging destination" if index == 0 else "must be an absolute path"
                self.assertIn(detail, warning)
                cache = (build / "CMakeCache.txt").read_text()
                self.assertIn(f"HP2_CPP_ORACLE_APP:PATH={candidate}\n", cache)
                for args in ([], ["--target", "hp2_macos_dmg"]):
                    with self.subTest(target=args):
                        result = subprocess.run(["cmake", "--build", str(build), *args], text=True, capture_output=True)
                        self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
                        self.assertIn("Oracle packaging safety check failed", result.stdout + result.stderr)
                        self.assertEqual(sentinel.read_text(), "must survive")


if __name__ == "__main__":
    unittest.main(verbosity=2)
