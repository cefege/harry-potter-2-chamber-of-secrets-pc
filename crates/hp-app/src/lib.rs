//! hp-app: winit event-loop ownership, CLI parsing, launcher profile
//! store, input normalization, and the egui-wgpu app shell.
//!
//! Phase-5 slice contracts are ported from the C++ oracles:
//! - `Tests/AbiTests.cpp TestCommandLineLoad` (load-slot consumption),
//! - `Tests/InputContractTests.cpp` mapping rows (frozen keysym quirks,
//!   wheel/mouse/text normalization, release-all ordering),
//! - `Tests/LauncherTests.cpp` under `HP2_LAUNCHER_TESTING=1` (catalog
//!   persistence, launch-selection contract, profile isolation, legacy
//!   migration, settings validation).
//!
//! Headless CI: window/event-loop/GPU-surface behavior cannot run without a
//! display server; those paths compile and are exercised by pure-function
//! tests only, with interactive verification deferred to manual runs.

pub mod app;
pub mod atomic_file;
pub mod cli;
pub mod error;
pub mod ini_text;
pub mod input;
pub mod input_keys;
pub mod migration;
pub mod policy;
pub mod settings;
pub mod settings_model;
pub mod store;

pub use error::AppError;
