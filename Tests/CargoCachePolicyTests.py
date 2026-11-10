#!/usr/bin/env python3
"""Exercise the real CMake policy in isolated script-mode fixtures."""

import os
from pathlib import Path
import subprocess
import unittest

ROOT = Path(__file__).resolve().parents[1]
ARTIFACTS = Path(os.environ["HP2_ARTIFACT_DIR"]).resolve()
HELPER = ROOT / "Build/CMake/HP2CargoTestCache.cmake"


def quote(value):
    # Bracket arguments do not expand variables, escapes, or encoded flags.
    marker = "="
    while "]" + marker + "]" in value:
        marker += "="
    return "[" + marker + "[" + value + "]" + marker + "]"


class CargoCachePolicyTests(unittest.TestCase):
    def check_command(self, enabled, args, expected, repeat=False):
        work = ARTIFACTS / self.id().split(".")[-1] / "build tree with spaces"
        work.mkdir(parents=True, exist_ok=True)
        shared = str(work / "Testing/HP2/CargoCache")
        args = [a.replace("@CMAKE@", "${CMAKE_COMMAND}") for a in args]
        expected = [a.replace("@SHARED@", shared) for a in expected]

        def cmake_arg(arg):
            if arg == "${CMAKE_COMMAND}":
                return '"${CMAKE_COMMAND}"'
            return quote(arg)

        expected = [a.replace("@CMAKE@", "${CMAKE_COMMAND}") for a in expected]
        lines = [
            "cmake_minimum_required(VERSION 3.20)",
            f"set(HP2_SHARED_TEST_CARGO_CACHE {'ON' if enabled else 'OFF'} CACHE BOOL \"\")",
            f"include({quote(str(HELPER))})",
            "hp2_prepare_cargo_test_command(result " + " ".join(map(cmake_arg, args)) + ")",
        ]
        if repeat:
            lines += [
                "hp2_prepare_cargo_test_command(again ${result})",
                'if(NOT "${result}" STREQUAL "${again}")',
                '  message(FATAL_ERROR "second application changed command")',
                "endif()",
            ]
        lines += [
            "list(LENGTH result actual_length)",
            f"if(NOT actual_length EQUAL {len(expected)})",
            '  message(FATAL_ERROR "argument count: ${actual_length}; result=${result}")',
            "endif()",
        ]
        for index, arg in enumerate(expected):
            lines += [
                f"list(GET result {index} actual)",
                f"set(expected {cmake_arg(arg)})",
                'if(NOT "${actual}" STREQUAL "${expected}")',
                f'  message(FATAL_ERROR "argument {index}: <${{actual}}> != <${{expected}}>")',
                "endif()",
            ]
        fixture = work / "fixture.cmake"
        fixture.write_text("\n".join(lines) + "\n", encoding="utf-8")
        result = subprocess.run(["cmake", "-P", str(fixture)], cwd=work,
                                text=True, capture_output=True, check=False)
        (work / "cmake.log").write_text(result.stdout + result.stderr, encoding="utf-8")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_off_equivalence(self):
        args = ["@CMAKE@", "-E", "env", "CARGO_TARGET_DIR=/old cache", "cargo", "test"]
        self.check_command(False, args, args)

    def test_on_environment_and_paths_flags(self):
        args = ["@CMAKE@", "-E", "env", "HOME=/isolated home",
                "CARGO_ENCODED_RUSTFLAGS=-C\x1ftarget-cpu=native\x1f--cfg=hello world",
                "CARGO_TARGET_DIR=/old cache", "TMPDIR=/tmp/test dir",
                "/cargo path/cargo", "test", "--features=one,two"]
        expected = args.copy()
        expected[5] = "CARGO_TARGET_DIR=@SHARED@"
        self.check_command(True, args, expected)

    def test_no_rewrite_after_executable(self):
        args = ["@CMAKE@", "-E", "env", "A=B", "cargo", "CARGO_TARGET_DIR=/argument"]
        self.check_command(True, args, args)

    def test_other_command(self):
        args = ["not-cmake", "-E", "env", "CARGO_TARGET_DIR=/old", "cargo"]
        self.check_command(True, args, args)

    def test_without_cargo_env(self):
        args = ["@CMAKE@", "-E", "env", "HOME=/isolated", "cargo", "test"]
        self.check_command(True, args, args)

    def test_idempotence(self):
        args = ["@CMAKE@", "-E", "env", "CARGO_TARGET_DIR=/old", "cargo", "test"]
        expected = args.copy()
        expected[3] = "CARGO_TARGET_DIR=@SHARED@"
        self.check_command(True, args, expected, repeat=True)

    def test_non_shell_environment_name(self):
        args = ["@CMAKE@", "-E", "env", "name-with-dash=value",
                "CARGO_TARGET_DIR=/old", "cargo"]
        expected = args.copy()
        expected[4] = "CARGO_TARGET_DIR=@SHARED@"
        self.check_command(True, args, expected)

    def test_missing_executable(self):
        args = ["@CMAKE@", "-E", "env", "CARGO_TARGET_DIR=/old"]
        self.check_command(True, args, args)

    def test_semicolon_argument(self):
        args = ["@CMAKE@", "-E", "env", "CARGO_TARGET_DIR=/old", "cargo", "a;b"]
        expected = args.copy()
        expected[3] = "CARGO_TARGET_DIR=@SHARED@"
        self.check_command(True, args, expected)


if __name__ == "__main__":
    unittest.main(verbosity=2)
