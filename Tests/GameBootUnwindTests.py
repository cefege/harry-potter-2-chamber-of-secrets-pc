#!/usr/bin/env python3
"""Fail-closed source contract for BootHost automatic drop order."""

from pathlib import Path
import unittest


PREFIX = """use hp2_abi::engine::generated::UGameEngine;
use hp2_core::core_access::{CoreOptions, CoreProcessMode, RuntimeError, RuntimeResult};
use hp2_engine::EngineOwner;
use hp2_launcher::boot::{
    Hp2LauncherBootLeaf as Leaf, Hp2LauncherBootServices, Hp2LauncherBootValue,
};
use std::{ffi::c_void, fs, io::Write, path::PathBuf, ptr};

#[cfg(test)]
mod unwind_tests;

"""
DECLARATION = """static LOG_TO_STDOUT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub(super) struct BootHost {
    pub(super) args: Vec<String>,
    pub(super) data_root: Option<PathBuf>,
    pub(super) user_root: Option<PathBuf>,
    pub(super) system_ini: Option<PathBuf>,
    pub(super) user_ini: Option<PathBuf>,
    pub(super) override_root: Option<String>,
    pub(super) forwarded: String,
    pub(super) engine_command_line: String,
    // Reporters must detach before their owner is dropped during unwinding.
    pub(super) reporters: Option<crate::reporters::RuntimeReporters>,
    pub(super) owner: Option<EngineOwner>,
    pub(super) engine: *mut UGameEngine,
    pub(super) splash: Option<hp2_platform::launcher_splash::SplashHandle>,
    pub(super) is_client: bool,
    pub(super) lazy_load: bool,
    pub(super) is_started: bool,
    pub(super) is_guarded: bool,
    pub(super) exit_requested: bool,
    pub(super) fixed_delta: f32,
    pub(super) test_ticks: i32,
    pub(super) first_error: Option<RuntimeError>,
}

impl BootHost {"""

REPORTER = "    pub(super) reporters: Option<crate::reporters::RuntimeReporters>,\n"
OWNER = "    pub(super) owner: Option<EngineOwner>,\n"


def valid_source(source):
    # Deliberately reject block comments, extra uses and any spelling of Drop.
    # This exact source-shape contract must be reviewed when the host changes.
    return (source.startswith(PREFIX + DECLARATION) and source.count(DECLARATION) == 1
            and source.count("BootHost") == 3
            and "/*" not in source and "*/" not in source and "Drop" not in source
            and source.count(REPORTER) == 1 and source.count(OWNER) == 1)


class GameBootUnwindTests(unittest.TestCase):
    def test_reporters_are_declared_before_owner(self):
        source = (Path(__file__).resolve().parents[1] / "crates/hp2-game/src/boot_services.rs").read_text()
        self.assertTrue(valid_source(source), "BootHost source shape changed or reporters do not precede owner")

    def test_invalid_source_shapes_are_rejected(self):
        source = (Path(__file__).resolve().parents[1] / "crates/hp2-game/src/boot_services.rs").read_text()
        self.assertTrue(valid_source(source), "baseline must match before negative fixtures")
        cases = {
            "reordered": source.replace(REPORTER + OWNER, OWNER + REPORTER),
            "missing": source.replace(REPORTER, ""),
            "duplicate": source.replace(REPORTER, REPORTER + REPORTER),
            "comment fake": source.replace(REPORTER, "    // " + REPORTER.lstrip()),
            "block comment fake": source.replace(REPORTER, "    /*\n" + REPORTER + "    */\n"),
            "field cfg": source.replace(REPORTER, "    #[cfg(test)]\n" + REPORTER),
            "cfg attr": source.replace(REPORTER, "    #[cfg_attr(test, allow(dead_code))]\n" + REPORTER),
            "struct cfg": source.replace("pub(super) struct BootHost {", "#[cfg(test)]\npub(super) struct BootHost {"),
            "changed representation": source.replace(REPORTER, REPORTER.replace("Option<", "Box<")),
            "duplicate declaration": source + "\n" + DECLARATION,
            "custom drop": source + "\nimpl Drop for BootHost { fn drop(&mut self) {} }\n",
        }
        for name, candidate in cases.items():
            with self.subTest(name=name):
                self.assertNotEqual(candidate, source, "fixture must change the source")
                self.assertFalse(valid_source(candidate), "unsafe source shape must fail closed")


if __name__ == "__main__":
    unittest.main()
