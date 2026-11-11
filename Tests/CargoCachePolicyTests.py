#!/usr/bin/env python3
"""Exercise the real CMake policy in isolated script-mode fixtures."""

import hashlib
import json
import os
from pathlib import Path
import re
import shlex
import shutil
import subprocess
import sys
import tempfile
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

    def check_rustup_home(self, explicit, expected):
        work = ARTIFACTS / self.id().split(".")[-1]
        work.mkdir(parents=True, exist_ok=True)
        lines = [
            "cmake_minimum_required(VERSION 3.20)",
            f"include({quote(str(HELPER))})",
            'set(ENV{HOME} "/configured home")',
            f"set(ENV{{RUSTUP_HOME}} {quote(explicit)})",
            "hp2_test_rustup_home(result)",
            # A later isolated test HOME must not change the resolved build input.
            'set(ENV{HOME} "/isolated test home")',
            f"set(expected {quote(expected)})",
            'if(NOT "${result}" STREQUAL "${expected}")',
            '  message(FATAL_ERROR "toolchain home: <${result}> != <${expected}>")',
            "endif()",
        ]
        fixture = work / "fixture.cmake"
        fixture.write_text("\n".join(lines) + "\n", encoding="utf-8")
        result = subprocess.run(["cmake", "-P", str(fixture)], cwd=work,
                                text=True, capture_output=True, check=False)
        (work / "cmake.log").write_text(result.stdout + result.stderr, encoding="utf-8")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_workflows_seed_file_api_before_first_configure(self):
        cases = [
            ("ci.yml", {"macos-arm64", "linux-x64", "linux-arm64"}),
            ("release.yml", {"macos-arm64"}),
        ]
        work_root = ARTIFACTS / self.id().split(".")[-1]
        work_root.mkdir(parents=True, exist_ok=True)
        for workflow, expected_presets in cases:
            text = (ROOT / ".github/workflows" / workflow).read_text(encoding="utf-8")
            blocks = re.findall(
                r"(?m)^      - name: Configure\n        run: \|\n((?:          [^\n]*\n)+)",
                text,
            )
            seen = set()
            for block in blocks:
                lines = [line[10:] for line in block.splitlines()]
                commands = [line for line in lines if line.startswith("cmake --preset ")]
                self.assertEqual(len(commands), 1, block)
                command = commands[0]
                preset = shlex.split(command)[2]
                self.assertNotIn(preset, seen)
                seen.add(preset)
                query = f"out/{preset}/.cmake/api/v1/query"
                seed = f'touch "{query}/codemodel-v2"'
                self.assertIn(seed, lines)
                self.assertLess(lines.index(seed), lines.index(command))
                work = Path(tempfile.mkdtemp(prefix=f"{workflow}-{preset}-", dir=work_root))
                source = work / "source"
                source.mkdir()
                # Include the real policy: its in-configure query write alone is too late.
                fixture = [
                    "cmake_minimum_required(VERSION 3.20)",
                    "project(FileApiFirstConfigure NONE)",
                    f"include({quote(str(ROOT / 'Build/CMake/HP2NativeFree.cmake'))})",
                ]
                (source / "CMakeLists.txt").write_text("\n".join(fixture) + "\n", encoding="utf-8")
                replacement = f"cmake -S source -B out/{preset}"
                script = "\n".join(lines).replace(command, replacement)
                (work / "configure.sh").write_text(script + "\n", encoding="utf-8")
                result = subprocess.run(["bash", "-euo", "pipefail", "-c", script],
                                        cwd=work, text=True, capture_output=True, check=False)
                (work / "configure.log").write_text(result.stdout + result.stderr, encoding="utf-8")
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                reply = work / f"out/{preset}/.cmake/api/v1/reply"
                indices = list(reply.glob("index-*.json"))
                self.assertEqual(len(indices), 1, f"missing first-configure File API index: {work}")
                index = json.loads(indices[0].read_text(encoding="utf-8"))
                model = index["reply"]["codemodel-v2"]
                self.assertEqual(model["kind"], "codemodel")
                self.assertEqual(model["version"]["major"], 2)
                self.assertTrue((reply / model["jsonFile"]).is_file())
            self.assertEqual(seen, expected_presets, workflow)

    def check_generated_header_snapshot(self, scenario):
        root = ARTIFACTS / "ci-generated-headers"
        root.mkdir(parents=True, exist_ok=True)
        work = Path(tempfile.mkdtemp(prefix="lifecycle-", dir=root))
        source = work / "source"
        build = source / "out/synthetic"
        sdl = build / "_deps/hp2_sdl2-src"
        openal = build / "_deps/hp2_openal_soft-src"
        for directory in (source / "Build/CMake", sdl / "include", openal / "cmake", openal / "hrtf"):
            directory.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT / "Build/CMake/HP2NativeFree.cmake", source / "Build/CMake/HP2NativeFree.cmake")
        shutil.copy2(ROOT / "Build/refresh_native_provenance.py", source / "Build/refresh_native_provenance.py")
        shutil.copy2(ROOT / "Build/native-free-allowlist.json", source / "Build/native-free-allowlist.json")
        (source / "CMakePresets.json").write_text(json.dumps({
            "version": 3,
            "configurePresets": [{"name": "synthetic", "generator": "Ninja",
                                  "environment": {"HP2_PRESET_TEST": "initial"},
                                  "binaryDir": "${sourceDir}/out/synthetic",
                                  "cacheVariables": {"CMAKE_BUILD_TYPE": "Debug",
                                                     "HP2_TEST_SECRET": "sentinel-secret-never-in-report"}}],
        }), encoding="utf-8")
        (sdl / "include/SDL_config.h.cmake").write_text("sdl template\n", encoding="utf-8")
        (sdl / "CMakeLists.txt").write_text(
            'add_custom_target(SDL2-static)\n'
            'file(GENERATE OUTPUT "${CMAKE_CURRENT_BINARY_DIR}/include-config-release/SDL2/SDL_config.h" CONTENT "sdl generated\\n")\n',
            encoding="utf-8",
        )
        (openal / "cmake/bin2h.script.cmake").write_text("# synthetic generator\n", encoding="utf-8")
        (openal / "hrtf/Default HRTF.mhr").write_text("hrtf bytes\n", encoding="utf-8")
        (openal / "CMakeLists.txt").write_text(
            'add_custom_target(OpenAL ALL\n'
            '  COMMAND "${CMAKE_COMMAND}" -E copy_if_different\n'
            '    "${CMAKE_CURRENT_SOURCE_DIR}/hrtf/Default HRTF.mhr"\n'
            '    "${CMAKE_CURRENT_BINARY_DIR}/default_hrtf.hpp"\n'
            '  BYPRODUCTS "${CMAKE_CURRENT_BINARY_DIR}/default_hrtf.hpp" VERBATIM)\n',
            encoding="utf-8",
        )
        (source / "CMakeLists.txt").write_text(
            'cmake_minimum_required(VERSION 3.20)\n'
            'project(GeneratedHeaderLifecycle LANGUAGES NONE)\n'
            'add_custom_target(graph_probe ALL COMMAND "${CMAKE_COMMAND}" -E echo "$ENV{HP2_GRAPH_VALUE}" VERBATIM)\n'
            'include("${CMAKE_SOURCE_DIR}/Build/CMake/HP2NativeFree.cmake")\n'
            'add_subdirectory("${CMAKE_BINARY_DIR}/_deps/hp2_sdl2-src" "${CMAKE_BINARY_DIR}/_deps/hp2_sdl2-build")\n'
            'add_subdirectory("${CMAKE_BINARY_DIR}/_deps/hp2_openal_soft-src" "${CMAKE_BINARY_DIR}/_deps/hp2_openal_soft-build")\n'
            '_hp2_native_free_generated_headers(headers)\n'
            'file(WRITE "${CMAKE_BINARY_DIR}/hp2-native-free-manifest.json" "[${headers}]")\n',
            encoding="utf-8",
        )

        artifact_dir = work / "helper-artifacts"
        artifact_dir.mkdir()
        env = {**os.environ, "HP2_ARTIFACT_DIR": str(artifact_dir), "HP2_GRAPH_VALUE": "initial"}
        helper = [sys.executable, "-B", str(source / "Build/refresh_native_provenance.py"),
                  "--preset", "synthetic", "--build-dir", "out/synthetic"]

        def run(name, command):
            result = subprocess.run(command, cwd=source, env=env, text=True, capture_output=True, check=False)
            (work / f"{name}.log").write_text(result.stdout + result.stderr, encoding="utf-8")
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

        configure = ["cmake", "--preset", "synthetic"]
        run("initial-configure", configure)
        snapshot = build / "hp2-native-free-manifest.json"
        self.assertEqual(json.loads(snapshot.read_text(encoding="utf-8")), [])
        paths = {
            "_deps/hp2_sdl2-build/include-config-release/SDL2/SDL_config.h": "SDL2-static",
            "_deps/hp2_openal_soft-build/default_hrtf.hpp": "OpenAL",
        }
        self.assertTrue((build / next(iter(paths))).is_file(), "SDL output is generated after configure")
        self.assertFalse((build / "_deps/hp2_openal_soft-build/default_hrtf.hpp").exists())
        run("record-build-state", helper + ["--record-build-state"])
        self.assertTrue((artifact_dir / "native-build-state.json").is_file())
        run("build", ["cmake", "--build", str(build)])
        before = {path: (build / path).read_bytes() for path in paths}
        cache_before = (build / "CMakeCache.txt").read_bytes()
        commands_before = subprocess.run(["ninja", "-C", str(build), "-t", "commands"],
                                         check=True, capture_output=True).stdout
        if scenario == "regenerate":
            sdl_cmake = sdl / "CMakeLists.txt"
            old = sdl_cmake.read_text(encoding="utf-8")
            self.assertIn("sdl generated", old)
            sdl_cmake.write_text(old.replace("sdl generated", "sdl changed"), encoding="utf-8")
        elif scenario == "cache":
            presets = source / "CMakePresets.json"
            config = json.loads(presets.read_text(encoding="utf-8"))
            config["configurePresets"][0]["cacheVariables"]["CMAKE_BUILD_TYPE"] = "Release"
            presets.write_text(json.dumps(config), encoding="utf-8")
        elif scenario == "preset_environment":
            presets = source / "CMakePresets.json"
            config = json.loads(presets.read_text(encoding="utf-8"))
            config["configurePresets"][0]["environment"]["HP2_PRESET_TEST"] = "changed"
            presets.write_text(json.dumps(config), encoding="utf-8")
        elif scenario == "ninja_environment":
            env["HP2_GRAPH_VALUE"] = "changed"
        elif scenario == "missing_state":
            (artifact_dir / "native-build-state.json").unlink()
        elif scenario == "header_symlink":
            header = build / "_deps/hp2_sdl2-build/include-config-release/SDL2/SDL_config.h"
            first = build / "identical-first.h"
            second = build / "identical-second.h"
            first.write_bytes(header.read_bytes())
            second.write_bytes(header.read_bytes())
            header.unlink()
            header.symlink_to(first)
            self.assertEqual(header.read_bytes(), before[next(iter(paths))])
            header.unlink()
            header.symlink_to(second)
            self.assertEqual(header.read_bytes(), before[next(iter(paths))])
        elif scenario == "prefix_symlink":
            prefix = build / "_deps/hp2_sdl2-build/include/SDL2"
            prefix.parent.mkdir(parents=True, exist_ok=True)
            first = build / "prefix-first"
            second = build / "prefix-second"
            for directory in (first, second):
                directory.mkdir()
                (directory / "same.h").write_bytes(b"identical header bytes\n")
            prefix.symlink_to(first, target_is_directory=True)
            original = (prefix / "same.h").read_bytes()
            prefix.unlink()
            prefix.symlink_to(second, target_is_directory=True)
            self.assertEqual((prefix / "same.h").read_bytes(), original)
        existing_evidence = set(artifact_dir.glob("refresh-native-provenance-*"))
        result = subprocess.run(helper, cwd=source, env=env, text=True, capture_output=True, check=False)
        (work / "refresh.log").write_text(result.stdout + result.stderr, encoding="utf-8")
        evidence = list(set(artifact_dir.glob("refresh-native-provenance-*")) - existing_evidence)
        self.assertEqual(len(evidence), 1, result.stdout + result.stderr)
        report_text = (evidence[0] / "report.json").read_text(encoding="utf-8")
        self.assertNotIn("sentinel-secret-never-in-report", report_text)
        report = json.loads(report_text)
        if scenario in ("cache", "preset_environment", "ninja_environment", "missing_state", "header_symlink", "prefix_symlink"):
            self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
            self.assertFalse(report["ok"])
            diagnostic = report["error"].lower()
            expected = {"cache": ("cache", "preset"), "preset_environment": ("preset",),
                        "ninja_environment": ("ninja",), "missing_state": ("state",),
                        "header_symlink": ("symlink",), "prefix_symlink": ("symlink",)}
            self.assertTrue(any(word in diagnostic for word in expected[scenario]), diagnostic)
            if scenario in ("cache", "preset_environment", "ninja_environment"):
                for path, data in before.items():
                    self.assertEqual((build / path).read_bytes(), data, path)
            if scenario in ("preset_environment", "ninja_environment"):
                self.assertEqual((build / "CMakeCache.txt").read_bytes(), cache_before)
            if scenario == "ninja_environment":
                commands_after = subprocess.run(["ninja", "-C", str(build), "-t", "commands"],
                                                check=True, capture_output=True).stdout
                self.assertNotEqual(commands_before, commands_after)
            return
        old_hashes = json.loads((evidence[0] / "before.json").read_text(encoding="utf-8"))
        new_hashes = json.loads((evidence[0] / "after.json").read_text(encoding="utf-8"))
        self.assertEqual(report["configure_status"], 0, result.stdout + result.stderr)
        self.assertTrue((evidence[0] / "manifest-before.json").is_file())
        self.assertTrue((evidence[0] / "manifest-after.json").is_file())
        self.assertEqual(set(old_hashes), set(paths))
        if scenario == "regenerate":
            changed_path = "_deps/hp2_sdl2-build/include-config-release/SDL2/SDL_config.h"
            self.assertNotEqual(result.returncode, 0, "changed generated header must be rejected")
            self.assertFalse(report["ok"])
            self.assertEqual(report["changed"], [changed_path])
            self.assertEqual(report["added"], [])
            self.assertEqual(report["removed"], [])
            self.assertIn("generated headers changed", report["error"])
            self.assertNotEqual(old_hashes[changed_path], new_hashes[changed_path])
            self.assertNotEqual((build / changed_path).read_bytes(), before[changed_path])
            return
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertTrue(report["ok"])
        self.assertEqual(old_hashes, new_hashes)
        records = json.loads(snapshot.read_text(encoding="utf-8"))
        self.assertEqual(len(records), len(paths))
        self.assertEqual({record["path"] for record in records}, set(paths))
        for record in records:
            path = record["path"]
            self.assertEqual(record["target"], paths[path])
            self.assertEqual((build / path).read_bytes(), before[path], path)
            self.assertEqual(record["sha256"], hashlib.sha256(before[path]).hexdigest())
            for source_key, hash_key in (("template_path", "template_sha256"),
                                         ("cmake_source", "cmake_sha256"),
                                         ("input_source", "input_sha256")):
                self.assertEqual(record[hash_key], hashlib.sha256(Path(record[source_key]).read_bytes()).hexdigest())

    def test_generated_header_snapshot_after_build_and_refresh(self):
        self.check_generated_header_snapshot("unchanged")

    def test_generated_header_refresh_rejects_regeneration(self):
        self.check_generated_header_snapshot("regenerate")

    def test_generated_header_refresh_rejects_cache_change(self):
        self.check_generated_header_snapshot("cache")

    def test_refresh_rejects_prebuild_preset_environment_change(self):
        self.check_generated_header_snapshot("preset_environment")

    def test_refresh_rejects_ninja_command_graph_environment_change(self):
        self.check_generated_header_snapshot("ninja_environment")

    def test_refresh_requires_recorded_build_state(self):
        self.check_generated_header_snapshot("missing_state")

    def test_generated_header_refresh_rejects_header_symlink_retarget(self):
        self.check_generated_header_snapshot("header_symlink")

    def test_generated_header_refresh_rejects_prefix_symlink_retarget(self):
        self.check_generated_header_snapshot("prefix_symlink")

    def test_workflows_refresh_after_build_before_tests_or_packaging(self):
        cases = [
            ("ci.yml", "build", "macos-arm64", "test"),
            ("ci.yml", "linux-x64", "linux-x64", "test"),
            ("ci.yml", "linux-arm64", "linux-arm64", "test"),
            ("release.yml", "publish", "macos-arm64", "package"),
        ]
        for workflow, job, preset, destination in cases:
            with self.subTest(workflow=workflow, job=job):
                text = (ROOT / ".github/workflows" / workflow).read_text(encoding="utf-8")
                match = re.search(rf"(?ms)^  {re.escape(job)}:\n(.*?)(?=^  [A-Za-z0-9_-]+:\n|\Z)", text)
                self.assertIsNotNone(match, f"missing {job} job in {workflow}")
                block = match.group(1)
                build = re.search(rf"(?m)^\s*(?:run:\s*)?cmake --build --preset {re.escape(preset)}(?:\s|$)", block)
                self.assertIsNotNone(build, f"missing {preset} build")
                record = re.search(rf"(?m)^\s*(?:run:\s*)?python3 -B Build/refresh_native_provenance\.py (?=[^\n]*--record-build-state)(?=[^\n]*--preset {re.escape(preset)}(?:\s|$))(?=[^\n]*--build-dir out/{re.escape(preset)}(?:\s|$))[^\n]*", block)
                self.assertIsNotNone(record, f"missing pre-build {preset} state recording")
                self.assertLess(record.start(), build.start(), f"{preset} state recorded after build")
                refresh = re.search(rf"(?m)^\s*(?:run:\s*)?python3 -B Build/refresh_native_provenance\.py --preset {re.escape(preset)} --build-dir out/{re.escape(preset)}(?:\s|$)", block[build.end():])
                self.assertIsNotNone(refresh, f"missing guarded post-build {preset} refresh")
                refresh_start = build.end() + refresh.start()
                if destination == "test":
                    end = re.search(rf"(?m)^\s*(?:run:\s*)?(?:xvfb-run -a )?ctest --preset {re.escape(preset)}[^\n]*-L data-none", block)
                else:
                    end = re.search(r"(?m)^\s*(?:run:\s*)?cmake --build out/macos-arm64 --target hp2_macos_dmg(?:\s|$)", block)
                self.assertIsNotNone(end, f"missing {destination} for {preset}")
                self.assertLess(refresh_start, end.start(), f"{preset} refresh follows {destination}")

    def test_explicit_rustup_home(self):
        self.check_rustup_home("/toolchain path", "/toolchain path")

    def test_default_rustup_home(self):
        self.check_rustup_home("", "/configured home/.rustup")

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
